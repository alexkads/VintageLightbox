//! O esquema `vlb://`: de onde a tela empacotada é servida (DESKTOP_TAURI §0).
//!
//! # Por que um esquema próprio, e não o `tauri://` de sempre
//!
//! A tela do site chama **rotas internas** por caminho relativo:
//! - as imagens (`/dashboard/sessoes-fotograficas/fotos/{id}/previa`…), que exigem
//!   o token e aparecem em `<img>` e no `fetch` do wasm da biblioteca;
//! - os bilhetes de envio, os presets, as sobras e o "revelando", chamados também
//!   de dentro dos *workers*.
//!
//! No site, quem responde é o Next. No app, um caminho relativo cai no mesmo
//! esquema da página, de onde quer que venha (página, *worker*, wasm, `<img>`).
//! Servindo a tela por um esquema nosso, essas rotas chegam aqui, e o Rust as
//! repassa à API com o token. O resto do caminho é a tela compilada
//! (`interface/`), com o `index.html` para as rotas do React Router.
//!
//! 🔑 Para as permissões do Tauri, uma página de esquema registrado pelo app é
//! **local**, como `tauri://`.
//!
//! # O envio das fotos
//!
//! O navegador manda o arquivo **direto à API**, com o bilhete. O CORS do
//! backend aceita uma origem só, e o app não é ela. Por isso, no app, o bilhete
//! aponta para `vlb://…/envio-publico/…`, e o Rust repassa o envio.

use std::borrow::Cow;

use infrastructure::CorpoCru;
use serde::Deserialize;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{AppHandle, Manager, Runtime};

use crate::api::ContaDoApp;

pub const ESQUEMA: &str = "vlb";

/// O prefixo das rotas internas da tela, igual ao do site.
const ROTA: &str = "/dashboard/sessoes-fotograficas/";
/// O repasse do envio por bilhete (rota pública da API).
pub const ENVIO: &str = "/envio-publico";

/// O endereço da tela empacotada, na rota pedida.
///
/// No macOS e no Linux, `vlb://localhost/rota`. No Windows o WebView2 só aceita
/// esquema próprio como `http://vlb.localhost/rota`.
pub fn endereco(rota: &str) -> tauri::Url {
    let base = if cfg!(windows) {
        format!("http://{ESQUEMA}.localhost")
    } else {
        format!("{ESQUEMA}://localhost")
    };
    tauri::Url::parse(&format!("{base}{rota}")).expect("endereço da tela")
}

/// O que uma rota interna vira na API.
#[derive(Debug, PartialEq, Eq)]
pub enum Destino {
    /// Uma imagem: os bytes, sem interpretar.
    Imagem(String),
    /// JSON repassado, com o erro traduzido para `{ message }`.
    Json(String),
    /// Um bilhete: a resposta ganha a URL do envio pelo app.
    Bilhete(String),
}

/// A rota interna que o site responde, traduzida para o caminho da API.
///
/// `None` quando o caminho não é rota interna: aí ele é um arquivo da tela.
pub fn rota_interna(caminho: &str) -> Option<Destino> {
    let resto = caminho.strip_prefix(ROTA)?;
    let partes: Vec<&str> = resto.split('/').collect();
    let seguro = |p: &str| {
        !p.is_empty()
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    match partes.as_slice() {
        ["fotos", id, tipo] if seguro(id) => {
            let api = format!("/pos-venda/fotos/{id}/{tipo}");
            match *tipo {
                "previa" | "miniatura" | "copia-de-trabalho" | "original" => {
                    Some(Destino::Imagem(api))
                }
                "restaurar-original" => Some(Destino::Json(api)),
                "bilhete-de-revelacao" | "bilhete-de-bruto" => Some(Destino::Bilhete(api)),
                _ => None,
            }
        }
        ["presets"] => Some(Destino::Json("/revelacao/presets".into())),
        ["presets", "importar"] => Some(Destino::Json("/revelacao/presets/importar".into())),
        ["presets", id] if seguro(id) => Some(Destino::Json(format!("/revelacao/presets/{id}"))),
        ["sobras-locais"] => Some(Destino::Json("/pos-venda/sobras-locais".into())),
        [id, "bilhete"] if seguro(id) => Some(Destino::Bilhete(format!(
            "/pos-venda/galerias/{id}/bilhete"
        ))),
        [id, "revelando"] if seguro(id) => {
            Some(Destino::Json(format!("/pos-venda/galerias/{id}/revelando")))
        }
        _ => None,
    }
}

type Resposta = Response<Cow<'static, [u8]>>;

fn resposta(status: StatusCode, tipo: &str, corpo: Vec<u8>) -> Resposta {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, tipo)
        // Os *workers* e o wasm pedem daqui com `fetch`: sem isto, o WebKit
        // recusaria a resposta.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Cow::Owned(corpo))
        .expect("resposta montada")
}

fn mensagem(status: StatusCode, texto: &str) -> Resposta {
    let corpo = serde_json::json!({ "message": texto })
        .to_string()
        .into_bytes();
    resposta(status, "application/json", corpo)
}

/// Trata um pedido do esquema `vlb://`.
pub async fn tratar<R: Runtime>(app: &AppHandle<R>, pedido: Request<Vec<u8>>) -> Resposta {
    let caminho = pedido.uri().path().to_string();

    if let Some(publico) = caminho.strip_prefix(ENVIO) {
        return repassar_envio(app, &pedido, publico).await;
    }
    if caminho.starts_with(crate::rotas_do_catalogo::PREFIXO) {
        return crate::rotas_do_catalogo::tratar(app, pedido).await;
    }
    if let Some(destino) = rota_interna(&caminho) {
        return repassar(app, &pedido, destino).await;
    }
    arquivo_da_tela(app, &caminho)
}

fn arquivo_da_tela<R: Runtime>(app: &AppHandle<R>, caminho: &str) -> Resposta {
    let resolvedor = app.asset_resolver();
    // Uma rota do React Router não é arquivo: devolve o `index.html`.
    let achado = resolvedor
        .get(caminho.to_string())
        .or_else(|| resolvedor.get("index.html".to_string()));
    match achado {
        Some(asset) => resposta(StatusCode::OK, &asset.mime_type, asset.bytes),
        None => resposta(
            StatusCode::NOT_FOUND,
            "text/plain",
            b"nao encontrado".to_vec(),
        ),
    }
}

fn corpo_do_pedido(pedido: &Request<Vec<u8>>) -> Option<CorpoCru> {
    if pedido.body().is_empty() {
        return None;
    }
    let tipo = pedido
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    Some(CorpoCru {
        tipo,
        bytes: pedido.body().clone(),
    })
}

async fn repassar<R: Runtime>(
    app: &AppHandle<R>,
    pedido: &Request<Vec<u8>>,
    destino: Destino,
) -> Resposta {
    let conta = app.state::<ContaDoApp>();
    let Some(sessao) = conta.sessao().await else {
        return mensagem(StatusCode::UNAUTHORIZED, "a sessão acabou: entre de novo");
    };
    let metodo = pedido.method().as_str();
    let caminho = match &destino {
        Destino::Imagem(c) | Destino::Json(c) | Destino::Bilhete(c) => c.as_str(),
    };
    // O bilhete é pedido sem corpo pela tela, e a API espera `{}`.
    let corpo = corpo_do_pedido(pedido).or_else(|| {
        matches!(destino, Destino::Bilhete(_)).then(|| CorpoCru {
            tipo: "application/json".into(),
            bytes: b"{}".to_vec(),
        })
    });
    let recebido = match conta
        .api
        .chamar(Some(&sessao), metodo, caminho, corpo)
        .await
    {
        Ok(r) => r,
        Err(erro) => return mensagem(StatusCode::BAD_GATEWAY, &erro.to_string()),
    };
    let status = StatusCode::from_u16(recebido.status).unwrap_or(StatusCode::BAD_GATEWAY);
    let tipo = recebido
        .tipo
        .clone()
        .unwrap_or_else(|| "application/octet-stream".into());

    match destino {
        Destino::Imagem(_) => {
            if !status.is_success() {
                // Como o site: 401, 403 e 404 passam; o resto vira 502.
                let status = if matches!(recebido.status, 401 | 403 | 404) {
                    status
                } else {
                    StatusCode::BAD_GATEWAY
                };
                return resposta(status, "text/plain", Vec::new());
            }
            let mut r = resposta(StatusCode::OK, &tipo, recebido.bytes);
            r.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("private, max-age=3600"),
            );
            r
        }
        _ if !status.is_success() => mensagem(status, &mensagem_da_api(&recebido.bytes)),
        Destino::Json(_) => resposta(status, &tipo, recebido.bytes),
        Destino::Bilhete(_) => {
            #[derive(Deserialize)]
            struct Bilhete {
                caminho: String,
                validade_em_segundos: i64,
            }
            match serde_json::from_slice::<Bilhete>(&recebido.bytes) {
                Ok(b) => {
                    let base = origem_do_pedido(pedido);
                    let corpo = serde_json::json!({
                        "url": format!("{base}{ENVIO}{}", b.caminho),
                        "validade_em_segundos": b.validade_em_segundos,
                    });
                    resposta(
                        StatusCode::OK,
                        "application/json",
                        corpo.to_string().into_bytes(),
                    )
                }
                Err(_) => mensagem(StatusCode::BAD_GATEWAY, "bilhete ilegível"),
            }
        }
    }
}

/// O envio por bilhete: rota pública, sem token, com o corpo como veio.
async fn repassar_envio<R: Runtime>(
    app: &AppHandle<R>,
    pedido: &Request<Vec<u8>>,
    publico: &str,
) -> Resposta {
    // 🔒 Só o que um bilhete aponta: rotas da API sob `/api/v2`.
    let Some(caminho) = publico.strip_prefix("/api/v2") else {
        return mensagem(StatusCode::FORBIDDEN, "envio fora da API");
    };
    let caminho = match pedido.uri().query() {
        Some(q) => format!("{caminho}?{q}"),
        None => caminho.to_string(),
    };
    if crate::navegacao::DESENVOLVIMENTO {
        eprintln!(
            "[envio] {} {} corpo {} bytes, tipo {:?}",
            pedido.method(),
            caminho,
            pedido.body().len(),
            pedido.headers().get(header::CONTENT_TYPE)
        );
    }
    let conta = app.state::<ContaDoApp>();
    match conta
        .api
        .chamar(
            None,
            pedido.method().as_str(),
            &caminho,
            corpo_do_pedido(pedido),
        )
        .await
    {
        Ok(r) => {
            let status = StatusCode::from_u16(r.status).unwrap_or(StatusCode::BAD_GATEWAY);
            let tipo = r.tipo.unwrap_or_else(|| "application/json".into());
            resposta(status, &tipo, r.bytes)
        }
        Err(erro) => mensagem(StatusCode::BAD_GATEWAY, &erro.to_string()),
    }
}

fn origem_do_pedido(pedido: &Request<Vec<u8>>) -> String {
    let uri = pedido.uri();
    match (uri.scheme_str(), uri.authority()) {
        (Some(esquema), Some(autoridade)) => format!("{esquema}://{autoridade}"),
        _ => endereco("").as_str().trim_end_matches('/').to_string(),
    }
}

fn mensagem_da_api(bytes: &[u8]) -> String {
    #[derive(Deserialize)]
    struct Envelope {
        error: Option<Detalhe>,
    }
    #[derive(Deserialize)]
    struct Detalhe {
        message: Option<String>,
    }
    serde_json::from_slice::<Envelope>(bytes)
        .ok()
        .and_then(|e| e.error)
        .and_then(|d| d.message)
        .unwrap_or_else(|| String::from_utf8_lossy(bytes).chars().take(200).collect())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn as_imagens_viram_rotas_da_api() {
        assert_eq!(
            rota_interna("/dashboard/sessoes-fotograficas/fotos/abc-1/miniatura"),
            Some(Destino::Imagem("/pos-venda/fotos/abc-1/miniatura".into()))
        );
        assert_eq!(
            rota_interna("/dashboard/sessoes-fotograficas/fotos/abc/original"),
            Some(Destino::Imagem("/pos-venda/fotos/abc/original".into()))
        );
    }

    #[test]
    fn os_bilhetes_e_o_resto() {
        assert_eq!(
            rota_interna("/dashboard/sessoes-fotograficas/g1/bilhete"),
            Some(Destino::Bilhete("/pos-venda/galerias/g1/bilhete".into()))
        );
        assert_eq!(
            rota_interna("/dashboard/sessoes-fotograficas/fotos/f1/bilhete-de-bruto"),
            Some(Destino::Bilhete(
                "/pos-venda/fotos/f1/bilhete-de-bruto".into()
            ))
        );
        assert_eq!(
            rota_interna("/dashboard/sessoes-fotograficas/presets/importar"),
            Some(Destino::Json("/revelacao/presets/importar".into()))
        );
        assert_eq!(
            rota_interna("/dashboard/sessoes-fotograficas/presets/p1"),
            Some(Destino::Json("/revelacao/presets/p1".into()))
        );
        assert_eq!(
            rota_interna("/dashboard/sessoes-fotograficas/sobras-locais"),
            Some(Destino::Json("/pos-venda/sobras-locais".into()))
        );
        assert_eq!(
            rota_interna("/dashboard/sessoes-fotograficas/g1/revelando"),
            Some(Destino::Json("/pos-venda/galerias/g1/revelando".into()))
        );
    }

    #[test]
    fn as_telas_nao_sao_rotas_internas() {
        assert_eq!(rota_interna("/dashboard/sessoes-fotograficas/g1"), None);
        assert_eq!(rota_interna("/dashboard/sessoes-fotograficas/nova"), None);
        assert_eq!(rota_interna("/assets/index.js"), None);
    }

    #[test]
    fn um_id_estranho_nao_vira_caminho_da_api() {
        assert_eq!(
            rota_interna("/dashboard/sessoes-fotograficas/fotos/..%2F/previa"),
            None
        );
        assert_eq!(
            rota_interna("/dashboard/sessoes-fotograficas/fotos/a.b/previa"),
            None
        );
        assert_eq!(
            rota_interna("/dashboard/sessoes-fotograficas/fotos/x/qualquer"),
            None
        );
    }
}
