//! 🧭 O assistente de sete etapas da nova sessão (a rota `nova` do site).

use gpui_kit::TestAppContext;
use serde_json::json;

use super::{abrir_o_app, Cenario, Estudio};
use crate::app::Tela;
use crate::sessoes::nova::tela::{Confirmacao, ItemDaBusca, TipoDeBusca};
use crate::sessoes::tela::NovaPedida;

/// O botão "Nova sessão" da lista, como o operador o clica.
fn abrir_o_assistente(e: &Estudio, cx: &mut TestAppContext) {
    e.app(cx, |app, _w, cx| {
        app.sessoes.update(cx, |_tela, cx| cx.emit(NovaPedida));
    });
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::NovaSessao);
        assert_eq!(
            app.nova_sessao.read(cx).etapa(),
            2,
            "o app já é o aplicativo"
        );
    });
}

fn id_do_rascunho(e: &Estudio, cx: &mut TestAppContext) -> String {
    let mut id = String::new();
    e.app(cx, |app, _w, cx| {
        id = app.nova_sessao.read(cx).rascunho_para_teste().id_provisorio;
    });
    id
}

/// Uma foto importada no rascunho — o importador de mentira não grava no
/// catálogo, então o teste grava por ele.
fn foto_no_rascunho(e: &Estudio, id: &str, rascunho: &str) {
    let mut foto = super::local(id);
    foto.id = format!("id-{id}");
    foto.sessao_id = Some(rascunho.to_string());
    foto.width = Some(600);
    foto.height = Some(400);
    e.acervo.fotos.lock().unwrap().push(foto);
}

/// 🎬 **Do começo ao fim**: fotos com a receita padrão, a etapa 3 que só
/// segura sem preço e estúdio, "Criar" que leva à pendência, o agendamento que
/// completa o contato, e a sessão criada com as fotos passando para ela.
#[gpui_kit::test]
fn a_sessao_nasce_com_o_atendimento_e_as_fotos(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.site.responder_json(
        "nova-presets",
        Ok(json!([{"id": "u1", "nome": "Meu", "ajustes": {"exposure": 0.5}}])),
    );
    e.entrar_na_conta(cx);
    abrir_o_assistente(&e, cx);

    // 📷 Etapa 2: as fotos entram sob o id provisório — nada sobe.
    let rascunho = id_do_rascunho(&e, cx);
    assert!(rascunho.starts_with("rascunho:"));
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.importar_arquivos(vec!["/cartao/DSC_1.jpg".into()], window, cx)
        });
    });
    let (arquivos, opcoes) = e.importador.importados().pop().expect("o lote saiu");
    assert_eq!(arquivos, ["/cartao/DSC_1.jpg"]);
    assert_eq!(opcoes.sessao_id.as_deref(), Some(rascunho.as_str()));
    foto_no_rascunho(&e, "nova-1", &rascunho);
    e.esperar(cx);
    assert!(e.site.subidas().is_empty(), "rascunho não sobe nada");

    // O preset e o corte padrão vão para a foto, a partir do neutro.
    e.app(cx, |app, _w, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            assert_eq!(tela.quantas_fotos(), 1);
            tela.escolher_preset(Some("u1".into()), cx);
            tela.escolher_proporcao(Some("1:1".into()), cx);
        });
    });
    e.esperar(cx);
    let (foto, ajustes, corte) = e.gravador.gravado().pop().expect("a receita foi gravada");
    assert_eq!(foto, "id-nova-1");
    assert_eq!(ajustes.exposure, 0.5);
    let (w, h) = (corte.largura.unwrap() * 600., corte.altura.unwrap() * 400.);
    assert!((w - h).abs() < 1., "o corte 1:1 centralizado: {w}×{h}");

    // Etapa 3: sem estúdio, "Avançar" segura e diz por quê.
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.ir(3, window, cx);
            tela.escolher_estudio("", window, cx);
            tela.avancar(window, cx);
            assert_eq!(tela.etapa(), 3);
            assert_eq!(
                tela.aviso_para_teste().as_deref(),
                Some("Escolha o estúdio.")
            );
            tela.escolher_produto("p1", window, cx);
            tela.escolher_estudio("e1", window, cx);
            // Sem título avança: o título só segura o "Criar".
            tela.avancar(window, cx);
            assert_eq!(tela.etapa(), 4);
            tela.criar(window, cx);
            assert_eq!(tela.etapa(), 3, "Criar leva à pendência");
            assert_eq!(
                tela.aviso_para_teste().as_deref(),
                Some("Informe o título.")
            );
            tela.digitar("", "", "", window, cx);
        });
    });

    // Etapa 4: o agendamento dá o título e o WhatsApp, que estavam vazios.
    e.site.responder_json(
        "nova-busca-agendamentos",
        Ok(json!([
            {"id": "ag1", "nome": "Maria", "whatsapp": "47999", "inicio": "2026-09-13T17:00:00Z",
             "status": "Confirmed", "estudio_id": "e1"},
            {"id": "ag2", "nome": "Cancelada", "status": "Cancelled"},
        ])),
    );
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.abrir_busca(TipoDeBusca::Agendamento, window, cx)
        });
    });
    e.esperar(cx);
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            let itens = tela.itens_da_busca();
            assert_eq!(itens.len(), 1, "o cancelado some");
            tela.escolher_da_busca(itens[0].clone(), window, cx);
            let f = tela.rascunho_para_teste().formulario;
            assert_eq!(f.titulo, "Maria — 13/09");
            assert_eq!(f.whatsapp, "47999");
            assert_eq!(f.agendamento.map(|a| a.id).as_deref(), Some("ag1"));
        });
    });
    let busca = e
        .site
        .pedidos_json()
        .into_iter()
        .find(|p| p.rotulo == "nova-busca-agendamentos")
        .expect("a busca saiu");
    assert!(
        busca
            .caminho
            .starts_with("/pos-venda/busca/agendamentos?de="),
        "{}",
        busca.caminho
    );

    // 🚀 Criar: o corpo leva o atendimento, e as fotos mudam de dono.
    e.site.responder_json(
        "nova-criada",
        Ok(json!({"id": "g9", "titulo": "Maria — 13/09",
                  "preset_padrao_id": "u1", "proporcao_padrao": "1:1"})),
    );
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.ir(7, window, cx);
            tela.criar(window, cx);
        });
    });
    e.esperar(cx);
    let criada = e
        .site
        .pedidos_json()
        .into_iter()
        .find(|p| p.rotulo == "nova-criada")
        .expect("o POST saiu");
    assert_eq!(
        (criada.metodo, criada.caminho.as_str()),
        ("POST", "/pos-venda/galerias")
    );
    let corpo = criada.corpo.expect("com corpo");
    assert_eq!(corpo["produto_id"], "p1");
    assert_eq!(corpo["estudio_id"], "e1");
    assert_eq!(corpo["ensaio_id"], "ag1");
    assert_eq!(corpo["preset_padrao_id"], "u1");
    assert_eq!(corpo["proporcao_padrao"], "1:1");
    assert_eq!(corpo["email"], serde_json::Value::Null);

    let dono = e
        .acervo
        .fotos
        .lock()
        .unwrap()
        .iter()
        .find(|f| f.id == "id-nova-1")
        .and_then(|f| f.sessao_id.clone());
    assert_eq!(dono.as_deref(), Some("g9"), "a foto passou para a sessão");
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::Sessao, "entra na sessão criada");
        // O próximo "Nova sessão" começa limpo.
        let seguinte = app.nova_sessao.read(cx).rascunho_para_teste();
        assert_ne!(seguinte.id_provisorio, rascunho);
        assert!(seguinte.formulario.titulo.is_empty());
    });
}

/// 🚀 **Criar não espera a cópia** (dono, 21/set/2026: *"a importação das
/// fotos e a aplicação dos efeitos do passo 2 precisa acontecer em segundo
/// plano quando estiver na tela da sessão para classificação e sinalização"*).
///
/// Com o cartão ainda copiando, "Criar" cria a galeria e entra na sessão na
/// hora; a barra da sessão mostra a cópia, e as fotos que terminam depois
/// passam para ela e sobem (C20) — sem o operador sair de onde classifica.
#[gpui_kit::test]
fn criar_com_a_copia_correndo_entra_na_sessao_e_o_resto_chega(cx: &mut TestAppContext) {
    let e = abrir_o_app(
        cx,
        Cenario {
            importador_demorado: true,
            ..Cenario::default()
        },
    );
    e.entrar_na_conta(cx);
    abrir_o_assistente(&e, cx);
    let rascunho = id_do_rascunho(&e, cx);

    // 📷 Três fotos no cartão; só a primeira terminou de copiar.
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.importar_arquivos(
                vec![
                    "/cartao/DSC_1.jpg".into(),
                    "/cartao/DSC_2.jpg".into(),
                    "/cartao/DSC_3.jpg".into(),
                ],
                window,
                cx,
            )
        });
    });
    e.importador.responder_uma(); // Começou
    foto_no_rascunho(&e, "nova-1", &rascunho);
    e.importador.responder_uma(); // a primeira ficou pronta
    e.esperar(cx);

    // 🚀 Criar no meio da cópia: o POST sai na hora.
    e.site.responder_json(
        "nova-criada",
        Ok(json!({"id": "g9", "titulo": "Ensaio da Ana"})),
    );
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.escolher_produto("p1", window, cx);
            tela.escolher_estudio("e1", window, cx);
            tela.digitar("Ensaio da Ana", "", "", window, cx);
            tela.criar(window, cx);
        });
    });
    e.esperar(cx);
    assert!(
        e.site
            .pedidos_json()
            .iter()
            .any(|p| p.rotulo == "nova-criada"),
        "o POST não espera a cópia"
    );
    let dono = |id: &str| {
        e.acervo
            .fotos
            .lock()
            .unwrap()
            .iter()
            .find(|f| f.id == id)
            .and_then(|f| f.sessao_id.clone())
    };
    assert_eq!(
        dono("id-nova-1").as_deref(),
        Some("g9"),
        "a pronta já passou"
    );
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::Sessao, "o operador já está na sessão");
        let detalhe = app.detalhe.read(cx);
        assert!(detalhe.importando(), "a barra da sessão mostra a cópia");
        let andamento = detalhe.importacao().expect("com andamento");
        assert_eq!((andamento.prontas(), andamento.total), (1, 3));
        // 📏 A barra do pé conta a cópia **uma vez**: a da sessão é o espelho
        // da que o assistente continua.
        assert_eq!(app.copia_da_barra_do_pe(cx), Some((3, 1)));
    });

    // 📥 O resto termina de copiar com o operador na sessão.
    foto_no_rascunho(&e, "nova-2", &rascunho);
    foto_no_rascunho(&e, "nova-3", &rascunho);
    e.importador.responder();
    e.esperar(cx);
    e.esperar(cx);

    assert_eq!(
        dono("id-nova-2").as_deref(),
        Some("g9"),
        "a que terminou depois também"
    );
    assert_eq!(dono("id-nova-3").as_deref(), Some("g9"));
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::Sessao, "e ninguém tirou o operador de lá");
        let detalhe = app.detalhe.read(cx);
        assert!(!detalhe.importando(), "a cópia terminou");
        assert_eq!(
            app.copia_da_barra_do_pe(cx),
            None,
            "e a barra a dá por feita"
        );
        assert!(
            ["id-nova-1", "id-nova-2", "id-nova-3"]
                .iter()
                .all(|id| detalhe.como_esta(id).is_some()),
            "as três na grade da sessão"
        );
        assert!(
            app.nova_sessao.read(cx).levando_para_teste().is_none(),
            "nada ficou esperando para ser levado"
        );
    });
    let subidas: Vec<String> = e.site.subidas().into_iter().map(|s| s.1).collect();
    for id in ["id-nova-1", "id-nova-2", "id-nova-3"] {
        assert!(
            subidas.contains(&id.to_string()),
            "{id} sobe (C20): {subidas:?}"
        );
    }
}

/// 🚨 **A cópia que termina durante a troca da criação também chega**
/// (26/set/2026: 40 fotos de 24 MB, 12 ficaram para trás).
///
/// "Criar" passa as fotos do rascunho para a sessão com um `UPDATE`, e a
/// resposta dele volta um respiro depois. Se as últimas fotos são gravadas
/// nesse meio — depois do `UPDATE`, antes da resposta — e o `Terminou` chega
/// junto, a tela via a cópia terminada e não armava a varredura seguinte: as
/// fotos ficavam com o id do rascunho, que é trocado logo em seguida, e
/// nenhuma troca mais as levava.
#[gpui_kit::test]
fn a_copia_que_termina_durante_a_troca_nao_fica_no_rascunho(cx: &mut TestAppContext) {
    let e = abrir_o_app(
        cx,
        Cenario {
            importador_demorado: true,
            ..Cenario::default()
        },
    );
    e.entrar_na_conta(cx);
    abrir_o_assistente(&e, cx);
    let rascunho = id_do_rascunho(&e, cx);

    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.importar_arquivos(
                vec![
                    "/cartao/DSC_1.jpg".into(),
                    "/cartao/DSC_2.jpg".into(),
                    "/cartao/DSC_3.jpg".into(),
                ],
                window,
                cx,
            )
        });
    });
    e.importador.responder_uma(); // Começou
    foto_no_rascunho(&e, "nova-1", &rascunho);
    e.importador.responder_uma(); // a primeira ficou pronta
    e.esperar(cx);

    // 🚀 Criar: o `UPDATE` roda, e a resposta dele ainda não voltou.
    e.acervo
        .segurar_trocas
        .store(true, std::sync::atomic::Ordering::Relaxed);
    e.site.responder_json(
        "nova-criada",
        Ok(json!({"id": "g9", "titulo": "Ensaio da Ana"})),
    );
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.escolher_produto("p1", window, cx);
            tela.escolher_estudio("e1", window, cx);
            tela.digitar("Ensaio da Ana", "", "", window, cx);
            tela.criar(window, cx);
        });
    });
    e.esperar(cx);

    // 📥 As duas últimas gravadas depois do `UPDATE`, e a cópia termina.
    foto_no_rascunho(&e, "nova-2", &rascunho);
    foto_no_rascunho(&e, "nova-3", &rascunho);
    e.importador.responder();
    e.esperar(cx);

    // A resposta da troca enfim volta.
    e.acervo
        .segurar_trocas
        .store(false, std::sync::atomic::Ordering::Relaxed);
    e.acervo.responder_as_trocas();
    e.esperar(cx);
    e.esperar(cx);

    let dono = |id: &str| {
        e.acervo
            .fotos
            .lock()
            .unwrap()
            .iter()
            .find(|f| f.id == id)
            .and_then(|f| f.sessao_id.clone())
    };
    for id in ["id-nova-1", "id-nova-2", "id-nova-3"] {
        assert_eq!(dono(id).as_deref(), Some("g9"), "{id} chegou à sessão");
    }
    e.app(cx, |app, _w, cx| {
        assert!(
            app.nova_sessao.read(cx).levando_para_teste().is_none(),
            "e nada ficou esperando para ser levado"
        );
    });
}

/// 🚨 **A foto a caminho da sessão criada não é "de um rascunho antigo".**
///
/// Com a cópia seguindo em segundo plano, o rascunho da tela já é outro, e a
/// foto gravada entre duas trocas carrega o id velho por um respiro. A
/// releitura a via como órfã e oferecia "Apagar" — que tira do catálogo **e do
/// disco** as fotos da sessão anterior no meio da cópia.
#[gpui_kit::test]
fn a_foto_em_transito_nao_e_oferecida_para_apagar(cx: &mut TestAppContext) {
    let e = abrir_o_app(
        cx,
        Cenario {
            importador_demorado: true,
            ..Cenario::default()
        },
    );
    e.entrar_na_conta(cx);
    abrir_o_assistente(&e, cx);
    let rascunho = id_do_rascunho(&e, cx);

    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.importar_arquivos(
                vec!["/cartao/DSC_1.jpg".into(), "/cartao/DSC_2.jpg".into()],
                window,
                cx,
            )
        });
    });
    e.importador.responder_uma(); // Começou
    foto_no_rascunho(&e, "nova-1", &rascunho);
    e.importador.responder_uma();
    e.esperar(cx);
    e.site.responder_json(
        "nova-criada",
        Ok(json!({"id": "g9", "titulo": "Ensaio da Ana"})),
    );
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.escolher_produto("p1", window, cx);
            tela.escolher_estudio("e1", window, cx);
            tela.digitar("Ensaio da Ana", "", "", window, cx);
            tela.criar(window, cx);
        });
    });
    e.esperar(cx);

    // A segunda gravada com o id velho, e a tela relê antes da próxima troca.
    foto_no_rascunho(&e, "nova-2", &rascunho);
    e.app(cx, |app, window, cx| {
        app.nova_sessao
            .update(cx, |tela, cx| tela.reler_fotos_para_teste(window, cx));
    });
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        let nova = app.nova_sessao.read(cx);
        assert!(nova.levando_para_teste().is_some(), "a cópia ainda anda");
        assert!(
            nova.orfas_para_teste().is_empty(),
            "a foto a caminho não é órfã: {:?}",
            nova.orfas_para_teste()
        );
    });

    // E termina na sessão.
    e.importador.responder();
    e.esperar(cx);
    e.esperar(cx);
    let dono = e
        .acervo
        .fotos
        .lock()
        .unwrap()
        .iter()
        .find(|f| f.id == "id-nova-2")
        .and_then(|f| f.sessao_id.clone());
    assert_eq!(dono.as_deref(), Some("g9"));
}

/// 🧯 O site recusa: a frase dele aparece, com o caminho para a etapa, e
/// nenhuma galeria fica registrada no rascunho.
#[gpui_kit::test]
fn a_recusa_do_site_leva_a_etapa(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    abrir_o_assistente(&e, cx);
    e.site.responder_json(
        "nova-criada",
        Err("o site respondeu 409: Este voucher já foi usado.".into()),
    );
    e.site.responder_json(
        "nova-busca-vouchers",
        Ok(json!([{"id": "v1", "numero": 12, "nome": "Ana", "email": "ana@x.com"}])),
    );
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.escolher_produto("p1", window, cx);
            tela.escolher_estudio("e1", window, cx);
            tela.digitar("Ensaio da Ana", "", "", window, cx);
            tela.abrir_busca(TipoDeBusca::Voucher, window, cx);
        });
    });
    e.esperar(cx);
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            let Some(ItemDaBusca::Voucher(v)) = tela.itens_da_busca().first().cloned() else {
                panic!("o voucher chegou");
            };
            assert_eq!(v.numero, "12");
            tela.escolher_da_busca(ItemDaBusca::Voucher(v), window, cx);
            assert_eq!(tela.rascunho_para_teste().formulario.email, "ana@x.com");
            tela.ir(7, window, cx);
            tela.criar(window, cx);
        });
    });
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::NovaSessao);
        let tela = app.nova_sessao.read(cx);
        assert_eq!(
            tela.erro_para_teste(),
            Some(("Este voucher já foi usado.".into(), Some(5)))
        );
        assert!(tela.rascunho_para_teste().criada_id.is_none());
    });
}

/// 🗑️ "Descartar" apaga as fotos do rascunho e começa outro; "Voltar"
/// guarda, e a volta pergunta se retoma.
#[gpui_kit::test]
fn descartar_apaga_e_voltar_guarda(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    abrir_o_assistente(&e, cx);
    let primeiro = id_do_rascunho(&e, cx);
    foto_no_rascunho(&e, "vai-sumir", &primeiro);

    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.pedir_confirmacao(Confirmacao::Descartar, cx);
            tela.confirmar_para_teste(window, cx);
        });
    });
    e.esperar(cx);
    assert!(
        e.acervo
            .fotos
            .lock()
            .unwrap()
            .iter()
            .all(|f| f.sessao_id.as_deref() != Some(primeiro.as_str())),
        "as fotos do rascunho saíram do catálogo"
    );
    let segundo = id_do_rascunho(&e, cx);
    assert_ne!(segundo, primeiro);

    // Um título, e "Voltar": o rascunho fica; entrar de novo pergunta.
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.digitar("Batizado", "", "", window, cx);
            tela.voltar_as_sessoes(cx);
        });
    });
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessoes));
    abrir_o_assistente(&e, cx);
    e.app(cx, |app, _w, cx| {
        let tela = app.nova_sessao.read(cx);
        assert!(
            !tela.perguntando_se_retoma(),
            "é o mesmo que estava na tela"
        );
        assert_eq!(tela.rascunho_para_teste().formulario.titulo, "Batizado");
    });
}

/// 🚨 **Digitar o título na etapa 3, com o teclado de verdade.**
///
/// Todo teste daqui preenchia o formulário por `digitar(...)`, que escreve no
/// rascunho sem passar pelo campo. O caminho do operador — foco no `Input`,
/// tecla, `InputEvent::Change` — nunca foi exercido (dono, 20/set/2026: *"não
/// consigo digitar o título como se tivesse um bug no input"*).
#[gpui_kit::test]
fn o_titulo_aceita_o_teclado(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    abrir_o_assistente(&e, cx);

    // "Criar" sem título leva à etapa 3 e põe o foco no campo, que é como o
    // operador chega nele.
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.ir(3, window, cx);
            tela.escolher_produto("p1", window, cx);
            tela.escolher_estudio("e1", window, cx);
            tela.criar(window, cx);
        });
    });
    e.esperar(cx);

    e.teclar(cx, "b o d a s");
    e.esperar(cx);

    e.app(cx, |app, _w, cx| {
        assert_eq!(
            app.nova_sessao
                .read(cx)
                .rascunho_para_teste()
                .formulario
                .titulo,
            "bodas",
            "o que se digita tem de chegar ao formulário"
        );
    });
}

/// 🚨 **A lista que chega do servidor não pode tirar o foco de quem digita.**
///
/// O operador abre a etapa 3 e começa pelo título, que é o primeiro campo.
/// Produtos e estúdios chegam da rede logo depois — e `atualizar_escolhas`
/// refaz os dois `Select` passando a janela. Se isso mover o foco, o resto do
/// que ele digitou cai fora do campo.
#[gpui_kit::test]
fn a_lista_que_chega_nao_rouba_o_foco_do_titulo(cx: &mut TestAppContext) {
    let e = abrir_o_app(
        cx,
        Cenario {
            site: Box::new(|site| site.demorada = true),
            ..Default::default()
        },
    );
    e.entrar_na_conta(cx);
    e.app(cx, |app, _w, cx| {
        app.sessoes.update(cx, |_tela, cx| cx.emit(NovaPedida));
    });
    e.esperar(cx);

    // Etapa 3, foco no título — o caminho do "Criar" com pendência.
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.ir(3, window, cx);
            tela.criar(window, cx);
        });
    });
    e.esperar(cx);
    e.teclar(cx, "b o");

    // A rede responde AGORA: galerias, produtos e estúdios de uma vez.
    e.site.responder();
    e.esperar(cx);

    e.teclar(cx, "d a s");
    e.esperar(cx);

    e.app(cx, |app, _w, cx| {
        assert_eq!(
            app.nova_sessao
                .read(cx)
                .rascunho_para_teste()
                .formulario
                .titulo,
            "bodas",
            "as listas chegaram no meio da digitação e levaram o foco embora"
        );
    });
}

/// 🚨 **Clicar no título e digitar** — o caminho do dedo, do começo ao fim.
///
/// Os testes daqui punham o foco por `criar()` (a pendência leva ao campo) ou
/// escreviam no rascunho por `digitar(...)`. Nenhum clicava no campo, que é
/// como o operador chega nele.
#[gpui_kit::test]
fn clicar_no_titulo_e_digitar(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    abrir_o_assistente(&e, cx);
    e.app(cx, |app, window, cx| {
        app.nova_sessao
            .update(cx, |tela, cx| tela.ir(3, window, cx));
    });
    e.esperar(cx);

    let mut visual = gpui_kit::VisualTestContext::from_window(e.raiz.into(), cx);
    visual.run_until_parked();
    let onde = visual
        .debug_bounds("nova-titulo")
        .expect("o campo do título está desenhado");
    visual.simulate_click(onde.center(), gpui_kit::Modifiers::none());
    visual.run_until_parked();

    e.teclar(cx, "b o d a s");
    e.esperar(cx);

    e.app(cx, |app, _w, cx| {
        assert_eq!(
            app.nova_sessao
                .read(cx)
                .rascunho_para_teste()
                .formulario
                .titulo,
            "bodas",
            "clicar no campo tem de dar o foco a ele"
        );
    });
}

/// 🚨 **Digitar o título com a importação correndo por baixo** — o caso real.
///
/// Dono, 20/set/2026: *"não consigo digitar o título como se tivesse um bug no
/// input"*, com a bandeja marcando 54 → 55 → 60 fotos esperando nota: as fotos
/// entravam enquanto ele escrevia. A cada leva a tela relê o catálogo, refaz
/// `self.fotos` e aplica a receita — e a colheita acorda a ~10 Hz.
#[gpui_kit::test]
fn digitar_o_titulo_com_a_importacao_correndo(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    abrir_o_assistente(&e, cx);
    let rascunho = id_do_rascunho(&e, cx);

    // Uma leva entra, como o cartão despejando.
    e.app(cx, |app, window, cx| {
        app.nova_sessao.update(cx, |tela, cx| {
            tela.importar_arquivos(
                (1..=8).map(|n| format!("/cartao/DSC_{n}.jpg")).collect(),
                window,
                cx,
            )
        });
    });
    for n in 1..=8 {
        foto_no_rascunho(&e, &format!("leva-{n}"), &rascunho);
    }

    e.app(cx, |app, window, cx| {
        app.nova_sessao
            .update(cx, |tela, cx| tela.ir(3, window, cx));
    });

    // Clica no campo e digita enquanto o catálogo ainda está se mexendo.
    let mut visual = gpui_kit::VisualTestContext::from_window(e.raiz.into(), cx);
    visual.run_until_parked();
    let onde = visual
        .debug_bounds("nova-titulo")
        .expect("o campo do título está desenhado");
    visual.simulate_click(onde.center(), gpui_kit::Modifiers::none());
    visual.run_until_parked();

    for tecla in ["b", "o", "d", "a", "s"] {
        e.teclar(cx, tecla);
        // Entre uma tecla e outra, a colheita acorda e o catálogo é relido.
        e.esperar(cx);
    }

    e.app(cx, |app, _w, cx| {
        assert_eq!(
            app.nova_sessao
                .read(cx)
                .rascunho_para_teste()
                .formulario
                .titulo,
            "bodas",
            "a importação por baixo comeu o que foi digitado"
        );
    });
}

/// 🚨 **A grade do assistente continua mostrando as fotos** — com a leitura do
/// cache fora da linha da interface.
///
/// A miniatura deixou de ser lida dentro do desenho e passa por uma thread
/// (`sessoes::nova::miniaturas`), que a devolve por canal. O ganho é a linha da
/// interface livre; o risco é a grade ficar cinza para sempre se ninguém
/// colher. Este teste é o que separa os dois.
#[gpui_kit::test]
fn a_grade_do_assistente_recebe_as_miniaturas_lidas_na_thread(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    abrir_o_assistente(&e, cx);
    let rascunho = id_do_rascunho(&e, cx);

    // Duas fotos no rascunho, com prévia já no cache (o `abrir_o_app` grava a
    // de cada nome de `LOCAIS`).
    for nome in ["DSC_101.jpg", "DSC_102.jpg"] {
        let mut foto = super::local(nome);
        foto.sessao_id = Some(rascunho.clone());
        e.acervo.fotos.lock().unwrap().push(foto);
    }
    e.app(cx, |app, window, cx| {
        app.nova_sessao
            .update(cx, |tela, cx| tela.reler_fotos_para_teste(window, cx));
    });

    // 🔑 **Desenhar e deixar o relógio andar, alternadamente.** Quem pede as
    // miniaturas é `preparar_miniaturas`, dentro do `render` — sem um quadro,
    // nada é pedido; e quem recolhe a releitura do catálogo é a colheita, que
    // só acorda com o relógio andando.
    //
    // E a thread das miniaturas é de verdade: o relógio do teste não a faz
    // andar. Dá-se a volta até ela responder, com teto — grade cinza para
    // sempre é o defeito que este teste existe para pegar.
    let mut quantas = 0;
    for _ in 0..80 {
        e.esperar(cx);
        {
            let visual = gpui_kit::VisualTestContext::from_window(e.raiz.into(), cx);
            visual.run_until_parked();
        }
        quantas = e.app(cx, |app, _w, cx| {
            app.nova_sessao.read(cx).quantas_miniaturas()
        });
        if quantas == 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(
        quantas, 2,
        "as duas fotos do rascunho têm prévia no cache: a grade tinha de mostrá-las"
    );
}
