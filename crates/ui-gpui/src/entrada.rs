//! A porta do app: entrar na conta do site. Não há outra.
//!
//! # Por que existe, e o que ela reverte
//!
//! Até 6/set/2026 o app abria direto na Biblioteca, e a conta do site só era
//! pedida dentro do modal de pós-venda — porque o objetivo dizia que *"o app tem
//! de ser útil sozinho: nada da triagem ou da revelação depende do site"*.
//!
//! O dono reverteu isso no mesmo dia. A reversão veio primeiro com uma saída —
//! um botão "trabalhar offline" — e a saída **caiu em 6/set/2026**, no mesmo
//! dia, pelo motivo que decide: *"o propósito dele é integração com o pós-venda
//! da RecordarFotos"*. Um app que abre sem conta é um catálogo local que fala
//! com o site quando dá; um que só abre com conta é parte do pós-venda.
//!
//! 🚨 **Trabalhar sem rede não foi descartado — foi adiado com forma própria.**
//! O que vai existir é **sincronização**: o app guarda o que foi feito sem rede
//! e concilia quando ela volta. Um botão que só desliga o site é o contrário
//! disso: ele deixa o operador subir 200 fotos na triagem e descobrir no balcão
//! que nada foi para o site — e não guarda nada para conciliar depois.
//!
//! # A senha não passa mais por aqui — e a sessão dura quinze dias
//!
//! Até esta entrega a tela pedia e-mail e senha, mandava os dois ao site e
//! guardava só o token de acesso: quinze minutos de sessão, e o de renovação
//! jogado fora. Na prática era **uma senha por dia de trabalho**, digitada num
//! aplicativo desktop, e sem caminho nenhum para quem entra no site pelo Google.
//!
//! Agora o app abre o navegador. O operador confirma no site, com o que ele já
//! usa lá, e o app recebe de volta um par de tokens — quinze dias de renovação,
//! guardados no chaveiro do sistema, renovados por baixo antes de cada chamada.
//! Do segundo dia em diante esta tela nem aparece: a sessão é retomada na
//! abertura.
//!
//! A senha do estúdio deixou de existir dentro deste processo, e isso é metade
//! do ganho — ver `infrastructure::pos_venda::autorizacao` para a outra metade,
//! que é o código de dois minutos não valer nada sem o verificador.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use domain::services::pos_venda::Sessao;
use gpui_kit::component::ActiveTheme;
use gpui_kit::{div, prelude::*, px, Context, EventEmitter, SharedString, Task, Window};

use crate::pos_venda::config::Configuracao;
use crate::pos_venda::porta::{Publicador, Recado};

/// De quanto em quanto a tela pergunta se o site respondeu.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// O que a tela avisa quando a conta entrou. É o único desfecho dela.
pub struct Entrou(pub Sessao);

impl EventEmitter<Entrou> for Entrada {}

pub struct Entrada {
    publicador: Arc<dyn Publicador>,
    config: Configuracao,
    /// A autorização está no ar: ou lendo o chaveiro, ou esperando o navegador.
    entrando: bool,
    /// O navegador foi aberto e o operador está decidindo lá.
    /// Separado de [`Entrada::entrando`] porque a frase muda: "verificando"
    /// dura um piscar, "confirme no navegador" dura o que o operador demorar.
    no_navegador: bool,
    erro: Option<SharedString>,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

impl Entrada {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        config: Configuracao,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut tela = Self {
            publicador,
            config,
            entrando: false,
            no_navegador: false,
            erro: None,
            recados: channel(),
            colhendo: false,
            _colheita: None,
        };

        // 🔑 A retomada é a primeira coisa que acontece, antes de qualquer
        // pixel: no caso comum — o operador que autorizou semana passada — esta
        // tela existe por um piscar e o app abre direto na Biblioteca. Esperar o
        // clique para só então descobrir que há sessão faria o app pedir todo dia
        // um botão que ele não precisava.
        tela.retomar(cx);
        tela
    }

    /// Pergunta ao chaveiro se ainda há sessão. Silenciosa: `SemSessao` só
    /// mostra o convite a autorizar, e não é erro.
    fn retomar(&mut self, cx: &mut Context<Self>) {
        self.entrando = true;
        self.publicador.retomar(self.recados.0.clone());
        self.acompanhar(cx);
    }

    /// Abre o navegador para autorizar este computador.
    pub fn entrar(&mut self, cx: &mut Context<Self>) {
        if self.entrando {
            return;
        }
        self.entrando = true;
        self.no_navegador = true;
        self.erro = None;
        self.publicador.autorizar(self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// Desiste da espera e volta ao convite.
    ///
    /// # 🚨 Por que a tela precisa de saída
    ///
    /// *"Tinha que ter um timeout pois a tela tá travada"* (dono, 19/set/2026).
    /// O prazo existe — [`PRAZO_PARA_AUTORIZAR`] são cinco minutos, em
    /// `infrastructure::pos_venda::autorizacao` —, mas cinco minutos de roda
    /// girando são indistinguíveis de um app pendurado, e no GNOME, onde não há
    /// barra de janela, o operador nem fechar podia: só restava matar o
    /// processo. O caso que trouxe a queixa nem chegava ao prazo — a conta não
    /// tinha `ManageMedia`, o site recusava com 403 e **nada voltava para cá**.
    ///
    /// # 🔑 O canal é trocado, e é isso que desliga a tentativa abandonada
    ///
    /// A autorização vive numa tarefa do tokio que não dá para cancelar daqui:
    /// ela segue esperando o navegador até o prazo dela. Trocar o canal deixa o
    /// `Sender` dela falando para um `Receiver` que já morreu — o `send` falha
    /// em silêncio, como já falha hoje quando a tela fecha. Sem isso, uma
    /// autorização abandonada entraria no app cinco minutos depois, sozinha.
    pub fn desistir(&mut self, cx: &mut Context<Self>) {
        if !self.no_navegador {
            return;
        }
        self.recados = channel();
        self.entrando = false;
        self.no_navegador = false;
        self.erro = None;
        cx.notify();
    }

    fn acompanhar(&mut self, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;
        self._colheita = Some(cx.spawn(async move |esta, cx| loop {
            cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
            let Ok(continua) = esta.update(cx, |tela, cx| tela.colher(cx)) else {
                break;
            };
            if !continua {
                break;
            }
        }));
    }

    /// Drena o canal. Devolve se vale continuar acordando.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            match recado {
                Recado::Entrou(sessao) => {
                    self.entrando = false;
                    self.no_navegador = false;
                    cx.emit(Entrou(sessao));
                }
                // Primeira abertura, ou os quinze dias venceram: o convite a
                // autorizar já é a tela — nada de vermelho.
                Recado::SemSessao => {
                    self.entrando = false;
                    self.no_navegador = false;
                }
                Recado::Falhou(erro) => {
                    self.entrando = false;
                    self.no_navegador = false;
                    self.erro = Some(erro.into());
                }
                // Nenhum dos outros nasce daqui: esta tela só entra.
                _ => {}
            }
        }
        if mudou {
            cx.notify();
        }
        let continua = self.entrando;
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    pub fn entrando(&self) -> bool {
        self.entrando
    }

    pub fn erro(&self) -> Option<&SharedString> {
        self.erro.as_ref()
    }
}

/// A capa do estúdio, com as cores e as medidas de
/// `frontend/src/components/marca/capa-do-estudio.tsx` (dono, 2026-09-16: a
/// entrada *"tem que ser muito linda com uma imagem vintage de fundo"*). É a
/// mesma capa de `/autorizar-app`.
///
/// 🔑 **Nada vem da rede.** A foto (já com o filtro sépia do site aplicado,
/// porque o GPUI não tem filtro de imagem) e o selo estão embutidos
/// (`imagens/`), e as serifadas são as que o macOS já tem.
mod capa {
    use gpui_kit::{rgb, rgba, Hsla};

    pub const FUNDO: u32 = 0x140d09;
    pub const TEXTO: u32 = 0xf3e6cf;
    pub const OURO: u32 = 0xd9a441;
    pub const TITULO: u32 = 0xecc57c;
    pub const PARAGRAFO: u32 = 0xeadcc3;
    pub const BOTAO_CIMA: u32 = 0xf0c86a;
    pub const BOTAO_BAIXO: u32 = 0xc8912f;
    pub const SOBRE_O_BOTAO: u32 = 0x2a1a0e;
    /// `'Didot', 'Bodoni 72', …`: a primeira que o sistema tiver.
    pub const SERIFADA: &str = "Didot";

    pub fn cor(c: u32) -> Hsla {
        rgb(c).into()
    }

    /// O fundo da capa com transparência (`rgba(20,12,8,a)`).
    pub fn veu(alfa: f32) -> Hsla {
        let a = (alfa.clamp(0., 1.) * 255.).round() as u32;
        rgba(0x140c0800 | a).into()
    }
}

impl Entrada {
    /// Uma faixa do véu horizontal da capa, de `de` a `ate` (0–1 da largura).
    fn faixa(de: f32, ate: f32, alfa_de: f32, alfa_ate: f32) -> gpui_kit::Div {
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(gpui_kit::relative(de))
            .w(gpui_kit::relative(ate - de))
            .bg(gpui_kit::linear_gradient(
                90.,
                gpui_kit::linear_color_stop(capa::veu(alfa_de), 0.),
                gpui_kit::linear_color_stop(capa::veu(alfa_ate), 1.),
            ))
    }
}

impl Render for Entrada {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use capa::cor;
        use gpui_kit::component::{h_flex, v_flex, Icon};
        use gpui_kit::{
            img, linear_color_stop, linear_gradient, relative, Animation, AnimationExt, FontWeight,
            ObjectFit, StyledImage,
        };

        use crate::recursos::Icone;

        let rotulo = if self.no_navegador {
            "Confirme no navegador…"
        } else if self.entrando {
            "Verificando…"
        } else {
            "Entrar com a conta RecordarFotos"
        };
        let dica = if self.no_navegador {
            "Confirme a entrada na janela que abriu no navegador e volte para cá. \
             O pedido vale por cinco minutos."
                .to_string()
        } else {
            format!(
                "O navegador abre para você confirmar a conta em {}. Depois é só voltar para cá.",
                self.config.site().trim_start_matches("https://")
            )
        };
        let ocupado = self.entrando;

        let botao = h_flex()
            .id("entrada-entrar")
            .h(px(56.))
            .px(px(32.))
            .gap(px(12.))
            .rounded_full()
            .bg(linear_gradient(
                180.,
                linear_color_stop(cor(capa::BOTAO_CIMA), 0.),
                linear_color_stop(cor(capa::BOTAO_BAIXO), 1.),
            ))
            .shadow_lg()
            .text_color(cor(capa::SOBRE_O_BOTAO))
            .text_size(px(16.))
            .font_weight(FontWeight::SEMIBOLD)
            .when(!ocupado, |b| {
                b.cursor_pointer()
                    .hover(|s| s.opacity(0.92))
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.entrar(cx)))
            })
            .when(ocupado, |b| {
                b.cursor_default().child(
                    Icon::new(Icone::LoaderCircle).size(px(18.)).with_animation(
                        "entrada-girando",
                        Animation::new(std::time::Duration::from_secs(1)).repeat(),
                        |icone, delta| {
                            icone.transform(gpui_kit::Transformation::rotate(gpui_kit::percentage(
                                delta,
                            )))
                        },
                    ),
                )
            })
            .child(rotulo)
            .when(!ocupado, |b| {
                b.child(Icon::new(Icone::ArrowRight).size(px(20.)))
            });

        let conteudo = v_flex()
            .max_w(px(576.))
            .child(
                img("imagens/selo.png")
                    .w(px(208.))
                    .h(px(167.))
                    .object_fit(ObjectFit::Contain)
                    .mb(px(32.)),
            )
            .child(
                h_flex()
                    .mb(px(16.))
                    .gap(px(12.))
                    .child(div().w(px(40.)).h(px(1.)).bg(linear_gradient(
                        90.,
                        linear_color_stop(capa::veu(0.), 0.),
                        linear_color_stop(cor(capa::OURO), 1.),
                    )))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cor(capa::OURO))
                            .child("R E V E L A Ç Ã O   ·   C L A S S I F I C A Ç Ã O   ·   B A L C Ã O"),
                    ),
            )
            .child(
                div()
                    .font_family(capa::SERIFADA)
                    .text_size(px(72.))
                    .line_height(relative(1.05))
                    .font_weight(FontWeight::BOLD)
                    .text_color(cor(capa::TITULO))
                    .child("Vintage")
                    .child("Lightbox"),
            )
            .child(
                h_flex()
                    .my(px(28.))
                    .gap(px(12.))
                    .child(div().w(px(64.)).h(px(1.)).bg(cor(capa::OURO).opacity(0.6)))
                    .child(
                        div()
                            .text_size(px(8.))
                            .text_color(cor(capa::OURO))
                            .child("◆"),
                    )
                    .child(div().w(px(64.)).h(px(1.)).bg(cor(capa::OURO).opacity(0.6))),
            )
            .child(
                div()
                    .max_w(px(448.))
                    .font_family(capa::SERIFADA)
                    .text_size(px(18.))
                    .line_height(relative(1.6))
                    .text_color(cor(capa::PARAGRAFO).opacity(0.9))
                    .child(
                        "Do cartão da câmera à galeria do cliente: as fotos do ensaio reveladas, \
                         escolhidas e vendidas ali mesmo, no balcão.",
                    ),
            )
            .child(
                v_flex()
                    .mt(px(40.))
                    .items_start()
                    .child(botao)
                    .child(
                        div()
                            .mt(px(16.))
                            .max_w(px(448.))
                            .text_sm()
                            .text_color(cor(capa::PARAGRAFO).opacity(0.6))
                            .child(dica),
                    )
                    // 🚪 A saída da espera. Só aparece enquanto ela dura, e é
                    // a única coisa clicável da tela nesse estado.
                    .when(self.no_navegador, |d| {
                        d.child(
                            div()
                                .id("entrada-desistir")
                                .mt(px(12.))
                                .text_sm()
                                .cursor_pointer()
                                .text_color(cor(capa::OURO).opacity(0.85))
                                .hover(|s| s.text_color(cor(capa::OURO)))
                                .child("Cancelar e voltar")
                                .on_click(cx.listener(|tela, _ev, _window, cx| tela.desistir(cx))),
                        )
                    })
                    .when_some(self.erro.clone(), |d, erro| {
                        d.child(
                            div()
                                .mt(px(16.))
                                .max_w(px(448.))
                                .px(px(12.))
                                .py(px(8.))
                                .rounded(px(8.))
                                .bg(cx.theme().danger.opacity(0.2))
                                .border_1()
                                .border_color(cx.theme().danger.opacity(0.6))
                                .text_sm()
                                .text_color(cor(capa::TEXTO))
                                .child(erro),
                        )
                    }),
            )
            .with_animation(
                "entrada-surgir",
                Animation::new(std::time::Duration::from_millis(900))
                    .with_easing(gpui_kit::ease_out_quint()),
                |conteudo, delta| conteudo.opacity(delta).mt(px(14. * (1. - delta))),
            );

        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(cor(capa::FUNDO))
            .text_color(cor(capa::TEXTO))
            // A foto ocupa a direita e some em direção ao texto.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(relative(0.78))
                    .child(
                        img("imagens/capa-canela.jpeg")
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    ),
            )
            // A máscara do site (transparente → 60% em 22% → cheia em 45% da
            // foto), feita com o fundo por cima, que dá o mesmo resultado.
            .child(Self::faixa(0.22, 0.22 + 0.78 * 0.22, 1., 0.4))
            .child(Self::faixa(0.22 + 0.78 * 0.22, 0.22 + 0.78 * 0.45, 0.4, 0.))
            // O véu que escurece o lado do texto (`linear-gradient(90deg, …)`).
            .child(Self::faixa(0., 0.32, 0.92, 0.78))
            .child(Self::faixa(0.32, 0.50, 0.78, 0.25))
            .child(Self::faixa(0.50, 0.72, 0.25, 0.))
            .child(Self::faixa(0.72, 1.0, 0., 0.3))
            // O fecho de cima e de baixo, no lugar do gradiente radial.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(relative(0.3))
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(capa::veu(0.55), 0.),
                        linear_color_stop(capa::veu(0.), 1.),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(relative(0.3))
                    .bg(linear_gradient(
                        0.,
                        linear_color_stop(capa::veu(0.7), 0.),
                        linear_color_stop(capa::veu(0.), 1.),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .px(px(96.))
                    .py(px(48.))
                    .child(conteudo),
            )
            // 🪟 **A barra de janela desta tela.** Ela não tem o cabeçalho de
            // 56 px do app, e no GNOME não há barra do sistema: sem esta faixa a
            // janela não se move nem fecha — e é a primeira tela que o app
            // mostra. Fora do Linux ela fica vazia e só serve de espaço.
            .child(
                crate::janela::como_barra_de_titulo(div(), "barra-da-entrada", window, cx)
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .px(px(8.))
                    .child(crate::janela::controles(
                        "janela-entrada",
                        cor(capa::PARAGRAFO),
                        window,
                        cx,
                    )),
            )
            .child(
                h_flex()
                    .absolute()
                    .bottom(px(24.))
                    .left(px(96.))
                    .right(px(96.))
                    .justify_between()
                    .text_xs()
                    .text_color(cor(capa::PARAGRAFO).opacity(0.45))
                    .child("G R A M A D O   ·   C A N E L A")
                    .child(
                        div()
                            .font_family(capa::SERIFADA)
                            .italic()
                            .child("Ensaio no estúdio de Canela"),
                    ),
            )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use crate::pos_venda::porta::mentira::PublicadorDeMentira;
    use gpui_kit::TestAppContext;

    /// A tela já com a retomada resolvida — que é como o operador a encontra.
    ///
    /// ⚠️ **Sem colher o `SemSessao` da abertura, `entrar` não faz nada**: ela
    /// recusa enquanto `entrando` estiver de pé, e a retomada o deixa de pé até
    /// a primeira colheita. No app isso acontece num piscar, antes de existir
    /// clique; no teste, se não for feito à mão, o clique cai no vazio e o teste
    /// passa medindo outra coisa.
    fn tela(
        publicador: Arc<PublicadorDeMentira>,
        cx: &mut TestAppContext,
    ) -> gpui_kit::WindowHandle<Entrada> {
        cx.update(gpui_kit::init);
        let janela = cx.add_window(|window, cx| {
            Entrada::nova(publicador, Configuracao::default(), window, cx)
        });
        janela
            .update(cx, |tela, _window, cx| {
                tela.colher(cx);
                assert!(!tela.entrando(), "a retomada não achou sessão e terminou");
            })
            .expect("a janela de teste");
        janela
    }

    /// 🚪 **A espera tem saída, e a saída volta ao convite.**
    ///
    /// Antes de 19/set/2026 não havia: quem apertava "Entrar" ficava com a roda
    /// girando até o prazo de cinco minutos do `infrastructure`, e no GNOME —
    /// onde o app desenha a própria janela — nem fechar dava, porque a tela de
    /// entrada não tinha barra. O caminho que trouxe a queixa nem chegava ao
    /// prazo: o site recusava com 403 e nada voltava.
    #[gpui_kit::test]
    fn desistir_da_espera_devolve_o_convite(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira {
            // A rede que não responde: é o que põe a tela no estado de espera.
            demorada: true,
            ..Default::default()
        });
        let janela = tela(publicador.clone(), cx);

        janela
            .update(cx, |tela, _window, cx| {
                tela.entrar(cx);
                assert!(tela.entrando(), "apertou entrar: a tela está esperando");
                assert!(tela.no_navegador, "e a espera é a do navegador");

                tela.desistir(cx);
                assert!(!tela.entrando(), "desistiu: a tela não espera mais");
                assert!(!tela.no_navegador, "e não fala mais em navegador");
                assert!(
                    tela.erro().is_none(),
                    "desistir é escolha do operador, não falha — nada de vermelho"
                );
            })
            .expect("a janela de teste");
    }

    /// 🚨 **A autorização abandonada não entra pela porta dos fundos.**
    ///
    /// A tarefa que espera o navegador não se cancela daqui: ela segue de pé até
    /// o prazo dela. Sem trocar o canal, o operador que desistiu veria o app
    /// entrar sozinho minutos depois — com a conta que ele decidiu não usar.
    #[gpui_kit::test]
    fn o_que_volta_depois_de_desistir_nao_entra(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira {
            demorada: true,
            ..Default::default()
        });
        let janela = tela(publicador.clone(), cx);

        janela
            .update(cx, |tela, _window, cx| {
                tela.entrar(cx);
                tela.desistir(cx);
            })
            .expect("a janela de teste");

        // A rede responde agora, tarde: o `Sender` que ela tem é o do canal
        // trocado, e não há mais quem o escute.
        publicador.responder();

        janela
            .update(cx, |tela, _window, cx| {
                tela.colher(cx);
                assert!(
                    !tela.entrando(),
                    "a resposta atrasada não pode reabrir a espera"
                );
            })
            .expect("a janela de teste");
    }
}
