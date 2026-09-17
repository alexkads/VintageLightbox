//! O que se associa a uma sessão nova — agendamento, voucher, compra
//! antecipada e parceiro —, como o assistente os guarda e os mostra.
//!
//! É o `associacoes/associacoes.ts` e a leitura das `actions.ts` do site,
//! portados: as mesmas buscas (`/pos-venda/busca/*`, `/pos-venda/parceiros`),
//! a mesma ordem de quem procura e os mesmos rótulos.
//!
//! 🔑 **Só texto e número**, como no site: estes objetos vão para o rascunho
//! em JSON, e as datas já saem formatadas no fuso do estúdio, com o ISO ao lado
//! para ordenar.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Quantos resultados cada busca pede.
pub const LIMITE_DA_BUSCA: usize = 40;
/// A busca de agendamentos sem texto olha esta janela em volta de hoje.
pub const DIAS_ATRAS: i64 = 60;
pub const DIAS_A_FRENTE: i64 = 14;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgendamentoEscolhido {
    pub id: String,
    pub nome: Option<String>,
    pub whatsapp: Option<String>,
    pub email: Option<String>,
    /// ISO — a chave da ordem.
    pub inicio_iso: Option<String>,
    /// "13/09/2026 14:00", no fuso do estúdio.
    pub quando: Option<String>,
    pub estudio_id: Option<String>,
    pub estudio_nome: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoucherEscolhido {
    pub id: String,
    pub numero: String,
    pub nome: Option<String>,
    pub whatsapp: Option<String>,
    pub email: Option<String>,
    pub parceiro: Option<String>,
    pub criado_em_iso: Option<String>,
    pub criado_em: Option<String>,
    /// Texto livre da coluna, mostrado como está.
    pub agendamento: Option<String>,
    pub utilizado_em: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemDaCompra {
    pub titulo: String,
    pub quantidade: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompraEscolhida {
    pub id: String,
    /// Centavos.
    pub total: Option<i64>,
    pub status: String,
    pub pago_em_iso: Option<String>,
    pub pago_em: Option<String>,
    pub criado_em_iso: Option<String>,
    pub comprador_nome: Option<String>,
    pub comprador_email: Option<String>,
    pub whatsapp: Option<String>,
    pub itens: Vec<ItemDaCompra>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParceiroEscolhido {
    pub id: String,
    pub nome: String,
    pub tipo: String,
    pub whatsapp: Option<String>,
    pub email: Option<String>,
    pub ativo: bool,
}

// ── Leitura das respostas da API ─────────────────────────────────────────

fn texto(v: &Value, campo: &str) -> Option<String> {
    v.get(campo)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

fn obrigatorio(v: &Value, campo: &str) -> Option<String> {
    match v.get(campo)? {
        Value::String(t) => Some(t.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn itens_da_lista(v: &Value) -> impl Iterator<Item = &Value> {
    v.as_array().into_iter().flatten()
}

/// Um instante da API (`2026-09-13T17:00:00Z`) no fuso do estúdio.
fn no_fuso(iso: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let instante = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    let brasilia = chrono::FixedOffset::west_opt(3 * 3600)?;
    Some(instante.with_timezone(&brasilia))
}

/// `formatarDataHoraBR`: "13/09/2026 14:00".
pub fn data_e_hora_br(iso: &str) -> Option<String> {
    no_fuso(iso).map(|d| d.format("%d/%m/%Y %H:%M").to_string())
}

/// `formatarDataBR`: "13/09/2026".
pub fn data_br(iso: &str) -> Option<String> {
    no_fuso(iso).map(|d| d.format("%d/%m/%Y").to_string())
}

/// Reais em texto ou número (`"299.90"`, `299.9`) → centavos, sem ponto
/// flutuante no caminho do texto.
fn centavos(v: &Value) -> Option<i64> {
    match v {
        Value::String(t) => biblioteca_core::dinheiro::ler_campo(&t.replace('.', ",")),
        Value::Number(n) => n
            .as_i64()
            .map(|r| r * 100)
            .or_else(|| n.as_f64().map(|r| (r * 100.0).round() as i64)),
        _ => None,
    }
}

/// Cancelado não gera galeria: some da busca.
pub fn cancelado(status: &str) -> bool {
    matches!(status, "Cancelled" | "CancelledLastMinute")
}

pub fn agendamentos_da_api(v: &Value) -> Vec<AgendamentoEscolhido> {
    itens_da_lista(v)
        .filter_map(|a| {
            let status = obrigatorio(a, "status")?;
            if cancelado(&status) {
                return None;
            }
            let inicio = texto(a, "inicio");
            Some(AgendamentoEscolhido {
                id: obrigatorio(a, "id")?,
                nome: texto(a, "nome"),
                whatsapp: texto(a, "whatsapp"),
                email: texto(a, "email"),
                quando: inicio.as_deref().and_then(data_e_hora_br),
                inicio_iso: inicio,
                estudio_id: texto(a, "estudio_id"),
                estudio_nome: texto(a, "estudio_nome"),
                status,
            })
        })
        .collect()
}

pub fn vouchers_da_api(v: &Value) -> Vec<VoucherEscolhido> {
    itens_da_lista(v)
        .filter_map(|x| {
            let criado = texto(x, "criado_em");
            Some(VoucherEscolhido {
                id: obrigatorio(x, "id")?,
                numero: obrigatorio(x, "numero")?,
                nome: texto(x, "nome"),
                whatsapp: texto(x, "whatsapp"),
                email: texto(x, "email"),
                parceiro: texto(x, "parceiro"),
                criado_em: criado.as_deref().and_then(data_br),
                criado_em_iso: criado,
                agendamento: texto(x, "agendamento_data_hora"),
                utilizado_em: texto(x, "utilizado_em").as_deref().and_then(data_br),
            })
        })
        .collect()
}

pub fn compras_da_api(v: &Value) -> Vec<CompraEscolhida> {
    itens_da_lista(v)
        .filter_map(|c| {
            let pago = texto(c, "pago_em");
            Some(CompraEscolhida {
                id: obrigatorio(c, "id")?,
                total: c.get("total").and_then(centavos),
                status: obrigatorio(c, "status").unwrap_or_default(),
                pago_em: pago.as_deref().and_then(data_e_hora_br),
                pago_em_iso: pago,
                criado_em_iso: texto(c, "criado_em"),
                comprador_nome: texto(c, "comprador_nome"),
                comprador_email: texto(c, "comprador_email"),
                whatsapp: texto(c, "whatsapp_contato"),
                itens: itens_da_lista(c.get("itens").unwrap_or(&Value::Null))
                    .filter_map(|i| {
                        Some(ItemDaCompra {
                            titulo: obrigatorio(i, "titulo")?,
                            quantidade: i.get("quantidade").and_then(Value::as_i64)?,
                        })
                    })
                    .collect(),
            })
        })
        .collect()
}

pub fn parceiro_da_api(p: &Value) -> Option<ParceiroEscolhido> {
    Some(ParceiroEscolhido {
        id: obrigatorio(p, "id")?,
        nome: obrigatorio(p, "nome")?,
        tipo: obrigatorio(p, "tipo").unwrap_or_default(),
        whatsapp: texto(p, "whatsapp"),
        email: texto(p, "email"),
        ativo: p.get("ativo").and_then(Value::as_bool).unwrap_or(true),
    })
}

pub fn parceiros_da_api(v: &Value) -> Vec<ParceiroEscolhido> {
    itens_da_lista(v).filter_map(parceiro_da_api).collect()
}

// ── Ordem ────────────────────────────────────────────────────────────────

fn segundos(iso: Option<&str>) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(iso?)
        .ok()
        .map(|d| d.timestamp())
}

/// O mais perto de agora primeiro, dos dois lados; sem data, no fim.
pub fn ordenar_agendamentos(lista: &mut [AgendamentoEscolhido], agora: i64) {
    lista.sort_by_key(|a| {
        segundos(a.inicio_iso.as_deref())
            .map(|t| (t - agora).unsigned_abs())
            .unwrap_or(u64::MAX)
    });
}

/// O voucher ainda não usado primeiro, e dentro de cada grupo o mais recente.
pub fn ordenar_vouchers(lista: &mut [VoucherEscolhido]) {
    lista.sort_by_key(|v| {
        (
            v.utilizado_em.is_some(),
            std::cmp::Reverse(segundos(v.criado_em_iso.as_deref()).unwrap_or(0)),
        )
    });
}

/// A compra paga primeiro, a mais recente antes; a não paga depois.
pub fn ordenar_compras(lista: &mut [CompraEscolhida]) {
    lista.sort_by_key(|c| {
        let t = segundos(c.pago_em_iso.as_deref())
            .or_else(|| segundos(c.criado_em_iso.as_deref()))
            .unwrap_or(0);
        (c.pago_em_iso.is_none(), std::cmp::Reverse(t))
    });
}

// ── Rótulos ──────────────────────────────────────────────────────────────

/// "2× Ensaio família · 1× Foto extra", com as sobras contadas.
pub fn resumo_dos_itens(itens: &[ItemDaCompra], maximo: usize) -> String {
    let visiveis: Vec<String> = itens
        .iter()
        .take(maximo)
        .map(|i| format!("{}× {}", i.quantidade, i.titulo))
        .collect();
    let resto = itens.len().saturating_sub(maximo);
    if resto > 0 {
        format!("{} · +{resto}", visiveis.join(" · "))
    } else {
        visiveis.join(" · ")
    }
}

/// Junta o que existe, na ordem dada.
pub fn juntar(partes: &[Option<&str>]) -> String {
    partes
        .iter()
        .flatten()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

pub fn rotulo_do_status_do_agendamento(status: &str) -> String {
    match status {
        "Pending" => "Pendente",
        "Confirmed" => "Confirmado",
        "Completed" => "Realizado",
        "Rescheduled" => "Reagendado",
        "NoShow" => "Não compareceu",
        outro => outro,
    }
    .to_string()
}

pub fn rotulo_do_status_da_compra(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "paid" | "approved" => "Paga".into(),
        "completed" => "Concluída".into(),
        "pending" => "Pendente".into(),
        "cancelled" | "canceled" => "Cancelada".into(),
        "refunded" => "Estornada".into(),
        _ => status.to_string(),
    }
}

/// `(valor, rótulo)` — a ordem da tela.
pub const TIPOS_DE_PARCEIRO: [(&str, &str); 8] = [
    ("hotel", "Hotel"),
    ("pousada", "Pousada"),
    ("agencia_de_turismo", "Agência de turismo"),
    ("guia", "Guia"),
    ("restaurante_cafe", "Restaurante/Café"),
    ("influenciador", "Influenciador"),
    ("site_de_ofertas", "Site de ofertas"),
    ("outro", "Outro"),
];

pub fn rotulo_do_tipo_de_parceiro(valor: &str) -> String {
    TIPOS_DE_PARCEIRO
        .iter()
        .find(|(v, _)| *v == valor)
        .map(|(_, r)| r.to_string())
        .unwrap_or_else(|| valor.to_string())
}

/// `(valor, rótulo)` — "Como conheceu", na ordem da tela.
pub const COMO_CONHECEU: [(&str, &str); 9] = [
    ("instagram", "Instagram"),
    ("facebook", "Facebook"),
    ("tiktok", "TikTok"),
    ("google", "Google"),
    ("indicacao", "Indicação de amigo"),
    ("parceiro", "Parceiro"),
    ("passou_em_frente", "Passou em frente"),
    ("ja_e_cliente", "Já é cliente"),
    ("outro", "Outro"),
];

pub fn rotulo_de_como_conheceu(valor: &str) -> String {
    COMO_CONHECEU
        .iter()
        .find(|(v, _)| *v == valor)
        .map(|(_, r)| r.to_string())
        .unwrap_or_else(|| valor.to_string())
}

pub fn eh_como_conheceu(valor: &str) -> bool {
    COMO_CONHECEU.iter().any(|(v, _)| *v == valor)
}

fn normalizar(texto: &str) -> String {
    biblioteca_core::sessoes::normalizar(texto)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// O parceiro com este nome, se já estiver na lista — o "usar este" do `409`.
pub fn achar_parceiro_pelo_nome<'a>(
    lista: &'a [ParceiroEscolhido],
    nome: &str,
) -> Option<&'a ParceiroEscolhido> {
    let alvo = normalizar(nome);
    if alvo.is_empty() {
        return None;
    }
    lista.iter().find(|p| normalizar(&p.nome) == alvo)
}

/// Percentual da query string (RFC 3986): o que não é "não reservado" vira
/// `%XX` byte a byte.
fn codificar(texto: &str) -> String {
    texto
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A query string das buscas, com o texto codificado.
pub fn consulta(pares: &[(&str, Option<String>)]) -> String {
    let partes: Vec<String> = pares
        .iter()
        .filter_map(|(k, v)| {
            v.as_ref()
                .filter(|v| !v.is_empty())
                .map(|v| format!("{k}={}", codificar(v)))
        })
        .collect();
    if partes.is_empty() {
        String::new()
    } else {
        format!("?{}", partes.join("&"))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    #[test]
    fn agendamento_cancelado_some_e_a_data_sai_no_fuso_do_estudio() {
        let lista = agendamentos_da_api(&json!([
            {"id": "a1", "nome": "Maria", "inicio": "2026-09-13T17:00:00Z", "status": "Confirmed"},
            {"id": "a2", "nome": "João", "inicio": null, "status": "Cancelled"},
        ]));
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].quando.as_deref(), Some("13/09/2026 14:00"));
    }

    #[test]
    fn agendamentos_pelo_mais_perto_de_agora() {
        let mut lista = agendamentos_da_api(&json!([
            {"id": "longe", "inicio": "2026-01-01T00:00:00Z", "status": "Pending"},
            {"id": "sem", "status": "Pending"},
            {"id": "perto", "inicio": "2026-09-13T00:00:00Z", "status": "Pending"},
        ]));
        let agora = chrono::DateTime::parse_from_rfc3339("2026-09-14T00:00:00Z")
            .unwrap()
            .timestamp();
        ordenar_agendamentos(&mut lista, agora);
        let ids: Vec<_> = lista.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["perto", "longe", "sem"]);
    }

    #[test]
    fn voucher_nao_usado_primeiro_e_o_mais_recente_antes() {
        let mut lista = vouchers_da_api(&json!([
            {"id": "usado", "numero": 1, "criado_em": "2026-09-10T00:00:00Z", "utilizado_em": "2026-09-11T00:00:00Z"},
            {"id": "velho", "numero": "2", "criado_em": "2026-08-01T00:00:00Z"},
            {"id": "novo", "numero": "3", "criado_em": "2026-09-01T00:00:00Z"},
        ]));
        ordenar_vouchers(&mut lista);
        let ids: Vec<_> = lista.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["novo", "velho", "usado"]);
        assert_eq!(lista[2].numero, "1");
    }

    #[test]
    fn compra_paga_primeiro_e_o_total_em_centavos() {
        let mut lista = compras_da_api(&json!([
            {"id": "pendente", "total": "10.00", "status": "pending", "criado_em": "2026-09-12T00:00:00Z", "itens": []},
            {"id": "paga", "total": "299.90", "status": "paid", "pago_em": "2026-09-01T00:00:00Z",
             "itens": [{"titulo": "Ensaio", "quantidade": 2}]},
        ]));
        ordenar_compras(&mut lista);
        assert_eq!(lista[0].id, "paga");
        assert_eq!(lista[0].total, Some(29990));
        assert_eq!(rotulo_do_status_da_compra("PAID"), "Paga");
    }

    #[test]
    fn resumo_dos_itens_conta_as_sobras() {
        let itens: Vec<ItemDaCompra> = (1..=5)
            .map(|i| ItemDaCompra {
                titulo: format!("X{i}"),
                quantidade: i,
            })
            .collect();
        assert_eq!(resumo_dos_itens(&itens, 3), "1× X1 · 2× X2 · 3× X3 · +2");
        assert_eq!(resumo_dos_itens(&[], 3), "");
    }

    #[test]
    fn parceiro_pelo_nome_sem_acento_nem_caixa() {
        let lista =
            parceiros_da_api(&json!([{"id": "p", "nome": "Hotel  Serra", "tipo": "hotel"}]));
        assert!(achar_parceiro_pelo_nome(&lista, " hótel serra ").is_some());
        assert!(achar_parceiro_pelo_nome(&lista, "").is_none());
    }

    #[test]
    fn consulta_codifica_e_pula_o_vazio() {
        assert_eq!(
            consulta(&[
                ("busca", Some("Maria José".into())),
                ("de", None),
                ("limite", Some("40".into()))
            ]),
            "?busca=Maria%20Jos%C3%A9&limite=40"
        );
        assert_eq!(consulta(&[("busca", Some(String::new()))]), "");
    }
}
