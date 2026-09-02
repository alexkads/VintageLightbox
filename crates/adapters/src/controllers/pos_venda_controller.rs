//! A fronteira da interface com o pós-venda: `Result<_, String>`, como os outros.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use domain::services::pos_venda::{Galeria, NovaGaleria, PosVendaApi, Produto, Sessao};
use domain::value_objects::PhotoId;
use domain::DomainError;
use use_cases::pos_venda::{Pedido, Progresso, PublicarNoPosVendaUseCase};

pub struct PosVendaController {
    api: Arc<dyn PosVendaApi>,
    publicar: Arc<PublicarNoPosVendaUseCase>,
}

impl PosVendaController {
    pub fn new(api: Arc<dyn PosVendaApi>, publicar: Arc<PublicarNoPosVendaUseCase>) -> Self {
        Self { api, publicar }
    }

    /// `Err` com a frase que a tela mostra. Credencial recusada e rede caída
    /// chegam como frases diferentes — é o que a variante do domínio existe
    /// para permitir.
    pub async fn entrar(&self, email: &str, senha: &str) -> Result<Sessao, String> {
        self.api.entrar(email, senha).await.map_err(frase)
    }

    pub async fn produtos(&self, sessao: &Sessao) -> Result<Vec<Produto>, String> {
        self.api.produtos(sessao).await.map_err(frase)
    }

    /// `fotos` são os ids como a grade os conhece, na ordem em que devem
    /// aparecer no site.
    pub async fn publicar(
        &self,
        sessao: Sessao,
        galeria: NovaGaleria,
        fotos: Vec<String>,
        canal: Sender<Progresso>,
    ) -> Result<Galeria, String> {
        let fotos = fotos
            .iter()
            .map(|id| PhotoId::from_string(id).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;

        self.publicar
            .execute(
                Pedido {
                    sessao,
                    galeria,
                    fotos,
                },
                canal,
            )
            .await
            .map_err(frase)
    }
}

fn frase(erro: DomainError) -> String {
    match erro {
        DomainError::AcessoRecusado => "e-mail ou senha recusados pelo site".to_string(),
        outro => outro.to_string(),
    }
}
