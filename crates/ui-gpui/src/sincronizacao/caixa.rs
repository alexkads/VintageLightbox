//! 💰 **O caixa sem internet** (20/10/2026).
//!
//! Toda forma de pagamento do balcão já é registro manual — o dinheiro na
//! gaveta, o PIX na chave do estúdio, o cartão na maquininha. O dinheiro entra
//! com ou sem internet; o que faltava era o registro. Aqui ficam as três
//! gravações que o caixa faz sem rede, e como a tela as vê antes de o site
//! saber delas:
//!
//! | Gesto | Rota | Sem rede |
//! |---|---|---|
//! | abrir o caixa | `POST /pos-venda/caixa` | guardado; o caixa aparece aberto |
//! | vender | `POST /pos-venda/caixa/vendas` | guardada; entra no cupom e nos totais |
//! | sangria e suprimento | `POST /pos-venda/caixa/movimentos` | guardado; entra no caixa |
//!
//! ⚠️ **Estorno, conferência e fechamento precisam do site**: o fechamento é
//! cego (o esperado vem de lá) e o estorno mexe em venda que o site numerou.
//! Sem rede, eles dizem isso na tela.
//!
//! 🔑 **A chave de idempotência já existia** (`20261003160000_venda_idempotente`
//! e `…170000_caixa_idempotente` no backend): a fila repete a venda até ouvir
//! resposta, e o site grava uma vez só.

use serde_json::{json, Value};

/// As gravações do caixa que a fila sabe guardar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoCaixa {
    Abrir,
    Venda,
    Movimento,
}

fn rota(caminho: &str) -> &str {
    caminho.split('?').next().unwrap_or(caminho)
}

/// A gravação é do caixa e pode esperar a rede?
pub fn guardavel(metodo: &str, caminho: &str) -> Option<DoCaixa> {
    if !metodo.eq_ignore_ascii_case("POST") {
        return None;
    }
    match rota(caminho) {
        "/pos-venda/caixa" => Some(DoCaixa::Abrir),
        "/pos-venda/caixa/vendas" => Some(DoCaixa::Venda),
        "/pos-venda/caixa/movimentos" => Some(DoCaixa::Movimento),
        _ => None,
    }
}

/// O método do pedido como `&'static str` — o `PedidoJson` da porta só aceita
/// esse.
pub fn metodo_estatico(metodo: &str) -> &'static str {
    match metodo.to_ascii_uppercase().as_str() {
        "GET" => "GET",
        "PATCH" => "PATCH",
        "PUT" => "PUT",
        "DELETE" => "DELETE",
        _ => "POST",
    }
}

fn soma(lista: Option<&Value>, campo: &str) -> i64 {
    lista
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(|i| i.get(campo)?.as_i64()).sum())
        .unwrap_or(0)
}

fn id_local(corpo: &Value) -> String {
    format!(
        "sem-rede-{}",
        corpo.get("chave").and_then(Value::as_str).unwrap_or("?")
    )
}

/// A venda como o site a devolveria — o que a tela recebe na hora, sem rede.
///
/// 🔑 `guardada_sem_rede` é o que faz a tela dizer "sobe quando a rede
/// voltar" em vez de "Venda #0 registrada". O número é do site, e não existe
/// até lá.
fn venda_guardada(corpo: &Value, agora: &str) -> Value {
    let itens = corpo.get("itens");
    let total = (soma(itens, "cobrado_centavos")
        - corpo
            .get("desconto_no_total_centavos")
            .and_then(Value::as_i64)
            .unwrap_or(0))
    .max(0);
    let pago = soma(corpo.get("pagamentos"), "valor_centavos");
    let fotos: Vec<Value> = itens
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(|i| i.get("foto_id").cloned()).collect())
        .unwrap_or_default();
    json!({
        "id": id_local(corpo),
        "numero": 0,
        "galeria_id": corpo.get("galeria_id").cloned().unwrap_or(Value::Null),
        "itens": itens.and_then(Value::as_array).map(|l| l.iter().map(|i| json!({
            "foto_id": i.get("foto_id").cloned().unwrap_or(Value::Null),
            "arquivo": i.get("arquivo").cloned().unwrap_or(Value::Null),
        })).collect::<Vec<_>>()).unwrap_or_default(),
        "pagamentos": corpo.get("pagamentos").cloned().unwrap_or(json!([])),
        "total_centavos": total,
        "troco_centavos": (pago - total).max(0),
        "criada_em": agora,
        // Estornar a venda que o site ainda não numerou não é possível.
        "estornavel_centavos": 0,
        "fotos_vendidas": fotos,
        "guardada_sem_rede": true,
    })
}

fn movimento_guardado(corpo: &Value, agora: &str) -> Value {
    json!({
        "id": id_local(corpo),
        "tipo": corpo.get("tipo").cloned().unwrap_or(Value::Null),
        "valor_centavos": corpo.get("valor_centavos").cloned().unwrap_or(json!(0)),
        "motivo": corpo.get("motivo").cloned().unwrap_or(json!("")),
        "criado_em": agora,
        "guardada_sem_rede": true,
    })
}

fn caixa_guardado(corpo: &Value, agora: &str) -> Value {
    json!({
        "id": id_local(corpo),
        "estudio_id": corpo.get("estudio_id").cloned().unwrap_or(Value::Null),
        "fundo_de_troco_centavos": corpo.get("fundo_de_troco_centavos").cloned().unwrap_or(json!(0)),
        "aberto_em": agora,
        "local": corpo.get("local").cloned().unwrap_or(Value::Null),
        "movimentos": [],
        "vendas": [],
        "estornos": [],
        "guardada_sem_rede": true,
    })
}

fn hora(corpo: &Value) -> String {
    corpo
        .get("ocorrido_em")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| chrono::Local::now().to_rfc3339())
}

/// A resposta que a tela recebe quando a gravação ficou na fila.
pub fn resposta_guardada(tipo: DoCaixa, corpo: &Value) -> Value {
    let agora = hora(corpo);
    match tipo {
        DoCaixa::Abrir => caixa_guardado(corpo, &agora),
        DoCaixa::Venda => venda_guardada(corpo, &agora),
        DoCaixa::Movimento => movimento_guardado(corpo, &agora),
    }
}

/// Um pedido do caixa que espera a rede: `(rota, corpo)`.
pub type Pendente<'a> = (&'a str, &'a Value);

fn parametro<'a>(caminho: &'a str, nome: &str) -> Option<&'a str> {
    caminho
        .split_once('?')?
        .1
        .split('&')
        .find_map(|par| par.strip_prefix(nome)?.strip_prefix('='))
}

/// 📴 A leitura do caixa com o que espera a rede por cima: o caixa aberto sem
/// rede aparece aberto, e a venda e o movimento guardados entram nele.
///
/// `valor` é o que o site (ou o espelho) mandou; `None` quando nem um nem
/// outro tinha resposta — aí só o que foi feito aqui conta.
pub fn projetar(caminho: &str, valor: Option<Value>, pendentes: &[Pendente<'_>]) -> Option<Value> {
    let r = rota(caminho);
    if r == "/pos-venda/caixa" {
        let estudio = parametro(caminho, "estudio_id");
        let local = parametro(caminho, "local");
        let do_mesmo_caixa = |corpo: &Value| {
            let mesmo_estudio =
                estudio.is_none_or(|e| corpo.get("estudio_id").and_then(Value::as_str) == Some(e));
            let mesmo_local = local.is_none_or(|l| {
                corpo
                    .get("local")
                    .and_then(Value::as_str)
                    .is_none_or(|daqui| daqui == l)
            });
            mesmo_estudio && mesmo_local
        };
        let mut caixa = match valor {
            Some(Value::Null) | None => pendentes
                .iter()
                .find(|(rota, corpo)| *rota == "/pos-venda/caixa" && do_mesmo_caixa(corpo))
                .map(|(_, corpo)| caixa_guardado(corpo, &hora(corpo)))?,
            Some(valor) => valor,
        };
        for (rota, corpo) in pendentes {
            match *rota {
                "/pos-venda/caixa/vendas" => {
                    empurrar(&mut caixa, "vendas", venda_guardada(corpo, &hora(corpo)))
                }
                "/pos-venda/caixa/movimentos" if do_mesmo_caixa(corpo) => empurrar(
                    &mut caixa,
                    "movimentos",
                    movimento_guardado(corpo, &hora(corpo)),
                ),
                _ => {}
            }
        }
        return Some(caixa);
    }
    if let Some(galeria) = r
        .strip_prefix("/pos-venda/caixa/galerias/")
        .and_then(|resto| resto.strip_suffix("/vendas"))
    {
        let mut vendas = match valor {
            Some(Value::Array(lista)) => lista,
            Some(outro) => return Some(outro),
            None => Vec::new(),
        };
        for (rota, corpo) in pendentes {
            if *rota == "/pos-venda/caixa/vendas"
                && corpo.get("galeria_id").and_then(Value::as_str) == Some(galeria)
            {
                vendas.push(venda_guardada(corpo, &hora(corpo)));
            }
        }
        return Some(Value::Array(vendas));
    }
    valor
}

fn empurrar(caixa: &mut Value, lista: &str, item: Value) {
    match caixa.get_mut(lista).and_then(Value::as_array_mut) {
        Some(itens) => itens.push(item),
        None => {
            if let Some(mapa) = caixa.as_object_mut() {
                mapa.insert(lista.into(), json!([item]));
            }
        }
    }
}

/// A linha da tela "Sincronização".
pub fn descrever(caminho: &str, corpo: &Value) -> String {
    let reais = |centavos: i64| biblioteca_core::dinheiro::formatar(centavos);
    match guardavel("POST", caminho) {
        Some(DoCaixa::Abrir) => format!(
            "Abrir o caixa com {} de fundo",
            reais(
                corpo
                    .get("fundo_de_troco_centavos")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
            )
        ),
        Some(DoCaixa::Venda) => {
            let venda = venda_guardada(corpo, "");
            format!(
                "Venda de {}",
                reais(venda["total_centavos"].as_i64().unwrap_or(0))
            )
        }
        Some(DoCaixa::Movimento) => format!(
            "{} de {}",
            match corpo.get("tipo").and_then(Value::as_str) {
                Some("sangria") => "Sangria",
                Some("suprimento") => "Suprimento",
                _ => "Movimento",
            },
            reais(
                corpo
                    .get("valor_centavos")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
            )
        ),
        None => format!("Gravar no site ({caminho})"),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn venda(galeria: &str, cobrado: i64, pago: i64) -> Value {
        json!({
            "galeria_id": galeria,
            "itens": [
                { "foto_id": "f1", "arquivo": "DSC_1.jpg", "cobrado_centavos": cobrado },
            ],
            "desconto_no_total_centavos": 0,
            "pagamentos": [{ "forma": "dinheiro", "valor_centavos": pago, "detalhe": null }],
            "chave": "k1",
            "ocorrido_em": "2026-10-20T14:00:00-03:00",
        })
    }

    #[test]
    fn so_abrir_vender_e_movimentar_esperam_a_rede() {
        assert_eq!(guardavel("POST", "/pos-venda/caixa"), Some(DoCaixa::Abrir));
        assert_eq!(
            guardavel("POST", "/pos-venda/caixa/vendas"),
            Some(DoCaixa::Venda)
        );
        assert_eq!(
            guardavel("POST", "/pos-venda/caixa/movimentos"),
            Some(DoCaixa::Movimento)
        );
        assert_eq!(guardavel("POST", "/pos-venda/caixa/fechar"), None);
        assert_eq!(guardavel("POST", "/pos-venda/caixa/conferir"), None);
        assert_eq!(
            guardavel("POST", "/pos-venda/caixa/vendas/v1/estornos"),
            None
        );
        assert_eq!(guardavel("GET", "/pos-venda/caixa"), None);
    }

    /// A venda guardada é lida pelo mesmo leitor da do site — a tela não
    /// distingue, a não ser pela marca.
    #[test]
    fn a_venda_guardada_tem_total_troco_e_a_marca() {
        let resposta = resposta_guardada(DoCaixa::Venda, &venda("g1", 3000, 5000));
        assert_eq!(resposta["guardada_sem_rede"], true);
        let lida = crate::caixa::dados::ler_venda(resposta).unwrap();
        assert_eq!(lida.total, 3000);
        assert_eq!(lida.troco, 2000);
        assert_eq!(lida.fotos_vendidas, vec!["f1".to_string()]);
        assert_eq!(lida.criada_em, "2026-10-20T14:00:00-03:00");
    }

    #[test]
    fn o_caixa_aberto_sem_rede_aparece_com_a_venda_e_a_sangria() {
        let abrir = json!({
            "estudio_id": "s1", "fundo_de_troco_centavos": 10000, "chave": "a", "local": "ESTUDIO",
        });
        let sangria = json!({
            "estudio_id": "s1", "tipo": "sangria", "valor_centavos": 2000, "motivo": "banco",
            "chave": "m", "local": "ESTUDIO",
        });
        let de_outro = json!({
            "estudio_id": "s2", "tipo": "sangria", "valor_centavos": 1, "chave": "x",
        });
        let v = venda("g1", 3000, 3000);
        let pendentes: Vec<Pendente> = vec![
            ("/pos-venda/caixa", &abrir),
            ("/pos-venda/caixa/vendas", &v),
            ("/pos-venda/caixa/movimentos", &sangria),
            ("/pos-venda/caixa/movimentos", &de_outro),
        ];
        let caminho = "/pos-venda/caixa?estudio_id=s1&local=ESTUDIO";
        // O site disse "fechado" (ou nada disse): vale o aberto aqui.
        for valor in [Some(Value::Null), None] {
            let caixa = projetar(caminho, valor, &pendentes).unwrap();
            let lido = crate::caixa::dados::ler_caixa_aberto(caixa)
                .unwrap()
                .unwrap();
            assert_eq!(lido.fundo_de_troco, 10000);
            assert_eq!(lido.vendas.len(), 1);
            assert_eq!(lido.movimentos.len(), 1);
        }
        // Sem nada guardado e o caixa fechado lá, segue fechado.
        assert_eq!(projetar(caminho, Some(Value::Null), &[]), None);

        // O caixa que já estava aberto no site recebe a venda guardada.
        let do_site = json!({ "id": "c1", "estudio_id": "s1", "vendas": [{ "id": "v0", "numero": 7, "total_centavos": 100 }] });
        let caixa = projetar(caminho, Some(do_site), &pendentes[1..2]).unwrap();
        assert_eq!(caixa["vendas"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn as_vendas_da_sessao_levam_so_as_guardadas_dela() {
        let a = venda("g1", 100, 100);
        let b = venda("g2", 100, 100);
        let pendentes: Vec<Pendente> = vec![
            ("/pos-venda/caixa/vendas", &a),
            ("/pos-venda/caixa/vendas", &b),
        ];
        let vendas = projetar(
            "/pos-venda/caixa/galerias/g1/vendas",
            Some(json!([])),
            &pendentes,
        )
        .unwrap();
        assert_eq!(vendas.as_array().unwrap().len(), 1);
        let sem_espelho =
            projetar("/pos-venda/caixa/galerias/g1/vendas", None, &pendentes).unwrap();
        assert_eq!(sem_espelho.as_array().unwrap().len(), 1);
    }
}
