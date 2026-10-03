//! 🖥️ **Em qual monitor a tela do cliente abre**, quando há mais de um.
//!
//! # Por que perguntar
//!
//! No Fedora (GNOME/Wayland) o app não posiciona janela: a tela do cliente
//! nascia no monitor do operador, e ele a arrastava até o do cliente a cada
//! atendimento (dono, 03/out/2026). Adivinhar não servia — o GPUI no Wayland
//! não sabe qual é o monitor principal, e a lista vem em ordem qualquer.
//!
//! Então, com dois monitores ou mais, o botão pergunta. A pergunta vem com o
//! monitor da última vez marcado, e `Enter` confirma: no dia a dia ela custa
//! uma tecla. `1`–`4` escolhem direto, da esquerda para a direita.
//!
//! O lugar é posto pelo GPUI: `display_id` no macOS, no Windows e no X11; no
//! Wayland, tela cheia naquele monitor (`cliente::ao_nascer` e o patch em
//! `vendor/gpui-pre-linux`).

use gpui_kit::component::{v_flex, ActiveTheme};
use gpui_kit::{actions, prelude::*, px, AnyElement, App, Context, DisplayId, Window};

use super::{caminho_do_modo_do_cliente, Aplicativo};
use crate::cliente::{self, Lembranca, Monitor};
use crate::estilo;

/// O contexto de teclado da pergunta: mais fundo que o da raiz, então o `1`
/// daqui escolhe o monitor em vez de dar uma estrela à foto, e o `Esc` fecha a
/// pergunta em vez de voltar para a Biblioteca.
const CONTEXTO: &str = "EscolhaDeMonitor";

actions!(
    vintagelightbox,
    [
        ConfirmarMonitor,
        CancelarEscolhaDeMonitor,
        PrimeiroMonitor,
        SegundoMonitor,
        TerceiroMonitor,
        QuartoMonitor
    ]
);

pub(super) fn init(cx: &mut App) {
    cx.bind_keys([
        gpui_kit::KeyBinding::new("enter", ConfirmarMonitor, Some(CONTEXTO)),
        gpui_kit::KeyBinding::new("escape", CancelarEscolhaDeMonitor, Some(CONTEXTO)),
        gpui_kit::KeyBinding::new("1", PrimeiroMonitor, Some(CONTEXTO)),
        gpui_kit::KeyBinding::new("2", SegundoMonitor, Some(CONTEXTO)),
        gpui_kit::KeyBinding::new("3", TerceiroMonitor, Some(CONTEXTO)),
        gpui_kit::KeyBinding::new("4", QuartoMonitor, Some(CONTEXTO)),
    ]);
}

/// Os monitores, da esquerda para a direita, o marcado e o do app.
struct Escolha {
    todos: Vec<Monitor<DisplayId>>,
    sugerido: Option<DisplayId>,
    do_app: Option<DisplayId>,
}

impl Escolha {
    /// 🔑 **Lida da janela a cada quadro**, e não guardada ao abrir: um monitor
    /// tirado com a pergunta na tela some da lista, em vez de virar um botão
    /// que abre a janela em lugar nenhum.
    fn ler(window: &Window, cx: &App) -> Self {
        let do_app = window.display(cx).map(|tela| tela.id());
        let lembrado = Lembranca::ler(&caminho_do_modo_do_cliente()).monitor;
        let todos = cliente::em_ordem(
            cx.displays()
                .iter()
                .map(|tela| Monitor {
                    id: tela.id(),
                    uuid: tela.uuid().ok().map(|uuid| uuid.to_string()),
                    limites: tela.bounds(),
                })
                .collect(),
        );
        let sugerido = cliente::monitor_sugerido(&todos, lembrado.as_deref(), do_app);
        Self {
            todos,
            sugerido,
            do_app,
        }
    }
}

impl Aplicativo {
    /// Desiste: a pergunta fecha e nada abre.
    pub(crate) fn cancelar_escolha_de_monitor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.escolha_de_monitor.fechar(window, cx);
        cx.notify();
    }

    /// O `n`-ésimo monitor da esquerda (a partir de 0), se existir.
    fn escolher_monitor(&mut self, n: usize, window: &mut Window, cx: &mut Context<Self>) {
        let escolha = Escolha::ler(window, cx);
        if let Some(monitor) = escolha.todos.get(n) {
            self.abrir_cliente_em(monitor.id, escolha.do_app, cx);
        }
    }

    fn confirmar_monitor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let escolha = Escolha::ler(window, cx);
        if let Some(id) = escolha.sugerido {
            self.abrir_cliente_em(id, escolha.do_app, cx);
        }
    }

    /// A pergunta, no `Dialog` do gpui-kit: um botão por monitor, o marcado em
    /// destaque.
    pub(super) fn escolha_de_monitor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        // O foco é o que faz o `Enter` e o `1`–`4` chegarem aqui. Tomado no
        // primeiro quadro, porque quem abre (o botão, a Revelação) não tem a
        // janela à mão; o `Modal` guarda quem o tinha e devolve ao fechar.
        if !self.foco_da_escolha.is_focused(window) {
            let foco = self.foco_da_escolha.clone();
            self.escolha_de_monitor.focar(&foco, window, cx);
        }

        let escolha = Escolha::ler(window, cx);
        let do_app = escolha.do_app;
        let botoes = escolha.todos.iter().enumerate().fold(
            crate::dialogo::rodape_da_pergunta(cx).flex_wrap(),
            |rodape, (i, monitor)| {
                let id = monitor.id;
                let nome = format!("escolher-monitor-{}", i + 1);
                let botao = if Some(id) == escolha.sugerido {
                    estilo::botao_primario(nome, cx)
                } else {
                    estilo::botao_contorno(nome, cx)
                };
                rodape.child(
                    botao
                        .child(cliente::rotulo(i + 1, monitor, do_app))
                        .on_click(
                            cx.listener(move |app, _, _, cx| app.abrir_cliente_em(id, do_app, cx)),
                        ),
                )
            },
        );

        let miolo =
            v_flex()
                .id("escolha-de-monitor")
                .key_context(CONTEXTO)
                .track_focus(&self.foco_da_escolha)
                .on_action(cx.listener(|app, _: &ConfirmarMonitor, window, cx| {
                    app.confirmar_monitor(window, cx)
                }))
                .on_action(
                    cx.listener(|app, _: &CancelarEscolhaDeMonitor, window, cx| {
                        app.cancelar_escolha_de_monitor(window, cx)
                    }),
                )
                .on_action(cx.listener(|app, _: &PrimeiroMonitor, window, cx| {
                    app.escolher_monitor(0, window, cx)
                }))
                .on_action(cx.listener(|app, _: &SegundoMonitor, window, cx| {
                    app.escolher_monitor(1, window, cx)
                }))
                .on_action(cx.listener(|app, _: &TerceiroMonitor, window, cx| {
                    app.escolher_monitor(2, window, cx)
                }))
                .on_action(cx.listener(|app, _: &QuartoMonitor, window, cx| {
                    app.escolher_monitor(3, window, cx)
                }))
                .gap(px(8.))
                .child(crate::dialogo::miolo_da_pergunta(
                    "pergunta-do-monitor",
                    "Em qual monitor abrir a tela do cliente?",
                    "No monitor do cliente ela abre em tela cheia; F alterna para janela.",
                    cx,
                ))
                .child(
                    gpui_kit::div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Enter abre no destacado · 1–4 escolhem da esquerda para a direita"),
                )
                .into_any_element();

        crate::dialogo::desenhar_conteudo(
            Some(miolo),
            Some(botoes.into_any_element()),
            crate::dialogo::Jeito::dialogo(560.),
            |app, window, cx| app.cancelar_escolha_de_monitor(window, cx),
            window,
            cx,
        )
    }
}
