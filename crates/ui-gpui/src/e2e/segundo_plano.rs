//! 🛎️ O trabalho que continua com a janela minimizada ou fechada — a bandeja.
//!
//! A janela do teste não tem alça nativa: o que o sistema faria com ela
//! (minimizar, esconder, tirar do Dock) passa pelo dublê de
//! `segundo_plano::janela::plataforma`, que anota os gestos e deixa o cenário
//! dizer se a janela está minimizada.

use std::time::Duration;

use gpui::{TestAppContext, VisualTestContext};

use super::{abrir_o_ensaio, Cenario, Estudio};
use crate::revelacao::tela::PedidoDaRevelacao;
use crate::segundo_plano::janela::plataforma;

/// `(na bandeja, pediu para sair)`.
fn bandeja(cx: &mut TestAppContext) -> (bool, bool) {
    cx.update(|cx| crate::segundo_plano::estado_para_teste(cx))
        .expect("o segundo plano está ligado")
}

/// Algumas voltas do laço da bandeja (uma olhada a cada ~0,75 s).
fn voltas(cx: &mut TestAppContext) {
    for _ in 0..8 {
        cx.executor().advance_clock(Duration::from_millis(150));
        cx.run_until_parked();
    }
}

/// O ensaio aberto, com uma revelação da foto `a` esperando o site responder.
fn com_envio_na_fila(cx: &mut TestAppContext) -> Estudio {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            segundo_plano: true,
            site: Box::new(|site| site.demorada = true),
            ..Cenario::default()
        },
    );
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.5, cx));
    e.esperar(cx);
    e.revelacao(cx, |_tela, _w, cx| {
        cx.emit(PedidoDaRevelacao::SalvarNaGaleria)
    });
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        assert!(app.sincronias_pendentes() > 0, "o envio está no ar");
        assert!(app.retrato_do_segundo_plano(cx).ha_envio_pendente());
    });
    e
}

/// 🎬 **Minimizar leva à bandeja, e voltar tira.** O trabalho não para: a
/// janela minimizada continua sendo a mesma.
#[gpui::test]
fn minimizar_vai_para_a_bandeja_e_voltar_tira(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            segundo_plano: true,
            ..Cenario::default()
        },
    );
    voltas(cx);
    assert_eq!(
        bandeja(cx),
        (false, false),
        "com a janela à vista, nada de bandeja"
    );

    plataforma::fingir_minimizada(true);
    voltas(cx);
    assert_eq!(
        bandeja(cx),
        (true, false),
        "minimizada: bandeja, e o app segue"
    );
    assert!(plataforma::gestos().contains(&"fora do dock"));

    plataforma::fingir_minimizada(false);
    voltas(cx);
    assert_eq!(
        bandeja(cx),
        (false, false),
        "de volta pela barra de tarefas"
    );
    assert!(plataforma::gestos().contains(&"no dock"));

    // Sem envio na fila, fechar fecha.
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    assert!(visual.simulate_close(), "nada pendente: a janela fecha");
}

/// 🎬 **G9: fechar com envio na fila só esconde**, fecha junto a tela do
/// cliente, e o app termina sozinho quando o site responde.
#[gpui::test]
fn fechar_com_envio_esconde_e_sai_quando_a_fila_esvazia(cx: &mut TestAppContext) {
    let e = com_envio_na_fila(cx);
    e.app(cx, |app, _w, cx| {
        app.alternar_cliente(cx);
        assert!(app.cliente_aberto());
    });

    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    assert!(
        !visual.simulate_close(),
        "com envio na fila a janela não fecha"
    );
    voltas(cx);

    // 🚪 **O primeiro fechamento avisa** (dono, 2026-09-20): a janela fica, e
    // o aviso diz o que está pendente.
    assert_eq!(
        bandeja(cx),
        (false, false),
        "a janela não some enquanto o aviso está na tela"
    );
    e.app(cx, |app, _w, _cx| {
        let aviso = app
            .aviso_de_fechamento_para_teste()
            .expect("o aviso tinha de estar na tela");
        assert!(
            aviso.contains("Subindo"),
            "e diz o que está pendente: {aviso}"
        );
    });

    // "Continuar em segundo plano": agora sim, a bandeja.
    e.app(cx, |app, _w, cx| app.fechar_em_segundo_plano(cx));
    voltas(cx);
    e.app(cx, |app, _w, _cx| {
        assert!(
            app.aviso_de_fechamento_para_teste().is_none(),
            "o aviso sai com a decisão"
        );
    });
    assert_eq!(bandeja(cx), (true, false), "foi para a bandeja, e não saiu");
    assert!(plataforma::gestos().contains(&"esconder"));
    e.app(cx, |app, _w, _cx| {
        assert!(
            !app.cliente_aberto(),
            "a tela do cliente não fica sozinha no monitor"
        );
    });

    // O site responde: a fila esvazia e o app termina.
    e.site.responder();
    voltas(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.sincronias_pendentes(), 0));
    voltas(cx);
    assert!(bandeja(cx).1, "a fila esvaziou: o app pediu para sair");
    assert_eq!(e.site.reveladas().len(), 1, "e o envio chegou antes");
}

/// 🎬 **Abrir de novo antes de a fila esvaziar mantém o app aberto** — quem
/// fechou está de volta.
#[gpui::test]
fn reabrir_antes_de_esvaziar_mantem_o_app(cx: &mut TestAppContext) {
    let e = com_envio_na_fila(cx);
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    assert!(!visual.simulate_close());
    voltas(cx);
    // O aviso vem primeiro; o operador responde "continuar em segundo plano".
    e.app(cx, |app, _w, cx| app.fechar_em_segundo_plano(cx));
    voltas(cx);
    assert_eq!(bandeja(cx), (true, false));

    // O ícone do Dock (ou "Abrir o VintageLightbox").
    cx.update(crate::segundo_plano::ao_reabrir);
    voltas(cx);
    assert_eq!(bandeja(cx), (false, false), "a janela voltou");
    assert!(plataforma::gestos().contains(&"mostrar"));

    e.site.responder();
    voltas(cx);
    voltas(cx);
    assert_eq!(
        bandeja(cx),
        (false, false),
        "com a janela à vista, o app não sai"
    );
}

/// 🎬 **Download no ar não é envio**: a foto `d` não tem cópia no cache e a
/// rede a segura, mas a bandeja não diz "Subindo" e fechar fecha — o G9 só
/// vale para envio (achado pelo estresse, 17/set/2026).
#[gpui::test]
fn download_no_ar_nao_segura_a_janela(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            segundo_plano: true,
            site: Box::new(|site| site.copia_demorada = true),
            ..Cenario::default()
        },
    );
    e.revelar_pela_barra(cx);
    e.revelacao(cx, |tela, window, cx| {
        let d = tela
            .acervo()
            .iter()
            .position(|f| f.id == "site:d")
            .expect("a d está na tira");
        tela.ir_para(d, window, cx);
    });
    e.esperar(cx);
    assert!(
        e.site.baixadas().contains(&"d".to_string()),
        "a cópia da d foi pedida"
    );
    e.app(cx, |app, _w, cx| {
        assert!(app.baixas_pendentes() > 0, "e está no ar");
        assert_eq!(app.sincronias_pendentes(), 0, "sem envio nenhum");
        let retrato = app.retrato_do_segundo_plano(cx);
        assert_eq!(retrato.subindo, 0);
        assert!(!retrato.ha_envio_pendente());
    });
    voltas(cx);
    assert_eq!(bandeja(cx), (false, false));

    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    assert!(
        visual.simulate_close(),
        "só download na fila: a janela fecha"
    );
}
