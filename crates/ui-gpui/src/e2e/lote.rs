//! 📦 O que a Revelação faz com mais de uma foto, e os dois botões do fim:
//! "Baixar JPEG" e "Salvar na galeria e sair".

use gpui::TestAppContext;

use super::{abrir_o_ensaio, Cenario};
use crate::app::Tela;
use crate::revelacao::tela::PedidoDaRevelacao;

/// O botão da barra da Revelação que emite `pedido`.
fn botao(e: &super::Estudio, cx: &mut TestAppContext, pedido: PedidoDaRevelacao) {
    e.revelacao(cx, move |_tela, _w, cx| cx.emit(pedido));
}

/// 🎬 **Sincronizar N e zerar N**: `⌘A` marca a tira, a caixa do
/// "Sincronizar" confirma pelo Enter, a receita vai para as marcadas (menos a
/// comprada) sem subir nada, e o "Zerar N fotos" devolve todas ao neutro —
/// inclusive o enquadramento.
#[gpui::test]
fn sincronizar_e_zerar_as_marcadas(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 1.0, cx));
    e.esperar(cx);

    e.teclar(cx, "cmd-a");
    let alvos = e.revelacao(cx, |tela, _w, _cx| {
        let mut ids: Vec<String> = tela
            .alvos_da_sincronizacao()
            .into_iter()
            .map(|f| f.id)
            .collect();
        ids.sort();
        ids
    });
    assert_eq!(
        alvos,
        vec![
            "id-DSC_101.jpg",
            "id-DSC_102.jpg",
            "site:a",
            "site:b",
            "site:d"
        ],
        "a tira inteira, menos a comprada"
    );

    // A caixa das flags, confirmada pelo Enter.
    e.revelacao(cx, |tela, window, cx| tela.abrir_sincronizacao(window, cx));
    e.teclar(cx, "enter");
    e.esperar(cx);
    let sincronizadas: Vec<String> = e
        .gravador
        .gravado()
        .into_iter()
        .filter(|(_, ajustes, _)| ajustes.exposure == 1.0)
        .map(|(id, _, _)| id)
        .collect();
    for id in ["site:b", "site:d", "id-DSC_101.jpg", "id-DSC_102.jpg"] {
        assert!(
            sincronizadas.contains(&id.to_string()),
            "{id} recebeu a receita: {sincronizadas:?}"
        );
    }
    assert!(
        !e.gravador.gravado().iter().any(|(id, _, _)| id == "site:c"),
        "a comprada fica de fora"
    );
    assert!(e.site.reveladas().is_empty(), "sincronizar não sobe nada");
    e.app(cx, |app, _w, cx| {
        let mut fila = app.a_subir_para_teste();
        fila.sort();
        assert_eq!(fila, vec!["b", "d"], "as do site esperam o Salvar");
        assert!(app.revelacao.read(cx).ha_o_que_salvar());
    });

    // A seta leva a foto sincronizada com a receita nova.
    e.revelacao(cx, |tela, window, cx| {
        let b = tela
            .acervo()
            .iter()
            .position(|f| f.id == "site:b")
            .expect("b na tira");
        tela.ir_para(b, window, cx);
    });
    e.revelacao(cx, |tela, _w, _cx| assert_eq!(tela.ajustes().exposure, 1.0));

    // Um enquadramento em b, para o "Zerar" provar que o leva junto.
    e.teclar(cx, "r ] enter");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.enquadramento().rotation_90(), 1)
    });

    // "Zerar N fotos": esta pelo histórico, as outras pela raiz.
    e.teclar(cx, "cmd-a");
    e.revelacao(cx, |tela, window, cx| {
        assert!(tela.outras_a_zerar().len() >= 3, "as marcadas com receita");
        tela.clicar_em_zerar_tudo(window, cx);
    });
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.ajustes().exposure, 0.0);
        assert_eq!(
            tela.enquadramento().rotation_90(),
            0,
            "o enquadramento zerou junto"
        );
        assert!(tela.pode_desfazer(), "esta volta pelo ⌘Z");
        assert!(
            tela.outras_a_zerar().is_empty(),
            "e as outras também zeraram"
        );
    });
    let a = e
        .gravador
        .gravado()
        .into_iter()
        .rev()
        .find(|(id, _, _)| id == "site:a")
        .expect("a foi zerada");
    assert_eq!(a.1.exposure, 0.0);
    assert_eq!(a.2.largura, Some(1.0), "o corte inteiro vai escrito");
}

/// 🎬 **A comprada não se revela**: abre, mostra, e nada dela vai ao banco
/// nem ao site — nem pelo Enquadrar do teclado, nem pelo Salvar.
#[gpui::test]
fn a_foto_comprada_fica_travada(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "c");
    e.revelacao(cx, |tela, window, cx| {
        assert!(!tela.pode_revelar());
        assert!(!tela.ha_o_que_salvar());
        tela.redefinir_ajustes(window, cx);
        assert!(!tela.pode_desfazer(), "o Zerar não age na comprada");
    });
    e.teclar(cx, "r ] enter");
    e.esperar(cx);
    assert!(
        !e.gravador.gravado().iter().any(|(id, _, _)| id == "site:c"),
        "nada da comprada vai ao banco"
    );

    botao(&e, cx, PedidoDaRevelacao::SalvarNaGaleria);
    e.esperar(cx);
    assert!(e.site.reveladas().is_empty(), "nem ao site");
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessao, "sem o que salvar, o botão só sai");
    });
}

/// 🎬 **"Baixar JPEG"**: a foto do site é revelada em resolução cheia pelo
/// site com o que está na tela, e o arquivo cai na pasta de downloads — a de
/// teste. A foto que só está no disco sai pela exportação local.
#[gpui::test]
fn baixar_jpeg_da_foto_do_site_e_da_local(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.6, cx));
    e.teclar(cx, "shift-h");

    botao(&e, cx, PedidoDaRevelacao::Exportar);
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(tela.gerando_jpeg(), "o botão diz que está gerando")
    });
    e.esperar(cx);

    let integrais = e.site.integrais();
    assert_eq!(integrais.len(), 1);
    assert_eq!(integrais[0].0, "a");
    assert_eq!(
        integrais[0].1.exposure, 0.6,
        "com o que está na tela, salvo ou não"
    );
    assert!(integrais[0].2.flip_horizontal(), "e o enquadramento junto");
    assert!(e.site.reveladas().is_empty(), "baixar não sobe nada");

    let pasta = e.app(cx, |app, _w, _cx| app.pasta_dos_downloads_para_teste());
    let arquivo = pasta.join("a-revelada.jpg");
    assert!(
        arquivo.exists(),
        "o JPEG foi gravado em {}",
        pasta.display()
    );
    let na_pasta_do_usuario = directories::UserDirs::new()
        .and_then(|d| d.download_dir().map(std::path::Path::to_path_buf));
    assert_ne!(
        Some(pasta.clone()),
        na_pasta_do_usuario,
        "nunca na pasta de quem roda"
    );
    e.revelacao(cx, |tela, _w, _cx| assert!(!tela.gerando_jpeg()));
    let _ = std::fs::remove_dir_all(&pasta);

    // A foto local: a exportação abre com ela.
    e.revelacao(cx, |tela, window, cx| {
        let local = tela
            .acervo()
            .iter()
            .position(|f| f.id == "id-DSC_101.jpg")
            .expect("a local na tira");
        tela.ir_para(local, window, cx);
    });
    botao(&e, cx, PedidoDaRevelacao::Exportar);
    e.app(cx, |app, _w, cx| {
        assert!(app.exportando(), "a local sai pela exportação");
        assert_eq!(app.exportacao_para_teste().read(cx).quantas(), 1);
    });
    assert_eq!(e.site.integrais().len(), 1, "sem ida ao site para a local");
}

/// 🎬 **"Salvar na galeria e sair"**: sobe a aberta e as que o "Sincronizar"
/// deixou na fila, espera o lote inteiro, esvazia o depósito e volta para a
/// sessão.
#[gpui::test]
fn salvar_na_galeria_sobe_o_lote_e_sai(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            site: Box::new(|site| site.demorada = true),
            ..Cenario::default()
        },
    );
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.4, cx));
    e.esperar(cx);
    // b entra na fila pelo Sincronizar (a e b marcadas).
    e.revelacao(cx, |tela, window, cx| {
        let b = tela
            .acervo()
            .iter()
            .position(|f| f.id == "site:b")
            .expect("b na tira");
        tela.clicar_na_tira(
            b,
            biblioteca_core::selecao::Modificadores {
                aditivo: true,
                faixa: false,
            },
            window,
            cx,
        );
    });
    botao(&e, cx, PedidoDaRevelacao::Sincronizar);
    e.esperar(cx);
    assert!(!e.gravador.deposito().is_empty(), "a e b no depósito");

    botao(&e, cx, PedidoDaRevelacao::SalvarNaGaleria);
    e.esperar(cx);
    let mut reveladas: Vec<String> = e.site.reveladas().into_iter().map(|r| r.0).collect();
    reveladas.sort();
    assert_eq!(reveladas, vec!["a", "b"], "a aberta e a da fila");
    e.app(cx, |app, _w, _cx| {
        assert_eq!(
            app.tela(),
            Tela::Revelacao,
            "fica enquanto o site não responde"
        );
    });

    // O site responde uma de cada vez: a tela só sai na última.
    e.site.responder_uma();
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Revelacao, "falta uma")
    });
    e.site.responder();
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        assert_eq!(
            app.tela(),
            Tela::Sessao,
            "o lote terminou: volta para a sessão"
        );
        assert!(app.a_subir_para_teste().is_empty(), "a fila esvaziou");
        assert!(app.recusas_para_teste().is_empty());
    });
    assert!(
        e.gravador.deposito().is_empty(),
        "o depósito esvaziou: a verdade é o site"
    );
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.erro().map(|f| f.to_string()).as_deref(),
            Some("revelação salva na galeria")
        );
    });
}

/// 🎬 **O site recusa o Salvar**: a tela fica, a recusa vai para o canto, e
/// a receita continua no depósito para tentar de novo.
#[gpui::test]
fn salvar_na_galeria_com_falha_fica_na_tela(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            site: Box::new(|site| {
                site.salvar_falha = Some("o site respondeu 410: foto apagada".into())
            }),
            ..Cenario::default()
        },
    );
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.4, cx));
    e.esperar(cx);

    botao(&e, cx, PedidoDaRevelacao::SalvarNaGaleria);
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::Revelacao, "com falha, a tela fica");
        assert_eq!(
            app.recusas_para_teste(),
            ["o site respondeu 410: foto apagada"]
        );
        assert!(
            app.revelacao.read(cx).ha_o_que_salvar(),
            "e ainda há o que salvar"
        );
    });
    assert!(
        e.gravador.deposito().iter().any(|(id, _)| id == "a"),
        "a receita continua guardada"
    );
}
