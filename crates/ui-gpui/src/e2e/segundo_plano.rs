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

    // 🔄 Sem envio na fila, fechar **também** vai para a bandeja (dono,
    // 2026-09-21): o app não termina ao fechar a janela.
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    assert!(
        !visual.simulate_close(),
        "a janela não fecha: vai para a bandeja"
    );
    voltas(cx);
    assert_eq!(bandeja(cx), (true, false), "na bandeja, e o app segue");
}

/// 🎬 **Fechar com envio na fila leva à bandeja, e o envio termina lá** —
/// sem aviso, e sem o app sair quando a fila esvazia (dono, 2026-09-21:
/// *"quero que o sistema fique na bandeja ao fechar, assim podemos continuar
/// com os processos em segundo plano"*). Fecha junto a tela do cliente.
#[gpui::test]
fn fechar_com_envio_vai_para_a_bandeja_e_o_envio_termina_la(cx: &mut TestAppContext) {
    let e = com_envio_na_fila(cx);
    e.app(cx, |app, _w, cx| {
        app.alternar_cliente(cx);
        assert!(app.cliente_aberto());
    });

    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    assert!(!visual.simulate_close(), "a janela não fecha");
    voltas(cx);
    assert_eq!(bandeja(cx), (true, false), "foi para a bandeja, e não saiu");
    assert!(plataforma::gestos().contains(&"esconder"));
    e.app(cx, |app, _w, _cx| {
        assert!(
            !app.cliente_aberto(),
            "a tela do cliente não fica sozinha no monitor"
        );
    });

    // O site responde: a fila esvazia, e o app continua na bandeja.
    e.site.responder();
    voltas(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.sincronias_pendentes(), 0));
    voltas(cx);
    assert_eq!(
        bandeja(cx),
        (true, false),
        "a fila esvaziou e o app segue na bandeja"
    );
    assert_eq!(e.site.reveladas().len(), 1, "e o envio chegou");
}

/// 🎬 **O "Fechar" da barra do app faz o que o fechar do sistema faz.** No
/// GNOME ele é o único que existe, e chamava `remove_window` direto — que não
/// passa pelo `on_window_should_close`: o app encerrava em vez de ir para a
/// bandeja (dono, 24/set/2026).
#[gpui::test]
fn o_fechar_da_barra_do_app_leva_a_bandeja_e_nao_encerra(cx: &mut TestAppContext) {
    let e = com_envio_na_fila(cx);
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.update(crate::segundo_plano::fechar_pelo_botao);
    voltas(cx);
    assert_eq!(bandeja(cx), (true, false), "foi para a bandeja, e não saiu");
    assert!(plataforma::gestos().contains(&"esconder"));
    assert!(
        e.raiz.update(cx, |_, _, _| ()).is_ok(),
        "a janela principal continua existindo"
    );
}

/// 🎬 **Abrir de novo traz a janela da bandeja**, com o envio ainda no ar.
#[gpui::test]
fn reabrir_traz_a_janela_da_bandeja(cx: &mut TestAppContext) {
    let e = com_envio_na_fila(cx);
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    assert!(!visual.simulate_close());
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
        "com a janela à vista, o app segue"
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

    // Fechar vai para a bandeja em qualquer caso (dono, 2026-09-21) — o que
    // este cenário prova é o retrato acima: download não conta como envio.
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    assert!(!visual.simulate_close(), "a janela vai para a bandeja");
    voltas(cx);
    assert_eq!(bandeja(cx), (true, false));
}

/// Clica no botão marcado com `debug_selector`, ou afirma que ele existe.
fn aviso_na_tela(e: &Estudio, cx: &mut TestAppContext) -> bool {
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    visual.debug_bounds("aviso-de-saida").is_some()
}

/// 🎬 **Sem bandeja e sem envio, fechar fecha** (dono, 25/set/2026: *"eu não
/// consigo fechar o sistema, pois o mesmo não vai pra bandeja no linux"*). No
/// GNOME sem a extensão AppIndicator o ícone não aparece: ir para a bandeja
/// era sumir sem o "Sair".
#[gpui::test]
fn sem_bandeja_no_sistema_e_sem_envio_fechar_encerra(cx: &mut TestAppContext) {
    crate::bandeja::teste::fingir_sem_bandeja();
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            segundo_plano: true,
            ..Cenario::default()
        },
    );
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    assert!(visual.simulate_close(), "a janela fecha — e fechar a principal encerra o app");
    assert!(!plataforma::gestos().contains(&"esconder"), "não foi para uma bandeja que não existe");
}

/// 🎬 **Sem bandeja e com envio no ar, fechar pergunta** — e "Esperar terminar
/// e sair" sai sozinho quando a fila esvazia (dono, 25/set/2026: *"perguntar
/// só se houver envio"*).
#[gpui::test]
fn sem_bandeja_com_envio_fechar_pergunta_e_espera_a_fila_para_sair(cx: &mut TestAppContext) {
    use crate::app::Saida;

    crate::bandeja::teste::fingir_sem_bandeja();
    let e = com_envio_na_fila(cx);
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.update(crate::segundo_plano::fechar_pelo_botao);
    voltas(cx);
    assert!(e.raiz.update(cx, |_, _, _| ()).is_ok(), "a janela não fechou");
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.saida_para_teste(), Some(Saida::Perguntando));
    });
    assert!(aviso_na_tela(&e, cx), "o aviso está desenhado");
    assert_eq!(bandeja(cx), (false, false), "nem bandeja, nem saída ainda");

    e.app(cx, |app, _w, cx| app.esperar_a_fila_e_sair(cx));
    voltas(cx);
    assert_eq!(bandeja(cx), (false, false), "com a fila no ar, espera");

    e.site.responder();
    voltas(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.sincronias_pendentes(), 0));
    voltas(cx);
    assert_eq!(bandeja(cx), (false, true), "a fila esvaziou e o app saiu");
    assert_eq!(e.site.reveladas().len(), 1, "e o envio chegou antes");
}

/// 🎬 **"Minimizar e continuar" não sai** — e o envio termina com a janela
/// minimizada.
#[gpui::test]
fn sem_bandeja_minimizar_e_continuar_nao_sai(cx: &mut TestAppContext) {
    crate::bandeja::teste::fingir_sem_bandeja();
    let e = com_envio_na_fila(cx);
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    assert!(!visual.simulate_close(), "com envio no ar, não fecha calado");
    // O botão chama `desistir_de_sair` e minimiza; a janela de teste do GPUI
    // não sabe minimizar (`unimplemented!`), então o cenário faz só a parte
    // do app.
    e.app(cx, |app, _w, cx| {
        app.desistir_de_sair(cx);
        assert_eq!(app.saida_para_teste(), None, "o aviso sai da frente");
    });
    e.site.responder();
    voltas(cx);
    voltas(cx);
    assert_eq!(bandeja(cx), (false, false), "o app continua, sem sair");
    assert_eq!(e.site.reveladas().len(), 1);
}
