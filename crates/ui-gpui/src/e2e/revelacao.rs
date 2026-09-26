//! 🎚️ A Revelação: a tira, os sliders, o histórico, as abas, a curva por
//! ponto e as predefinições.

use biblioteca_core::acervo::{Estado, Filtro};
use gpui::TestAppContext;

use super::{abrir_o_ensaio, Cenario};
use crate::app::Tela;
use crate::revelacao::curva::{curva_neutra, Canal};
use crate::revelacao::lightroom::Arquivo;
use crate::revelacao::presets::ordem::Grupo;

/// 🎬 **Abrir da sessão, andar pela tira e revelar com histórico.**
///
/// Cada gesto de slider é **um** passo do `⌘Z`, e o `⇧⌘Z` volta um a um —
/// com as teclas de verdade, nas duas plataformas.
#[gpui::test]
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

    // Dois gestos: exposição e contraste.
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 1.5, cx));
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, cx| {
        assert_eq!(tela.ajustes().exposure, 1.5);
        tela.arrastar_slider(1, 1.3, cx);
    });
    e.esperar(cx);
    let gravado = e.gravador.gravado();
    assert_eq!(gravado.len(), 2, "um gesto, uma gravação: {gravado:?}");
    assert!(gravado.iter().all(|(id, _, _)| id == "site:a"));
    assert!(
        e.gravador.deposito().iter().any(|(id, _)| id == "a"),
        "a receita da foto do site fica no depósito até subir"
    );

    // ⌘Z desfaz o contraste, e só ele.
    e.teclar(cx, "cmd-z");
    e.revelacao(cx, |tela, _w, cx| {
        assert_eq!(tela.ajustes().contrast, 1.0);
        assert_eq!(tela.ajustes().exposure, 1.5, "um passo por gesto");
        assert_eq!(tela.valor_do_slider(1, cx), 1.0, "o slider voltou junto");
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
        assert_eq!(tela.ajustes().contrast, 1.3);
        assert!(!tela.pode_refazer());
    });
    let ultimo = e
        .gravador
        .gravado()
        .last()
        .cloned()
        .expect("o refazer gravou");
    assert_eq!(
        ultimo.1.contrast, 1.3,
        "o histórico vai para o banco também"
    );

    // Esc sai para a galeria; voltar à foto traz a receita dela.
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessao));
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.ajustes().exposure, 1.5, "sair e voltar traz a receita");
        assert_eq!(tela.ajustes().contrast, 1.3);
        assert!(
            !tela.pode_desfazer(),
            "a foto reaberta começa um histórico novo"
        );
    });
}

/// 🎬 **As abas sRGB e RGB, e a curva por ponto.**
#[gpui::test]
fn abas_de_espaco_e_curva_por_ponto(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "b");

    e.revelacao(cx, |tela, _w, cx| {
        assert!(!tela.na_aba_rgb(), "abre na sRGB");
        assert_eq!(tela.abas_alteradas(), (false, false));
        tela.arrastar_slider(0, 0.8, cx);
    });
    // 🔑 O `Change` do slider chega à tela quando o gesto termina — como o
    // arrasto de verdade, que é um evento e não uma chamada.
    e.revelacao(cx, |tela, _w, cx| {
        assert_eq!(
            tela.abas_alteradas(),
            (true, false),
            "o ponto âmbar da sRGB"
        );
        tela.escolher_aba_rgb(true, cx);
        assert!(tela.na_aba_rgb());
        let rgb = tela
            .controle_onde(|d| d.secao.painel().no_rgb() && !d.discreto)
            .expect("um slider contínuo da aba RGB");
        tela.arrastar_slider(rgb, 0.5, cx);
    });
    e.revelacao(cx, |tela, _w, cx| {
        assert_eq!(tela.abas_alteradas(), (true, true), "o ponto âmbar da RGB");
        tela.escolher_aba_rgb(false, cx);
        assert!(!tela.na_aba_rgb());
    });
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
#[gpui::test]
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
                    texto: Some(
                        include_str!("../revelacao/lightroom/recordarfotos-pb.dtstyle").into(),
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
        let (ajustes_do_cliente, _) = tela.receita_para_o_cliente();
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

    // 📂 Importar do Lightroom e do darktable, pelo seletor do sistema.
    e.revelacao(cx, |tela, _w, cx| tela.importar_do_lightroom(cx));
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, cx| {
        assert_eq!(tela.relatorio_da_importacao(), Some((2, 2)));
        let minhas = tela.coluna_de_predefinicoes(cx).1;
        assert!(
            minhas.contains(&"Cross Process-Cyan".to_string()),
            "{minhas:?}"
        );
        assert!(
            minhas.contains(&"RecordarFotos P&B".to_string()),
            "{minhas:?}"
        );
        tela.fechar_relatorio(cx);
        assert_eq!(tela.relatorio_da_importacao(), None);
    });
    assert_eq!(e.guarda.salvos().len(), 3);

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
#[gpui::test]
fn o_canto_dos_envios_se_arrasta_para_fora_da_tira(cx: &mut TestAppContext) {
    use gpui::{point, px, Modifiers, MouseButton};

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

    let mut visual = gpui::VisualTestContext::from_window(e.raiz.into(), cx);
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
    let mut visual = gpui::VisualTestContext::from_window(e.raiz.into(), cx);
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
