//! ✂️ O Enquadrar: o `R` do site, com as teclas dele.

use gpui::TestAppContext;

use super::{abrir_o_ensaio, Cenario};
use crate::app::Tela;
use crate::revelacao::corte::Alca;

fn perto(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

/// 🎬 **Enquadrar de ponta a ponta**, numa foto de 160×120: `R` entra, a
/// alça recorta, `]` e `[` giram **levando o retângulo**, `⇧H`/`⇧V`
/// espelham, o endireitar encolhe e cresce de volta, a proporção 1:1 segura
/// o arrasto, `Enter` confirma, `Esc` só sai da ferramenta, "Voltar à foto
/// inteira" recomeça e o `⌘Z` devolve o enquadramento.
#[gpui::test]
fn enquadrar_girar_espelhar_endireitar_e_proporcao(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");

    // R entra na ferramenta; o zoom fica desligado nela.
    e.teclar(cx, "r");
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(tela.cortando(), "R abre o Enquadrar");
        assert_eq!(tela.proporcao_travada(), None);
    });
    e.teclar(cx, "z");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.nivel_do_zoom(),
            crate::revelacao::zoom::Nivel::Encaixar,
            "no Enquadrar não há zoom"
        );
    });

    // A alça esquerda, 40 px para dentro: um quarto da foto.
    e.revelacao(cx, |tela, window, cx| {
        tela.arrastar_no_corte(Some(Alca::Esquerda), 40., 0., window, cx);
        let c = tela.corte_na_ferramenta();
        assert!(perto(c.crop_x(), 0.25), "x = {}", c.crop_x());
        assert!(perto(c.crop_width(), 0.75));
        assert!(tela.cortando(), "soltar não fecha a ferramenta");
    });
    let gravado = e.gravador.gravado();
    assert_eq!(
        gravado.last().map(|g| g.2.x),
        Some(Some(0.25)),
        "a alça grava ao soltar"
    );

    // ] gira à direita e o retângulo vai junto: (x, y, l, a) → (1−y−a, x, a, l).
    e.teclar(cx, "]");
    e.revelacao(cx, |tela, _w, _cx| {
        let c = tela.corte_na_ferramenta();
        assert_eq!(c.rotation_90(), 1);
        assert!(perto(c.crop_x(), 0.0) && perto(c.crop_y(), 0.25));
        assert!(perto(c.crop_width(), 1.0) && perto(c.crop_height(), 0.75));
    });
    e.teclar(cx, "[");
    e.revelacao(cx, |tela, _w, _cx| {
        let c = tela.corte_na_ferramenta();
        assert_eq!(c.rotation_90(), 0, "[ desfaz o ]");
        assert!(perto(c.crop_x(), 0.25) && perto(c.crop_width(), 0.75));
    });

    // Espelhar nos dois eixos, e de volta no horizontal.
    e.teclar(cx, "shift-h shift-v");
    e.revelacao(cx, |tela, _w, _cx| {
        let c = tela.corte_na_ferramenta();
        assert!(c.flip_horizontal() && c.flip_vertical());
    });
    e.teclar(cx, "shift-h");
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(!tela.corte_na_ferramenta().flip_horizontal());
    });

    // Endireitar pelo slider: o retângulo encolhe para não mostrar canto
    // vazio, e cresce de volta ao zerar.
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_angulo(8., cx));
    e.revelacao(cx, |tela, _w, _cx| {
        let c = tela.corte_na_ferramenta();
        assert!(perto(c.angle(), 8.), "ângulo {}", c.angle());
        assert!(c.crop_width() < 0.75, "encolheu: {}", c.crop_width());
    });
    e.esperar(cx);
    assert!(
        e.gravador
            .gravado()
            .last()
            .is_some_and(|g| g.2.angulo.is_some_and(|a| perto(a, 8.))),
        "o endireitar grava quando o gesto assenta"
    );
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_angulo(0., cx));
    e.revelacao(cx, |tela, _w, _cx| {
        let c = tela.corte_na_ferramenta();
        assert!(perto(c.angle(), 0.));
        assert!(
            perto(c.crop_width(), 0.75),
            "cresceu de volta: {}",
            c.crop_width()
        );
    });
    e.esperar(cx);

    // Proporção 1:1: remodela na hora, e a alça arrasta mantendo-a.
    e.revelacao(cx, |tela, window, cx| {
        tela.travar_proporcao(Some(1.0), cx);
        assert_eq!(tela.proporcao_travada(), Some(1.0));
        let quadrado = |tela: &crate::revelacao::tela::Revelacao| {
            let c = tela.corte_na_ferramenta();
            (c.crop_width() * 160., c.crop_height() * 120.)
        };
        let (l, a) = quadrado(tela);
        assert!((l - a).abs() <= 1., "1:1 na hora: {l} × {a}");
        tela.arrastar_no_corte(Some(Alca::Direita), -10., 0., window, cx);
        let (l2, a2) = quadrado(tela);
        assert!((l2 - a2).abs() <= 1., "1:1 depois do arrasto: {l2} × {a2}");
        assert!(l2 < l, "a alça encolheu o quadrado");
    });

    // Enter confirma e sai; o enquadramento fica.
    let antes_do_enter = e.revelacao(cx, |tela, _w, _cx| tela.enquadramento());
    e.teclar(cx, "enter");
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(!tela.cortando(), "Enter sai da ferramenta");
        assert_eq!(tela.enquadramento(), antes_do_enter);
    });

    // R de novo, e Esc só sai da ferramenta — a revelação continua.
    e.teclar(cx, "r");
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, cx| {
        assert_eq!(
            app.tela(),
            Tela::Revelacao,
            "Esc no Enquadrar não fecha o editor"
        );
        assert!(!app.revelacao.read(cx).cortando());
    });

    // "Voltar à foto inteira" e o ⌘Z que devolve o recorte.
    e.teclar(cx, "r");
    e.revelacao(cx, |tela, window, cx| {
        tela.recomecar_corte(window, cx);
        let c = tela.corte_na_ferramenta();
        assert!(perto(c.crop_x(), 0.) && perto(c.crop_width(), 1.) && perto(c.angle(), 0.));
    });
    e.teclar(cx, "escape");
    e.teclar(cx, "cmd-z");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.enquadramento(),
            antes_do_enter,
            "⌘Z devolve o enquadramento"
        );
    });

    // Esc fora da ferramenta fecha o editor.
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessao));
}
