//! Os gestos da **janela** na barra do app: dois cliques para maximizar e
//! arrastar para mover.
//!
//! # Por que isto existe
//!
//! *"No `crates/ui-gpui` eu não consigo maximizar com dois cliques na barra e
//! nem movimentar no Fedora"* (dono, 17/set/2026), e logo depois: *"o ajuste
//! precisa ser no Mac, Linux e Windows"*.
//!
//! A barra que o operador vê no alto do app **não é a barra da janela** — é o
//! cabeçalho de 56 px do próprio app, interface como qualquer outra. Nos três
//! sistemas ela ignorava os dois gestos; o que mudava era o quanto isso doía:
//!
//! | | barra do sistema | o que faltava |
//! |---|---|---|
//! | macOS | sim, acima do cabeçalho | os gestos **no cabeçalho** |
//! | Windows | sim, acima do cabeçalho | os gestos **no cabeçalho** |
//! | Fedora (GNOME/Wayland) | **não existe** | os gestos, e não havia outro lugar |
//!
//! O GNOME roda Wayland e **não implementa `xdg-decoration`**, o protocolo com
//! que um app pede a barra ao compositor: toda janela ali é decorada pelo
//! próprio app. O GPUI, nesse modo, desenha a moldura e a sombra — e mais nada.
//! Por isso a queixa veio de lá: no Mac e no Windows sobrava a barra do sistema
//! logo acima, e no Fedora não sobrava nada.
//!
//! # 🚨 Duas armadilhas que custaram leitura do GPUI
//!
//! 1. **`WindowControlArea` não resolve.** Marcar a região com
//!    `.window_control_area(WindowControlArea::Drag)` parece ser a resposta, e é
//!    — **só no Windows**, onde vira `HTCAPTION`. No Wayland, no X11 e no macOS
//!    o `on_hit_test_window_control` do GPUI 0.2.2 é um método vazio: a marcação
//!    é aceita e ignorada. E no Windows ela entregaria ao sistema a faixa
//!    inteira, **com os botões que moram nela** — o do menu lateral e o da
//!    conta —, que é troca ruim.
//! 2. **`start_window_move` só existe no Linux.** No `platform.rs` do GPUI ele
//!    tem corpo vazio por padrão, e nem o backend do macOS nem o do Windows o
//!    sobrescrevem. Chamá-lo lá não falha: não faz nada.
//!
//! # O que fica valendo, por sistema
//!
//! - **Dois cliques → maximiza e restaura: nos três.** No macOS por
//!   `titlebar_double_click`, que obedece à preferência do sistema
//!   (`AppleActionOnDoubleClick` — há quem a ponha em minimizar, ou em nada);
//!   nos outros por `zoom_window`.
//! - **Arrastar → move: no Linux.** É onde não há alternativa. No macOS e no
//!   Windows a barra do sistema continua logo acima e move a janela como sempre
//!   moveu; fazer o **cabeçalho** arrastar exigiria descer ao AppKit
//!   (`performWindowDragWithEvent:`) e ao Win32 (`WM_NCLBUTTONDOWN`), e os dois
//!   abrem laço de evento próprio dentro de um `update` do GPUI — que é
//!   exatamente o que derruba o app com "RefCell already borrowed"
//!   (`segundo_plano/janela.rs` registra a lição, conferida em 17/set/2026).
//!   Fica para quando houver uma máquina de cada para conferir.

use gpui::{
    div, prelude::*, px, App, Div, Hsla, InteractiveElement, MouseButton, SharedString, Stateful,
    Window,
};
use gpui_component::{ActiveTheme as _, Icon, InteractiveElementExt as _};

use crate::recursos::Icone;

/// Um elemento que passa a se comportar como barra de título.
///
/// 🔑 **O arrasto não começa no `mouse_down`, e sim no primeiro movimento com o
/// botão apertado.** `start_window_move` entrega o ponteiro ao compositor:
/// chamá-lo já na descida roubaria o clique dos botões que moram dentro da
/// barra, que nunca chegariam a disparar. Esperar o movimento é o que separa
/// "clicou" de "arrastou" — e é como o `TitleBar` do `gpui-component` faz.
///
/// `id` é o do elemento no GPUI: duas barras com o mesmo id na mesma janela
/// dividiriam estado sem querer. Hoje elas nunca coexistem — a tela da sessão
/// não tem o cabeçalho do app —, e é justamente por isso que o dia em que
/// coexistirem não pode passar despercebido.
pub fn como_barra_de_titulo(
    elemento: Div,
    id: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let arrastando = window.use_state(cx, |_, _| Arrastando(false));

    elemento
        .id(id)
        .on_double_click(|_, window, cx| maximizar_ou_restaurar(window, cx))
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(&arrastando, |estado, _, _, _| estado.0 = true),
        )
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(&arrastando, |estado, _, _, _| estado.0 = false),
        )
        .on_mouse_down_out(window.listener_for(&arrastando, |estado, _, _, _| estado.0 = false))
        .on_mouse_move(window.listener_for(&arrastando, |estado, _, window, _| {
            if estado.0 {
                // Uma vez só: daqui em diante quem manda no ponteiro é o
                // compositor, e o `mouse_up` que apagaria isto não chega mais.
                estado.0 = false;
                window.start_window_move();
            }
        }))
}

/// Maximiza, ou restaura se já estiver maximizada.
///
/// ⚠️ **No macOS não é `zoom_window`.** O duplo clique na barra de título é
/// configurável no sistema (`AppleActionOnDoubleClick`: aumentar/reduzir,
/// minimizar ou nada), e `titlebar_double_click` é o que consulta essa
/// preferência. Maximizar à força seria o app decidindo por quem já decidiu.
pub fn maximizar_ou_restaurar(window: &mut Window, _cx: &mut App) {
    #[cfg(target_os = "macos")]
    window.titlebar_double_click();
    #[cfg(not(target_os = "macos"))]
    window.zoom_window();
}

/// Se o botão esquerdo está apertado sobre a barra.
///
/// Entidade, e não campo da tela: a barra é montada em lugares diferentes, e
/// nenhum deles tem estado próprio para guardar isto.
struct Arrastando(bool);

impl Render for Arrastando {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Os botões de janela — minimizar, maximizar/restaurar e fechar.
///
/// # 🚨 Por que isto existe
///
/// *"O comportamento no Ubuntu 26 GNOME tá muito ruim, sem botão de fechar"*
/// (dono, 19/set/2026). O GNOME não decora janela nenhuma (o porquê está no
/// alto deste módulo), então **os botões que o operador procura no canto não
/// existem** — quem os desenha tem de ser o app. Até aqui o `ui-gpui` desenhava
/// nenhum: no Fedora e no Ubuntu a única saída era matar o processo.
///
/// Pior na **tela de entrada**, que não tem cabeçalho: ali não havia barra, nem
/// botão, nem gesto — a janela não fechava nem se movia, e é a primeira coisa
/// que o app mostra.
///
/// # No macOS e no Windows isto não desenha nada, de propósito
///
/// Lá o sistema já põe os seus logo acima do cabeçalho. Desenhar os nossos
/// deixaria **dois** jogos de botão na mesma janela, e o de baixo faria o que o
/// de cima já faz. Por isso a função devolve um `div()` vazio fora do Linux, em
/// vez de o chamador lembrar de um `cfg!` em cada lugar.
///
/// `prefixo` entra no id de cada botão: duas barras na mesma janela com o mesmo
/// id dividiriam estado sem querer.
pub fn controles(prefixo: &'static str, cor: Hsla, window: &Window, cx: &App) -> Div {
    if !cfg!(target_os = "linux") {
        return div();
    }

    // O fundo do hover: cinza do tema nos dois primeiros, vermelho no fechar —
    // o mesmo contrato visual de qualquer barra de título, e o que distingue o
    // botão que **encerra** dos que só mudam o tamanho.
    let realce = cx.theme().secondary_hover;
    let perigo = cx.theme().danger;

    let (icone_do_meio, dica_do_meio) = if window.is_maximized() {
        (Icone::Minimize2, "Restaurar")
    } else {
        (Icone::Maximize2, "Maximizar")
    };

    div()
        .flex()
        .flex_none()
        .items_center()
        .child(botao(
            SharedString::from(format!("{prefixo}-minimizar")),
            Icone::Minus,
            "Minimizar",
            cor,
            realce,
            |window, _| window.minimize_window(),
        ))
        .child(botao(
            SharedString::from(format!("{prefixo}-maximizar")),
            icone_do_meio,
            dica_do_meio,
            cor,
            realce,
            maximizar_ou_restaurar,
        ))
        .child(botao(
            SharedString::from(format!("{prefixo}-fechar")),
            Icone::X,
            "Fechar",
            cor,
            perigo,
            // 🚨 `remove_window`, e não `quit`: fechar a janela principal é o
            // gesto de sair do app, mas quem decide o que fazer com as outras
            // (a segunda tela, um modal do sistema) é o GPUI.
            |window, _| window.remove_window(),
        ))
}

/// Um botão da barra: o quadrado com o ícone, o realce e o clique.
fn botao(
    id: impl Into<gpui::ElementId>,
    icone: Icone,
    dica: &'static str,
    cor: Hsla,
    realce: Hsla,
    acao: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(34.))
        .h(px(28.))
        .rounded(px(4.))
        .cursor_pointer()
        .text_color(cor)
        .hover(move |s| s.bg(realce))
        .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(dica).build(window, cx))
        // 🔑 O clique **para aqui**. Estes botões moram dentro da barra que
        // arrasta a janela: sem isto, apertar "fechar" começaria um arrasto e o
        // compositor levaria o ponteiro embora antes do clique acontecer.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            acao(window, cx);
        })
        .child(Icon::new(icone).size(px(15.)))
}
