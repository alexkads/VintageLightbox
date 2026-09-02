//! `PosVendaApi` sobre a API v2 do `recordarfotos.com.br`, com `reqwest`.
//!
//! # As quatro rotas
//!
//! | Porta | Rota |
//! |---|---|
//! | `entrar` | `POST /api/v2/auth/login` → `{ access_token, … }` |
//! | `produtos` | `GET /api/v2/products/admin?limit=200&offset=0` (inclui inativos) |
//! | `criar_galeria` | `POST /api/v2/pos-venda/galerias` → `201 { id, titulo, … }` |
//! | `enviar_foto` | `POST /api/v2/pos-venda/galerias/{id}/fotos`, multipart `file` + `estado` + `ordem` |
//! | `avisar_fotos_prontas` | `POST /api/v2/pos-venda/galerias/{id}/avisar` → `201` |
//!
//! ⚠️ **`native-tls`, e não `rustls`.** O `sqlx` deste crate já traz a pilha
//! TLS do sistema; uma segunda pilha ao lado dela seria compilar duas vezes o
//! mesmo trabalho e carregar dois conjuntos de raízes.
//!
//! A base da URL vem pelo construtor, nunca cravada: é o que permite o teste
//! subir um servidor local, e o estúdio apontar para homologação.

use std::time::Duration;

use async_trait::async_trait;
use domain::services::pos_venda::{
    FotoEnviada, FotoParaEnviar, Galeria, NovaGaleria, PosVendaApi, Produto, Sessao,
};
use domain::{DomainError, DomainResult};
use serde::Deserialize;
use serde_json::json;

/// O teto de uma chamada. Um original de 30 MB numa subida de estúdio leva
/// dezenas de segundos; o padrão do `reqwest` (nenhum) deixaria uma conexão
/// morta pendurada para sempre.
const TEMPO_LIMITE: Duration = Duration::from_secs(180);

pub struct PosVendaApiHttp {
    base: String,
    client: reqwest::Client,
}

impl PosVendaApiHttp {
    /// `base` é a raiz do site da API, sem `/api/v2`: `https://api.recordarfotos.com.br`.
    pub fn nova(base: impl Into<String>) -> Self {
        let base = base.into().trim_end_matches('/').to_string();
        let client = reqwest::Client::builder()
            .timeout(TEMPO_LIMITE)
            .build()
            .expect("o cliente HTTP padrão sempre constrói");
        Self { base, client }
    }

    fn url(&self, caminho: &str) -> String {
        format!("{}/api/v2{caminho}", self.base)
    }
}

#[derive(Deserialize)]
struct TokenPair {
    access_token: String,
}

#[derive(Deserialize)]
struct ProdutoDaApi {
    id: String,
    name: String,
    price: serde_json::Value,
    #[serde(default)]
    inactive: bool,
    #[serde(default)]
    deleted_at: Option<String>,
}

#[derive(Deserialize)]
struct ProdutoComCategoria {
    product: ProdutoDaApi,
}

#[derive(Deserialize)]
struct GaleriaDaApi {
    id: String,
    titulo: String,
}

#[derive(Deserialize)]
struct FotoDaApi {
    id: String,
}

#[derive(Deserialize)]
struct EnvelopeDeErro {
    error: Option<CorpoDeErro>,
}

#[derive(Deserialize)]
struct CorpoDeErro {
    message: Option<String>,
}

fn rede(e: reqwest::Error) -> DomainError {
    DomainError::InfrastructureError(format!("sem resposta do site: {e}"))
}

/// Traduz uma resposta que não é 2xx. `401` no login é credencial; nas outras
/// rotas é sessão vencida — as duas viram [`DomainError::AcessoRecusado`], porque
/// o remédio é o mesmo: entrar de novo.
async fn recusa(resposta: reqwest::Response) -> DomainError {
    let status = resposta.status();
    let corpo = resposta.text().await.unwrap_or_default();
    let mensagem = serde_json::from_str::<EnvelopeDeErro>(&corpo)
        .ok()
        .and_then(|e| e.error)
        .and_then(|e| e.message)
        .unwrap_or_else(|| corpo.chars().take(200).collect());

    if status == reqwest::StatusCode::UNAUTHORIZED {
        return DomainError::AcessoRecusado;
    }
    DomainError::InfrastructureError(format!("o site respondeu {status}: {mensagem}"))
}

async fn ler<T: for<'de> Deserialize<'de>>(resposta: reqwest::Response) -> DomainResult<T> {
    if !resposta.status().is_success() {
        return Err(recusa(resposta).await);
    }
    resposta
        .json::<T>()
        .await
        .map_err(|e| DomainError::InfrastructureError(format!("resposta ilegível: {e}")))
}

#[async_trait]
impl PosVendaApi for PosVendaApiHttp {
    async fn entrar(&self, email: &str, senha: &str) -> DomainResult<Sessao> {
        let resposta = self
            .client
            .post(self.url("/auth/login"))
            .json(&json!({ "email": email, "password": senha }))
            .send()
            .await
            .map_err(rede)?;

        // O site responde 400/422 a e-mail malformado ou senha curta — para quem
        // está na frente do app é a mesma coisa que credencial recusada.
        if matches!(resposta.status().as_u16(), 400 | 401 | 422) {
            return Err(DomainError::AcessoRecusado);
        }

        let tokens: TokenPair = ler(resposta).await?;
        Ok(Sessao {
            access_token: tokens.access_token,
        })
    }

    async fn produtos(&self, sessao: &Sessao) -> DomainResult<Vec<Produto>> {
        let resposta = self
            .client
            .get(self.url("/products/admin?limit=200&offset=0"))
            .bearer_auth(&sessao.access_token)
            .send()
            .await
            .map_err(rede)?;

        let lista: Vec<ProdutoComCategoria> = ler(resposta).await?;
        Ok(lista
            .into_iter()
            .map(|p| p.product)
            // Apagado não vende, e o site recusaria a galeria — não vale mostrar.
            .filter(|p| p.deleted_at.is_none())
            .map(|p| Produto {
                id: p.id,
                nome: p.name,
                preco: match p.price {
                    serde_json::Value::String(s) => s,
                    outro => outro.to_string(),
                },
                inativo: p.inactive,
            })
            .collect())
    }

    async fn criar_galeria(&self, sessao: &Sessao, nova: &NovaGaleria) -> DomainResult<Galeria> {
        let resposta = self
            .client
            .post(self.url("/pos-venda/galerias"))
            .bearer_auth(&sessao.access_token)
            .json(&json!({
                "titulo": nova.titulo,
                "email": nova.email,
                "whatsapp": nova.whatsapp,
                "produto_id": nova.produto_id,
            }))
            .send()
            .await
            .map_err(rede)?;

        let galeria: GaleriaDaApi = ler(resposta).await?;
        Ok(Galeria {
            id: galeria.id,
            titulo: galeria.titulo,
        })
    }

    async fn enviar_foto(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        foto: FotoParaEnviar,
    ) -> DomainResult<FotoEnviada> {
        let arquivo = reqwest::multipart::Part::bytes(foto.jpeg)
            .file_name(foto.nome.clone())
            .mime_str("image/jpeg")
            .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;
        let form = reqwest::multipart::Form::new()
            .text("estado", foto.estado.como_texto())
            .text("ordem", foto.ordem.to_string())
            .part("file", arquivo);

        let resposta = self
            .client
            .post(self.url(&format!("/pos-venda/galerias/{galeria_id}/fotos")))
            .bearer_auth(&sessao.access_token)
            .multipart(form)
            .send()
            .await
            .map_err(rede)?;

        let enviada: FotoDaApi = ler(resposta).await?;
        Ok(FotoEnviada { id: enviada.id })
    }

    async fn avisar_fotos_prontas(&self, sessao: &Sessao, galeria_id: &str) -> DomainResult<()> {
        let resposta = self
            .client
            .post(self.url(&format!("/pos-venda/galerias/{galeria_id}/avisar")))
            .bearer_auth(&sessao.access_token)
            .json(&json!({}))
            .send()
            .await
            .map_err(rede)?;
        if !resposta.status().is_success() {
            return Err(recusa(resposta).await);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::services::pos_venda::EstadoNoBalcao;
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn entrar_devolve_a_sessao_e_senha_errada_e_acesso_recusado() {
        let servidor = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/auth/login"))
            .and(body_string_contains("\"password\":\"certa\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "tok", "refresh_token": "ref", "expires_in": 3600
            })))
            .mount(&servidor)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v2/auth/login"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": { "code": "UNAUTHORIZED", "message": "credenciais invalidas" }
            })))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let sessao = api.entrar("op@x.com", "certa").await.unwrap();
        assert_eq!(sessao.access_token, "tok");

        assert!(matches!(
            api.entrar("op@x.com", "errada").await.unwrap_err(),
            DomainError::AcessoRecusado
        ));
    }

    #[tokio::test]
    async fn produtos_le_o_catalogo_administrativo_e_esconde_apagados() {
        let servidor = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/products/admin"))
            .and(query_param("limit", "200"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "product": { "id": "p1", "name": "Foto avulsa", "price": "29.90", "inactive": true }, "category": {} },
                { "product": { "id": "p2", "name": "Apagado", "price": "1.00", "deleted_at": "2026-01-01T00:00:00Z" }, "category": {} }
            ])))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let sessao = Sessao {
            access_token: "tok".into(),
        };
        let produtos = api.produtos(&sessao).await.unwrap();
        assert_eq!(produtos.len(), 1);
        assert_eq!(produtos[0].id, "p1");
        assert_eq!(produtos[0].preco, "29.90");
        assert!(produtos[0].inativo);
    }

    #[tokio::test]
    async fn enviar_foto_manda_o_multipart_com_o_estado_do_balcao() {
        let servidor = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/pos-venda/galerias/g1/fotos"))
            .and(header("authorization", "Bearer tok"))
            .and(body_string_contains("name=\"estado\""))
            .and(body_string_contains("levada_no_balcao"))
            .and(body_string_contains("name=\"ordem\""))
            .and(body_string_contains("filename=\"DSC_001.jpg\""))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "f1", "galeria_id": "g1", "arquivo": "DSC_001.jpg",
                "estado": "levada_no_balcao", "liberada": true
            })))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let sessao = Sessao {
            access_token: "tok".into(),
        };
        let enviada = api
            .enviar_foto(
                &sessao,
                "g1",
                FotoParaEnviar {
                    nome: "DSC_001.jpg".into(),
                    // ASCII de propósito: o `body_string_contains` do wiremock
                    // não casa corpo que não é UTF-8, e um JPEG de verdade não é.
                    jpeg: b"jpeg-de-mentira".to_vec(),
                    estado: EstadoNoBalcao::LevadaNoBalcao,
                    ordem: 3,
                },
            )
            .await
            .unwrap();
        assert_eq!(enviada.id, "f1");
    }

    #[tokio::test]
    async fn avisar_chama_a_rota_da_galeria_e_traz_a_recusa_do_site() {
        let servidor = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/pos-venda/galerias/g1/avisar"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "id": "a1" })))
            .mount(&servidor)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v2/pos-venda/galerias/g2/avisar"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error": { "code": "BAD_REQUEST", "message": "a galeria nao tem e-mail para receber o aviso" }
            })))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let sessao = Sessao {
            access_token: "tok".into(),
        };
        api.avisar_fotos_prontas(&sessao, "g1").await.unwrap();
        let erro = api.avisar_fotos_prontas(&sessao, "g2").await.unwrap_err();
        assert!(erro.to_string().contains("nao tem e-mail"), "{erro}");
    }

    /// Sessão vencida no meio do lote é "entre de novo", não "sem rede".
    #[tokio::test]
    async fn galeria_recusada_traz_a_mensagem_do_site_e_401_e_acesso_recusado() {
        let servidor = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/pos-venda/galerias"))
            .and(body_string_contains("\"produto_id\":\"nao-existe\""))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "error": { "code": "NOT_FOUND", "message": "produto nao encontrado" }
            })))
            .mount(&servidor)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v2/pos-venda/galerias"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let sessao = Sessao {
            access_token: "tok".into(),
        };
        let nova = |produto: &str| NovaGaleria {
            titulo: "Ensaio".into(),
            email: Some("maria@x.com".into()),
            whatsapp: None,
            produto_id: produto.into(),
        };

        let erro = api
            .criar_galeria(&sessao, &nova("nao-existe"))
            .await
            .unwrap_err();
        assert!(
            erro.to_string().contains("produto nao encontrado"),
            "{erro}"
        );
        assert!(matches!(
            api.criar_galeria(&sessao, &nova("p1")).await.unwrap_err(),
            DomainError::AcessoRecusado
        ));
    }
}
