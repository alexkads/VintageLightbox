//! As rotas `vlb://…/catalogo/…`: o catálogo para a página **e para os workers**.
//!
//! Um worker não alcança os comandos do Tauri (não há `__TAURI__` lá dentro),
//! mas alcança o esquema da página com `fetch`. É por aqui que a importação,
//! que roda num worker, lê e grava a área temporária no SQLite (etapa D,
//! passo 2).
//!
//! 🚨 **Os bytes vão como `ArrayBuffer`.** O WebKit não entrega a um esquema
//! próprio o corpo de um `Blob` nem de um `FormData`. Uma gravação é um corpo
//! só, no formato de [`ler_gravacoes`].
//!
//! # As rotas da área temporária
//!
//! | Método | Caminho | O quê |
//! |---|---|---|
//! | GET | `/catalogo/importacao[?galeria=ID]` | os itens (sem bytes) |
//! | GET | `/catalogo/importacao/{id}` | um item |
//! | GET | `/catalogo/importacao/{id}/{arquivo,arquivoBruto,previa}` | os bytes |
//! | POST | `/catalogo/importacao` | gravações, numa transação |
//! | DELETE | `/catalogo/importacao/{id}` | apaga o item e os bytes |
//! | POST | `/catalogo/importacao/trocar-galeria` | `{de, para}` |
//! | POST | `/catalogo/importacao/apagar-galeria` | `{galeria}` |

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::Deserialize;
use serde_json::{json, Map, Value};
use tauri::http::{header, Method, Request, Response, StatusCode};
use tauri::{AppHandle, Manager, Runtime};

use crate::catalogo::importacao::{Bytes, Gravacao, Gravado, Parte, Qual};
use crate::catalogo::Catalogo;

pub const PREFIXO: &str = "/catalogo/";

type Resposta = Response<Cow<'static, [u8]>>;

fn resposta(status: StatusCode, tipo: &str, corpo: Vec<u8>) -> Resposta {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, tipo)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        // Os bytes de um item mudam com a mesma URL: nada de cache.
        .header(header::CACHE_CONTROL, "no-store")
        .body(Cow::Owned(corpo))
        .expect("resposta montada")
}

fn json_(status: StatusCode, valor: &Value) -> Resposta {
    resposta(status, "application/json", valor.to_string().into_bytes())
}

fn erro(status: StatusCode, texto: &str) -> Resposta {
    json_(status, &json!({ "message": texto }))
}

fn agora() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// O cabeçalho de uma gravação no corpo.
#[derive(Deserialize)]
struct Cabecalho {
    operacoes: Vec<Operacao>,
}

#[derive(Deserialize)]
struct Operacao {
    id: String,
    #[serde(default)]
    substituir: bool,
    #[serde(default)]
    campos: Map<String, Value>,
    #[serde(default)]
    remover: Vec<String>,
    #[serde(default)]
    versao: Option<i64>,
    /// Por parte: ausente (fica), `null` (sai) ou a descrição dos bytes novos.
    #[serde(default)]
    partes: BTreeMap<String, Option<Descricao>>,
}

#[derive(Deserialize)]
struct Descricao {
    tipo: String,
    #[serde(default)]
    nome: Option<String>,
    tamanho: usize,
}

/// Lê o corpo de uma gravação.
///
/// `[4 bytes: tamanho do cabeçalho, big-endian][cabeçalho JSON][bytes]`, e os
/// bytes vêm na ordem das operações e, em cada uma, na ordem
/// `arquivo`, `arquivoBruto`, `previa`, só das partes novas.
pub fn ler_gravacoes(corpo: &[u8]) -> Result<Vec<Gravacao>, String> {
    let tamanho = corpo
        .get(..4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or("corpo curto demais")?;
    let fim = 4usize.checked_add(tamanho).ok_or("cabeçalho inválido")?;
    let cabecalho: Cabecalho =
        serde_json::from_slice(corpo.get(4..fim).ok_or("cabeçalho cortado")?)
            .map_err(|e| format!("cabeçalho ilegível: {e}"))?;
    let mut resto = &corpo[fim..];
    let mut gravacoes = Vec::with_capacity(cabecalho.operacoes.len());
    for op in cabecalho.operacoes {
        let mut partes = BTreeMap::new();
        for qual in ["arquivo", "arquivoBruto", "previa"] {
            match op.partes.get(qual) {
                None => {}
                Some(None) => {
                    partes.insert(qual.to_string(), Parte::Apagar);
                }
                Some(Some(d)) => {
                    if resto.len() < d.tamanho {
                        return Err(format!("faltam bytes de {qual} em {}", op.id));
                    }
                    let (estes, depois) = resto.split_at(d.tamanho);
                    resto = depois;
                    partes.insert(
                        qual.to_string(),
                        Parte::Nova(Bytes {
                            tipo: d.tipo.clone(),
                            nome: d.nome.clone(),
                            bytes: estes.to_vec(),
                        }),
                    );
                }
            }
        }
        gravacoes.push(Gravacao {
            id: op.id,
            substituir: op.substituir,
            campos: op.campos,
            remover: op.remover,
            partes,
            versao: op.versao,
        });
    }
    if !resto.is_empty() {
        return Err(format!("sobraram {} bytes no corpo", resto.len()));
    }
    Ok(gravacoes)
}

fn gravado_em_json(g: &Gravado) -> Value {
    match g {
        Gravado::Pronto(item) => json!({ "tipo": "pronto", "item": item }),
        Gravado::Conflito { id } => json!({ "tipo": "conflito", "id": id }),
        Gravado::Ausente { id } => json!({ "tipo": "ausente", "id": id }),
    }
}

fn parametro(consulta: Option<&str>, nome: &str) -> Option<String> {
    consulta?.split('&').find_map(|par| {
        let (k, v) = par.split_once('=')?;
        (k == nome).then(|| {
            percent_encoding::percent_decode_str(v)
                .decode_utf8_lossy()
                .into_owned()
        })
    })
}

/// Trata um pedido `/catalogo/…`.
pub async fn tratar<R: Runtime>(app: &AppHandle<R>, pedido: Request<Vec<u8>>) -> Resposta {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || tratar_agora(&app, &pedido))
        .await
        .unwrap_or_else(|_| {
            erro(
                StatusCode::INTERNAL_SERVER_ERROR,
                "o catálogo não respondeu",
            )
        })
}

fn tratar_agora<R: Runtime>(app: &AppHandle<R>, pedido: &Request<Vec<u8>>) -> Resposta {
    let Some(estado) = app.try_state::<Mutex<Catalogo>>() else {
        return erro(
            StatusCode::SERVICE_UNAVAILABLE,
            "o catálogo não está aberto",
        );
    };
    let caminho = pedido.uri().path();
    let Some(resto) = caminho.strip_prefix(PREFIXO) else {
        return erro(StatusCode::NOT_FOUND, "rota do catálogo desconhecida");
    };
    let partes: Vec<String> = resto
        .split('/')
        .map(|p| {
            percent_encoding::percent_decode_str(p)
                .decode_utf8_lossy()
                .into_owned()
        })
        .collect();
    let partes: Vec<&str> = partes.iter().map(String::as_str).collect();
    let metodo = pedido.method();
    let mut catalogo = estado.lock().expect("catálogo");

    let feito: Result<Resposta, crate::catalogo::ErroDoCatalogo> = (|| {
        Ok(match (metodo, partes.as_slice()) {
            (&Method::GET, ["importacao"]) => {
                let galeria = parametro(pedido.uri().query(), "galeria");
                let itens = catalogo.importacao_itens(galeria.as_deref())?;
                json_(StatusCode::OK, &json!(itens))
            }
            (&Method::POST, ["importacao"]) => match ler_gravacoes(pedido.body()) {
                Ok(gravacoes) => {
                    let feitos = catalogo.importacao_gravar(gravacoes, &agora())?;
                    let lista: Vec<Value> = feitos.iter().map(gravado_em_json).collect();
                    json_(StatusCode::OK, &Value::Array(lista))
                }
                Err(motivo) => erro(StatusCode::BAD_REQUEST, &motivo),
            },
            (&Method::POST, ["importacao", "trocar-galeria"]) => {
                #[derive(Deserialize)]
                struct Troca {
                    de: String,
                    para: String,
                }
                match serde_json::from_slice::<Troca>(pedido.body()) {
                    Ok(t) => {
                        let itens = catalogo.importacao_trocar_galeria(&t.de, &t.para, &agora())?;
                        json_(StatusCode::OK, &json!(itens))
                    }
                    Err(e) => erro(StatusCode::BAD_REQUEST, &e.to_string()),
                }
            }
            (&Method::POST, ["importacao", "apagar-galeria"]) => {
                #[derive(Deserialize)]
                struct Qual {
                    galeria: String,
                }
                match serde_json::from_slice::<Qual>(pedido.body()) {
                    Ok(q) => json_(
                        StatusCode::OK,
                        &json!(catalogo.importacao_apagar_galeria(&q.galeria)?),
                    ),
                    Err(e) => erro(StatusCode::BAD_REQUEST, &e.to_string()),
                }
            }
            (&Method::GET, ["importacao", id]) => match catalogo.importacao_item(id)? {
                Some(item) => json_(StatusCode::OK, &json!(item)),
                None => erro(StatusCode::NOT_FOUND, "item inexistente"),
            },
            (&Method::DELETE, ["importacao", id]) => json_(
                StatusCode::OK,
                &json!({ "apagado": catalogo.importacao_apagar(id)? }),
            ),
            (&Method::GET, ["importacao", id, parte]) => {
                let Some(qual) = Qual::do_caminho(parte) else {
                    return Ok(erro(StatusCode::NOT_FOUND, "parte desconhecida"));
                };
                match catalogo.importacao_arquivo(id, qual)? {
                    Some((caminho, guardada)) => match std::fs::read(&caminho) {
                        Ok(bytes) => resposta(StatusCode::OK, &guardada.tipo, bytes),
                        Err(e) => erro(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            &format!("os bytes sumiram do catálogo: {e}"),
                        ),
                    },
                    None => erro(StatusCode::NOT_FOUND, "sem bytes"),
                }
            }
            _ => erro(StatusCode::NOT_FOUND, "rota do catálogo desconhecida"),
        })
    })();
    match feito {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[catálogo] {} {}: {e}", metodo, caminho);
            erro(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn corpo(cabecalho: Value, bytes: &[u8]) -> Vec<u8> {
        let texto = cabecalho.to_string();
        let mut c = (texto.len() as u32).to_be_bytes().to_vec();
        c.extend_from_slice(texto.as_bytes());
        c.extend_from_slice(bytes);
        c
    }

    #[test]
    fn as_partes_saem_na_ordem_de_cada_operacao() {
        let c = corpo(
            json!({"operacoes": [
                {"id": "a", "substituir": true, "campos": {"nota": 5},
                 "partes": {"previa": {"tipo": "image/jpeg", "tamanho": 2},
                            "arquivo": {"tipo": "image/webp", "nome": "x.webp", "tamanho": 3}}},
                {"id": "b", "remover": ["erro"], "versao": 4, "partes": {"arquivo": null}}
            ]}),
            b"ARQPR",
        );
        let g = ler_gravacoes(&c).unwrap();
        assert_eq!(g.len(), 2);
        assert!(g[0].substituir);
        assert_eq!(
            g[0].partes["arquivo"],
            Parte::Nova(Bytes {
                tipo: "image/webp".into(),
                nome: Some("x.webp".into()),
                bytes: b"ARQ".to_vec()
            })
        );
        match &g[0].partes["previa"] {
            Parte::Nova(b) => assert_eq!(b.bytes, b"PR"),
            outra => panic!("{outra:?}"),
        }
        assert!(!g[0].partes.contains_key("arquivoBruto"));
        assert_eq!(g[1].partes["arquivo"], Parte::Apagar);
        assert_eq!(g[1].versao, Some(4));
        assert_eq!(g[1].remover, vec!["erro".to_string()]);
    }

    #[test]
    fn corpo_torto_e_recusado() {
        assert!(ler_gravacoes(b"").is_err());
        assert!(ler_gravacoes(&[0, 0, 0, 50, b'{']).is_err());
        let falta = corpo(
            json!({"operacoes": [{"id": "a", "partes": {"arquivo": {"tipo": "x", "tamanho": 10}}}]}),
            b"123",
        );
        assert!(ler_gravacoes(&falta).is_err());
        let sobra = corpo(json!({"operacoes": []}), b"123");
        assert!(ler_gravacoes(&sobra).is_err());
    }

    #[test]
    fn o_parametro_da_consulta_e_decodificado() {
        assert_eq!(
            parametro(Some("x=1&galeria=rascunho%3Aabc"), "galeria").as_deref(),
            Some("rascunho:abc")
        );
        assert_eq!(parametro(None, "galeria"), None);
    }
}
