//! Os pedidos que o painel faz à API — cada gesto do site, com o mesmo
//! caminho, método e corpo (`lib/api/{whatsapp,instagram,lexdesk,telegram,
//! messenger}.ts` e `actions.ts`).
//!
//! 🔑 **Montados aqui, e não na tela**, para o teste conferir a rota de cada
//! canal sem abrir janela: é onde os cinco canais divergem de verdade — o
//! WhatsApp "pausa o bot", os outros "assumem a conversa", e o site ainda
//! pede o nome de quem assume.

use serde_json::json;

use super::modelo::{Canal, Chave, Status};
use crate::pos_venda::porta::PedidoJson;

/// Conversas por página do WhatsApp — o `POR_PAGINA` do site.
pub const POR_PAGINA: u32 = 15;

/// `encodeURIComponent`: tudo que não é letra, número ou `-_.!~*'()` vira
/// `%XX` (em UTF-8).
pub fn codificar(texto: &str) -> String {
    let mut fora = String::with_capacity(texto.len());
    for byte in texto.as_bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => fora.push(*byte as char),
            outro => fora.push_str(&format!("%{outro:02X}")),
        }
    }
    fora
}

/// `GET /whatsapp/conversations` — a página, com as mensagens dela.
///
/// `show_automated=true` sempre: o site parou de esconder as automáticas
/// (dono, 2026-09-14: *"quero que ele veja"*).
pub fn conversas_do_whatsapp(status: Status, busca: &str, pagina: u32) -> PedidoJson {
    let mut query = Vec::new();
    let busca = busca.trim();
    if !busca.is_empty() {
        query.push(format!("search_query={}", codificar(busca)));
    }
    if let Some(filtro) = status.para_o_whatsapp() {
        query.push(format!("status_filter={filtro}"));
    }
    query.push("show_automated=true".into());
    query.push(format!("page={}", pagina.max(1)));
    query.push(format!("limit={POR_PAGINA}"));
    PedidoJson::ler(
        "wa-conversas",
        format!("/whatsapp/conversations?{}", query.join("&")),
    )
}

/// A conversa aberta que não está na página listada (veio de um aviso ou de
/// uma urgência): uma busca dirigida ao contato, sem filtro que a esconda.
pub fn conversa_fora_da_pagina(contato: &str) -> PedidoJson {
    PedidoJson::ler(
        "wa-fora",
        format!(
            "/whatsapp/conversations?contact_filter={}&show_automated=true&page=1&limit=1",
            codificar(contato)
        ),
    )
}

/// O rótulo da lista de cada um dos outros canais.
pub fn rotulo_da_lista(canal: Canal) -> &'static str {
    match canal {
        Canal::WhatsApp => "wa-conversas",
        Canal::Instagram => "ig-conversas",
        Canal::Web => "web-conversas",
        Canal::Telegram => "tg-conversas",
        Canal::Messenger => "ms-conversas",
    }
}

/// O canal de um rótulo de lista (o caminho de volta de [`rotulo_da_lista`]).
pub fn canal_da_lista(rotulo: &str) -> Option<Canal> {
    Canal::TODOS
        .into_iter()
        .find(|c| *c != Canal::WhatsApp && rotulo_da_lista(*c) == rotulo)
}

/// `GET /<canal>/conversas` — a lista inteira de um dos outros canais.
pub fn conversas_do_canal(canal: Canal) -> PedidoJson {
    PedidoJson::ler(
        rotulo_da_lista(canal),
        format!("{}/conversas", canal.raiz()),
    )
}

/// `GET /<canal>/conversas/{contato}/mensagens` — o histórico de quem abriu.
pub fn historico(chave: &Chave) -> PedidoJson {
    PedidoJson::ler(
        "historico",
        format!(
            "{}/conversas/{}/mensagens",
            chave.canal.raiz(),
            codificar(&chave.id)
        ),
    )
}

/// O voucher de cada contato da página — POST porque são quinze números.
pub fn vouchers(contatos: &[String]) -> PedidoJson {
    PedidoJson::gravar(
        "vouchers",
        "POST",
        "/whatsapp/vouchers/info",
        json!({ "contact_ids": contatos }),
    )
}

/// O cadastro no site de cada contato da página.
pub fn cadastros(contatos: &[String]) -> PedidoJson {
    PedidoJson::gravar(
        "cadastros",
        "POST",
        "/crm/contacts/by-whatsapp",
        json!({ "numeros": contatos }),
    )
}

pub fn respostas_rapidas() -> PedidoJson {
    PedidoJson::ler("respostas", "/whatsapp/quick-responses")
}

pub fn urgencias() -> PedidoJson {
    PedidoJson::ler("urgencias", "/crm/urgent")
}

/// "Enviar": o WhatsApp tem rota própria; os outros quatro, `POST
/// /<canal>/mensagens {contato, texto}`.
pub fn enviar(chave: &Chave, texto: &str) -> PedidoJson {
    match chave.canal {
        Canal::WhatsApp => PedidoJson::gravar(
            "enviada",
            "POST",
            "/whatsapp/messages/send",
            json!({ "contact_id": chave.id, "message": texto }),
        ),
        canal => PedidoJson::gravar(
            "enviada",
            "POST",
            format!("{}/mensagens", canal.raiz()),
            json!({ "contato": chave.id, "texto": texto }),
        ),
    }
}

/// A resposta rápida conta um uso depois de sair.
pub fn contar_uso(resposta_id: &str) -> PedidoJson {
    PedidoJson::gravar(
        "uso-contado",
        "POST",
        format!(
            "/whatsapp/quick-responses/{}/increment-usage",
            codificar(resposta_id)
        ),
        json!({}),
    )
}

/// "Assumir" (`atender = true`) e "Devolver ao bot".
///
/// | Canal | Assumir | Devolver |
/// |---|---|---|
/// | WhatsApp | `POST /whatsapp/bot-pause/pause` | `…/resume` |
/// | Site | `POST /lexdesk/conversas/{c}/assumir {atendente}` | `…/devolver` |
/// | Os outros | `POST /<canal>/conversas/{c}/assumir` | `…/devolver` |
///
/// `paused_by`/`resumed_by` vão como `dashboard_admin`, o mesmo valor do
/// site: o backend e os relatórios não distinguem de qual tela veio.
pub fn alternar(chave: &Chave, atender: bool, atendente: Option<&str>) -> PedidoJson {
    let rotulo = if atender { "assumida" } else { "devolvida" };
    match chave.canal {
        Canal::WhatsApp if atender => PedidoJson::gravar(
            rotulo,
            "POST",
            "/whatsapp/bot-pause/pause",
            json!({
                "contact_id": chave.id,
                "paused_by": "dashboard_admin",
                "reason": "Pausado via dashboard para atendimento manual",
            }),
        ),
        Canal::WhatsApp => PedidoJson::gravar(
            rotulo,
            "POST",
            "/whatsapp/bot-pause/resume",
            json!({ "contact_id": chave.id, "resumed_by": "dashboard_admin" }),
        ),
        canal => {
            let acao = if atender { "assumir" } else { "devolver" };
            let corpo = match (canal, atender) {
                (Canal::Web, true) => json!({ "atendente": atendente.unwrap_or("").trim() }),
                _ => json!({}),
            };
            PedidoJson::gravar(
                rotulo,
                "POST",
                format!("{}/conversas/{}/{acao}", canal.raiz(), codificar(&chave.id)),
                corpo,
            )
        }
    }
}

/// "Excluir histórico" — só WhatsApp, e só depois do `QUERO EXCLUIR!`.
pub fn apagar_historico(contato: &str) -> PedidoJson {
    PedidoJson {
        rotulo: "historico-apagado",
        metodo: "DELETE",
        caminho: format!("/whatsapp/messages/recipient/{}", codificar(contato)),
        corpo: None,
    }
}

/// A frase que o operador digita antes de apagar — a do site, letra por letra.
pub const CONFIRMACAO_DE_EXCLUSAO: &str = "QUERO EXCLUIR!";

/// As três mudanças de urgência: "Assumir", "Resolver" (com as notas) e
/// "Descartar" (falso positivo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MudancaDaUrgencia {
    Assumir,
    Resolver,
    Descartar,
}

pub fn mudar_urgencia(id: &str, mudanca: MudancaDaUrgencia, notas: &str) -> PedidoJson {
    let (rotulo, corpo) = match mudanca {
        MudancaDaUrgencia::Assumir => ("urgencia-assumida", json!({ "status": "IN_PROGRESS" })),
        MudancaDaUrgencia::Resolver => (
            "urgencia-resolvida",
            json!({ "status": "RESOLVED", "notes": notas.trim() }),
        ),
        MudancaDaUrgencia::Descartar => (
            "urgencia-descartada",
            json!({ "status": "DISMISSED", "notes": "Falso positivo - descartado pelo admin" }),
        ),
    };
    PedidoJson::gravar(
        rotulo,
        "PATCH",
        format!("/crm/urgent/{}/status", codificar(id)),
        corpo,
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn codifica_como_o_encode_uri_component() {
        assert_eq!(codificar("João Silva"), "Jo%C3%A3o%20Silva");
        assert_eq!(codificar("a&b=c/d?"), "a%26b%3Dc%2Fd%3F");
        assert_eq!(codificar("5554-_.!~*'()"), "5554-_.!~*'()");
    }

    #[test]
    fn a_pagina_do_whatsapp_manda_so_o_que_tem() {
        let p = conversas_do_whatsapp(Status::Todas, "  ", 1);
        assert_eq!(p.rotulo, "wa-conversas");
        assert_eq!(p.metodo, "GET");
        assert_eq!(
            p.caminho,
            "/whatsapp/conversations?show_automated=true&page=1&limit=15"
        );
        let p = conversas_do_whatsapp(Status::Humano, " maria josé ", 3);
        assert_eq!(
            p.caminho,
            "/whatsapp/conversations?search_query=maria%20jos%C3%A9&status_filter=manual\
             &show_automated=true&page=3&limit=15"
        );
        assert!(conversas_do_whatsapp(Status::Bot, "", 0)
            .caminho
            .contains("page=1"));
        assert_eq!(
            conversa_fora_da_pagina("5554").caminho,
            "/whatsapp/conversations?contact_filter=5554&show_automated=true&page=1&limit=1"
        );
    }

    #[test]
    fn cada_canal_lista_e_le_o_historico_na_raiz_dele() {
        for (canal, lista, historico_) in [
            (
                Canal::Instagram,
                "/instagram/conversas",
                "/instagram/conversas/ig%2F1/mensagens",
            ),
            (
                Canal::Web,
                "/lexdesk/conversas",
                "/lexdesk/conversas/ig%2F1/mensagens",
            ),
            (
                Canal::Telegram,
                "/telegram/conversas",
                "/telegram/conversas/ig%2F1/mensagens",
            ),
            (
                Canal::Messenger,
                "/messenger/conversas",
                "/messenger/conversas/ig%2F1/mensagens",
            ),
        ] {
            let p = conversas_do_canal(canal);
            assert_eq!(p.caminho, lista);
            assert_eq!(canal_da_lista(p.rotulo), Some(canal));
            assert_eq!(historico(&Chave::nova(canal, "ig/1")).caminho, historico_);
        }
        assert_eq!(
            canal_da_lista("wa-conversas"),
            None,
            "o WhatsApp tem leitura própria"
        );
    }

    #[test]
    fn enviar_segue_o_contrato_de_cada_canal() {
        let wa = enviar(&Chave::nova(Canal::WhatsApp, "5554"), "oi");
        assert_eq!(
            (wa.metodo, wa.caminho.as_str()),
            ("POST", "/whatsapp/messages/send")
        );
        assert_eq!(
            wa.corpo.unwrap(),
            json!({"contact_id": "5554", "message": "oi"})
        );
        let tg = enviar(&Chave::nova(Canal::Telegram, "99"), "olá");
        assert_eq!(tg.caminho, "/telegram/mensagens");
        assert_eq!(tg.corpo.unwrap(), json!({"contato": "99", "texto": "olá"}));
        assert_eq!(
            enviar(&Chave::nova(Canal::Web, "s"), "x").caminho,
            "/lexdesk/mensagens"
        );
        assert_eq!(
            contar_uso("r 1").caminho,
            "/whatsapp/quick-responses/r%201/increment-usage"
        );
    }

    #[test]
    fn assumir_e_devolver_falam_a_lingua_de_cada_canal() {
        let wa = Chave::nova(Canal::WhatsApp, "5554");
        let pausa = alternar(&wa, true, None);
        assert_eq!(pausa.caminho, "/whatsapp/bot-pause/pause");
        assert_eq!(
            pausa.corpo.unwrap(),
            json!({"contact_id": "5554", "paused_by": "dashboard_admin",
                   "reason": "Pausado via dashboard para atendimento manual"})
        );
        let volta = alternar(&wa, false, None);
        assert_eq!(volta.caminho, "/whatsapp/bot-pause/resume");
        assert_eq!(
            volta.corpo.unwrap(),
            json!({"contact_id": "5554", "resumed_by": "dashboard_admin"})
        );

        let site = Chave::nova(Canal::Web, "sess 1");
        let assumida = alternar(&site, true, Some("  Ana "));
        assert_eq!(assumida.caminho, "/lexdesk/conversas/sess%201/assumir");
        assert_eq!(assumida.corpo.unwrap(), json!({"atendente": "Ana"}));
        assert_eq!(
            alternar(&site, false, None).caminho,
            "/lexdesk/conversas/sess%201/devolver"
        );

        let ig = alternar(&Chave::nova(Canal::Instagram, "i"), true, Some("ignorado"));
        assert_eq!(ig.caminho, "/instagram/conversas/i/assumir");
        assert_eq!(ig.corpo.unwrap(), json!({}), "só o site leva o nome");
        assert_eq!(ig.rotulo, "assumida");
        assert_eq!(
            alternar(&Chave::nova(Canal::Messenger, "m"), false, None).rotulo,
            "devolvida"
        );
    }

    #[test]
    fn as_urgencias_e_a_exclusao() {
        let a = mudar_urgencia("u1", MudancaDaUrgencia::Assumir, "");
        assert_eq!(
            (a.metodo, a.caminho.as_str()),
            ("PATCH", "/crm/urgent/u1/status")
        );
        assert_eq!(a.corpo.unwrap(), json!({"status": "IN_PROGRESS"}));
        let r = mudar_urgencia("u1", MudancaDaUrgencia::Resolver, "  resolvido \n");
        assert_eq!(
            r.corpo.unwrap(),
            json!({"status": "RESOLVED", "notes": "resolvido"})
        );
        let d = mudar_urgencia("u1", MudancaDaUrgencia::Descartar, "");
        assert_eq!(
            d.corpo.unwrap(),
            json!({"status": "DISMISSED", "notes": "Falso positivo - descartado pelo admin"})
        );
        let x = apagar_historico("5554");
        assert_eq!(
            (x.metodo, x.caminho.as_str()),
            ("DELETE", "/whatsapp/messages/recipient/5554")
        );
        assert!(x.corpo.is_none());
        assert_eq!(CONFIRMACAO_DE_EXCLUSAO, "QUERO EXCLUIR!");
        assert_eq!(
            vouchers(&["1".into()]).corpo.unwrap(),
            json!({"contact_ids": ["1"]})
        );
        assert_eq!(
            cadastros(&["1".into()]).corpo.unwrap(),
            json!({"numeros": ["1"]})
        );
    }
}
