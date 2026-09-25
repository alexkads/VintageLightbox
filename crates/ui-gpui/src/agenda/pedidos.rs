//! Os pedidos da agenda à API — os de `lib/api/booking.ts` do site, com o
//! mesmo caminho, método e corpo.

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use serde_json::{json, Map, Value};

use crate::chatbot::pedidos::codificar;
use crate::pos_venda::porta::PedidoJson;

/// Os estúdios do seletor.
pub fn estudios() -> PedidoJson {
    PedidoJson::ler("estudios", "/bookings/studios")
}

fn intervalo(
    rotulo: &'static str,
    de: NaiveDate,
    ate: NaiveDate,
    estudio: Option<&str>,
) -> PedidoJson {
    let mut caminho = format!(
        "/bookings/agenda?from={}&to={}",
        de.format("%Y-%m-%d"),
        ate.format("%Y-%m-%d")
    );
    if let Some(estudio) = estudio {
        caminho.push_str(&format!("&studio_id={}", codificar(estudio)));
    }
    PedidoJson::ler(rotulo, caminho)
}

/// O período que o calendário mostra.
pub fn agenda(de: NaiveDate, ate: NaiveDate, estudio: Option<&str>) -> PedidoJson {
    intervalo("agenda", de, ate, estudio)
}

/// "Agendamentos de hoje" — pedido à parte, porque o calendário pode estar
/// em outro mês.
pub fn hoje(dia: NaiveDate, estudio: Option<&str>) -> PedidoJson {
    intervalo("hoje", dia, dia, estudio)
}

/// Os indicadores — sem estúdio: somam todos, como no site.
pub fn indicadores() -> PedidoJson {
    PedidoJson::ler("indicadores", "/bookings/stats")
}

fn utc(instante: DateTime<Utc>) -> String {
    instante.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// "Confirmar reagendamento".
pub fn reagendar(id: &str, inicio: DateTime<Utc>, fim: DateTime<Utc>) -> PedidoJson {
    PedidoJson::gravar(
        "reagendado",
        "PATCH",
        format!("/bookings/{}/reschedule", codificar(id)),
        json!({ "start_time": utc(inicio), "end_time": utc(fim) }),
    )
}

/// O que o "Registrar atendimento" manda. Campo vazio **não vai**: o
/// backend guarda o que já tinha.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Atendimento {
    pub valor_pago: Option<f64>,
    pub hora_entrada: Option<DateTime<Utc>>,
    pub hora_saida: Option<DateTime<Utc>>,
    pub observacoes: Option<String>,
}

/// "Confirmar atendimento" — o backend marca como concluído.
pub fn atendimento(id: &str, dados: &Atendimento) -> PedidoJson {
    let mut corpo = Map::new();
    if let Some(valor) = dados.valor_pago {
        corpo.insert("valor_pago".into(), json!(valor));
    }
    if let Some(entrada) = dados.hora_entrada {
        corpo.insert("hora_entrada".into(), json!(utc(entrada)));
    }
    if let Some(saida) = dados.hora_saida {
        corpo.insert("hora_saida".into(), json!(utc(saida)));
    }
    if let Some(obs) = dados
        .observacoes
        .as_deref()
        .map(str::trim)
        .filter(|o| !o.is_empty())
    {
        corpo.insert("observacoes_atendimento".into(), json!(obs));
    }
    PedidoJson::gravar(
        "atendido",
        "PATCH",
        format!("/bookings/{}/attendance", codificar(id)),
        Value::Object(corpo),
    )
}

/// "Descancelar". A resposta é `{id, status}`, e não o agendamento inteiro.
pub fn descancelar(id: &str) -> PedidoJson {
    PedidoJson::gravar(
        "restaurado",
        "POST",
        format!("/bookings/{}/uncancel", codificar(id)),
        json!({}),
    )
}

/// "Excluir" — `/bookings/agenda/{id}`, a rota do painel, e não
/// `/bookings/{id}`.
pub fn excluir(id: &str) -> PedidoJson {
    PedidoJson {
        rotulo: "excluido",
        metodo: "DELETE",
        caminho: format!("/bookings/agenda/{}", codificar(id)),
        corpo: None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn d(a: i32, m: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, dia).unwrap()
    }

    fn t(texto: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(texto)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn as_leituras() {
        assert_eq!(estudios().caminho, "/bookings/studios");
        assert_eq!(
            agenda(d(2026, 8, 25), d(2026, 10, 7), None).caminho,
            "/bookings/agenda?from=2026-08-25&to=2026-10-07"
        );
        let hoje = hoje(d(2026, 9, 25), Some("estúdio 1"));
        assert_eq!(hoje.rotulo, "hoje");
        assert_eq!(
            hoje.caminho,
            "/bookings/agenda?from=2026-09-25&to=2026-09-25&studio_id=est%C3%BAdio%201"
        );
        assert_eq!(indicadores().caminho, "/bookings/stats");
    }

    #[test]
    fn as_escritas_seguem_o_site() {
        let r = reagendar("e1", t("2026-09-25T17:00:00Z"), t("2026-09-25T18:30:00Z"));
        assert_eq!(
            (r.metodo, r.caminho.as_str()),
            ("PATCH", "/bookings/e1/reschedule")
        );
        assert_eq!(
            r.corpo.unwrap(),
            json!({"start_time": "2026-09-25T17:00:00Z", "end_time": "2026-09-25T18:30:00Z"})
        );

        let a = atendimento(
            "e1",
            &Atendimento {
                valor_pago: Some(250.5),
                hora_entrada: Some(t("2026-09-25T17:05:00Z")),
                hora_saida: None,
                observacoes: Some("  pagou no pix ".into()),
            },
        );
        assert_eq!(
            (a.metodo, a.caminho.as_str()),
            ("PATCH", "/bookings/e1/attendance")
        );
        assert_eq!(
            a.corpo.unwrap(),
            json!({"valor_pago": 250.5, "hora_entrada": "2026-09-25T17:05:00Z",
                   "observacoes_atendimento": "pagou no pix"}),
            "campo vazio não vai"
        );
        assert_eq!(
            atendimento("e1", &Atendimento::default()).corpo.unwrap(),
            json!({})
        );

        let u = descancelar("e1");
        assert_eq!(
            (u.metodo, u.caminho.as_str()),
            ("POST", "/bookings/e1/uncancel")
        );
        let x = excluir("e 1");
        assert_eq!(
            (x.metodo, x.caminho.as_str()),
            ("DELETE", "/bookings/agenda/e%201")
        );
        assert!(x.corpo.is_none());
    }
}
