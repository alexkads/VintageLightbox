//! 📋 A lista de sessões, a sessão nova, a retenção e o caixa da rota.

use biblioteca_core::sessoes::Situacao;
use gpui::TestAppContext;

use super::{abrir_o_app, Cenario, GALERIA};
use crate::app::Tela;

/// 🎬 **A lista**: chega do site, a busca acha sem acento, o recorte por
/// situação filtra, "Limpar" devolve tudo, e recarregar pede de novo.
#[gpui::test]
fn a_lista_busca_recorta_e_recarrega(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);

    e.app(cx, |app, window, cx| {
        assert_eq!(app.tela(), Tela::Sessoes);
        app.sessoes.update(cx, |tela, cx| {
            assert_eq!(tela.quantas(), 2, "as duas sessões do site");
            assert_eq!(tela.id_na_posicao(1).as_deref(), Some(GALERIA));
            // 📅 **A lista abre em hoje** (dono, 18/set/2026). As deste cenário
            // são de outro dia, então o primeiro gesto é o que o operador faria
            // para ver o arquivo: "Tudo" no seletor de período.
            assert!(tela.filtrando(cx), "hoje é um recorte");
            tela.escolher_periodo_para_teste(None, cx);
            assert!(!tela.filtrando(cx));

            // "joao" acha "João".
            tela.buscar("joao", window, cx);
            assert!(tela.filtrando(cx));
            assert_eq!(tela.titulos_visiveis(cx), vec!["Casamento do João"]);

            // A busca e o recorte juntos: nenhuma das duas está "aberta pelo
            // cliente".
            tela.buscar("", window, cx);
            tela.filtrar_por(Some(Situacao::AbertaPeloCliente), cx);
            assert!(tela.filtrando(cx));
            assert!(tela.titulos_visiveis(cx).is_empty());

            // ⚠️ **"Limpar" devolve o padrão, e o padrão é hoje** — ver
            // `limpar_filtros`. Para o arquivo inteiro é o "Tudo" do seletor.
            tela.limpar_filtros(window, cx);
            tela.escolher_periodo_para_teste(None, cx);
            assert!(!tela.filtrando(cx));
            assert_eq!(tela.titulos_visiveis(cx).len(), 2);
        });
    });

    // `Esc` na lista não leva a lugar nenhum — nem à Biblioteca.
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessoes, "Esc fica na lista")
    });

    // Recarregar (o menu "Sessões fotográficas" também recarrega) vê a
    // sessão que alguém abriu pelo site nesse meio tempo.
    e.site
        .galerias
        .lock()
        .unwrap()
        .push(super::galeria_do_painel("g3", "Batizado do Pedro", None));
    e.app(cx, |app, window, cx| app.ir_para(Tela::Sessoes, window, cx));
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.sessoes.read(cx).quantas(), 3, "a lista foi relida");
    });
}

/// 🎬 **Sessão nova**: só o título é obrigatório, o e-mail pela metade é
/// recusado na tela, e criar **entra** na sessão criada.
#[gpui::test]
fn abrir_sessao_nova_e_entrar_nela(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);

    e.app(cx, |app, window, cx| {
        app.sessoes.update(cx, |tela, cx| {
            tela.comecar_nova(window, cx);
            assert!(tela.abrindo_nova());
            tela.escolher_estudio("e1".into(), cx);
            // O e-mail pela metade não sai daqui.
            tela.preencher_para_teste("Aniversário da Bia", "bia@", window, cx);
            tela.criar(cx);
            assert_eq!(
                tela.erro_para_teste().as_deref(),
                Some("o e-mail do cliente não parece completo")
            );
            // Sem contato, sai.
            tela.preencher_para_teste("Aniversário da Bia", "", window, cx);
            tela.criar(cx);
        });
    });
    e.esperar(cx);

    let criadas = e.site.criadas();
    assert_eq!(criadas.len(), 1, "uma sessão pedida ao site: {criadas:?}");
    assert_eq!(criadas[0].titulo, "Aniversário da Bia");
    assert_eq!(criadas[0].email, None, "o contato fica para o fim");
    assert_eq!(criadas[0].estudio_id.as_deref(), Some("e1"));
    assert_eq!(
        criadas[0].produto_id, "p1",
        "a faixa sugerida é a da última"
    );

    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::Sessao, "criar entra na sessão criada");
        assert!(!app.sessoes.read(cx).abrindo_nova());
        assert_eq!(app.sessao_aberta_para_teste(), Some("g1"));
    });
    // A criada vira "g1" no site de mentira, e a lista a mostra de volta.
    assert!(e.site.abertas().contains(&"g1".to_string()));
}

/// 🎬 **Retenção**: abre com a política do servidor, recusa o que não passa
/// na conferência do site, grava o que passa (`PUT`) e volta às sessões.
#[gpui::test]
fn a_retencao_le_recusa_e_grava(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);

    e.app(cx, |app, window, cx| {
        app.ir_para(Tela::Retencao, window, cx)
    });
    e.esperar(cx);
    let leituras: Vec<String> = e
        .site
        .pedidos_json()
        .into_iter()
        .filter(|p| p.rotulo == "retencao")
        .map(|p| format!("{} {}", p.metodo, p.caminho))
        .collect();
    assert_eq!(leituras, vec!["GET /pos-venda/configuracao"]);

    // Um prazo em branco não grava.
    e.app(cx, |app, window, cx| {
        assert_eq!(app.tela(), Tela::Retencao);
        app.retencao.update(cx, |tela, cx| {
            tela.digitar(["", "60", "7", "15"], window, cx);
            tela.salvar(window, cx);
            assert!(
                tela.erros()[0].is_some(),
                "o prazo vazio é recusado na tela"
            );
        });
    });
    assert!(
        e.site.pedidos_json().iter().all(|p| p.metodo == "GET"),
        "a recusa local não vai ao site"
    );

    e.app(cx, |app, window, cx| {
        app.retencao.update(cx, |tela, cx| {
            tela.digitar(["45", "60", "7", "15"], window, cx);
            tela.salvar(window, cx);
            assert_eq!(tela.erros(), [None; 4]);
        });
    });
    e.esperar(cx);
    let gravacoes: Vec<_> = e
        .site
        .pedidos_json()
        .into_iter()
        .filter(|p| p.metodo != "GET")
        .collect();
    assert_eq!(gravacoes.len(), 1, "{gravacoes:?}");
    assert_eq!(gravacoes[0].metodo, "PUT");
    assert_eq!(gravacoes[0].caminho, "/pos-venda/configuracao");
    let corpo = gravacoes[0].corpo.clone().expect("o corpo do PUT");
    assert_eq!(corpo["dias_a_venda"], 45);
    assert_eq!(
        corpo["apagar_automaticamente"], true,
        "o que veio do servidor fica"
    );

    e.teclar(cx, "escape");
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Retencao, "Esc fica na retenção")
    });

    // "← Sessões fotográficas".
    e.app(cx, |app, _w, cx| {
        app.retencao.update(cx, |_tela, cx| {
            cx.emit(crate::sessoes::retencao::PedidoDaRetencao::Voltar)
        });
    });
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessoes));
}

/// 🎬 **O caixa da rota**: o menu leva a ele, ele carrega o estúdio, os
/// funcionários e o catálogo, e escolher uma sessão lá abre a galeria dela —
/// cuja volta é para o caixa.
#[gpui::test]
fn o_caixa_da_rota_abre_a_galeria_e_a_volta_e_para_ele(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);

    e.app(cx, |app, window, cx| app.ir_para(Tela::Caixa, window, cx));
    e.esperar(cx);
    let rotulos: Vec<&str> = e.site.pedidos_json().iter().map(|p| p.rotulo).collect();
    for esperado in ["estudios", "galerias", "funcionarios", "catalogo"] {
        assert!(
            rotulos.contains(&esperado),
            "o caixa pediu {esperado}: {rotulos:?}"
        );
    }
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::Caixa, "Esc fica no caixa");
        assert!(!app.caixa.read(cx).flutuante(), "a rota é a tela inteira");
        assert_eq!(app.caixa.read(cx).estudio_do_caixa(), Some("e1"));
    });

    // "Abrir sessão" no caixa: a galeria abre, e o Voltar dela é para o caixa.
    e.app(cx, |app, _w, cx| {
        app.caixa.update(cx, |_tela, cx| {
            cx.emit(crate::caixa::tela::PedidoDoCaixa::AbrirSessao(
                GALERIA.into(),
            ))
        });
    });
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessao);
        assert!(app.pode_trabalhar());
    });
    e.detalhe(cx, |_tela, _w, cx| {
        cx.emit(crate::sessoes::detalhe::Pedido::Voltar)
    });
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::Caixa, "a volta é para de onde se veio");
        assert!(!app.pode_trabalhar(), "sair da galeria fecha a sessão");
        assert_eq!(app.caixa.read(cx).sessao_escolhida(), Some(GALERIA));
    });
}
