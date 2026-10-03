//! 🧭 **O assunto da tela mora na faixa do cabeçalho** — título, descrição e
//! ações ao lado do botão do menu, e não no corpo.
//!
//! *"Caixa, Chatbot e Agendamento não estão utilizando header da tela
//! corretamente, ficando abaixo, deixando a mesma vazia"* (dono, 03/10/2026).
//! Cada uma desenhava o próprio cabeçalho dentro do corpo, e a faixa de 56 px
//! ficava só com o botão do menu. Backup e Retenção tinham o mesmo defeito.
//!
//! O teste desenha cada tela numa janela de 1280 × 720, com o menu recolhido e
//! aberto, e afirma: o título e as ações estão **dentro da faixa**, e as ações
//! **cabem na janela** (a faixa recorta o que passa da borda).

use gpui_kit::{px, size, TestAppContext, VisualTestContext};

use super::{abrir_o_app, Cenario, Estudio};
use crate::app::Tela;

/// O `h-14` do cabeçalho do app (`app::painel::ALTURA_DO_CABECALHO`).
const ALTURA_DO_CABECALHO: f32 = 56.;

const TELAS: [Tela; 5] = [
    Tela::Caixa,
    Tela::Chatbot,
    Tela::Agenda,
    Tela::Backup,
    Tela::Retencao,
];

fn conferir(e: &Estudio, cx: &mut TestAppContext, onde: &str) {
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    let largura = visual.update(|window, _| window.viewport_size().width);
    for alvo in ["cabecalho-titulo", "cabecalho-acoes"] {
        let b = visual
            .debug_bounds(alvo)
            .unwrap_or_else(|| panic!("{onde}: {alvo} não está desenhado"));
        assert!(
            b.top() >= px(0.) && b.bottom() <= px(ALTURA_DO_CABECALHO),
            "{onde}: {alvo} fora da faixa de {ALTURA_DO_CABECALHO} px ({b:?})"
        );
        assert!(
            b.right() <= largura - px(16.),
            "{onde}: {alvo} passa da borda da janela ({b:?}, largura {largura:?})"
        );
    }
}

#[gpui_kit::test]
fn o_assunto_de_cada_tela_fica_na_faixa_do_cabecalho(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    VisualTestContext::from_window(e.raiz.into(), cx).simulate_resize(size(px(1280.), px(720.)));
    for menu_aberto in [false, true] {
        e.app(cx, |app, _, cx| {
            if app.menu_lateral_aberto() != menu_aberto {
                app.alternar_menu_lateral(cx);
            }
        });
        for tela in TELAS {
            e.app(cx, |app, window, cx| app.ir_para(tela, window, cx));
            e.esperar(cx);
            conferir(
                &e,
                cx,
                &format!(
                    "{tela:?}, menu {}",
                    if menu_aberto { "aberto" } else { "recolhido" }
                ),
            );
        }
    }
}
