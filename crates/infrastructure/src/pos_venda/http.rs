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
//! | `original` | `GET /api/v2/pos-venda/fotos/{id}/original` — o arquivo cheio, não a cópia de trabalho |
//! | `bilhete_de_revelacao` | `POST /api/v2/pos-venda/fotos/{id}/bilhete-de-revelacao` → o bilhete de uma hora |
//! | `salvar_revelacao` | `POST /api/v2/public/pos-venda/revelacao/{bilhete}`, multipart `file` + `ajustes` (sem token) |
//! | `restaurar_original` | `POST /api/v2/pos-venda/fotos/{id}/restaurar-original` — o bruto volta ao lugar, sem upload |
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
    CofreDeSessao, ContagemDeFotos, EstadoDaFotoNoSite, Estudio, FotoDaGaleria, FotoEnviada,
    FotoParaEnviar, Galeria, GaleriaAberta, GaleriaDoPainel, LinkDeAcesso, MudancaDaFoto,
    MudancaDaGaleria, NovaGaleria, PagoNoCaixa, PosVendaApi, Produto, ResumoSimples,
    ResumosDoAtendimento, Sessao, TotaisDaGaleria,
};
use domain::{DomainError, DomainResult};
use serde::Deserialize;
use serde_json::json;

use crate::pos_venda::autorizacao::{abrir_no_navegador, PedidoDeAutorizacao};

/// O teto de uma chamada. Um original de 30 MB numa subida de estúdio leva
/// dezenas de segundos; o padrão do `reqwest` (nenhum) deixaria uma conexão
/// morta pendurada para sempre.
const TEMPO_LIMITE: Duration = Duration::from_secs(180);

/// O teto de um fluxo de eventos ([`PosVendaApiHttp::escutar`]). Não é o que
/// acusa conexão morta — isso é o silêncio. É só para nenhum fluxo viver para
/// sempre: passada meia hora ele fecha, e quem escuta reconecta com o token de
/// agora.
const TETO_DO_FLUXO: Duration = Duration::from_secs(30 * 60);

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
    normal_price: Option<serde_json::Value>,
    #[serde(default)]
    inactive: bool,
    #[serde(default)]
    deleted_at: Option<String>,
}

/// Um estúdio como `GET /bookings/studios` devolve — só o que a tela usa.
#[derive(Deserialize)]
struct EstudioDaApi {
    id: String,
    name: String,
    #[serde(default)]
    city: String,
    /// A rota já devolve só os ativos; o filtro aqui é defesa, e o padrão é
    /// `true` para uma resposta sem o campo não esvaziar a lista.
    #[serde(default = "verdadeiro")]
    is_active: bool,
    /// As fotos de cenário do cadastro. A primeira é a capa — a mesma que o
    /// site mostra no agendamento.
    #[serde(default)]
    fotos_urls: Vec<String>,
}

fn verdadeiro() -> bool {
    true
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

/// A resposta do bilhete de revelação. O `caminho` e a validade vêm junto e não
/// são lidos: quem monta a URL do envio é este cliente, e um caminho vindo do
/// servidor seria uma segunda verdade sobre a mesma rota.
#[derive(Deserialize)]
struct BilheteDaApi {
    bilhete: String,
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
    // 🔚 `422` é "falta o contato do cliente para concluir" — a resposta do link
    // e do aviso a uma sessão sem e-mail (2026-09-13). Nas rotas
    // que este cliente chama, é o único `422` de regra: o outro (JSON que não
    // cabe no tipo) seria defeito daqui, e o corpo é montado por nós.
    if status == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
        return DomainError::FaltaEmail(mensagem);
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
    async fn pedir_json(
        &self,
        sessao: &Sessao,
        metodo: &str,
        caminho: &str,
        corpo: Option<serde_json::Value>,
    ) -> DomainResult<serde_json::Value> {
        let corpo = corpo.map(|valor| CorpoCru {
            tipo: "application/json".into(),
            bytes: valor.to_string().into_bytes(),
        });
        let resposta = self.chamar(Some(sessao), metodo, caminho, corpo).await?;
        let texto = String::from_utf8_lossy(&resposta.bytes);
        if !(200..300).contains(&resposta.status) {
            if resposta.status == 401 {
                return Err(DomainError::AcessoRecusado);
            }
            let mensagem = serde_json::from_str::<EnvelopeDeErro>(&texto)
                .ok()
                .and_then(|e| e.error)
                .and_then(|e| e.message)
                .unwrap_or_else(|| texto.chars().take(200).collect());
            return Err(DomainError::InfrastructureError(format!(
                "o site respondeu {}: {mensagem}",
                resposta.status
            )));
        }
        if resposta.bytes.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_slice(&resposta.bytes)
            .map_err(|e| DomainError::InfrastructureError(format!("resposta ilegível: {e}")))
    }

    async fn autorizar_pelo_navegador(&self) -> DomainResult<Sessao> {
        // O servidor local sobe **antes** do navegador: se ele abrisse depois, a
        // volta poderia chegar numa porta que ainda não escuta, e o operador
        // veria "não foi possível conectar" numa autorização que deu certo.
        let pedido = PedidoDeAutorizacao::novo().await?;
        (self.abridor)(&pedido.url(&self.site));

        let code = pedido.esperar_codigo(&self.site).await?;

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
            // 🚨 Os dois endereços vão para o log, e é por causa de um caso real
            // (6/set/2026): o operador autorizou em **produção**, viu
            // "Computador autorizado" no navegador, e o app recusou — porque
            // apresentou o código à API **local**, que não assinou nada daquilo.
            // Da tela, os dois desfechos são a mesma frase; aqui eles se separam.
            eprintln!(
                "⚠️  Autorização recusada. Site que autorizou: {} · API que recusou: {}",
                self.site, self.base
            );
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
                // O balcão começa pelo preço cheio; `price` é o valor da vitrine
                // com desconto para a compra antecipada no site.
                preco: match p.normal_price.unwrap_or(p.price) {
                    serde_json::Value::String(s) => s,
                    outro => outro.to_string(),
                },
                inativo: p.inactive,
            })
            .collect())
    }

    async fn estudios(&self, sessao: &Sessao) -> DomainResult<Vec<Estudio>> {
        let resposta = self
            .client
            .get(self.url("/bookings/studios"))
            .bearer_auth(self.token(sessao).await?)
            .send()
            .await
            .map_err(rede)?;

        let lista: Vec<EstudioDaApi> = ler(resposta).await?;
        Ok(lista
            .into_iter()
            .filter(|e| e.is_active)
            .map(|e| Estudio {
                id: e.id,
                nome: e.name,
                cidade: e.city,
                foto: e.fotos_urls.into_iter().next(),
            })
            .collect())
    }

    /// A capa do estúdio — um arquivo **público** do R2.
    ///
    /// 🔑 **Sem token e sem a base da API**: a URL vem inteira do cadastro, e
    /// `studios/` é servido sem autenticação (o mesmo arquivo que o site mostra
    /// no agendamento).
    async fn arquivo_publico(&self, url: &str) -> DomainResult<Vec<u8>> {
        let resposta = self.client.get(url).send().await.map_err(rede)?;
        if !resposta.status().is_success() {
            return Err(DomainError::InfrastructureError(format!(
                "o arquivo público respondeu {}",
                resposta.status()
            )));
        }
        Ok(resposta.bytes().await.map_err(rede)?.to_vec())
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
                "estudio_id": nova.estudio_id,
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
        let mut form = reqwest::multipart::Form::new()
            .text("estado", foto.estado.como_texto())
            .text("ordem", foto.ordem.to_string());
        // 🚨 **Sem a nota o site recusa com `400`** — *"a foto sobe
        // classificada: informe a nota de 1 a 5"*. Ela faltava aqui, e o passo 3
        // do app não subia foto nenhuma.
        if let Some(nota) = foto.nota {
            form = form.text("nota", nota.to_string());
        }
        // A faixa da leva, quando o operador escolheu uma na barra de envio.
        // Vazio é ausência, e o site trata assim: a foto segue a galeria.
        if let Some(produto) = foto.produto_id.as_ref().filter(|p| !p.trim().is_empty()) {
            form = form.text("produto_id", produto.clone());
        }
        // 🔑 A chave de idempotência: reenviar a mesma foto devolve a que já
        // está lá, em vez de uma segunda cópia na galeria do cliente.
        if let Some(chave) = foto.chave_do_cliente.clone() {
            form = form.text("chave_do_cliente", chave);
        }
        let mut form = form.part("file", arquivo);
        // 🚨 **O bruto sobe junto quando a foto vai revelada.** Sem ele o site
        // recebe só o JPEG tratado e passa a tratá-lo como o original: "Zerar
        // tudo" lá não tem o que restaurar. Ver `FotoParaEnviar::bruto`.
        if let Some(bytes) = foto.bruto {
            let parte = reqwest::multipart::Part::bytes(bytes)
                .file_name(foto.nome.clone())
                .mime_str("image/jpeg")
                .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;
            form = form.part("file_bruto", parte);
            // 🔑 **E a receita, em JSON** — é com ela que o editor do site abre a
            // foto revelada, e não no neutro. Ver `receita.rs`.
            if let Some(receita) = &foto.ajustes {
                form = form.text("ajustes", receita.to_string());
            }
        }

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
        if let Some(produto) = &mudanca.produto_id {
            corpo.insert("produto_id".into(), json!(produto));
        }
        if let Some(preco) = &mudanca.preco_de_venda {
            corpo.insert("preco_de_venda".into(), json!(preco));
        }
        // 🔄 A rejeição — a tecla `X` (contrato C21). Booleano simples, e não
        // `Option<Option<_>>`: não há "apagar a rejeição", há desfazê-la, que é
        // `false`. O site aplica este campo por último quando ele vem junto com
        // a nota, porque a rejeição é a palavra final sobre a foto aparecer.
        if let Some(rejeitada) = mudanca.rejeitada {
            corpo.insert("rejeitada".into(), json!(rejeitada));
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

    async fn atualizar_galeria(
        &self,
        sessao: &Sessao,
        galeria_id: &str,
        mudanca: &MudancaDaGaleria,
    ) -> DomainResult<()> {
        // O mesmo cuidado de `mudar_foto`: nada a mudar não vai à rede.
        if mudanca.vazia() {
            return Ok(());
        }

        // 🔑 Ausente do mapa é "não mexer", `null` no mapa é "apagar" — é o que
        // deixa apagar o último contato (armadilha nº 26 do site).
        let mut corpo = serde_json::Map::new();
        if let Some(titulo) = &mudanca.titulo {
            corpo.insert("titulo".into(), json!(titulo));
        }
        if let Some(email) = &mudanca.email {
            corpo.insert("email".into(), json!(email));
        }
        if let Some(whatsapp) = &mudanca.whatsapp {
            corpo.insert("whatsapp".into(), json!(whatsapp));
        }
        // 🔑 Os três estados de cada campo, como no `PATCH` da foto: ausente do
        // mapa é "não mexer", `null` é "desassociar".
        for (campo, valor) in [
            ("estudio_id", &mudanca.estudio_id),
            ("ensaio_id", &mudanca.ensaio_id),
            ("voucher_id", &mudanca.voucher_id),
            ("pedido_id", &mudanca.pedido_id),
            ("como_conheceu", &mudanca.como_conheceu),
            ("como_conheceu_detalhe", &mudanca.como_conheceu_detalhe),
            ("parceiro_id", &mudanca.parceiro_id),
            ("preset_padrao_id", &mudanca.preset_padrao_id),
            ("proporcao_padrao", &mudanca.proporcao_padrao),
        ] {
            if let Some(valor) = valor {
                corpo.insert(campo.into(), json!(valor));
            }
        }

        let resposta = self
            .client
            .patch(self.url(&format!("/pos-venda/galerias/{galeria_id}")))
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
                    rejeitada: f.rejeitada_em.is_some(),
                    nota: f.nota,
                    produto_efetivo: f.produto_efetivo,
                    produto_id: f.produto_id,
                    tamanho_bytes: f.tamanho_bytes,
                    preco_de_venda: f.preco_de_venda,
                    pedido_id: f.pedido_id,
                    downloads: f.downloads,
                    // `ajustes` é `null` enquanto ninguém revelou a foto.
                    revelada: f.ajustes.is_some(),
                    ajustes: f.ajustes,
                })
                .collect(),
            vence_venda: aberta.vence_venda.map(|q| q.timestamp()),
            vence_download: aberta.vence_download.map(|q| q.timestamp()),
            resumos: ResumosDoAtendimento {
                agendamento: aberta.agendamento.map(|a| ResumoSimples {
                    titulo: a.nome.unwrap_or_else(|| "Agendamento".into()),
                    detalhe: [
                        a.inicio.map(data_e_hora_br),
                        a.estudio_nome,
                        (!a.status.trim().is_empty()).then_some(a.status),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · "),
                }),
                voucher: aberta.voucher.map(|v| ResumoSimples {
                    titulo: format!("Voucher {}", v.numero),
                    detalhe: [
                        v.nome,
                        (!v.parceiro.trim().is_empty()).then_some(v.parceiro),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · "),
                }),
                pedido: aberta.pedido.map(|p| ResumoSimples {
                    titulo: match p.total {
                        Some(total) => format!("Compra de R$ {total}"),
                        None => "Compra antecipada".into(),
                    },
                    detalhe: [
                        p.comprador_nome,
                        p.pago_em.map(|q| format!("paga em {}", data_e_hora_br(q))),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · "),
                }),
                parceiro: aberta.parceiro.map(|p| ResumoSimples {
                    titulo: p.nome,
                    detalhe: p.tipo.unwrap_or_default(),
                }),
            },
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

    async fn original(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<Vec<u8>> {
        self.bytes_da_imagem(sessao, &format!("/pos-venda/fotos/{foto_id}/original"))
            .await
    }

    async fn bilhete_de_revelacao(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<String> {
        let resposta = self
            .client
            .post(self.url(&format!("/pos-venda/fotos/{foto_id}/bilhete-de-revelacao")))
            .bearer_auth(self.token(sessao).await?)
            .json(&json!({}))
            .send()
            .await
            .map_err(rede)?;

        let bilhete: BilheteDaApi = ler(resposta).await?;
        Ok(bilhete.bilhete)
    }

    async fn salvar_revelacao(
        &self,
        bilhete: &str,
        jpeg: Vec<u8>,
        ajustes: serde_json::Value,
    ) -> DomainResult<()> {
        let arquivo = reqwest::multipart::Part::bytes(jpeg)
            .file_name("revelada.jpg")
            .mime_str("image/jpeg")
            .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;
        let form = reqwest::multipart::Form::new()
            .text("ajustes", ajustes.to_string())
            .part("file", arquivo);

        // 🔑 **Sem `bearer_auth`**: a rota é pública e o bilhete é a credencial,
        // como no navegador. Mandar o token junto não daria erro — daria um
        // segundo caminho de autorização para a mesma rota.
        let resposta = self
            .client
            .post(self.url(&format!("/public/pos-venda/revelacao/{bilhete}")))
            .multipart(form)
            .send()
            .await
            .map_err(rede)?;

        if !resposta.status().is_success() {
            return Err(recusa(resposta).await);
        }
        Ok(())
    }

    async fn restaurar_original(&self, sessao: &Sessao, foto_id: &str) -> DomainResult<()> {
        // Com token, e não com bilhete: nada sobe aqui. O bilhete existe para o
        // upload que o servidor do site não aguenta — este pedido é vazio.
        let resposta = self
            .client
            .post(self.url(&format!("/pos-venda/fotos/{foto_id}/restaurar-original")))
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

    /// Um pedido qualquer à API, com o token de agora, e a resposta crua.
    ///
    /// É a porta crua da API: quem chama monta o pedido, e este cliente põe o
    /// token, renova quando vence e devolve o status e os bytes, sem
    /// interpretar nada.
    /// Uma resposta `4xx` ou `5xx` volta como resposta, e não como erro: quem lê
    /// o envelope de erro é o cliente do site.
    ///
    /// 🔒 `caminho` é sempre relativo a `/api/v2` desta API. Um endereço
    /// completo, ou que tente subir de pasta, é recusado. Sem `sessao`, o
    /// pedido vai sem token (rota pública).
    pub async fn chamar(
        &self,
        sessao: Option<&Sessao>,
        metodo: &str,
        caminho: &str,
        corpo: Option<CorpoCru>,
    ) -> DomainResult<RespostaCrua> {
        if !caminho.starts_with('/') || caminho.contains("://") || caminho.contains("..") {
            return Err(DomainError::InvalidOperation(format!(
                "caminho de API recusado: {caminho}"
            )));
        }
        let metodo = reqwest::Method::from_bytes(metodo.as_bytes())
            .map_err(|_| DomainError::InvalidOperation(format!("método recusado: {metodo}")))?;
        let mut pedido = self.client.request(metodo, self.url(caminho));
        // Sem sessão é rota pública (o envio por bilhete): o bilhete é a
        // autorização, e mandar o token junto não acrescentaria nada.
        if let Some(sessao) = sessao {
            pedido = pedido.bearer_auth(self.token(sessao).await?);
        }
        if let Some(corpo) = corpo {
            pedido = pedido
                .header(reqwest::header::CONTENT_TYPE, corpo.tipo)
                .body(corpo.bytes);
        }
        let resposta = pedido.send().await.map_err(rede)?;
        let status = resposta.status().as_u16();
        let tipo = resposta
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let bytes = resposta
            .bytes()
            .await
            .map_err(|e| DomainError::InfrastructureError(format!("resposta incompleta: {e}")))?
            .to_vec();
        Ok(RespostaCrua {
            status,
            tipo,
            bytes,
        })
    }

    /// Um fluxo de eventos (SSE) da API, entregue pedaço a pedaço a
    /// `ao_chegar` conforme chega — o tempo real do painel do chatbot
    /// (`/whatsapp/eventos` e os irmãos).
    ///
    /// Volta `Ok` quando o servidor fecha o fluxo, e erro quando ele recusa,
    /// quando a rede cai ou quando o fluxo fica calado mais que
    /// `silencio_maximo`. Nos três casos quem chama reconecta: esta função não
    /// tenta de novo, porque quem sabe a pausa entre tentativas é a tela.
    ///
    /// # Por que o silêncio, e não o teto de 180 s
    ///
    /// O [`TEMPO_LIMITE`] do cliente vale para o pedido **inteiro**, corpo
    /// incluído: um fluxo que dura horas seria cortado aos três minutos. Aqui o
    /// teto do pedido é trocado por um de meia hora, e o que acusa a conexão
    /// morta é o silêncio. O servidor manda `keep-alive` a cada 15 s
    /// (`tempo_real.rs` do backend); três sem chegar é Wi-Fi que caiu sem
    /// avisar, e sem esta conta o painel ficaria "conectado" a um cano mudo.
    ///
    /// 🔒 Mesma regra de caminho de [`PosVendaApiHttp::chamar`]: relativo a
    /// `/api/v2`, nunca endereço completo.
    pub async fn escutar(
        &self,
        sessao: &Sessao,
        caminho: &str,
        silencio_maximo: Duration,
        mut ao_chegar: impl FnMut(&[u8]) + Send,
    ) -> DomainResult<()> {
        use futures::StreamExt;

        if !caminho.starts_with('/') || caminho.contains("://") || caminho.contains("..") {
            return Err(DomainError::InvalidOperation(format!(
                "caminho de API recusado: {caminho}"
            )));
        }
        let resposta = self
            .client
            .get(self.url(caminho))
            .bearer_auth(self.token(sessao).await?)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .timeout(TETO_DO_FLUXO)
            .send()
            .await
            .map_err(rede)?;
        if resposta.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(DomainError::AcessoRecusado);
        }
        if !resposta.status().is_success() {
            return Err(DomainError::InfrastructureError(format!(
                "o site respondeu {} ao fluxo {caminho}",
                resposta.status().as_u16()
            )));
        }
        let mut fluxo = resposta.bytes_stream();
        loop {
            match tokio::time::timeout(silencio_maximo, fluxo.next()).await {
                Err(_) => {
                    return Err(DomainError::InfrastructureError(format!(
                        "o fluxo {caminho} ficou calado por {}s",
                        silencio_maximo.as_secs()
                    )))
                }
                Ok(None) => return Ok(()),
                Ok(Some(Err(erro))) => return Err(rede(erro)),
                Ok(Some(Ok(pedaco))) => ao_chegar(&pedaco),
            }
        }
    }

    /// `PUT` numa URL **assinada**, lendo o arquivo **do disco** e relatando
    /// quantos bytes já saíram.
    ///
    /// # Por que o caminho, e não os bytes
    ///
    /// 🚨 **A primeira versão recebia `Vec<u8>` e o app comeu 15,77 GB** (foto
    /// do dono, 2026-09-19, subindo uma pasta de ~1.300 RAW de 12 MB). Eram
    /// dois erros somados: a tela lia a pasta inteira para a memória antes de
    /// subir, e **aqui** os mesmos bytes viravam uma segunda cópia, fatiada em
    /// pedaços, para poder relatar progresso.
    ///
    /// Agora o arquivo é lido em pedaços conforme sai, e o que fica na memória
    /// é um pedaço — não o arquivo, muito menos a pasta.
    ///
    /// 🚨 **Sem `bearer_auth`, nunca.** A assinatura do R2 vai na query; um
    /// `Authorization` nosso junto faz o próprio R2 recusar com
    /// `InvalidArgument`, e a mensagem não menciona o cabeçalho.
    ///
    /// ⚠️ **O aviso é do que foi entregue ao sistema operacional**, e não do que
    /// o R2 confirmou: o último pedaço chega a 100% um instante antes de a
    /// resposta voltar. É a mesma verdade que o `upload.onprogress` do
    /// navegador conta, e a tela só dá a peça por pronta quando a resposta vem.
    pub async fn enviar_arquivo_assinado(
        &self,
        url: &str,
        tipo: &str,
        origem: &std::path::Path,
        avisou: Arc<dyn Fn(u64) + Send + Sync>,
    ) -> DomainResult<()> {
        use futures::StreamExt;

        /// O tamanho de cada pedaço lido do disco e entregue ao `reqwest`.
        ///
        /// 256 KiB: pequeno o bastante para a barra andar em arquivo de poucos
        /// MB, grande o bastante para um RAW de 80 MB não virar 20 mil avisos —
        /// cada um deles um `cx.notify()` do outro lado.
        const PEDACO: usize = 256 * 1024;

        let total = tokio::fs::metadata(origem)
            .await
            .map_err(|e| DomainError::InfrastructureError(format!("{}: {e}", origem.display())))?
            .len();
        let arquivo = tokio::fs::File::open(origem)
            .await
            .map_err(|e| DomainError::InfrastructureError(format!("{}: {e}", origem.display())))?;

        // 🔑 **`ReaderStream` do `tokio-util`, e não um `unfold` à mão.** É a
        // peça que existe para exatamente isto — transformar um `AsyncRead` em
        // fluxo de pedaços —, e escrevê-la de novo só acrescentaria um lugar
        // onde o caso de erro pode ficar errado (a primeira versão precisava de
        // uma bandeira para não repetir a falha para sempre).
        let mut enviados = 0u64;
        let fluxo =
            tokio_util::io::ReaderStream::with_capacity(arquivo, PEDACO).map(move |pedaco| {
                if let Ok(dados) = &pedaco {
                    enviados += dados.len() as u64;
                    avisou(enviados);
                }
                pedaco
            });

        let resposta = self
            .client
            .put(url)
            .header(reqwest::header::CONTENT_TYPE, tipo)
            // O R2 precisa do tamanho: sem ele o `reqwest` manda
            // `Transfer-Encoding: chunked`, que a API do S3 recusa.
            .header(reqwest::header::CONTENT_LENGTH, total)
            .body(reqwest::Body::wrap_stream(fluxo))
            .send()
            .await
            .map_err(rede)?;

        if resposta.status().is_success() {
            return Ok(());
        }
        let status = resposta.status();
        let corpo = resposta.text().await.unwrap_or_default();
        Err(DomainError::InfrastructureError(format!(
            "o armazenamento respondeu {status}: {corpo}"
        )))
    }

    /// `GET` numa URL **assinada** — a foto da prévia e o arquivo do zip.
    ///
    /// O par de [`Self::enviar_para_url_assinada`], e existe pelo mesmo motivo:
    /// o destino não é esta API, e o `chamar` recusa endereço absoluto de
    /// propósito.
    ///
    /// 🚨 **Sem `bearer_auth`**: a assinatura do R2 vai na query, e um
    /// `Authorization` nosso junto faz o próprio R2 recusar.
    pub async fn buscar_url_assinada(&self, url: &str) -> DomainResult<Vec<u8>> {
        let resposta = self.client.get(url).send().await.map_err(rede)?;
        if !resposta.status().is_success() {
            let status = resposta.status();
            let corpo = resposta.text().await.unwrap_or_default();
            return Err(DomainError::InfrastructureError(format!(
                "o armazenamento respondeu {status}: {corpo}"
            )));
        }
        Ok(resposta
            .bytes()
            .await
            .map_err(|e| DomainError::InfrastructureError(format!("leitura incompleta: {e}")))?
            .to_vec())
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

/// O corpo de um pedido de [`PosVendaApiHttp::chamar`], com o tipo dele.
#[derive(Debug)]
pub struct CorpoCru {
    pub tipo: String,
    pub bytes: Vec<u8>,
}

/// O que [`PosVendaApiHttp::chamar`] devolve.
#[derive(Debug)]
pub struct RespostaCrua {
    pub status: u16,
    pub tipo: Option<String>,
    pub bytes: Vec<u8>,
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
    criada_por: Option<String>,
    #[serde(default)]
    expira_em: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    fotos: ContagemDaApi,
    /// ⚠️ **`Option`, e não zero por padrão**: a galeria de uma API anterior ao
    /// campo precisa chegar como "não sei", para o rodapé poder dizer que o
    /// total está menor que o real. Zero calado é a única resposta errada aqui.
    #[serde(default)]
    totais: Option<TotaisDaApi>,
    /// 💵 Pelo mesmo motivo do vizinho: `null` e campo ausente são "não sei",
    /// e o objeto com `vendas: 0` é "não passou pelo caixa".
    #[serde(default)]
    caixa: Option<PagoNoCaixaDaApi>,
    #[serde(default)]
    preset_padrao_id: Option<String>,
    #[serde(default)]
    proporcao_padrao: Option<String>,
    #[serde(default)]
    estudio_id: Option<String>,
    #[serde(default)]
    ensaio_id: Option<String>,
    #[serde(default)]
    voucher_id: Option<String>,
    #[serde(default)]
    pedido_id: Option<String>,
    #[serde(default)]
    como_conheceu: Option<String>,
    #[serde(default)]
    como_conheceu_detalhe: Option<String>,
    #[serde(default)]
    parceiro_id: Option<String>,
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

/// 💵 `PagoNoCaixa` do site. Tudo com `default`: a API anterior ao campo não o
/// manda, e um `null` explícito quer dizer "não deu para perguntar ao caixa".
#[derive(Deserialize, Default)]
struct PagoNoCaixaDaApi {
    #[serde(default)]
    vendas: u32,
    #[serde(default)]
    bruto_centavos: i64,
    #[serde(default)]
    estornado_centavos: i64,
    #[serde(default)]
    liquido_centavos: i64,
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
            criada_por: g.criada_por,
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
            caixa: g.caixa.map(|c| PagoNoCaixa {
                vendas: c.vendas,
                bruto_centavos: c.bruto_centavos,
                estornado_centavos: c.estornado_centavos,
                liquido_centavos: c.liquido_centavos,
            }),
            preset_padrao_id: g.preset_padrao_id,
            proporcao_padrao: g.proporcao_padrao,
            estudio_id: g.estudio_id,
            ensaio_id: g.ensaio_id,
            voucher_id: g.voucher_id,
            pedido_id: g.pedido_id,
            como_conheceu: g.como_conheceu,
            como_conheceu_detalhe: g.como_conheceu_detalhe,
            parceiro_id: g.parceiro_id,
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
    #[serde(default)]
    agendamento: Option<AgendamentoDaApi>,
    #[serde(default)]
    voucher: Option<VoucherDaApi>,
    #[serde(default)]
    pedido: Option<PedidoDaApi>,
    #[serde(default)]
    parceiro: Option<ParceiroDaApi>,
}

/// Os quatro resumos do atendimento, como a API os devolve. Cada um vira um
/// [`ResumoSimples`] — texto pronto para a gaveta.
#[derive(Deserialize)]
struct AgendamentoDaApi {
    #[serde(default)]
    nome: Option<String>,
    #[serde(default)]
    inicio: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    estudio_nome: Option<String>,
    #[serde(default)]
    status: String,
}

#[derive(Deserialize)]
struct VoucherDaApi {
    numero: String,
    #[serde(default)]
    nome: Option<String>,
    #[serde(default)]
    parceiro: String,
}

#[derive(Deserialize)]
struct PedidoDaApi {
    #[serde(default)]
    total: Option<String>,
    #[serde(default)]
    comprador_nome: Option<String>,
    #[serde(default)]
    pago_em: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
struct ParceiroDaApi {
    nome: String,
    #[serde(default)]
    tipo: Option<String>,
}

/// `2026-09-13T14:00:00Z` → `13/09/2026, 11:00`, no fuso do estúdio.
fn data_e_hora_br(quando: chrono::DateTime<chrono::Utc>) -> String {
    let brasilia = chrono::FixedOffset::west_opt(3 * 3600).expect("fuso do estúdio");
    quando
        .with_timezone(&brasilia)
        .format("%d/%m/%Y, %H:%M")
        .to_string()
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
    /// Quando a foto foi **rejeitada** — a tecla `X` (contrato C21). `null` =
    /// não rejeitada.
    ///
    /// 🔑 **Chega como data e vira `bool`**: o site guarda o instante porque a
    /// linha do tempo dele precisa dele; aqui a grade só pergunta "está
    /// rejeitada?". `#[serde(default)]` porque um backend anterior a
    /// 2026-09-20 não manda o campo — e sem ele a galeria inteira falharia ao
    /// desserializar.
    #[serde(default)]
    rejeitada_em: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    nota: Option<u8>,
    #[serde(default)]
    produto_efetivo: String,
    /// A faixa fixada **nesta** foto — `null` quando ela segue a galeria.
    #[serde(default)]
    produto_id: Option<String>,
    #[serde(default)]
    tamanho_bytes: Option<u64>,
    #[serde(default)]
    preco_de_venda: Option<String>,
    #[serde(default)]
    pedido_id: Option<String>,
    #[serde(default)]
    downloads: u32,
    /// Os 53 ajustes da revelação (por nome) e o enquadramento (`corte_*`).
    ///
    /// ⚠️ Passam **inteiros** para a tela: é com eles que uma foto já revelada
    /// reabre com os sliders no lugar, aqui como no editor do site.
    #[serde(default)]
    ajustes: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct LinkDaApi {
    link: String,
    /// 🚨 **`null` desde 2026-09-20 — o link da galeria não expira.**
    ///
    /// `Option` **e** `#[serde(default)]`: sem o primeiro, a resposta de hoje
    /// (`null`) derruba a desserialização e o operador vê "não foi possível
    /// gerar o link" com o link já assinado do outro lado; sem o segundo, um
    /// backend que um dia pare de mandar o campo faria o mesmo. Ver
    /// `LinkDeAcesso`.
    #[serde(default)]
    validade_em_segundos: Option<i64>,
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
                { "product": { "id": "p1", "name": "Foto avulsa", "price": "19.91", "normal_price": "29.90", "inactive": true }, "category": {} },
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
    async fn enviar_foto_manda_o_multipart_com_estado_nota_e_chave() {
        let servidor = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/pos-venda/galerias/g1/fotos"))
            .and(header("authorization", "Bearer tok"))
            .and(body_string_contains("name=\"estado\""))
            .and(body_string_contains("levada_no_balcao"))
            .and(body_string_contains("name=\"ordem\""))
            // 🚨 **Sem a `nota` o site responde `400`** — *"a foto sobe
            // classificada"*. O campo faltava no multipart, e com ele faltando
            // o passo 3 do app não subia foto nenhuma.
            .and(body_string_contains("name=\"nota\""))
            // 🔑 A chave de idempotência: reenviar devolve a foto que já está lá.
            .and(body_string_contains("name=\"chave_do_cliente\""))
            .and(body_string_contains("3f1c9a6e-0000-4000-8000-000000000001"))
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
                    bruto: None,
                    ajustes: None,
                    estado: EstadoNoBalcao::LevadaNoBalcao,
                    produto_id: Some("p2".into()),
                    ordem: 3,
                    nota: Some(4),
                    chave_do_cliente: Some("3f1c9a6e-0000-4000-8000-000000000001".into()),
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
            estudio_id: None,
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
                    "caixa": {
                        "vendas": 2,
                        "bruto_centavos": 15000,
                        "estornado_centavos": 2500,
                        "liquido_centavos": 12500
                    },
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

        // 💵 O caixa vem em centavos inteiros, ao contrário dos totais ao lado
        // — e o líquido é o que a coluna mostra.
        let pago = galerias[0].caixa.expect("g1 passou pelo caixa");
        assert_eq!(pago.vendas, 2);
        assert_eq!(pago.bruto_centavos, 15_000);
        assert_eq!(pago.liquido_centavos, 12_500);
        // 🚨 E a galeria sem o campo não vira "cobrada, e deu zero": ela chega
        // como "não sei", que é o `?` da coluna.
        assert_eq!(galerias[1].caixa, None);
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
                "validade_em_segundos": null
            })))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let link = api.link_da_galeria(&sessao_valida(), "g1").await.unwrap();

        assert_eq!(link.url, "https://recordarfotos.com.br/entrar?t=abc123");
        assert_eq!(
            link.validade_em_segundos, None,
            "o link da galeria não expira desde 2026-09-20"
        );
    }

    /// 🚨 **O link de 7 dias continua sendo lido** — e o campo ausente também.
    ///
    /// O site de hoje manda `null`, mas o app conversa com o que estiver no ar:
    /// uma máquina antiga no meio de um deploy ainda responde `604800`. As duas
    /// formas precisam abrir a mesma tela; a que **não** pode acontecer é a
    /// resposta inteira ser recusada por causa deste campo.
    #[tokio::test]
    async fn o_link_aceita_o_prazo_antigo_e_a_ausencia_do_campo() {
        for (corpo, esperado) in [
            (
                json!({ "link": "https://site/x", "validade_em_segundos": 604800 }),
                Some(604_800),
            ),
            (json!({ "link": "https://site/x" }), None),
        ] {
            let servidor = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/api/v2/pos-venda/galerias/g1/link"))
                .respond_with(ResponseTemplate::new(200).set_body_json(corpo))
                .mount(&servidor)
                .await;

            let api = PosVendaApiHttp::nova(servidor.uri());
            let link = api.link_da_galeria(&sessao_valida(), "g1").await.unwrap();
            assert_eq!(link.validade_em_segundos, esperado);
        }
    }

    /// 🔚 Sessão sem contato: o `422` do link e do aviso chega como
    /// `FaltaEmail`, com a frase do site — é o que a tela lê para pedir o
    /// contato em vez de mostrar erro.
    #[tokio::test]
    async fn sem_email_o_link_e_o_aviso_voltam_como_falta_de_email() {
        let servidor = MockServer::start().await;
        let corpo = json!({
            "error": {
                "code": "UNPROCESSABLE_ENTITY",
                "message": "informe o e-mail do cliente antes de gerar o link"
            }
        });
        Mock::given(method("POST"))
            .and(path("/api/v2/pos-venda/galerias/g1/link"))
            .respond_with(ResponseTemplate::new(422).set_body_json(corpo.clone()))
            .mount(&servidor)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v2/pos-venda/galerias/g1/avisar"))
            .respond_with(ResponseTemplate::new(422).set_body_json(corpo))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let sessao = sessao_valida();

        let erro = api.link_da_galeria(&sessao, "g1").await.unwrap_err();
        assert!(
            matches!(&erro, DomainError::FaltaEmail(frase) if frase.contains("informe o e-mail")),
            "{erro}"
        );
        let erro = api.avisar_fotos_prontas(&sessao, "g1").await.unwrap_err();
        assert!(matches!(erro, DomainError::FaltaEmail(_)), "{erro}");
    }

    /// *"Deve ser obrigatório informar o Preço por Foto e o Estúdio."* — dono,
    /// 2026-09-13. A lista traz só os ativos, e a criação leva o estúdio.
    #[tokio::test]
    async fn os_estudios_ativos_chegam_e_a_criacao_leva_o_estudio() {
        let servidor = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/bookings/studios"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {
                    "id": "s1", "name": "Gramado", "city": "Gramado", "is_active": true,
                    // A capa do cadastro: a **primeira** da lista.
                    "fotos_urls": ["https://r2/studios/gramado-1.jpg", "https://r2/studios/gramado-2.jpg"]
                },
                { "id": "s2", "name": "Fechado", "city": "Canela", "is_active": false }
            ])))
            .mount(&servidor)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v2/pos-venda/galerias"))
            .and(body_string_contains("\"estudio_id\":\"s1\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_json(json!({ "id": "g1", "titulo": "Ensaio" })),
            )
            .expect(1)
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let sessao = sessao_valida();

        assert_eq!(
            api.estudios(&sessao).await.unwrap(),
            vec![Estudio {
                id: "s1".into(),
                nome: "Gramado".into(),
                cidade: "Gramado".into(),
                foto: Some("https://r2/studios/gramado-1.jpg".into()),
            }]
        );
        api.criar_galeria(
            &sessao,
            &NovaGaleria {
                titulo: "Ensaio".into(),
                email: None,
                whatsapp: None,
                produto_id: "p1".into(),
                estudio_id: Some("s1".into()),
            },
        )
        .await
        .unwrap();
    }

    /// O `PATCH` da sessão leva só o que mudou, e `null` apaga — inclusive o
    /// último contato. Mudança vazia não vai à rede (o `expect(1)` reprova).
    #[tokio::test]
    async fn o_patch_da_galeria_leva_so_o_que_mudou_e_null_apaga() {
        let servidor = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v2/pos-venda/galerias/g1"))
            .and(header("authorization", "Bearer tok"))
            .and(wiremock::matchers::body_json(
                json!({ "titulo": "Ensaio da Ana", "email": null }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "g1" })))
            .expect(1)
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let sessao = sessao_valida();

        api.atualizar_galeria(
            &sessao,
            "g1",
            &MudancaDaGaleria {
                titulo: Some("Ensaio da Ana".into()),
                email: Some(None),
                whatsapp: None,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        api.atualizar_galeria(&sessao, "g1", &MudancaDaGaleria::default())
            .await
            .unwrap();
    }

    /// ❌ **`rejeitada_em` do site vira `rejeitada` na grade** — contrato C21.
    ///
    /// 🚨 **E o campo ausente não derruba a galeria inteira.** Um backend
    /// anterior a 2026-09-20 não manda a coluna; sem o `serde(default)` a
    /// resposta inteira ficaria ilegível e o operador veria "não foi possível
    /// abrir a sessão" numa sessão perfeitamente normal — o mesmo desfecho que
    /// o `null` da validade do link causou em 2026-09-20.
    #[tokio::test]
    async fn a_rejeitada_do_site_chega_a_grade_e_a_ausencia_dela_nao_quebra() {
        let servidor = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/pos-venda/galerias/g1"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "galeria": {
                    "id": "g1",
                    "titulo": "Ensaio",
                    "email": null,
                    "whatsapp": null,
                    "produto_id": "p1",
                    "criada_em": "2026-09-20T10:00:00Z"
                },
                "fotos": [
                    {
                        "id": "f1",
                        "arquivo": "f1.jpg",
                        "estado": "disponivel",
                        "rejeitada_em": "2026-09-20T12:00:00Z"
                    },
                    { "id": "f2", "arquivo": "f2.jpg", "estado": "disponivel" }
                ]
            })))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let aberta = api.abrir_galeria(&sessao_valida(), "g1").await.unwrap();

        assert!(aberta.fotos[0].rejeitada, "a marcada volta rejeitada");
        assert!(
            !aberta.fotos[1].rejeitada,
            "e a sem o campo é só uma foto sem curadoria — não uma rejeitada"
        );
    }

    /// ❌ **O `X` vai no `PATCH` como booleano, e sozinho basta.**
    ///
    /// 🚨 **`MudancaDaFoto::vazia` precisava conhecer o campo novo**: sem isso
    /// um `X` sozinho seria "mudança vazia", e o gesto morreria no cliente HTTP
    /// sem nunca chegar à rede — calado, que é o pior desfecho.
    #[tokio::test]
    async fn a_rejeicao_vai_sozinha_no_patch_da_foto() {
        let servidor = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v2/pos-venda/fotos/f1"))
            .and(header("authorization", "Bearer tok"))
            .and(wiremock::matchers::body_json(json!({ "rejeitada": true })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "f1" })))
            .expect(1)
            .mount(&servidor)
            .await;

        let mudanca = MudancaDaFoto {
            rejeitada: Some(true),
            ..Default::default()
        };
        assert!(!mudanca.vazia(), "um X sozinho é mudança");

        PosVendaApiHttp::nova(servidor.uri())
            .mudar_foto(&sessao_valida(), "f1", &mudanca)
            .await
            .unwrap();
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

    /// O fluxo chega inteiro a quem escuta, com o token, e o fim do fluxo é
    /// `Ok` — é o que manda a tela reconectar em vez de dar a conexão por
    /// perdida.
    #[tokio::test]
    async fn o_fluxo_de_eventos_chega_a_quem_escuta_e_o_fim_e_ok() {
        let servidor = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/whatsapp/eventos"))
            .and(header("authorization", "Bearer tok"))
            .and(header("accept", "text/event-stream"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(
                        "event: pronto\ndata: 1\n\ndata: {\"tipo\":\"mensagem_recebida\"}\n\n",
                    ),
            )
            .mount(&servidor)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/instagram/eventos"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&servidor)
            .await;

        let api = PosVendaApiHttp::nova(servidor.uri());
        let mut recebido = Vec::new();
        api.escutar(
            &sessao_valida(),
            "/whatsapp/eventos",
            Duration::from_secs(5),
            |pedaco| recebido.extend_from_slice(pedaco),
        )
        .await
        .unwrap();
        let texto = String::from_utf8(recebido).unwrap();
        assert!(texto.starts_with("event: pronto"));
        assert!(texto.contains("mensagem_recebida"));

        // Sessão recusada tem desfecho próprio: reconectar em laço não cura.
        let erro = api
            .escutar(
                &sessao_valida(),
                "/instagram/eventos",
                Duration::from_secs(5),
                |_| {},
            )
            .await
            .unwrap_err();
        assert!(matches!(erro, DomainError::AcessoRecusado), "{erro}");

        let erro = api
            .escutar(
                &sessao_valida(),
                "https://outro/x",
                Duration::from_secs(5),
                |_| {},
            )
            .await
            .unwrap_err();
        assert!(matches!(erro, DomainError::InvalidOperation(_)), "{erro}");
    }

    /// 🔑 **O cano mudo.** Um servidor que responde o cabeçalho e depois se
    /// cala (Wi-Fi que caiu sem fechar a conexão) tem de virar erro no prazo do
    /// silêncio — sem isto o painel mostraria "Tempo real" para sempre, sem
    /// receber nada.
    #[tokio::test]
    async fn o_fluxo_calado_alem_do_prazo_vira_erro() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let ouvinte = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endereco = ouvinte.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut conexao, _) = ouvinte.accept().await.unwrap();
            let mut pedido = [0u8; 2048];
            let _ = conexao.read(&mut pedido).await;
            let _ = conexao
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                      transfer-encoding: chunked\r\n\r\n\
                      f\r\nevent: pronto\n\n\r\n",
                )
                .await;
            // E nunca mais nada — nem fecha.
            tokio::time::sleep(Duration::from_secs(30)).await;
            drop(conexao);
        });

        let api = PosVendaApiHttp::nova(format!("http://{endereco}"));
        let mut recebido = Vec::new();
        let inicio = std::time::Instant::now();
        let erro = api
            .escutar(
                &sessao_valida(),
                "/whatsapp/eventos",
                Duration::from_millis(300),
                |pedaco| recebido.extend_from_slice(pedaco),
            )
            .await
            .unwrap_err();
        assert!(erro.to_string().contains("calado"), "{erro}");
        assert!(inicio.elapsed() < Duration::from_secs(5));
        assert_eq!(
            recebido, b"event: pronto\n\n",
            "o que chegou antes do silêncio foi entregue"
        );
    }
}
