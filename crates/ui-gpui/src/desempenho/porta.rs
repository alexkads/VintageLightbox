//! A porta entre o painel de Desempenho e o banco — o padrão da casa (ver
//! CLAUDE.md, "Portas para o mundo assíncrono").
//!
//! 🔑 **Tudo em segundo plano.** Cada pedido vira uma tarefa no tokio capturado
//! no `main` e volta por um `oneshot`, que a tela espera numa tarefa dela — o
//! quadro nunca espera o SQLite. Gravar é um lote só (`SqliteDesempenho`).
//!
//! Os travamentos que o vigia guardou em disco entram no banco por
//! [`DepositoDeDesempenho::importar_pendentes`], na primeira vez que o painel
//! abre depois deles — e o arquivo só sai depois de a gravação confirmar.

use std::path::PathBuf;
use std::sync::Arc;

use domain::desempenho::{CabecalhoDaSessao, LinhaDeComparacao, SessaoDeDesempenho};
use domain::repositories::DesempenhoRepository;
use tokio::sync::oneshot;

pub trait DepositoDeDesempenho: Send + Sync + 'static {
    fn salvar(&self, sessao: Arc<SessaoDeDesempenho>) -> oneshot::Receiver<Result<(), String>>;
    fn listar(&self) -> oneshot::Receiver<Vec<CabecalhoDaSessao>>;
    fn carregar(&self, id: String) -> oneshot::Receiver<Option<SessaoDeDesempenho>>;
    fn comparar(&self, operacao: String) -> oneshot::Receiver<Vec<LinhaDeComparacao>>;
    fn apagar(&self, id: String) -> oneshot::Receiver<Result<(), String>>;
    /// Leva ao banco as sessões desta pasta (travamentos, ou um arquivo
    /// exportado de outra máquina) e responde quantas entraram. Com
    /// `apagar_depois`, cada arquivo sai depois de gravado.
    fn importar(&self, arquivos: Vec<PathBuf>, apagar_depois: bool) -> oneshot::Receiver<usize>;
}

/// O depósito de verdade: o repositório SQLite numa `Handle` do tokio.
pub struct DepositoNoBanco {
    repositorio: Arc<dyn DesempenhoRepository>,
    tokio: tokio::runtime::Handle,
}

impl DepositoNoBanco {
    pub fn novo(repositorio: Arc<dyn DesempenhoRepository>, tokio: tokio::runtime::Handle) -> Self {
        Self { repositorio, tokio }
    }
}

impl DepositoDeDesempenho for DepositoNoBanco {
    fn salvar(&self, sessao: Arc<SessaoDeDesempenho>) -> oneshot::Receiver<Result<(), String>> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let _ = envia.send(repositorio.salvar(&sessao).await.map_err(|e| e.to_string()));
        });
        recebe
    }

    fn listar(&self) -> oneshot::Receiver<Vec<CabecalhoDaSessao>> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let _ = envia.send(repositorio.listar(200).await.unwrap_or_else(|e| {
                eprintln!("⚠️ [Desempenho] não listou as sessões: {e}");
                Vec::new()
            }));
        });
        recebe
    }

    fn carregar(&self, id: String) -> oneshot::Receiver<Option<SessaoDeDesempenho>> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let _ = envia.send(repositorio.carregar(&id).await.ok().flatten());
        });
        recebe
    }

    fn comparar(&self, operacao: String) -> oneshot::Receiver<Vec<LinhaDeComparacao>> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let _ = envia.send(repositorio.comparar(&operacao).await.unwrap_or_default());
        });
        recebe
    }

    fn apagar(&self, id: String) -> oneshot::Receiver<Result<(), String>> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let _ = envia.send(repositorio.apagar(&id).await.map_err(|e| e.to_string()));
        });
        recebe
    }

    fn importar(&self, arquivos: Vec<PathBuf>, apagar_depois: bool) -> oneshot::Receiver<usize> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let lidas = tokio::task::spawn_blocking(move || ler_arquivos(&arquivos))
                .await
                .unwrap_or_default();
            let mut entraram = 0;
            for (caminho, sessoes) in lidas {
                let mut todas = true;
                for sessao in sessoes {
                    match repositorio.salvar(&sessao).await {
                        Ok(()) => entraram += 1,
                        Err(e) => {
                            todas = false;
                            eprintln!("⚠️ [Desempenho] {caminho:?} não entrou no banco: {e}");
                        }
                    }
                }
                if todas && apagar_depois {
                    let _ = std::fs::remove_file(&caminho);
                }
            }
            let _ = envia.send(entraram);
        });
        recebe
    }
}

/// Um arquivo pode ter uma sessão (a do vigia) ou uma lista (a exportação).
pub fn ler_arquivos(arquivos: &[PathBuf]) -> Vec<(PathBuf, Vec<SessaoDeDesempenho>)> {
    arquivos
        .iter()
        .filter_map(|caminho| {
            let bytes = std::fs::read(caminho).ok()?;
            let sessoes = serde_json::from_slice::<Vec<SessaoDeDesempenho>>(&bytes)
                .or_else(|_| serde_json::from_slice::<SessaoDeDesempenho>(&bytes).map(|s| vec![s]))
                .ok()?;
            Some((caminho.clone(), sessoes))
        })
        .collect()
}

/// Os arquivos que o vigia deixou.
pub fn travamentos_pendentes() -> Vec<PathBuf> {
    let Some(pasta) = super::vigia::pasta_dos_pendentes() else {
        return Vec::new();
    };
    super::vigia::pendentes_em(&pasta)
        .into_iter()
        .map(|(p, _)| p)
        .collect()
}

#[cfg(test)]
pub mod mentira {
    //! O depósito em memória: o que foi gravado, na ordem.
    use super::*;
    use parking_lot::Mutex;

    #[derive(Default)]
    pub struct DepositoDeMentira {
        pub gravadas: Mutex<Vec<SessaoDeDesempenho>>,
    }

    impl DepositoDeDesempenho for DepositoDeMentira {
        fn salvar(&self, sessao: Arc<SessaoDeDesempenho>) -> oneshot::Receiver<Result<(), String>> {
            let (e, r) = oneshot::channel();
            let mut g = self.gravadas.lock();
            g.retain(|s| s.cabecalho.id != sessao.cabecalho.id);
            g.push((*sessao).clone());
            let _ = e.send(Ok(()));
            r
        }
        fn listar(&self) -> oneshot::Receiver<Vec<CabecalhoDaSessao>> {
            let (e, r) = oneshot::channel();
            let _ = e.send(
                self.gravadas
                    .lock()
                    .iter()
                    .rev()
                    .map(|s| s.cabecalho.clone())
                    .collect(),
            );
            r
        }
        fn carregar(&self, id: String) -> oneshot::Receiver<Option<SessaoDeDesempenho>> {
            let (e, r) = oneshot::channel();
            let _ = e.send(
                self.gravadas
                    .lock()
                    .iter()
                    .find(|s| s.cabecalho.id == id)
                    .cloned(),
            );
            r
        }
        fn comparar(&self, operacao: String) -> oneshot::Receiver<Vec<LinhaDeComparacao>> {
            let (e, r) = oneshot::channel();
            let linhas = self
                .gravadas
                .lock()
                .iter()
                .flat_map(|s| {
                    s.metricas
                        .iter()
                        .filter(|m| m.operacao == operacao)
                        .map(|m| LinhaDeComparacao {
                            sessao_id: s.cabecalho.id.clone(),
                            iniciada_em: s.cabecalho.iniciada_em.clone(),
                            sistema: s.cabecalho.sistema.clone(),
                            gpu_nome: s.cabecalho.gpu_nome.clone(),
                            gpu_backend: s.cabecalho.gpu_backend.clone(),
                            taxa_do_monitor_hz: s.cabecalho.taxa_do_monitor_hz,
                            metrica: m.clone(),
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            let _ = e.send(linhas);
            r
        }
        fn apagar(&self, id: String) -> oneshot::Receiver<Result<(), String>> {
            let (e, r) = oneshot::channel();
            self.gravadas.lock().retain(|s| s.cabecalho.id != id);
            let _ = e.send(Ok(()));
            r
        }
        fn importar(&self, arquivos: Vec<PathBuf>, _apagar: bool) -> oneshot::Receiver<usize> {
            let (e, r) = oneshot::channel();
            let mut n = 0;
            for (_, sessoes) in ler_arquivos(&arquivos) {
                for s in sessoes {
                    let _ = self.salvar(Arc::new(s));
                    n += 1;
                }
            }
            let _ = e.send(n);
            r
        }
    }
}
