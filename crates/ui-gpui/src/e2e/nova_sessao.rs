//! 🧭 O assistente de sete etapas da nova sessão (a rota `nova` do site).

use gpui::TestAppContext;
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
#[gpui::test]
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

/// 🧯 O site recusa: a frase dele aparece, com o caminho para a etapa, e
/// nenhuma galeria fica registrada no rascunho.
#[gpui::test]
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
#[gpui::test]
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
#[gpui::test]
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
            app.nova_sessao.read(cx).rascunho_para_teste().formulario.titulo,
            "bodas",
            "o que se digita tem de chegar ao formulário"
        );
    });
}
