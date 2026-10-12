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
                .map(|tela| {
                    let uuid = tela.uuid().ok().map(|uuid| uuid.to_string());
                    Monitor {
                        id: tela.id(),
                        nome: cliente::nome_do_monitor::do_sistema(
                            tela.id().into(),
                            uuid.as_deref(),
                        ),
                        uuid,
                        limites: tela.bounds(),
                    }
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
        let tema = cx.theme();
        let (borda, apagado, primaria, sobre_primaria, fundo, cartao) = (
            tema.border,
            tema.muted_foreground,
            tema.primary,
            tema.primary_foreground,
            tema.background,
            tema.muted,
        );

        // 🖥️ **Cada monitor desenhado como uma tela** (dono, 12/out/2026:
        // *"deixe esse modal mais bonito, com um desenho de uma tela"*), na
        // ordem e na proporção em que estão na mesa: o operador reconhece o
        // monitor pela forma antes de ler o nome. A maior tela dá a escala, e
        // as outras saem proporcionais a ela.
        const MAIOR_TELA: f32 = 184.;
        let maior = escolha
            .todos
            .iter()
            .map(|m| f32::from(m.limites.size.width).max(f32::from(m.limites.size.height)))
            .fold(1.0_f32, f32::max);
        let telas = escolha.todos.iter().enumerate().fold(
            gpui_kit::div()
                .flex()
                .flex_wrap()
                .items_end()
                .justify_center()
                .gap(px(20.))
                .pt(px(8.)),
            |linha, (i, monitor)| {
                let id = monitor.id;
                let destacado = Some(id) == escolha.sugerido;
                let e_o_do_app = do_app == Some(id);
                let tamanho = monitor.limites.size;
                let (largura, altura) = (
                    f32::from(tamanho.width) / maior * MAIOR_TELA,
                    f32::from(tamanho.height) / maior * MAIOR_TELA,
                );
                let contorno = if destacado { primaria } else { borda };
                let seletor = format!("escolher-monitor-{}", i + 1);
                let dica = cliente::rotulo(i + 1, monitor, do_app);
                linha.child(
                    v_flex()
                        .id(gpui_kit::SharedString::from(seletor.clone()))
                        .debug_selector(move || seletor)
                        .items_center()
                        .gap(px(10.))
                        .p(px(12.))
                        .min_w(px(168.))
                        .rounded(crate::tema::canto(12.))
                        .border_1()
                        .border_color(if destacado {
                            primaria.opacity(0.6)
                        } else {
                            gpui_kit::transparent_black()
                        })
                        .when(destacado, |c| c.bg(primaria.opacity(0.08)))
                        .hover(|c| c.bg(cartao))
                        .cursor_pointer()
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(dica.clone())
                                .build(window, cx)
                        })
                        .on_click(
                            cx.listener(move |app, _, _, cx| app.abrir_cliente_em(id, do_app, cx)),
                        )
                        // A tela, o pescoço e o pé.
                        .child(
                            v_flex()
                                .items_center()
                                .child(
                                    gpui_kit::div()
                                        .relative()
                                        .w(px(largura))
                                        .h(px(altura))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(crate::tema::canto(6.))
                                        .border_2()
                                        .border_color(contorno)
                                        .bg(if destacado {
                                            primaria.opacity(0.18)
                                        } else {
                                            fundo
                                        })
                                        // O número, grande: é o que as teclas
                                        // 1–4 escolhem.
                                        .child(
                                            gpui_kit::div()
                                                .text_size(px((altura * 0.42).clamp(18., 44.)))
                                                .font_weight(gpui_kit::FontWeight::BOLD)
                                                .text_color(if destacado {
                                                    primaria
                                                } else {
                                                    apagado
                                                })
                                                .child((i + 1).to_string()),
                                        )
                                        // Uma janelinha no canto: é nesta
                                        // tela que o app está aberto.
                                        .when(e_o_do_app, |tela| {
                                            tela.child(
                                                gpui_kit::div()
                                                    .absolute()
                                                    .left(px(8.))
                                                    .bottom(px(8.))
                                                    .w(px(largura * 0.3))
                                                    .h(px(altura * 0.3))
                                                    .rounded(crate::tema::canto(3.))
                                                    .border_1()
                                                    .border_color(apagado)
                                                    .bg(cartao)
                                                    .child(
                                                        gpui_kit::div()
                                                            .h(px(4.))
                                                            .rounded_t(crate::tema::canto(2.))
                                                            .bg(apagado),
                                                    ),
                                            )
                                        }),
                                )
                                .child(gpui_kit::div().w(px(14.)).h(px(8.)).bg(contorno))
                                .child(
                                    gpui_kit::div()
                                        .w(px(56.))
                                        .h(px(4.))
                                        .rounded(px(2.))
                                        .bg(contorno),
                                ),
                        )
                        // O nome como o sistema o chama, e o tamanho.
                        .child(
                            v_flex()
                                .items_center()
                                .gap(px(2.))
                                .child(
                                    gpui_kit::div()
                                        .text_sm()
                                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                                        .child(
                                            monitor
                                                .nome
                                                .clone()
                                                .unwrap_or_else(|| format!("Monitor {}", i + 1)),
                                        ),
                                )
                                .child(gpui_kit::div().text_xs().text_color(apagado).child(
                                    format!(
                                        "{} × {}",
                                        f32::from(tamanho.width).round() as i32,
                                        f32::from(tamanho.height).round() as i32,
                                    ),
                                )),
                        )
                        // Para que serve cada uma, numa pílula.
                        .child(
                            gpui_kit::div()
                                .px(px(8.))
                                .py(px(2.))
                                .rounded(px(999.))
                                .text_xs()
                                .when(destacado, |p| p.bg(primaria).text_color(sobre_primaria))
                                .when(!destacado, |p| {
                                    p.border_1().border_color(borda).text_color(apagado)
                                })
                                .child(match (destacado, e_o_do_app) {
                                    (true, true) => "Abre aqui · onde está o app",
                                    (true, false) => "Abre aqui",
                                    (false, true) => "Onde está o app",
                                    (false, false) => "Disponível",
                                }),
                        ),
                )
            },
        );

        // O rodapé: as teclas à esquerda, o botão do destacado à direita.
        let nome_do_destacado = escolha
            .todos
            .iter()
            .enumerate()
            .find(|(_, m)| Some(m.id) == escolha.sugerido)
            .map(|(i, m)| {
                m.nome
                    .clone()
                    .unwrap_or_else(|| format!("Monitor {}", i + 1))
            });
        let rodape = crate::dialogo::rodape_da_pergunta(cx)
            .justify_between()
            .items_center()
            .child(
                gpui_kit::component::h_flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .text_xs()
                    .text_color(apagado)
                    .child(estilo::tecla("1"))
                    .child("–")
                    .child(estilo::tecla("4"))
                    .child("escolhem ·")
                    .child(estilo::tecla("F"))
                    .child("tela cheia ou janela ·")
                    .child(estilo::tecla("Esc"))
                    .child("cancela"),
            )
            .children(nome_do_destacado.map(|nome| {
                estilo::botao_primario("abrir-no-monitor-destacado", cx)
                    .child(format!("Abrir em {nome}"))
                    .child(estilo::tecla("Enter"))
                    .on_click(cx.listener(|app, _, window, cx| app.confirmar_monitor(window, cx)))
            }));

        let miolo = v_flex()
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
            .on_action(
                cx.listener(|app, _: &QuartoMonitor, window, cx| {
                    app.escolher_monitor(3, window, cx)
                }),
            )
            .gap(px(12.))
            .child(crate::dialogo::miolo_da_pergunta(
                "pergunta-do-monitor",
                "Em qual monitor abrir a tela do cliente?",
                "Escolha a tela que fica virada para o cliente. Nela a foto abre em tela cheia.",
                cx,
            ))
            .child(telas)
            .into_any_element();

        crate::dialogo::desenhar_conteudo(
            Some(miolo),
            Some(rodape.into_any_element()),
            crate::dialogo::Jeito::dialogo(640.),
            |app, window, cx| app.cancelar_escolha_de_monitor(window, cx),
            window,
            cx,
        )
    }
}
