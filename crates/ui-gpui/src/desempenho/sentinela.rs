//! O sentinela: o elemento pintado por último em cada quadro da janela
//! principal — é ele que diz "este quadro terminou".
//!
//! # Por que um elemento, e não o `render`
//!
//! 🚨 O pedido é explícito: **não contar chamadas de `render` como FPS**. O
//! `render` de uma view pode rodar sem virar quadro, e várias views renderizam
//! num mesmo quadro. O que o GPUI 0.3 faz por quadro é `window.draw()` (layout
//! e pintura da árvore inteira) seguido de `window.present()`, na mesma volta
//! do laço — e ele não expõe gancho para nenhum dos dois.
//!
//! Então: um `canvas` de tamanho zero dentro de um `deferred` de prioridade
//! máxima. Os `deferred` são pintados depois da árvore, em ordem de prioridade,
//! e este vem por último: a pintura dele é o fim do `draw`. Dali ele agenda uma
//! tarefa na thread da interface, que só roda **depois** de a volta do laço
//! terminar — depois do `present` —, e o horário dela é o teto da apresentação.
//!
//! # A rolagem, de qualquer tela
//!
//! O sentinela também registra, durante a pintura, um ouvinte de rolagem na
//! **fase de captura**: ela passa por todos os ouvintes antes de qualquer
//! elemento poder parar a propagação, então nenhuma lista ou grade a esconde.
//!
//! 🔑 Desligada a captura, o sentinela nem entra na árvore (`Aplicativo`).

use gpui_kit::{canvas, deferred, prelude::*, px, DispatchPhase, ScrollWheelEvent};

use super::Operacao;

/// O sentinela, para ser o último filho da raiz enquanto a captura está ligada.
pub fn sentinela() -> impl IntoElement {
    deferred(
        canvas(
            |_, _, _| (),
            |_, _, window, cx| {
                window.on_mouse_event(|_: &ScrollWheelEvent, fase, _, _| {
                    if fase == DispatchPhase::Capture {
                        super::operacao(Operacao::Rolagem);
                    }
                });
                if let Some(seq) = super::quadro_pintado() {
                    // A primeira tarefa depois do quadro: o `present` já foi.
                    cx.foreground_executor()
                        .spawn(async move { super::quadro_apresentado(seq) })
                        .detach();
                }
            },
        )
        .w(px(0.))
        .h(px(0.)),
    )
    .with_priority(usize::MAX)
}
