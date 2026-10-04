//! 💾 "Recuperar cartão", do menu lateral até a sessão do cliente.

use std::sync::Arc;

use gpui_kit::TestAppContext;

use super::chatbot::{clicar, desenhado, toasts};
use super::{abrir_o_app, Cenario, Estudio, GALERIA};
use crate::app::Tela;
use crate::importacao::explorador::mentira::SeletorDeMentira;
use crate::recuperacao::estado::Recado;
use crate::recuperacao::porta::mentira::RecuperadorDeMentira;
use crate::recuperacao::PastaRecuperada;
use domain::recuperacao::CartaoBruto;

fn cartao() -> CartaoBruto {
    CartaoBruto {
        dispositivo: "/dev/disk4".into(),
        nome: "EOS_DIGITAL".into(),
        tamanho: 64_000_000_000,
        montagens: vec!["/Volumes/EOS_DIGITAL".into()],
    }
}

fn entrar_com_a_recuperacao(cx: &mut TestAppContext) -> (Estudio, Arc<RecuperadorDeMentira>) {
    let e = abrir_o_app(cx, Cenario::default());
    let porta = Arc::new(RecuperadorDeMentira::default());
    *porta.lista.lock().unwrap() = vec![cartao()];
    *porta.respostas.lock().unwrap() = vec![Recado::Terminou {
        achadas: 12,
        interrompida: false,
        ilegiveis: 0,
    }];
    e.app(cx, |app, _w, cx| {
        app.ligar_recuperacao(
            porta.clone(),
            Arc::new(SeletorDeMentira::escolhe("/Users/estudio/Recuperadas")),
            cx,
        )
    });
    e.entrar_na_conta(cx);
    e.esperar(cx);
    (e, porta)
}

/// 🔑 **É item do menu lateral, e não botão escondido na importação** (dono,
/// 29/set/2026). Do clique no menu ao "Importar 12 fotos", e de lá à sessão:
/// o "Do cartão ou pasta…" oferece a pasta recuperada em primeiro lugar.
#[gpui_kit::test]
fn o_menu_lateral_leva_do_cartao_formatado_a_sessao_do_cliente(cx: &mut TestAppContext) {
    let (e, porta) = entrar_com_a_recuperacao(cx);

    clicar(&e, cx, "menu-Recuperar cartão");
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Recuperacao));
    assert!(
        desenhado(&e, cx, "cartao-0"),
        "o cartão plugado aparece na lista"
    );

    clicar(&e, cx, "recuperacao-destino");
    e.esperar(cx);
    clicar(&e, cx, "recuperacao-comecar");
    e.esperar(cx);
    assert_eq!(
        *porta.comecados.lock().unwrap(),
        [(
            "/dev/disk4".to_string(),
            "/Users/estudio/Recuperadas".to_string()
        )]
    );

    assert!(
        desenhado(&e, cx, "recuperacao-andamento-bloco"),
        "a barra de andamento fica na tela"
    );

    clicar(&e, cx, "recuperacao-importar");
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::Sessoes, "tudo entra numa sessão");
        assert_eq!(
            cx.try_global::<PastaRecuperada>(),
            Some(&PastaRecuperada {
                caminho: "/Users/estudio/Recuperadas".into(),
                fotos: 12
            })
        );
    });
    assert!(
        toasts(&e, cx)
            .iter()
            .any(|(t, erro)| !erro && t.contains("Do cartão ou pasta")),
        "o toast diz onde as fotos estão: {:?}",
        toasts(&e, cx)
    );

    e.app(cx, |app, _w, cx| {
        app.sessoes
            .update(cx, |tela, cx| tela.abrir(GALERIA.into(), cx));
    });
    e.esperar(cx);
    clicar(&e, cx, "detalhe-importar");
    clicar(&e, cx, "origem-botao");
    e.esperar(cx);
    // Sem `esperar`: a varredura de mentira volta vazia, e a janela fecha.
    clicar(&e, cx, "origem-recuperadas");
    e.detalhe(cx, |tela, _w, cx| {
        let origem = tela.origem_para_teste().expect("a sessão tem a origem");
        assert_eq!(
            origem.read(cx).raiz_da_selecao(),
            Some("/Users/estudio/Recuperadas"),
            "a janela de escolher abre na pasta recuperada"
        );
    });
}

/// Sem a montagem ligar a recuperação, o item não aparece — o que não está
/// pronto no app não aparece nele.
#[gpui_kit::test]
fn sem_recuperacao_ligada_o_menu_nao_tem_o_item(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    e.esperar(cx);
    assert!(desenhado(&e, cx, "menu-Caixa"));
    assert!(!desenhado(&e, cx, "menu-Recuperar cartão"));
}
