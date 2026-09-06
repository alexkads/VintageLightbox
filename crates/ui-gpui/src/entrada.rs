//! A porta do app: entrar na conta do site, ou dizer que hoje é sem rede.
//!
//! # Por que existe, e o que ela reverte
//!
//! Até 6/set/2026 o app abria direto na Biblioteca, e a conta do site só era
//! pedida dentro do modal de pós-venda — porque o objetivo dizia que *"o app tem
//! de ser útil sozinho: nada da triagem ou da revelação depende do site"*.
//!
//! O dono reverteu isso no mesmo dia, com um limite: **abre pedindo a conta, e
//! quem está sem rede escolhe trabalhar offline**. O princípio não caiu inteiro
//! — ele virou uma escolha explícita, feita uma vez, em vez de um estado que o
//! app assume calado.
//!
//! 🚨 **A tela precisa dizer o que se perde ao pular.** Um botão "trabalhar
//! offline" sem essa frase transforma a escolha em armadilha: o operador pula
//! por pressa, sobe 200 fotos na triagem, e descobre no balcão que nada foi
//! para o site. A frase é o que torna a escolha uma escolha.
//!
//! # O token não é lembrado, e o e-mail é
//!
//! O mesmo motivo de [`crate::pos_venda::config`]: o arquivo mora ao lado do
//! catálogo, em JSON legível, e vai em todo backup dele. Uma senha por dia de
//! trabalho é o preço, e é barato.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use domain::services::pos_venda::Sessao;
use gpui::{div, prelude::*, px, Context, EventEmitter, SharedString, Task, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Disableable, Sizable};

use crate::pos_venda::config::{self, Configuracao};
use crate::pos_venda::porta::{Publicador, Recado};

/// De quanto em quanto a tela pergunta se o site respondeu.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// Como o app vai trabalhar nesta abertura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modo {
    /// Entrou na conta do site: tudo disponível.
    Online(Sessao),
    /// Escolheu trabalhar sem rede: importar, revelar e triar; nada que fale
    /// com o site.
    Offline,
}

/// O que a tela avisa quando a escolha foi feita.
pub struct Escolheu(pub Modo);

impl EventEmitter<Escolheu> for Entrada {}

pub struct Entrada {
    publicador: Arc<dyn Publicador>,
    config: Configuracao,
    email: gpui::Entity<InputState>,
    senha: gpui::Entity<InputState>,
    entrando: bool,
    erro: Option<SharedString>,
    recados: (Sender<Recado>, Receiver<Recado>),
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

impl Entrada {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        config: Configuracao,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // O e-mail da última vez já vem preenchido: é o mesmo operador todo dia.
        let email = cx.new(|cx| InputState::new(window, cx).default_value(config.email.clone()));
        let senha = cx.new(|cx| InputState::new(window, cx).masked(true));

        Self {
            publicador,
            config,
            email,
            senha,
            entrando: false,
            erro: None,
            recados: channel(),
            colhendo: false,
            _colheita: None,
        }
    }

    /// Manda a credencial ao site. A resposta chega pelo canal.
    pub fn entrar(&mut self, cx: &mut Context<Self>) {
        if self.entrando {
            return;
        }
        let email = self.email.read(cx).value().trim().to_string();
        let senha = self.senha.read(cx).value().to_string();
        if email.is_empty() || senha.is_empty() {
            self.erro = Some("preencha e-mail e senha".into());
            cx.notify();
            return;
        }

        self.entrando = true;
        self.erro = None;
        self.publicador.entrar(email, senha, self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// A escolha explícita de trabalhar sem o site.
    pub fn trabalhar_offline(&mut self, cx: &mut Context<Self>) {
        cx.emit(Escolheu(Modo::Offline));
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
                    // O e-mail que entrou é lembrado; a senha, nunca.
                    self.config.email = self.email.read(cx).value().trim().to_string();
                    config::gravar(&self.config);
                    cx.emit(Escolheu(Modo::Online(sessao)));
                }
                Recado::Falhou(erro) => {
                    self.entrando = false;
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

    fn campo(
        rotulo: &str,
        estado: &gpui::Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(rotulo.to_string()),
            )
            .child(Input::new(estado).xsmall())
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
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(div().text_lg().child("VintageLightbox"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Entrar na conta de {}", self.config.base_url)),
                    )
                    .child(Self::campo("e-mail", &self.email, cx))
                    .child(Self::campo("senha", &self.senha, cx))
                    .when_some(self.erro.clone(), |cartao, erro| {
                        cartao.child(div().text_xs().text_color(cx.theme().danger).child(erro))
                    })
                    .child(
                        Button::new("entrada-entrar")
                            .label(if self.entrando {
                                "Entrando…"
                            } else {
                                "Entrar"
                            })
                            .small()
                            .primary()
                            .disabled(self.entrando)
                            .on_click(cx.listener(|tela, _ev, _window, cx| tela.entrar(cx))),
                    )
                    // 🚨 A frase é o que faz da saída uma escolha, e não uma
                    // armadilha: sem ela o operador pula por pressa e descobre
                    // no balcão que nada foi para o site.
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "Sem entrar, o app importa, revela e tria normalmente — \
                                 mas não lista sessões, não publica, não registra o balcão \
                                 e não gera o link do cliente.",
                            ),
                    )
                    .child(
                        Button::new("entrada-offline")
                            .label("Trabalhar offline")
                            .xsmall()
                            .disabled(self.entrando)
                            .on_click(
                                cx.listener(|tela, _ev, _window, cx| tela.trabalhar_offline(cx)),
                            ),
                    ),
            )
    }
}
