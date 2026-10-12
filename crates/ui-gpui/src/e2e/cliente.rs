//! 🖥️ A segunda tela, virada para o cliente.

use gpui_kit::TestAppContext;

use super::{abrir_o_ensaio, Cenario};
use crate::app::Tela;
use crate::sessoes::detalhe::Pedido;

/// A foto e a exposição que a tela do cliente recebeu por último.
fn no_cliente(e: &super::Estudio, cx: &mut TestAppContext) -> Option<(String, f32)> {
    e.app(cx, |app, _w, _cx| {
        app.parametros_no_cliente()
            .map(|(id, ajustes)| (id, ajustes.exposure))
    })
}

/// 🎬 **A tela do cliente acompanha o operador**: abre pelo botão da galeria
/// na foto em foco, segue o foco, entra na revelação com ele, vê cada gesto,
/// a prévia da predefinição e o "Antes" segurado — e fecha pelo mesmo botão.
#[gpui_kit::test]
fn a_tela_do_cliente_acompanha_a_galeria_e_a_revelacao(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    e.detalhe(cx, |tela, _w, cx| {
        tela.focar_foto("a", cx);
        cx.emit(Pedido::TelaDoCliente);
    });
    e.app(cx, |app, _w, _cx| {
        assert!(app.cliente_aberto(), "o botão da galeria abre")
    });
    assert_eq!(no_cliente(&e, cx), Some(("site:a".into(), 0.0)));

    // O foco anda na grade: o cliente acompanha.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("b", cx));
    assert_eq!(no_cliente(&e, cx).map(|c| c.0).as_deref(), Some("site:b"));

    // Na revelação, a foto aberta e cada gesto.
    e.revelar_a_do_site(cx, "a");
    assert_eq!(no_cliente(&e, cx).map(|c| c.0).as_deref(), Some("site:a"));
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 1.2, cx));
    assert_eq!(
        no_cliente(&e, cx),
        Some(("site:a".into(), 1.2)),
        "a edição ao vivo"
    );

    // A prévia da predefinição também chega ao cliente.
    e.revelacao(cx, |tela, _w, cx| {
        let escuro = tela.predefinicao("Escuro").expect("a do sistema");
        tela.prever(Some(&escuro), cx);
    });
    assert_eq!(no_cliente(&e, cx), Some(("site:a".into(), -1.0)));
    e.revelacao(cx, |tela, _w, cx| tela.prever(None, cx));
    assert_eq!(no_cliente(&e, cx), Some(("site:a".into(), 1.2)));

    // O "Antes": segurar o `\` mostra a foto sem ajuste, soltar devolve.
    e.teclar(cx, "\\");
    e.revelacao(cx, |tela, _w, _cx| assert!(tela.mostrando_original()));
    assert_eq!(
        no_cliente(&e, cx),
        Some(("site:a".into(), 0.0)),
        "o cliente vê o antes"
    );
    e.soltar(cx, "\\");
    e.revelacao(cx, |tela, _w, _cx| assert!(!tela.mostrando_original()));
    assert_eq!(no_cliente(&e, cx), Some(("site:a".into(), 1.2)));

    // Sair da revelação: a galeria volta a mandar — e a foto que acabou de
    // ser revelada continua revelada do lado do cliente.
    e.esperar(cx);
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessao));
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("a", cx));
    assert_eq!(
        no_cliente(&e, cx),
        Some(("site:a".into(), 1.2)),
        "de volta à galeria, o cliente não pode ver a foto de antes da revelação"
    );

    // O mesmo botão fecha.
    e.detalhe(cx, |_tela, _w, cx| cx.emit(Pedido::TelaDoCliente));
    e.app(cx, |app, _w, _cx| {
        assert!(!app.cliente_aberto(), "e o mesmo botão fecha")
    });
}

/// Os números, o foco, as fotos por linha e as escolhidas da grade que a tela
/// do cliente recebeu por último.
#[allow(clippy::type_complexity)]
fn grade_no_cliente(
    e: &super::Estudio,
    cx: &mut TestAppContext,
) -> Option<(Vec<usize>, Option<usize>, usize, Vec<bool>)> {
    e.app(cx, |app, _w, _cx| {
        app.grade_no_cliente().map(|g| {
            (
                g.itens.iter().map(|i| i.numero).collect(),
                g.foco,
                g.por_linha,
                g.itens.iter().map(|i| i.escolhida).collect(),
            )
        })
    })
}

/// O recorte do operador agora: os ids na ordem da grade dele e quais estão
/// levadas no balcão.
fn recorte(e: &super::Estudio, cx: &mut TestAppContext) -> (Vec<String>, Vec<bool>) {
    e.detalhe(cx, |tela, _w, _cx| {
        let ids = tela.ids_visiveis();
        let levadas = ids
            .iter()
            .map(|id| {
                tela.como_esta(id).map(|(estado, ..)| estado)
                    == Some(biblioteca_core::Estado::LevadaNoBalcao)
            })
            .collect();
        (ids, levadas)
    })
}

/// 🎬 **A tela do cliente em grade**: o operador escolhe o modo e as fotos
/// por linha no menu da galeria; cada foto leva o número da posição dela na
/// sessão — o mesmo dos dois lados do balcão, com ou sem filtro —, o foco acompanha, a
/// revelação volta à foto única e a galeria devolve a grade.
#[gpui_kit::test]
fn a_tela_do_cliente_em_grade_numera_as_fotos_do_recorte(cx: &mut TestAppContext) {
    use biblioteca_core::Filtro;
    let e = abrir_o_ensaio(cx, Cenario::default());

    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.modo_do_cliente(), (false, 4), "nasce na foto única")
    });
    e.detalhe(cx, |tela, _w, cx| {
        tela.focar_foto("a", cx);
        cx.emit(Pedido::ModoDoCliente {
            grade: true,
            por_linha: 4,
        });
    });
    assert_eq!(
        grade_no_cliente(&e, cx),
        None,
        "sem a janela aberta, nada é mandado"
    );
    e.detalhe(cx, |tela, _w, cx| {
        assert_eq!(tela.modo_do_cliente(), (true, 4), "o menu marca a grade");
        cx.emit(Pedido::TelaDoCliente);
    });
    e.app(cx, |app, _w, _cx| assert!(app.cliente_aberto()));

    // As fotos do ensaio, numeradas pela ordem do recorte do operador; as
    // levadas no balcão vão como escolhidas.
    let (ids, levadas) = recorte(&e, cx);
    let todas: Vec<usize> = (1..=ids.len()).collect();
    let posicao = |id: &str| ids.iter().position(|i| i == id);
    assert!(ids.len() >= 4 && levadas.iter().any(|l| *l));
    assert_eq!(
        grade_no_cliente(&e, cx),
        Some((todas.clone(), posicao("a"), 4, levadas.clone()))
    );
    assert_eq!(no_cliente(&e, cx), None, "na grade não vai foto única");

    // A janela desenha a grade, com a página do foco.
    let janela = e
        .app(cx, |app, _w, _cx| app.janela_do_cliente_para_teste())
        .expect("a janela do cliente");
    janela
        .update(cx, |cliente, window, _cx| {
            let grade = cliente.grade_mostrada().expect("a grade está no ar");
            assert_eq!(grade.itens.len(), ids.len());
            let desenho = cliente
                .desenho_da_grade(window.viewport_size())
                .expect("o desenho");
            assert_eq!(desenho.pagina, 0);
            assert!(desenho.faixa.contains(&posicao("a").unwrap()));
            assert_eq!(desenho.arranjo.por_linha, 4);
        })
        .expect("a janela está aberta");
    cx.run_until_parked();

    // O foco anda: o número em foco acompanha, os números não mudam.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("d", cx));
    let (numeros, foco, ..) = grade_no_cliente(&e, cx).expect("a grade");
    assert_eq!((numeros, foco), (todas.clone(), posicao("d")));

    // As fotos por linha mudam a página, nunca o número; fora da faixa, o
    // limite.
    e.detalhe(cx, |_t, _w, cx| {
        cx.emit(Pedido::ModoDoCliente {
            grade: true,
            por_linha: 6,
        })
    });
    let (numeros, foco, por_linha, _) = grade_no_cliente(&e, cx).expect("a grade");
    assert_eq!((numeros, foco, por_linha), (todas.clone(), posicao("d"), 6));
    e.detalhe(cx, |_t, _w, cx| {
        cx.emit(Pedido::ModoDoCliente {
            grade: true,
            por_linha: 99,
        })
    });
    assert_eq!(grade_no_cliente(&e, cx).map(|g| g.2), Some(8));
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.modo_do_cliente(), (true, 8))
    });

    // 🚨 O recorte do operador muda: a grade do cliente mostra só as do
    // recorte, e **cada uma com o número que já tinha** (dono, 11/out/2026:
    // o filtro não pode renumerar, o cliente já disse "quero a 3").
    let filtradas = e.detalhe(cx, |tela, _w, cx| {
        tela.filtrar(
            Filtro::Situacao(biblioteca_core::Estado::LevadaNoBalcao),
            cx,
        );
        tela.ids_visiveis()
    });
    let visiveis = filtradas.len();
    let os_de_antes: Vec<usize> = filtradas
        .iter()
        .map(|id| posicao(id).expect("estava em Todas") + 1)
        .collect();
    let (numeros, ..) = grade_no_cliente(&e, cx).expect("a grade");
    assert_eq!(numeros, os_de_antes);
    assert_ne!(
        numeros,
        (1..=visiveis).collect::<Vec<_>>(),
        "senão o teste não distingue a renumeração"
    );
    assert!(visiveis < ids.len(), "o recorte encolheu");
    e.detalhe(cx, |tela, _w, cx| tela.filtrar(Filtro::Todas, cx));
    assert_eq!(grade_no_cliente(&e, cx).map(|g| g.0), Some(todas.clone()));

    // Na revelação o cliente vê a foto, ao vivo — e a galeria devolve a grade.
    e.revelar_a_do_site(cx, "a");
    assert_eq!(grade_no_cliente(&e, cx), None, "na revelação, a foto");
    assert_eq!(no_cliente(&e, cx).map(|c| c.0).as_deref(), Some("site:a"));
    e.esperar(cx);
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessao));
    assert!(
        grade_no_cliente(&e, cx).is_some(),
        "de volta à galeria, a grade volta"
    );

    // "Uma foto" no menu: a tela volta à foto em foco, na hora.
    e.detalhe(cx, |tela, _w, cx| {
        tela.focar_foto("b", cx);
        cx.emit(Pedido::ModoDoCliente {
            grade: false,
            por_linha: 8,
        });
    });
    assert_eq!(grade_no_cliente(&e, cx), None);
    assert_eq!(no_cliente(&e, cx).map(|c| c.0).as_deref(), Some("site:b"));

    // De volta à grade, fechar e reabrir: o modo fica.
    e.detalhe(cx, |_t, _w, cx| {
        cx.emit(Pedido::ModoDoCliente {
            grade: true,
            por_linha: 3,
        })
    });
    assert_eq!(grade_no_cliente(&e, cx).map(|g| g.2), Some(3));
    e.detalhe(cx, |_tela, _w, cx| cx.emit(Pedido::TelaDoCliente));
    e.app(cx, |app, _w, _cx| assert!(!app.cliente_aberto()));
    assert_eq!(grade_no_cliente(&e, cx), None, "fechada, nada no ar");
    e.detalhe(cx, |_tela, _w, cx| cx.emit(Pedido::TelaDoCliente));
    assert_eq!(
        grade_no_cliente(&e, cx).map(|g| (g.0, g.2)),
        Some((todas, 3)),
        "reaberta, a grade volta como estava"
    );
}

/// 🖥️ **Na Revelação, a tela do cliente abre pela ficha em cima da tira** —
/// a mesma da sessão, junto dos recortes (dono, 11/out/2026: *"deveria ter
/// feito a mesma coisa na filmstrip da revelação"*). O clique é o do dedo, na
/// ficha desenhada; e o modo escolhido ali é o que a galeria passa a usar.
#[gpui_kit::test]
fn na_revelacao_a_tela_do_cliente_abre_pela_ficha_da_tira(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Revelacao);
        assert!(!app.cliente_aberto());
    });

    super::chatbot::clicar(&e, cx, "revelacao-tela-do-cliente");
    e.app(cx, |app, _w, _cx| assert!(app.cliente_aberto()));
    assert_eq!(no_cliente(&e, cx).map(|c| c.0).as_deref(), Some("site:a"));

    // A ficha do modo está ao lado, na mesma linha.
    let mut visual = super::chatbot::quadro_novo(&e, cx);
    let (tela, modo) = (
        visual.debug_bounds("revelacao-tela-do-cliente").unwrap(),
        visual
            .debug_bounds("revelacao-modo-do-cliente")
            .expect("a ficha do modo está na tira da Revelação"),
    );
    assert_eq!(tela.origin.y, modo.origin.y, "as duas na mesma linha");
    assert!(modo.origin.x > tela.origin.x);

    super::chatbot::clicar(&e, cx, "revelacao-tela-do-cliente");
    e.app(cx, |app, _w, _cx| assert!(!app.cliente_aberto()));
}
