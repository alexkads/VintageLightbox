//! O JSON do caixa, lido e escrito — o `lib/schemas/caixa.ts` do site.
//!
//! Escrito **a partir do que o backend serializa** (`handlers/pos_venda_caixa.rs`
//! e `handlers/pos_venda_painel.rs`), como os schemas do site. O que a tela
//! desenha vira as structs de [`biblioteca_core::caixa`], para a conta não ter
//! duas versões; o que só a tela usa (o estúdio, o funcionário, o caixa aberto)
//! mora aqui.
//!
//! ⚠️ **Tolerante onde o site é tolerante.** Campo novo que ainda não chegou
//! (`auxiliar`, `primeira_contagem`, `estudio_id` da galeria) entra com o padrão,
//! em vez de derrubar a tela inteira — a armadilha nº 33 do site.

use std::collections::HashSet;

use biblioteca_core::acervo::Estado;
use biblioteca_core::caixa::{
    Estorno, Faixa, FormaDePagamento, FotoDoCupom, ItemVendido, NovaVenda, NovoEstorno,
    PagamentoGravado, PorForma, SessaoACobrar, TipoDeMovimento, Venda,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// A leitura falhou: a frase que a tela mostra, quando mostra.
fn ilegivel(o_que: &str, erro: serde_json::Error) -> String {
    format!("{o_que} veio num formato inesperado: {erro}")
}

// ── Dinheiro e hora ─────────────────────────────────────────────────────────

/// O `parseMoney` do site: o backend manda `rust_decimal` como texto
/// (`"99.99"`) e às vezes como número; o que não é dinheiro vira zero.
///
/// Arredonda no terceiro decimal em vez de cortar: `299.8999…` é R$ 299,90.
pub fn centavos_da_api(texto: &str) -> i64 {
    let texto = texto.trim();
    let (negativo, texto) = match texto.strip_prefix('-') {
        Some(resto) => (true, resto),
        None => (false, texto),
    };
    let (inteiros, decimais) = texto.split_once('.').unwrap_or((texto, ""));
    let digitos = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if !digitos(inteiros) || !digitos(decimais) {
        return 0;
    }
    let reais: i64 = if inteiros.is_empty() {
        0
    } else {
        match inteiros.parse() {
            Ok(r) => r,
            Err(_) => return 0,
        }
    };
    let tres: i64 = format!("{:0<3}", &decimais[..decimais.len().min(3)])
        .parse()
        .unwrap_or(0);
    // `Math.round(tres / 10)`: meio para cima.
    let centavos = reais * 100 + (tres + 5) / 10;
    if negativo {
        -centavos
    } else {
        centavos
    }
}

fn dinheiro_da_api<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::String(s)) => Some(centavos_da_api(&s)),
        Some(Value::Number(n)) => Some(centavos_da_api(&n.to_string())),
        _ => None,
    })
}

/// O fuso do estúdio (`America/Sao_Paulo`, sem horário de verão desde 2019).
fn no_estudio(iso: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let brasilia = chrono::FixedOffset::west_opt(3 * 3600)?;
    chrono::DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|d| d.with_timezone(&brasilia))
}

/// `"14:09"`.
pub fn hora_br(iso: &str) -> String {
    no_estudio(iso).map_or_else(String::new, |d| d.format("%H:%M").to_string())
}

/// `"16/09/2026"`.
pub fn data_br(iso: &str) -> String {
    no_estudio(iso).map_or_else(String::new, |d| d.format("%d/%m/%Y").to_string())
}

/// `"16/09, 14:09"` — o `dataEHora` do diálogo de vendas.
pub fn dia_e_hora_br(iso: &str) -> String {
    no_estudio(iso).map_or_else(String::new, |d| d.format("%d/%m, %H:%M").to_string())
}

/// O `mensagemDe` das ações do site: a frase do backend nos casos que
/// importam, a conta sem direito, e o padrão no resto.
pub fn mensagem_do_erro(erro: &str, padrao: &str) -> String {
    mensagem_do_erro_com(erro, padrao, "Sua conta não pode operar o caixa.")
}

/// O mesmo, com a frase da conta sem direito de cada ação (a da galeria é
/// "administrar galerias").
pub fn mensagem_do_erro_com(erro: &str, padrao: &str, proibido: &str) -> String {
    let Some((_, depois)) = erro.split_once("o site respondeu ") else {
        return padrao.to_string();
    };
    let (status, mensagem) = depois.split_once(": ").unwrap_or((depois, ""));
    match status.trim().parse::<u16>() {
        Ok(403) => proibido.into(),
        Ok(400 | 404 | 409 | 422) if !mensagem.trim().is_empty() => mensagem.trim().into(),
        _ => padrao.to_string(),
    }
}

/// O status HTTP de um erro da porta, quando ele diz.
pub fn status_do_erro(erro: &str) -> Option<u16> {
    let (_, depois) = erro.split_once("o site respondeu ")?;
    depois.split(':').next()?.trim().parse().ok()
}

// ── Estúdios, funcionários e a lista de sessões ─────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstudioDoCaixa {
    pub id: String,
    pub nome: String,
    pub cidade: String,
}

#[derive(Deserialize)]
struct EstudioDaApi {
    id: String,
    name: String,
    #[serde(default)]
    city: String,
    #[serde(default)]
    is_active: bool,
}

/// `GET /bookings/studios/admin`, só os ativos.
pub fn ler_estudios(valor: Value) -> Result<Vec<EstudioDoCaixa>, String> {
    let lista: Vec<EstudioDaApi> =
        serde_json::from_value(valor).map_err(|e| ilegivel("A lista de estúdios", e))?;
    Ok(lista
        .into_iter()
        .filter(|e| e.is_active)
        .map(|e| EstudioDoCaixa {
            id: e.id,
            nome: e.name,
            cidade: e.city,
        })
        .collect())
}

/// `domain::pos_venda::Funcionario`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Funcionario {
    pub id: String,
    pub nome: String,
    #[serde(default)]
    pub whatsapp: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub ativo: bool,
}

/// `GET /pos-venda/funcionarios`.
pub fn ler_funcionarios(valor: Value) -> Result<Vec<Funcionario>, String> {
    serde_json::from_value(valor).map_err(|e| ilegivel("A lista de funcionários", e))
}

/// `POST /pos-venda/funcionarios`.
pub fn ler_funcionario(valor: Value) -> Result<Funcionario, String> {
    serde_json::from_value(valor).map_err(|e| ilegivel("O funcionário", e))
}

/// O corpo do cadastro: contato vazio vai nulo.
pub fn novo_funcionario_json(nome: &str, whatsapp: &str, email: &str) -> Value {
    let texto = |s: &str| {
        let s = s.trim();
        (!s.is_empty()).then(|| s.to_string())
    };
    json!({ "nome": nome, "whatsapp": texto(whatsapp), "email": texto(email) })
}

/// Uma linha da lista, com o estúdio para filtrar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessaoDaLista {
    pub sessao: SessaoACobrar,
    pub estudio_id: Option<String>,
}

#[derive(Deserialize)]
struct ContagemDaLista {
    #[serde(default)]
    levadas_no_balcao: i64,
}

#[derive(Deserialize)]
struct GaleriaDaLista {
    id: String,
    titulo: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    whatsapp: Option<String>,
    #[serde(default)]
    estudio_id: Option<String>,
    #[serde(default)]
    criada_em: String,
    fotos: ContagemDaLista,
}

/// `GET /pos-venda/galerias`.
pub fn ler_galerias(valor: Value) -> Result<Vec<SessaoDaLista>, String> {
    let lista: Vec<GaleriaDaLista> =
        serde_json::from_value(valor).map_err(|e| ilegivel("A lista de sessões", e))?;
    Ok(lista
        .into_iter()
        .map(|g| SessaoDaLista {
            sessao: SessaoACobrar {
                id: g.id,
                titulo: g.titulo,
                // `g.email ?? g.whatsapp`
                contato: g.email.or(g.whatsapp),
                criada_em: g.criada_em,
                sinalizadas: g.fotos.levadas_no_balcao,
            },
            estudio_id: g.estudio_id,
        })
        .collect())
}

// ── O catálogo e a galeria aberta ───────────────────────────────────────────

#[derive(Deserialize)]
struct ProdutoDoCatalogo {
    id: String,
    name: String,
    #[serde(default, deserialize_with = "dinheiro_da_api")]
    price: Option<i64>,
    #[serde(default, deserialize_with = "dinheiro_da_api")]
    normal_price: Option<i64>,
}

#[derive(Deserialize)]
struct LinhaDoCatalogo {
    product: ProdutoDoCatalogo,
}

/// Uma faixa com o id do produto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaixaDoCatalogo {
    pub id: String,
    pub faixa: Faixa,
}

/// `GET /products/admin?limit=&offset=`: o preço de balcão é o cheio
/// (`normal_price ?? price`, o `precoDeBalcao` do site).
pub fn ler_catalogo(valor: Value) -> Result<Vec<FaixaDoCatalogo>, String> {
    let lista: Vec<LinhaDoCatalogo> =
        serde_json::from_value(valor).map_err(|e| ilegivel("O catálogo", e))?;
    Ok(lista
        .into_iter()
        .map(|l| FaixaDoCatalogo {
            id: l.product.id,
            faixa: Faixa {
                nome: l.product.name,
                preco: l
                    .product
                    .normal_price
                    .or(l.product.price)
                    .unwrap_or_default(),
            },
        })
        .collect())
}

#[derive(Deserialize)]
struct ProdutoDaGaleria {
    id: String,
    nome: String,
    #[serde(default, deserialize_with = "dinheiro_da_api")]
    preco: Option<i64>,
    #[serde(default, deserialize_with = "dinheiro_da_api")]
    preco_cheio: Option<i64>,
}

impl ProdutoDaGaleria {
    /// A preço de balcão: o cheio, ou o preço quando o backend não o manda.
    fn de_balcao(self) -> FaixaDoCatalogo {
        FaixaDoCatalogo {
            id: self.id,
            faixa: Faixa {
                nome: self.nome,
                preco: self.preco_cheio.or(self.preco).unwrap_or_default(),
            },
        }
    }
}

#[derive(Deserialize)]
struct FotoDaGaleria {
    id: String,
    arquivo: String,
    estado: String,
    #[serde(default)]
    ordem: i64,
    #[serde(default)]
    nota: Option<u8>,
    #[serde(default)]
    apagada_em: Option<String>,
    /// ❌ Quando a foto foi rejeitada (C21). `#[serde(default)]` porque um
    /// backend anterior a 2026-09-20 não manda o campo — e sem ele a galeria
    /// inteira ficaria ilegível para o caixa.
    #[serde(default)]
    rejeitada_em: Option<String>,
    produto_efetivo: String,
    #[serde(default)]
    produto_id: Option<String>,
    #[serde(default, deserialize_with = "dinheiro_da_api")]
    preco_negociado: Option<i64>,
    #[serde(default)]
    observacao_da_negociacao: Option<String>,
}

#[derive(Deserialize)]
struct CabecaDaGaleria {
    id: String,
    titulo: String,
    #[serde(default)]
    estudio_id: Option<String>,
}

#[derive(Deserialize)]
struct GaleriaAbertaDaApi {
    galeria: CabecaDaGaleria,
    fotos: Vec<FotoDaGaleria>,
    produto: ProdutoDaGaleria,
    #[serde(default)]
    produtos: Vec<ProdutoDaGaleria>,
}

/// O que o cupom precisa da galeria aberta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GaleriaDoCaixa {
    pub id: String,
    pub titulo: String,
    pub estudio_id: Option<String>,
    pub fotos: Vec<FotoDoCupom>,
    /// O produto padrão, a preço de balcão.
    pub padrao: Faixa,
    /// As faixas em uso na galeria, a preço de balcão.
    pub produtos: Vec<FaixaDoCatalogo>,
}

fn estado(texto: &str) -> Estado {
    match texto {
        "levada_no_balcao" => Estado::LevadaNoBalcao,
        "comprada" => Estado::Comprada,
        _ => Estado::Disponivel,
    }
}

/// `GET /pos-venda/galerias/{id}`.
pub fn ler_galeria(valor: Value) -> Result<GaleriaDoCaixa, String> {
    let g: GaleriaAbertaDaApi =
        serde_json::from_value(valor).map_err(|e| ilegivel("A sessão", e))?;
    Ok(GaleriaDoCaixa {
        id: g.galeria.id,
        titulo: g.galeria.titulo,
        estudio_id: g.galeria.estudio_id,
        fotos: g
            .fotos
            .into_iter()
            .map(|f| FotoDoCupom {
                id: f.id,
                ordem: f.ordem,
                arquivo: f.arquivo,
                estado: estado(&f.estado),
                apagada: f.apagada_em.is_some(),
                nota: f.nota,
                // ❌ A rejeitada não entra no cupom (C21) — e é ela, e não a
                // falta de nota, que o balcão recusa desde 2026-09-20.
                rejeitada: f.rejeitada_em.is_some(),
                sem_marcacao: false,
                produto_efetivo: f.produto_efetivo,
                produto_id: f.produto_id,
                preco_negociado: f.preco_negociado,
                observacao: f.observacao_da_negociacao,
            })
            .collect(),
        padrao: g.produto.de_balcao().faixa,
        produtos: g
            .produtos
            .into_iter()
            .map(ProdutoDaGaleria::de_balcao)
            .collect(),
    })
}

/// As faixas onde o cupom acha nome e preço: o catálogo inteiro, mais as da
/// galeria que ele não tem (um produto apagado do catálogo ainda precisa ter
/// nome na foto que o usa).
pub fn juntar_faixas(
    catalogo: &[FaixaDoCatalogo],
    da_galeria: &[FaixaDoCatalogo],
) -> Vec<FaixaDoCatalogo> {
    let mut faixas = catalogo.to_vec();
    for p in da_galeria {
        if !faixas.iter().any(|f| f.id == p.id) {
            faixas.push(p.clone());
        }
    }
    faixas
}

// ── O caixa, as vendas e a conferência ──────────────────────────────────────

#[derive(Deserialize)]
struct PagamentoDaApi {
    forma: String,
    valor_centavos: i64,
    #[serde(default)]
    detalhe: Option<String>,
}

fn pagamentos(lista: Vec<PagamentoDaApi>) -> Vec<PagamentoGravado> {
    lista
        .into_iter()
        .filter_map(|p| {
            Some(PagamentoGravado {
                forma: FormaDePagamento::da_chave(&p.forma)?,
                valor: p.valor_centavos,
                detalhe: p.detalhe,
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct EstornoDaApi {
    id: String,
    numero: i64,
    valor_centavos: i64,
    #[serde(default)]
    motivo: String,
    #[serde(default)]
    fotos: Vec<String>,
    #[serde(default)]
    pagamentos: Vec<PagamentoDaApi>,
    #[serde(default)]
    criado_em: String,
}

impl From<EstornoDaApi> for Estorno {
    fn from(e: EstornoDaApi) -> Self {
        Estorno {
            id: e.id,
            numero: e.numero,
            valor: e.valor_centavos,
            motivo: e.motivo,
            fotos: e.fotos,
            pagamentos: pagamentos(e.pagamentos),
            criado_em: e.criado_em,
        }
    }
}

#[derive(Deserialize)]
struct ItemVendidoDaApi {
    foto_id: String,
    #[serde(default)]
    arquivo: String,
    #[serde(default)]
    estornada: bool,
    #[serde(default)]
    valor_sugerido_de_estorno_centavos: i64,
}

#[derive(Deserialize)]
struct VendaDaApi {
    id: String,
    numero: i64,
    #[serde(default)]
    galeria_id: String,
    #[serde(default)]
    itens: Vec<ItemVendidoDaApi>,
    #[serde(default)]
    pagamentos: Vec<PagamentoDaApi>,
    total_centavos: i64,
    #[serde(default)]
    troco_centavos: i64,
    #[serde(default)]
    fotografo: String,
    #[serde(default)]
    atendente: String,
    #[serde(default)]
    auxiliar: Option<String>,
    #[serde(default)]
    criada_em: String,
    #[serde(default)]
    estornado_centavos: i64,
    #[serde(default)]
    estornavel_centavos: i64,
    #[serde(default)]
    fotos_vendidas: Vec<String>,
    #[serde(default)]
    estornos: Vec<EstornoDaApi>,
}

impl From<VendaDaApi> for Venda {
    fn from(v: VendaDaApi) -> Self {
        Venda {
            id: v.id,
            numero: v.numero,
            galeria_id: v.galeria_id,
            itens: v
                .itens
                .into_iter()
                .map(|i| ItemVendido {
                    foto_id: i.foto_id,
                    arquivo: i.arquivo,
                    estornada: i.estornada,
                    valor_sugerido_de_estorno: i.valor_sugerido_de_estorno_centavos,
                })
                .collect(),
            pagamentos: pagamentos(v.pagamentos),
            total: v.total_centavos,
            troco: v.troco_centavos,
            fotografo: v.fotografo,
            atendente: v.atendente,
            auxiliar: v.auxiliar,
            criada_em: v.criada_em,
            estornado: v.estornado_centavos,
            estornavel: v.estornavel_centavos,
            fotos_vendidas: v.fotos_vendidas,
            estornos: v.estornos.into_iter().map(Into::into).collect(),
        }
    }
}

/// Uma venda (`POST /caixa/vendas`, `…/estornos`).
pub fn ler_venda(valor: Value) -> Result<Venda, String> {
    serde_json::from_value::<VendaDaApi>(valor)
        .map(Into::into)
        .map_err(|e| ilegivel("A venda", e))
}

/// `GET /caixa/galerias/{id}/vendas`.
pub fn ler_vendas(valor: Value) -> Result<Vec<Venda>, String> {
    serde_json::from_value::<Vec<VendaDaApi>>(valor)
        .map(|l| l.into_iter().map(Into::into).collect())
        .map_err(|e| ilegivel("As vendas da sessão", e))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Movimento {
    pub id: String,
    pub tipo: TipoDeMovimento,
    pub valor: i64,
    pub motivo: String,
    pub criado_em: String,
}

#[derive(Deserialize)]
struct MovimentoDaApi {
    id: String,
    tipo: String,
    valor_centavos: i64,
    #[serde(default)]
    motivo: String,
    #[serde(default)]
    criado_em: String,
}

fn por_forma(mapa: Option<serde_json::Map<String, Value>>) -> Option<PorForma> {
    mapa.map(|m| {
        m.into_iter()
            .filter_map(|(k, v)| Some((FormaDePagamento::da_chave(&k)?, v.as_i64()?)))
            .collect()
    })
}

/// O caixa do estúdio. `contado`, `esperado` e `diferenca` só vêm **depois**
/// de fechado — o fechamento é cego.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaixaDoBalcao {
    pub id: String,
    pub estudio_id: String,
    pub operador_email: String,
    pub fundo_de_troco: i64,
    pub aberto_em: String,
    pub contado: Option<PorForma>,
    pub esperado: Option<PorForma>,
    pub diferenca: Option<PorForma>,
    pub primeira_contagem: Option<PorForma>,
    pub movimentos: Vec<Movimento>,
    pub vendas: Vec<Venda>,
    pub estornos: Vec<Estorno>,
}

#[derive(Deserialize)]
struct CaixaDaApi {
    id: String,
    #[serde(default)]
    estudio_id: String,
    #[serde(default)]
    operador_email: String,
    #[serde(default)]
    fundo_de_troco_centavos: i64,
    #[serde(default)]
    aberto_em: String,
    #[serde(default)]
    contado: Option<serde_json::Map<String, Value>>,
    #[serde(default)]
    esperado: Option<serde_json::Map<String, Value>>,
    #[serde(default)]
    diferenca: Option<serde_json::Map<String, Value>>,
    #[serde(default)]
    primeira_contagem: Option<serde_json::Map<String, Value>>,
    #[serde(default)]
    movimentos: Vec<MovimentoDaApi>,
    #[serde(default)]
    vendas: Vec<VendaDaApi>,
    #[serde(default)]
    estornos: Vec<EstornoDaApi>,
}

fn caixa_da_api(c: CaixaDaApi) -> CaixaDoBalcao {
    CaixaDoBalcao {
        id: c.id,
        estudio_id: c.estudio_id,
        operador_email: c.operador_email,
        fundo_de_troco: c.fundo_de_troco_centavos,
        aberto_em: c.aberto_em,
        contado: por_forma(c.contado),
        esperado: por_forma(c.esperado),
        diferenca: por_forma(c.diferenca),
        primeira_contagem: por_forma(c.primeira_contagem),
        movimentos: c
            .movimentos
            .into_iter()
            .map(|m| Movimento {
                id: m.id,
                tipo: TipoDeMovimento::da_chave(&m.tipo).unwrap_or_default(),
                valor: m.valor_centavos,
                motivo: m.motivo,
                criado_em: m.criado_em,
            })
            .collect(),
        vendas: c.vendas.into_iter().map(Into::into).collect(),
        estornos: c.estornos.into_iter().map(Into::into).collect(),
    }
}

/// `GET /caixa?estudio_id=` — `null` é caixa fechado.
pub fn ler_caixa_aberto(valor: Value) -> Result<Option<CaixaDoBalcao>, String> {
    serde_json::from_value::<Option<CaixaDaApi>>(valor)
        .map(|c| c.map(caixa_da_api))
        .map_err(|e| ilegivel("O caixa", e))
}

/// `POST /caixa` e `POST /caixa/fechar`.
pub fn ler_caixa(valor: Value) -> Result<CaixaDoBalcao, String> {
    serde_json::from_value::<CaixaDaApi>(valor)
        .map(caixa_da_api)
        .map_err(|e| ilegivel("O caixa", e))
}

/// `POST /caixa/conferir`: a contagem ao lado do esperado, com o caixa aberto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conferencia {
    pub contado: PorForma,
    pub esperado: PorForma,
    /// `contado − esperado`: positivo sobra, negativo falta.
    pub diferenca: PorForma,
}

#[derive(Deserialize)]
struct ConferenciaDaApi {
    #[serde(default)]
    contado: serde_json::Map<String, Value>,
    #[serde(default)]
    esperado: serde_json::Map<String, Value>,
    #[serde(default)]
    diferenca: serde_json::Map<String, Value>,
}

pub fn ler_conferencia(valor: Value) -> Result<Conferencia, String> {
    let c: ConferenciaDaApi =
        serde_json::from_value(valor).map_err(|e| ilegivel("A conferência", e))?;
    Ok(Conferencia {
        contado: por_forma(Some(c.contado)).unwrap_or_default(),
        esperado: por_forma(Some(c.esperado)).unwrap_or_default(),
        diferenca: por_forma(Some(c.diferenca)).unwrap_or_default(),
    })
}

// ── Os corpos das gravações ─────────────────────────────────────────────────

fn pagamentos_json(lista: &[PagamentoGravado]) -> Value {
    Value::Array(
        lista
            .iter()
            .map(|p| {
                json!({
                    "forma": p.forma.chave(),
                    "valor_centavos": p.valor,
                    "detalhe": p.detalhe,
                })
            })
            .collect(),
    )
}

/// O `VendaRequisicao` do backend.
pub fn nova_venda_json(v: &NovaVenda) -> Value {
    json!({
        "galeria_id": v.galeria_id,
        "itens": v.itens.iter().map(|i| json!({
            "foto_id": i.foto_id,
            "ordem": i.ordem,
            "arquivo": i.arquivo,
            "faixa": i.faixa,
            "cheio_centavos": i.cheio,
            "cobrado_centavos": i.cobrado,
            "tipo_de_negociacao": i.tipo_de_negociacao,
            "etiqueta": i.etiqueta,
        })).collect::<Vec<_>>(),
        "desconto_no_total_centavos": v.desconto_no_total,
        "motivo_do_desconto": v.motivo_do_desconto,
        "pagamentos": pagamentos_json(&v.pagamentos),
        "fotografo_id": v.fotografo_id,
        "atendente_id": v.atendente_id,
        "auxiliar_id": v.auxiliar_id,
    })
}

/// O `EstornoRequisicao` do backend.
pub fn novo_estorno_json(e: &NovoEstorno) -> Value {
    json!({
        "fotos": e.fotos,
        "valor_centavos": e.valor,
        "motivo": e.motivo,
        "pagamentos": pagamentos_json(&e.pagamentos),
    })
}

/// O contado por forma, com as chaves do backend.
pub fn contado_json(contado: &PorForma) -> Value {
    Value::Object(
        contado
            .iter()
            .map(|(f, v)| (f.chave().to_string(), json!(v)))
            .collect(),
    )
}

/// Os ids de um conjunto, na ordem em que aparecem numa lista — para o corpo
/// sair igual a cada vez.
pub fn na_ordem(ordem: &[String], conjunto: &HashSet<String>) -> Vec<String> {
    ordem
        .iter()
        .filter(|id| conjunto.contains(*id))
        .cloned()
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_dinheiro_da_api_arredonda_no_terceiro_decimal() {
        assert_eq!(centavos_da_api("99.99"), 9999);
        assert_eq!(centavos_da_api("299.8999999"), 29990);
        assert_eq!(centavos_da_api("25"), 2500);
        assert_eq!(centavos_da_api("1.5"), 150);
        assert_eq!(centavos_da_api("-2.50"), -250);
        assert_eq!(centavos_da_api("abc"), 0);
        assert_eq!(centavos_da_api(""), 0);
    }

    #[test]
    fn a_hora_e_a_do_estudio() {
        assert_eq!(hora_br("2026-09-16T17:09:00Z"), "14:09");
        assert_eq!(data_br("2026-09-16T01:00:00Z"), "15/09/2026");
        assert_eq!(dia_e_hora_br("2026-09-16T17:09:00+00:00"), "16/09, 14:09");
        assert_eq!(hora_br("lixo"), "");
    }

    #[test]
    fn o_erro_do_backend_so_aparece_nos_casos_que_importam() {
        let padrao = "Não foi possível abrir o caixa.";
        assert_eq!(
            mensagem_do_erro(
                "Erro de infraestrutura: o site respondeu 409: O caixa já está aberto.",
                padrao
            ),
            "O caixa já está aberto."
        );
        assert_eq!(
            mensagem_do_erro("Erro de infraestrutura: o site respondeu 403: nope", padrao),
            "Sua conta não pode operar o caixa."
        );
        assert_eq!(
            mensagem_do_erro("Erro de infraestrutura: o site respondeu 500: boom", padrao),
            padrao
        );
        assert_eq!(mensagem_do_erro("rede caída", padrao), padrao);
        assert_eq!(
            status_do_erro("x: o site respondeu 410: excluída"),
            Some(410)
        );
        assert_eq!(status_do_erro("rede caída"), None);
    }

    #[test]
    fn so_os_estudios_ativos_entram() {
        let lidos = ler_estudios(json!([
            { "id": "a", "name": "Centro", "city": "Gramado", "is_active": true, "extra": 1 },
            { "id": "b", "name": "Fechado", "city": "Canela", "is_active": false },
        ]))
        .unwrap();
        assert_eq!(
            lidos,
            vec![EstudioDoCaixa {
                id: "a".into(),
                nome: "Centro".into(),
                cidade: "Gramado".into()
            }]
        );
    }

    #[test]
    fn a_lista_de_sessoes_usa_o_email_e_senao_o_whatsapp() {
        let lidas = ler_galerias(json!([
            { "id": "g1", "titulo": "A", "email": null, "whatsapp": "5551", "estudio_id": "e",
              "criada_em": "2026-09-16T10:00:00Z", "fotos": { "levadas_no_balcao": 3, "disponiveis": 1 } },
            { "id": "g2", "titulo": "B", "email": "b@x", "whatsapp": "5552",
              "criada_em": "2026-09-15T10:00:00Z", "fotos": {} },
        ]))
        .unwrap();
        assert_eq!(lidas[0].sessao.contato.as_deref(), Some("5551"));
        assert_eq!(lidas[0].sessao.sinalizadas, 3);
        assert_eq!(lidas[0].estudio_id.as_deref(), Some("e"));
        assert_eq!(lidas[1].sessao.contato.as_deref(), Some("b@x"));
        assert_eq!(lidas[1].estudio_id, None);
    }

    #[test]
    fn a_galeria_aberta_vira_as_fotos_do_cupom_a_preco_de_balcao() {
        let g = ler_galeria(json!({
            "galeria": { "id": "g", "titulo": "Ensaio", "estudio_id": "e", "produto_id": "p" },
            "fotos": [
                { "id": "f1", "arquivo": "a.jpg", "estado": "levada_no_balcao", "ordem": 2, "nota": 4,
                  "apagada_em": null, "produto_efetivo": "p", "preco_negociado": "15.00",
                  "observacao_da_negociacao": "Desconto" },
                { "id": "f2", "arquivo": "b.jpg", "estado": "comprada", "ordem": 1,
                  "apagada_em": "2026-09-01T00:00:00Z", "produto_efetivo": "p" }
            ],
            "produto": { "id": "p", "nome": "Digital", "preco": "20.00", "preco_cheio": "25.00" },
            "produtos": [{ "id": "p", "nome": "Digital", "preco": 20 }],
            "avisos": []
        }))
        .unwrap();
        assert_eq!(g.estudio_id.as_deref(), Some("e"));
        assert_eq!(g.padrao.preco, 2500);
        assert_eq!(g.produtos[0].faixa.preco, 2000);
        assert_eq!(g.fotos[0].preco_negociado, Some(1500));
        assert_eq!(g.fotos[0].nota, Some(4));
        assert_eq!(g.fotos[0].estado, Estado::LevadaNoBalcao);
        assert!(g.fotos[1].apagada);
        assert_eq!(g.fotos[1].estado, Estado::Comprada);
        assert_eq!(g.fotos[1].preco_negociado, None);
    }

    #[test]
    fn o_catalogo_manda_e_a_galeria_completa() {
        let catalogo = ler_catalogo(json!([
            { "product": { "id": "p", "name": "Digital", "price": "20.00", "normal_price": "25.00" } },
            { "product": { "id": "q", "name": "Impressa", "price": "40.00", "normal_price": null } },
        ]))
        .unwrap();
        assert_eq!(catalogo[0].faixa.preco, 2500);
        assert_eq!(catalogo[1].faixa.preco, 4000);
        let da_galeria = vec![
            FaixaDoCatalogo {
                id: "p".into(),
                faixa: Faixa {
                    nome: "outro nome".into(),
                    preco: 1,
                },
            },
            FaixaDoCatalogo {
                id: "apagado".into(),
                faixa: Faixa {
                    nome: "Antigo".into(),
                    preco: 900,
                },
            },
        ];
        let faixas = juntar_faixas(&catalogo, &da_galeria);
        let ids: Vec<&str> = faixas.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["p", "q", "apagado"]);
        assert_eq!(faixas[0].faixa.nome, "Digital");
    }

    fn venda_json() -> Value {
        json!({
            "id": "v1", "numero": 5, "caixa_id": "c", "galeria_id": "g",
            "itens": [{ "foto_id": "f1", "ordem": 1, "arquivo": "a.jpg", "faixa": "Digital",
                        "cheio_centavos": 2500, "cobrado_centavos": 2500, "desconto_centavos": 0,
                        "tipo_de_negociacao": null, "etiqueta": null, "estornada": false,
                        "valor_sugerido_de_estorno_centavos": 2500 }],
            "pagamentos": [{ "forma": "credito", "valor_centavos": 2500, "detalhe": "Visa · NSU 1" },
                           { "forma": "cheque", "valor_centavos": 1, "detalhe": null }],
            "subtotal_centavos": 2500, "descontos_nos_itens_centavos": 0,
            "pago_em_parceiro_centavos": 0, "desconto_no_total_centavos": 0,
            "motivo_do_desconto": null, "total_centavos": 2500, "recebido_centavos": 2500,
            "troco_centavos": 0, "fotografo_id": "a", "fotografo": "Ana",
            "atendente_id": "b", "atendente": "Bia",
            "criada_por": "x", "criada_em": "2026-09-16T17:09:00Z",
            "estornado_centavos": 0, "estornavel_centavos": 2500,
            "fotos_vendidas": ["f1"], "estornos": []
        })
    }

    #[test]
    fn a_venda_le_pagamentos_conhecidos_e_o_auxiliar_ausente() {
        let v = ler_venda(venda_json()).unwrap();
        assert_eq!(v.numero, 5);
        assert_eq!(v.pagamentos.len(), 1, "a forma desconhecida fica de fora");
        assert_eq!(v.pagamentos[0].forma, FormaDePagamento::Credito);
        assert_eq!(v.auxiliar, None);
        assert_eq!(v.itens[0].valor_sugerido_de_estorno, 2500);
        assert_eq!(ler_vendas(json!([venda_json()])).unwrap().len(), 1);
    }

    #[test]
    fn o_caixa_nulo_e_fechado_e_o_aberto_traz_o_movimento() {
        assert_eq!(ler_caixa_aberto(Value::Null).unwrap(), None);
        let c = ler_caixa_aberto(json!({
            "id": "c", "estudio_id": "e", "estudio_nome": "Centro",
            "operador_email": "op@x", "fundo_de_troco_centavos": 5000,
            "aberto_em": "2026-09-16T18:58:00Z", "fechado_em": null, "fechado_por": null,
            "contado": null, "esperado": null, "diferenca": null,
            "observacao_do_fechamento": null,
            "movimentos": [{ "id": "m", "tipo": "suprimento", "valor_centavos": 100,
                              "motivo": "troco", "criado_por": "op", "criado_em": "2026-09-16T19:00:00Z" }],
            "vendas": [venda_json()],
            "estornos": []
        }))
        .unwrap()
        .unwrap();
        assert_eq!(c.fundo_de_troco, 5000);
        assert_eq!(c.movimentos[0].tipo, TipoDeMovimento::Suprimento);
        assert_eq!(c.vendas.len(), 1);
        assert_eq!(c.primeira_contagem, None);
    }

    #[test]
    fn a_conferencia_le_as_formas_conhecidas() {
        let c = ler_conferencia(json!({
            "contado": { "dinheiro": 100, "pix": 0 },
            "esperado": { "dinheiro": 50, "moeda_estranha": 1 },
            "diferenca": { "dinheiro": 50 }
        }))
        .unwrap();
        assert_eq!(c.contado.len(), 2);
        assert_eq!(c.esperado.len(), 1);
        assert_eq!(c.diferenca[&FormaDePagamento::Dinheiro], 50);
    }

    #[test]
    fn os_corpos_saem_com_as_chaves_do_backend() {
        let contado = PorForma::from([(FormaDePagamento::Dinheiro, 100)]);
        assert_eq!(contado_json(&contado), json!({ "dinheiro": 100 }));
        let estorno = NovoEstorno {
            fotos: vec!["f".into()],
            valor: 10,
            motivo: "m".into(),
            pagamentos: vec![PagamentoGravado {
                forma: FormaDePagamento::Pix,
                valor: 10,
                detalhe: None,
            }],
        };
        assert_eq!(
            novo_estorno_json(&estorno),
            json!({ "fotos": ["f"], "valor_centavos": 10, "motivo": "m",
                    "pagamentos": [{ "forma": "pix", "valor_centavos": 10, "detalhe": null }] })
        );
        assert_eq!(
            novo_funcionario_json("Ana", " ", "a@x"),
            json!({ "nome": "Ana", "whatsapp": null, "email": "a@x" })
        );
    }
}
