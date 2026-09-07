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
use gpui::{div, prelude::*, px, Context, EventEmitter, SharedString, Task, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Disableable, Sizable};

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

impl Render for Entrada {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .w(px(360.))
                    .p(px(24.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(cx.theme().border)
                    // 🔑 O cartão é um degrau acima do fundo, e não um retângulo
                    // desenhado só com borda. É a primeira tela do app: sem
                    // elevação nenhuma, a janela abre parecendo uma caixa de
                    // diálogo que ficou pela metade.
                    .bg(cx.theme().popover)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            // A marca: o âmbar da caixa de luz, o mesmo que
                            // marca sessão e balcão no resto do app.
                            .child(
                                div()
                                    .w(px(3.))
                                    .h(px(18.))
                                    .rounded(px(2.))
                                    .bg(crate::tema::cores::quente()),
                            )
                            .child(div().text_lg().child("VintageLightbox")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(if self.no_navegador {
                                "Confirme no navegador que acabou de abrir. \
                                 Esta janela continua sozinha quando você voltar."
                                    .to_string()
                            } else {
                                // O endereço mostrado é o do **site**, não o da
                                // API: é nele que o operador vai entrar, e
                                // anunciar a API aqui foi o que fez o dono ler
                                // "entrar em http://localhost:8080" numa tela
                                // que ia abrir produção (6/set/2026).
                                format!(
                                    "O navegador vai abrir para você entrar em {}. \
                                     A senha não passa por aqui.",
                                    self.config.site()
                                )
                            }),
                    )
                    .when_some(self.erro.clone(), |cartao, erro| {
                        cartao.child(
                            // ⚠️ Fundo, e não só letra vermelha: o erro aparece
                            // entre dois campos e uma frase de aviso, todos em
                            // `text_xs`. Sem uma faixa própria ele é mais uma
                            // linha pequena no meio de outras três.
                            div()
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().danger.opacity(0.15))
                                .border_l_2()
                                .border_color(cx.theme().danger)
                                .text_xs()
                                .text_color(cx.theme().foreground)
                                .child(erro),
                        )
                    })
                    .child(
                        Button::new("entrada-entrar")
                            .label(if self.no_navegador {
                                "Aguardando o navegador…"
                            } else if self.entrando {
                                "Verificando…"
                            } else {
                                "Entrar pelo navegador"
                            })
                            .small()
                            .primary()
                            .disabled(self.entrando)
                            .on_click(cx.listener(|tela, _ev, _window, cx| tela.entrar(cx))),
                    )
                    // 🚨 A frase diz por que não há saída, e não é decoração:
                    // sem ela a tela parece um login que alguém esqueceu de
                    // deixar pular. O app trabalha **dentro** de um ensaio do
                    // site — sem conta não há ensaio a que as fotos pertençam.
                    .child(
                        div()
                            .pt(px(10.))
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "Importar, revelar, escolher com o cliente e entregar \
                                 acontecem dentro de um ensaio do site. Trabalhar sem rede \
                                 vai existir por sincronização — ainda não existe.",
                            ),
                    ),
            )
    }
}
