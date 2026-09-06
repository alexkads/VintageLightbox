//! A fronteira da interface com o pós-venda: `Result<_, String>`, como os outros.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use domain::services::pos_venda::{
    Galeria, GaleriaDoPainel, LinkDeAcesso, MudancaDaFoto, NovaGaleria, PosVendaApi, Produto,
    Sessao,
};
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

    /// As galerias que já existem — para subir numa delas em vez de criar uma
    /// por leva.
    pub async fn galerias(&self, sessao: &Sessao) -> Result<Vec<GaleriaDoPainel>, String> {
        self.api.galerias(sessao).await.map_err(frase)
    }

    /// Cria uma sessão fotográfica vazia.
    ///
    /// 🔑 **Separado de [`Self::publicar`]**, que também cria uma: publicar é
    /// "sobe estas fotos numa galeria nova", e a lista precisa de "abre a
    /// galeria agora, as fotos vêm depois" — que é a ordem do fluxo do balcão,
    /// onde o cadastro do cliente acontece antes de a triagem terminar.
    pub async fn criar_galeria(
        &self,
        sessao: &Sessao,
        nova: &NovaGaleria,
    ) -> Result<Galeria, String> {
        self.api.criar_galeria(sessao, nova).await.map_err(frase)
    }

    /// O que o balcão registra numa foto que já está no site: o estado, a
    /// negociação, a nota.
    pub async fn mudar_foto(
        &self,
        sessao: &Sessao,
        foto_id: &str,
        mudanca: &MudancaDaFoto,
    ) -> Result<(), String> {
        self.api
            .mudar_foto(sessao, foto_id, mudanca)
            .await
            .map_err(frase)
    }

    /// Tira a foto do storage — o que zerar a classificação faz.
    pub async fn remover_foto(&self, sessao: &Sessao, foto_id: &str) -> Result<(), String> {
        self.api.remover_foto(sessao, foto_id).await.map_err(frase)
    }

    /// O link que entra sem senha, para mandar ao cliente.
    pub async fn link_da_galeria(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
    ) -> Result<LinkDeAcesso, String> {
        self.api
            .link_da_galeria(sessao, galeria_id)
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
