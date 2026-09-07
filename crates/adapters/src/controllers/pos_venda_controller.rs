//! A fronteira da interface com o pós-venda: `Result<_, String>`, como os outros.

use std::sync::Arc;

use domain::services::pos_venda::{
    EstadoNoBalcao, Galeria, GaleriaAberta, GaleriaDoPainel, LinkDeAcesso, MudancaDaFoto,
    NovaGaleria, PosVendaApi, Produto, Sessao,
};
use domain::value_objects::PhotoId;
use domain::DomainError;
use use_cases::pos_venda::PublicarNoPosVendaUseCase;

pub struct PosVendaController {
    api: Arc<dyn PosVendaApi>,
    publicar: Arc<PublicarNoPosVendaUseCase>,
}

impl PosVendaController {
    pub fn new(api: Arc<dyn PosVendaApi>, publicar: Arc<PublicarNoPosVendaUseCase>) -> Self {
        Self { api, publicar }
    }

    /// Autoriza este computador pelo navegador. Bloqueia até o operador
    /// decidir na outra janela — a tela mostra "aguardando o navegador" enquanto
    /// isso.
    ///
    /// `Err` com a frase que a tela mostra. Recusa e rede caída chegam como
    /// frases diferentes — é o que a variante do domínio existe para permitir.
    pub async fn autorizar(&self) -> Result<Sessao, String> {
        self.api.autorizar_pelo_navegador().await.map_err(frase)
    }

    /// A sessão de ontem, se ainda valer. `Ok(None)` é "precisa autorizar".
    pub async fn retomar(&self) -> Result<Option<Sessao>, String> {
        self.api.retomar_sessao().await.map_err(frase)
    }

    /// Esquece a sessão — o "sair" da tela.
    pub async fn sair(&self) {
        self.api.sair().await;
    }

    pub async fn produtos(&self, sessao: &Sessao) -> Result<Vec<Produto>, String> {
        self.api.produtos(sessao).await.map_err(frase)
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

    /// Sobe **uma** foto para uma galeria que já existe — o passo 3 do fluxo.
    pub async fn enviar_uma(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        foto_id: &str,
        ordem: u32,
        estado: Option<EstadoNoBalcao>,
    ) -> Result<String, String> {
        let id = PhotoId::from_string(foto_id).map_err(|e| e.to_string())?;
        self.publicar
            .enviar_uma(sessao, galeria_id, &id, ordem, estado)
            .await
    }

    /// Sobe um **arquivo do disco** para a sessão, sem passar pelo catálogo.
    pub async fn enviar_arquivo(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        caminho: &str,
        ordem: u32,
        estado: EstadoNoBalcao,
    ) -> Result<String, String> {
        self.publicar
            .enviar_arquivo(sessao, galeria_id, caminho, ordem, estado)
            .await
    }

    /// Tira a foto do storage — o que zerar a classificação faz.
    pub async fn remover_do_site(&self, sessao: &Sessao, foto_id: &str) -> Result<(), String> {
        let id = PhotoId::from_string(foto_id).map_err(|e| e.to_string())?;
        self.publicar.remover_do_site(sessao, &id).await
    }

    /// Entra numa sessão: a galeria e as fotos que estão nela.
    pub async fn abrir_galeria(&self, sessao: &Sessao, id: &str) -> Result<GaleriaAberta, String> {
        self.api.abrir_galeria(sessao, id).await.map_err(frase)
    }

    /// Manda ao cliente o e-mail "suas fotos estão prontas".
    pub async fn avisar(&self, sessao: &Sessao, galeria_id: &str) -> Result<(), String> {
        self.api
            .avisar_fotos_prontas(sessao, galeria_id)
            .await
            .map_err(frase)
    }

    /// A miniatura de uma foto do site, para a grade da sessão.
    pub async fn miniatura(&self, sessao: &Sessao, foto_id: &str) -> Result<Vec<u8>, String> {
        self.api.miniatura(sessao, foto_id).await.map_err(frase)
    }

    /// Os bytes da cópia de trabalho de uma foto do site — o passo 11.
    pub async fn copia_de_trabalho(
        &self,
        sessao: &Sessao,
        foto_id: &str,
    ) -> Result<Vec<u8>, String> {
        self.api
            .copia_de_trabalho(sessao, foto_id)
            .await
            .map_err(frase)
    }

    /// O **original** — o arquivo cheio, e não a cópia de trabalho de 2048 px.
    /// É dele que a revelação salva na galeria sai.
    pub async fn original(&self, sessao: &Sessao, foto_id: &str) -> Result<Vec<u8>, String> {
        self.api.original(sessao, foto_id).await.map_err(frase)
    }

    /// **Salvar na galeria**: o JPEG revelado entra no lugar do original.
    ///
    /// `foto_no_site` é o id **remoto** — o do storage, e não o do catálogo
    /// local: quem está sendo substituída é a foto que o cliente vai baixar.
    pub async fn salvar_revelacao(
        &self,
        sessao: &Sessao,
        foto_no_site: &str,
        jpeg: Vec<u8>,
        ajustes: serde_json::Value,
    ) -> Result<(), String> {
        self.publicar
            .salvar_revelacao(sessao, foto_no_site, jpeg, ajustes)
            .await
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
        // ⚠️ Esta frase falava de **senha** até 6/set/2026, e a senha deixou de
        // passar por aqui — o operador via "e-mail ou senha recusados" depois de
        // um fluxo em que não digitou nem um nem outro. As três causas reais são
        // as citadas: conta sem acesso, sessão vencida, ou o código que expirou
        // enquanto a janela do navegador ficava aberta.
        DomainError::AcessoRecusado => {
            "o site recusou a autorização — confira se entrou com a conta do estúdio, \
             e tente de novo"
                .to_string()
        }
        outro => outro.to_string(),
    }
}
