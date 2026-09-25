//! 🪟 **Os botões de janela em todas as telas** — minimizar, maximizar e
//! fechar, no canto de cima, um jogo só.
//!
//! *"Você precisa testar melhor! Pois tem tela que não tem o minimizar,
//! maximizar e fechar, coloque um teste checando todas as telas!"* (dono,
//! 25/set/2026). No GNOME a janela não tem barra do sistema: se a tela não
//! desenha os botões, eles não existem — foi o que aconteceu na Revelação. E
//! na galeria eles ficavam na segunda linha, debaixo das guias, onde ninguém
//! procura.
//!
//! O teste faz de conta que é o GNOME (`janela::teste::forcar_barra_do_app`)
//! e, em cada tela, desenha um quadro e afirma três coisas:
//!
//! 1. os três botões estão desenhados;
//! 2. estão na **primeira linha** da janela, no **canto direito**;
//! 3. há **um jogo só** — a faixa das guias e a barra de baixo não desenham os
//!    dois ao mesmo tempo.
//!
//! 🔑 A lista de telas é conferida pelo compilador ([`toda_tela_esta_aqui`]):
//! uma `Tela` nova que não entre aqui não compila.

use gpui::{px, TestAppContext, VisualTestContext};

use super::{abrir_o_app, abrir_o_ensaio, Cenario, Estudio};
use crate::app::Tela;
use crate::janela::teste;

const BOTOES: [&str; 3] = ["janela-minimizar", "janela-maximizar", "janela-fechar"];

/// A primeira linha da janela: a faixa das guias (36 px) ou o cabeçalho
/// (até 56 px). Um botão abaixo disto está numa segunda linha.
const PRIMEIRA_LINHA: f32 = 60.;

/// Os três botões mais as margens: 3 × 20 + 2 × 12 + 2 × 12.
const CANTO_DIREITO: f32 = 140.;

/// As telas que a raiz troca por `ir_para` sem sessão aberta.
const SEM_SESSAO: [Tela; 7] = [
    Tela::Biblioteca,
    Tela::Impressao,
    Tela::Sessoes,
    Tela::Caixa,
    Tela::Retencao,
    Tela::NovaSessao,
    Tela::Backup,
];

/// 🔒 Uma `Tela` nova tem de entrar em [`SEM_SESSAO`] ou num cenário abaixo —
/// e este `match` sem `_` é o que obriga a olhar para cá.
#[allow(dead_code)]
fn toda_tela_esta_aqui(tela: Tela) {
    match tela {
        Tela::Biblioteca
        | Tela::Impressao
        | Tela::Sessoes
        | Tela::Caixa
        | Tela::Retencao
        | Tela::NovaSessao
        | Tela::Backup => {} // SEM_SESSAO
        Tela::Sessao | Tela::Revelacao => {} // os cenários com sessão
    }
}

/// Desenha um quadro e afirma: um jogo de botões, na primeira linha, no canto
/// direito.
fn conferir(e: &Estudio, cx: &mut TestAppContext, onde: &str) {
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.run_until_parked();
    teste::barras_com_botoes();
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();

    let barras = teste::barras_com_botoes();
    assert_eq!(
        barras.len(),
        1,
        "{onde}: as barras com botões de janela foram {barras:?} — tem de ser uma só"
    );
    let largura = visual.update(|window, _| window.viewport_size().width);
    for botao in BOTOES {
        let b = visual
            .debug_bounds(botao)
            .unwrap_or_else(|| panic!("{onde}: o botão {botao} não está na tela ({barras:?})"));
        assert!(
            b.bottom() <= px(PRIMEIRA_LINHA),
            "{onde}: {botao} não está na primeira linha da janela ({b:?}, {barras:?})"
        );
        assert!(
            b.right() >= largura - px(CANTO_DIREITO),
            "{onde}: {botao} não está no canto direito ({b:?}, largura {largura:?}, {barras:?})"
        );
    }
}

fn ir(e: &Estudio, cx: &mut TestAppContext, tela: Tela) {
    e.app(cx, |app, window, cx| app.ir_para(tela, window, cx));
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), tela, "ir_para({tela:?}) chegou lá");
    });
}

/// 🎬 **A porta** — a primeira tela que o app mostra, e a que não tem
/// cabeçalho: os botões moram por cima da capa.
#[gpui::test]
fn a_tela_de_entrada_tem_os_botoes_de_janela(cx: &mut TestAppContext) {
    teste::forcar_barra_do_app();
    let e = abrir_o_app(cx, Cenario::default());
    conferir(&e, cx, "entrada");
}

/// 🎬 **Toda tela do menu, sem sessão aberta** — o cabeçalho do app, ou a
/// barra da própria tela onde ele não existe (a nova sessão).
#[gpui::test]
fn toda_tela_sem_sessao_tem_os_botoes_de_janela(cx: &mut TestAppContext) {
    teste::forcar_barra_do_app();
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    for tela in SEM_SESSAO {
        ir(&e, cx, tela);
        conferir(&e, cx, &format!("{tela:?} sem sessão"));
    }
}

/// 🎬 **Com sessão aberta em guia** — a galeria, a lista de sessões e a
/// Revelação têm a faixa das guias em cima: os botões moram nela, e a barra de
/// baixo não repete.
#[gpui::test]
fn com_guia_aberta_os_botoes_ficam_na_faixa_das_guias(cx: &mut TestAppContext) {
    teste::forcar_barra_do_app();
    let e = abrir_o_ensaio(cx, Cenario::default());
    conferir(&e, cx, "Sessao com guia");

    // A Revelação — a tela da queixa.
    e.revelar_pela_barra(cx);
    conferir(&e, cx, "Revelacao com guia");

    // As outras telas do menu, com a guia ainda aberta.
    for tela in SEM_SESSAO {
        ir(&e, cx, tela);
        conferir(&e, cx, &format!("{tela:?} com guia aberta"));
    }
}

/// Desenha a tela do cliente e afirma: os três botões, no alto, à direita.
fn conferir_o_cliente(janela: gpui::AnyWindowHandle, cx: &mut TestAppContext, onde: &str) {
    let mut visual = VisualTestContext::from_window(janela, cx);
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    let largura = visual.update(|window, _| window.viewport_size().width);
    for botao in BOTOES {
        let b = visual
            .debug_bounds(botao)
            .unwrap_or_else(|| panic!("{onde}: o botão {botao} não está na tela do cliente"));
        assert!(b.bottom() <= px(PRIMEIRA_LINHA), "{onde}: {botao} fora do alto ({b:?})");
        assert!(
            b.right() >= largura - px(CANTO_DIREITO),
            "{onde}: {botao} fora do canto direito ({b:?})"
        );
    }
}

/// 🎬 **A tela do cliente tem os botões em janela e em tela cheia** — e, em
/// tela cheia, "Restaurar" sai dela (dono, 25/set/2026: *"mesmo que estiver
/// com tela cheia precisa ter o minimizar, restaurar e fechar"*).
#[gpui::test]
fn a_tela_do_cliente_tem_os_botoes_mesmo_em_tela_cheia(cx: &mut TestAppContext) {
    use crate::sessoes::detalhe::Pedido;

    teste::forcar_barra_do_app();
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.detalhe(cx, |tela, _w, cx| {
        tela.focar_foto("a", cx);
        cx.emit(Pedido::TelaDoCliente);
    });
    let janela: gpui::AnyWindowHandle = e
        .app(cx, |app, _w, _cx| app.janela_do_cliente_para_teste())
        .expect("a tela do cliente abriu")
        .into();
    conferir_o_cliente(janela, cx, "cliente em janela");

    let mut visual = VisualTestContext::from_window(janela, cx);
    visual.update(|window, _| {
        if !window.is_fullscreen() {
            window.toggle_fullscreen();
        }
    });
    visual.run_until_parked();
    assert!(visual.update(|window, _| window.is_fullscreen()), "entrou em tela cheia");
    conferir_o_cliente(janela, cx, "cliente em tela cheia");

    // "Restaurar" em tela cheia sai dela — pelo clique de verdade.
    let mut visual = VisualTestContext::from_window(janela, cx);
    let restaurar = visual
        .debug_bounds("janela-maximizar")
        .expect("o botão do meio está lá");
    visual.simulate_click(restaurar.center(), gpui::Modifiers::none());
    visual.run_until_parked();
    assert!(
        !visual.update(|window, _| window.is_fullscreen()),
        "Restaurar tirou a tela do cliente da tela cheia"
    );
}
