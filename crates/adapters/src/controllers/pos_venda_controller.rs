//! A fronteira da interface com o pós-venda: `Result<_, String>`, como os outros.

use std::sync::Arc;

use domain::services::pos_venda::{
    EstadoNoBalcao, Estudio, Galeria, GaleriaAberta, GaleriaDoPainel, LinkDeAcesso, MudancaDaFoto,
    MudancaDaGaleria, NovaGaleria, PosVendaApi, Produto, Sessao,
};
use domain::value_objects::PhotoId;
use domain::DomainError;
use use_cases::pos_venda::PublicarNoPosVendaUseCase;

/// 🔚 Por que o link ou o aviso não saíram.
///
/// A exceção à fronteira `Result<_, String>`: "falta o e-mail do cliente"
/// (`422`, desde 2026-09-13) **não é erro para mostrar**, é o pedido do e-mail
/// que a tela abre antes de seguir com o gesto. Achatado em frase, a tela teria
/// de ler o texto para decidir — e texto muda sem aviso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecusaDoFimDaSessao {
    /// A sessão não tem e-mail (sem contato, ou só WhatsApp). A frase é a do site.
    FaltaEmail(String),
    /// Qualquer outra recusa, já na frase que a tela mostra.
    Outra(String),
}

fn recusa_do_fim(erro: DomainError) -> RecusaDoFimDaSessao {
    match erro {
        DomainError::FaltaEmail(frase) => RecusaDoFimDaSessao::FaltaEmail(frase),
        outro => RecusaDoFimDaSessao::Outra(frase(outro)),
    }
}

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

    /// Um pedido JSON em nome da conta — ver [`PosVendaApi::pedir_json`].
    pub async fn pedir_json(
        &self,
        sessao: &Sessao,
        metodo: &str,
        caminho: &str,
        corpo: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        self.api
            .pedir_json(sessao, metodo, caminho, corpo)
            .await
            .map_err(frase)
    }

    pub async fn produtos(&self, sessao: &Sessao) -> Result<Vec<Produto>, String> {
        self.api.produtos(sessao).await.map_err(frase)
    }

    /// Os estúdios ativos — a escolha obrigatória ao abrir sessão.
    pub async fn estudios(&self, sessao: &Sessao) -> Result<Vec<Estudio>, String> {
        self.api.estudios(sessao).await.map_err(frase)
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
    #[allow(clippy::too_many_arguments)]
    pub async fn enviar_uma(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        foto_id: &str,
        ordem: u32,
        estado: Option<EstadoNoBalcao>,
        nota: Option<u8>,
        produto_id: Option<String>,
    ) -> Result<use_cases::pos_venda::Subida, String> {
        let id = PhotoId::from_string(foto_id).map_err(|e| e.to_string())?;
        self.publicar
            .enviar_uma(sessao, galeria_id, &id, ordem, estado, nota, produto_id)
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

    /// Rejeita a foto que tem cópia aqui: ela sai da nuvem e fica marcada.
    pub async fn rejeitar_tirando_da_nuvem(
        &self,
        sessao: &Sessao,
        foto_id: &str,
    ) -> Result<(), String> {
        let id = PhotoId::from_string(foto_id).map_err(|e| e.to_string())?;
        self.publicar.rejeitar_tirando_da_nuvem(sessao, &id).await
    }

    /// Tira do site pelo id **de lá** — o fim do resgate.
    pub async fn remover_remoto(&self, sessao: &Sessao, no_site: &str) -> Result<(), String> {
        self.publicar.remover_remoto(sessao, no_site).await
    }

    /// Entra numa sessão: a galeria e as fotos que estão nela.
    pub async fn abrir_galeria(&self, sessao: &Sessao, id: &str) -> Result<GaleriaAberta, String> {
        self.api.abrir_galeria(sessao, id).await.map_err(frase)
    }

    /// Manda ao cliente o e-mail "suas fotos estão prontas".
    ///
    /// 🔚 Sessão sem e-mail volta como [`RecusaDoFimDaSessao::FaltaEmail`].
    pub async fn avisar(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
    ) -> Result<(), RecusaDoFimDaSessao> {
        self.api
            .avisar_fotos_prontas(sessao, galeria_id)
            .await
            .map_err(recusa_do_fim)
    }

    /// Muda título, e-mail ou WhatsApp da sessão — só o que veio.
    pub async fn atualizar_galeria(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        mudanca: &MudancaDaGaleria,
    ) -> Result<(), String> {
        self.api
            .atualizar_galeria(sessao, galeria_id, mudanca)
            .await
            .map_err(frase)
    }

    /// A miniatura de uma foto do site, para a grade da sessão.
    pub async fn miniatura(&self, sessao: &Sessao, foto_id: &str) -> Result<Vec<u8>, String> {
        self.api.miniatura(sessao, foto_id).await.map_err(frase)
    }

    /// A capa de um estúdio, pela URL pública do cadastro.
    pub async fn arquivo_publico(&self, url: &str) -> Result<Vec<u8>, String> {
        self.api.arquivo_publico(url).await.map_err(frase)
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

    /// **Zerar tudo salvo**: o bruto volta a ser o original desta foto no site.
    ///
    /// 🔑 Não sobe arquivo nenhum — ver `PublicarNoPosVendaUseCase::restaurar_original`.
    pub async fn restaurar_original(
        &self,
        sessao: &Sessao,
        foto_no_site: &str,
    ) -> Result<(), String> {
        self.publicar.restaurar_original(sessao, foto_no_site).await
    }

    /// O link que entra sem senha, para mandar ao cliente.
    ///
    /// 🔚 Sessão sem e-mail volta como [`RecusaDoFimDaSessao::FaltaEmail`].
    pub async fn link_da_galeria(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
    ) -> Result<LinkDeAcesso, RecusaDoFimDaSessao> {
        self.api
            .link_da_galeria(sessao, galeria_id)
            .await
            .map_err(recusa_do_fim)
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
