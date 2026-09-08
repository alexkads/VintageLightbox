//! Gerar a folha e entregá-la — ao disco, ou ao sistema.
//!
//! 🚨 **Os dois botões que esta porta liga anunciavam *"coming soon"***, nos dois
//! apps. Era o exemplo canônico do critério 5 do objetivo: botão que promete e
//! não faz é pior que botão ausente.
//!
//! Mesma forma das outras portas da casa: o caminho de baixo é `async` do tokio,
//! o GPUI não roda futuros dele, e o `Handle` é capturado no `main`.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use domain::repositories::PhotoRepository;
use domain::value_objects::{ExportOptions, PhotoId};
use infrastructure::image_exporter::ImageExporterImpl;

use super::pagina::Leiaute;

/// O que a tela precisa saber depois de pedir a folha.
#[derive(Debug, Clone, PartialEq)]
pub enum Recado {
    /// Saiu, e onde. `imprimindo` diz se foi entregue ao sistema.
    Pronta {
        onde: PathBuf,
        imprimindo: bool,
    },
    Falhou(String),
}

/// Para onde a folha vai.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destino {
    /// Um arquivo escolhido por quem clicou.
    Arquivo,
    /// Um arquivo temporário, entregue ao diálogo de impressão do sistema.
    Impressora,
}

pub trait Folha: Send + Sync + 'static {
    /// Monta o PDF das fotos, na ordem dada, e o entrega.
    fn gerar(
        &self,
        fotos: Vec<String>,
        leiaute: Leiaute,
        destino: Destino,
        arquivo: Option<PathBuf>,
        canal: Sender<Recado>,
    );
}

pub struct FolhaDoDisco {
    repositorio: Arc<dyn PhotoRepository>,
    exportador: Arc<ImageExporterImpl>,
    tokio: tokio::runtime::Handle,
}

impl FolhaDoDisco {
    pub fn nova(
        repositorio: Arc<dyn PhotoRepository>,
        exportador: Arc<ImageExporterImpl>,
        tokio: tokio::runtime::Handle,
    ) -> Self {
        Self {
            repositorio,
            exportador,
            tokio,
        }
    }
}

impl Folha for FolhaDoDisco {
    fn gerar(
        &self,
        fotos: Vec<String>,
        leiaute: Leiaute,
        destino: Destino,
        arquivo: Option<PathBuf>,
        canal: Sender<Recado>,
    ) {
        let repositorio = self.repositorio.clone();
        let exportador = self.exportador.clone();

        self.tokio.spawn(async move {
            let recado = montar(&repositorio, &exportador, fotos, leiaute, destino, arquivo).await;
            let _ = canal.send(recado);
        });
    }
}

async fn montar(
    repositorio: &Arc<dyn PhotoRepository>,
    exportador: &Arc<ImageExporterImpl>,
    fotos: Vec<String>,
    leiaute: Leiaute,
    destino: Destino,
    arquivo: Option<PathBuf>,
) -> Recado {
    // 🔑 **As fotos entram reveladas e enquadradas, pelo mesmo caminho da
    // exportação.** Imprimir o arquivo original seria o defeito que a exportação
    // tinha até hoje de manhã: a tela mostrando uma coisa e o papel saindo outra.
    let mut imagens = Vec::with_capacity(fotos.len());
    for id in fotos {
        let Ok(photo_id) = PhotoId::from_string(&id) else {
            return Recado::Falhou(format!("id de foto inválido: {id}"));
        };
        let foto = match repositorio.find_by_id(&photo_id).await {
            Ok(Some(f)) => f,
            Ok(None) => return Recado::Falhou(format!("foto não encontrada: {id}")),
            Err(erro) => return Recado::Falhou(erro.to_string()),
        };

        // ⚠️ **Sem marca d'água e sem redimensionar**: a folha é para o papel, e
        // reduzir aqui jogaria fora justamente a resolução que a impressão usa.
        match exportador.renderizar(&foto, &ExportOptions::default()) {
            Ok(imagem) => imagens.push(imagem),
            Err(erro) => return Recado::Falhou(erro.to_string()),
        }
    }

    let bytes = match super::pdf::gerar(&leiaute, &imagens) {
        Ok(b) => b,
        Err(erro) => return Recado::Falhou(erro),
    };

    let onde = match (destino, arquivo) {
        (Destino::Arquivo, Some(caminho)) => caminho,
        (Destino::Arquivo, None) => return Recado::Falhou("nenhum arquivo escolhido".into()),
        // ⚠️ O temporário leva o nome do app: quem abrir a janela de impressão
        // vê de onde a folha veio, em vez de um nome aleatório.
        (Destino::Impressora, _) => std::env::temp_dir().join("VintageLightbox — folha.pdf"),
    };

    if let Err(erro) = tokio::fs::write(&onde, &bytes).await {
        return Recado::Falhou(format!("não foi possível gravar o PDF: {erro}"));
    }

    let imprimindo = destino == Destino::Impressora;
    if imprimindo {
        // 🔑 **Quem imprime é o sistema, e é o certo.** O diálogo dele tem
        // impressora, bandeja, qualidade e "salvar como PDF" — e muda com a
        // impressora instalada. Um diálogo próprio seria uma versão pior de algo
        // que já existe e que quem imprime conhece.
        if let Err(erro) = abrir_no_sistema(&onde) {
            return Recado::Falhou(erro);
        }
    }

    Recado::Pronta { onde, imprimindo }
}

#[cfg(target_os = "macos")]
fn abrir_no_sistema(caminho: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(caminho)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("não foi possível abrir o PDF: {e}"))
}

#[cfg(not(target_os = "macos"))]
fn abrir_no_sistema(_caminho: &std::path::Path) -> Result<(), String> {
    // ⚠️ O produto declara macOS e Windows; o caminho do Windows entra quando
    // houver onde conferi-lo. Falhar aqui é melhor que gravar o PDF e não abrir
    // nada, que pareceria "o botão não faz nada".
    Err("entregar ao sistema de impressão só está ligado no macOS".to_string())
}

/// A folha de mentira: guarda o que foi pedido e responde na hora.
#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct FolhaDeMentira {
        pub pedidos: Mutex<Vec<(Vec<String>, Leiaute, Destino)>>,
    }

    impl FolhaDeMentira {
        pub fn pedidos(&self) -> Vec<(Vec<String>, Leiaute, Destino)> {
            self.pedidos.lock().expect("os pedidos").clone()
        }
    }

    impl Folha for FolhaDeMentira {
        fn gerar(
            &self,
            fotos: Vec<String>,
            leiaute: Leiaute,
            destino: Destino,
            arquivo: Option<PathBuf>,
            canal: Sender<Recado>,
        ) {
            self.pedidos
                .lock()
                .expect("os pedidos")
                .push((fotos, leiaute, destino));
            let _ = canal.send(Recado::Pronta {
                onde: arquivo.unwrap_or_else(|| PathBuf::from("/tmp/folha.pdf")),
                imprimindo: destino == Destino::Impressora,
            });
        }
    }
}
