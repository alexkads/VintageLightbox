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
    ContagemDeFotos, EstadoDaFotoNoSite, FotoDaGaleria, FotoEnviada, FotoParaEnviar, Galeria,
    GaleriaAberta, GaleriaDoPainel, LinkDeAcesso, MudancaDaFoto, NovaGaleria, PosVendaApi, Produto,
    Sessao, TotaisDaGaleria,
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
    // 🔑 O `404` sai como variante própria porque quem remove precisa distinguir
    // "já não está lá" (o desfecho desejado) de "não deu para falar com o site".
    if status == reqwest::StatusCode::NOT_FOUND {
        return DomainError::NaoEncontradoNoSite(mensagem);
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

    async fn galerias(&self, sessao: &Sessao) -> DomainResult<Vec<GaleriaDoPainel>> {
        let resposta = self
            .client
            .get(self.url("/pos-venda/galerias"))
            .bearer_auth(&sessao.access_token)
            .send()
            .await
            .map_err(rede)?;

        let lista: Vec<GaleriaDoPainelDaApi> = ler(resposta).await?;
        Ok(lista.into_iter().map(GaleriaDoPainel::from).collect())
    }

    async fn mudar_foto(
        &self,
        sessao: &Sessao,
        foto_id: &str,
        mudanca: &MudancaDaFoto,
    ) -> DomainResult<()> {
        // O site recusa um `PATCH` sem campo nenhum, e com razão. Sair aqui é
        // não gastar uma ida à rede para trazer esse erro de volta.
        if mudanca.vazia() {
            return Ok(());
        }

        let mut corpo = serde_json::Map::new();
        if let Some(estado) = mudanca.estado {
            corpo.insert("estado".into(), json!(estado.como_texto()));
        }
        // 🔑 Os três estados de cada campo sobrevivem à serialização: ausente do
        // mapa é "não mexer", `null` no mapa é "apagar". Um `Option` comum
        // achataria os dois em ausente.
        if let Some(preco) = &mudanca.preco_negociado {
            corpo.insert("preco_negociado".into(), json!(preco));
        }
        if let Some(observacao) = &mudanca.observacao_da_negociacao {
            corpo.insert("observacao_da_negociacao".into(), json!(observacao));
        }
        if let Some(nota) = &mudanca.nota {
            corpo.insert("nota".into(), json!(nota));
        }

        let resposta = self
            .client
            .patch(self.url(&format!("/pos-venda/fotos/{foto_id}")))
            .bearer_auth(&sessao.access_token)
            .json(&serde_json::Value::Object(corpo))
            .send()
            .await
            .map_err(rede)?;

        if !resposta.status().is_success() {
            return Err(recusa(resposta).await);
        }
        Ok(())
    }

    async fn remover_foto(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<()> {
        let resposta = self
            .client
            .delete(self.url(&format!("/pos-venda/fotos/{foto_id}")))
            .bearer_auth(&sessao.access_token)
            .send()
            .await
            .map_err(rede)?;

        if !resposta.status().is_success() {
            return Err(recusa(resposta).await);
        }
        Ok(())
    }

    async fn link_da_galeria(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
    ) -> DomainResult<LinkDeAcesso> {
        let resposta = self
            .client
            .post(self.url(&format!("/pos-venda/galerias/{galeria_id}/link")))
            .bearer_auth(&sessao.access_token)
            .json(&json!({}))
            .send()
            .await
            .map_err(rede)?;

        let link: LinkDaApi = ler(resposta).await?;
        Ok(LinkDeAcesso {
            url: link.link,
            validade_em_segundos: link.validade_em_segundos,
        })
    }

    async fn abrir_galeria(&self, sessao: &Sessao, id: &str) -> DomainResult<GaleriaAberta> {
        let resposta = self
            .client
            .get(self.url(&format!("/pos-venda/galerias/{id}")))
            .bearer_auth(&sessao.access_token)
            .send()
            .await
            .map_err(rede)?;

        let aberta: GaleriaAbertaDaApi = ler(resposta).await?;
        Ok(GaleriaAberta {
            galeria: GaleriaDoPainel::from(aberta.galeria),
            fotos: aberta
                .fotos
                .into_iter()
                .map(|f| FotoDaGaleria {
                    id: f.id,
                    arquivo: f.arquivo,
                    estado: EstadoDaFotoNoSite::do_texto(&f.estado),
                    ordem: f.ordem,
                    preco_negociado: f.preco_negociado,
                    observacao_da_negociacao: f.observacao_da_negociacao,
                    apagada: f.apagada_em.is_some(),
                    nota: f.nota,
                    produto_efetivo: f.produto_efetivo,
                    preco_de_venda: f.preco_de_venda,
                    pedido_id: f.pedido_id,
                    downloads: f.downloads,
                    // `ajustes` é `null` enquanto ninguém revelou a foto.
                    revelada: f.ajustes.is_some(),
                })
                .collect(),
            vence_venda: aberta.vence_venda.map(|q| q.timestamp()),
            vence_download: aberta.vence_download.map(|q| q.timestamp()),
        })
    }

    async fn miniatura(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<Vec<u8>> {
        self.bytes_da_imagem(sessao, &format!("/pos-venda/fotos/{foto_id}/miniatura"))
            .await
    }

    async fn copia_de_trabalho(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<Vec<u8>> {
        self.bytes_da_imagem(
            sessao,
            &format!("/pos-venda/fotos/{foto_id}/copia-de-trabalho"),
        )
        .await
    }
}

impl PosVendaApiHttp {
    /// As rotas de imagem devolvem **bytes**, e não JSON.
    ///
    /// 🔑 Passar por `ler` desserializaria e falharia com "resposta ilegível"
    /// num corpo perfeitamente legível — o tipo de erro que manda procurar
    /// defeito no site.
    async fn bytes_da_imagem(&self, sessao: &Sessao, caminho: &str) -> DomainResult<Vec<u8>> {
        let resposta = self
            .client
            .get(self.url(caminho))
            .bearer_auth(&sessao.access_token)
            .send()
            .await
            .map_err(rede)?;

        if !resposta.status().is_success() {
            return Err(recusa(resposta).await);
        }
        resposta
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| DomainError::InfrastructureError(format!("imagem incompleta: {e}")))
    }
}

/// A galeria como o painel a lista. O que não interessa ao balcão (quem criou,
/// o ensaio, o estúdio) fica de fora: o serde ignora o que sobra.
#[derive(Deserialize)]
struct GaleriaDoPainelDaApi {
    id: String,
    titulo: String,
    email: Option<String>,
    whatsapp: Option<String>,
    produto_id: String,
    #[serde(default)]
    user_id: Option<String>,
    criada_em: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    expira_em: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    fotos: ContagemDaApi,
    /// ⚠️ **`Option`, e não zero por padrão**: a galeria de uma API anterior ao
    /// campo precisa chegar como "não sei", para o rodapé poder dizer que o
    /// total está menor que o real. Zero calado é a única resposta errada aqui.
    #[serde(default)]
    totais: Option<TotaisDaApi>,
}

#[derive(Deserialize, Default)]
struct ContagemDaApi {
    #[serde(default)]
    levadas_no_balcao: u32,
    #[serde(default)]
    disponiveis: u32,
    #[serde(default)]
    compradas: u32,
    #[serde(default)]
    apagadas: u32,
}

#[derive(Deserialize)]
struct TotaisDaApi {
    balcao: String,
    pos_venda: String,
}

impl From<GaleriaDoPainelDaApi> for GaleriaDoPainel {
    fn from(g: GaleriaDoPainelDaApi) -> Self {
        Self {
            id: g.id,
            titulo: g.titulo,
            email: g.email,
            whatsapp: g.whatsapp,
            produto_id: g.produto_id,
            user_id: g.user_id,
            // 🔑 Reduzida ao dia **aqui**, e não na tela: o eixo do gráfico é
            // por dia, e guardar a hora daria duas galerias do mesmo dia em
            // degraus diferentes assim que alguém esquecesse de cortar.
            criada_em_iso: g.criada_em.format("%Y-%m-%d").to_string(),
            expira_em: g.expira_em.map(|quando| quando.timestamp()),
            fotos: ContagemDeFotos {
                levadas_no_balcao: g.fotos.levadas_no_balcao,
                disponiveis: g.fotos.disponiveis,
                compradas: g.fotos.compradas,
                apagadas: g.fotos.apagadas,
            },
            totais: g.totais.map(|t| TotaisDaGaleria {
                balcao: t.balcao,
                pos_venda: t.pos_venda,
            }),
        }
    }
}

#[derive(Deserialize)]
struct GaleriaAbertaDaApi {
    galeria: GaleriaDoPainelDaApi,
    fotos: Vec<FotoDaGaleriaDaApi>,
    #[serde(default)]
    vence_venda: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    vence_download: Option<chrono::DateTime<chrono::Utc>>,
}

/// A foto como o painel a devolve. O que a grade do desktop não usa (downloads,
/// pedido, tamanho, os 46 ajustes) fica de fora — o serde ignora o que sobra.
///
/// ⚠️ Nome distinto do `FotoDaApi` que a **subida** usa: aquele é a resposta de
/// `POST …/fotos`, e traz só o id.
#[derive(Deserialize)]
struct FotoDaGaleriaDaApi {
    id: String,
    arquivo: String,
    estado: String,
    #[serde(default)]
    ordem: i32,
    #[serde(default)]
    preco_negociado: Option<String>,
    #[serde(default)]
    observacao_da_negociacao: Option<String>,
    #[serde(default)]
    apagada_em: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    nota: Option<u8>,
    #[serde(default)]
    produto_efetivo: String,
    #[serde(default)]
    preco_de_venda: Option<String>,
    #[serde(default)]
    pedido_id: Option<String>,
    #[serde(default)]
    downloads: u32,
    /// Os 46 ajustes da revelação feita no navegador. Aqui só interessa se
    /// existem — quem revela lê os valores do próprio motor.
    #[serde(default)]
    ajustes: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct LinkDaApi {
    link: String,
    validade_em_segundos: i64,
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

    /// A lista de galerias traz o contato, e ignora o que o painel manda a mais.
    ///
    /// 🔑 O contato é o que identifica o cliente no balcão: duas galerias com o
    /// mesmo título e clientes diferentes é o caso comum de um estúdio, e
    /// escolher a errada manda as fotos de um cliente para outro.
    #[tokio::test]
    async fn galerias_lista_as_que_existem_com_o_contato() {
        let servidor = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/pos-venda/galerias"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {
                    "id": "g1",
                    "titulo": "Ana — 12/09",
                    "email": "ana@exemplo.com",
                    "whatsapp": null,
                    "produto_id": "p1",
                    "user_id": "u1",
                    "criada_em": "2026-09-06T12:00:00Z",
                    "expira_em": "2026-10-07T12:00:00Z",
                    "fotos": {
                        "levadas_no_balcao": 4,
                        "disponiveis": 8,
                        "compradas": 0,
                        "apagadas": 0
                    },
                    "totais": { "balcao": "150.00", "pos_venda": "45.00" },
                    // O painel manda mais que isto; o serde ignora o que sobra.
                    "criada_por": "operador",
                    "ensaio_id": null,
                    "estudio_id": null
                },
                {
                    "id": "g2",
                    "titulo": "Bruno",
                    "email": null,
                    "whatsapp": "5551999998888",
                    "produto_id": "p1",
                    "criada_em": "2026-09-06T13:00:00Z",
                    "criada_por": "operador"
                }
            ])))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let galerias = api
            .galerias(&Sessao {
                access_token: "tok".into(),
            })
            .await
            .unwrap();

        assert_eq!(galerias.len(), 2);
        assert_eq!(galerias[0].id, "g1");
        assert_eq!(galerias[0].email.as_deref(), Some("ana@exemplo.com"));
        assert_eq!(galerias[1].whatsapp.as_deref(), Some("5551999998888"));
        assert_eq!(galerias[1].email, None);

        // A data chega reduzida ao dia — é o carimbo do eixo do gráfico.
        assert_eq!(galerias[0].criada_em_iso, "2026-09-06");
        assert_eq!(galerias[0].fotos.levadas_no_balcao, 4);
        assert_eq!(galerias[0].fotos.disponiveis, 8);
        assert_eq!(
            galerias[0].totais.as_ref().map(|t| t.balcao.as_str()),
            Some("150.00")
        );

        // 🚨 A galeria sem o campo `totais` chega como "não sei", e não como
        // zero: é o que permite ao rodapé dizer que o total está menor que o
        // real, em vez de anunciar um número curto como se fosse completo.
        assert_eq!(galerias[1].totais, None);
        assert_eq!(galerias[1].expira_em, None, "sem prazo é sem prazo");
        assert_eq!(
            galerias[0].expira_em,
            Some(1_791_374_400),
            "o prazo vira segundos, que é o que a conta da situação compara"
        );
    }

    /// 🚨 Os três estados de cada campo sobrevivem à ida pela rede.
    ///
    /// Ausente é "não mexer", `null` é "apagar", valor é "gravar". Se o
    /// `Option<Option<_>>` fosse achatado num `Option`, "não mexer no preço" e
    /// "voltar ao preço da faixa" virariam a mesma requisição — e o operador
    /// que só quisesse anotar o motivo apagaria o valor sem pedir.
    #[tokio::test]
    async fn mudar_foto_distingue_ausente_de_nulo() {
        let servidor = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v2/pos-venda/fotos/f1"))
            .and(header("authorization", "Bearer tok"))
            .and(body_string_contains("\"preco_negociado\":\"15.00\""))
            .and(body_string_contains(
                "\"observacao_da_negociacao\":\"Desconto",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "f1" })))
            .mount(&servidor)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v2/pos-venda/fotos/f2"))
            .and(body_string_contains("\"preco_negociado\":null"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "f2" })))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let sessao = Sessao {
            access_token: "tok".into(),
        };

        api.mudar_foto(
            &sessao,
            "f1",
            &MudancaDaFoto {
                preco_negociado: Some(Some("15.00".into())),
                observacao_da_negociacao: Some(Some("Desconto — cliente antigo".into())),
                ..MudancaDaFoto::default()
            },
        )
        .await
        .unwrap();

        api.mudar_foto(
            &sessao,
            "f2",
            &MudancaDaFoto {
                preco_negociado: Some(None),
                ..MudancaDaFoto::default()
            },
        )
        .await
        .unwrap();
    }

    /// ⚠️ Mudança vazia não vira ida à rede.
    ///
    /// O site recusa um `PATCH` sem campo nenhum, e com razão. Sair antes é não
    /// gastar uma viagem para trazer esse erro de volta — e o `MockServer` sem
    /// nenhuma rota montada é o que prova que ninguém saiu daqui.
    #[tokio::test]
    async fn mudar_foto_com_nada_a_mudar_nao_chama_o_site() {
        let servidor = MockServer::start().await;
        let api = PosVendaApiHttp::nova(servidor.uri());
        api.mudar_foto(
            &Sessao {
                access_token: "tok".into(),
            },
            "f1",
            &MudancaDaFoto::default(),
        )
        .await
        .unwrap();
        assert!(
            servidor.received_requests().await.unwrap().is_empty(),
            "nada podia ter saído daqui"
        );
    }

    /// Remover é o que zerar a classificação faz: a foto sai do storage e volta
    /// a ser só local. O site responde `204`, sem corpo.
    #[tokio::test]
    async fn remover_foto_aceita_o_204_sem_corpo() {
        let servidor = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/v2/pos-venda/fotos/f1"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&servidor)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/api/v2/pos-venda/fotos/f9"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "error": { "code": "NOT_FOUND", "message": "foto nao encontrada" }
            })))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let sessao = Sessao {
            access_token: "tok".into(),
        };
        api.remover_foto(&sessao, "f1").await.unwrap();
        let erro = api.remover_foto(&sessao, "f9").await.unwrap_err();
        assert!(erro.to_string().contains("nao encontrada"), "{erro}");
    }

    /// 🚨 O link vem do backend, e não é montado aqui.
    ///
    /// `/meus-ensaios/{id}` exige sessão, e o cliente não tem conta — ele saiu
    /// do estúdio, não do site. Montar o endereço no app daria um link que
    /// parece certo e leva ao `/login`. Foi o defeito que a web teve até
    /// 4/set/2026.
    #[tokio::test]
    async fn o_link_da_galeria_vem_assinado_do_site() {
        let servidor = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/pos-venda/galerias/g1/link"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "link": "https://recordarfotos.com.br/entrar?t=abc123",
                "validade_em_segundos": 604800
            })))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let link = api
            .link_da_galeria(
                &Sessao {
                    access_token: "tok".into(),
                },
                "g1",
            )
            .await
            .unwrap();

        assert_eq!(link.url, "https://recordarfotos.com.br/entrar?t=abc123");
        assert_eq!(link.validade_em_segundos, 604_800);
    }

    /// 🚨 A cópia de trabalho vem como **bytes**, e não como JSON.
    ///
    /// A rota devolve a imagem. Passar a resposta pelo `ler` genérico falharia
    /// com "resposta ilegível" num corpo perfeitamente legível — o tipo de erro
    /// que manda procurar defeito no site.
    #[tokio::test]
    async fn a_copia_de_trabalho_vem_como_bytes() {
        let servidor = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/pos-venda/fotos/f1/copia-de-trabalho"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![0xFF, 0xD8, 0xFF, 0xE0])
                    .insert_header("content-type", "image/jpeg"),
            )
            .mount(&servidor)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/pos-venda/fotos/f9/copia-de-trabalho"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "error": { "code": "NOT_FOUND", "message": "foto nao encontrada" }
            })))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let sessao = Sessao {
            access_token: "tok".into(),
        };

        let bytes = api.copia_de_trabalho(&sessao, "f1").await.unwrap();
        assert_eq!(bytes, vec![0xFF, 0xD8, 0xFF, 0xE0], "o começo de um JPEG");

        let erro = api.copia_de_trabalho(&sessao, "f9").await.unwrap_err();
        assert!(
            matches!(erro, DomainError::NaoEncontradoNoSite(_)),
            "a foto que saiu do site tem desfecho próprio: {erro}"
        );
    }
}
