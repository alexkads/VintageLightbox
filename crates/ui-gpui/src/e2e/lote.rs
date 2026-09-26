//! 📦 O que a Revelação faz com mais de uma foto, e os dois botões do fim:
//! "Baixar JPEG" e "Salvar na galeria e sair".

use domain::services::PreviewType;
use gpui_kit::TestAppContext;

use super::{abrir_o_ensaio, Cenario};
use crate::app::Tela;
use crate::revelacao::persistencia;
use crate::revelacao::tela::PedidoDaRevelacao;

/// O botão da barra da Revelação que emite `pedido`.
fn botao(e: &super::Estudio, cx: &mut TestAppContext, pedido: PedidoDaRevelacao) {
    e.revelacao(cx, move |_tela, _w, cx| cx.emit(pedido));
}

/// 🎬 **Sincronizar N e zerar N**: `⌘A` marca a tira, a caixa do
/// "Sincronizar" confirma pelo Enter, a receita vai para as marcadas (menos a
/// comprada) sem subir nada, e o "Zerar N fotos" devolve todas ao neutro —
/// inclusive o enquadramento.
#[gpui_kit::test]
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
#[gpui_kit::test]
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
#[gpui_kit::test]
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
#[gpui_kit::test]
fn salvar_na_galeria_sobe_o_lote_e_sai(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            site: Box::new(|site| site.demorada = true),
            ..Cenario::default()
        },
    );
    // 🚨 **A subida do ensaio (C20) divide a esteira com o "Salvar".** Com
    // `demorada`, as duas locais que sobem ao abrir a sessão ocupam vagas das
    // três em voo, e as revelações esperariam atrás delas — que é o
    // comportamento certo, e não o que este cenário mede. A rede responde
    // primeiro, e a esteira fica livre para o lote.
    e.site.responder();
    e.esperar(cx);

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
    // 🚨 **A tela sai na hora, e o lote sobe atrás** (dono, 18/set/2026: *"não
    // pode travar o fluxo, devendo continuar na tela de sessão de fotos"*).
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessao, "o editor não segura o operador");
        let avisos = app.avisos_dados_para_teste();
        assert!(
            avisos
                .iter()
                .any(|(t, _)| t.contains("pode continuar com o cliente")),
            "e o aviso diz o que está acontecendo: {avisos:?}"
        );
    });

    // O site responde uma de cada vez; o lote continua no ar até a última.
    e.site.responder_uma();
    e.esperar(cx);
    e.site.responder();
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessao);
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
        let avisos = app.avisos_dados_para_teste();
        assert_eq!(
            avisos.last().map(|(t, _)| t.as_str()),
            Some("2 revelações salvas na galeria"),
            "uma frase por lote, e não uma por foto: {avisos:?}"
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

/// 🎬 **O site recusa o Salvar**: a recusa vai para o canto dos envios, e a
/// receita continua no depósito para tentar de novo.
///
/// 🔑 **A tela sai assim mesmo**, porque o lote sobe em segundo plano: segurar
/// o editor esperando uma resposta que pode demorar segundos é o que o dono
/// pediu para acabar (18/set/2026). O que protege o trabalho não é a tela
/// parada — é a receita continuar aqui.
#[gpui_kit::test]
fn salvar_na_galeria_com_falha_deixa_a_receita_no_deposito(cx: &mut TestAppContext) {
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
        assert_eq!(app.tela(), Tela::Sessao, "o lote sobe em segundo plano");
        assert_eq!(
            app.recusas_para_teste(),
            ["a.jpg: o site respondeu 410: foto apagada"],
            "a recusa vai para o canto dos envios, com o NOME do arquivo — um \
             id não diz ao operador qual foto ficou para trás"
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
    // 🚨 **Três tentativas antes de desistir** (dono, 18/set/2026: *"essa
    // rotina precisa ser um tanque de guerra!"*). Uma recusa de verdade — um
    // 410 — não muda com a repetição, e é por isso que ela acaba no canto; o
    // que a repetição salva é o 502 de momento, que passa na segunda.
    assert_eq!(
        e.site.reveladas().len(),
        crate::envios::TENTATIVAS as usize,
        "o site tinha de ter sido tentado três vezes antes da recusa"
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
#[gpui_kit::test]
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

    // 6 · O clique sai da revelação na hora — o lote sobe atrás.
    botao(&e, cx, PedidoDaRevelacao::SalvarNaGaleria);
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessao, "o editor não segura o operador");
    });
    e.site.responder();
    e.esperar(cx);
}

/// 🎬 **O lote de duas ou mais avisa uma vez só, no fim.**
///
/// 🚨 **Era um rótulo no botão** (`Salvando 1/2…`), e ele morreu com o pedido
/// de 18/set/2026: o editor fecha na hora, então não há botão onde contar. O
/// que resta é o aviso — e ele é **por lote**: com vinte fotos, vinte toasts
/// seriam vinte interrupções para quem já está com o próximo cliente.
#[gpui_kit::test]
fn o_lote_avisa_uma_vez_so_quando_termina(cx: &mut TestAppContext) {
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

    // A primeira resposta **não** avisa: o lote ainda está no ar.
    e.site.responder_uma_revelacao();
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        let avisos = app.avisos_dados_para_teste();
        assert!(
            !avisos.iter().any(|(t, _)| t.contains("salvas na galeria")),
            "meio do lote não anuncia fim: {avisos:?}"
        );
    });

    e.site.responder();
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| {
        let avisos = app.avisos_dados_para_teste();
        assert_eq!(
            avisos.last().map(|(t, _)| t.as_str()),
            Some("2 revelações salvas na galeria")
        );
    });
}

/// 🎬 **O aviso não pode cobrir o botão** — a razão de os toasts terem saído do
/// canto direito.
///
/// O que o teste alcança é o **lugar** deles: a lista de avisos da raiz, que o
/// `render` desenha no topo ao centro. Antes disso era o `push_notification` do
/// `gpui-component`, cuja lista mora fixa em `top_4().right_4()` — por cima de
/// "Tela do cliente", "Baixar JPEG" e "Salvar na galeria e sair".
#[gpui_kit::test]
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
            avisos
                .iter()
                .any(|(t, erro)| t.contains("segundo plano") && !erro),
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
#[gpui_kit::test]
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
#[gpui_kit::test]
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
#[gpui_kit::test]
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
    // O serviço das prévias anda numa thread de verdade, com relógio de
    // verdade: a `d` espera a cópia de trabalho chegar do storage antes de
    // virar o neutro.
    for _ in 0..60 {
        let andando = e.app(cx, |app, _w, _cx| app.previas_andando());
        if !andando {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        e.esperar(cx);
    }

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
    //
    // 🔄 **A da foto do site vira o neutro, e não some** (dono, 2026-09-21).
    // Apagar a prévia fazia a tira voltar à imagem da galeria — que ainda tem a
    // receita desfeita até o "Salvar". O que se cobra é que a prévia da receita
    // (a 8×8 gravada acima) não esteja mais lá.
    for id in ["site:a", "site:b", "site:d"] {
        let chave = crate::revelacao::persistencia::chave_da_revelada(id);
        let velha = e
            .previews
            .get_thumbnail(&chave)
            .is_some_and(|m| (m.width(), m.height()) == (8, 8));
        assert!(!velha, "{id} ainda mostra a receita desfeita");
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
#[gpui_kit::test]
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

/// A rede responde as subidas do ensaio que estavam presas, e o site fica como
/// o `subir` de verdade o deixa: a linha da foto existe lá, **com a receita que
/// foi junto**, e o id remoto voltou ao catálogo.
///
/// Devolve `(id local, receita que subiu)` de cada uma.
fn a_rede_responde_a_subida(e: &super::Estudio) -> Vec<(String, Option<String>)> {
    use crate::pos_venda::porta::Recado;
    let subiram: Vec<(String, Option<String>)> = e
        .site
        .guardados
        .lock()
        .unwrap()
        .iter()
        .filter_map(|(_, recado)| match recado {
            Recado::ClassificadaSubiu { foto_id, receita } => {
                Some((foto_id.clone(), receita.clone()))
            }
            _ => None,
        })
        .collect();
    for (i, (foto_id, receita)) in subiram.iter().cloned().enumerate() {
        let no_site = format!("site-{foto_id}");
        if let Some(foto) = e
            .acervo
            .fotos
            .lock()
            .unwrap()
            .iter_mut()
            .find(|f| f.id == foto_id)
        {
            foto.pos_venda_foto_id = Some(no_site.clone());
        }
        let mut linha = super::do_site(
            &no_site,
            10 + i as i32,
            domain::services::pos_venda::EstadoDaFotoNoSite::Disponivel,
            None,
        );
        linha.ajustes = receita.map(|r| serde_json::from_str(&r).expect("a receita é JSON"));
        linha.revelada = linha.ajustes.is_some();
        e.site.fotos_da_sessao.lock().unwrap().push(linha);
    }
    e.site.responder();
    subiram
}

/// A receita da foto aberta na Revelação, depois de levá-la até `id`.
fn reabrir(e: &super::Estudio, cx: &mut TestAppContext, id: &str) -> (String, f32, f32) {
    e.revelar_pela_barra(cx);
    let alvo = id.to_string();
    e.revelacao(cx, |tela, window, cx| {
        let posicao = tela
            .acervo()
            .iter()
            .position(|f| f.id == alvo)
            .unwrap_or_else(|| panic!("{alvo} não está na tira"));
        tela.ir_para(posicao, window, cx);
    });
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, _cx| {
        let aberta = tela.foto_aberta().map(|f| f.id.clone()).unwrap_or_default();
        let ajustes = tela.ajustes();
        (aberta, ajustes.exposure, ajustes.contrast)
    })
}

/// O ensaio com a receita padrão (contraste 1,25) começando a subir, com a
/// rede lenta: as duas locais saíram e nenhuma resposta voltou.
fn o_ensaio_subindo(cx: &mut TestAppContext) -> super::Estudio {
    let e = super::abrir_o_app(
        cx,
        Cenario {
            site: Box::new(|site| site.demorada = true),
            ..Cenario::default()
        },
    );
    e.entrar_na_conta(cx);
    let padrao = crate::revelacao::processador::Ajustes {
        contrast: 1.25,
        ..Default::default()
    };
    for foto in e.acervo.fotos.lock().unwrap().iter_mut() {
        persistencia::na_foto(foto, padrao, Default::default());
    }
    e.detalhe(cx, |_tela, _w, cx| {
        cx.emit(crate::sessoes::detalhe::Pedido::CatalogoMudou)
    });
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        app.sessoes
            .update(cx, |tela, cx| tela.abrir(super::GALERIA.into(), cx));
    });
    e.esperar(cx);
    let mut subindo: Vec<String> = e.site.subidas().into_iter().map(|s| s.1).collect();
    subindo.sort();
    assert_eq!(
        subindo,
        vec!["id-DSC_101.jpg", "id-DSC_102.jpg"],
        "o ensaio começou a subir, e a rede ainda não respondeu"
    );
    e
}

/// 🚨 **Revelar a foto que ainda está subindo, salvar, e a edição continuar
/// lá depois da barra verde** (dono, 24/set/2026: *"gravo a edição, aparece
/// uma barra verde e desfaz a minha edição para a primeira foto importada"*).
///
/// Reproduzido contra a pilha local antes de virar teste: 24 fotos importadas,
/// a exposição da quarta levada a −0,5 enquanto ela ainda estava "No disco",
/// "Salvar na galeria e sair". O catálogo ficou com −0,5, o servidor com 0 — a
/// subida tinha lido a foto antes do ajuste —, e quando ela terminou de subir
/// a foto passou a ser a do site: a Revelação a reabria com a receita da
/// importação.
///
/// O cenário prende a janela inteira, e não só a conta:
///
/// 1. o ensaio entra com a receita padrão (contraste 1,25) e começa a subir,
///    com a rede lenta — a subida lê cada foto **antes** do ajuste;
/// 2. a Revelação abre a **segunda** foto local, não a primeira, e o ajuste
///    chega ao catálogo;
/// 3. "Salvar na galeria e sair" volta para a sessão, com a foto ainda subindo;
/// 4. a rede responde (a barra verde): a foto vira a do site com a receita da
///    importação — e **a tela precisa continuar mostrando a do operador**;
/// 5. a receita que perdeu a partida sobe como revelação, só a desta foto;
/// 6. com tudo respondido, o depósito se esvazia e a verdade é o site — que
///    agora tem a edição.
///
/// E, em cada passo, o que não pode acontecer: a edição cair noutra foto, a
/// foto intocada subir de novo, e a tira duplicar a foto que mudou de casa.
#[gpui_kit::test]
fn revelar_a_foto_que_ainda_sobe_e_salvar_nao_desfaz_a_edicao(cx: &mut TestAppContext) {
    let e = o_ensaio_subindo(cx);

    // 2 · A segunda foto local, ainda no disco.
    let (aberta, _, contraste) = reabrir(&e, cx, "id-DSC_102.jpg");
    assert_eq!(aberta, "id-DSC_102.jpg");
    assert_eq!(contraste, 1.25, "abre com a receita padrão");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, -0.5, cx));
    e.esperar(cx);
    let no_catalogo = |e: &super::Estudio, id: &str| {
        let fotos = e.acervo.fotos.lock().unwrap();
        let foto = fotos.iter().find(|f| f.id == id).expect("no catálogo");
        persistencia::da_foto(foto)
    };
    assert_eq!(no_catalogo(&e, "id-DSC_102.jpg").exposure, -0.5);
    assert_eq!(
        no_catalogo(&e, "id-DSC_101.jpg").exposure,
        0.0,
        "a edição não caiu na primeira foto"
    );

    // 3 · Salvar e sair, com a foto ainda subindo.
    botao(&e, cx, PedidoDaRevelacao::SalvarNaGaleria);
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessao));
    assert!(
        e.site.reveladas().is_empty(),
        "a foto ainda não é do site: não há o que revelar lá"
    );

    // 4 · A rede responde: a subida chegou com a receita da importação.
    let subiram = a_rede_responde_a_subida(&e);
    let exposicao_que_subiu = |id: &str| {
        subiram
            .iter()
            .find(|(foto, _)| foto == id)
            .and_then(|(_, receita)| receita.as_deref())
            .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok())
            .and_then(|r| r["exposure"].as_f64())
    };
    assert_eq!(
        exposicao_que_subiu("id-DSC_102.jpg"),
        Some(0.0),
        "o cenário é o do defeito: a subida leu a foto antes do ajuste"
    );
    e.esperar(cx);
    e.esperar(cx);
    let linha = |e: &super::Estudio, id: &str| {
        e.site
            .fotos_da_sessao
            .lock()
            .unwrap()
            .iter()
            .find(|f| f.id == id)
            .and_then(|f| f.ajustes.clone())
    };
    let (aberta, exposicao, contraste) = reabrir(&e, cx, "site:site-id-DSC_102.jpg");
    assert_eq!(aberta, "site:site-id-DSC_102.jpg");
    assert_eq!(
        (exposicao, contraste),
        (-0.5, 1.25),
        "depois da barra verde a foto continua com a edição, e não com a receita da importação"
    );
    e.revelacao(cx, |tela, _w, _cx| {
        let ids: Vec<&str> = tela.acervo().iter().map(|f| f.id.as_str()).collect();
        let mut unicos = ids.clone();
        unicos.sort();
        unicos.dedup();
        assert_eq!(ids.len(), unicos.len(), "nenhuma foto em dobro: {ids:?}");
        assert!(
            !ids.contains(&"id-DSC_102.jpg"),
            "a local saiu da tira quando a do site entrou: {ids:?}"
        );
    });
    botao(&e, cx, PedidoDaRevelacao::Sair);
    e.esperar(cx);

    // 5 · Só a receita que perdeu a partida sobe — a da foto intocada, não.
    let reveladas: Vec<(String, f32, f32)> = e
        .site
        .reveladas()
        .into_iter()
        .map(|(id, ajustes, _)| (id, ajustes.exposure, ajustes.contrast))
        .collect();
    assert_eq!(
        reveladas,
        vec![("site-id-DSC_102.jpg".to_string(), -0.5, 1.25)],
        "a edição vai ao site, e a outra foto não sobe de novo"
    );

    // 6 · Tudo respondido: o depósito esvazia e o site é a verdade.
    e.site.responder();
    e.esperar(cx);
    e.esperar(cx);
    assert!(e.gravador.deposito().is_empty(), "o depósito esvaziou");
    e.app(cx, |app, _w, _cx| {
        assert!(app.a_subir_para_teste().is_empty(), "nada a salvar");
        assert!(app.recusas_para_teste().is_empty());
    });
    assert_eq!(
        linha(&e, "site-id-DSC_102.jpg").and_then(|a| a["exposure"].as_f64()),
        Some(-0.5),
        "o site tem a edição"
    );
    let (_, exposicao, contraste) = reabrir(&e, cx, "site:site-id-DSC_102.jpg");
    assert_eq!((exposicao, contraste), (-0.5, 1.25), "e reabre com ela");
    botao(&e, cx, PedidoDaRevelacao::Sair);
    e.esperar(cx);
    let (_, exposicao, _) = reabrir(&e, cx, "site:site-id-DSC_101.jpg");
    assert_eq!(
        exposicao, 0.0,
        "a primeira foto continua como foi importada"
    );
}

/// 🚨 **A foto termina de subir com a Revelação aberta nela, e o ajuste feito
/// depois vai ao site** — a segunda janela do mesmo defeito.
///
/// Achada rodando o app contra a pilha local depois do conserto da primeira:
/// a foto subiu às 39,07 s, o ajuste chegou às 40,49 s, com a tira ainda na
/// cópia local. A conciliação da subida já tinha passado (as duas receitas
/// eram iguais), e o ajuste caiu na linha local — que a grade não mostra mais
/// e o "Salvar" não levava. O servidor ficou com a receita da importação.
#[gpui_kit::test]
fn a_foto_termina_de_subir_com_a_revelacao_aberta_e_o_ajuste_vai_ao_site(cx: &mut TestAppContext) {
    let e = o_ensaio_subindo(cx);
    let (aberta, _, _) = reabrir(&e, cx, "id-DSC_102.jpg");
    assert_eq!(aberta, "id-DSC_102.jpg");
    // 🚨 A receita da **outra** foto muda no catálogo depois de a tira abrir —
    // a receita padrão que chega atrasada. A cópia dela na tira fica velha, e
    // ninguém a tocou na Revelação: o site tem de terminar com a do catálogo.
    {
        let mut fotos = e.acervo.fotos.lock().unwrap();
        let outra = fotos
            .iter_mut()
            .find(|f| f.id == "id-DSC_101.jpg")
            .expect("no catálogo");
        let ajustes = crate::revelacao::processador::Ajustes {
            contrast: 1.4,
            ..Default::default()
        };
        persistencia::na_foto(outra, ajustes, Default::default());
    }

    // A barra verde com o operador ainda na foto: ela subiu sem ajuste nenhum.
    a_rede_responde_a_subida(&e);
    e.esperar(cx);
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        assert_eq!(
            app.tela(),
            Tela::Revelacao,
            "a subida não tira ninguém do editor"
        );
        assert_eq!(
            app.revelacao.read(cx).foto_aberta().map(|f| f.id.clone()),
            Some("id-DSC_102.jpg".to_string()),
            "e não troca a foto aberta no meio do trabalho"
        );
    });
    let so_a_102 = |e: &super::Estudio| -> Vec<(String, f32, f32)> {
        e.site
            .reveladas()
            .into_iter()
            .filter(|(id, _, _)| id == "site-id-DSC_102.jpg")
            .map(|(id, ajustes, _)| (id, ajustes.exposure, ajustes.contrast))
            .collect()
    };
    assert!(so_a_102(&e).is_empty(), "nada mudou nela: nada a revelar");

    // O ajuste chega depois, na cópia local que continua aberta.
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, -0.5, cx));
    e.esperar(cx);
    botao(&e, cx, PedidoDaRevelacao::SalvarNaGaleria);
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessao));
    assert_eq!(
        so_a_102(&e),
        vec![("site-id-DSC_102.jpg".to_string(), -0.5, 1.25)],
        "o Salvar leva o ajuste à foto do site"
    );
    let da_101: Vec<f32> = e
        .site
        .reveladas()
        .into_iter()
        .filter(|(id, _, _)| id == "site-id-DSC_101.jpg")
        .map(|(_, ajustes, _)| ajustes.contrast)
        .collect();
    assert_eq!(
        da_101,
        vec![1.4],
        "a outra sobe uma vez, com a receita do catálogo, e nunca com a cópia velha da tira"
    );
    let (_, exposicao, contraste) = reabrir(&e, cx, "site:site-id-DSC_102.jpg");
    assert_eq!(
        (exposicao, contraste),
        (-0.5, 1.25),
        "a foto reabre com o ajuste enquanto a revelação sobe"
    );
    botao(&e, cx, PedidoDaRevelacao::Sair);
    e.esperar(cx);

    e.site.responder();
    e.esperar(cx);
    e.esperar(cx);
    assert!(e.gravador.deposito().is_empty(), "o depósito esvaziou");
    let (_, exposicao, _) = reabrir(&e, cx, "site:site-id-DSC_102.jpg");
    assert_eq!(
        exposicao, -0.5,
        "e continua com ele depois que o site responde"
    );
    botao(&e, cx, PedidoDaRevelacao::Sair);
    e.esperar(cx);
    let (_, exposicao, contraste) = reabrir(&e, cx, "site:site-id-DSC_101.jpg");
    assert_eq!(
        (exposicao, contraste),
        (0.0, 1.4),
        "a primeira foto não recebeu a edição, e ficou com a receita do catálogo"
    );
}

/// O ensaio aberto com a Revelação na **segunda** foto local, e a receita da
/// sessão ("Sépia à moda antiga", contraste 1,25) ainda por escolher — ela
/// chega depois, pelo [`chega_a_receita_padrao`].
fn revelando_antes_da_receita_padrao(cx: &mut TestAppContext) -> super::Estudio {
    let e = com_a_receita_da_sessao_por_escolher(cx);
    let (aberta, exposicao, contraste) = reabrir(&e, cx, "id-DSC_102.jpg");
    assert_eq!(aberta, "id-DSC_102.jpg");
    assert_eq!((exposicao, contraste), (0.0, 1.0), "abre no neutro");
    e
}

/// O ensaio aberto na grade, com "Sépia à moda antiga" (contraste 1,25) entre
/// as predefinições conhecidas, e nenhuma receita escolhida ainda.
fn com_a_receita_da_sessao_por_escolher(cx: &mut TestAppContext) -> super::Estudio {
    use domain::entities::preset::PresetAdjustments;
    use domain::entities::Preset;

    let mut presets = super::presets_do_sistema();
    presets.push(Preset::system(
        "Sépia à moda antiga",
        PresetAdjustments::vazia().com("contrast", 1.25),
    ));
    // 🔑 **Rede lenta, como a de verdade**: com a subida respondendo na hora,
    // a resposta relê o catálogo e esconde a cópia velha que a grade guarda —
    // e a corrida não aparece (visto ao escrever este cenário).
    abrir_o_ensaio(
        cx,
        Cenario {
            presets,
            site: Box::new(|site| site.demorada = true),
            ..Cenario::default()
        },
    )
}

/// A sessão ganha a receita padrão, e o serviço (uma thread de verdade) a
/// grava nas locais em segundo plano — o que a importação faz ao entrar.
///
/// `segurando` é a foto que o serviço **não alcança ainda**: sem a prévia dela
/// no cache, ele a adia, como na importação real, em que a fila anda uma foto
/// por vez. É a janela em que o operador ajusta a foto que a receita ainda vai
/// gravar. [`solta_a_receita_padrao`] devolve a prévia.
fn chega_a_receita_padrao(e: &super::Estudio, cx: &mut TestAppContext, segurando: Option<&str>) {
    if let Some(id) = segurando {
        e.previews.apagar(id);
    }
    for galeria in e.site.galerias.lock().unwrap().iter_mut() {
        if galeria.id == super::GALERIA {
            galeria.preset_padrao_id = Some("sistema:sepia".into());
        }
    }
    e.detalhe(cx, |tela, _w, cx| tela.reler(cx));
    e.esperar(cx);
    e.detalhe(cx, |_tela, _w, cx| {
        cx.emit(crate::sessoes::detalhe::Pedido::CatalogoMudou)
    });
    // A thread do serviço anda no relógio de verdade; a tela, no do teste.
    let segurando = segurando.is_some();
    esperar_o_servico(e, cx, |e, cx| {
        let (pedidas, andando) = e.app(cx, |app, _w, _cx| {
            (app.receita_padrao_pedida(), app.receita_padrao_andando())
        });
        let outra_pronta = e
            .gravador
            .gravado()
            .iter()
            .any(|(id, ajustes, _)| id == "id-DSC_101.jpg" && ajustes.contrast == 1.25);
        // A segurada fica na fila; sem ela, a fila inteira seca.
        pedidas >= 2 && outra_pronta && (segurando || !andando)
    });
}

/// Devolve a prévia que [`chega_a_receita_padrao`] segurou, e espera a fila secar.
fn solta_a_receita_padrao(e: &super::Estudio, cx: &mut TestAppContext, id: &str) {
    e.previews
        .save_preview(id, &super::imagem(160, 120, 110))
        .expect("devolver a prévia");
    esperar_o_servico(e, cx, |e, cx| {
        !e.app(cx, |app, _w, _cx| app.receita_padrao_andando())
    });
}

fn esperar_o_servico(
    e: &super::Estudio,
    cx: &mut TestAppContext,
    pronto: impl Fn(&super::Estudio, &mut TestAppContext) -> bool,
) {
    // Prazo de relógio de verdade, e não de voltas: com a suíte inteira em
    // paralelo, a thread do serviço disputa a máquina com as outras.
    let prazo = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        e.esperar(cx);
        if pronto(e, cx) {
            break;
        }
        assert!(
            std::time::Instant::now() < prazo,
            "o serviço da receita padrão não terminou em 30 s"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    e.esperar(cx);
    e.esperar(cx);
}

/// A última receita gravada no catálogo para `id`: `(exposição, contraste)`.
fn gravada(e: &super::Estudio, id: &str) -> (f32, f32) {
    e.gravador
        .gravado()
        .into_iter()
        .rev()
        .find(|(outra, _, _)| outra == id)
        .map(|(_, ajustes, _)| (ajustes.exposure, ajustes.contrast))
        .unwrap_or_else(|| panic!("nada gravado em {id}"))
}

/// 🚨 **A receita padrão que chega com a Revelação aberta não some no primeiro
/// ajuste** — a ordem em que a foto perdia o contraste da sessão.
///
/// Visto contra a pilha local (24/set/2026): a Q_0001 e a R_0001 foram abertas
/// logo depois de importadas, antes de a receita padrão chegar a elas. A tela
/// ficou nos sliders do neutro, e o primeiro ajuste gravou a foto inteira a
/// partir deles: exposição −0,5 e contraste **1,0**, e não 1,25. A foto subiu
/// assim, com o site e o catálogo de acordo — e errados.
#[gpui_kit::test]
fn a_receita_padrao_que_chega_com_a_revelacao_aberta_nao_some_no_primeiro_ajuste(
    cx: &mut TestAppContext,
) {
    let e = revelando_antes_da_receita_padrao(cx);
    chega_a_receita_padrao(&e, cx, None);
    assert_eq!(
        gravada(&e, "id-DSC_102.jpg"),
        (0.0, 1.25),
        "a receita chegou ao catálogo"
    );
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.foto_aberta().map(|f| f.id.clone()).as_deref(),
            Some("id-DSC_102.jpg"),
            "a foto aberta continua a mesma"
        );
        assert_eq!(
            tela.ajustes().contrast,
            1.25,
            "e os sliders mostram a receita que chegou"
        );
        assert!(
            !tela.pode_desfazer(),
            "sem gesto do operador, nada a desfazer"
        );
    });

    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, -0.5, cx));
    e.esperar(cx);
    assert_eq!(
        gravada(&e, "id-DSC_102.jpg"),
        (-0.5, 1.25),
        "o ajuste grava por cima da receita padrão, e não do neutro"
    );
    assert_eq!(
        gravada(&e, "id-DSC_101.jpg"),
        (0.0, 1.25),
        "a outra recebeu a receita"
    );

    e.teclar(cx, "cmd-z");
    e.esperar(cx);
    assert_eq!(
        gravada(&e, "id-DSC_102.jpg"),
        (0.0, 1.25),
        "desfazer volta à receita padrão, e não ao neutro de antes dela"
    );
}

/// 🚨 **A receita padrão que chegou antes de a Revelação abrir vale no primeiro
/// ajuste** — a terceira porta da mesma corrida.
///
/// Visto contra a pilha local (24/set/2026), já com as duas primeiras
/// fechadas: o serviço gravou a receita na U_0001 um segundo depois da
/// importação, e a Revelação só abriu cinco segundos depois — a partir da
/// cópia do catálogo que a grade guarda, que ninguém tinha relido. A foto
/// abriu no neutro, com o catálogo já em 1,25, e o ajuste gravou o neutro por
/// cima.
#[gpui_kit::test]
fn a_receita_padrao_que_chegou_antes_de_abrir_a_revelacao_vale_no_primeiro_ajuste(
    cx: &mut TestAppContext,
) {
    let e = com_a_receita_da_sessao_por_escolher(cx);
    let antes_da_receita = e.acervo.fotos.lock().unwrap().clone();
    chega_a_receita_padrao(&e, cx, None);
    assert_eq!(
        gravada(&e, "id-DSC_102.jpg"),
        (0.0, 1.25),
        "a receita chegou ao catálogo"
    );

    // 1 · Logo depois do aviso, sem releitura do catálogo no meio.
    let (aberta, exposicao, contraste) = reabrir(&e, cx, "id-DSC_102.jpg");
    assert_eq!(aberta, "id-DSC_102.jpg");
    assert_eq!(
        (exposicao, contraste),
        (0.0, 1.25),
        "a Revelação abre com o que o catálogo tem, e não com a cópia de antes"
    );
    botao(&e, cx, PedidoDaRevelacao::Sair);
    e.esperar(cx);

    // 2 · 🚨 Uma releitura que começou antes de a receita chegar ao disco
    // termina agora — a segunda metade do que se viu na pilha local: a cópia
    // da grade voltava à de antes mesmo depois do aviso.
    *e.acervo.leitura_atrasada.lock().unwrap() = Some(antes_da_receita);
    e.detalhe(cx, |_tela, _w, cx| {
        cx.emit(crate::sessoes::detalhe::Pedido::CatalogoMudou)
    });
    e.esperar(cx);
    assert!(
        e.acervo.leitura_atrasada.lock().unwrap().is_none(),
        "a leitura atrasada foi entregue"
    );
    let (_, exposicao, contraste) = reabrir(&e, cx, "id-DSC_102.jpg");
    assert_eq!(
        (exposicao, contraste),
        (0.0, 1.25),
        "a leitura atrasada não desfaz a receita que o gravador já gravou"
    );

    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, -0.5, cx));
    e.esperar(cx);
    assert_eq!(
        gravada(&e, "id-DSC_102.jpg"),
        (-0.5, 1.25),
        "o ajuste grava por cima da receita padrão"
    );
}

/// 🚨 **O ajuste feito enquanto a receita padrão espera na fila não é apagado
/// por ela** — a outra ordem da mesma corrida.
///
/// A receita é pedida na importação e gravada uma foto por vez, em segundo
/// plano; o operador abre e ajusta a foto que ainda está na fila. O serviço
/// gravava a foto **inteira** quando chegava a vez dela, e o ajuste sumia.
/// Agora ela entra só no que o operador não mexeu, a tela mostra as duas, e o
/// `⌘Z` tira o ajuste — e não a receita.
#[gpui_kit::test]
fn o_ajuste_feito_enquanto_a_receita_padrao_espera_nao_e_apagado_por_ela(cx: &mut TestAppContext) {
    let e = revelando_antes_da_receita_padrao(cx);
    chega_a_receita_padrao(&e, cx, Some("id-DSC_102.jpg"));
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, -0.5, cx));
    e.esperar(cx);
    assert_eq!(
        gravada(&e, "id-DSC_102.jpg"),
        (-0.5, 1.0),
        "a receita ainda não chegou a ela"
    );

    solta_a_receita_padrao(&e, cx, "id-DSC_102.jpg");
    assert_eq!(
        gravada(&e, "id-DSC_102.jpg"),
        (-0.5, 1.25),
        "o catálogo fica com o ajuste e com a receita"
    );
    e.revelacao(cx, |tela, _w, _cx| {
        let ajustes = tela.ajustes();
        assert_eq!(
            (ajustes.exposure, ajustes.contrast),
            (-0.5, 1.25),
            "e a tela também"
        );
    });

    e.teclar(cx, "cmd-z");
    e.esperar(cx);
    e.revelacao(cx, |tela, _w, _cx| {
        let ajustes = tela.ajustes();
        assert_eq!(
            (ajustes.exposure, ajustes.contrast),
            (0.0, 1.25),
            "desfazer tira o ajuste, e não a receita padrão"
        );
    });
    assert_eq!(gravada(&e, "id-DSC_102.jpg"), (0.0, 1.25));
}

/// 🚨 **E com a Revelação já em outra foto**: quem protege o ajuste é o
/// próprio serviço, que mescla com o que o operador gravou desde o pedido.
///
/// O operador ajusta a foto que espera a receita, volta para a sessão e abre
/// outra. A tira nova não guarda o que a de antes sabia; sem a mescla no
/// serviço, a receita apagaria o ajuste sem ninguém olhando.
#[gpui_kit::test]
fn o_ajuste_sobrevive_a_receita_padrao_mesmo_com_a_revelacao_em_outra_foto(
    cx: &mut TestAppContext,
) {
    let e = revelando_antes_da_receita_padrao(cx);
    chega_a_receita_padrao(&e, cx, Some("id-DSC_102.jpg"));
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, -0.5, cx));
    e.esperar(cx);
    botao(&e, cx, PedidoDaRevelacao::Sair);
    e.esperar(cx);
    let (aberta, _, _) = reabrir(&e, cx, "id-DSC_101.jpg");
    assert_eq!(
        aberta, "id-DSC_101.jpg",
        "a Revelação abriu de novo, noutra foto"
    );

    solta_a_receita_padrao(&e, cx, "id-DSC_102.jpg");
    assert_eq!(
        gravada(&e, "id-DSC_102.jpg"),
        (-0.5, 1.25),
        "o serviço gravou a receita por cima só do que o operador não mexeu"
    );
}
