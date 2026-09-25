//! 🚪 A porta, a conta e a moldura do painel.

use gpui::TestAppContext;
use gpui_component::ActiveTheme;

use super::{abrir_o_app, sessao, Cenario, GALERIA};
use crate::app::Tela;
use crate::tema::Escolha;

/// 🎬 **Da porta ao Sair**: entrar, ver quem está logado, recolher o menu
/// lateral pelo `⌘B`, trocar o tema pelo menu da conta e sair.
///
/// 🔑 O `⌘B` é apertado de verdade — uma ligação que não casa não falha, só
/// não faz nada. E o tema é conferido no `Theme` global, que é o que a tela
/// pinta, e não só no campo da escolha.
#[gpui::test]
fn da_porta_ao_sair_pela_conta(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());

    // 🚪 A porta vem antes de tudo: nada trabalha, e as teclas da grade não
    // chegam por trás da tela de login.
    e.app(cx, |app, _w, _cx| {
        assert!(!app.entrou());
        assert!(!app.pode_trabalhar());
    });
    e.teclar(cx, "cmd-b");
    e.app(cx, |app, _w, _cx| {
        assert!(!app.menu_lateral_aberto(), "a porta não tem menu");
    });

    // Entra: a lista de sessões é a primeira tela, e a conta vem de /auth/me.
    e.entrar_na_conta(cx);
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessoes);
        assert_eq!(app.sessao(), Some(&sessao()));
        let conta = app.conta().expect("o /auth/me respondeu");
        assert_eq!(conta.email, "alex@exemplo.com");
        assert_eq!(conta.exibido(), "Alex");
        assert_eq!(conta.inicial(), "A");
    });
    let pedidos: Vec<(&str, String)> = e
        .site
        .pedidos_json()
        .into_iter()
        .map(|p| (p.metodo, p.caminho))
        .collect();
    assert!(
        pedidos.contains(&("GET", "/auth/me".to_string())),
        "a conta é lida do site: {pedidos:?}"
    );
    assert!(
        e.site.pedidos_json().iter().all(|p| p.metodo == "GET"),
        "entrar não grava nada"
    );

    // ⌘B abre e fecha o menu lateral, nas duas plataformas.
    e.teclar(cx, "cmd-b");
    e.app(cx, |app, _w, _cx| {
        assert!(app.menu_lateral_aberto(), "⌘B abre")
    });
    e.teclar(cx, "ctrl-b");
    e.app(cx, |app, _w, _cx| {
        assert!(!app.menu_lateral_aberto(), "Ctrl+B fecha")
    });

    // O menu da conta: tema Escuro, Claro e Sistema — cada um chega ao tema
    // que a tela pinta.
    e.app(cx, |app, _w, cx| {
        app.alternar_menu_da_conta(cx);
        assert!(app.menu_da_conta_aberto());
    });
    for (escolha, escuro) in [
        (Escolha::Escuro, Some(true)),
        (Escolha::Claro, Some(false)),
        (Escolha::Sistema, None),
    ] {
        e.app(cx, |app, window, cx| {
            app.escolher_tema(escolha, window, cx);
            assert_eq!(app.escolha_de_tema(), escolha);
            if let Some(escuro) = escuro {
                assert_eq!(
                    cx.theme().mode.is_dark(),
                    escuro,
                    "o tema {escolha:?} chegou ao que a tela pinta"
                );
            }
        });
    }

    // Trocar de tela pelo menu fecha o menu da conta.
    e.app(cx, |app, window, cx| {
        app.ir_para(Tela::Caixa, window, cx);
        assert!(!app.menu_da_conta_aberto());
        assert_eq!(app.tela(), Tela::Caixa);
        app.ir_para(Tela::Sessoes, window, cx);
    });

    // Entra num ensaio e sai da conta de dentro dele: a sessão fecha junto.
    e.app(cx, |app, _w, cx| {
        app.entrar_na_sessao(GALERIA.into(), cx);
        assert!(app.pode_trabalhar());
        app.alternar_menu_da_conta(cx);
        app.sair_da_conta(cx);
    });
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        assert!(!app.entrou(), "Sair volta para a porta");
        assert!(!app.pode_trabalhar(), "e o ensaio fechou junto");
        assert!(app.conta().is_none());
        assert!(!app.menu_da_conta_aberto());
    });
    assert_eq!(
        *e.site.sessao_guardada.lock().unwrap(),
        None,
        "o chaveiro foi esquecido"
    );
}

/// 📦 **O Backup mora no menu da conta**, e não no lateral (dono, 2026-09-25:
/// *"não é tão utilizado no dia a dia"*). O menu lateral fica com o que o
/// balcão usa o dia inteiro; o Backup se abre pelo retrato da conta, com um
/// clique de verdade.
#[gpui::test]
fn o_backup_se_abre_pelo_menu_da_conta(cx: &mut TestAppContext) {
    use super::chatbot::{clicar, desenhado};

    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    e.teclar(cx, "cmd-b");

    assert!(
        desenhado(&e, cx, "menu-Chatbot"),
        "o menu lateral está na tela"
    );
    assert!(
        !desenhado(&e, cx, "menu-Backup de arquivos"),
        "o Backup saiu do menu lateral"
    );

    e.app(cx, |app, _w, cx| app.alternar_menu_da_conta(cx));
    clicar(&e, cx, "conta-backup");
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Backup);
        assert!(!app.menu_da_conta_aberto(), "abrir o Backup fecha o menu");
    });
}
