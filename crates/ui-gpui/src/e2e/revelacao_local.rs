//! 🖌️ A Revelação local pelo teclado.

use gpui_kit::{Modifiers, TestAppContext, VisualTestContext};

use super::{abrir_o_ensaio, Cenario};

/// 🎬 **`[` e `]` com o pincel na mão mudam o tamanho, não giram a foto**;
/// sem ferramenta, voltam a girar.
#[gpui_kit::test]
fn colchetes_mudam_o_tamanho_do_pincel(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");

    e.teclar(cx, "k");
    let antes = e.revelacao(cx, |tela, _w, _cx| {
        assert!(tela.com_ferramenta_local(), "K pega o pincel");
        tela.raio_local()
    });
    e.teclar(cx, "]");
    let maior = e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.corte_na_ferramenta().rotation_90(), 0, "] não gira");
        tela.raio_local()
    });
    assert!(maior > antes, "] aumenta: {antes} → {maior}");
    e.teclar(cx, "[ [");
    let menor = e.revelacao(cx, |tela, _w, _cx| tela.raio_local());
    assert!(menor < antes, "[ diminui: {antes} → {menor}");

    // 🚨 O cursor esquecido na busca de predefinições: ali `]` é texto.
    e.revelacao(cx, |tela, window, cx| tela.focar_a_busca(window, cx));
    e.teclar(cx, "]");
    assert_eq!(
        e.revelacao(cx, |tela, _w, _cx| tela.raio_local()),
        menor,
        "no campo, ] é texto"
    );
    // Clicar na foto devolve o teclado aos atalhos.
    let meio = e.revelacao(cx, |tela, _w, _cx| tela.meio_do_palco());
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_click(meio, Modifiers::default());
    visual.run_until_parked();
    e.teclar(cx, "]");
    let depois = e.revelacao(cx, |tela, _w, _cx| tela.raio_local());
    assert!(
        depois > menor,
        "clicar na foto tira o foco do campo: {menor} → {depois}"
    );
}
