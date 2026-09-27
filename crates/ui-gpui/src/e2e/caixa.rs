//! 🧾 O caixa flutuante da galeria — por cima da grade e da revelação.

use gpui_kit::{Modifiers, TestAppContext, VisualTestContext};
use serde_json::json;

use super::{abrir_o_ensaio, Cenario, GALERIA};
use crate::app::Tela;
use crate::pos_venda::porta::PedidoJson;

fn gravacoes(e: &super::Estudio) -> Vec<PedidoJson> {
    e.site
        .pedidos_json()
        .into_iter()
        .filter(|p| p.metodo != "GET")
        .collect()
}

/// 🎬 **O caixa na galeria, do cupom à venda**: o painel segue a sessão
/// aberta, F9 recolhe e abre, o ajuste rápido dá cortesia a um item, F4 sem
/// os nomes pede as pessoas, e com eles abre o pagamento — que registra a
/// venda na API de mentira.
#[gpui_kit::test]
fn o_caixa_da_galeria_do_cupom_a_venda(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.esperar(cx);

    e.app(cx, |app, _w, cx| {
        let caixa = app.caixa_flutuante.read(cx);
        assert!(caixa.flutuante());
        assert!(caixa.visivel(), "na galeria o painel está à vista");
        assert_eq!(caixa.sessao_escolhida(), Some(GALERIA));
        assert!(caixa.caixa_do_estudio_aberto());
        let (itens, total) = caixa.cupom_para_teste();
        assert_eq!(itens, vec!["a", "b"], "as levadas no balcão");
        assert_eq!(total, 8000, "o preço de balcão das duas");
    });

    // F9 recolhe e abre — pela tecla de verdade.
    let minimizado = e.app(cx, |app, _w, cx| app.caixa_flutuante.read(cx).minimizado());
    e.teclar(cx, "f9");
    e.app(cx, |app, _w, cx| {
        assert_eq!(
            app.caixa_flutuante.read(cx).minimizado(),
            !minimizado,
            "F9 alterna"
        );
    });
    e.teclar(cx, "f9");
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.caixa_flutuante.read(cx).minimizado(), minimizado)
    });

    // O ajuste rápido: ↓ escolhe o item, E abre a barra, C dá cortesia.
    e.app(cx, |app, window, cx| {
        app.caixa_flutuante.update(cx, |caixa, cx| {
            assert!(caixa.tecla_no_cupom("down", false, window, cx));
            assert!(caixa.tecla_no_cupom("e", false, window, cx));
            assert!(caixa.tecla_no_cupom("c", false, window, cx));
        });
    });
    e.esperar(cx);
    let g = gravacoes(&e);
    assert_eq!(g.len(), 1, "{g:?}");
    assert_eq!(g[0].metodo, "PATCH");
    assert_eq!(g[0].caminho, "/pos-venda/fotos/a");
    assert_eq!(
        g[0].corpo,
        Some(json!({ "preco_negociado": "0.00", "observacao_da_negociacao": "Cortesia" }))
    );

    // F4 sem os nomes: as pessoas primeiro.
    e.teclar(cx, "f4");
    e.app(cx, |app, window, cx| {
        app.caixa_flutuante.update(cx, |caixa, cx| {
            assert_eq!(caixa.dialogo_do_caixa(), Some("Pessoas"));
            caixa.fechar_dialogo_do_caixa(window, cx);
            caixa.escolher_as_pessoas("f1");
        });
    });
    e.app(cx, |app, _w, _cx| {
        assert_eq!(
            app.tela(),
            Tela::Sessao,
            "fechar o diálogo não sai da galeria"
        );
    });

    // F4 com os nomes: o pagamento. Pix no valor todo, e Enter conclui.
    e.teclar(cx, "f4");
    e.app(cx, |app, window, cx| {
        app.caixa_flutuante.update(cx, |caixa, cx| {
            assert_eq!(caixa.dialogo_do_caixa(), Some("Pagamento"));
            let (_, total) = caixa.cupom_para_teste();
            let valor = format!("{},{:02}", total / 100, total % 100);
            caixa.lancar_pagamento(2, &valor, window, cx);
            assert_eq!(caixa.pagamentos_lancados(), 1);
            caixa.confirmar_dialogo(window, cx);
        });
    });
    e.esperar(cx);
    let vendas: Vec<PedidoJson> = gravacoes(&e)
        .into_iter()
        .filter(|p| p.rotulo == "venda")
        .collect();
    assert_eq!(vendas.len(), 1, "uma venda registrada");
    assert_eq!(vendas[0].caminho, "/pos-venda/caixa/vendas");
    let corpo = vendas[0].corpo.clone().expect("o corpo da venda");
    assert_eq!(corpo["galeria_id"], GALERIA);
    assert_eq!(corpo["pagamentos"][0]["forma"], "pix");
    assert_eq!(corpo["fotografo_id"], "f1");
    e.app(cx, |app, _w, cx| {
        let caixa = app.caixa_flutuante.read(cx);
        assert_eq!(caixa.dialogo_do_caixa(), None, "o pagamento fechou");
        assert_eq!(caixa.ultima_venda_para_teste(), Some(12));
    });
}

/// 🎬 **O caixa acompanha a revelação e some fora da sessão**: na revelação
/// as teclas F continuam valendo; na lista de sessões, o painel some e o F9
/// não faz nada.
#[gpui_kit::test]
fn o_caixa_na_revelacao_e_fora_da_sessao(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");

    e.app(cx, |app, _w, cx| {
        assert!(
            app.caixa_flutuante.read(cx).visivel(),
            "o painel está na revelação"
        );
    });
    e.teclar(cx, "f7");
    e.app(cx, |app, window, cx| {
        app.caixa_flutuante.update(cx, |caixa, cx| {
            assert_eq!(caixa.dialogo_do_caixa(), Some("Vendas"), "F7 na revelação");
            caixa.fechar_dialogo_do_caixa(window, cx);
        });
        assert_eq!(app.tela(), Tela::Revelacao);
    });

    // F1 (atalhos) também; a revelação não sai do lugar.
    e.teclar(cx, "f1");
    e.app(cx, |app, window, cx| {
        app.caixa_flutuante.update(cx, |caixa, cx| {
            assert_eq!(caixa.dialogo_do_caixa(), Some("Atalhos"));
            caixa.fechar_dialogo_do_caixa(window, cx);
        });
    });

    // Fora da sessão, o painel some e não pega tecla.
    e.teclar(cx, "escape");
    e.detalhe(cx, |_tela, _w, cx| {
        cx.emit(crate::sessoes::detalhe::Pedido::Voltar)
    });
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessoes));
    let minimizado = e.app(cx, |app, _w, cx| {
        assert!(
            !app.caixa_flutuante.read(cx).visivel(),
            "fora da sessão não há painel"
        );
        app.caixa_flutuante.read(cx).minimizado()
    });
    e.teclar(cx, "f9");
    e.app(cx, |app, _w, cx| {
        assert_eq!(
            app.caixa_flutuante.read(cx).minimizado(),
            minimizado,
            "F9 não chega"
        );
    });
}

/// 🎬 **O clique na tira tira o foco do cupom**, e as setas voltam a trocar a
/// foto.
///
/// Com o foco no cupom, ele engole as setas e as teclas de um caractere — é o
/// que impede a nota de cair na grade de trás. Mas a tira não é focável, e no
/// GPUI clicar nela não tirava o foco de ninguém: depois de um clique no
/// caixa, as setas, a nota e o `P` da Revelação morriam até a guia ser fechada
/// e reaberta (dono, 27/set/2026).
#[gpui_kit::test]
fn o_clique_fora_do_cupom_devolve_as_setas_a_revelacao(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    let clicar = |cx: &mut TestAppContext, ponto| {
        let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
        visual.simulate_click(ponto, Modifiers::none());
        visual.run_until_parked();
        e.esperar(cx);
    };
    let no_cupom = |cx: &mut TestAppContext| {
        e.app(cx, |_app, window, _cx| {
            window
                .context_stack()
                .iter()
                .any(|c| c.contains("CaixaCupom"))
        })
    };

    let centro = e
        .app(cx, |app, window, cx| {
            app.caixa_flutuante
                .read(cx)
                .centro_do_painel(window.viewport_size())
        })
        .expect("o painel está desenhado");
    clicar(cx, centro);
    assert!(no_cupom(cx), "o clique no cupom leva as teclas para ele");

    let (ponto, alvo) = e.revelacao(cx, |tela, _w, _cx| {
        (tela.centro_da_miniatura(2), tela.na_tira()[2])
    });
    clicar(cx, ponto.expect("a tira está desenhada"));
    assert!(!no_cupom(cx), "o clique na tira tira o foco do cupom");
    e.revelacao(cx, |tela, _w, _cx| assert_eq!(tela.posicao(), alvo));

    e.teclar(cx, "right");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.posicao(),
            tela.na_tira()[3],
            "depois do clique na tira, a seta troca a foto"
        );
    });

    // De volta ao cupom, as setas são dele de novo — e não da tira.
    clicar(cx, centro);
    assert!(no_cupom(cx));
}
