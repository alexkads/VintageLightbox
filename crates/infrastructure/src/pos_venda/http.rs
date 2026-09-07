//! `PosVendaApi` sobre a API v2 do `recordarfotos.com.br`, com `reqwest`.
//!
//! # As quatro rotas
//!
//! | Porta | Rota |
//! |---|---|
//! | `autorizar_pelo_navegador` | `POST /api/v2/auth/app/token` → o par de tokens |
//! | (renovação automática) | `POST /api/v2/auth/refresh` |
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

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use domain::services::pos_venda::{
    CofreDeSessao, ContagemDeFotos, EstadoDaFotoNoSite, FotoDaGaleria, FotoEnviada, FotoParaEnviar,
    Galeria, GaleriaAberta, GaleriaDoPainel, LinkDeAcesso, MudancaDaFoto, NovaGaleria, PosVendaApi,
    Produto, Sessao, TotaisDaGaleria,
};
use domain::{DomainError, DomainResult};
use serde::Deserialize;
use serde_json::json;

use crate::pos_venda::autorizacao::{abrir_no_navegador, PedidoDeAutorizacao};

/// O teto de uma chamada. Um original de 30 MB numa subida de estúdio leva
/// dezenas de segundos; o padrão do `reqwest` (nenhum) deixaria uma conexão
/// morta pendurada para sempre.
const TEMPO_LIMITE: Duration = Duration::from_secs(180);

/// Onde o **site** mora — é ele que autentica, e não a API.
///
/// Sobrescrito por `VLB_SITE_URL`, para apontar a homologação sem recompilar.
/// Separado da base da API porque são dois endereços: a API responde em
/// `api.recordarfotos.com.br`, e quem abre a tela de autorização é o site.
pub const SITE_PADRAO: &str = "https://recordarfotos.com.br";

pub struct PosVendaApiHttp {
    base: String,
    site: String,
    client: reqwest::Client,
    /// A sessão que vale **agora** — renovada por baixo das telas.
    ///
    /// 🔑 **`Mutex` do tokio, e o guard é mantido durante a renovação.** Numa
    /// subida de trinta fotos há trinta chamadas em voo; sem serializar, todas
    /// veriam o token vencido no mesmo instante e trinta renovações sairiam ao
    /// mesmo tempo. Com o guard segurado, a primeira renova e as outras já
    /// encontram o token novo.
    viva: tokio::sync::Mutex<Option<Sessao>>,
    /// Onde a sessão dorme entre uma abertura do app e a seguinte.
    cofre: Arc<dyn CofreDeSessao>,
    /// Quem leva o operador à tela de autorização.
    ///
    /// Injetável por um motivo prático: em produção isto abre o navegador da
    /// máquina, e um teste que exercitasse o fluxo abriria uma janela de verdade
    /// na cara de quem roda `cargo test`. Com a porta aqui, o teste faz o papel
    /// do navegador — e, de quebra, o fluxo inteiro passa a ser exercitável,
    /// incluindo a costura entre a URL anunciada e a porta que está escutando.
    abridor: Arc<dyn Fn(&str) + Send + Sync>,
}

impl PosVendaApiHttp {
    /// `base` é a raiz do site da API, sem `/api/v2`: `https://api.recordarfotos.com.br`.
    pub fn nova(base: impl Into<String>) -> Self {
        let base = base.into().trim_end_matches('/').to_string();
        let client = reqwest::Client::builder()
            .timeout(TEMPO_LIMITE)
            .build()
            .expect("o cliente HTTP padrão sempre constrói");
        Self {
            base,
            site: std::env::var("VLB_SITE_URL")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| SITE_PADRAO.to_string()),
            client,
            viva: tokio::sync::Mutex::new(None),
            cofre: Arc::new(crate::pos_venda::cofre::CofreEmMemoria::default()),
            abridor: Arc::new(|url: &str| {
                if !abrir_no_navegador(url) {
                    // Não é fim de fluxo: o servidor local já espera, e o
                    // operador ainda pode abrir o endereço à mão.
                    eprintln!("⚠️  Não foi possível abrir o navegador. Abra este endereço:\n{url}");
                }
            }),
        }
    }

    /// Troca quem leva o operador à tela — ver [`PosVendaApiHttp::abridor`].
    pub fn com_abridor(mut self, abridor: Arc<dyn Fn(&str) + Send + Sync>) -> Self {
        self.abridor = abridor;
        self
    }

    /// Liga o chaveiro do sistema. Sem isto a sessão morre com o processo — é o
    /// que o teste usa, e o que vale se o chaveiro não existir na máquina.
    pub fn com_cofre(mut self, cofre: Arc<dyn CofreDeSessao>) -> Self {
        self.cofre = cofre;
        self
    }

    /// Aponta o site que autentica — o teste manda o próprio servidor local.
    pub fn com_site(mut self, site: impl Into<String>) -> Self {
        self.site = site.into().trim_end_matches('/').to_string();
        self
    }

    fn url(&self, caminho: &str) -> String {
        format!("{}/api/v2{caminho}", self.base)
    }
}

/// O que o backend devolve em `/auth/app/token` e `/auth/refresh`.
#[derive(Deserialize)]
struct TokenPair {
    access_token: String,
    refresh_token: String,
    /// Segundos até o de acesso vencer.
    expires_in: i64,
    /// Segundos até o de renovação vencer — quinze dias, para esta classe de
    /// cliente. Ausente em backend anterior a esta entrega: o padrão conservador
    /// é sete dias, que era o que ele emitia.
    #[serde(default = "sete_dias")]
    refresh_expires_in: i64,
}

fn sete_dias() -> i64 {
    7 * 86_400
}

impl TokenPair {
    fn em_sessao(self, agora: i64) -> Sessao {
        Sessao {
            access_token: self.access_token,
            refresh_token: self.refresh_token,
            access_vence_em: agora + self.expires_in,
            refresh_vence_em: agora + self.refresh_expires_in,
        }
    }
}

fn agora() -> i64 {
    chrono::Utc::now().timestamp()
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
    async fn autorizar_pelo_navegador(&self) -> DomainResult<Sessao> {
        // O servidor local sobe **antes** do navegador: se ele abrisse depois, a
        // volta poderia chegar numa porta que ainda não escuta, e o operador
        // veria "não foi possível conectar" numa autorização que deu certo.
        let pedido = PedidoDeAutorizacao::novo().await?;
        (self.abridor)(&pedido.url(&self.site));

        let code = pedido.esperar_codigo().await?;

        let resposta = self
            .client
            .post(self.url("/auth/app/token"))
            .json(&json!({ "code": code, "verificador": pedido.verificador }))
            .send()
            .await
            .map_err(rede)?;

        // `403` é conta sem direito de operar o estúdio; `400` é código vencido
        // ou já gasto. Para quem está na frente do app as duas terminam do mesmo
        // jeito — autorizar de novo, talvez com outra conta.
        if matches!(resposta.status().as_u16(), 400 | 401 | 403) {
            return Err(DomainError::AcessoRecusado);
        }

        let par: TokenPair = ler(resposta).await?;
        let sessao = par.em_sessao(agora());
        self.adotar(sessao.clone()).await;
        Ok(sessao)
    }

    async fn retomar_sessao(&self) -> DomainResult<Option<Sessao>> {
        let Some(guardada) = self.cofre.ler() else {
            return Ok(None);
        };

        // Passou dos quinze dias: não há o que renovar, e insistir só gastaria
        // uma ida ao servidor para ouvir 401.
        if !guardada.renovavel(agora()) {
            self.cofre.esquecer();
            return Ok(None);
        }

        *self.viva.lock().await = Some(guardada.clone());
        Ok(Some(guardada))
    }

    async fn sair(&self) {
        *self.viva.lock().await = None;
        self.cofre.esquecer();
    }

    async fn produtos(&self, sessao: &Sessao) -> DomainResult<Vec<Produto>> {
        let resposta = self
            .client
            .get(self.url("/products/admin?limit=200&offset=0"))
            .bearer_auth(self.token(sessao).await?)
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
            .bearer_auth(self.token(sessao).await?)
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
            .bearer_auth(self.token(sessao).await?)
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
            .bearer_auth(self.token(sessao).await?)
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
            .bearer_auth(self.token(sessao).await?)
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
            .bearer_auth(self.token(sessao).await?)
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
            .bearer_auth(self.token(sessao).await?)
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
            .bearer_auth(self.token(sessao).await?)
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
            .bearer_auth(self.token(sessao).await?)
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
    /// O token que a próxima chamada deve carregar — renovado se preciso.
    ///
    /// # Por que a sessão recebida não é a autoridade
    ///
    /// A tela guarda a sessão de quando entrou e a passa em toda chamada; o
    /// token dentro dela envelhece em quinze minutos. Quem sabe o token de agora
    /// é este cliente, que renovou por baixo. A sessão do parâmetro serve para
    /// **semear** a viva — no primeiro uso depois de o app abrir com o que
    /// estava no chaveiro.
    async fn token(&self, sessao: &Sessao) -> DomainResult<String> {
        let agora = agora();
        let mut viva = self.viva.lock().await;
        let atual = viva.get_or_insert_with(|| sessao.clone()).clone();

        if atual.acesso_utilizavel(agora) {
            return Ok(atual.access_token);
        }

        // Sem renovação possível a sessão acabou de verdade: `AcessoRecusado`
        // manda a tela pedir autorização de novo, que é o único remédio.
        if !atual.renovavel(agora) {
            self.cofre.esquecer();
            return Err(DomainError::AcessoRecusado);
        }

        let renovada = self.renovar(&atual.refresh_token).await?;
        *viva = Some(renovada.clone());
        // Guardar depois de a renovação dar certo, nunca antes: gravar um par
        // que não chegou a valer deixaria o chaveiro apontando para uma sessão
        // que não existe.
        self.cofre.guardar(&renovada);
        Ok(renovada.access_token)
    }

    /// Troca o refresh token por um par novo. O papel é relido do banco pelo
    /// backend — quem foi rebaixado não continua operando por quinze dias.
    async fn renovar(&self, refresh_token: &str) -> DomainResult<Sessao> {
        let resposta = self
            .client
            .post(self.url("/auth/refresh"))
            .json(&json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .map_err(rede)?;

        if resposta.status() == reqwest::StatusCode::UNAUTHORIZED {
            self.cofre.esquecer();
            return Err(DomainError::AcessoRecusado);
        }

        let par: TokenPair = ler(resposta).await?;
        Ok(par.em_sessao(agora()))
    }

    /// Passa a valer esta sessão, aqui e no chaveiro.
    async fn adotar(&self, sessao: Sessao) {
        *self.viva.lock().await = Some(sessao.clone());
        self.cofre.guardar(&sessao);
    }

    /// As rotas de imagem devolvem **bytes**, e não JSON.
    ///
    /// 🔑 Passar por `ler` desserializaria e falharia com "resposta ilegível"
    /// num corpo perfeitamente legível — o tipo de erro que manda procurar
    /// defeito no site.
    async fn bytes_da_imagem(&self, sessao: &Sessao, caminho: &str) -> DomainResult<Vec<u8>> {
        let resposta = self
            .client
            .get(self.url(caminho))
            .bearer_auth(self.token(sessao).await?)
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
    use crate::pos_venda::cofre::CofreEmMemoria;
    use domain::services::pos_venda::EstadoNoBalcao;
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Uma sessão com folga nos dois prazos — o caso comum das chamadas.
    fn sessao_valida() -> Sessao {
        Sessao {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            access_vence_em: agora() + 900,
            refresh_vence_em: agora() + 15 * 86_400,
        }
    }

    #[tokio::test]
    async fn a_sessao_do_chaveiro_e_retomada_e_a_vencida_e_esquecida() {
        let servidor = MockServer::start().await;

        // Uma sessão de ontem, ainda dentro dos quinze dias.
        let cofre = Arc::new(CofreEmMemoria::default());
        cofre.guardar(&Sessao {
            access_token: "tok-velho".into(),
            refresh_token: "ref".into(),
            access_vence_em: agora() - 10,
            refresh_vence_em: agora() + 10 * 86_400,
        });

        let api = PosVendaApiHttp::nova(servidor.uri()).com_cofre(cofre.clone());
        assert_eq!(
            api.retomar_sessao().await.unwrap().unwrap().refresh_token,
            "ref"
        );

        // Passados os quinze dias não há o que renovar, e o chaveiro é limpo:
        // manter a sessão lá faria toda abertura do app tentar e falhar.
        cofre.guardar(&Sessao {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            access_vence_em: agora() - 86_400,
            refresh_vence_em: agora() - 10,
        });
        assert!(api.retomar_sessao().await.unwrap().is_none());
        assert!(cofre.ler().is_none());
    }

    /// 🔑 O coração da entrega: token de acesso vencido **não** deixa a sessão
    /// morrer — o cliente renova sozinho e a chamada segue.
    #[tokio::test]
    async fn o_acesso_vencido_e_renovado_por_baixo_e_a_chamada_continua() {
        let servidor = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/auth/refresh"))
            .and(body_string_contains("\"refresh_token\":\"ref-1\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "tok-novo",
                "refresh_token": "ref-2",
                "expires_in": 900,
                "refresh_expires_in": 15 * 86_400
            })))
            .mount(&servidor)
            .await;
        // A chamada seguinte só passa com o token **novo** — é isso que prova
        // que a renovação chegou até o cabeçalho.
        Mock::given(method("GET"))
            .and(path("/api/v2/products/admin"))
            .and(header("authorization", "Bearer tok-novo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&servidor)
            .await;

        let cofre = Arc::new(CofreEmMemoria::default());
        let api = PosVendaApiHttp::nova(servidor.uri()).com_cofre(cofre.clone());

        let vencida = Sessao {
            access_token: "tok-vencido".into(),
            refresh_token: "ref-1".into(),
            access_vence_em: agora() - 10,
            refresh_vence_em: agora() + 15 * 86_400,
        };
        api.produtos(&vencida).await.unwrap();

        // E o par novo fica guardado: a próxima abertura do app não reautoriza.
        let guardada = cofre.ler().expect("a sessão renovada vai para o chaveiro");
        assert_eq!(guardada.access_token, "tok-novo");
        assert_eq!(guardada.refresh_token, "ref-2");
        assert!(guardada.refresh_vence_em - agora() > 14 * 86_400);
    }

    /// Refresh recusado é sessão acabada: o chaveiro é limpo, para o app não
    /// tentar de novo amanhã com o que já não vale.
    #[tokio::test]
    async fn refresh_recusado_limpa_o_chaveiro_e_pede_autorizacao_de_novo() {
        let servidor = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/auth/refresh"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": { "code": "UNAUTHORIZED", "message": "expirado" }
            })))
            .mount(&servidor)
            .await;

        let cofre = Arc::new(CofreEmMemoria::default());
        let api = PosVendaApiHttp::nova(servidor.uri()).com_cofre(cofre.clone());
        let vencida = Sessao {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            access_vence_em: agora() - 10,
            refresh_vence_em: agora() + 86_400,
        };
        cofre.guardar(&vencida);

        assert!(matches!(
            api.produtos(&vencida).await.unwrap_err(),
            DomainError::AcessoRecusado
        ));
        assert!(cofre.ler().is_none());
    }

    /// Sem renovação possível, nem se tenta: uma ida ao servidor para ouvir 401
    /// atrasa a tela sem mudar o desfecho.
    #[tokio::test]
    async fn sessao_alem_dos_quinze_dias_nao_chama_o_servidor() {
        let servidor = MockServer::start().await;
        let api = PosVendaApiHttp::nova(servidor.uri());

        let morta = Sessao {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            access_vence_em: agora() - 86_400,
            refresh_vence_em: agora() - 10,
        };

        assert!(matches!(
            api.produtos(&morta).await.unwrap_err(),
            DomainError::AcessoRecusado
        ));
        // Nenhum `Mock` foi montado: se tivesse havido chamada, o wiremock
        // responderia 404 e o erro seria outro.
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
        let sessao = sessao_valida();
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
        let sessao = sessao_valida();
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
        let sessao = sessao_valida();
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
        let sessao = sessao_valida();
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
        let galerias = api.galerias(&sessao_valida()).await.unwrap();

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
        let sessao = sessao_valida();

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
        api.mudar_foto(&sessao_valida(), "f1", &MudancaDaFoto::default())
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
        let sessao = sessao_valida();
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
        let link = api.link_da_galeria(&sessao_valida(), "g1").await.unwrap();

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
        let sessao = sessao_valida();

        let bytes = api.copia_de_trabalho(&sessao, "f1").await.unwrap();
        assert_eq!(bytes, vec![0xFF, 0xD8, 0xFF, 0xE0], "o começo de um JPEG");

        let erro = api.copia_de_trabalho(&sessao, "f9").await.unwrap_err();
        assert!(
            matches!(erro, DomainError::NaoEncontradoNoSite(_)),
            "a foto que saiu do site tem desfecho próprio: {erro}"
        );
    }
}
