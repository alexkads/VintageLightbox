//! 🖥️ A segunda tela, virada para o cliente.

use gpui_kit::TestAppContext;

use super::{abrir_o_ensaio, Cenario};
use crate::app::Tela;
use crate::sessoes::detalhe::Pedido;

/// A foto e a exposição que a tela do cliente recebeu por último.
fn no_cliente(e: &super::Estudio, cx: &mut TestAppContext) -> Option<(String, f32)> {
    e.app(cx, |app, _w, _cx| {
        app.receita_no_cliente()
            .map(|(id, ajustes)| (id, ajustes.exposure))
    })
}

/// 🎬 **A tela do cliente acompanha o operador**: abre pelo botão da galeria
/// na foto em foco, segue o foco, entra na revelação com ele, vê cada gesto,
/// a prévia da predefinição e o "Antes" segurado — e fecha pelo mesmo botão.
#[gpui_kit::test]
fn a_tela_do_cliente_acompanha_a_galeria_e_a_revelacao(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    e.detalhe(cx, |tela, _w, cx| {
        tela.focar_foto("a", cx);
        cx.emit(Pedido::TelaDoCliente);
    });
    e.app(cx, |app, _w, _cx| {
        assert!(app.cliente_aberto(), "o botão da galeria abre")
    });
    assert_eq!(no_cliente(&e, cx), Some(("site:a".into(), 0.0)));

    // O foco anda na grade: o cliente acompanha.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("b", cx));
    assert_eq!(no_cliente(&e, cx).map(|c| c.0).as_deref(), Some("site:b"));

    // Na revelação, a foto aberta e cada gesto.
    e.revelar_a_do_site(cx, "a");
    assert_eq!(no_cliente(&e, cx).map(|c| c.0).as_deref(), Some("site:a"));
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 1.2, cx));
    assert_eq!(
        no_cliente(&e, cx),
        Some(("site:a".into(), 1.2)),
        "a edição ao vivo"
    );

    // A prévia da predefinição também chega ao cliente.
    e.revelacao(cx, |tela, _w, cx| {
        let escuro = tela.predefinicao("Escuro").expect("a do sistema");
        tela.prever(Some(&escuro), cx);
    });
    assert_eq!(no_cliente(&e, cx), Some(("site:a".into(), -1.0)));
    e.revelacao(cx, |tela, _w, cx| tela.prever(None, cx));
    assert_eq!(no_cliente(&e, cx), Some(("site:a".into(), 1.2)));

    // O "Antes": segurar o `\` mostra a foto sem ajuste, soltar devolve.
    e.teclar(cx, "\\");
    e.revelacao(cx, |tela, _w, _cx| assert!(tela.mostrando_original()));
    assert_eq!(
        no_cliente(&e, cx),
        Some(("site:a".into(), 0.0)),
        "o cliente vê o antes"
    );
    e.soltar(cx, "\\");
    e.revelacao(cx, |tela, _w, _cx| assert!(!tela.mostrando_original()));
    assert_eq!(no_cliente(&e, cx), Some(("site:a".into(), 1.2)));

    // Sair da revelação: a galeria volta a mandar — e a foto que acabou de
    // ser revelada continua revelada do lado do cliente.
    e.esperar(cx);
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessao));
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("a", cx));
    assert_eq!(
        no_cliente(&e, cx),
        Some(("site:a".into(), 1.2)),
        "de volta à galeria, o cliente não pode ver a foto de antes da revelação"
    );

    // O mesmo botão fecha.
    e.detalhe(cx, |_tela, _w, cx| cx.emit(Pedido::TelaDoCliente));
    e.app(cx, |app, _w, _cx| {
        assert!(!app.cliente_aberto(), "e o mesmo botão fecha")
    });
}
