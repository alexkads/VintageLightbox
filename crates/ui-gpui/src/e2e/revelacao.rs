//! 🎚️ A Revelação: a tira, os sliders, o histórico, as abas, a curva por
//! ponto e as predefinições.

use biblioteca_core::acervo::{Estado, Filtro};
use gpui_kit::TestAppContext;

use super::{abrir_o_ensaio, Cenario};
use crate::app::Tela;
use crate::revelacao::curva::{curva_neutra, Canal};
use crate::revelacao::lightroom::Arquivo;
use crate::revelacao::presets::ordem::Grupo;

/// 🎬 **Abrir da sessão, andar pela tira e revelar com histórico.**
///
/// Cada gesto de slider é **um** passo do `⌘Z`, e o `⇧⌘Z` volta um a um —
/// com as teclas de verdade, nas duas plataformas.
#[gpui_kit::test]
fn andar_pela_tira_revelar_e_desfazer_um_gesto_por_vez(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");

    // A tira tem a sessão inteira; as setas andam por ela.
    let (tira, posicao) = e.revelacao(cx, |tela, _w, _cx| {
        let ids: Vec<String> = tela.acervo().iter().map(|f| f.id.clone()).collect();
        (ids, tela.posicao())
    });
    assert!(tira.len() >= 6, "a sessão inteira na tira: {tira:?}");
    e.teclar(cx, "right");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.posicao(), posicao + 1, "→ anda uma foto");
    });
    e.teclar(cx, "left");
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.foto_aberta().map(|f| f.id.as_str()), Some("site:a"));
    });

    // O recorte da tira é o da sessão: só as levadas.
    e.revelacao(cx, |tela, _w, cx| {
        tela.recortar(Filtro::Situacao(Estado::LevadaNoBalcao), cx);
        assert_eq!(tela.na_tira().len(), 2);
        tela.recortar(Filtro::Todas, cx);
        assert_eq!(tela.na_tira().len(), tira.len());
    });

    // Dois gestos: exposição e contraste. O contraste anda na escala do
    // Lightroom (+30), e o motor guarda o multiplicador (1,3).
    let contraste = 1.0 + 30.0 / 100.0;
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 1.5, cx));
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, cx| {
        assert_eq!(tela.ajustes().exposure, 1.5);
        tela.arrastar_slider(1, 30.0, cx);
    });
    e.esperar(cx);
    let gravado = e.gravador.gravado();
    assert_eq!(gravado.len(), 2, "um gesto, uma gravação: {gravado:?}");
    assert!(gravado.iter().all(|(id, _, _)| id == "site:a"));
    assert!(
        e.gravador.deposito().iter().any(|(id, _)| id == "a"),
        "a revelação da foto do site fica no depósito até subir"
    );

    // ⌘Z desfaz o contraste, e só ele.
    e.teclar(cx, "cmd-z");
    e.revelacao(cx, |tela, _w, cx| {
        assert_eq!(tela.ajustes().contrast, 1.0);
        assert_eq!(tela.ajustes().exposure, 1.5, "um passo por gesto");
        assert_eq!(tela.valor_do_slider(1, cx), 0.0, "o slider voltou junto");
    });
    e.teclar(cx, "ctrl-z");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.ajustes().exposure, 0.0);
        assert!(!tela.pode_desfazer());
    });
    e.teclar(cx, "cmd-shift-z");
    e.revelacao(cx, |tela, _w, _cx| assert_eq!(tela.ajustes().exposure, 1.5));
    e.teclar(cx, "ctrl-shift-z");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.ajustes().contrast, contraste);
        assert!(!tela.pode_refazer());
    });
    let ultimo = e
        .gravador
        .gravado()
        .last()
        .cloned()
        .expect("o refazer gravou");
    assert_eq!(
        ultimo.1.contrast, contraste,
        "o histórico vai para o banco também"
    );

    // Esc sai para a galeria; voltar à foto traz a revelação dela.
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessao));
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.ajustes().exposure,
            1.5,
            "sair e voltar traz a revelação"
        );
        assert_eq!(tela.ajustes().contrast, contraste);
        // 📜 A foto reaberta traz o histórico dela, como no Lightroom — e a
        // foto do site, que volta com o corte escrito por extenso, não ganha
        // um "Mudou fora da Revelação" à toa.
        assert!(
            tela.pode_desfazer(),
            "a foto reaberta traz o histórico dela"
        );
        let nomes: Vec<_> = tela
            .passos_do_historico()
            .0
            .iter()
            .map(|p| p.rotulo.nome.clone())
            .collect();
        assert!(
            !nomes
                .iter()
                .any(|n| n == crate::revelacao::historico::DE_FORA),
            "nada mudou por fora: {nomes:?}"
        );
    });
}

/// 🎬 **A curva por ponto.** (As abas sRGB e RGB saíram em 2/out/2026, com
/// os módulos do darktable.)
#[gpui_kit::test]
fn curva_por_ponto(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "b");
    e.esperar(cx);

    // A curva do vermelho: arrastar um nó, e o duplo clique o devolve à reta.
    e.revelacao(cx, |tela, _w, cx| {
        tela.escolher_canal_da_curva(Canal::Vermelho, cx);
        tela.arrastar_no_da_curva(4, 200.0, cx);
        assert_eq!(tela.alturas_da_curva(Canal::Vermelho)[4], 200.0);
        assert_eq!(
            tela.alturas_da_curva(Canal::Rgb),
            curva_neutra(),
            "só o canal escolhido mudou"
        );
    });
    e.esperar(cx);
    let gravado = e.gravador.gravado();
    let ultimo = gravado.last().expect("o nó foi gravado");
    assert_eq!(ultimo.0, "site:b");
    assert_eq!(ultimo.1.curva_r4, 200.0);

    e.revelacao(cx, |tela, window, cx| {
        tela.duplo_clique_no_da_curva(4, window, cx);
        assert_eq!(tela.alturas_da_curva(Canal::Vermelho), curva_neutra());
        assert!(tela.pode_desfazer());
    });
    e.teclar(cx, "cmd-z");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.alturas_da_curva(Canal::Vermelho)[4],
            200.0,
            "o duplo clique é um passo de histórico"
        );
    });
}

/// 🎬 **A Curva de tons do Lightroom, com o ponteiro de verdade**: arrastar
/// para cima no gráfico sobe a região sob o ponteiro, o pino da barra leva o
/// divisor junto, e o círculo vermelho troca para a curva por ponto.
#[gpui_kit::test]
fn a_curva_de_tons_responde_ao_ponteiro(cx: &mut TestAppContext) {
    use crate::revelacao::tela::painel::ModoDaCurva;
    use gpui_kit::{point, px, Modifiers, MouseButton, VisualTestContext};

    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "b");
    // O caixa flutuante fica por cima do canto da coluna: o F9 o minimiza.
    e.teclar(cx, "f9");
    e.revelacao(cx, |tela, _w, cx| {
        tela.seguir_o_roteiro_do_painel("fechar Básico", cx);
        tela.seguir_o_roteiro_do_painel("abrir Curva de tons", cx);
        assert_eq!(tela.modo_da_curva(), ModoDaCurva::Parametrica);
    });
    e.esperar(cx);

    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.run_until_parked();
    let grafico = visual
        .debug_bounds("grafico-da-curva")
        .expect("o gráfico da curva está na coluna");
    assert!(
        (f32::from(grafico.size.width) - f32::from(grafico.size.height)).abs() < 1.0,
        "o gráfico é quadrado: {grafico:?}"
    );

    // Arrastar para cima a 62% da largura — a região dos claros.
    let x = grafico.origin.x + grafico.size.width * 0.62;
    let y = grafico.origin.y + grafico.size.height * 0.5;
    let nada = Modifiers::none();
    // ⚠️ No harness o primeiro evento de ponteiro da janela só a apresenta ao
    // mouse: sem este movimento antes, o primeiro apertar se perde.
    visual.simulate_mouse_move(point(x, y), None, nada);
    visual.simulate_mouse_down(point(x, y), MouseButton::Left, nada);
    for passo in 1..=6 {
        visual.simulate_mouse_move(
            point(x, y - px(10.0 * passo as f32)),
            MouseButton::Left,
            nada,
        );
    }
    visual.simulate_mouse_up(point(x, y - px(60.)), MouseButton::Left, nada);
    visual.run_until_parked();
    let claros = e.revelacao(cx, |tela, _w, _cx| tela.ajustes().tone_curve_lights);
    assert!(
        claros > 0.0,
        "o arrasto para cima não subiu os claros ({claros})"
    );
    let outros = e.revelacao(cx, |tela, _w, _cx| {
        let a = tela.ajustes();
        (
            a.tone_curve_shadows,
            a.tone_curve_darks,
            a.tone_curve_highlights,
        )
    });
    assert_eq!(outros, (0.0, 0.0, 0.0), "só a região sob o ponteiro");

    // O pino dos médios, arrastado para a direita.
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    let barra = visual
        .debug_bounds("barra-de-divisao")
        .expect("a barra de divisão");
    let margem = barra.size.width * (8.0 / 260.0);
    let util = barra.size.width - margem * 2.0;
    let meio = barra.origin.x + margem + util * 0.5;
    let y = barra.origin.y + px(8.);
    visual.simulate_mouse_down(point(meio, y), MouseButton::Left, nada);
    for passo in 1..=5 {
        visual.simulate_mouse_move(
            point(meio + util * (0.02 * passo as f32), y),
            MouseButton::Left,
            nada,
        );
    }
    visual.simulate_mouse_up(point(meio + util * 0.1, y), MouseButton::Left, nada);
    visual.run_until_parked();
    let medios = e.revelacao(cx, |tela, _w, _cx| tela.ajustes().tone_curve_split_midtones);
    assert!(
        (medios - 60.0).abs() <= 1.0,
        "o pino dos médios devia ir a ~60, foi a {medios}"
    );
    e.esperar(cx);
    let gravado = e.gravador.gravado();
    let ultimo = gravado.last().expect("a curva foi gravada");
    assert!(ultimo.1.tone_curve_lights > 0.0);
    assert_eq!(ultimo.1.tone_curve_split_midtones, medios);

    // O círculo vermelho troca para a curva por ponto do vermelho.
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    let vermelho = visual
        .debug_bounds("canal-Vermelho")
        .expect("o círculo vermelho");
    visual.simulate_click(vermelho.center(), nada);
    visual.run_until_parked();
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.modo_da_curva(), ModoDaCurva::Ponto(Canal::Vermelho));
    });
}

/// Um `.lrtemplate` como o Lightroom os grava — a estrutura é Lua.
const LRTEMPLATE: &str = r#"s = {
	id = "D728662A-4FF5-4327-A6AA-444291159768",
	internalName = "Cross Process Cyan",
	title = "Cross Process-Cyan",
	type = "Develop",
	value = {
		settings = {
			Clarity2012 = 25,
			Contrast2012 = -20,
			Saturation = 10,
		},
		uuid = "1C3DBCB9-9A9F-4D6C-A7BA-A42A5B8EB4E0",
	},
	version = 0,
}"#;

/// 🎬 **Predefinições, de ponta a ponta**: prever sem mexer, aplicar com
/// desfazer, criar, renomear, reordenar, importar do Lightroom e do
/// darktable, e apagar com a pergunta — confirmada pelo Enter.
#[gpui_kit::test]
fn predefinicoes_prever_aplicar_criar_renomear_reordenar_importar_e_apagar(
    cx: &mut TestAppContext,
) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            arquivos_de_predefinicao: vec![
                Arquivo {
                    nome: "Cross Process-Cyan.lrtemplate".into(),
                    texto: Some(LRTEMPLATE.into()),
                },
                Arquivo {
                    nome: "RecordarFotos P&B.dtstyle".into(),
                    // Um estilo do darktable: desde 2/out/2026 vai para os
                    // ilegíveis, e não vira predefinição.
                    texto: Some(
                        "<darktable_style version=\"1.0\"><info><name>RecordarFotos P&amp;B</name></info><style></style></darktable_style>"
                            .into(),
                    ),
                },
            ],
            ..Cenario::default()
        },
    );
    e.revelar_a_do_site(cx, "a");

    // 👁️ Prever muda o que a GPU desenha, não os ajustes.
    e.revelacao(cx, |tela, _w, cx| {
        let claro = tela.predefinicao("Claro").expect("a do sistema");
        tela.prever(Some(&claro), cx);
        assert!(tela.previa().is_some());
        assert_eq!(
            tela.ajustes().exposure,
            0.0,
            "a prévia não entra nos ajustes"
        );
        let (ajustes_do_cliente, _) = tela.parametros_para_o_cliente();
        assert_eq!(ajustes_do_cliente.exposure, 1.0, "o cliente vê a prévia");
        tela.prever(None, cx);
        assert!(tela.previa().is_none());
    });
    assert!(e.gravador.gravado().is_empty(), "prever não grava");

    // ✅ Aplicar é um passo de histórico, gravado na hora.
    e.revelacao(cx, |tela, window, cx| {
        let escuro = tela.predefinicao("Escuro").expect("a do sistema");
        tela.aplicar_preset(&escuro, window, cx);
        assert_eq!(tela.ajustes().exposure, -1.0);
    });
    assert_eq!(e.gravador.gravado().len(), 1, "aplicar grava sem esperar");
    e.teclar(cx, "cmd-z");
    e.revelacao(cx, |tela, _w, _cx| assert_eq!(tela.ajustes().exposure, 0.0));

    // ➕ Criar: sem ajuste nada é guardado; com ajuste, entra em "Minhas".
    e.revelacao(cx, |tela, window, cx| {
        tela.alternar_formulario_de_preset(window, cx);
        assert!(tela.formulario_de_predefinicao_aberto());
        tela.digitar_nome_da_predefinicao("Pôr do sol", window, cx);
        tela.salvar_preset(window, cx);
        assert!(
            tela.formulario_de_predefinicao_aberto(),
            "nada fora do neutro"
        );
        tela.arrastar_slider(0, 0.7, cx);
    });
    e.revelacao(cx, |tela, window, cx| {
        tela.salvar_preset(window, cx);
        assert!(!tela.formulario_de_predefinicao_aberto());
        assert_eq!(tela.coluna_de_predefinicoes(cx).1, vec!["Pôr do sol"]);
    });
    let salvos = e.guarda.salvos();
    assert_eq!(salvos.len(), 1);
    assert_eq!(salvos[0].adjustments.get("exposure"), Some(0.7));

    // ✏️ Renomear.
    e.revelacao(cx, |tela, window, cx| {
        let minha = tela.predefinicao("Pôr do sol").expect("a criada");
        tela.comecar_a_renomear(minha.id, window, cx);
        tela.renomear_preset(minha.id, "  Fim de tarde ".into(), cx);
        assert_eq!(tela.coluna_de_predefinicoes(cx).1, vec!["Fim de tarde"]);
    });
    assert_eq!(e.guarda.renomeados()[0].1, "Fim de tarde");

    // ↕️ Reordenar pelo arrasto, e voltar à ordem padrão.
    e.revelacao(cx, |tela, _w, cx| {
        assert_eq!(
            tela.coluna_de_predefinicoes(cx).0,
            vec!["Claro", "Escuro", "Neutro+"]
        );
        tela.arrastar_predefinicao_do_sistema(1, 0, cx);
        assert_eq!(
            tela.coluna_de_predefinicoes(cx).0,
            vec!["Escuro", "Claro", "Neutro+"]
        );
        tela.definir_ordem_dos_presets(Grupo::Sistema, None, cx);
        assert_eq!(
            tela.coluna_de_predefinicoes(cx).0,
            vec!["Claro", "Escuro", "Neutro+"]
        );
    });

    // 📂 Importar do Lightroom pelo seletor do sistema; o do darktable não entra.
    e.revelacao(cx, |tela, _w, cx| tela.importar_do_lightroom(cx));
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, cx| {
        assert_eq!(tela.relatorio_da_importacao(), Some((2, 1)));
        let minhas = tela.coluna_de_predefinicoes(cx).1;
        assert!(
            minhas.contains(&"Cross Process-Cyan".to_string()),
            "{minhas:?}"
        );
        assert!(
            !minhas.contains(&"RecordarFotos P&B".to_string()),
            "{minhas:?}"
        );
        tela.fechar_relatorio(cx);
        assert_eq!(tela.relatorio_da_importacao(), None);
    });
    assert_eq!(e.guarda.salvos().len(), 2);

    // 🗑️ Apagar pergunta; "Cancelar" não apaga, Enter apaga.
    e.revelacao(cx, |tela, window, cx| {
        let minha = tela.predefinicao("Fim de tarde").expect("a renomeada");
        tela.clicar_na_lixeira(&minha, window, cx);
        assert_eq!(
            tela.nome_na_pergunta_de_apagar().as_deref(),
            Some("Fim de tarde")
        );
        tela.responder_pergunta(false, window, cx);
        assert!(tela.predefinicao("Fim de tarde").is_some());
        tela.clicar_na_lixeira(&minha, window, cx);
    });
    e.teclar(cx, "enter");
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(tela.nome_na_pergunta_de_apagar().is_none());
        assert!(
            tela.predefinicao("Fim de tarde").is_none(),
            "o Enter confirmou"
        );
    });
    assert_eq!(e.guarda.apagados().len(), 1);
}

/// 🎬 **O canto dos envios sai de cima da tira pela alça**, como o caixa
/// (dono, 24/set/2026: *"em alguns momentos esse status pode ficar por cima do
/// filmstrip"*).
///
/// Com o ensaio subindo e a Revelação aberta, o mouse de verdade agarra a alça,
/// leva o canto 400 px para a direita e 300 px para cima e solta: o canto vai
/// junto, e fica. Soltar sem a alça apertada não mexe em nada.
#[gpui_kit::test]
fn o_canto_dos_envios_se_arrasta_para_fora_da_tira(cx: &mut TestAppContext) {
    use gpui_kit::{point, px, Modifiers, MouseButton};

    let e = abrir_o_ensaio(
        cx,
        Cenario {
            site: Box::new(|site| site.demorada = true),
            ..Cenario::default()
        },
    );
    e.revelar_a_do_site(cx, "a");
    e.app(cx, |app, _w, _cx| {
        assert!(app.sincronias_pendentes() > 0, "o ensaio está subindo");
        assert_eq!(app.canto_dos_envios_para_teste(), (0., 0.), "nasce no pé");
    });

    let mut visual = gpui_kit::VisualTestContext::from_window(e.raiz.into(), cx);
    let antes = visual
        .debug_bounds("canto-dos-envios")
        .expect("o canto aparece na Revelação");
    let alca = visual
        .debug_bounds("canto-dos-envios-alca")
        .expect("com a alça");
    let de = alca.center();
    let ate = point(de.x + px(400.), de.y - px(300.));
    visual.simulate_mouse_down(de, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_move(
        point(de.x + px(200.), de.y - px(150.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    visual.simulate_mouse_move(ate, MouseButton::Left, Modifiers::none());
    visual.simulate_mouse_up(ate, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Revelacao, "arrastar não sai do editor");
        assert_eq!(app.canto_dos_envios_para_teste(), (400., -300.));
    });
    let mut visual = gpui_kit::VisualTestContext::from_window(e.raiz.into(), cx);
    let depois = visual
        .debug_bounds("canto-dos-envios")
        .expect("o canto continua à vista");
    assert_eq!(
        depois.origin.x - antes.origin.x,
        px(400.),
        "foi para a direita"
    );
    assert_eq!(antes.origin.y - depois.origin.y, px(300.), "e para cima");

    // O ponteiro que anda sem a alça apertada não leva o canto.
    visual.simulate_mouse_move(
        point(px(10.), px(10.)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.run_until_parked();
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.canto_dos_envios_para_teste(), (400., -300.), "e fica");
    });
}

/// 🎬 **A tira classifica como a galeria** (dono, 27/set/2026: *"os dois
/// estão em sintonia"*): a nota, o `P` e o `X` teclados na Revelação valem
/// para a aberta — ou para as marcadas da tira, se ela está entre elas — e a
/// tira mostra o que a grade da sessão mostra.
#[gpui_kit::test]
fn a_tira_aceita_a_nota_o_p_e_o_x_da_galeria(cx: &mut TestAppContext) {
    use biblioteca_core::Modificadores;

    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    // A tira e a grade dizem o mesmo sobre `a` e `b`.
    let em_sintonia = |cx: &mut TestAppContext| {
        let ids = vec!["a".to_string(), "b".to_string()];
        let da_grade = e.detalhe(cx, |tela, _w, _cx| tela.classificacoes(&ids));
        e.revelacao(cx, |tela, _w, _cx| {
            for (id, grade) in &da_grade {
                let tira = tela.classificacao_na_tira(id).expect("a foto está na tira");
                assert_eq!(
                    (tira.nota, tira.estado, tira.apagada),
                    (grade.nota, grade.estado, grade.apagada),
                    "a tira mostra o que a grade mostra: {id}"
                );
            }
        });
    };

    e.teclar(cx, "3");
    e.esperar(cx);
    let negociadas = e.site.negociadas();
    let ultima = negociadas.last().expect("a nota foi ao site");
    assert_eq!((ultima.0.as_str(), ultima.1.nota), ("a", Some(Some(3))));
    em_sintonia(cx);

    // Com a aberta e mais uma marcadas na tira, a nota cai nas duas.
    e.revelacao(cx, |tela, window, cx| {
        let b = tela
            .acervo()
            .iter()
            .position(|f| f.id == "site:b")
            .expect("b na tira");
        tela.clicar_na_tira(
            b,
            Modificadores {
                aditivo: true,
                faixa: false,
            },
            window,
            cx,
        );
    });
    let antes = e.site.negociadas().len();
    e.teclar(cx, "5");
    e.esperar(cx);
    let novas: Vec<_> = e.site.negociadas()[antes..]
        .iter()
        .map(|(id, m)| (id.clone(), m.nota))
        .collect();
    assert_eq!(
        novas,
        vec![
            ("a".to_string(), Some(Some(5))),
            ("b".to_string(), Some(Some(5)))
        ]
    );
    em_sintonia(cx);

    // O `P` nas duas marcadas: as levadas voltam à venda, como na galeria.
    let antes = e.site.negociadas().len();
    e.teclar(cx, "p");
    e.esperar(cx);
    assert!(e.site.negociadas().len() > antes, "o P da tira foi ao site");
    em_sintonia(cx);

    // E o `X`: na foto da nuvem ele vai ao resgate, que pede o bruto antes de
    // tirar qualquer coisa de lá — como na galeria.
    let antes = e.site.originais().len();
    e.teclar(cx, "x");
    e.esperar(cx);
    assert_eq!(
        e.site.originais()[antes..],
        ["a", "b"],
        "o X da tira chegou às duas"
    );
    em_sintonia(cx);
}

/// 🗑️ **A lixeira da linha apaga com o mouse de verdade** (dono, 3/out/2026:
/// *"A exclusão de preset não está funcionando"*). O teste acima chama o
/// método da lixeira direto; este clica nela e no "Apagar" da pergunta.
#[gpui_kit::test]
fn a_lixeira_da_linha_apaga_com_o_clique(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, VisualTestContext};

    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, window, cx| {
        tela.alternar_formulario_de_preset(window, cx);
        tela.digitar_nome_da_predefinicao("Pôr do sol", window, cx);
        tela.arrastar_slider(0, 0.7, cx);
    });
    e.revelacao(cx, |tela, window, cx| {
        tela.salvar_preset(window, cx);
        assert_eq!(tela.coluna_de_predefinicoes(cx).1, vec!["Pôr do sol"]);
    });

    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.run_until_parked();
    let lixeira = visual
        .debug_bounds("lixeira-Pôr do sol")
        .expect("a lixeira da linha é desenhada");
    visual.simulate_mouse_move(lixeira.center(), None, Modifiers::none());
    visual.simulate_click(lixeira.center(), Modifiers::none());
    visual.run_until_parked();
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.nome_na_pergunta_de_apagar().as_deref(),
            Some("Pôr do sol"),
            "o clique na lixeira pergunta"
        );
    });

    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.run_until_parked();
    let apagar = visual
        .debug_bounds("confirmar-apagar")
        .expect("o \"Apagar\" da pergunta é desenhado");
    visual.simulate_mouse_move(apagar.center(), None, Modifiers::none());
    visual.simulate_click(apagar.center(), Modifiers::none());
    visual.run_until_parked();
    e.revelacao(cx, |tela, _w, cx| {
        assert!(tela.nome_na_pergunta_de_apagar().is_none());
        assert!(
            tela.coluna_de_predefinicoes(cx).1.is_empty(),
            "o \"Apagar\" tira da lista"
        );
    });
    assert_eq!(e.guarda.apagados().len(), 1, "e do banco");
}
