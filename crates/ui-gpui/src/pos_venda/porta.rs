//! A ponte entre a tela e o `PosVendaController` — a mesma forma das outras
//! portas: o controller é `async` do tokio, o GPUI não roda futuros dele, e o
//! `Handle` é capturado no `main` antes de `Application::run` tomar a thread.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use adapters::controllers::PosVendaController;
use domain::services::pos_venda::{
    EstadoNoBalcao, Galeria, GaleriaAberta, GaleriaDoPainel, LinkDeAcesso, MudancaDaFoto,
    NovaGaleria, Produto, Sessao,
};
use domain::value_objects::CropSettings;
use infrastructure::gpu_adjustments::Ajustes;
use infrastructure::ImageExporterImpl;

/// O que volta pelo canal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recado {
    Entrou(Sessao),
    Produtos(Vec<Produto>),
    /// O link que entra sem senha, pronto para ir ao cliente.
    Link(LinkDeAcesso),
    /// As sessões fotográficas que já existem.
    Galerias(Vec<GaleriaDoPainel>),
    /// Uma sessão recém-aberta, ainda sem foto nenhuma.
    Criada(Galeria),
    /// A sessão em que se entrou: a galeria e as fotos que estão nela.
    Aberta(Box<GaleriaAberta>),
    /// A miniatura de uma foto da sessão.
    Miniatura {
        foto_id: String,
        bytes: Vec<u8>,
    },
    /// Os pixels de uma foto do site — o passo 11.
    ///
    /// Leva o id da foto junto: um download que volta depois de a seta ter
    /// andado não pode pintar a foto errada, e quem confere isso é a tela.
    Pixels {
        foto_id: String,
        bytes: Vec<u8>,
    },
    /// O revelado entrou no lugar do original — "Salvar na galeria e sair".
    RevelacaoSalva,
    /// O passo 3 terminou para uma foto: ela subiu, ou saiu do storage.
    ///
    /// 🔑 **Notifica, não descreve.** Quem escuta só precisa saber que o
    /// catálogo mudou, para reler — os números estão no banco.
    Sincronizou,
    /// Não há sessão guardada (ou ela venceu): a tela mostra o convite a
    /// autorizar. **Não é falha** — é o estado normal da primeira abertura, e
    /// tratá-lo como erro pintaria de vermelho um app recém-instalado.
    SemSessao,
    /// Uma frase para a tela — autorização recusada, rede caída, galeria
    /// recusada.
    Falhou(String),
}

pub trait Publicador: Send + Sync + 'static {
    /// Abre o navegador para o operador autorizar este computador.
    ///
    /// Todas devolvem na hora; a resposta vem pelo canal — e esta demora o que o
    /// operador demorar na outra janela, que é justamente por isso que ela não
    /// pode bloquear a interface.
    fn autorizar(&self, canal: Sender<Recado>);

    /// A sessão de ontem, do chaveiro. Responde `Entrou` se ainda valer, e
    /// `SemSessao` se for preciso autorizar.
    fn retomar(&self, canal: Sender<Recado>);

    /// Esquece a sessão — o "sair" da tela.
    fn sair(&self, canal: Sender<Recado>);
    fn produtos(&self, sessao: Sessao, canal: Sender<Recado>);
    /// O passo 7 do fluxo: o link do cliente.
    ///
    /// ⚠️ **Pedido ao site, nunca montado aqui.** O endereço da galeria exige
    /// sessão e o cliente não tem conta — ele saiu do estúdio, não do site.
    fn link(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>);
    /// A lista de sessões fotográficas.
    fn galerias(&self, sessao: Sessao, canal: Sender<Recado>);
    /// Abre uma sessão vazia — as fotos vêm depois.
    fn criar_galeria(&self, sessao: Sessao, nova: NovaGaleria, canal: Sender<Recado>);
    /// Sobe uma foto para uma sessão que já existe.
    ///
    /// `estado` manda quando vem preenchido — é a leva escolhida antes dos
    /// arquivos, na tela da sessão. `None` cai na marcação da tecla `B`, que é
    /// o caminho do passo 3 (classificar sobe).
    fn subir_classificada(
        &self,
        sessao: Sessao,
        galeria_id: String,
        foto_id: String,
        ordem: u32,
        estado: Option<EstadoNoBalcao>,
        canal: Sender<Recado>,
    );
    /// O passo 3 ao contrário: a classificação foi zerada, a foto sai do storage.
    fn tirar_do_site(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>);
    /// O passo 6: o que o cliente acertou no balcão, gravado na foto do site.
    fn negociar(
        &self,
        sessao: Sessao,
        foto_id: String,
        mudanca: MudancaDaFoto,
        canal: Sender<Recado>,
    );
    /// Sobe um arquivo do disco para a sessão — o envio da web.
    fn enviar_arquivo(
        &self,
        sessao: Sessao,
        galeria_id: String,
        caminho: String,
        ordem: u32,
        estado: EstadoNoBalcao,
        canal: Sender<Recado>,
    );
    /// Entrar numa sessão — o mesmo gesto que abre a rota `[id]` na web.
    fn abrir_galeria(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>);
    /// A miniatura de uma foto da sessão, para a grade.
    fn miniatura(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>);
    /// **Salvar na galeria**: o revelado entra no lugar do original.
    ///
    /// 🔑 É o botão do editor, e não uma publicação: a foto já é da galeria
    /// aberta, e nada aqui cria galeria nem pergunta pelo cliente.
    ///
    /// 🚨 **O original é baixado e revelado aqui, e não na tela.** A tela tem a
    /// cópia de trabalho de 2048 px — é dela que os sliders andam —, e subir
    /// aquilo entregaria ao cliente uma foto de 2048 px no lugar do original.
    /// É o mesmo caminho do `revelarIntegral` do site, e ele mora nesta porta
    /// porque é aqui que existe tokio para a rede e o motor de GPU para revelar,
    /// os dois fora da thread que desenha.
    fn salvar_revelacao(
        &self,
        sessao: Sessao,
        foto_no_site: String,
        ajustes: Ajustes,
        corte: CropSettings,
        canal: Sender<Recado>,
    );
    /// Manda ao cliente o e-mail "suas fotos estão prontas".
    fn avisar(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>);
    /// O passo 11: os pixels da foto que só existe no storage.
    ///
    /// `foto_local` é o id **do catálogo**, e volta no recado: é por ele que a
    /// tela confere se a foto na frente ainda é a mesma.
    fn copia_de_trabalho(
        &self,
        sessao: Sessao,
        foto_local: String,
        foto_no_site: String,
        canal: Sender<Recado>,
    );
}

pub struct PublicadorDaApi {
    controlador: Arc<PosVendaController>,
    /// O mesmo motor da exportação e da impressão — um só, e não um por gesto.
    /// Abrir um `Motor` custa um dispositivo wgpu; três caminhos com três
    /// dispositivos dariam três respostas possíveis para a mesma foto.
    exportador: Arc<ImageExporterImpl>,
    tokio: tokio::runtime::Handle,
}

impl PublicadorDaApi {
    pub fn novo(
        controlador: Arc<PosVendaController>,
        exportador: Arc<ImageExporterImpl>,
        tokio: tokio::runtime::Handle,
    ) -> Self {
        Self {
            controlador,
            exportador,
            tokio,
        }
    }
}

/// A qualidade do JPEG que vai para a galeria.
///
/// 🔑 **92, o mesmo do editor do site** (`QUALIDADE` em `editor.tsx`). O que o
/// cliente baixa não pode depender de por qual das duas telas a foto passou.
const QUALIDADE: u8 = 92;

/// Baixa o original, revela com o que está na tela e sobe no lugar dele.
///
/// ⚠️ **Revelar é síncrono e come CPU e GPU**, então ele corre num
/// `spawn_blocking`: dentro de uma tarefa do tokio ele seguraria a thread do
/// executor, e com ela toda a rede do app — inclusive o upload que vem logo
/// depois.
async fn revelar_e_salvar(
    controlador: &PosVendaController,
    exportador: &Arc<ImageExporterImpl>,
    sessao: &Sessao,
    foto_no_site: &str,
    ajustes: Ajustes,
    corte: CropSettings,
) -> Result<(), String> {
    let original = controlador.original(sessao, foto_no_site).await?;

    let exportador = exportador.clone();
    let para_revelar = corte.clone();
    let jpeg = tokio::task::spawn_blocking(move || {
        exportador.renderizar_bytes(&original, &ajustes, &para_revelar, QUALIDADE)
    })
    .await
    .map_err(|e| format!("a revelação não terminou: {e}"))?
    .map_err(|e| e.to_string())?;

    controlador
        .salvar_revelacao(
            sessao,
            foto_no_site,
            jpeg,
            ajustes_em_json(&ajustes, &corte),
        )
        .await
}

/// Os ajustes e o enquadramento como o site os grava: um objeto só, por nome.
///
/// 🔑 **O corte entra com o prefixo `corte_`**, que é o que `corteParaJson` do
/// editor faz — e é assim que a web lê de volta. Um segundo formato aqui faria
/// a foto revelada no app abrir sem enquadramento no navegador.
fn ajustes_em_json(ajustes: &Ajustes, corte: &CropSettings) -> serde_json::Value {
    let mut json = serde_json::to_value(ajustes).unwrap_or_else(|_| serde_json::json!({}));
    if let Some(objeto) = json.as_object_mut() {
        objeto.insert("corte_x".into(), corte.crop_x().into());
        objeto.insert("corte_y".into(), corte.crop_y().into());
        objeto.insert("corte_largura".into(), corte.crop_width().into());
        objeto.insert("corte_altura".into(), corte.crop_height().into());
        objeto.insert("corte_giro90".into(), corte.rotation_90().into());
        objeto.insert("corte_angulo".into(), corte.angle().into());
        objeto.insert(
            "corte_espelho_h".into(),
            i32::from(corte.flip_horizontal()).into(),
        );
        objeto.insert(
            "corte_espelho_v".into(),
            i32::from(corte.flip_vertical()).into(),
        );
    }
    json
}

impl Publicador for PublicadorDaApi {
    fn autorizar(&self, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.autorizar().await {
                Ok(sessao) => Recado::Entrou(sessao),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn retomar(&self, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.retomar().await {
                Ok(Some(sessao)) => Recado::Entrou(sessao),
                Ok(None) => Recado::SemSessao,
                // Falha ao ler o chaveiro não é motivo para assustar ninguém: o
                // desfecho é o mesmo de não haver sessão — autorizar.
                Err(_) => Recado::SemSessao,
            };
            let _ = canal.send(recado);
        });
    }

    fn sair(&self, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            controlador.sair().await;
            let _ = canal.send(Recado::SemSessao);
        });
    }

    fn produtos(&self, sessao: Sessao, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.produtos(&sessao).await {
                Ok(produtos) => Recado::Produtos(produtos),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn salvar_revelacao(
        &self,
        sessao: Sessao,
        foto_no_site: String,
        ajustes: Ajustes,
        corte: CropSettings,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        let exportador = self.exportador.clone();
        self.tokio.spawn(async move {
            let recado = match revelar_e_salvar(
                &controlador,
                &exportador,
                &sessao,
                &foto_no_site,
                ajustes,
                corte,
            )
            .await
            {
                Ok(()) => Recado::RevelacaoSalva,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn link(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.link_da_galeria(&sessao, &galeria_id).await {
                Ok(link) => Recado::Link(link),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn galerias(&self, sessao: Sessao, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.galerias(&sessao).await {
                Ok(lista) => Recado::Galerias(lista),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn criar_galeria(&self, sessao: Sessao, nova: NovaGaleria, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.criar_galeria(&sessao, &nova).await {
                Ok(galeria) => Recado::Criada(galeria),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn subir_classificada(
        &self,
        sessao: Sessao,
        galeria_id: String,
        foto_id: String,
        ordem: u32,
        estado: Option<EstadoNoBalcao>,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador
                .enviar_uma(&sessao, &galeria_id, &foto_id, ordem, estado)
                .await
            {
                Ok(_) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn tirar_do_site(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.remover_do_site(&sessao, &foto_id).await {
                Ok(()) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn negociar(
        &self,
        sessao: Sessao,
        foto_id: String,
        mudanca: MudancaDaFoto,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.mudar_foto(&sessao, &foto_id, &mudanca).await {
                Ok(()) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn enviar_arquivo(
        &self,
        sessao: Sessao,
        galeria_id: String,
        caminho: String,
        ordem: u32,
        estado: EstadoNoBalcao,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador
                .enviar_arquivo(&sessao, &galeria_id, &caminho, ordem, estado)
                .await
            {
                Ok(_) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn abrir_galeria(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.abrir_galeria(&sessao, &galeria_id).await {
                Ok(aberta) => Recado::Aberta(Box::new(aberta)),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn miniatura(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.miniatura(&sessao, &foto_id).await {
                Ok(bytes) => Recado::Miniatura { foto_id, bytes },
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn avisar(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.avisar(&sessao, &galeria_id).await {
                Ok(()) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn copia_de_trabalho(
        &self,
        sessao: Sessao,
        foto_local: String,
        foto_no_site: String,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.copia_de_trabalho(&sessao, &foto_no_site).await {
                Ok(bytes) => Recado::Pixels {
                    foto_id: foto_local,
                    bytes,
                },
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }
}

#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    /// Entra com qualquer senha, lista os produtos que recebeu, e "publica"
    /// respondendo o andamento inteiro na hora.
    #[derive(Default)]
    pub struct PublicadorDeMentira {
        pub produtos: Vec<Produto>,
        /// Liga a recusa do site — para a tela poder ser testada com "não
        /// autorizado". Invertido de propósito: o `Default` do teste é o caminho
        /// que dá certo.
        pub recusa_autorizacao: bool,
        /// Quantas vezes o navegador foi aberto.
        pub autorizacoes: Mutex<Vec<()>>,
        /// O que o chaveiro devolve na retomada.
        pub sessao_guardada: Mutex<Option<Sessao>>,
        /// De quais galerias o link foi pedido.
        pub links: Mutex<Vec<String>>,
        /// As sessões que a listagem vai encontrar.
        pub galerias: Mutex<Vec<GaleriaDoPainel>>,
        /// As que foram abertas por esta tela.
        pub criadas: Mutex<Vec<NovaGaleria>>,
        /// `(galeria, foto, ordem)` de cada classificada que subiu.
        pub subidas: Mutex<Vec<(String, String, u32)>>,
        /// As fotos tiradas do storage.
        pub tiradas: Mutex<Vec<String>>,
        /// O que foi negociado, por foto.
        pub negociadas: Mutex<Vec<(String, MudancaDaFoto)>>,
        /// Os ids no site cujos pixels foram pedidos.
        pub baixadas: Mutex<Vec<String>>,
        /// As sessões em que se entrou.
        pub abertas: Mutex<Vec<String>>,
        /// As galerias cujo cliente foi avisado.
        pub avisadas: Mutex<Vec<String>>,
        /// O estado pedido em cada subida — `None` é "o da tecla B".
        pub estados_pedidos: Mutex<Vec<Option<EstadoNoBalcao>>>,
        /// O que a sessão aberta vai mostrar.
        pub fotos_da_sessao: Mutex<Vec<domain::services::pos_venda::FotoDaGaleria>>,
        /// `(galeria, caminho, ordem, estado)` de cada arquivo do disco enviado.
        pub arquivos_enviados: Mutex<Vec<(String, String, u32, EstadoNoBalcao)>>,
        /// `(foto no site, ajustes, corte)` de cada revelação salva.
        pub reveladas: Mutex<Vec<(String, Ajustes, CropSettings)>>,
        /// Segura as respostas em vez de mandá-las na hora — o que a rede faz.
        ///
        /// 🚨 **O `Default` responde no mesmo instante, e isso escondia um
        /// defeito**: a colheita da tela podia desistir antes da resposta e
        /// nenhum teste reclamava, porque a mentira nunca chegava atrasada. Com
        /// isto ligado, o recado só sai em [`PublicadorDeMentira::responder`].
        pub demorada: bool,
        /// Os recados presos, esperando `responder()`.
        pub guardados: Mutex<Vec<(Sender<Recado>, Recado)>>,
    }

    /// Um JPEG 1×1 cinza, codificado de verdade.
    fn jpeg_de_um_pixel() -> Vec<u8> {
        let mut bytes = Vec::new();
        let imagem = image::RgbImage::from_pixel(1, 1, image::Rgb([128, 128, 128]));
        image::DynamicImage::ImageRgb8(imagem)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Jpeg,
            )
            .expect("codificar um pixel");
        bytes
    }

    impl PublicadorDeMentira {
        pub fn links(&self) -> Vec<String> {
            self.links.lock().expect("os links").clone()
        }

        pub fn criadas(&self) -> Vec<NovaGaleria> {
            self.criadas.lock().expect("as criadas").clone()
        }

        pub fn subidas(&self) -> Vec<(String, String, u32)> {
            self.subidas.lock().expect("as subidas").clone()
        }

        pub fn tiradas(&self) -> Vec<String> {
            self.tiradas.lock().expect("as tiradas").clone()
        }

        pub fn negociadas(&self) -> Vec<(String, MudancaDaFoto)> {
            self.negociadas.lock().expect("as negociadas").clone()
        }

        pub fn baixadas(&self) -> Vec<String> {
            self.baixadas.lock().expect("as baixadas").clone()
        }

        pub fn abertas(&self) -> Vec<String> {
            self.abertas.lock().expect("as abertas").clone()
        }

        pub fn arquivos_enviados(&self) -> Vec<(String, String, u32, EstadoNoBalcao)> {
            self.arquivos_enviados.lock().expect("os arquivos").clone()
        }

        pub fn estados_pedidos(&self) -> Vec<Option<EstadoNoBalcao>> {
            self.estados_pedidos.lock().expect("os estados").clone()
        }

        pub fn reveladas(&self) -> Vec<(String, Ajustes, CropSettings)> {
            self.reveladas.lock().expect("as reveladas").clone()
        }

        /// Manda **uma** das guardadas, a mais antiga.
        ///
        /// 🔑 Existe para provar o que `responder` não prova: num lote, as
        /// respostas chegam **espalhadas no tempo**, uma por foto. Soltar todas
        /// de uma vez esconderia exatamente o defeito de quem para de escutar
        /// na primeira.
        pub fn responder_uma(&self) {
            let mut guardados = self.guardados.lock().expect("os guardados");
            if guardados.is_empty() {
                return;
            }
            let (canal, recado) = guardados.remove(0);
            let _ = canal.send(recado);
        }

        /// Manda o que `demorada` segurou — a rede respondendo, enfim.
        pub fn responder(&self) {
            for (canal, recado) in self.guardados.lock().expect("os guardados").drain(..) {
                let _ = canal.send(recado);
            }
        }

        fn responder_ou_guardar(&self, canal: Sender<Recado>, recado: Recado) {
            if self.demorada {
                self.guardados
                    .lock()
                    .expect("os guardados")
                    .push((canal, recado));
            } else {
                let _ = canal.send(recado);
            }
        }
    }

    impl Publicador for PublicadorDeMentira {
        fn autorizar(&self, canal: Sender<Recado>) {
            self.autorizacoes.lock().expect("as autorizacoes").push(());
            let _ = canal.send(if !self.recusa_autorizacao {
                Recado::Entrou(Sessao {
                    access_token: "tok-de-mentira".into(),
                    refresh_token: "ref-de-mentira".into(),
                    access_vence_em: i64::MAX,
                    refresh_vence_em: i64::MAX,
                })
            } else {
                Recado::Falhou("o site recusou a autorização".into())
            });
        }

        fn retomar(&self, canal: Sender<Recado>) {
            let _ = canal.send(
                match self.sessao_guardada.lock().expect("a guardada").clone() {
                    Some(sessao) => Recado::Entrou(sessao),
                    None => Recado::SemSessao,
                },
            );
        }

        fn sair(&self, canal: Sender<Recado>) {
            *self.sessao_guardada.lock().expect("a guardada") = None;
            let _ = canal.send(Recado::SemSessao);
        }

        fn produtos(&self, _sessao: Sessao, canal: Sender<Recado>) {
            self.responder_ou_guardar(canal, Recado::Produtos(self.produtos.clone()));
        }

        fn salvar_revelacao(
            &self,
            _sessao: Sessao,
            foto_no_site: String,
            ajustes: Ajustes,
            corte: CropSettings,
            canal: Sender<Recado>,
        ) {
            self.reveladas
                .lock()
                .expect("as reveladas")
                .push((foto_no_site, ajustes, corte));
            self.responder_ou_guardar(canal, Recado::RevelacaoSalva);
        }

        fn link(&self, _sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
            self.links
                .lock()
                .expect("os links")
                .push(galeria_id.clone());
            self.responder_ou_guardar(
                canal,
                Recado::Link(LinkDeAcesso {
                    url: format!("https://recordarfotos.com.br/entrar?t={galeria_id}"),
                    validade_em_segundos: 604_800,
                }),
            );
        }

        fn subir_classificada(
            &self,
            _sessao: Sessao,
            galeria_id: String,
            foto_id: String,
            ordem: u32,
            estado: Option<EstadoNoBalcao>,
            canal: Sender<Recado>,
        ) {
            self.subidas
                .lock()
                .expect("as subidas")
                .push((galeria_id, foto_id, ordem));
            self.estados_pedidos
                .lock()
                .expect("os estados")
                .push(estado);
            let _ = canal.send(Recado::Sincronizou);
        }

        fn tirar_do_site(&self, _sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
            self.tiradas.lock().expect("as tiradas").push(foto_id);
            let _ = canal.send(Recado::Sincronizou);
        }

        fn enviar_arquivo(
            &self,
            _sessao: Sessao,
            galeria_id: String,
            caminho: String,
            ordem: u32,
            estado: EstadoNoBalcao,
            canal: Sender<Recado>,
        ) {
            self.arquivos_enviados
                .lock()
                .expect("os arquivos")
                .push((galeria_id, caminho, ordem, estado));
            let _ = canal.send(Recado::Sincronizou);
        }

        fn abrir_galeria(&self, _sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
            self.abertas
                .lock()
                .expect("as abertas")
                .push(galeria_id.clone());
            let galeria = self
                .galerias
                .lock()
                .expect("as galerias")
                .iter()
                .find(|g| g.id == galeria_id)
                .cloned();
            let Some(galeria) = galeria else {
                let _ = canal.send(Recado::Falhou("essa sessão não existe".into()));
                return;
            };
            let _ = canal.send(Recado::Aberta(Box::new(GaleriaAberta {
                galeria,
                fotos: self.fotos_da_sessao.lock().expect("as fotos").clone(),
                vence_venda: None,
                vence_download: None,
            })));
        }

        fn avisar(&self, _sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
            self.avisadas.lock().expect("as avisadas").push(galeria_id);
            let _ = canal.send(Recado::Sincronizou);
        }

        fn miniatura(&self, _sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
            let _ = canal.send(Recado::Miniatura {
                foto_id,
                bytes: jpeg_de_um_pixel(),
            });
        }

        fn copia_de_trabalho(
            &self,
            _sessao: Sessao,
            foto_local: String,
            foto_no_site: String,
            canal: Sender<Recado>,
        ) {
            self.baixadas
                .lock()
                .expect("as baixadas")
                .push(foto_no_site);
            // Um JPEG 1×1 de verdade: a tela decodifica o que chega, e um vetor
            // de lixo faria o teste passar por um caminho que a produção não tem.
            let _ = canal.send(Recado::Pixels {
                foto_id: foto_local,
                bytes: jpeg_de_um_pixel(),
            });
        }

        fn negociar(
            &self,
            _sessao: Sessao,
            foto_id: String,
            mudanca: MudancaDaFoto,
            canal: Sender<Recado>,
        ) {
            self.negociadas
                .lock()
                .expect("as negociadas")
                .push((foto_id, mudanca));
            let _ = canal.send(Recado::Sincronizou);
        }

        fn galerias(&self, _sessao: Sessao, canal: Sender<Recado>) {
            let _ = canal.send(Recado::Galerias(
                self.galerias.lock().expect("as galerias").clone(),
            ));
        }

        fn criar_galeria(&self, _sessao: Sessao, nova: NovaGaleria, canal: Sender<Recado>) {
            let id = format!("g{}", self.criadas.lock().expect("as criadas").len() + 1);
            self.criadas.lock().expect("as criadas").push(nova.clone());
            // A criada entra na lista, como entraria no site.
            self.galerias
                .lock()
                .expect("as galerias")
                .push(GaleriaDoPainel {
                    id: id.clone(),
                    titulo: nova.titulo.clone(),
                    email: nova.email.clone(),
                    whatsapp: nova.whatsapp.clone(),
                    produto_id: nova.produto_id.clone(),
                    user_id: None,
                    criada_em_iso: "2026-09-06".into(),
                    expira_em: None,
                    fotos: domain::services::pos_venda::ContagemDeFotos::default(),
                    totais: None,
                });
            let _ = canal.send(Recado::Criada(Galeria {
                id,
                titulo: nova.titulo,
            }));
        }
    }
}
