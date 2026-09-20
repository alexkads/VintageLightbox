//! 🖼️ Dentro da sessão: a galeria do ensaio, como a rota `[id]` do site.

use biblioteca_core::acervo::{Estado, Filtro};
use domain::services::pos_venda::{EstadoNoBalcao, MudancaDaGaleria};
use gpui::{TestAppContext, VisualTestContext};

use super::{abrir_o_ensaio, local, Cenario, Estudio, GALERIA};
use crate::app::Tela;
use crate::impressao::porta::Destino;

/// Clica no botão marcado com `debug_selector`, onde o dedo clicaria.
fn clicar(e: &Estudio, cx: &mut TestAppContext, alvo: &'static str) {
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.run_until_parked();
    let onde = visual
        .debug_bounds(alvo)
        .unwrap_or_else(|| panic!("o botão {alvo} não está desenhado na tela"));
    visual.simulate_click(onde.center(), gpui::Modifiers::none());
    visual.run_until_parked();
}

/// 🚨 **A receita padrão da sessão vale para quem chega depois.**
///
/// A predefinição e a proporção escolhidas na etapa 2 do assistente ficam **na
/// galeria**. Quem importa mais fotos dentro da sessão espera o mesmo visual — e
/// era o que não acontecia: o serviço da receita só atendia o assistente, e a
/// leva seguinte entrava crua (achado do dono, 17/set/2026).
#[gpui::test]
fn a_foto_importada_na_sessao_recebe_a_receita_padrao(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            site: Box::new(|site| {
                for galeria in site.galerias.lock().unwrap().iter_mut() {
                    if galeria.id == GALERIA {
                        galeria.preset_padrao_id = Some("sistema:sepia".into());
                        galeria.proporcao_padrao = Some("1:1".into());
                    }
                }
            }),
            ..Default::default()
        },
    );

    // As locais do ensaio já entram na conta da receita: elas são as que ainda
    // não subiram, e é nelas que a receita da galeria manda.
    e.app(cx, |app, _w, _cx| {
        assert!(
            app.receita_padrao_pedida() > 0,
            "a receita da galeria tinha de ser pedida para as fotos locais"
        );
    });
}

/// 🧾 **A faixa escolhida na barra sobe com a foto.**
///
/// É a primeira das duas escolhas antes dos arquivos, no site: a sessão mista
/// sobe a mãe sozinha numa faixa e a família em outra. O campo existia na tela
/// (`escolher_faixa`) e **não chegava ao site** — nem o catálogo de faixas era
/// carregado, então o seletor nem aparecia.
#[gpui::test]
fn a_faixa_da_barra_sobe_com_a_foto_classificada(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    e.detalhe(cx, |tela, _w, cx| {
        assert!(!tela.produtos().is_empty(), "o catálogo chega com a sessão");
        tela.escolher_faixa(Some("p1".into()), cx);
    });

    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("id-DSC_101.jpg", cx));
    e.teclar(cx, "4");
    e.esperar(cx);

    assert_eq!(
        e.site.faixas_pedidas(),
        vec![Some("p1".to_string())],
        "a foto tinha de subir na faixa escolhida"
    );
}

/// 🎬 **Importar, classificar e levar**: o botão "Importar" abre o seletor do
/// sistema, o lote vai para o catálogo **local** com o carimbo do ensaio, a
/// nota dada pela tecla sobe a foto (passo 3), e o `B` leva a do site.
#[gpui::test]
fn importar_classificar_e_levar_pelas_teclas(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    // A grade é uma só: as quatro do site e as duas locais.
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.total_visivel(), 6, "site e disco na mesma grade");
        assert_eq!(tela.contagem(), (2, 1, 1), "levadas, à venda, compradas");
    });

    // 📥 Importar, pelo clique no botão.
    e.acervo.fotos.lock().unwrap().push(local("DSC_201.jpg"));
    clicar(&e, cx, "detalhe-importar");
    e.esperar(cx);
    assert_eq!(e.seletor_de_fotos.pedidos(), 1, "a janela do sistema abriu");
    let lotes = e.importador.importados();
    assert_eq!(lotes.len(), 1);
    assert_eq!(
        lotes[0].0,
        vec!["/cartao/DSC_201.jpg", "/cartao/DSC_202.jpg"]
    );
    assert_eq!(lotes[0].1.sessao_id.as_deref(), Some(GALERIA));
    assert!(
        e.site.arquivos_enviados().is_empty(),
        "importar não sobe nada — quem autoriza é a nota"
    );
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.total_visivel(), 7, "a importada entrou na grade");
    });

    // ⭐ A nota pela tecla, numa foto local: ela sobe.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("id-DSC_101.jpg", cx));
    e.teclar(cx, "4");
    e.esperar(cx);
    let subidas = e.site.subidas();
    assert_eq!(subidas.len(), 1, "classificar sobe: {subidas:?}");
    assert_eq!(subidas[0].0, GALERIA);
    assert_eq!(subidas[0].1, "id-DSC_101.jpg");
    assert_eq!(
        e.site.notas_pedidas(),
        vec![Some(4)],
        "com a nota que acabou de ser dada"
    );

    // O `B` numa foto local não tem onde gravar: a tela diz o que fazer.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("id-DSC_102.jpg", cx));
    e.teclar(cx, "b");
    e.detalhe(cx, |tela, _w, _cx| {
        assert!(
            tela.erro().is_some_and(|f| f.contains("classifique-as")),
            "sinalizar a que não subiu avisa: {:?}",
            tela.erro()
        );
    });

    // 🛍️ O `B` na foto à venda: ela vira levada no balcão.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("d", cx));
    e.teclar(cx, "b");
    e.esperar(cx);
    let negociadas = e.site.negociadas();
    assert_eq!(negociadas.len(), 1, "{negociadas:?}");
    assert_eq!(negociadas[0].0, "d");
    assert_eq!(negociadas[0].1.estado, Some(EstadoNoBalcao::LevadaNoBalcao));

    // ⭐ E a nota nela, que já está no site, é uma mudança da foto.
    e.teclar(cx, "5");
    e.esperar(cx);
    let negociadas = e.site.negociadas();
    assert_eq!(negociadas.len(), 2);
    assert_eq!(negociadas[1].1.nota, Some(Some(5)));

    // 🚨 **O `0` numa foto do acervo pergunta antes** — e nada sai da nuvem
    // enquanto a resposta não vem. O caminho de volta inteiro (o bruto para cá,
    // o catálogo, os parâmetros, e só então a remoção) está em
    // `e2e::resgate`; aqui o que se prende é que a tecla **abre a pergunta** em
    // vez de recusar, como fazia até 18/set/2026.
    e.teclar(cx, "0");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.fotos_na_pergunta_de_tirar_do_acervo(),
            vec!["d.jpg".to_string()],
            "a pergunta segura a foto até o operador responder"
        );
    });
    assert_eq!(e.site.negociadas().len(), 2, "o 0 não foi ao site");
    assert!(e.site.tiradas().is_empty());
    e.detalhe(cx, |tela, _w, cx| tela.cancelar_tirar_do_acervo(cx));

    // A comprada não muda por lote.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("c", cx));
    e.teclar(cx, "b");
    e.esperar(cx);
    assert_eq!(e.site.negociadas().len(), 2, "a comprada fica de fora");
}

/// 🎬 **Recortes, zoom e marcação**: os chips recortam, a seleção limpa ao
/// trocar de recorte, `⌘A`/`⌘D` marcam e desmarcam, as setas andam, e o zoom
/// da grade anda dentro dos limites.
#[gpui::test]
fn recortes_zoom_e_marcacao_da_grade(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    e.teclar(cx, "cmd-a");
    e.detalhe(cx, |tela, _w, cx| {
        assert_eq!(tela.quantas_marcadas(), 6, "⌘A marca a grade inteira");
        tela.filtrar(Filtro::Situacao(Estado::LevadaNoBalcao), cx);
        assert_eq!(tela.total_visivel(), 2, "as duas levadas");
        assert_eq!(
            tela.quantas_marcadas(),
            0,
            "trocar o recorte limpa a seleção"
        );
    });

    e.teclar(cx, "ctrl-a");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.marcadas(), vec!["a".to_string(), "b".to_string()]);
    });
    e.teclar(cx, "cmd-d");
    e.detalhe(cx, |tela, _w, cx| {
        assert_eq!(tela.quantas_marcadas(), 0, "⌘D desmarca");
        tela.filtrar(Filtro::SemNota, cx);
        assert_eq!(tela.total_visivel(), 2, "as locais não têm nota");
        tela.filtrar(Filtro::Classificadas, cx);
        assert_eq!(tela.total_visivel(), 4);
        tela.filtrar(Filtro::Situacao(Estado::Comprada), cx);
        assert_eq!(tela.total_visivel(), 1);
        tela.filtrar(Filtro::Todas, cx);
    });

    // As setas andam na grade da sessão, sem dar a volta.
    e.teclar(cx, "right");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.posicao_em_foco(), Some(0))
    });
    e.teclar(cx, "right right");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.posicao_em_foco(), Some(2))
    });
    e.teclar(cx, "left left left left");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.posicao_em_foco(), Some(0))
    });
    // Numa janela estreita a grade tem mais de uma linha, e ↓ desce uma
    // linha inteira — o passo é o número de colunas que cabem.
    cx.simulate_window_resize(e.raiz.into(), gpui::size(gpui::px(760.), gpui::px(700.)));
    cx.run_until_parked();
    let colunas = e.detalhe(cx, |tela, window, _cx| tela.colunas_visiveis(window));
    assert!(
        colunas > 0 && colunas < 6,
        "a janela estreita quebra a grade: {colunas}"
    );
    e.teclar(cx, "down");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.posicao_em_foco(),
            Some(colunas),
            "↓ desce uma linha inteira"
        );
    });
    e.teclar(cx, "up");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.posicao_em_foco(), Some(0))
    });

    // O zoom da grade: aumenta, diminui e para nos limites.
    e.detalhe(cx, |tela, window, cx| {
        let antes = tela.zoom();
        tela.ajustar_zoom(40., window, cx);
        assert!(tela.zoom() > antes);
        for _ in 0..50 {
            tela.ajustar_zoom(40., window, cx);
        }
        let teto = tela.zoom();
        tela.ajustar_zoom(40., window, cx);
        assert_eq!(tela.zoom(), teto, "o zoom tem teto");
        for _ in 0..50 {
            tela.ajustar_zoom(-40., window, cx);
        }
        assert!(tela.zoom() < antes, "e desce até o piso");
    });
}

/// 🎬 **Negociar e imprimir as marcadas**: os botões da barra levam a seleção
/// da grade ao balcão e à folha — com o id do site atravessando para a
/// Biblioteca, que é quem o balcão lê.
#[gpui::test]
fn negociar_e_imprimir_as_marcadas(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    e.detalhe(cx, |tela, _w, cx| {
        tela.marcar_ids(&["a".into(), "d".into()], cx);
        let marcadas = tela.marcadas();
        cx.emit(crate::sessoes::detalhe::Pedido::Negociar(marcadas));
    });
    e.app(cx, |app, _w, cx| {
        assert!(app.no_balcao(), "o balcão abriu");
        let negociaveis: Vec<String> = app
            .balcao
            .read(cx)
            .negociaveis()
            .iter()
            .filter_map(|f| f.pos_venda_foto_id.clone())
            .collect();
        assert_eq!(negociaveis, vec!["a".to_string(), "d".to_string()]);
        app.balcao.update(cx, |tela, cx| tela.registrar(cx));
    });
    e.esperar(cx);
    let negociadas = e.site.negociadas();
    assert_eq!(
        negociadas
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "d"],
        "o acerto foi para as duas marcadas"
    );
    assert!(negociadas
        .iter()
        .all(|(_, m)| m.observacao_da_negociacao.is_some()));
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.balcao.read(cx).gravadas(), 2);
        app.fechar_balcao(cx);
        assert!(!app.no_balcao());
    });

    // 🖨️ Imprimir as mesmas.
    e.detalhe(cx, |tela, _w, cx| {
        let marcadas = tela.marcadas();
        cx.emit(crate::sessoes::detalhe::Pedido::Imprimir(marcadas));
    });
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::Impressao);
        let folha = app.impressao_para_teste();
        assert_eq!(folha.read(cx).escolhidas(), 2);
        folha.update(cx, |tela, cx| tela.imprimir(cx));
    });
    e.esperar(cx);
    let pedidos = e.folha.pedidos();
    assert_eq!(pedidos.len(), 1);
    assert_eq!(
        pedidos[0].0,
        vec!["site:a".to_string(), "site:d".to_string()]
    );
    assert_eq!(pedidos[0].2, Destino::Impressora);

    // Esc sai da folha de volta para a galeria, com a sessão aberta.
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessao);
        assert!(app.pode_trabalhar());
    });
}

/// 🎬 **Exportar**: o botão da barra abre o modal com a grade, a pasta vem do
/// seletor, e o lote vai para o exportador — nada de pasta real.
#[gpui::test]
fn exportar_pelo_botao_da_barra(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    let pasta = tempfile::TempDir::new().expect("pasta de destino");

    e.detalhe(cx, |tela, _w, cx| {
        tela.marcar_ids(&["id-DSC_101.jpg".into()], cx)
    });
    clicar(&e, cx, "detalhe-exportar");
    e.app(cx, |app, _w, cx| {
        assert!(app.exportando(), "o modal abriu");
        let modal = app.exportacao_para_teste();
        assert!(modal.read(cx).quantas() >= 1);
        let destino = pasta.path().to_path_buf();
        modal.update(cx, |tela, cx| {
            tela.escolher_pasta_para_teste(destino, cx);
            tela.exportar(cx);
        });
    });
    e.esperar(cx);
    let pedidos = e.exportador.pedidos();
    assert_eq!(pedidos.len(), 1, "um lote");
    assert!(pedidos[0]
        .iter()
        .all(|s| s.destino.starts_with(pasta.path())));
    e.app(cx, |app, window, cx| {
        app.fechar_exportacao(window, cx);
        assert!(!app.exportando());
    });
}

/// 🎬 **O fim da sessão**: editar os dados do cliente, copiar o link (que vai
/// para a área de transferência) e avisar o cliente.
#[gpui::test]
fn dados_do_cliente_link_e_aviso(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    e.detalhe(cx, |tela, window, cx| {
        assert!(tela.tem_email());
        tela.editar_dados_do_cliente(cx);
        tela.digitar_dados_do_cliente(Some("Ensaio da Ana e do Rui"), None, None, window, cx);
        tela.gravar_dados_do_cliente(cx);
    });
    e.esperar(cx);
    assert_eq!(
        e.site.atualizacoes(),
        vec![(
            GALERIA.to_string(),
            MudancaDaGaleria {
                titulo: Some("Ensaio da Ana e do Rui".into()),
                ..Default::default()
            }
        )],
        "o PATCH leva só o que mudou"
    );
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.aberta().map(|a| a.galeria.titulo.clone()).as_deref(),
            Some("Ensaio da Ana e do Rui")
        );
    });

    // 🔗 Copiar link.
    e.detalhe(cx, |tela, _w, cx| tela.pedir_o_link(cx));
    e.esperar(cx);
    assert_eq!(e.site.links(), vec![GALERIA.to_string()]);
    let url = e.detalhe(cx, |tela, _w, _cx| tela.link().map(|l| l.url.clone()));
    let url = url.expect("o link chegou");
    assert_eq!(
        cx.read_from_clipboard().and_then(|c| c.text()),
        Some(url),
        "o link foi para a área de transferência"
    );
    // 🚨 **E ele não expira** (dono, 2026-09-20): o site responde
    // `validade_em_segundos: null`, e eram 7 dias até então. O que este degrau
    // fixa é a tela **atravessar** o `null` — a forma de falhar era a resposta
    // inteira ser recusada, com o link já assinado do outro lado.
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.link().and_then(|l| l.validade_em_segundos),
            None,
            "o link da galeria não expira"
        );
    });

    // 📧 Avisar o cliente.
    e.detalhe(cx, |tela, _w, cx| tela.avisar(cx));
    e.esperar(cx);
    assert_eq!(e.site.avisadas(), vec![GALERIA.to_string()]);

    // ← Voltar: a sessão fecha e a lista volta.
    e.detalhe(cx, |_tela, _w, cx| {
        cx.emit(crate::sessoes::detalhe::Pedido::Voltar)
    });
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessoes);
        assert!(!app.pode_trabalhar());
    });
}

/// 🎬 **A sessão sem e-mail**: o link pede o contato, grava o e-mail e segue o
/// gesto — sem um segundo clique.
#[gpui::test]
fn o_link_de_uma_sessao_sem_email_pede_o_contato(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.app(cx, |app, _w, cx| app.entrar_na_sessao("g2".into(), cx));
    e.esperar(cx);

    e.detalhe(cx, |tela, window, cx| {
        assert_eq!(tela.galeria_id(), Some("g2"));
        assert!(!tela.tem_email());
        tela.pedir_o_link(cx);
        assert!(tela.motivo_do_formulario().is_some(), "o contato é pedido");
        tela.digitar_dados_do_cliente(None, Some("joao@exemplo.com"), None, window, cx);
        tela.gravar_dados_do_cliente(cx);
    });
    e.esperar(cx);
    assert_eq!(e.site.links(), vec!["g2".to_string()], "o gesto seguiu");
    e.detalhe(cx, |tela, _w, _cx| {
        assert!(tela.link().is_some());
        assert!(tela.motivo_do_formulario().is_none());
    });
}

/// 🎬 **`Esc` na galeria não mexe na grade.** O recorte e a seleção que o
/// operador escolheu ficam — o `Esc` é da Revelação e das ferramentas dela.
#[gpui::test]
fn esc_na_galeria_nao_troca_o_recorte_nem_a_selecao(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    // A tira da Revelação tem outro recorte e outra seleção.
    e.revelar_a_do_site(cx, "a");
    e.teclar(cx, "escape");
    e.detalhe(cx, |tela, _w, cx| {
        tela.filtrar(Filtro::Situacao(Estado::Comprada), cx);
        tela.marcar_ids(&["c".into()], cx);
    });

    e.teclar(cx, "escape");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.filtro(),
            Filtro::Situacao(Estado::Comprada),
            "o recorte ficou"
        );
        assert_eq!(tela.marcadas(), vec!["c".to_string()], "a seleção ficou");
    });
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessao);
        assert!(app.pode_trabalhar());
    });
}
