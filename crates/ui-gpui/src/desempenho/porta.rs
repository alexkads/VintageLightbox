//! A porta entre o painel de Desempenho e os bancos — o local e o do servidor.
//! O padrão da casa (ver CLAUDE.md, "Portas para o mundo assíncrono").
//!
//! # Dois bancos
//!
//! 🔑 **O SQLite do computador e o Postgres do servidor.** Até a 0.1.29 a sessão
//! ficava só no computador, e a medição de um balcão Linux só chegava a quem
//! comparava exportada num arquivo. O dono pediu (27/09): *"O ideal era salvar
//! no banco de dados"*. Agora o "Salvar no banco" grava nos dois:
//!
//! - no **computador** primeiro: é o que funciona sem rede e sem conta;
//! - no **servidor** (`POST /app-desktop/desempenho`, a mesma trilha da
//!   telemetria do app): lá ficam as sessões de todos os balcões, e a aba
//!   Comparar põe a mesma operação lado a lado por sistema.
//!
//! Sem rede ou sem conta, a sessão vai para uma **fila em disco**
//! (`~/.vintagelightbox/desempenho/para-enviar/`) e sobe em [`sincronizar`],
//! que roda quando a janela de Desempenho abre. A mesma sincronização manda as
//! sessões do banco local que o servidor ainda não tem — as gravadas antes de
//! existir o envio.
//!
//! As listas (Sessões, Comparar) vêm do servidor, com todas as máquinas; sem
//! ele, do computador, e a tela diz isso.
//!
//! [`sincronizar`]: DepositoDeDesempenho::sincronizar

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use domain::desempenho::{
    CabecalhoDaSessao, LinhaDeComparacao, MetricaDeDesempenho, SessaoDeDesempenho,
};
use domain::repositories::DesempenhoRepository;
use tokio::sync::oneshot;

/// Onde a sessão ficou depois do "Salvar no banco".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destino {
    /// No computador e no servidor.
    Servidor,
    /// Só no computador, com o motivo; a fila manda depois.
    SoNesteComputador(String),
}

/// A lista das sessões, e de onde ela veio.
#[derive(Debug, Clone, Default)]
pub struct Lista {
    pub sessoes: Vec<CabecalhoDaSessao>,
    /// A lista é a do servidor (todas as máquinas), mais as locais que ainda
    /// não subiram. `false`: só as deste computador.
    pub do_servidor: bool,
}

pub trait DepositoDeDesempenho: Send + Sync + 'static {
    fn salvar(&self, sessao: Arc<SessaoDeDesempenho>)
        -> oneshot::Receiver<Result<Destino, String>>;
    fn listar(&self) -> oneshot::Receiver<Lista>;
    fn carregar(&self, id: String) -> oneshot::Receiver<Option<SessaoDeDesempenho>>;
    fn comparar(&self, operacao: String) -> oneshot::Receiver<Vec<LinhaDeComparacao>>;
    fn apagar(&self, id: String) -> oneshot::Receiver<Result<(), String>>;
    /// Leva ao banco as sessões destes arquivos (travamentos, ou um arquivo
    /// exportado de outra máquina) e responde quantas entraram. Com
    /// `apagar_depois`, cada arquivo sai depois de gravado.
    fn importar(&self, arquivos: Vec<PathBuf>, apagar_depois: bool) -> oneshot::Receiver<usize>;
    /// Manda ao servidor a fila e as sessões locais que ele ainda não tem.
    /// Responde quantas subiram.
    fn sincronizar(&self) -> oneshot::Receiver<usize>;
}

// ── O servidor ─────────────────────────────────────────────────────────────

const CAMINHO: &str = "/app-desktop/desempenho";

/// A pasta da fila de envio.
pub fn pasta_da_fila() -> Option<PathBuf> {
    crate::atualizacao::compilar::casa().map(|c| c.join("desempenho").join("para-enviar"))
}

/// O corpo que o servidor espera: o id, a máquina e a sessão inteira.
pub fn corpo_do_envio(sessao: &SessaoDeDesempenho, maquina_id: &str) -> Option<Vec<u8>> {
    serde_json::to_vec(&serde_json::json!({
        "id": sessao.cabecalho.id,
        "maquina_id": maquina_id,
        "sessao": sessao,
    }))
    .ok()
}

/// Manda uma sessão. `Ok` gravada; `Err` com o motivo (sem conta, sem rede, o
/// que o servidor respondeu).
async fn enviar(sessao: &SessaoDeDesempenho) -> Result<(), String> {
    let maquina = &crate::telemetria::maquina::deste().id;
    let corpo = corpo_do_envio(sessao, maquina).ok_or("não deu para montar o envio")?;
    match crate::telemetria::chamar("POST", CAMINHO, Some(corpo)).await {
        Some((status, _)) if (200..300).contains(&status) => Ok(()),
        Some((status, bytes)) => Err(format!(
            "o servidor respondeu {status}: {}",
            String::from_utf8_lossy(&bytes)
                .chars()
                .take(200)
                .collect::<String>()
        )),
        None => Err("sem conexão com o servidor (ou sem conta aberta)".into()),
    }
}

async fn ler_json<T: serde::de::DeserializeOwned>(caminho: &str) -> Option<T> {
    match crate::telemetria::chamar("GET", caminho, None).await {
        Some((200, bytes)) => serde_json::from_slice(&bytes).ok(),
        _ => None,
    }
}

#[derive(serde::Deserialize)]
struct ResumoDoServidor {
    id: String,
    #[serde(default)]
    cabecalho: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct SessaoDoServidor {
    sessao: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct OperacaoDoServidor {
    id: String,
    #[serde(default)]
    cabecalho: serde_json::Value,
    #[serde(default)]
    metricas: serde_json::Value,
}

/// As linhas de comparação que o servidor devolve, na forma da tela.
fn linhas_do_servidor(lista: Vec<OperacaoDoServidor>) -> Vec<LinhaDeComparacao> {
    lista
        .into_iter()
        .flat_map(|s| {
            let c: CabecalhoDaSessao = serde_json::from_value(s.cabecalho).unwrap_or_default();
            let metricas: Vec<MetricaDeDesempenho> =
                serde_json::from_value(s.metricas).unwrap_or_default();
            let id = s.id;
            metricas.into_iter().map(move |metrica| LinhaDeComparacao {
                sessao_id: id.clone(),
                iniciada_em: c.iniciada_em.clone(),
                sistema: c.sistema.clone(),
                gpu_nome: c.gpu_nome.clone(),
                gpu_backend: c.gpu_backend.clone(),
                taxa_do_monitor_hz: c.taxa_do_monitor_hz,
                metrica,
            })
        })
        .collect()
}

// ── A fila em disco ────────────────────────────────────────────────────────

fn para_a_fila(sessao: &SessaoDeDesempenho) {
    if let Some(pasta) = pasta_da_fila() {
        super::vigia::gravar_em(&pasta, sessao);
    }
}

fn fila() -> Vec<(PathBuf, SessaoDeDesempenho)> {
    pasta_da_fila()
        .map(|p| super::vigia::pendentes_em(&p))
        .unwrap_or_default()
}

// ── O depósito de verdade ──────────────────────────────────────────────────

/// O SQLite do computador numa `Handle` do tokio, e o servidor pela conta da
/// telemetria.
pub struct DepositoNoBanco {
    repositorio: Arc<dyn DesempenhoRepository>,
    tokio: tokio::runtime::Handle,
}

impl DepositoNoBanco {
    pub fn novo(repositorio: Arc<dyn DesempenhoRepository>, tokio: tokio::runtime::Handle) -> Self {
        Self { repositorio, tokio }
    }
}

/// Grava no computador e tenta o servidor; sem ele, a fila.
async fn salvar_nos_dois(
    repositorio: &Arc<dyn DesempenhoRepository>,
    sessao: &SessaoDeDesempenho,
) -> Result<Destino, String> {
    repositorio
        .salvar(sessao)
        .await
        .map_err(|e| e.to_string())?;
    match enviar(sessao).await {
        Ok(()) => Ok(Destino::Servidor),
        Err(motivo) => {
            para_a_fila(sessao);
            Ok(Destino::SoNesteComputador(motivo))
        }
    }
}

impl DepositoDeDesempenho for DepositoNoBanco {
    fn salvar(
        &self,
        sessao: Arc<SessaoDeDesempenho>,
    ) -> oneshot::Receiver<Result<Destino, String>> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let _ = envia.send(salvar_nos_dois(&repositorio, &sessao).await);
        });
        recebe
    }

    fn listar(&self) -> oneshot::Receiver<Lista> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let locais = repositorio.listar(200).await.unwrap_or_else(|e| {
                eprintln!("⚠️ [Desempenho] não listou as sessões locais: {e}");
                Vec::new()
            });
            let lista = match ler_json::<Vec<ResumoDoServidor>>(&format!("{CAMINHO}?limite=200"))
                .await
            {
                Some(do_servidor) => {
                    let ids: HashSet<String> = do_servidor.iter().map(|s| s.id.clone()).collect();
                    let mut sessoes: Vec<CabecalhoDaSessao> = do_servidor
                        .into_iter()
                        .filter_map(|s| {
                            let mut c: CabecalhoDaSessao =
                                serde_json::from_value(s.cabecalho).ok()?;
                            c.id = s.id;
                            Some(c)
                        })
                        .collect();
                    // As locais que ainda não subiram também aparecem.
                    sessoes.extend(locais.into_iter().filter(|c| !ids.contains(&c.id)));
                    sessoes.sort_by(|a, b| b.iniciada_em.cmp(&a.iniciada_em));
                    Lista {
                        sessoes,
                        do_servidor: true,
                    }
                }
                None => Lista {
                    sessoes: locais,
                    do_servidor: false,
                },
            };
            let _ = envia.send(lista);
        });
        recebe
    }

    fn carregar(&self, id: String) -> oneshot::Receiver<Option<SessaoDeDesempenho>> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let local = repositorio.carregar(&id).await.ok().flatten();
            let sessao = match local {
                Some(s) => Some(s),
                None => ler_json::<SessaoDoServidor>(&format!("{CAMINHO}/{id}"))
                    .await
                    .and_then(|s| serde_json::from_value(s.sessao).ok()),
            };
            let _ = envia.send(sessao);
        });
        recebe
    }

    fn comparar(&self, operacao: String) -> oneshot::Receiver<Vec<LinhaDeComparacao>> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let caminho = format!(
                "{CAMINHO}/comparar?operacao={}",
                crate::telemetria::maquina::na_query(&operacao)
            );
            let linhas = match ler_json::<Vec<OperacaoDoServidor>>(&caminho).await {
                Some(lista) => linhas_do_servidor(lista),
                None => repositorio.comparar(&operacao).await.unwrap_or_default(),
            };
            let _ = envia.send(linhas);
        });
        recebe
    }

    fn apagar(&self, id: String) -> oneshot::Receiver<Result<(), String>> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let local = repositorio.apagar(&id).await.map_err(|e| e.to_string());
            if let Some(pasta) = pasta_da_fila() {
                let _ = std::fs::remove_file(pasta.join(format!("{id}.json")));
            }
            // Apagar do servidor é de quem vê o diagnóstico do sistema: para
            // o operador, o servidor responde 403 e a sessão fica lá.
            let resposta = match crate::telemetria::chamar("DELETE", &format!("{CAMINHO}/{id}"), None).await
            {
                Some((s, _)) if (200..300).contains(&s) || s == 404 => local,
                Some((403, _)) => Err(
                    "apagada deste computador; do servidor, só quem vê o diagnóstico do sistema apaga"
                        .into(),
                ),
                Some((s, _)) => Err(format!("apagada deste computador; o servidor respondeu {s}")),
                None => local,
            };
            let _ = envia.send(resposta);
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
                    match salvar_nos_dois(&repositorio, &sessao).await {
                        Ok(_) => entraram += 1,
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

    fn sincronizar(&self) -> oneshot::Receiver<usize> {
        let (envia, recebe) = oneshot::channel();
        let repositorio = self.repositorio.clone();
        self.tokio.spawn(async move {
            let mut subiram = 0;
            // A fila primeiro: o que já tentou subir e não conseguiu.
            for (arquivo, sessao) in tokio::task::spawn_blocking(fila).await.unwrap_or_default() {
                match enviar(&sessao).await {
                    Ok(()) => {
                        subiram += 1;
                        let _ = std::fs::remove_file(arquivo);
                    }
                    // Sem servidor agora: o resto da fila espera a próxima.
                    Err(_) => {
                        let _ = envia.send(subiram);
                        return;
                    }
                }
            }
            // Depois, as locais que o servidor não tem — as gravadas antes de
            // o envio existir (0.1.26–0.1.29).
            let Some(do_servidor) =
                ler_json::<Vec<ResumoDoServidor>>(&format!("{CAMINHO}?limite=500")).await
            else {
                let _ = envia.send(subiram);
                return;
            };
            let no_servidor: HashSet<String> = do_servidor.into_iter().map(|s| s.id).collect();
            for c in repositorio.listar(500).await.unwrap_or_default() {
                if no_servidor.contains(&c.id) {
                    continue;
                }
                let Some(sessao) = repositorio.carregar(&c.id).await.ok().flatten() else {
                    continue;
                };
                if enviar(&sessao).await.is_ok() {
                    subiram += 1;
                }
            }
            let _ = envia.send(subiram);
        });
        recebe
    }
}

/// Um arquivo pode ter uma sessão (a do vigia, a da fila) ou uma lista (a
/// exportação).
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
mod testes {
    use super::*;
    use domain::desempenho::{CabecalhoDaSessao, MetricaDeDesempenho};

    /// O corpo do envio é o que o servidor lê: id, máquina e a sessão com
    /// cabeçalho (sistema e início) e a lista de métricas.
    #[test]
    fn o_corpo_do_envio_e_o_que_o_servidor_le() {
        let sessao = SessaoDeDesempenho {
            cabecalho: CabecalhoDaSessao {
                id: "5d4f4149-0000-0000-0000-000000000000".into(),
                sistema: "linux".into(),
                iniciada_em: "2026-09-27T22:47:14+00:00".into(),
                ..Default::default()
            },
            metricas: vec![MetricaDeDesempenho {
                operacao: "rolagem".into(),
                ..Default::default()
            }],
            quadros: vec![],
        };
        let corpo: serde_json::Value =
            serde_json::from_slice(&corpo_do_envio(&sessao, "maquina-1").unwrap()).unwrap();
        assert_eq!(corpo["id"], "5d4f4149-0000-0000-0000-000000000000");
        assert_eq!(corpo["maquina_id"], "maquina-1");
        assert_eq!(corpo["sessao"]["cabecalho"]["sistema"], "linux");
        assert!(corpo["sessao"]["metricas"].is_array());
    }

    /// A comparação do servidor vira as linhas da tela, uma por métrica.
    #[test]
    fn a_comparacao_do_servidor_vira_linhas() {
        let lista: Vec<OperacaoDoServidor> = serde_json::from_value(serde_json::json!([{
            "id": "s1", "maquina_id": "m", "maquina_nome": "balcão",
            "cabecalho": {"sistema": "windows", "gpu_nome": "Vega 11", "taxa_do_monitor_hz": 59.0,
                          "campo_que_nao_existe_aqui": 1},
            "metricas": [
                {"operacao": "arrasto_de_slider", "etapa": "recorte_na_cpu", "p95_ms": 259.0},
                {"operacao": "arrasto_de_slider", "etapa": "intervalo_do_quadro", "p95_ms": 291.0}
            ]
        }]))
        .unwrap();
        let linhas = linhas_do_servidor(lista);
        assert_eq!(linhas.len(), 2);
        assert_eq!(linhas[0].sistema, "windows");
        assert_eq!(linhas[0].metrica.etapa, "recorte_na_cpu");
        assert_eq!(linhas[1].metrica.p95_ms, 291.0);
    }
}
