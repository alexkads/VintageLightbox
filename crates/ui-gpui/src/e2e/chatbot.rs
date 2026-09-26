//! 💬 O chatbot no desktop — cada gesto do `/dashboard/chatbot` do site, com
//! o app inteiro montado, a tela clicada onde o dedo clica e o site de
//! mentira anotando o que chegou.
//!
//! 🔑 **O que estes cenários provam que os testes do modelo não provam**: que
//! o clique chega ao pedido certo, que o tempo real chega à tela com o app em
//! outra tela, e que o aviso do sistema só sai quando a janela não está na
//! frente. A junta com a API de verdade é conferida contra a pilha local
//! (skill `conferir-o-app-desktop`); aqui é a janela.

use crate::campo::TrocarValor as _;
use std::time::Duration;

use gpui_kit::{Context, Modifiers, TestAppContext, VisualTestContext, Window};
use serde_json::{json, Value};

use super::{abrir_o_app, Cenario, Estudio};
use crate::app::Tela;
use crate::chatbot::modelo::{Canal, Chave, FiltroDeCanal, Janela};
use crate::chatbot::tela::Dialogo;
use crate::chatbot::Chatbot;
use crate::pos_venda::porta::PedidoJson;
use crate::tempo_real::{Aviso, EstadoDaConexao, Sinal};

const ANA: &str = "5554999991234";
const CAIO: &str = "5554888880000";

/// Um instante relativo a agora — a janela de 24 h é contada com o relógio de
/// verdade, e o cenário precisa dela aberta (ou fechada) de propósito.
fn ha(minutos: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::minutes(minutos)).to_rfc3339()
}

fn conversa_da_ana(pausada: bool) -> Value {
    json!({
        "contact_id": ANA, "profile_name": "Ana", "last_message": "oi, tudo bem?",
        "last_message_time": ha(5), "message_count": 2, "unread_count": 1,
        "is_paused": pausada, "has_only_automated_messages": false,
        "ultima_entrada": ha(60)
    })
}

fn mensagens_da_ana(quantas: usize) -> Vec<Value> {
    (0..quantas)
        .map(|i| {
            json!({
                "id": format!("m{i}"), "recipient_id": ANA,
                "event": if i % 2 == 0 { "received" } else { "delivered" },
                "profile_name": "Ana", "message_id": format!("wamid.{i}"),
                "message": format!("mensagem {i}"), "timestamp": ha(600 - i as i64),
                "is_edited": false, "is_automated": false, "automation_type": null,
                "created_at": ha(600 - i as i64)
            })
        })
        .collect()
}

fn pagina(conversas: Vec<Value>, mensagens: Vec<Value>, total: i64) -> Value {
    json!({ "conversations": conversas, "total_count": total, "has_more": false, "all_messages": mensagens })
}

/// O site com os cinco canais, uma urgência e uma resposta pronta.
fn preparar_o_site(e: &Estudio) {
    let s = &e.site;
    s.responder_json(
        "wa-conversas",
        Ok(pagina(vec![conversa_da_ana(false)], mensagens_da_ana(2), 1)),
    );
    s.responder_json(
        "ig-conversas",
        Ok(json!([{
            "contato": "ig-1", "nome": "Bia", "ultima_mensagem": "quero agendar",
            "ultima_atividade": ha(10), "assumida_por_humano": false, "sessao_id": "s1"
        }])),
    );
    s.responder_json(
        "web-conversas",
        Ok(json!([{
            "contato": "sess-1", "nome": "Visitante 7", "ultima_mensagem": "tem horário?",
            "ultima_atividade": ha(30), "assumida_por_humano": false, "sessao_id": "s2",
            "na_pagina": true, "saiu_ha_minutos": null, "email": null, "whatsapp": null,
            "primeira_url": null
        }])),
    );
    s.responder_json("tg-conversas", Ok(json!([])));
    s.responder_json("ms-conversas", Ok(json!([])));
    s.responder_json(
        "respostas",
        Ok(json!([{
            "id": "r1", "title": "Endereço", "message": "Rua Coberta, 12 — Gramado",
            "category": "location", "usage": 3
        }])),
    );
    s.responder_json(
        "urgencias",
        Ok(json!([{
            "id": "u1", "contact_id": CAIO, "profile_name": "Caio", "session_id": null,
            "reason": "TRANSFER_TO_HUMAN", "urgency_score": 10,
            "context_summary": "quero falar com alguém", "status": "PENDING",
            "waiting_time_minutes": 35, "priority_level": "HIGH", "message_count": 4,
            "already_transferred": false
        }])),
    );
    s.responder_json(
        "vouchers",
        Ok(json!([{
            "contact_id": ANA, "has_voucher": true, "voucher_code": "V-1",
            "voucher_used": false, "reminders_sent": 2, "reminder_opt_out": false
        }])),
    );
    s.responder_json(
        "cadastros",
        Ok(json!([{
            "user_id": "u-ana", "whatsapp": ANA, "abandoned_carts": 1,
            "completed_orders": 0, "last_order_date": null
        }])),
    );
    s.responder_json(
        "historico",
        Ok(json!([
            {"id": "i1", "message_id": "a", "direcao": "INBOUND", "conteudo": "quero agendar", "em": ha(10)},
            {"id": "i2", "message_id": "b", "direcao": "OUTBOUND", "conteudo": "Claro!", "em": ha(9)}
        ])),
    );
    s.responder_json(
        "wa-fora",
        Ok(pagina(
            vec![json!({
                "contact_id": CAIO, "profile_name": "Caio", "last_message": "atendente?",
                "last_message_time": ha(40), "message_count": 1, "unread_count": 1,
                "is_paused": false, "has_only_automated_messages": false,
                "ultima_entrada": ha(40)
            })],
            vec![json!({
                "id": "c1", "recipient_id": CAIO, "event": "received", "profile_name": "Caio",
                "message_id": "wamid.c", "message": "atendente?", "timestamp": ha(40),
                "is_edited": false, "is_automated": false, "automation_type": null,
                "created_at": ha(40)
            })],
            1,
        )),
    );
    s.responder_json("enviada", Ok(json!({"message_id": "wamid.novo"})));
    s.responder_json("assumida", Ok(Value::Null));
    s.responder_json("devolvida", Ok(Value::Null));
    s.responder_json("uso-contado", Ok(Value::Null));
    s.responder_json("urgencia-assumida", Ok(Value::Null));
    s.responder_json("urgencia-resolvida", Ok(Value::Null));
    s.responder_json("urgencia-descartada", Ok(Value::Null));
    s.responder_json("historico-apagado", Ok(json!(7)));
}

/// Entra na conta — o chatbot passa a escutar, mas continua escondido.
fn entrar(cx: &mut TestAppContext) -> Estudio {
    let e = abrir_o_app(cx, Cenario::default());
    preparar_o_site(&e);
    e.entrar_na_conta(cx);
    e
}

/// Entra e abre o chatbot **pelo menu lateral**, como o operador.
fn abrir_o_chatbot(cx: &mut TestAppContext) -> Estudio {
    let e = entrar(cx);
    clicar(&e, cx, "menu-Chatbot");
    e.esperar(cx);
    e
}

/// Um quadro novo. 🔑 O `debug_bounds` lê o **último quadro desenhado**, e o
/// `run_until_parked` não desenha: sem isto o teste clicaria (ou procuraria)
/// no que a tela mostrava antes do gesto anterior.
///
/// 🚨 **E o mapa nunca esvazia.** No GPUI 0.2.2 o `Frame::clear` limpa tudo
/// menos os `debug_bounds`: o que foi desenhado uma vez continua lá. Por isso
/// [`desenhado`] só serve para afirmar que algo **aparece**; que algo sumiu se
/// afirma pelo estado que decide o desenho.
pub(super) fn quadro_novo(e: &Estudio, cx: &mut TestAppContext) -> VisualTestContext {
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    visual
}

pub(super) fn clicar(e: &Estudio, cx: &mut TestAppContext, alvo: &str) {
    let mut visual = quadro_novo(e, cx);
    let alvo: &'static str = Box::leak(alvo.to_string().into_boxed_str());
    let onde = visual
        .debug_bounds(alvo)
        .unwrap_or_else(|| panic!("{alvo} não está desenhado na tela"));
    visual.simulate_click(onde.center(), Modifiers::none());
    visual.run_until_parked();
}

pub(super) fn desenhado(e: &Estudio, cx: &mut TestAppContext, alvo: &str) -> bool {
    let mut visual = quadro_novo(e, cx);
    let alvo: &'static str = Box::leak(alvo.to_string().into_boxed_str());
    visual.debug_bounds(alvo).is_some()
}

fn chatbot<R>(
    e: &Estudio,
    cx: &mut TestAppContext,
    f: impl FnOnce(&mut Chatbot, &mut Window, &mut Context<Chatbot>) -> R,
) -> R {
    e.app(cx, |app, window, cx| {
        let chatbot = app.chatbot.clone();
        chatbot.update(cx, |tela, cx| f(tela, window, cx))
    })
}

/// Os pedidos do chatbot ao site, sem os da conta e das sessões.
fn pedidos(e: &Estudio) -> Vec<PedidoJson> {
    e.site
        .pedidos_json()
        .into_iter()
        .filter(|p| {
            [
                "/whatsapp",
                "/instagram",
                "/lexdesk",
                "/telegram",
                "/messenger",
                "/crm",
            ]
            .iter()
            .any(|raiz| p.caminho.starts_with(raiz))
        })
        .collect()
}

fn caminhos(e: &Estudio) -> Vec<String> {
    pedidos(e)
        .into_iter()
        .map(|p| format!("{} {}", p.metodo, p.caminho))
        .collect()
}

fn leituras_da_lista(e: &Estudio) -> usize {
    pedidos(e)
        .iter()
        .filter(|p| {
            p.caminho.starts_with("/whatsapp/conversations?")
                && !p.caminho.contains("contact_filter")
        })
        .count()
}

pub(super) fn toasts(e: &Estudio, cx: &mut TestAppContext) -> Vec<(String, bool)> {
    e.app(cx, |app, _w, _cx| app.avisos_dados_para_teste())
}

fn evento(fonte: &'static str, dados: Value) -> Sinal {
    Sinal::Evento { fonte, dados }
}

fn mensagem_da_ana(previa: &str) -> Sinal {
    evento(
        "wa",
        json!({"tipo": "mensagem_recebida", "contato": ANA, "nome": "Ana", "previa": previa, "em": ha(0)}),
    )
}

/// Um passo do relógio do app, para o laço do chatbot colher.
pub(super) fn passo(cx: &mut TestAppContext) {
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.run_until_parked();
}

fn escrever(e: &Estudio, cx: &mut TestAppContext, texto: &str) {
    chatbot(e, cx, |t, w, cx| {
        let texto = texto.to_string();
        t.compositor
            .update(cx, |c, cx| c.trocar_valor(texto, w, cx));
    });
}

// ── A conta e a lista ──────────────────────────────────────────────────────

/// 🔑 **Escutar começa na entrada, ler só na tela.** O aviso tem de chegar
/// com o operador em qualquer tela — mas abrir o app não pode custar oito
/// leituras ao site para uma tela que ninguém abriu.
#[gpui_kit::test]
fn a_conta_entra_e_o_chatbot_escuta_os_cinco_canais_sem_ler_nada(cx: &mut TestAppContext) {
    let e = entrar(cx);
    assert_eq!(e.escuta.abertas(), 2, "a do chatbot e a da agenda");
    assert_eq!(
        e.escuta.caminhos_da_fonte("wa"),
        vec![
            "/whatsapp/eventos",
            "/instagram/eventos",
            "/messenger/eventos",
            "/telegram/eventos",
            "/lexdesk/eventos"
        ],
        "os cinco fluxos que o proxy do site funde"
    );
    assert!(pedidos(&e).is_empty(), "nada lido: {:?}", caminhos(&e));
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessoes));
}

/// Pelo menu: os cinco canais, as respostas e as urgências numa leva, o
/// voucher e o cadastro na leva seguinte (dependem da página), e a lista
/// fundida do mais recente para o mais antigo.
#[gpui_kit::test]
fn pelo_menu_le_os_cinco_canais_e_monta_a_lista_do_site(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Chatbot));
    assert_eq!(
        caminhos(&e),
        vec![
            "GET /whatsapp/conversations?show_automated=true&page=1&limit=15",
            "GET /instagram/conversas",
            "GET /lexdesk/conversas",
            "GET /telegram/conversas",
            "GET /messenger/conversas",
            "GET /whatsapp/quick-responses",
            "GET /crm/urgent",
            "POST /whatsapp/vouchers/info",
            "POST /crm/contacts/by-whatsapp",
        ]
    );
    let voucher = pedidos(&e)[7].corpo.clone().unwrap();
    assert_eq!(voucher, json!({"contact_ids": [ANA]}));

    chatbot(&e, cx, |t, _w, _cx| {
        let ids: Vec<String> = t.visiveis().into_iter().map(|c| c.chave.texto()).collect();
        assert_eq!(
            ids,
            [format!("wa:{ANA}"), "ig:ig-1".into(), "web:sess-1".into()]
        );
        assert_eq!(
            t.linha_de_resumo().as_ref(),
            "3 conversas · 3 com bot · 0 em atendimento"
        );
        let ana = &t.visiveis()[0];
        assert_eq!(ana.voucher.as_ref().map(|v| v.reminders_sent), Some(2));
        assert_eq!(ana.cadastro.as_ref().map(|c| c.abandoned_carts), Some(1));
        assert!(t.alguem_esperando(), "o Caio pediu atendente");
    });
    assert!(desenhado(&e, cx, "chatbot-conversa-2"));

    // As abas recortam sem reler — os dados são os mesmos.
    let antes = pedidos(&e).len();
    clicar(&e, cx, "aba-instagram");
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.canal, FiltroDeCanal::So(Canal::Instagram));
        assert_eq!(t.visiveis().len(), 1);
    });
    assert_eq!(pedidos(&e).len(), antes, "aba não relê");
    clicar(&e, cx, "aba-todas");

    // O status "Em atendimento" relê: no WhatsApp o filtro é do servidor.
    clicar(&e, cx, "status-humano");
    assert!(caminhos(&e).iter().any(|c| c
        == "GET /whatsapp/conversations?status_filter=manual&show_automated=true&page=1&limit=15"));
    chatbot(&e, cx, |t, _w, _cx| {
        assert!(t.visiveis().is_empty(), "ninguém em atendimento");
    });
}

/// A busca vale no Enter (o `submit` do site), volta à página 1 e vai
/// codificada; o X limpa e relê sem ela.
#[gpui_kit::test]
fn a_busca_vale_no_enter_e_o_x_limpa(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    chatbot(&e, cx, |t, w, cx| {
        t.busca
            .update(cx, |c, cx| c.trocar_valor("maria josé", w, cx));
        t.busca.update(cx, |c, cx| c.focus(w, cx));
    });
    assert_eq!(leituras_da_lista(&e), 1, "digitar não busca");
    e.teclar(cx, "enter");
    e.esperar(cx);
    assert!(caminhos(&e).iter().any(|c| c
        == "GET /whatsapp/conversations?search_query=maria%20jos%C3%A9&show_automated=true&page=1&limit=15"));
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.busca_aplicada, "maria josé");
        // Os outros canais são peneirados aqui: nenhum casa.
        assert!(t
            .visiveis()
            .iter()
            .all(|c| c.chave.canal == Canal::WhatsApp));
    });

    clicar(&e, cx, "chatbot-limpar-busca");
    e.esperar(cx);
    assert_eq!(
        caminhos(&e)
            .iter()
            .filter(|c| c.starts_with("GET /whatsapp/conversations?show_automated"))
            .count(),
        2,
        "a leitura sem busca voltou"
    );
    chatbot(&e, cx, |t, _w, cx| {
        assert_eq!(t.busca.read(cx).value().as_ref(), "", "o campo esvaziou");
        assert_eq!(t.busca_aplicada, "");
    });
}

/// "Anterior" e "Próxima" andam só o WhatsApp, e param nas pontas.
#[gpui_kit::test]
fn a_paginacao_anda_o_whatsapp_e_para_nas_pontas(cx: &mut TestAppContext) {
    let e = entrar(cx);
    e.site.responder_json(
        "wa-conversas",
        Ok(pagina(
            vec![conversa_da_ana(false)],
            mensagens_da_ana(2),
            31,
        )),
    );
    clicar(&e, cx, "menu-Chatbot");
    e.esperar(cx);
    chatbot(&e, cx, |t, _w, _cx| assert_eq!(t.paginas(), 3));

    clicar(&e, cx, "chatbot-anterior");
    e.esperar(cx);
    assert_eq!(
        leituras_da_lista(&e),
        1,
        "na primeira, Anterior não faz nada"
    );

    clicar(&e, cx, "chatbot-proxima");
    e.esperar(cx);
    clicar(&e, cx, "chatbot-proxima");
    e.esperar(cx);
    clicar(&e, cx, "chatbot-proxima");
    e.esperar(cx);
    let paginas: Vec<String> = caminhos(&e)
        .into_iter()
        .filter(|c| c.starts_with("GET /whatsapp/conversations?"))
        .collect();
    assert_eq!(
        paginas.len(),
        3,
        "a terceira Próxima não passa da última: {paginas:?}"
    );
    assert!(paginas[1].contains("page=2") && paginas[2].contains("page=3"));
}

// ── O tempo real e os avisos ───────────────────────────────────────────────

/// 🔔 **Mensagem nova com o operador em outra tela e a janela na frente**:
/// toast dentro do app, o selo acende no menu, e nada é relido — a tela do
/// chatbot está escondida.
#[gpui_kit::test]
fn mensagem_nova_em_outra_tela_vira_toast_e_acende_o_selo(cx: &mut TestAppContext) {
    let e = entrar(cx);
    assert!(!desenhado(&e, cx, "menu-selo-do-chatbot"));

    e.escuta.mandar(mensagem_da_ana("oi, tudo bem?"));
    e.esperar(cx);
    assert!(toasts(&e, cx).contains(&("Ana no WhatsApp — oi, tudo bem?".into(), false)));
    assert!(
        e.avisador.avisos().is_empty(),
        "a janela está na frente: sem aviso do sistema"
    );
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.quantas_novidades(), 1);
        assert!(t.novidades.contains(&Chave::nova(Canal::WhatsApp, ANA)));
    });
    assert!(desenhado(&e, cx, "menu-selo-do-chatbot"));
    assert!(pedidos(&e).is_empty(), "escondido, não relê");

    // Só mensagem do cliente avisa: a nossa saída e a pausa não.
    let quantos = toasts(&e, cx).len();
    e.escuta.mandar(evento(
        "wa",
        json!({"tipo": "mensagem_enviada", "contato": ANA}),
    ));
    e.escuta
        .mandar(evento("wa", json!({"tipo": "bot_pausado", "contato": ANA})));
    e.esperar(cx);
    assert_eq!(toasts(&e, cx).len(), quantos);

    // Voltar à tela relê o que ficou desatualizado.
    clicar(&e, cx, "menu-Chatbot");
    e.esperar(cx);
    assert_eq!(leituras_da_lista(&e), 1);
}

/// 🔔 **Com a janela atrás**, o aviso é do sistema, com os textos do site; o
/// clique nele traz o app, abre o chatbot e a conversa, e apaga a novidade.
#[gpui_kit::test]
fn com_a_janela_atras_vira_aviso_do_sistema_e_o_clique_abre_a_conversa(cx: &mut TestAppContext) {
    let e = entrar(cx);
    VisualTestContext::from_window(e.raiz.into(), cx).deactivate_window();

    e.escuta.mandar(evento(
        "ig",
        json!({"tipo": "mensagem_recebida", "contato": "ig-1", "previa": "quero agendar", "em": ha(0)}),
    ));
    e.escuta.mandar(evento(
        "web",
        json!({"tipo": "mensagem_recebida", "contato": "sess-1", "nome": null, "previa": "oi", "em": ha(0)}),
    ));
    e.esperar(cx);
    assert_eq!(
        e.avisador.avisos(),
        vec![
            Aviso {
                titulo: "Instagram Direct".into(),
                corpo: "quero agendar".into(),
                destino: "chatbot:ig:ig-1".into()
            },
            Aviso {
                titulo: "Visitante está no site".into(),
                corpo: "oi".into(),
                destino: "chatbot:web:sess-1".into()
            },
        ]
    );
    assert!(
        toasts(&e, cx).iter().all(|(t, _)| !t.contains("site")),
        "com a janela atrás o aviso é do sistema, não toast"
    );

    // O operador clica no último aviso: o do site.
    e.avisador.clicar_no_ultimo();
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Chatbot));
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.aberta, Some(Chave::nova(Canal::Web, "sess-1")));
        assert!(!t.novidades.contains(&Chave::nova(Canal::Web, "sess-1")));
        assert!(t.novidades.contains(&Chave::nova(Canal::Instagram, "ig-1")));
        assert_eq!(t.conversa_aberta().unwrap().presenca(), "na página agora");
    });
    assert!(caminhos(&e).contains(&"GET /lexdesk/conversas/sess-1/mensagens".to_string()));
}

/// O sino desliga o aviso do sistema — e só ele: a novidade continua acesa.
#[gpui_kit::test]
fn o_sino_desligado_cala_o_aviso_do_sistema_mas_nao_a_novidade(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    chatbot(&e, cx, |t, _w, _cx| {
        assert!(t.avisos_ligados(), "nasce ligado")
    });
    clicar(&e, cx, "chatbot-sino");
    chatbot(&e, cx, |t, _w, _cx| assert!(!t.avisos_ligados()));
    VisualTestContext::from_window(e.raiz.into(), cx).deactivate_window();
    e.escuta.mandar(mensagem_da_ana("alô"));
    e.esperar(cx);
    assert!(e.avisador.avisos().is_empty());
    chatbot(&e, cx, |t, _w, _cx| assert_eq!(t.quantas_novidades(), 1));

    // Ligado de novo, volta a avisar.
    clicar(&e, cx, "chatbot-sino");
    e.escuta.mandar(mensagem_da_ana("alô de novo"));
    e.esperar(cx);
    assert_eq!(e.avisador.avisos().len(), 1);
}

/// A conversa aberta e na frente não avisa nada: o operador está lendo. E
/// uma rajada de eventos vira **uma** releitura.
#[gpui_kit::test]
fn a_conversa_aberta_nao_avisa_e_a_rajada_vira_uma_releitura(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    clicar(&e, cx, "chatbot-conversa-0");
    let antes = leituras_da_lista(&e);
    let toasts_antes = toasts(&e, cx).len();
    for i in 0..5 {
        e.escuta.mandar(mensagem_da_ana(&format!("parte {i}")));
    }
    e.esperar(cx);
    assert_eq!(toasts(&e, cx).len(), toasts_antes, "nenhum toast");
    assert!(e.avisador.avisos().is_empty(), "nenhum aviso");
    chatbot(&e, cx, |t, _w, _cx| assert_eq!(t.quantas_novidades(), 0));
    assert_eq!(
        leituras_da_lista(&e),
        antes + 1,
        "cinco eventos, uma releitura"
    );
}

/// O relógio da releitura, do `canal.tsx` do site: o primeiro `pronto` é a
/// abertura (não relê), o segundo é reconexão (relê); `sincronizar` relê; e a
/// releitura de segurança é de 15 s sem conexão e de 300 s com ela.
#[gpui_kit::test]
fn o_relogio_da_releitura_segue_o_site(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    let mut n = leituras_da_lista(&e);

    e.escuta.mandar(Sinal::Pronto { fonte: "wa" });
    passo(cx);
    assert_eq!(leituras_da_lista(&e), n, "o primeiro pronto é a abertura");

    e.escuta.mandar(Sinal::Pronto { fonte: "wa" });
    passo(cx);
    passo(cx);
    n += 1;
    assert_eq!(leituras_da_lista(&e), n, "reconectou: relê na hora");

    e.escuta.mandar(Sinal::Sincronizar { fonte: "ig" });
    passo(cx);
    passo(cx);
    n += 1;
    assert_eq!(leituras_da_lista(&e), n, "sincronizar relê");

    // Sem as cinco fontes conectadas: a cada 15 s.
    cx.executor().advance_clock(Duration::from_secs(15));
    cx.run_until_parked();
    passo(cx);
    n += 1;
    assert_eq!(leituras_da_lista(&e), n, "15 s sem tempo real");

    // Com as cinco: 15 s não relê, 300 s sim.
    for canal in Canal::TODOS {
        e.escuta.mandar(Sinal::Conexao {
            fonte: canal.prefixo(),
            estado: EstadoDaConexao::Conectado,
        });
    }
    passo(cx);
    cx.executor().advance_clock(Duration::from_secs(16));
    cx.run_until_parked();
    passo(cx);
    assert_eq!(leituras_da_lista(&e), n, "conectado, 15 s não relê");
    cx.executor().advance_clock(Duration::from_secs(300));
    cx.run_until_parked();
    passo(cx);
    assert_eq!(leituras_da_lista(&e), n + 1, "300 s de segurança");
}

/// O indicador é o da pior fonte: "Tempo real" só com as cinco.
#[gpui_kit::test]
fn o_indicador_segue_as_cinco_fontes(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.conexao().rotulo(), "Conectando")
    });
    for canal in &Canal::TODOS[..4] {
        e.escuta.mandar(Sinal::Conexao {
            fonte: canal.prefixo(),
            estado: EstadoDaConexao::Conectado,
        });
    }
    passo(cx);
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.conexao(), EstadoDaConexao::Conectando, "falta uma")
    });
    e.escuta.mandar(Sinal::Conexao {
        fonte: "web",
        estado: EstadoDaConexao::Conectado,
    });
    passo(cx);
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.conexao().rotulo(), "Tempo real")
    });
    e.escuta.mandar(Sinal::Conexao {
        fonte: "tg",
        estado: EstadoDaConexao::Recusado,
    });
    passo(cx);
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.conexao().rotulo(), "Atualizando a cada 15s")
    });
    // Evento torto não derruba nada.
    e.escuta.mandar(evento("wa", json!({"sem": "tipo"})));
    e.escuta.mandar_pela_escuta_de(
        "wa",
        Sinal::Evento {
            fonte: "desconhecida",
            dados: json!({"tipo": "mensagem_recebida", "contato": "x"}),
        },
    );
    passo(cx);
    chatbot(&e, cx, |t, _w, _cx| assert_eq!(t.quantas_novidades(), 0));
}

// ── A conversa ─────────────────────────────────────────────────────────────

/// ⌨️ **Enter envia, Shift+Enter quebra linha** — teclas de verdade no
/// campo. O balão "enviando" aparece na hora, o campo esvazia, e o balão some
/// quando o servidor confirma (e a conversa relê).
#[gpui_kit::test]
fn enter_envia_shift_enter_quebra_linha_e_o_balao_some_quando_volta(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    clicar(&e, cx, "chatbot-conversa-0");
    chatbot(&e, cx, |t, w, cx| {
        t.compositor.update(cx, |c, cx| c.focus(w, cx))
    });
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_input("oi");
    visual.simulate_keystrokes("shift-enter");
    visual.simulate_input("tchau");
    visual.run_until_parked();
    chatbot(&e, cx, |t, _w, cx| {
        assert_eq!(t.compositor.read(cx).value().as_ref(), "oi\ntchau");
    });
    assert!(
        !caminhos(&e).iter().any(|c| c.contains("messages/send")),
        "Shift+Enter não envia"
    );

    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_keystrokes("enter");
    visual.run_until_parked();
    let envio = pedidos(&e)
        .into_iter()
        .find(|p| p.caminho == "/whatsapp/messages/send")
        .expect("o Enter enviou");
    assert_eq!(
        envio.corpo.unwrap(),
        json!({"contact_id": ANA, "message": "oi\ntchau"})
    );
    chatbot(&e, cx, |t, _w, cx| {
        assert_eq!(
            t.compositor.read(cx).value().as_ref(),
            "",
            "o campo esvazia na hora"
        );
        let pendentes = t.pendentes_da_aberta();
        assert_eq!(pendentes.len(), 1, "o balão 'enviando'");
        assert_eq!(pendentes[0].erro, None);
    });
    let antes = leituras_da_lista(&e);
    e.esperar(cx);
    chatbot(&e, cx, |t, _w, _cx| {
        assert!(t.pendentes_da_aberta().is_empty())
    });
    assert_eq!(
        leituras_da_lista(&e),
        antes + 1,
        "a conversa relê depois de enviar"
    );

    // Campo vazio: nem o Enter nem o botão mandam nada.
    let envios = || {
        caminhos(&e)
            .iter()
            .filter(|c| c.contains("messages/send"))
            .count()
    };
    let n = envios();
    clicar(&e, cx, "chatbot-enviar");
    assert_eq!(envios(), n);
}

/// Envio recusado fica no balão, com a frase do servidor, "Tentar de novo" e
/// "Descartar" — nada do que o operador escreveu se perde.
#[gpui_kit::test]
fn envio_recusado_fica_no_balao_com_tentar_de_novo(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    clicar(&e, cx, "chatbot-conversa-0");
    e.site.responder_json(
        "enviada",
        Err("o site respondeu 409: Janela de 24h fechada".into()),
    );
    escrever(&e, cx, "primeira");
    clicar(&e, cx, "chatbot-enviar");
    escrever(&e, cx, "segunda");
    clicar(&e, cx, "chatbot-enviar");
    e.esperar(cx);
    chatbot(&e, cx, |t, _w, _cx| {
        let pendentes = t.pendentes_da_aberta();
        assert_eq!(pendentes.len(), 2);
        assert!(pendentes
            .iter()
            .all(|p| p.erro.as_deref() == Some("Janela de 24h fechada")));
    });

    e.site
        .responder_json("enviada", Ok(json!({"message_id": "wamid.ok"})));
    clicar(&e, cx, "chatbot-tentar-1");
    e.esperar(cx);
    clicar(&e, cx, "chatbot-descartar-2");
    e.esperar(cx);
    chatbot(&e, cx, |t, _w, _cx| {
        assert!(t.pendentes_da_aberta().is_empty())
    });
    let envios: Vec<String> = pedidos(&e)
        .into_iter()
        .filter(|p| p.caminho == "/whatsapp/messages/send")
        .map(|p| p.corpo.unwrap()["message"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        envios,
        ["primeira", "segunda", "primeira"],
        "só a primeira foi de novo"
    );
}

/// ⚡ A resposta rápida sai com um clique, pela rota do canal, e conta um uso.
#[gpui_kit::test]
fn a_resposta_rapida_sai_e_conta_um_uso(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    clicar(&e, cx, "chatbot-conversa-1"); // a Bia, no Instagram
    e.esperar(cx);
    clicar(&e, cx, "chatbot-raio");
    chatbot(&e, cx, |t, _w, _cx| assert!(t.respostas_abertas));
    clicar(&e, cx, "chatbot-resposta-0");
    e.esperar(cx);
    let c = caminhos(&e);
    let envio = c
        .iter()
        .position(|p| p == "POST /instagram/mensagens")
        .expect("enviou");
    let uso = c
        .iter()
        .position(|p| p == "POST /whatsapp/quick-responses/r1/increment-usage")
        .expect("contou o uso");
    assert!(uso > envio, "o uso conta depois de a mensagem sair");
    let corpo = pedidos(&e)
        .into_iter()
        .find(|p| p.caminho == "/instagram/mensagens")
        .unwrap()
        .corpo
        .unwrap();
    assert_eq!(
        corpo,
        json!({"contato": "ig-1", "texto": "Rua Coberta, 12 — Gramado"})
    );
    chatbot(&e, cx, |t, _w, _cx| {
        assert!(!t.respostas_abertas, "o painel fecha")
    });
}

/// "Carregar mensagens anteriores (N)": 60 mensagens mostram 50, e o botão
/// traz as 10.
#[gpui_kit::test]
fn carregar_anteriores_mostra_o_historico_inteiro(cx: &mut TestAppContext) {
    let e = entrar(cx);
    e.site.responder_json(
        "wa-conversas",
        Ok(pagina(
            vec![conversa_da_ana(false)],
            mensagens_da_ana(60),
            1,
        )),
    );
    clicar(&e, cx, "menu-Chatbot");
    e.esperar(cx);
    clicar(&e, cx, "chatbot-conversa-0");
    chatbot(&e, cx, |t, _w, _cx| {
        let (visiveis, escondidas) = t.mensagens_da_aberta();
        assert_eq!((visiveis.len(), escondidas), (50, 10));
    });
    // O histórico abre rolado até o fim, com o botão lá em cima, fora de
    // vista: o operador rola até ele, como no site.
    chatbot(&e, cx, |t, _w, _cx| {
        t.rolagem
            .set_offset(gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.)))
    });
    clicar(&e, cx, "chatbot-anteriores");
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.mensagens_da_aberta().0.len(), 60)
    });
    // Trocar de conversa volta ao corte.
    clicar(&e, cx, "chatbot-conversa-1");
    clicar(&e, cx, "chatbot-conversa-0");
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.mensagens_da_aberta().1, 10)
    });
}

/// 🕐 **A janela de 24 h fechada tira o compositor** e põe o motivo no lugar —
/// e o gesto de enviar também não passa por baixo.
#[gpui_kit::test]
fn a_janela_fechada_troca_o_compositor_pelo_motivo(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    e.site.responder_json(
        "historico",
        Ok(json!([{"id": "i1", "message_id": "a", "direcao": "INBOUND", "conteudo": "oi", "em": ha(60 * 30)}])),
    );
    clicar(&e, cx, "chatbot-conversa-1"); // Instagram
    e.esperar(cx);
    chatbot(&e, cx, |t, _w, _cx| {
        assert!(matches!(
            t.janela_da_aberta(chrono::Utc::now()),
            Janela::Fechada { .. }
        ));
    });
    chatbot(&e, cx, |t, _w, _cx| {
        let decisao = crate::chatbot::modelo::decisao_do_compositor(
            Canal::Instagram,
            t.janela_da_aberta(chrono::Utc::now()),
        );
        assert!(decisao.bloqueado, "o compositor sai e dá lugar ao motivo");
        assert!(decisao
            .motivo
            .unwrap()
            .starts_with("A janela de 24 horas da Meta fechou"));
    });
    escrever(&e, cx, "tenta assim mesmo");
    chatbot(&e, cx, |t, w, cx| t.enviar_o_escrito(w, cx));
    e.esperar(cx);
    assert!(!caminhos(&e)
        .iter()
        .any(|c| c == "POST /instagram/mensagens"));

    // O site não é da Meta: sem janela.
    clicar(&e, cx, "chatbot-conversa-2");
    e.esperar(cx);
    assert!(desenhado(&e, cx, "chatbot-enviar"));
}

/// ✋ Assumir e devolver no WhatsApp: pausa e retoma o bot, com o toast do
/// site; se o servidor recusar, o botão volta ao que era.
#[gpui_kit::test]
fn assumir_e_devolver_no_whatsapp_e_a_volta_quando_o_servidor_recusa(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    clicar(&e, cx, "chatbot-conversa-0");
    // O servidor passa a responder a Ana como pausada, como faria de verdade.
    e.site.responder_json(
        "wa-conversas",
        Ok(pagina(vec![conversa_da_ana(true)], mensagens_da_ana(2), 1)),
    );
    clicar(&e, cx, "chatbot-alternar");
    chatbot(&e, cx, |t, _w, _cx| {
        assert!(
            t.conversa_aberta().unwrap().atendimento_humano,
            "otimista: na hora"
        )
    });
    e.esperar(cx);
    let pausa = pedidos(&e)
        .into_iter()
        .find(|p| p.caminho == "/whatsapp/bot-pause/pause")
        .expect("pausou");
    assert_eq!(pausa.corpo.unwrap()["paused_by"], "dashboard_admin");
    assert!(
        toasts(&e, cx).contains(&("Você assumiu a conversa — o bot está calado.".into(), false))
    );
    chatbot(&e, cx, |t, _w, _cx| {
        assert!(t.conversa_aberta().unwrap().atendimento_humano);
        assert_eq!(
            t.linha_de_resumo().as_ref(),
            "3 conversas · 2 com bot · 1 em atendimento"
        );
    });

    e.site
        .responder_json("devolvida", Err("o site respondeu 500: pane".into()));
    clicar(&e, cx, "chatbot-alternar");
    e.esperar(cx);
    assert!(caminhos(&e).contains(&"POST /whatsapp/bot-pause/resume".to_string()));
    assert!(toasts(&e, cx).contains(&("Erro ao executar ação".into(), true)));
    chatbot(&e, cx, |t, _w, _cx| {
        assert!(
            t.conversa_aberta().unwrap().atendimento_humano,
            "voltou ao que era"
        )
    });
}

/// 🙋 Assumir no site pergunta quem é, manda o nome, e lembra dele na
/// próxima. Cancelar não manda nada.
#[gpui_kit::test]
fn assumir_no_site_pergunta_quem_e_e_lembra_o_nome(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    clicar(&e, cx, "chatbot-conversa-2");
    e.esperar(cx);
    clicar(&e, cx, "chatbot-alternar");
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(
            t.dialogo.aberto().cloned(),
            Some(Dialogo::QuemAssume(Chave::nova(Canal::Web, "sess-1")))
        )
    });
    clicar(&e, cx, "quem-cancelar");
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.dialogo.aberto().cloned(), None)
    });
    assert!(!caminhos(&e).iter().any(|c| c.contains("/assumir")));

    clicar(&e, cx, "chatbot-alternar");
    chatbot(&e, cx, |t, w, cx| {
        t.nome_do_atendente
            .update(cx, |c, cx| c.trocar_valor("  Ana Paula ", w, cx))
    });
    clicar(&e, cx, "quem-assumir");
    e.esperar(cx);
    let assumida = pedidos(&e)
        .into_iter()
        .find(|p| p.caminho == "/lexdesk/conversas/sess-1/assumir")
        .expect("assumiu");
    assert_eq!(assumida.corpo.unwrap(), json!({"atendente": "Ana Paula"}));
    chatbot(&e, cx, |t, _w, cx| {
        assert_eq!(t.dialogo.aberto().cloned(), None);
        assert_eq!(t.preferencias.atendente, "Ana Paula");
        // A leitura voltou com o site dizendo "não assumida": o botão é Assumir.
        assert!(!t.conversa_aberta().unwrap().atendimento_humano);
        let _ = cx;
    });

    // Na próxima, o nome já vem escrito.
    clicar(&e, cx, "chatbot-alternar");
    chatbot(&e, cx, |t, _w, cx| {
        assert_eq!(t.nome_do_atendente.read(cx).value().as_ref(), "Ana Paula")
    });
}

/// 📋 "Copiar IGSID" copia o id do canal, com a frase do site; e o Instagram
/// não oferece "Excluir histórico", que só existe no WhatsApp.
#[gpui_kit::test]
fn mais_acoes_copia_o_id_e_so_o_whatsapp_exclui(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    clicar(&e, cx, "chatbot-conversa-1");
    e.esperar(cx);
    clicar(&e, cx, "chatbot-mais-acoes");
    assert!(!desenhado(&e, cx, "chatbot-excluir-historico"));
    clicar(&e, cx, "chatbot-copiar");
    assert_eq!(
        cx.read_from_clipboard().and_then(|c| c.text()).as_deref(),
        Some("ig-1")
    );
    assert!(toasts(&e, cx).contains(&("IGSID copiado.".into(), false)));
    chatbot(&e, cx, |t, _w, _cx| assert!(!t.mais_acoes, "o menu fecha"));
}

/// 🗑️ **Excluir histórico só com `QUERO EXCLUIR!` letra por letra** — o
/// botão apagado não passa, a frase quase certa não passa.
#[gpui_kit::test]
fn excluir_historico_so_com_a_frase_letra_por_letra(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    clicar(&e, cx, "chatbot-conversa-0");
    clicar(&e, cx, "chatbot-mais-acoes");
    clicar(&e, cx, "chatbot-excluir-historico");
    chatbot(&e, cx, |t, _w, _cx| {
        assert!(matches!(
            t.dialogo.aberto(),
            Some(Dialogo::ExcluirHistorico(_))
        ))
    });
    let apagou = |e: &Estudio| caminhos(e).iter().any(|c| c.starts_with("DELETE"));

    clicar(&e, cx, "excluir-confirmar");
    assert!(!apagou(&e), "vazio não apaga");
    for quase in ["quero excluir!", "QUERO EXCLUIR", " QUERO EXCLUIR!"] {
        chatbot(&e, cx, |t, w, cx| {
            t.confirmacao
                .update(cx, |c, cx| c.trocar_valor(quase, w, cx))
        });
        clicar(&e, cx, "excluir-confirmar");
        assert!(!apagou(&e), "{quase:?} não apaga");
    }
    chatbot(&e, cx, |t, w, cx| {
        t.confirmacao
            .update(cx, |c, cx| c.trocar_valor("QUERO EXCLUIR!", w, cx))
    });
    clicar(&e, cx, "excluir-confirmar");
    e.esperar(cx);
    assert!(caminhos(&e).contains(&format!("DELETE /whatsapp/messages/recipient/{ANA}")));
    assert!(toasts(&e, cx).contains(&(
        "Histórico de mensagens excluído com sucesso (7 mensagens)".into(),
        false
    )));
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.dialogo.aberto().cloned(), None)
    });
}

// ── Urgências ──────────────────────────────────────────────────────────────

/// 🚨 A lista de urgências inteira: assumir, resolver (as notas são
/// obrigatórias), descartar (com a pergunta, que cancela de volta à lista) e
/// "Ver conversa", que abre quem está fora da página.
#[gpui_kit::test]
fn urgencias_assumir_resolver_descartar_e_ver_a_conversa(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    clicar(&e, cx, "chatbot-urgentes");
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.dialogo.aberto().cloned(), Some(Dialogo::Urgencias))
    });

    clicar(&e, cx, "urgencia-assumir-0");
    e.esperar(cx);
    let assumida = pedidos(&e)
        .into_iter()
        .find(|p| p.metodo == "PATCH")
        .expect("assumiu");
    assert_eq!(assumida.caminho, "/crm/urgent/u1/status");
    assert_eq!(assumida.corpo.unwrap(), json!({"status": "IN_PROGRESS"}));
    assert!(toasts(&e, cx).contains(&("Marcado como em atendimento".into(), false)));

    // Resolver sem notas não sai.
    clicar(&e, cx, "urgencia-resolver-0");
    clicar(&e, cx, "resolver-confirmar");
    e.esperar(cx);
    let patches = |e: &Estudio| {
        pedidos(e)
            .into_iter()
            .filter(|p| p.metodo == "PATCH")
            .count()
    };
    assert_eq!(patches(&e), 1);
    chatbot(&e, cx, |t, w, cx| {
        t.notas
            .update(cx, |c, cx| c.trocar_valor("cliente atendido", w, cx))
    });
    clicar(&e, cx, "resolver-confirmar");
    e.esperar(cx);
    let resolvida = pedidos(&e)
        .into_iter()
        .filter(|p| p.metodo == "PATCH")
        .nth(1)
        .unwrap();
    assert_eq!(
        resolvida.corpo.unwrap(),
        json!({"status": "RESOLVED", "notes": "cliente atendido"})
    );
    assert!(toasts(&e, cx).contains(&("Urgência resolvida com sucesso!".into(), false)));
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(
            t.dialogo.aberto().cloned(),
            Some(Dialogo::Urgencias),
            "volta à lista"
        )
    });

    // Descartar pergunta; Cancelar volta à lista sem mandar nada.
    clicar(&e, cx, "urgencia-descartar-0");
    clicar(&e, cx, "descartar-cancelar");
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.dialogo.aberto().cloned(), Some(Dialogo::Urgencias))
    });
    assert_eq!(patches(&e), 2);
    clicar(&e, cx, "urgencia-descartar-0");
    clicar(&e, cx, "descartar-confirmar");
    e.esperar(cx);
    let descartada = pedidos(&e)
        .into_iter()
        .filter(|p| p.metodo == "PATCH")
        .nth(2)
        .unwrap();
    assert_eq!(
        descartada.corpo.unwrap(),
        json!({"status": "DISMISSED", "notes": "Falso positivo - descartado pelo admin"})
    );

    // "Ver conversa": o Caio não está na página — a busca dirigida o traz.
    clicar(&e, cx, "urgencia-ver-0");
    e.esperar(cx);
    assert!(caminhos(&e).contains(&format!(
        "GET /whatsapp/conversations?contact_filter={CAIO}&show_automated=true&page=1&limit=1"
    )));
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.dialogo.aberto().cloned(), None);
        assert_eq!(t.aberta, Some(Chave::nova(Canal::WhatsApp, CAIO)));
        let aberta = t.conversa_aberta().unwrap();
        assert_eq!(aberta.nome, "Caio");
        assert_eq!(t.mensagens_da_aberta().0.len(), 1);
        assert!(
            t.visiveis().iter().all(|c| c.chave.id != CAIO),
            "aberta, mas fora da lista: a lista é a página pedida"
        );
    });
}

/// O véu fecha o diálogo; o clique dentro da caixa, não.
#[gpui_kit::test]
fn o_veu_fecha_o_dialogo_e_a_caixa_nao(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    clicar(&e, cx, "chatbot-urgentes");
    let mut visual = quadro_novo(&e, cx);
    let caixa = visual
        .debug_bounds("urgencias-fechar")
        .expect("a caixa está na tela");
    // Um pouco acima do botão "Fechar": ainda dentro da caixa.
    visual.simulate_click(
        gpui_kit::point(caixa.center().x, caixa.origin.y - gpui_kit::px(20.)),
        Modifiers::none(),
    );
    visual.run_until_parked();
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.dialogo.aberto().cloned(), Some(Dialogo::Urgencias))
    });
    // Abaixo da caixa ainda é o véu (ele cobre a área do chatbot; o menu
    // lateral fica fora, como nas outras telas do app).
    let mut visual = quadro_novo(&e, cx);
    let fechar = visual.debug_bounds("urgencias-fechar").unwrap();
    visual.simulate_click(
        gpui_kit::point(
            fechar.center().x,
            fechar.origin.y + fechar.size.height + gpui_kit::px(60.),
        ),
        Modifiers::none(),
    );
    visual.run_until_parked();
    chatbot(&e, cx, |t, _w, _cx| {
        assert_eq!(t.dialogo.aberto().cloned(), None)
    });
}

/// 🚪 Sair da conta fecha os fluxos e esquece as conversas; entrar de novo
/// escuta de novo.
#[gpui_kit::test]
fn sair_da_conta_fecha_os_fluxos_e_esquece_as_conversas(cx: &mut TestAppContext) {
    let e = abrir_o_chatbot(cx);
    e.escuta.mandar(evento(
        "ig",
        json!({"tipo": "mensagem_recebida", "contato": "ig-9", "previa": "oi"}),
    ));
    e.esperar(cx);
    e.app(cx, |app, _w, cx| app.sair_da_conta(cx));
    e.esperar(cx);
    assert_eq!(
        e.escuta.largadas.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "as guardas do chatbot e da agenda caíram"
    );
    chatbot(&e, cx, |t, _w, _cx| {
        assert!(t.todas().is_empty());
        assert_eq!(t.quantas_novidades(), 0);
        assert!(t.urgencias.is_empty());
        assert!(!t.visivel);
    });
    e.app(cx, |app, _w, cx| app.entrar_na_conta(super::sessao(), cx));
    e.esperar(cx);
    assert_eq!(e.escuta.abertas(), 4, "entrou de novo: mais duas");
}
