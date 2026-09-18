//! 📦 O que a Revelação faz com mais de uma foto, e os dois botões do fim:
//! "Baixar JPEG" e "Salvar na galeria e sair".

use domain::services::PreviewType;
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
    // 🔑 **O sucesso é toast, não a faixa vermelha** (dono, 18/set/2026): a
    // faixa é onde mora erro, e um sucesso pintado de falha ensina o operador a
    // desconfiar do que deu certo.
    e.app(cx, |app, _w, _cx| {
        let avisos = app.avisos_para_teste();
        assert_eq!(
            avisos.last().map(|(t, _)| t.as_str()),
            Some("revelação salva na galeria")
        );
        assert!(
            avisos.iter().all(|(_, erro)| !erro),
            "nenhum deles é falha: {avisos:?}"
        );
    });
    e.detalhe(cx, |tela, _w, _cx| {
        assert!(tela.erro().is_none(), "e a faixa de erro fica limpa");
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

/// 🎬 **O botão "Salvar na galeria e sair", estado por estado** — o que o
/// operador lê nele em cada momento, comparado com o editor da web.
///
/// 🚨 **Nasceu de "tem momentos que ele falta"** (dono, 18/set/2026). Eram duas
/// coisas: o toast do `gpui-component` nasce no canto superior **direito** e
/// cobria os três últimos botões da barra por quatro segundos (agora os avisos
/// vão no topo **ao centro**, como o `sonner` do site), e o "Baixar JPEG" em
/// curso não desligava o "Salvar" — na web o `ocupado` desliga os dois.
///
/// Os quatro estados aqui são os quatro `title` do `<Button>` da web:
/// nada a salvar · esta foto · esta e mais N · a comprada e as N atrás dela.
#[gpui::test]
fn o_botao_de_salvar_conta_o_que_ha_para_salvar(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            site: Box::new(|site| site.demorada = true),
            ..Cenario::default()
        },
    );
    e.revelar_a_do_site(cx, "a");

    // 1 · Recém-aberta, sem tocar em nada: apagado, e a dica diz por quê.
    e.revelacao(cx, |tela, _w, _cx| {
        let botao = tela.botao_de_salvar();
        assert!(!botao.habilitado, "nada mudou: nada a salvar");
        assert_eq!(botao.rotulo, "Salvar na galeria e sair");
        assert_eq!(
            botao.dica,
            "Nada a salvar: o que está no canvas já está na galeria"
        );
    });

    // 2 · Um slider acende o botão, como o `sujo` do site.
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.4, cx));
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, _cx| {
        let botao = tela.botao_de_salvar();
        assert!(botao.habilitado, "a foto aberta tem receita nova");
        assert_eq!(botao.dica, "Salva esta foto na galeria e fecha o editor");
    });

    // 3 · Enquanto o "Baixar JPEG" trabalha, ele fica quieto — o `ocupado`.
    e.revelacao(cx, |tela, _w, cx| tela.definir_gerando_jpeg(true, cx));
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(
            !tela.botao_de_salvar().habilitado,
            "um JPEG por vez: os dois pedem o bruto em resolução cheia"
        );
    });
    e.revelacao(cx, |tela, _w, cx| tela.definir_gerando_jpeg(false, cx));

    // 4 · O "Sincronizar" deixa outra pendente: a dica passa a contá-la.
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
    e.revelacao(cx, |tela, _w, _cx| {
        let botao = tela.botao_de_salvar();
        assert!(botao.habilitado);
        assert_eq!(
            botao.dica, "Salva esta e mais 1 com edição pendente, e fecha o editor",
            "a dica conta as que o Sincronizar deixou"
        );
    });

    // 5 · Na comprada, que não se revela, o botão continua aceso pelas outras —
    // e a dica troca de frase (o `podeRevelar` do site).
    e.revelacao(cx, |tela, window, cx| {
        let c = tela
            .acervo()
            .iter()
            .position(|f| f.id == "site:c")
            .expect("a comprada na tira");
        tela.ir_para(c, window, cx);
    });
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(!tela.pode_revelar(), "site:c é a comprada");
        let botao = tela.botao_de_salvar();
        assert!(botao.habilitado, "as pendentes atrás dela ainda sobem");
        assert!(
            botao.dica.starts_with("Esta foi comprada e não se revela"),
            "a dica diz por que a do canvas fica de fora: {}",
            botao.dica
        );
    });

    // 6 · O lote subindo: o rótulo conta, e o botão não aceita um segundo clique.
    botao(&e, cx, PedidoDaRevelacao::SalvarNaGaleria);
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, _cx| {
        let botao = tela.botao_de_salvar();
        assert!(!botao.habilitado, "o lote já está no ar");
        assert_eq!(
            botao.rotulo, "Salvando 1/2…",
            "a comprada fica de fora, mas as duas pendentes sobem"
        );
    });
    e.site.responder();
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessao, "o lote terminou: volta à sessão");
    });
}

/// 🎬 **O lote de duas ou mais conta no rótulo**, uma resposta de cada vez —
/// é o `Salvando 1/2…` do site, e o que diz ao operador que a espera anda.
#[gpui::test]
fn o_rotulo_do_salvar_conta_o_lote(cx: &mut TestAppContext) {
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

    botao(&e, cx, PedidoDaRevelacao::SalvarNaGaleria);
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.botao_de_salvar().rotulo, "Salvando 1/2…");
    });
    e.site.responder_uma_revelacao();
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.botao_de_salvar().rotulo,
            "Salvando 2/2…",
            "a primeira respondeu; falta uma"
        );
    });
    e.site.responder();
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessao));
}

/// 🎬 **O aviso não pode cobrir o botão** — a razão de os toasts terem saído do
/// canto direito.
///
/// O que o teste alcança é o **lugar** deles: a lista de avisos da raiz, que o
/// `render` desenha no topo ao centro. Antes disso era o `push_notification` do
/// `gpui-component`, cuja lista mora fixa em `top_4().right_4()` — por cima de
/// "Tela do cliente", "Baixar JPEG" e "Salvar na galeria e sair".
#[gpui::test]
fn o_aviso_de_sucesso_fica_no_topo_e_some_sozinho(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.4, cx));
    e.esperar(cx);

    botao(&e, cx, PedidoDaRevelacao::SalvarNaGaleria);
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        let avisos = app.avisos_para_teste();
        assert!(
            avisos.iter().any(|(t, erro)| t.contains("salva") && !erro),
            "o sucesso aparece como toast verde: {avisos:?}"
        );
        assert!(
            avisos.len() <= 3,
            "e nunca mais do que cabe na tela: {avisos:?}"
        );
    });

    // ⏱️ Passados os quatro segundos do `sonner`, a tela volta limpa — e os
    // botões que ficam por baixo do aviso voltam a aparecer.
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(5));
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        assert!(
            app.avisos_para_teste().is_empty(),
            "o aviso some sozinho: {:?}",
            app.avisos_para_teste()
        );
    });
}

/// 🎬 **A foto tem uma cara só** — a da grade e a da Revelação.
///
/// 🚨 **Nasceu de "na galeria estava com um efeito, dei dois cliques e a
/// revelação estava com outro"** (dono, 18/set/2026). A grade desenha o que o
/// **servidor** tem, e o servidor só muda quando alguém salva na galeria; a
/// Revelação abre com a receita do **banco local**, que muda a cada gesto. Entre
/// um e outro, a mesma foto tinha duas caras — e quem vê as duas conclui,
/// corretamente, que uma delas está mentindo.
///
/// A web já pagou esse defeito (`usar-previas-reveladas.ts`, 11/set/2026) e
/// resolveu do mesmo jeito: uma prévia revelada **local**, que a grade consulta
/// antes da URL do servidor e que é apagada quando a foto sobe — porque daí em
/// diante quem é mais novo é o site.
#[gpui::test]
fn a_previa_local_revelada_nasce_ao_sair_e_morre_quando_a_foto_sobe(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            site: Box::new(|site| site.demorada = true),
            ..Cenario::default()
        },
    );
    let chave = crate::revelacao::persistencia::chave_da_revelada("site:a");
    assert!(
        !e.previews.tem(&chave, PreviewType::Thumbnail),
        "sem revelação, a grade mostra a foto do site"
    );

    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.6, cx));
    e.esperar(cx);
    // Sair guarda a prévia da foto aberta — é o que a grade vai mostrar.
    botao(&e, cx, PedidoDaRevelacao::Sair);
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessao));
    assert!(
        e.previews.tem(&chave, PreviewType::Thumbnail)
            && e.previews.tem(&chave, PreviewType::Large),
        "as duas chaves: a grade lê a miniatura, o painel lê o preview"
    );

    // E ela morre quando o site recebe a revelação.
    e.revelar_a_do_site(cx, "a");
    botao(&e, cx, PedidoDaRevelacao::SalvarNaGaleria);
    e.esperar(cx);
    e.site.responder();
    e.esperar(cx);
    assert!(
        !e.previews.tem(&chave, PreviewType::Thumbnail),
        "subiu: quem manda passa a ser o servidor"
    );
}

/// 🎬 **"Sincronizar N" muda a tira na hora** — e não só o banco.
///
/// 🚨 **Dono, 18/set/2026**: *"o botão de sincronizar não está com o mesmo
/// comportamento da WEB e não atualiza o filmstrip da revelação, dá a sensação
/// de que não aconteceu nada"*. Desde 11/set o "Sincronizar" copia **só a
/// receita**: o JPEG do servidor continua o de antes, então a única coisa capaz
/// de mostrar o efeito antes de salvar é a prévia local — que é o que o serviço
/// da receita padrão passou a gerar para cada foto sincronizada.
///
/// ⚠️ **O que este cenário alcança é o pedido**, e não o pixel: a thread do
/// serviço roda com `motor = None` em teste (nenhum cenário abre GPU), e quem
/// prova o cache é `a_receita_pronta_so_faz_o_cache`, em `receita_padrao.rs`.
#[gpui::test]
fn sincronizar_pede_a_previa_local_das_marcadas(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.8, cx));
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.receita_padrao_pedida(), 0, "nada pedido ainda");
    });

    e.teclar(cx, "cmd-a");
    botao(&e, cx, PedidoDaRevelacao::Sincronizar);
    e.esperar(cx);

    e.app(cx, |app, _w, _cx| {
        assert_eq!(
            app.receita_padrao_pedida(),
            4,
            "as quatro que receberam a receita (a comprada e a aberta ficam de fora)"
        );
    });
}

/// 🎬 **"Zerar N fotos" com a tira marcada** — o que o botão promete, e o que
/// a tela mostra depois.
///
/// 🚨 **Dono, 18/set/2026**: *"o 'Zerar tudo' precisa fazer exatamente como
/// acontece na WEB: quando selecionadas as fotos no filmstrip, precisa zerar as
/// selecionadas e atualizar as miniaturas"*. A primeira metade já valia — a
/// segunda não: o banco voltava ao neutro e a miniatura continuava mostrando a
/// receita que acabara de ser desfeita, porque a prévia local (a mesma que faz
/// o "Sincronizar" aparecer) ficava para trás.
///
/// O cenário estressa as seis coisas que o gesto toca de uma vez:
/// o rótulo do botão, quem entra no lote, o enquadramento, o histórico da
/// aberta, a prévia local de cada uma e a fila de envio.
#[gpui::test]
fn zerar_as_marcadas_limpa_receita_enquadramento_e_previas(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.9, cx));
    e.esperar(cx);
    // Um enquadramento na aberta: o "Zerar" promete levá-lo junto.
    e.teclar(cx, "r ] enter");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.enquadramento().rotation_90(), 1)
    });

    // A tira inteira marcada, e a receita copiada para ela.
    e.teclar(cx, "cmd-a");
    botao(&e, cx, PedidoDaRevelacao::Sincronizar);
    e.esperar(cx);

    // As prévias locais existem — é o estado de quem acabou de sincronizar.
    for id in ["site:a", "site:b", "site:d"] {
        e.previews
            .save_thumbnail(
                &crate::revelacao::persistencia::chave_da_revelada(id),
                &image::DynamicImage::ImageRgb8(image::RgbImage::new(8, 8)),
            )
            .expect("gravar a prévia local");
    }

    // O botão conta esta **e** as marcadas — o `quantasFotos` do site.
    e.revelacao(cx, |tela, _w, _cx| {
        assert!(
            tela.outras_a_zerar().len() >= 3,
            "as marcadas com receita entram no lote"
        );
        assert!(
            !tela.outras_a_zerar().iter().any(|f| f.id == "site:c"),
            "a comprada não se zera"
        );
    });

    e.revelacao(cx, |tela, window, cx| tela.clicar_em_zerar_tudo(window, cx));
    e.esperar(cx);

    // 1 · A aberta voltou ao neutro, com enquadramento e tudo — e dá `⌘Z`.
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.ajustes().exposure, 0.0);
        assert_eq!(tela.enquadramento().rotation_90(), 0);
        assert!(tela.pode_desfazer(), "esta volta pelo ⌘Z");
        assert!(
            tela.outras_a_zerar().is_empty(),
            "e as outras também zeraram"
        );
    });

    // 2 · O banco recebeu o neutro de cada uma.
    let zeradas: Vec<String> = e
        .gravador
        .gravado()
        .into_iter()
        .filter(|(_, ajustes, corte)| {
            *ajustes == crate::revelacao::processador::Ajustes::default()
                && corte.largura == Some(1.0)
        })
        .map(|(id, _, _)| id)
        .collect();
    for id in ["site:a", "site:b", "site:d"] {
        assert!(zeradas.contains(&id.to_string()), "{id} voltou ao neutro");
    }
    assert!(
        !zeradas.contains(&"site:c".to_string()),
        "a comprada ficou de fora do banco também"
    );

    // 3 · 🚨 E as miniaturas: sem isto o gesto muda o banco e não muda a tela.
    for id in ["site:a", "site:b", "site:d"] {
        assert!(
            !e.previews.tem(
                &crate::revelacao::persistencia::chave_da_revelada(id),
                PreviewType::Thumbnail
            ),
            "{id} ainda mostra a receita desfeita"
        );
    }

    // 4 · O neutro também precisa subir — e sobe pelos dois caminhos: as
    // marcadas pela fila da raiz, a aberta pelo depósito do gravador (foi a
    // própria tela que a zerou, pelo histórico).
    e.app(cx, |app, _w, _cx| {
        let mut fila = app.a_subir_para_teste();
        fila.sort();
        assert_eq!(fila, vec!["b", "d"], "as marcadas");
    });
    assert!(
        e.gravador.deposito().iter().any(|(id, _)| id == "a"),
        "e a aberta, pelo depósito: o Salvar leva as duas listas"
    );
}

/// 🎬 **A foto do palco também chega atualizada à grade** depois do
/// "Sincronizar".
///
/// 🚨 **Dono, 18/set/2026**: *"quando eu mando sincronizar os efeitos na
/// revelação e volto para a galeria, a primeira foto não é atualizada"*. Ela é
/// a única do lote que o "Sincronizar" **pula** — já está com a receita, foi
/// dela que ela saiu —, então ninguém avisava a grade a respeito dela. A prévia
/// local é gravada ao sair do editor; o que faltava era o recado.
#[gpui::test]
fn ao_sair_da_revelacao_a_grade_relê_a_foto_que_estava_no_palco(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.7, cx));
    e.esperar(cx);

    botao(&e, cx, PedidoDaRevelacao::Sair);
    e.esperar(cx);

    e.detalhe(cx, |tela, _w, _cx| {
        assert!(
            tela.reveladas_avisadas().iter().any(|id| id == "a"),
            "a grade não soube que a foto do palco mudou: {:?}",
            tela.reveladas_avisadas()
        );
    });
}
