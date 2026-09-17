//! 🔍 O zoom da Revelação, a folha de atalhos e o bruto em resolução cheia.

use gpui::TestAppContext;

use super::{abrir_o_ensaio, Cenario};
use crate::revelacao::zoom::Nivel;

/// 🎬 **As teclas do zoom, e o bruto que chega quando a cópia não basta.**
///
/// A foto `a` tem cópia de trabalho de 160 px e bruto de 640 px no site: ao
/// abrir, o lado do bruto é medido (um download); ampliada além da cópia, o
/// palco troca para o bruto **sem baixar de novo**; de volta ao encaixe, a
/// cópia volta.
#[gpui::test]
fn teclas_do_zoom_e_o_bruto_em_resolucao_cheia(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    // Uma janela de notebook: a foto ampliada passa da área.
    cx.simulate_window_resize(e.raiz.into(), gpui::size(gpui::px(900.), gpui::px(640.)));
    e.revelar_a_do_site(cx, "a");
    e.esperar(cx);

    assert_eq!(
        e.site.originais(),
        vec!["a".to_string()],
        "o lado do bruto é medido uma vez"
    );
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.fator_do_bruto_medido(),
            4.0,
            "640 do bruto sobre 160 da cópia"
        );
        assert_eq!(tela.nivel_do_zoom(), Nivel::Encaixar);
        assert!(!tela.foto_ampliada());
        assert!(!tela.bruto_na_tela());
    });

    // Home fora do zoom não faz nada (a tecla segue adiante).
    let centro = e.revelacao(cx, |tela, _w, _cx| tela.centro_do_zoom());
    e.teclar(cx, "home");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.centro_do_zoom(), centro)
    });

    // Z tocado: vai ao 1:1 e fica (soltar logo não volta).
    e.teclar(cx, "z");
    e.soltar(cx, "z");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.nivel_do_zoom(), Nivel::Razao(1.));
        assert!(tela.foto_ampliada(), "o 1:1 do bruto passa da área");
    });

    // O bruto entra no palco depois da espera, com os bytes já baixados.
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(tela.bruto_na_tela(), "o palco trocou a cópia pelo bruto");
        assert!(tela.tem_pixels());
    });
    assert_eq!(
        e.site.originais().len(),
        1,
        "o bruto medido foi reaproveitado"
    );

    // Home, End e as páginas percorrem a foto ampliada.
    e.teclar(cx, "home");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.centro_do_zoom(), (0., 0.))
    });
    e.teclar(cx, "end");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.centro_do_zoom(), (1., 1.))
    });
    e.teclar(cx, "home pagedown");
    let depois_da_pagina = e.revelacao(cx, |tela, _w, _cx| tela.centro_do_zoom());
    assert_ne!(depois_da_pagina, (0., 0.), "PgDn anda uma tela");
    e.teclar(cx, "pageup");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_ne!(tela.centro_do_zoom(), depois_da_pagina, "PgUp volta");
    });

    // ⌘− afasta, ⌘= aproxima, ⌘0 encaixa e ⌘⌥0 volta ao 1:1.
    e.teclar(cx, "cmd--");
    let afastado = e.revelacao(cx, |tela, _w, _cx| tela.nivel_do_zoom());
    assert_ne!(afastado, Nivel::Razao(1.), "⌘− afastou");
    e.teclar(cx, "cmd-=");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_ne!(tela.nivel_do_zoom(), afastado, "⌘= aproximou")
    });
    e.teclar(cx, "cmd-0");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.nivel_do_zoom(), Nivel::Encaixar);
        assert!(!tela.foto_ampliada());
    });
    // Encaixada, a cópia volta (1,5 s depois).
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(!tela.bruto_na_tela(), "a cópia voltou")
    });
    e.teclar(cx, "ctrl-alt-0");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.nivel_do_zoom(), Nivel::Razao(1.))
    });

    // Espaço tocado alterna o zoom, como no Lightroom.
    e.teclar(cx, "space");
    e.soltar(cx, "space");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.nivel_do_zoom(), Nivel::Encaixar)
    });

    // ? abre e fecha a folha de atalhos.
    e.teclar(cx, "?");
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(tela.ajuda_aberta(), "? abre a folha")
    });
    e.teclar(cx, "shift-/");
    e.revelacao(cx, |tela, _w, _cx| assert!(!tela.ajuda_aberta(), "e fecha"));
}

/// 🎬 **O bruto que não vem**: a tela fica na cópia, sem erro nem espera
/// eterna.
///
/// 🔑 O bruto só é pedido quando o lado dele já foi medido — e o download da
/// medida fica guardado para o zoom. Por isso o cenário mede `a` (a primeira
/// da tira) e `b`, que toma o lugar na mão, e volta para `a`: aí o zoom
/// precisa baixar de novo, e é essa ida que o site recusa.
#[gpui::test]
fn sem_o_bruto_a_tela_fica_na_copia(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    cx.simulate_window_resize(e.raiz.into(), gpui::size(gpui::px(900.), gpui::px(640.)));
    e.revelar_a_do_site(cx, "b");
    e.esperar(cx);
    assert_eq!(e.site.originais(), vec!["a".to_string(), "b".to_string()]);

    *e.site.bruto.lock().unwrap() = None;
    e.revelar_a_do_site(cx, "a");
    e.esperar(cx);
    assert_eq!(e.site.originais().len(), 2, "o lado de a ficou lembrado");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.fator_do_bruto_medido(), 4.0)
    });

    e.teclar(cx, "cmd-alt-0");
    e.esperar(cx);
    assert_eq!(e.site.originais().len(), 3, "o zoom pediu o bruto de a");
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(!tela.bruto_na_tela());
        assert!(!tela.carregando_o_bruto(), "não fica esperando para sempre");
        assert!(tela.tem_pixels(), "a cópia continua na tela");
        assert_eq!(tela.nivel_do_zoom(), Nivel::Razao(1.), "e o zoom continua");
    });
}
