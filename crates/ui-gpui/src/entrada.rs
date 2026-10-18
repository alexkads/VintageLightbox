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
//!
//! # Quatro portas, desde 2026-10-18
//!
//! *"Às vezes o usuário leigo não entende muito bem"* a ida e volta do
//! navegador (dono, 2026-10-18). A tela ganhou abas: **E-mail e senha**,
//! **Link no e-mail** (toca no link, até no celular, e o app entra sozinho) e
//! **Authenticator** (os seis dígitos do celular), além do **Navegador** de
//! sempre — o único para quem só entra pelo Google. As três novas abrem a mesma
//! sessão de quinze dias, no chaveiro, renovada por baixo
//! (`application::auth::entrar_no_app`, no backend): os dois motivos que
//! tiraram a senha daqui em setembro não voltam com ela. A senha digitada vai
//! ao site e some do campo; só a porta e o e-mail ficam lembrados
//! (`pos_venda::config`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use domain::services::pos_venda::Sessao;
use gpui_kit::component::input::{InputEvent, InputState, OtpEvent, OtpState};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    div, prelude::*, px, Context, Entity, EventEmitter, SharedString, Subscription, Task, Window,
};

use crate::pos_venda::config::{Configuracao, PortaDeEntrada};
use crate::pos_venda::porta::{Publicador, Recado};

/// O contexto de teclado da entrada.
const CONTEXTO: &str = "Entrada";

gpui_kit::actions!(entrada, [EnviarEntrada]);

/// ⏎ envia a aba da vez. 🚨 **Consumido por ação**, e não só ouvido pelo
/// `PressEnter` do campo: o `InputState` de uma linha propaga a tecla, e o
/// `\n` de volta num campo de uma linha derrubava o app (o mesmo arranjo da
/// negociação do balcão).
pub fn init(cx: &mut gpui_kit::App) {
    cx.bind_keys([gpui_kit::KeyBinding::new(
        "enter",
        EnviarEntrada,
        Some(CONTEXTO),
    )]);
}

/// As abas, na ordem da tela.
const ABAS: [PortaDeEntrada; 4] = [
    PortaDeEntrada::Credencial,
    PortaDeEntrada::Link,
    PortaDeEntrada::Autenticador,
    PortaDeEntrada::Navegador,
];

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
    /// A aba da vez.
    aba: PortaDeEntrada,
    email: Entity<InputState>,
    senha: Entity<InputState>,
    codigo: Entity<OtpState>,
    recuperacao: Entity<InputState>,
    /// A aba do Authenticator está pedindo um código de recuperação.
    usando_recuperacao: bool,
    /// O link saiu e a tela espera o toque no e-mail.
    esperando_link: bool,
    /// Liga para a espera do link parar — o "Cancelar e voltar".
    cancelar_link: Option<Arc<AtomicBool>>,
    /// Uma frase neutra (não é erro): "mandamos o link", "mandamos o e-mail".
    aviso: Option<SharedString>,
    /// A porta que está no ar, para lembrar a que deu certo.
    porta_em_curso: Option<PortaDeEntrada>,
    _assinaturas: Vec<Subscription>,
}

impl Entrada {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        config: Configuracao,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let lembrado = config.email_lembrado.clone().unwrap_or_default();
        let email = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("seu e-mail do estúdio")
                .default_value(lembrado)
        });
        let senha = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("senha")
                .masked(true)
        });
        let codigo = cx.new(|cx| OtpState::new(6, window, cx));
        let recuperacao =
            cx.new(|cx| InputState::new(window, cx).placeholder("código de recuperação"));

        // Os seis dígitos completos já entram — sem precisar do Enter.
        let assinaturas = vec![
            cx.subscribe_in(&codigo, window, |tela, _, evento: &OtpEvent, window, cx| {
                if matches!(evento, OtpEvent::Complete) {
                    tela.entrar_com_codigo(window, cx);
                }
            }),
            cx.subscribe_in(&email, window, |tela, _, evento: &InputEvent, _, cx| {
                if matches!(evento, InputEvent::Change) && tela.erro.is_some() {
                    tela.erro = None;
                    cx.notify();
                }
            }),
        ];

        let mut tela = Self {
            publicador,
            aba: config.ultima_porta.unwrap_or(PortaDeEntrada::Credencial),
            config,
            entrando: false,
            no_navegador: false,
            erro: None,
            recados: channel(),
            colhendo: false,
            _colheita: None,
            email,
            senha,
            codigo,
            recuperacao,
            usando_recuperacao: false,
            esperando_link: false,
            cancelar_link: None,
            aviso: None,
            porta_em_curso: None,
            _assinaturas: assinaturas,
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
        self.aviso = None;
        self.porta_em_curso = Some(PortaDeEntrada::Navegador);
        self.publicador.autorizar(self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// Troca de aba. Durante uma espera não troca: a espera tem a saída dela.
    pub fn escolher(&mut self, aba: PortaDeEntrada, window: &mut Window, cx: &mut Context<Self>) {
        if self.entrando || self.aba == aba {
            return;
        }
        self.aba = aba;
        self.erro = None;
        self.aviso = None;
        self.focar(window, cx);
        cx.notify();
    }

    /// O cursor no campo que a aba pede primeiro.
    fn focar(&self, window: &mut Window, cx: &mut Context<Self>) {
        let email_vazio = self.email_digitado(cx).is_empty();
        match self.aba {
            PortaDeEntrada::Navegador => {}
            _ if email_vazio => self.email.update(cx, |e, cx| e.focus(window, cx)),
            PortaDeEntrada::Credencial => self.senha.update(cx, |e, cx| e.focus(window, cx)),
            PortaDeEntrada::Autenticador if self.usando_recuperacao => {
                self.recuperacao.update(cx, |e, cx| e.focus(window, cx))
            }
            PortaDeEntrada::Autenticador => self.codigo.update(cx, |e, cx| e.focus(window, cx)),
            PortaDeEntrada::Link => self.email.update(cx, |e, cx| e.focus(window, cx)),
        }
    }

    fn email_digitado(&self, cx: &gpui_kit::App) -> String {
        self.email.read(cx).value().trim().to_string()
    }

    /// O ⏎ da tela: o gesto principal da aba da vez.
    fn enviar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.aba {
            PortaDeEntrada::Credencial => self.entrar_com_senha(window, cx),
            PortaDeEntrada::Link => self.pedir_link(cx),
            PortaDeEntrada::Autenticador => self.entrar_com_codigo(window, cx),
            PortaDeEntrada::Navegador => self.entrar(cx),
        }
    }

    /// Começa uma entrada sem navegador: confere o e-mail e põe a tela em espera.
    fn comecar(&mut self, porta: PortaDeEntrada, cx: &mut Context<Self>) -> Option<String> {
        if self.entrando {
            return None;
        }
        let email = self.email_digitado(cx);
        if !email.contains('@') {
            self.erro = Some("Digite o seu e-mail do estúdio.".into());
            cx.notify();
            return None;
        }
        self.entrando = true;
        self.erro = None;
        self.aviso = None;
        self.porta_em_curso = Some(porta);
        Some(email)
    }

    /// **E-mail e senha.** A senha sai do campo assim que vai ao site.
    pub fn entrar_com_senha(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let senha = self.senha.read(cx).value().to_string();
        if senha.is_empty() && !self.entrando {
            self.erro = Some("Digite a senha.".into());
            self.senha.update(cx, |e, cx| e.focus(window, cx));
            cx.notify();
            return;
        }
        let Some(email) = self.comecar(PortaDeEntrada::Credencial, cx) else {
            return;
        };
        self.senha.update(cx, |e, cx| e.set_value("", window, cx));
        self.publicador
            .entrar_com_senha(email, senha, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// **Google Authenticator** — os seis dígitos, ou um código de recuperação.
    pub fn entrar_com_codigo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let codigo = if self.usando_recuperacao {
            self.recuperacao.read(cx).value().trim().to_string()
        } else {
            self.codigo.read(cx).value().to_string()
        };
        if codigo.is_empty() || (!self.usando_recuperacao && codigo.len() != 6) {
            if !self.entrando {
                self.erro = Some(if self.usando_recuperacao {
                    "Digite um dos códigos de recuperação.".into()
                } else {
                    "Digite os seis dígitos do Authenticator.".into()
                });
                cx.notify();
            }
            return;
        }
        let Some(email) = self.comecar(PortaDeEntrada::Autenticador, cx) else {
            return;
        };
        self.codigo.update(cx, |e, cx| e.set_value("", window, cx));
        self.recuperacao
            .update(cx, |e, cx| e.set_value("", window, cx));
        self.publicador
            .entrar_com_autenticador(email, codigo, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// **Link no e-mail.** A tela espera o toque até o link vencer.
    pub fn pedir_link(&mut self, cx: &mut Context<Self>) {
        let Some(email) = self.comecar(PortaDeEntrada::Link, cx) else {
            return;
        };
        let cancelar = Arc::new(AtomicBool::new(false));
        self.cancelar_link = Some(cancelar.clone());
        self.publicador
            .entrar_pelo_link(email, cancelar, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// "Criar ou trocar a senha" — o e-mail do site com o link de escolher.
    pub fn pedir_troca_de_senha(&mut self, cx: &mut Context<Self>) {
        let Some(email) = self.comecar(PortaDeEntrada::Credencial, cx) else {
            return;
        };
        self.porta_em_curso = None;
        self.publicador
            .pedir_troca_de_senha(email, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// Troca o código do celular pelo de recuperação, e volta.
    pub fn alternar_recuperacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.entrando {
            return;
        }
        self.usando_recuperacao = !self.usando_recuperacao;
        self.erro = None;
        self.focar(window, cx);
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
    /// Uma frase na tela de entrada, vinda de fora — hoje, o motivo de a sessão
    /// ter sido encerrada pelo servidor. Some no próximo "Entrar".
    pub fn avisar(&mut self, texto: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.erro = Some(texto.into());
        cx.notify();
    }

    pub fn desistir(&mut self, cx: &mut Context<Self>) {
        if !self.no_navegador && !self.esperando_link {
            return;
        }
        // A espera do link pergunta ao site a cada três segundos: desligá-la é
        // parar as perguntas, e não só deixar de ouvir a resposta.
        if let Some(cancelar) = self.cancelar_link.take() {
            cancelar.store(true, Ordering::Relaxed);
        }
        self.recados = channel();
        self.entrando = false;
        self.no_navegador = false;
        self.esperando_link = false;
        self.porta_em_curso = None;
        self.erro = None;
        self.aviso = None;
        cx.notify();
    }

    /// Lembra por onde entrou e com que e-mail — a próxima abertura já vem na
    /// aba certa. Nos testes não grava: o arquivo é o do app de verdade.
    fn lembrar(&mut self, cx: &mut Context<Self>) {
        let Some(porta) = self.porta_em_curso.take() else {
            return;
        };
        self.config.ultima_porta = Some(porta);
        if porta != PortaDeEntrada::Navegador {
            self.config.email_lembrado = Some(self.email_digitado(cx));
        }
        if !cfg!(test) {
            crate::pos_venda::config::gravar(&self.config);
        }
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
                    self.esperando_link = false;
                    self.cancelar_link = None;
                    self.lembrar(cx);
                    cx.emit(Entrou(sessao));
                }
                // Primeira abertura, ou os quinze dias venceram: o convite a
                // autorizar já é a tela — nada de vermelho.
                Recado::SemSessao => {
                    self.entrando = false;
                    self.no_navegador = false;
                }
                // O link saiu: a tela segue esperando, agora o toque no e-mail.
                Recado::LinkEnviado { expira_em } => {
                    self.esperando_link = true;
                    let minutos = (expira_em / 60).max(1);
                    self.aviso = Some(
                        format!(
                            "Mandamos o link para {}. Abra o e-mail — pode ser no celular — e \
                             toque em “Entrar no VintageLightbox”. O app entra sozinho. Vale \
                             {minutos} minutos.",
                            self.email_digitado(cx)
                        )
                        .into(),
                    );
                }
                Recado::SenhaPedida => {
                    self.entrando = false;
                    self.aviso = Some(
                        format!(
                            "Se {} tiver conta, chegou um e-mail com o link para criar ou \
                             trocar a senha. Depois volte aqui e entre com ela.",
                            self.email_digitado(cx)
                        )
                        .into(),
                    );
                }
                Recado::Falhou(erro) => {
                    self.entrando = false;
                    self.no_navegador = false;
                    self.esperando_link = false;
                    self.cancelar_link = None;
                    self.porta_em_curso = None;
                    self.aviso = None;
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

    pub fn aviso(&self) -> Option<&SharedString> {
        self.aviso.as_ref()
    }

    pub fn aba(&self) -> PortaDeEntrada {
        self.aba
    }

    pub fn esperando_link(&self) -> bool {
        self.esperando_link
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
        use gpui_kit::component::button::{Button, ButtonVariants};
        use gpui_kit::component::input::{Input, OtpInput};
        use gpui_kit::component::spinner::Spinner;
        use gpui_kit::component::tab::{Tab, TabBar};
        use gpui_kit::component::{h_flex, v_flex, Disableable, Icon, Sizable};
        use gpui_kit::{
            img, linear_color_stop, linear_gradient, relative, Animation, AnimationExt, FontWeight,
            ObjectFit, StyledImage,
        };

        use crate::recursos::Icone;

        let rotulo = if self.no_navegador {
            "Confirme no navegador…"
        } else if self.entrando && self.aba == PortaDeEntrada::Navegador {
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
                "O navegador abre para você confirmar a conta em {}. É o caminho de quem \
                 entra pelo Google.",
                self.config.site().trim_start_matches("https://")
            )
        };
        let ocupado = self.entrando;

        // O botão dourado da capa, para o gesto principal de cada aba.
        let dourado = |id: &'static str, rotulo: &'static str, giro: bool| {
            h_flex()
                .id(id)
                .h(px(52.))
                .px(px(30.))
                .gap(px(12.))
                .rounded_full()
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(cor(capa::BOTAO_CIMA), 0.),
                    linear_color_stop(cor(capa::BOTAO_BAIXO), 1.),
                ))
                .shadow_lg()
                .text_color(cor(capa::SOBRE_O_BOTAO))
                .text_size(crate::tema::letra::em(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .when(!ocupado, |b| b.cursor_pointer().hover(|s| s.opacity(0.92)))
                .when(ocupado, |b| {
                    b.cursor_default().opacity(if giro { 1. } else { 0.6 })
                })
                .when(ocupado && giro, |b| {
                    b.child(
                        Spinner::new()
                            .with_size(px(18.))
                            .color(cor(capa::SOBRE_O_BOTAO)),
                    )
                })
                .child(rotulo)
                .when(!ocupado, |b| {
                    b.child(Icon::new(Icone::ArrowRight).size(px(20.)))
                })
        };
        let texto_da_capa = |t: String| {
            div()
                .max_w(px(448.))
                .text_sm()
                .text_color(cor(capa::PARAGRAFO).opacity(0.7))
                .child(t)
        };
        let ligacao = |id: &'static str, rotulo: &'static str| {
            Button::new(id)
                .link()
                .text_sm()
                .text_color(cor(capa::OURO).opacity(0.9))
                .child(rotulo)
        };

        let botao = dourado(
            "entrada-entrar",
            rotulo,
            self.aba == PortaDeEntrada::Navegador,
        )
        .when(!ocupado, |b| {
            b.on_click(cx.listener(|tela, _ev, _window, cx| tela.entrar(cx)))
        });

        // 🗂️ As abas. Durante uma espera ficam paradas: a espera tem a saída
        // dela ("Cancelar e voltar"), e trocar de aba no meio deixaria a
        // resposta chegar numa aba que já não é a dela.
        let tela = cx.entity().downgrade();
        let abas = TabBar::new("abas-da-entrada")
            .segmented()
            .selected_index(ABAS.iter().position(|a| *a == self.aba).unwrap_or(0))
            .on_click(move |indice, window, cx| {
                let aba = ABAS[*indice];
                let _ = tela.update(cx, |tela, cx| tela.escolher(aba, window, cx));
            })
            .child(
                Tab::new()
                    .debug_selector(|| "aba-credencial".into())
                    .label("E-mail e senha"),
            )
            .child(
                Tab::new()
                    .debug_selector(|| "aba-link".into())
                    .label("Link no e-mail"),
            )
            .child(
                Tab::new()
                    .debug_selector(|| "aba-autenticador".into())
                    .label("Authenticator"),
            )
            .child(
                Tab::new()
                    .debug_selector(|| "aba-navegador".into())
                    .label("Navegador"),
            );

        let campo_do_email = v_flex()
            .gap(px(6.))
            .when(self.aba == PortaDeEntrada::Navegador, |d| d.invisible())
            .child(
                div()
                    .text_xs()
                    .text_color(cor(capa::OURO).opacity(0.85))
                    .child("E-MAIL"),
            )
            .child(crate::estilo::campo(
                Input::new(&self.email).disabled(ocupado),
            ));

        let painel_da_senha = v_flex()
            .gap(px(14.))
            .child(
                v_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cor(capa::OURO).opacity(0.85))
                            .child("SENHA"),
                    )
                    .child(crate::estilo::campo(
                        Input::new(&self.senha).mask_toggle().disabled(ocupado),
                    )),
            )
            .child(
                h_flex()
                    .gap(px(20.))
                    .child(
                        dourado(
                            "entrada-senha",
                            if ocupado { "Entrando…" } else { "Entrar" },
                            true,
                        )
                        .when(!ocupado, |b| {
                            b.on_click(cx.listener(|tela, _ev, window, cx| {
                                tela.entrar_com_senha(window, cx)
                            }))
                        }),
                    )
                    .child(
                        ligacao("entrada-trocar-senha", "Criar ou trocar a senha").on_click(
                            cx.listener(|tela, _ev, _window, cx| tela.pedir_troca_de_senha(cx)),
                        ),
                    ),
            );

        let painel_do_link = v_flex()
            .gap(px(14.))
            .child(texto_da_capa(if self.esperando_link {
                "Esperando o toque no e-mail…".to_string()
            } else {
                "Mandamos um link para o seu e-mail. Toque nele — no computador ou no \
                 celular — e o app entra sozinho, sem senha."
                    .to_string()
            }))
            .child(
                dourado(
                    "entrada-link",
                    if self.esperando_link {
                        "Esperando o toque…"
                    } else {
                        "Mandar o link"
                    },
                    true,
                )
                .when(!ocupado, |b| {
                    b.on_click(cx.listener(|tela, _ev, _window, cx| tela.pedir_link(cx)))
                }),
            );

        let campo_do_codigo = if self.usando_recuperacao {
            crate::estilo::campo(Input::new(&self.recuperacao).disabled(ocupado)).into_any_element()
        } else {
            OtpInput::new(&self.codigo)
                .groups(2)
                .disabled(ocupado)
                .into_any_element()
        };
        let painel_do_codigo = v_flex()
            .gap(px(14.))
            .child(
                v_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cor(capa::OURO).opacity(0.85))
                            .child(if self.usando_recuperacao {
                                "CÓDIGO DE RECUPERAÇÃO"
                            } else {
                                "CÓDIGO DO GOOGLE AUTHENTICATOR"
                            }),
                    )
                    .child(campo_do_codigo),
            )
            .child(
                h_flex()
                    .gap(px(20.))
                    .child(
                        dourado(
                            "entrada-codigo",
                            if ocupado { "Entrando…" } else { "Entrar" },
                            true,
                        )
                        .when(!ocupado, |b| {
                            b.on_click(cx.listener(|tela, _ev, window, cx| {
                                tela.entrar_com_codigo(window, cx)
                            }))
                        }),
                    )
                    .child(
                        ligacao(
                            "entrada-recuperacao",
                            if self.usando_recuperacao {
                                "Usar o código do celular"
                            } else {
                                "Usar um código de recuperação"
                            },
                        )
                        .on_click(cx.listener(|tela, _ev, window, cx| {
                            tela.alternar_recuperacao(window, cx)
                        })),
                    ),
            )
            .child(texto_da_capa(
                "Abra o Google Authenticator no celular. Para cadastrar o celular, entre \
                 de outro jeito e vá em Configurações › Conta."
                    .to_string(),
            ));

        let painel_do_navegador = v_flex()
            .gap(px(14.))
            .items_start()
            .child(botao)
            .child(texto_da_capa(dica));

        let paineis = crate::estilo::mesmo_lugar([
            (
                self.aba == PortaDeEntrada::Credencial,
                painel_da_senha.into_any_element(),
            ),
            (
                self.aba == PortaDeEntrada::Link,
                painel_do_link.into_any_element(),
            ),
            (
                self.aba == PortaDeEntrada::Autenticador,
                painel_do_codigo.into_any_element(),
            ),
            (
                self.aba == PortaDeEntrada::Navegador,
                painel_do_navegador.into_any_element(),
            ),
        ]);

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
                            .text_size(crate::tema::letra::em(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cor(capa::OURO))
                            .child("R E V E L A Ç Ã O   ·   C L A S S I F I C A Ç Ã O   ·   B A L C Ã O"),
                    ),
            )
            .child(
                div()
                    .font_family(capa::SERIFADA)
                    .text_size(crate::tema::letra::em(72.))
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
                            .text_size(crate::tema::letra::em(8.))
                            .text_color(cor(capa::OURO))
                            .child("◆"),
                    )
                    .child(div().w(px(64.)).h(px(1.)).bg(cor(capa::OURO).opacity(0.6))),
            )
            .child(
                div()
                    .max_w(px(448.))
                    .font_family(capa::SERIFADA)
                    .text_size(crate::tema::letra::em(18.))
                    .line_height(relative(1.6))
                    .text_color(cor(capa::PARAGRAFO).opacity(0.9))
                    .child(
                        "Do cartão da câmera à galeria do cliente: as fotos do ensaio reveladas, \
                         escolhidas e vendidas ali mesmo, no balcão.",
                    ),
            )
            .child(
                v_flex()
                    .mt(px(32.))
                    .w(px(448.))
                    .gap(px(18.))
                    .child(div().flex().child(abas))
                    .child(campo_do_email)
                    .child(paineis)
                    // 🚪 A saída das esperas — a do navegador e a do link. Só
                    // aparece enquanto elas duram.
                    .child(
                        div().h(px(20.)).when(self.no_navegador || self.esperando_link, |d| {
                            d.child(
                                Button::new("entrada-desistir")
                                    .link()
                                    .text_sm()
                                    .text_color(cor(capa::OURO).opacity(0.85))
                                    .child("Cancelar e voltar")
                                    .on_click(
                                        cx.listener(|tela, _ev, _window, cx| tela.desistir(cx)),
                                    ),
                            )
                        }),
                    )
                    // A linha das frases: o aviso neutro ou o erro. Reservada,
                    // para a capa não pular quando uma delas aparece.
                    .child(
                        div()
                            .min_h(px(64.))
                            .when_some(self.aviso.clone(), |d, aviso| {
                                d.child(
                                    div()
                                        .px(px(12.))
                                        .py(px(8.))
                                        .rounded(crate::tema::canto(8.))
                                        .bg(cor(capa::OURO).opacity(0.12))
                                        .border_1()
                                        .border_color(cor(capa::OURO).opacity(0.4))
                                        .text_sm()
                                        .text_color(cor(capa::TEXTO))
                                        .child(aviso),
                                )
                            })
                            .when_some(self.erro.clone(), |d, erro| {
                                d.child(
                                    div()
                                        .px(px(12.))
                                        .py(px(8.))
                                        .rounded(crate::tema::canto(8.))
                                        .bg(cx.theme().danger.opacity(0.2))
                                        .border_1()
                                        .border_color(cx.theme().danger.opacity(0.6))
                                        .text_sm()
                                        .text_color(cor(capa::TEXTO))
                                        .child(crate::erro_da_api::legivel(&erro)),
                                )
                            }),
                    ),
            )
            .with_animation(
                "entrada-surgir",
                Animation::new(std::time::Duration::from_millis(900))
                    .with_easing(gpui_kit::ease_out_quint()),
                |conteudo, delta| conteudo.opacity(delta).mt(px(14. * (1. - delta))),
            );

        div()
            .key_context(CONTEXTO)
            .on_action(cx.listener(|tela, _: &EnviarEntrada, window, cx| tela.enviar(window, cx)))
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

    // ── As portas sem navegador (2026-10-18) ───────────────────────────────

    fn com_config(
        publicador: Arc<PublicadorDeMentira>,
        config: Configuracao,
        cx: &mut TestAppContext,
    ) -> gpui_kit::WindowHandle<Entrada> {
        cx.update(gpui_kit::init);
        let janela = cx.add_window(|window, cx| Entrada::nova(publicador, config, window, cx));
        janela
            .update(cx, |tela, _window, cx| {
                tela.colher(cx);
            })
            .expect("a janela de teste");
        janela
    }

    fn digitar(
        campo: &Entity<InputState>,
        texto: &str,
        window: &mut Window,
        cx: &mut Context<Entrada>,
    ) {
        campo.update(cx, |c, cx| c.set_value(texto.to_string(), window, cx));
    }

    #[gpui_kit::test]
    fn email_e_senha_certos_entram_e_a_porta_fica_lembrada(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = tela(publicador.clone(), cx);

        janela
            .update(cx, |tela, window, cx| {
                assert_eq!(
                    tela.aba(),
                    PortaDeEntrada::Credencial,
                    "a primeira vez abre na senha"
                );
                digitar(
                    &tela.email.clone(),
                    "balcao@recordarfotos.com.br",
                    window,
                    cx,
                );
                digitar(&tela.senha.clone(), "senha-certa", window, cx);
                tela.entrar_com_senha(window, cx);
                assert!(
                    tela.senha.read(cx).value().is_empty(),
                    "a senha sai do campo assim que vai ao site"
                );
                tela.colher(cx);
                assert!(!tela.entrando(), "entrou");
                assert!(tela.erro().is_none());
                assert_eq!(tela.config.ultima_porta, Some(PortaDeEntrada::Credencial));
                assert_eq!(
                    tela.config.email_lembrado.as_deref(),
                    Some("balcao@recordarfotos.com.br")
                );
            })
            .expect("a janela de teste");

        let entradas = publicador.entradas.lock().unwrap().clone();
        assert_eq!(
            entradas,
            vec![(
                "senha",
                "balcao@recordarfotos.com.br".to_string(),
                "senha-certa".to_string()
            )]
        );
    }

    #[gpui_kit::test]
    fn senha_errada_mostra_a_frase_do_site_e_nao_lembra_nada(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = tela(publicador.clone(), cx);

        janela
            .update(cx, |tela, window, cx| {
                digitar(
                    &tela.email.clone(),
                    "balcao@recordarfotos.com.br",
                    window,
                    cx,
                );
                digitar(&tela.senha.clone(), "errada", window, cx);
                tela.entrar_com_senha(window, cx);
                tela.colher(cx);
                assert!(!tela.entrando());
                assert_eq!(
                    tela.erro().map(|e| e.to_string()).as_deref(),
                    Some("e-mail ou senha incorretos.")
                );
                assert_eq!(
                    tela.config.ultima_porta, None,
                    "só a entrada que deu certo é lembrada"
                );
            })
            .expect("a janela de teste");
    }

    #[gpui_kit::test]
    fn sem_email_nada_sai_para_o_site(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = tela(publicador.clone(), cx);

        janela
            .update(cx, |tela, window, cx| {
                digitar(&tela.senha.clone(), "senha-certa", window, cx);
                tela.entrar_com_senha(window, cx);
                assert!(!tela.entrando());
                assert!(tela.erro().is_some_and(|e| e.contains("e-mail")));
            })
            .expect("a janela de teste");
        assert!(publicador.entradas.lock().unwrap().is_empty());
    }

    /// O link saiu: a tela espera o toque, diz para onde foi, e a saída desliga
    /// a espera — o toque que vier depois não entra.
    #[gpui_kit::test]
    fn o_link_espera_o_toque_e_desistir_desliga_a_espera(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = tela(publicador.clone(), cx);

        janela
            .update(cx, |tela, window, cx| {
                tela.escolher(PortaDeEntrada::Link, window, cx);
                digitar(
                    &tela.email.clone(),
                    "balcao@recordarfotos.com.br",
                    window,
                    cx,
                );
                tela.pedir_link(cx);
                tela.colher(cx);
                assert!(tela.entrando(), "esperando o toque no e-mail");
                assert!(tela.esperando_link());
                assert!(tela
                    .aviso()
                    .is_some_and(|a| a.contains("balcao@recordarfotos.com.br")));

                tela.desistir(cx);
                assert!(!tela.entrando());
                assert!(!tela.esperando_link());
                assert!(tela.aviso().is_none() && tela.erro().is_none());
            })
            .expect("a janela de teste");

        // A pessoa toca no link depois de o operador ter desistido.
        publicador.responder();
        janela
            .update(cx, |tela, _window, cx| {
                tela.colher(cx);
                assert!(!tela.entrando(), "o toque atrasado não reabre a espera");
                assert_eq!(tela.config.ultima_porta, None, "e não entrou");
            })
            .expect("a janela de teste");
    }

    #[gpui_kit::test]
    fn o_toque_no_link_entra_sozinho(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = tela(publicador.clone(), cx);

        janela
            .update(cx, |tela, window, cx| {
                tela.escolher(PortaDeEntrada::Link, window, cx);
                digitar(
                    &tela.email.clone(),
                    "balcao@recordarfotos.com.br",
                    window,
                    cx,
                );
                tela.pedir_link(cx);
                tela.colher(cx);
            })
            .expect("a janela de teste");
        publicador.responder();
        janela
            .update(cx, |tela, _window, cx| {
                tela.colher(cx);
                assert!(!tela.entrando(), "entrou");
                assert_eq!(tela.config.ultima_porta, Some(PortaDeEntrada::Link));
            })
            .expect("a janela de teste");
    }

    #[gpui_kit::test]
    fn o_codigo_do_authenticator_entra_e_o_errado_avisa(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = tela(publicador.clone(), cx);

        janela
            .update(cx, |tela, window, cx| {
                tela.escolher(PortaDeEntrada::Autenticador, window, cx);
                digitar(
                    &tela.email.clone(),
                    "balcao@recordarfotos.com.br",
                    window,
                    cx,
                );

                tela.codigo
                    .update(cx, |c, cx| c.set_value("000000", window, cx));
                tela.entrar_com_codigo(window, cx);
                tela.colher(cx);
                assert!(tela.erro().is_some_and(|e| e.contains("código")));
                assert!(
                    tela.codigo.read(cx).value().is_empty(),
                    "o código errado sai do campo"
                );

                tela.codigo
                    .update(cx, |c, cx| c.set_value("123456", window, cx));
                tela.entrar_com_codigo(window, cx);
                tela.colher(cx);
                assert!(tela.erro().is_none());
                assert_eq!(tela.config.ultima_porta, Some(PortaDeEntrada::Autenticador));
            })
            .expect("a janela de teste");
    }

    #[gpui_kit::test]
    fn a_tela_abre_na_aba_que_funcionou_com_o_email_preenchido(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let config = Configuracao {
            ultima_porta: Some(PortaDeEntrada::Autenticador),
            email_lembrado: Some("balcao@recordarfotos.com.br".into()),
            ..Configuracao::default()
        };
        let janela = com_config(publicador, config, cx);
        janela
            .update(cx, |tela, _window, cx| {
                assert_eq!(tela.aba(), PortaDeEntrada::Autenticador);
                assert_eq!(tela.email_digitado(cx), "balcao@recordarfotos.com.br");
            })
            .expect("a janela de teste");
    }

    #[gpui_kit::test]
    fn criar_ou_trocar_a_senha_avisa_que_o_email_saiu(cx: &mut TestAppContext) {
        let publicador = Arc::new(PublicadorDeMentira::default());
        let janela = tela(publicador.clone(), cx);
        janela
            .update(cx, |tela, window, cx| {
                digitar(
                    &tela.email.clone(),
                    "balcao@recordarfotos.com.br",
                    window,
                    cx,
                );
                tela.pedir_troca_de_senha(cx);
                tela.colher(cx);
                assert!(!tela.entrando());
                assert!(tela.aviso().is_some_and(|a| a.contains("criar ou")));
                assert_eq!(tela.config.ultima_porta, None, "pedir a senha não é entrar");
            })
            .expect("a janela de teste");
    }
}
