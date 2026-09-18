//! O caixa do balcão, em regras puras — o `caixa.ts` do site, portado, e as
//! decisões do `usar-pdv.tsx` e do `pdv-dialogos.tsx` que não são desenho.
//!
//! ⚠️ **Escopo novo do site (dono, 2026-09-15)**: o legado não tinha caixa. O
//! desktop o ganha pelo pedido de "mesmo gesto, mesmo resultado" entre o site e
//! o app — e o resultado só é o mesmo se a conta for a mesma. Quem grava e
//! confere de novo é o backend (`domain::pos_venda::caixa`); aqui mora o que a
//! tela precisa decidir **antes** de pedir: o que entra no cupom, quanto cada
//! item cobra, o desconto no total, o troco e o que cada diálogo recusa.
//!
//! # O que entra no cupom
//!
//! **As sinalizadas** — as levadas no balcão, e só as que a contagem da barra
//! conta como levadas: viva, com nota e com marcação. A comprada já foi paga no
//! MercadoPago e a à venda ainda não foi escolhida: nenhuma das duas se cobra
//! aqui. Uma negociação gravada numa delas **fica fora do total**, e a já
//! vendida numa venda anterior também — o cupom diz quantas são de cada, em vez
//! de somar calado só o que entra.
//!
//! # Quanto cada item cobra
//!
//! Parte do preço **de balcão** da faixa (o cheio) e aplica a negociação como
//! [`crate::negociacao::interpretar`] a lê:
//!
//! - sem negociação → o preço da faixa;
//! - cortesia → zero, e a faixa inteira é desconto;
//! - desconto, ou "outro" com valor → o valor cobrado;
//! - "outro" só com texto → o preço da faixa (o acerto não disse valor);
//! - já paga em outro site → zero **aqui**: o valor que ela guarda é o que o
//!   cliente pagou lá, e não entra no caixa do estúdio.
//!
//! 🔑 `subtotal − descontos − pago_em_parceiro = total`, sempre — o teste cobra,
//! e o `CHECK caixa_vendas_conta_fecha` do banco também.
//!
//! Todo dinheiro aqui é **centavo inteiro** (`i64`), como nas colunas.

use std::collections::{BTreeMap, HashSet};

use crate::acervo::Estado;
use crate::dinheiro;
use crate::negociacao::{self, Tipo};

// ── As formas de pagamento ──────────────────────────────────────────────────

/// `domain::pos_venda::caixa::FormaDePagamento`, na ordem das teclas 1 a 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FormaDePagamento {
    Dinheiro,
    Pix,
    Debito,
    Credito,
    Voucher,
    Parceiro,
    Transferencia,
    Outro,
}

impl FormaDePagamento {
    /// Na ordem das teclas: `TODAS[n - 1]` é a tecla `n`.
    pub const TODAS: [FormaDePagamento; 8] = [
        FormaDePagamento::Dinheiro,
        FormaDePagamento::Pix,
        FormaDePagamento::Debito,
        FormaDePagamento::Credito,
        FormaDePagamento::Voucher,
        FormaDePagamento::Parceiro,
        FormaDePagamento::Transferencia,
        FormaDePagamento::Outro,
    ];

    /// Como o serde do backend grava.
    pub fn chave(self) -> &'static str {
        match self {
            FormaDePagamento::Dinheiro => "dinheiro",
            FormaDePagamento::Pix => "pix",
            FormaDePagamento::Debito => "debito",
            FormaDePagamento::Credito => "credito",
            FormaDePagamento::Voucher => "voucher",
            FormaDePagamento::Parceiro => "parceiro",
            FormaDePagamento::Transferencia => "transferencia",
            FormaDePagamento::Outro => "outro",
        }
    }

    pub fn da_chave(chave: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|f| f.chave() == chave)
    }

    /// `ROTULO_DA_FORMA` do site.
    pub fn rotulo(self) -> &'static str {
        match self {
            FormaDePagamento::Dinheiro => "Dinheiro",
            FormaDePagamento::Pix => "PIX",
            FormaDePagamento::Debito => "Cartão de débito",
            FormaDePagamento::Credito => "Cartão de crédito",
            FormaDePagamento::Voucher => "Voucher",
            FormaDePagamento::Parceiro => "Site parceiro",
            FormaDePagamento::Transferencia => "Transferência",
            FormaDePagamento::Outro => "Outro",
        }
    }

    /// `DICA_DO_DETALHE`: o rótulo do campo de detalhe de cada forma.
    pub fn dica_do_detalhe(self) -> &'static str {
        match self {
            FormaDePagamento::Dinheiro => "",
            FormaDePagamento::Pix => "Identificador (opcional)",
            FormaDePagamento::Debito | FormaDePagamento::Credito => "NSU (opcional)",
            FormaDePagamento::Voucher => "Número do voucher",
            FormaDePagamento::Parceiro => "Site e cupom",
            FormaDePagamento::Transferencia => "Banco / comprovante (opcional)",
            FormaDePagamento::Outro => "O que foi (opcional)",
        }
    }

    /// Voucher e site parceiro não se lançam sem dizer qual.
    pub fn pede_detalhe(self) -> bool {
        matches!(self, FormaDePagamento::Voucher | FormaDePagamento::Parceiro)
    }

    /// Débito e crédito: escolhem a bandeira, e o NSU vai num campo à parte.
    pub fn e_cartao(self) -> bool {
        matches!(self, FormaDePagamento::Debito | FormaDePagamento::Credito)
    }

    /// A tecla do pagamento (`1`–`8`).
    pub fn da_tecla(n: usize) -> Option<Self> {
        n.checked_sub(1).and_then(|i| Self::TODAS.get(i).copied())
    }
}

/// Os rótulos das formas de uma lista, juntos por `" + "` — ou "sem pagamento".
pub fn formas_juntas(formas: impl IntoIterator<Item = FormaDePagamento>) -> String {
    let texto = formas
        .into_iter()
        .map(FormaDePagamento::rotulo)
        .collect::<Vec<_>>()
        .join(" + ");
    if texto.is_empty() {
        "sem pagamento".into()
    } else {
        texto
    }
}

/// As bandeiras do cartão (`BANDEIRAS_DE_CARTAO`): `(id, nome)`.
pub const BANDEIRAS: [(&str, &str); 6] = [
    ("visa", "Visa"),
    ("mastercard", "Mastercard"),
    ("elo", "Elo"),
    ("amex", "American Express"),
    ("hipercard", "Hipercard"),
    ("diners", "Diners"),
];

/// A bandeira pelo começo de um detalhe gravado (`"Visa · NSU 123"`).
pub fn bandeira_do_detalhe(detalhe: &str) -> Option<&'static str> {
    let texto = detalhe.trim();
    BANDEIRAS
        .iter()
        .find(|(_, nome)| texto == *nome || texto.starts_with(&format!("{nome} ·")))
        .map(|(id, _)| *id)
}

/// `"1 item"`, `"3 itens"`.
pub fn itens(n: usize) -> String {
    format!("{n} {}", if n == 1 { "item" } else { "itens" })
}

// ── O cupom ─────────────────────────────────────────────────────────────────

/// Uma foto da sessão, com o que o cupom lê dela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FotoDoCupom {
    pub id: String,
    /// O número da foto na sessão — o "código" do item.
    pub ordem: i64,
    pub arquivo: String,
    pub estado: Estado,
    pub apagada: bool,
    /// A nota de 1 a 5. `None` = sem nota.
    pub nota: Option<u8>,
    /// Só nas locais do site: a leva entrou sem marcação. O desktop não tem
    /// foto local na sessão; fica `false`.
    pub sem_marcacao: bool,
    /// A faixa que vale para ela, resolvida pelo backend.
    pub produto_efetivo: String,
    /// A faixa **gravada** na foto (`None` = o padrão da galeria) — o que a
    /// edição rápida do painel mostra escolhido.
    pub produto_id: Option<String>,
    pub preco_negociado: Option<i64>,
    pub observacao: Option<String>,
}

/// A faixa de um item: nome e preço de balcão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Faixa {
    pub nome: String,
    pub preco: i64,
}

/// O tipo de negociação como o backend o grava (`tipo_de_negociacao`).
pub fn chave_do_tipo(tipo: Tipo) -> &'static str {
    match tipo {
        Tipo::Cortesia => "cortesia",
        Tipo::Desconto => "desconto",
        Tipo::Parceiro => "parceiro",
        Tipo::Outro => "outro",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDoCupom {
    pub foto_id: String,
    pub ordem: i64,
    pub arquivo: String,
    pub faixa: String,
    /// O preço de balcão da faixa.
    pub cheio: i64,
    /// O que o balcão recebe por este item.
    pub cobrado: i64,
    /// `cheio − cobrado` quando é abatimento do estúdio; zero em parceiro.
    /// Negativo = cobrou acima da faixa.
    pub desconto: i64,
    /// O que o cliente pagou no site parceiro — informativo, fora do caixa.
    pub pago_fora: Option<i64>,
    pub tipo: Option<Tipo>,
    /// A etiqueta curta da negociação, para a linha do cupom.
    pub etiqueta: Option<String>,
    /// A faixa gravada na foto (`None` = o padrão da galeria).
    pub produto_id: Option<String>,
    /// Os dois campos gravados da negociação — o diálogo completo abre com eles.
    pub preco_negociado: Option<i64>,
    pub observacao: Option<String>,
}

impl ItemDoCupom {
    /// O "código" do item no painel: a ordem na sessão, `#0001`.
    pub fn codigo(&self) -> String {
        format!("#{:04}", self.ordem + 1)
    }

    /// Houve negociação gravada nesta foto.
    pub fn negociado(&self) -> bool {
        self.preco_negociado.is_some() || self.observacao.as_deref().is_some_and(|o| !o.is_empty())
    }
}

/// O cupom da sessão — o `Caixa` do `caixa.ts`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cupom {
    pub itens: Vec<ItemDoCupom>,
    pub subtotal: i64,
    pub descontos: i64,
    /// O preço de balcão das que já vieram pagas por site parceiro.
    pub pago_em_parceiro: i64,
    pub total: i64,
    /// Negociações gravadas em fotos que não estão no cupom.
    pub negociadas_fora: usize,
    /// Sinalizadas que já estão numa venda desta sessão.
    pub ja_vendidas: usize,
}

/// Entra no cupom? A mesma regra da contagem "Sinalizadas" da barra.
pub fn entra_no_caixa(f: &FotoDoCupom) -> bool {
    !f.apagada && f.nota.is_some() && !f.sem_marcacao && f.estado == Estado::LevadaNoBalcao
}

fn tem_negociacao(f: &FotoDoCupom) -> bool {
    f.preco_negociado.is_some() || f.observacao.as_deref().is_some_and(|o| !o.is_empty())
}

fn etiqueta_do_tipo(tipo: Tipo) -> &'static str {
    match tipo {
        Tipo::Cortesia => "Cortesia",
        Tipo::Desconto => "Desconto",
        Tipo::Parceiro => "Pago em site parceiro",
        Tipo::Outro => "Acerto",
    }
}

/// As fotos que continuam vendidas nesta sessão — o que o cupom já não cobra.
///
/// Quem decide é o backend (`fotos_vendidas`: os itens menos os estornados);
/// aqui só se juntam as vendas. A foto estornada volta ao cupom.
pub fn vendidas_na(vendas: &[Venda]) -> HashSet<String> {
    vendas
        .iter()
        .flat_map(|v| v.fotos_vendidas.iter().cloned())
        .collect()
}

/// Monta o cupom — o `fecharCaixa` do site.
pub fn fechar_caixa(
    fotos: &[FotoDoCupom],
    faixa_da: impl Fn(&FotoDoCupom) -> Faixa,
    vendidas: &HashSet<String>,
) -> Cupom {
    let mut cupom = Cupom::default();

    for f in fotos {
        if !entra_no_caixa(f) {
            if !f.apagada && tem_negociacao(f) {
                cupom.negociadas_fora += 1;
            }
            continue;
        }
        if vendidas.contains(&f.id) {
            cupom.ja_vendidas += 1;
            continue;
        }
        let faixa = faixa_da(f);
        let cheio = faixa.preco;
        let mut cobrado = cheio;
        let mut pago_fora = None;
        let mut tipo = None;
        let mut etiqueta = None;

        if tem_negociacao(f) {
            let n = negociacao::interpretar(f.preco_negociado, f.observacao.as_deref());
            tipo = Some(n.tipo);
            etiqueta = Some(if n.tipo == Tipo::Parceiro {
                let mut partes = Vec::new();
                if !n.parceiro.is_empty() {
                    partes.push(n.parceiro.clone());
                }
                if !n.cupom.is_empty() {
                    partes.push(format!("cupom {}", n.cupom));
                }
                partes.join(" · ")
            } else {
                etiqueta_do_tipo(n.tipo).to_string()
            });
            match n.tipo {
                Tipo::Cortesia => cobrado = 0,
                Tipo::Desconto | Tipo::Outro => cobrado = n.preco.unwrap_or(cheio),
                Tipo::Parceiro => {
                    cobrado = 0;
                    pago_fora = n.preco;
                }
            }
        }

        cupom.itens.push(ItemDoCupom {
            foto_id: f.id.clone(),
            ordem: f.ordem,
            arquivo: f.arquivo.clone(),
            faixa: faixa.nome,
            cheio,
            cobrado,
            desconto: if tipo == Some(Tipo::Parceiro) {
                0
            } else {
                cheio - cobrado
            },
            pago_fora,
            tipo,
            etiqueta,
            produto_id: f.produto_id.clone(),
            preco_negociado: f.preco_negociado,
            observacao: f.observacao.clone(),
        });
    }

    cupom.itens.sort_by(|a, b| {
        a.ordem
            .cmp(&b.ordem)
            .then_with(|| a.foto_id.cmp(&b.foto_id))
    });

    for i in &cupom.itens {
        cupom.subtotal += i.cheio;
        cupom.total += i.cobrado;
        if i.tipo == Some(Tipo::Parceiro) {
            cupom.pago_em_parceiro += i.cheio;
        } else {
            cupom.descontos += i.desconto;
        }
    }
    cupom
}

// ── As vendas e o estorno ───────────────────────────────────────────────────

/// Um pagamento gravado (numa venda ou num estorno).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagamentoGravado {
    pub forma: FormaDePagamento,
    pub valor: i64,
    pub detalhe: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemVendido {
    pub foto_id: String,
    pub arquivo: String,
    /// Já estornada nesta venda.
    pub estornada: bool,
    /// Quanto se devolveria estornando só esta foto (o desconto no total já rateado).
    pub valor_sugerido_de_estorno: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estorno {
    pub id: String,
    pub numero: i64,
    pub valor: i64,
    pub motivo: String,
    pub fotos: Vec<String>,
    pub pagamentos: Vec<PagamentoGravado>,
    /// RFC 3339, como veio.
    pub criado_em: String,
}

/// Uma venda do caixa, com o que as telas leem dela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Venda {
    pub id: String,
    pub numero: i64,
    pub galeria_id: String,
    pub itens: Vec<ItemVendido>,
    pub pagamentos: Vec<PagamentoGravado>,
    pub total: i64,
    pub troco: i64,
    pub fotografo: String,
    pub atendente: String,
    /// `None` só nas vendas anteriores a quem auxiliou existir.
    pub auxiliar: Option<String>,
    /// RFC 3339, como veio.
    pub criada_em: String,
    pub estornado: i64,
    pub estornavel: i64,
    /// As fotos que continuam vendidas: os itens menos os estornados.
    pub fotos_vendidas: Vec<String>,
    pub estornos: Vec<Estorno>,
}

impl Venda {
    /// A forma por onde a devolução sai por padrão: a de maior valor na venda.
    pub fn forma_principal(&self) -> FormaDePagamento {
        // O `sort` do site é estável e decrescente: no empate vale a primeira.
        let mut principal: Option<&PagamentoGravado> = None;
        for p in &self.pagamentos {
            if principal.is_none_or(|atual| p.valor > atual.valor) {
                principal = Some(p);
            }
        }
        principal.map_or(FormaDePagamento::Dinheiro, |p| p.forma)
    }
}

/// Quanto devolver por estas fotos de uma venda.
///
/// Estornando tudo o que resta, é exatamente o que falta — sem o centavo de
/// arredondamento que a soma dos itens deixaria sobrando; senão, a soma dos
/// escolhidos, nunca acima do que falta.
pub fn valor_sugerido_do_estorno(venda: &Venda, fotos: &HashSet<String>) -> i64 {
    let restantes = &venda.fotos_vendidas;
    if !restantes.is_empty() && restantes.iter().all(|f| fotos.contains(f)) {
        return venda.estornavel;
    }
    let soma: i64 = venda
        .itens
        .iter()
        .filter(|i| !i.estornada && fotos.contains(&i.foto_id))
        .map(|i| i.valor_sugerido_de_estorno)
        .sum();
    soma.min(venda.estornavel)
}

/// O que o formulário do estorno recusa antes de ir ao backend.
pub fn conferir_estorno(
    venda: &Venda,
    caixa_aberto: bool,
    escolhidas: &HashSet<String>,
    valor: Option<i64>,
    motivo: &str,
) -> Result<i64, String> {
    if !caixa_aberto {
        return Err("Abra o caixa do estúdio antes de estornar (F8).".into());
    }
    if escolhidas.is_empty() {
        return Err("Escolha ao menos uma foto.".into());
    }
    let valor = match valor {
        Some(v) if v >= 0 => v,
        _ => return Err("Valor inválido. Use o formato 25,00.".into()),
    };
    if valor > venda.estornavel {
        return Err(format!(
            "O estorno passa do que falta devolver ({}).",
            dinheiro::formatar(venda.estornavel)
        ));
    }
    if motivo.trim().is_empty() {
        return Err("Diga o motivo do estorno.".into());
    }
    Ok(valor)
}

/// O corpo do `POST /caixa/vendas/{id}/estornos`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovoEstorno {
    pub fotos: Vec<String>,
    pub valor: i64,
    pub motivo: String,
    pub pagamentos: Vec<PagamentoGravado>,
}

/// Monta o estorno: as fotos na ordem da venda, e a devolução só com valor.
pub fn montar_estorno(
    venda: &Venda,
    escolhidas: &HashSet<String>,
    valor: i64,
    motivo: &str,
    forma: FormaDePagamento,
    detalhe: &str,
) -> NovoEstorno {
    let detalhe = detalhe.trim();
    NovoEstorno {
        fotos: venda
            .itens
            .iter()
            .filter(|i| escolhidas.contains(&i.foto_id))
            .map(|i| i.foto_id.clone())
            .collect(),
        valor,
        motivo: motivo.trim().to_string(),
        pagamentos: if valor > 0 {
            vec![PagamentoGravado {
                forma,
                valor,
                detalhe: (!detalhe.is_empty()).then(|| detalhe.to_string()),
            }]
        } else {
            Vec::new()
        },
    }
}

// ── O desconto no total ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoDoDesconto {
    #[default]
    Valor,
    Percentual,
}

/// O desconto no total a partir do que foi digitado: `"10,00"` em reais ou
/// `"10"` (ou `"12,5"`) por cento. Nunca passa do total — a mesma recusa do
/// backend.
pub fn ler_desconto(texto: &str, modo: ModoDoDesconto, base: i64) -> Result<i64, String> {
    if texto.trim().is_empty() {
        return Ok(0);
    }
    match modo {
        ModoDoDesconto::Valor => match dinheiro::ler_campo(texto) {
            Some(c) if c >= 0 => {
                if c > base {
                    Err("O desconto passa do total.".into())
                } else {
                    Ok(c)
                }
            }
            _ => Err("Valor inválido. Use o formato 10,00.".into()),
        },
        ModoDoDesconto::Percentual => {
            // `Number(texto.trim().replace("%", "").replace(",", "."))`: só a
            // primeira ocorrência de cada, e o vazio é zero.
            let limpo = texto.trim().replacen('%', "", 1).replacen(',', ".", 1);
            let (mantissa, escala) =
                decimal(limpo.trim()).ok_or_else(|| "Percentual inválido.".to_string())?;
            if mantissa > 100 * escala {
                return Err("O desconto passa de 100%.".into());
            }
            // `Math.round(base * pct / 100)`, em inteiros.
            let numerador = i128::from(base) * mantissa;
            let denominador = 100 * escala;
            Ok(((numerador * 2 + denominador) / (2 * denominador)) as i64)
        }
    }
}

/// `"12.5"` → `(125, 10)`; vazio é zero. Só dígitos e um ponto.
fn decimal(texto: &str) -> Option<(i128, i128)> {
    if texto.is_empty() {
        return Some((0, 1));
    }
    let (inteiros, decimais) = texto.split_once('.').unwrap_or((texto, ""));
    if inteiros.is_empty() && decimais.is_empty() {
        return None;
    }
    let digitos = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if !digitos(inteiros) || !digitos(decimais) || inteiros.len() + decimais.len() > 30 {
        return None;
    }
    let mantissa: i128 = format!("{inteiros}{decimais}").parse().ok()?;
    Some((mantissa, 10_i128.pow(decimais.len() as u32)))
}

// ── O pagamento ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagamentoLancado {
    pub forma: FormaDePagamento,
    pub valor: i64,
    pub detalhe: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContaDoPagamento {
    pub recebido: i64,
    pub falta: i64,
    pub excedente_fora_do_dinheiro: i64,
    pub troco: i64,
    pub pronto: bool,
}

/// A conta do pagamento, com as **mesmas** regras do `conferir_venda` do
/// backend: falta receber bloqueia; o que passa do total só pode ser dinheiro
/// (cartão não dá troco); o troco é o que sobra.
pub fn conta_do_pagamento(total: i64, pagamentos: &[PagamentoLancado]) -> ContaDoPagamento {
    let mut recebido = 0;
    let mut fora_do_dinheiro = 0;
    for p in pagamentos {
        recebido += p.valor;
        if p.forma != FormaDePagamento::Dinheiro {
            fora_do_dinheiro += p.valor;
        }
    }
    let falta = (total - recebido).max(0);
    let excedente_fora_do_dinheiro = (fora_do_dinheiro - total).max(0);
    let pronto = falta == 0 && excedente_fora_do_dinheiro == 0;
    ContaDoPagamento {
        recebido,
        falta,
        excedente_fora_do_dinheiro,
        troco: if pronto { recebido - total } else { 0 },
        pronto,
    }
}

/// O "Lançar" do pagamento: confere o valor e o detalhe, e monta o que se grava.
///
/// No cartão, o detalhe é `"Bandeira · NSU 123"` — o que o recibo e a
/// conferência leem, e cabe nos 120 caracteres da coluna.
pub fn lancar(
    forma: FormaDePagamento,
    valor: &str,
    detalhe: &str,
    bandeira: Option<&str>,
) -> Result<PagamentoLancado, String> {
    let centavos = match dinheiro::ler_campo(valor) {
        Some(c) if c > 0 => c,
        _ => return Err("Valor inválido. Use o formato 50,00.".into()),
    };
    if forma.pede_detalhe() && detalhe.trim().is_empty() {
        return Err(format!(
            "Informe: {}.",
            forma.dica_do_detalhe().to_lowercase()
        ));
    }
    let detalhe = if forma.e_cartao() {
        let nome =
            bandeira.and_then(|b| BANDEIRAS.iter().find(|(id, _)| *id == b).map(|(_, n)| *n));
        let mut partes: Vec<String> = Vec::new();
        if let Some(nome) = nome {
            partes.push(nome.to_string());
        }
        if !detalhe.trim().is_empty() {
            partes.push(format!("NSU {}", detalhe.trim()));
        }
        partes.join(" · ")
    } else {
        detalhe.to_string()
    };
    Ok(PagamentoLancado {
        forma,
        valor: centavos,
        detalhe,
    })
}

/// Um item como o `POST /caixa/vendas` o espera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDaNovaVenda {
    pub foto_id: String,
    pub ordem: i64,
    pub arquivo: String,
    pub faixa: String,
    pub cheio: i64,
    pub cobrado: i64,
    pub tipo_de_negociacao: Option<&'static str>,
    pub etiqueta: Option<String>,
}

/// O corpo do `POST /caixa/vendas` — o `VendaRequisicao` do backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NovaVenda {
    pub galeria_id: String,
    pub itens: Vec<ItemDaNovaVenda>,
    pub desconto_no_total: i64,
    pub motivo_do_desconto: Option<String>,
    pub pagamentos: Vec<PagamentoGravado>,
    pub fotografo_id: String,
    pub atendente_id: String,
    pub auxiliar_id: String,
}

/// Quem fotografou, atendeu e auxiliou — ids do cadastro; pode ser o mesmo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pessoas {
    pub fotografo: Option<String>,
    pub atendente: Option<String>,
    pub auxiliar: Option<String>,
}

impl Pessoas {
    pub fn completas(&self) -> bool {
        self.fotografo.is_some() && self.atendente.is_some() && self.auxiliar.is_some()
    }

    pub fn alguma(&self) -> bool {
        self.fotografo.is_some() || self.atendente.is_some() || self.auxiliar.is_some()
    }

    /// O confirmar do diálogo: os três, ou a frase do site.
    pub fn conferir(&self) -> Result<(), String> {
        if self.completas() {
            Ok(())
        } else {
            Err("Escolha quem fotografou, quem atendeu e quem auxiliou.".into())
        }
    }

    /// O trio guardado só vale para quem continua no cadastro e ativo.
    pub fn so_ativos(&self, ativos: &HashSet<String>) -> Pessoas {
        let manter = |id: &Option<String>| id.clone().filter(|i| ativos.contains(i));
        Pessoas {
            fotografo: manter(&self.fotografo),
            atendente: manter(&self.atendente),
            auxiliar: manter(&self.auxiliar),
        }
    }
}

/// O que o PDV junta além do cupom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraDaVenda {
    pub desconto_no_total: i64,
    pub motivo_do_desconto: String,
    pub pagamentos: Vec<PagamentoLancado>,
    pub fotografo_id: String,
    pub atendente_id: String,
    pub auxiliar_id: String,
}

/// O corpo do `POST /caixa/vendas` a partir do cupom e do que o PDV juntou.
pub fn montar_venda(galeria_id: &str, cupom: &Cupom, extra: ExtraDaVenda) -> NovaVenda {
    let motivo = extra.motivo_do_desconto.trim();
    NovaVenda {
        galeria_id: galeria_id.to_string(),
        itens: cupom
            .itens
            .iter()
            .map(|i| ItemDaNovaVenda {
                foto_id: i.foto_id.clone(),
                ordem: i.ordem,
                arquivo: i.arquivo.clone(),
                faixa: i.faixa.clone(),
                cheio: i.cheio,
                cobrado: i.cobrado,
                tipo_de_negociacao: i.tipo.map(chave_do_tipo),
                etiqueta: i.etiqueta.clone(),
            })
            .collect(),
        desconto_no_total: extra.desconto_no_total,
        motivo_do_desconto: (extra.desconto_no_total > 0 && !motivo.is_empty())
            .then(|| motivo.to_string()),
        pagamentos: extra
            .pagamentos
            .into_iter()
            .map(|p| {
                let detalhe = p.detalhe.trim().to_string();
                PagamentoGravado {
                    forma: p.forma,
                    valor: p.valor,
                    detalhe: (!detalhe.is_empty()).then_some(detalhe),
                }
            })
            .collect(),
        fotografo_id: extra.fotografo_id,
        atendente_id: extra.atendente_id,
        auxiliar_id: extra.auxiliar_id,
    }
}

// ── Abrir, movimentar e fechar ──────────────────────────────────────────────

/// O fundo de troco digitado: vazio é zero.
pub fn ler_fundo_de_troco(texto: &str) -> Result<i64, String> {
    if texto.trim().is_empty() {
        return Ok(0);
    }
    match dinheiro::ler_campo(texto) {
        Some(c) if c >= 0 => Ok(c),
        _ => Err("Valor inválido. Use o formato 200,00.".into()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TipoDeMovimento {
    #[default]
    Sangria,
    Suprimento,
}

impl TipoDeMovimento {
    pub fn chave(self) -> &'static str {
        match self {
            TipoDeMovimento::Sangria => "sangria",
            TipoDeMovimento::Suprimento => "suprimento",
        }
    }

    pub fn da_chave(chave: &str) -> Option<Self> {
        match chave {
            "sangria" => Some(TipoDeMovimento::Sangria),
            "suprimento" => Some(TipoDeMovimento::Suprimento),
            _ => None,
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            TipoDeMovimento::Sangria => "Sangria",
            TipoDeMovimento::Suprimento => "Suprimento",
        }
    }

    /// `−` na sangria, `+` no suprimento.
    pub fn sinal(self) -> &'static str {
        match self {
            TipoDeMovimento::Sangria => "−",
            TipoDeMovimento::Suprimento => "+",
        }
    }
}

/// A sangria ou o suprimento: valor positivo e motivo dito.
pub fn ler_movimento(valor: &str, motivo: &str) -> Result<i64, String> {
    let centavos = match dinheiro::ler_campo(valor) {
        Some(c) if c > 0 => c,
        _ => return Err("Valor inválido. Use o formato 100,00.".into()),
    };
    if motivo.trim().is_empty() {
        return Err("Diga o motivo.".into());
    }
    Ok(centavos)
}

/// Valores por forma de pagamento (contado, esperado, diferença).
pub type PorForma = BTreeMap<FormaDePagamento, i64>;

/// Os textos da contagem cega em centavos; o primeiro valor inválido volta
/// como erro. Campo vazio fica de fora.
pub fn ler_contagem(textos: &[(FormaDePagamento, String)]) -> Result<PorForma, String> {
    let mut contado = PorForma::new();
    for forma in FormaDePagamento::TODAS {
        let Some((_, texto)) = textos.iter().find(|(f, _)| *f == forma) else {
            continue;
        };
        if texto.trim().is_empty() {
            continue;
        }
        match dinheiro::ler_campo(texto) {
            Some(v) if v >= 0 => {
                contado.insert(forma, v);
            }
            _ => return Err(format!("Valor inválido em {}.", forma.rotulo())),
        }
    }
    Ok(contado)
}

fn de(mapa: &PorForma, forma: FormaDePagamento) -> i64 {
    mapa.get(&forma).copied().unwrap_or(0)
}

/// As formas que a tabela do fechamento mostra: as que têm algum valor.
pub fn formas_da_tabela(
    contado: &PorForma,
    esperado: &PorForma,
    primeira: Option<&PorForma>,
) -> Vec<FormaDePagamento> {
    FormaDePagamento::TODAS
        .into_iter()
        .filter(|f| {
            de(contado, *f) != 0
                || de(esperado, *f) != 0
                || primeira.is_some_and(|p| de(p, *f) != 0)
        })
        .collect()
}

/// A frase da conferência e se ela é de alerta (alguma forma não bate).
pub fn frase_da_conferencia(diferenca: &PorForma) -> (String, bool) {
    let total: i64 = FormaDePagamento::TODAS
        .iter()
        .map(|f| de(diferenca, *f))
        .sum();
    let divergentes: Vec<FormaDePagamento> = FormaDePagamento::TODAS
        .into_iter()
        .filter(|f| de(diferenca, *f) != 0)
        .collect();
    if divergentes.is_empty() {
        return (
            "A contagem bate com o esperado em todas as formas.".into(),
            false,
        );
    }
    let nomes = divergentes
        .iter()
        .map(|f| f.rotulo())
        .collect::<Vec<_>>()
        .join(", ");
    let verbo = if divergentes.len() == 1 {
        "bate"
    } else {
        "batem"
    };
    let sinal = if total > 0 { "+" } else { "" };
    (
        format!(
            "{nomes} não {verbo} (total {sinal}{}). Se foi erro de digitação, corrija antes de fechar.",
            dinheiro::formatar(total)
        ),
        true,
    )
}

/// A contagem foi corrigida na conferência? A primeira difere da que fechou.
pub fn contagem_corrigida(primeira: Option<&PorForma>, contado: &PorForma) -> bool {
    primeira.is_some_and(|p| {
        FormaDePagamento::TODAS
            .iter()
            .any(|f| de(p, *f) != de(contado, *f))
    })
}

/// A diferença como a tabela a escreve: `+R$ 1,00`, `-R$ 1,00`, `R$ 0,00`.
pub fn diferenca_escrita(valor: i64) -> String {
    format!(
        "{}{}",
        if valor > 0 { "+" } else { "" },
        dinheiro::formatar(valor)
    )
}

// ── O que as teclas F fazem ─────────────────────────────────────────────────

/// Em que pé o caixa do estúdio está, para as decisões do PDV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SituacaoDoPdv {
    pub tem_estudio: bool,
    /// A API não respondeu: não dá para saber se o caixa está aberto.
    pub indisponivel: bool,
    pub aberto: bool,
}

/// O que um gesto do PDV resolve fazer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Desfecho {
    /// Segue para o que foi pedido.
    Seguir,
    /// O caixa está fechado: abre o diálogo de abrir.
    AbrirCaixa,
    /// Uma frase de erro, e nada abre.
    Erro(String),
    /// Uma frase informativa, e nada abre.
    Aviso(String),
    /// Faltam os nomes: as pessoas antes, e o pagamento em seguida.
    PessoasAntes,
}

/// A frase quando falta o estúdio — a desta tela diz onde escolhê-lo.
pub const SEM_ESTUDIO: &str = "Escolha o estúdio do caixa no alto da tela.";
const SEM_RESPOSTA: &str = "O caixa não respondeu. Recarregue a página.";

/// F2 e F6: só abrem se o caixa do estúdio puder ser usado agora.
pub fn precisa_caixa(s: SituacaoDoPdv) -> Desfecho {
    if !s.tem_estudio {
        Desfecho::Erro(SEM_ESTUDIO.into())
    } else if s.indisponivel {
        Desfecho::Erro(SEM_RESPOSTA.into())
    } else if !s.aberto {
        Desfecho::AbrirCaixa
    } else {
        Desfecho::Seguir
    }
}

/// F4: finalizar o pagamento.
pub fn finalizar(
    s: SituacaoDoPdv,
    tem_sessao: bool,
    itens: usize,
    pessoas_completas: bool,
) -> Desfecho {
    match precisa_caixa(s) {
        Desfecho::Seguir => {}
        outro => return outro,
    }
    if !tem_sessao {
        Desfecho::Aviso("Escolha a sessão a cobrar.".into())
    } else if itens == 0 {
        Desfecho::Aviso("Nenhuma foto sinalizada para cobrar.".into())
    } else if !pessoas_completas {
        Desfecho::PessoasAntes
    } else {
        Desfecho::Seguir
    }
}

/// F8: abre o caixa fechado, fecha o aberto. `Seguir` = o diálogo que cabe
/// (`aberto` diz qual).
pub fn abrir_ou_fechar(s: SituacaoDoPdv) -> Desfecho {
    if !s.tem_estudio {
        Desfecho::Erro(SEM_ESTUDIO.into())
    } else if s.indisponivel {
        Desfecho::Erro("O caixa não respondeu.".into())
    } else {
        Desfecho::Seguir
    }
}

/// `ATALHOS_DO_PDV`, só as teclas que a tela do caixa tem.
///
/// ⚠️ O site lista também F9 e as teclas do cupom (↑ ↓, E, T, C, D, S, ⌫, N):
/// são do painel flutuante da galeria, e a rota `/dashboard/caixa` não as liga
/// — mostrá-las aqui anunciaria o que a tela não faz.
pub const ATALHOS: [(&str, &str); 7] = [
    ("F1", "Esta lista de atalhos"),
    ("F2", "Desconto no total"),
    (
        "F3",
        "Quem fotografou, atendeu e auxiliou (do cadastro de funcionários)",
    ),
    ("F4", "Finalizar pagamento (e concluir, dentro dele)"),
    ("F6", "Sangria ou suprimento"),
    ("F7", "Vendas da sessão (e estornar) e o caixa do estúdio"),
    ("F8", "Abrir o caixa, ou fechar o aberto"),
];

/// As teclas que só o painel flutuante da galeria tem (`ATALHOS_DO_PDV` do
/// site, depois do F8).
pub const ATALHOS_DO_PAINEL: [(&str, &str); 10] = [
    ("F9", "Minimizar ou abrir o painel"),
    ("↑ ↓", "No cupom: troca de item"),
    (
        "E",
        "No cupom: abre ou fecha o ajuste do item (tipo de ensaio e negociação)",
    ),
    ("T", "Com o ajuste aberto: tipo de ensaio"),
    ("C", "No item: cortesia"),
    ("D", "No item: desconto (valor cobrado, Enter grava)"),
    ("S", "No item: pago em site parceiro (cupom, Enter grava)"),
    ("⌫", "No item: remove a negociação"),
    (
        "N",
        "No cupom: negociação completa do item (sem item ou com Shift, de todas as fotos)",
    ),
    (
        "Shift",
        "Com T, C, D, S ou ⌫: aplica a todos os itens do cupom",
    ),
];

/// A frase quando a sessão da galeria não tem estúdio.
pub const SESSAO_SEM_ESTUDIO: &str =
    "Esta sessão não tem estúdio: escolha o estúdio no cabeçalho antes de vender.";

/// Os centavos como o `PATCH` da foto os espera: decimal em texto (`"15.00"`).
pub fn decimal_da_api(centavos: i64) -> String {
    let sinal = if centavos < 0 { "-" } else { "" };
    let absoluto = centavos.unsigned_abs();
    format!("{sinal}{}.{:02}", absoluto / 100, absoluto % 100)
}

// ── A lista de sessões e o estúdio ──────────────────────────────────────────

/// Uma linha da lista de sessões do estúdio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessaoACobrar {
    pub id: String,
    pub titulo: String,
    pub contato: Option<String>,
    /// RFC 3339 — comparável como texto quando vem no mesmo formato.
    pub criada_em: String,
    /// As levadas no balcão, vendidas ou não: quem decide o cupom é a venda.
    pub sinalizadas: i64,
}

/// As com fotos sinalizadas primeiro; dentro de cada grupo, as mais recentes.
pub fn ordenar_sessoes(sessoes: &mut [SessaoACobrar]) {
    sessoes.sort_by(|a, b| {
        (b.sinalizadas > 0)
            .cmp(&(a.sinalizadas > 0))
            .then_with(|| b.criada_em.cmp(&a.criada_em))
    });
}

/// A busca da lista: título ou contato, sem caixa.
pub fn filtrar_sessoes<'a>(sessoes: &'a [SessaoACobrar], busca: &str) -> Vec<&'a SessaoACobrar> {
    let termo = busca.trim().to_lowercase();
    sessoes
        .iter()
        .filter(|s| {
            termo.is_empty()
                || s.titulo.to_lowercase().contains(&termo)
                || s.contato
                    .as_deref()
                    .is_some_and(|c| c.to_lowercase().contains(&termo))
        })
        .collect()
}

/// O estúdio do caixa: o pedido manda; senão o da sessão aberta; senão **o
/// estúdio desta máquina**; e só então o primeiro ativo.
///
/// 🚨 **`da_maquina` entrou em 18/set/2026** (dono: *"essa seleção de estúdio
/// [precisa] influenciar o caixa também"*). O último recurso — o primeiro
/// ativo — é um chute, e metade das vezes ele é o estúdio de outra cidade: o
/// caixa abria em Gramado para quem está em Canela. Com a escolha da entrada
/// das sessões, o chute só acontece para quem ainda não escolheu nada.
///
/// A ordem é a mesma do site (`caixa/carregar.ts`), e é o que faz as três
/// interfaces abrirem o mesmo caixa.
pub fn escolher_estudio(
    ativos: &[String],
    pedido: Option<&str>,
    da_sessao: Option<&str>,
    da_maquina: Option<&str>,
) -> Option<String> {
    let achar = |id: Option<&str>| id.and_then(|id| ativos.iter().find(|a| a.as_str() == id));
    achar(pedido)
        .or_else(|| achar(da_sessao))
        .or_else(|| achar(da_maquina))
        .or_else(|| ativos.first())
        .cloned()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn foto(id: &str) -> FotoDoCupom {
        FotoDoCupom {
            id: id.into(),
            ordem: 0,
            arquivo: format!("{id}.jpg"),
            estado: Estado::LevadaNoBalcao,
            apagada: false,
            nota: Some(3),
            sem_marcacao: false,
            produto_efetivo: "p".into(),
            produto_id: None,
            preco_negociado: None,
            observacao: None,
        }
    }

    fn com(mut f: FotoDoCupom, mudar: impl FnOnce(&mut FotoDoCupom)) -> FotoDoCupom {
        mudar(&mut f);
        f
    }

    fn negociada(id: &str, ordem: i64, preco: Option<i64>, obs: &str) -> FotoDoCupom {
        com(foto(id), |f| {
            f.ordem = ordem;
            f.preco_negociado = preco;
            f.observacao = Some(obs.into());
        })
    }

    fn faixa(_: &FotoDoCupom) -> Faixa {
        Faixa {
            nome: "Digital".into(),
            preco: 2500,
        }
    }

    fn nada() -> HashSet<String> {
        HashSet::new()
    }

    #[test]
    fn so_a_sinalizada_viva_com_nota_e_com_marcacao_entra() {
        assert!(entra_no_caixa(&foto("a")));
        assert!(!entra_no_caixa(
            &com(foto("b"), |f| f.estado = Estado::Disponivel)
        ));
        assert!(!entra_no_caixa(
            &com(foto("c"), |f| f.estado = Estado::Comprada)
        ));
        assert!(!entra_no_caixa(&com(foto("d"), |f| f.nota = None)));
        assert!(!entra_no_caixa(&com(foto("e"), |f| f.apagada = true)));
        assert!(!entra_no_caixa(&com(foto("f"), |f| f.sem_marcacao = true)));
    }

    #[test]
    fn sem_negociacao_cobra_a_faixa() {
        let c = fechar_caixa(
            &[foto("a"), com(foto("b"), |f| f.ordem = 1)],
            faixa,
            &nada(),
        );
        assert_eq!(c.itens.len(), 2);
        assert_eq!((c.subtotal, c.total, c.descontos), (5000, 5000, 0));
    }

    #[test]
    fn cortesia_zera_o_item_e_a_faixa_inteira_vira_desconto() {
        let c = fechar_caixa(
            &[negociada("a", 0, Some(0), "Cortesia — aniversário")],
            faixa,
            &nada(),
        );
        let i = &c.itens[0];
        assert_eq!((i.cobrado, i.desconto), (0, 2500));
        assert_eq!(i.tipo, Some(Tipo::Cortesia));
        assert_eq!(i.etiqueta.as_deref(), Some("Cortesia"));
        assert_eq!((c.total, c.descontos), (0, 2500));
    }

    #[test]
    fn desconto_cobra_o_valor_combinado() {
        let c = fechar_caixa(&[negociada("a", 0, Some(1500), "Desconto")], faixa, &nada());
        assert_eq!((c.itens[0].cobrado, c.itens[0].desconto), (1500, 1000));
        assert_eq!(c.total, 1500);
    }

    #[test]
    fn paga_em_parceiro_nao_entra_no_caixa_e_o_valor_de_la_e_so_informativo() {
        let c = fechar_caixa(
            &[negociada("a", 0, Some(1990), "TchêOfertas — cupom 123")],
            faixa,
            &nada(),
        );
        let i = &c.itens[0];
        assert_eq!((i.cobrado, i.desconto, i.pago_fora), (0, 0, Some(1990)));
        assert_eq!(i.etiqueta.as_deref(), Some("TchêOfertas · cupom 123"));
        assert_eq!((c.pago_em_parceiro, c.descontos, c.total), (2500, 0, 0));
    }

    #[test]
    fn acerto_so_em_texto_cobra_a_faixa_e_com_valor_cobra_o_valor() {
        let c = fechar_caixa(
            &[
                negociada("a", 0, None, "leva o pendrive"),
                negociada("b", 1, Some(3000), "impressa junto"),
            ],
            faixa,
            &nada(),
        );
        let cobrados: Vec<i64> = c.itens.iter().map(|i| i.cobrado).collect();
        assert_eq!(cobrados, vec![2500, 3000]);
        // Acima da faixa: desconto negativo, e a soma continua fechando.
        assert_eq!(c.descontos, -500);
    }

    #[test]
    fn negociacao_em_foto_fora_do_cupom_e_contada_e_nao_somada() {
        let c = fechar_caixa(
            &[
                com(negociada("a", 0, Some(0), "Cortesia"), |f| {
                    f.estado = Estado::Disponivel
                }),
                com(foto("b"), |f| {
                    f.estado = Estado::Disponivel;
                    f.apagada = true;
                    f.preco_negociado = Some(0);
                }),
            ],
            faixa,
            &nada(),
        );
        assert!(c.itens.is_empty());
        assert_eq!((c.total, c.negociadas_fora), (0, 1));
    }

    #[test]
    fn a_ja_vendida_sai_do_cupom_e_e_contada_a_parte() {
        let vendidas: HashSet<String> = ["a".to_string()].into();
        let c = fechar_caixa(
            &[foto("a"), com(foto("b"), |f| f.ordem = 1)],
            faixa,
            &vendidas,
        );
        let ids: Vec<&str> = c.itens.iter().map(|i| i.foto_id.as_str()).collect();
        assert_eq!(ids, vec!["b"]);
        assert_eq!((c.ja_vendidas, c.total), (1, 2500));
    }

    #[test]
    fn a_conta_fecha_e_os_itens_seguem_a_ordem_da_sessao() {
        let c = fechar_caixa(
            &[
                com(foto("z"), |f| f.ordem = 4),
                negociada("y", 2, Some(0), "Cortesia"),
                negociada("x", 3, Some(1000), "Desconto — combo"),
                negociada("w", 1, None, "ParksNet"),
            ],
            faixa,
            &nada(),
        );
        let ordens: Vec<i64> = c.itens.iter().map(|i| i.ordem).collect();
        assert_eq!(ordens, vec![1, 2, 3, 4]);
        assert_eq!(c.subtotal - c.descontos - c.pago_em_parceiro, c.total);
        assert_eq!(c.total, 3500);
    }

    #[test]
    fn a_observacao_vazia_nao_e_negociacao() {
        let c = fechar_caixa(&[negociada("a", 0, None, "")], faixa, &nada());
        assert_eq!(c.itens[0].tipo, None);
        assert_eq!(c.itens[0].cobrado, 2500);
    }

    fn venda() -> Venda {
        let item = |id: &str, estornada, sugerido| ItemVendido {
            foto_id: id.into(),
            arquivo: format!("{id}.jpg"),
            estornada,
            valor_sugerido_de_estorno: sugerido,
        };
        Venda {
            id: "v".into(),
            numero: 1,
            galeria_id: "g".into(),
            itens: vec![
                item("a", false, 2700),
                item("b", false, 900),
                item("c", true, 0),
            ],
            pagamentos: vec![],
            total: 3601,
            troco: 0,
            fotografo: "F".into(),
            atendente: "A".into(),
            auxiliar: None,
            criada_em: String::new(),
            estornado: 0,
            estornavel: 3601,
            fotos_vendidas: vec!["a".into(), "b".into()],
            estornos: vec![],
        }
    }

    fn conjunto(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn vendidas_na_junta_o_que_o_backend_diz_que_continua_vendido() {
        let mut outra = venda();
        outra.fotos_vendidas = vec![];
        let mut ids: Vec<String> = vendidas_na(&[venda(), outra]).into_iter().collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn estornar_tudo_o_que_resta_e_exatamente_o_que_falta() {
        assert_eq!(
            valor_sugerido_do_estorno(&venda(), &conjunto(&["a", "b"])),
            3601
        );
    }

    #[test]
    fn parte_das_fotos_soma_a_sugestao_sem_passar_do_que_falta() {
        assert_eq!(valor_sugerido_do_estorno(&venda(), &conjunto(&["b"])), 900);
        let mut pouca = venda();
        pouca.estornavel = 500;
        assert_eq!(valor_sugerido_do_estorno(&pouca, &conjunto(&["b"])), 500);
        assert_eq!(valor_sugerido_do_estorno(&venda(), &conjunto(&[])), 0);
    }

    #[test]
    fn o_estorno_recusa_na_ordem_do_site() {
        let v = venda();
        let a = conjunto(&["a"]);
        assert!(conferir_estorno(&v, false, &a, Some(1), "x")
            .unwrap_err()
            .contains("F8"));
        assert_eq!(
            conferir_estorno(&v, true, &conjunto(&[]), Some(1), "x").unwrap_err(),
            "Escolha ao menos uma foto."
        );
        assert!(conferir_estorno(&v, true, &a, None, "x")
            .unwrap_err()
            .contains("25,00"));
        assert_eq!(
            conferir_estorno(&v, true, &a, Some(9999), "x").unwrap_err(),
            "O estorno passa do que falta devolver (R$ 36,01)."
        );
        assert_eq!(
            conferir_estorno(&v, true, &a, Some(10), "  ").unwrap_err(),
            "Diga o motivo do estorno."
        );
        assert_eq!(conferir_estorno(&v, true, &a, Some(0), "desistiu"), Ok(0));
    }

    #[test]
    fn a_devolucao_so_vai_com_valor_e_o_detalhe_vazio_e_nulo() {
        let e = montar_estorno(
            &venda(),
            &conjunto(&["b", "a"]),
            3601,
            " desistiu ",
            FormaDePagamento::Pix,
            "  ",
        );
        assert_eq!(e.fotos, vec!["a", "b"]);
        assert_eq!(e.motivo, "desistiu");
        assert_eq!(
            e.pagamentos,
            vec![PagamentoGravado {
                forma: FormaDePagamento::Pix,
                valor: 3601,
                detalhe: None
            }]
        );
        let sem = montar_estorno(
            &venda(),
            &conjunto(&["a"]),
            0,
            "x",
            FormaDePagamento::Pix,
            "",
        );
        assert!(sem.pagamentos.is_empty());
    }

    #[test]
    fn a_devolucao_sai_pela_forma_de_maior_valor() {
        let mut v = venda();
        assert_eq!(v.forma_principal(), FormaDePagamento::Dinheiro);
        let p = |forma, valor| PagamentoGravado {
            forma,
            valor,
            detalhe: None,
        };
        v.pagamentos = vec![
            p(FormaDePagamento::Pix, 1000),
            p(FormaDePagamento::Credito, 3000),
            p(FormaDePagamento::Dinheiro, 3000),
        ];
        assert_eq!(v.forma_principal(), FormaDePagamento::Credito);
    }

    #[test]
    fn o_desconto_em_reais_em_percentual_e_nunca_acima_do_total() {
        use ModoDoDesconto::*;
        assert_eq!(ler_desconto("", Valor, 5000), Ok(0));
        assert_eq!(ler_desconto("10,00", Valor, 5000), Ok(1000));
        assert_eq!(ler_desconto("12,5", Percentual, 8000), Ok(1000));
        assert_eq!(ler_desconto("10%", Percentual, 2999), Ok(300));
        assert!(ler_desconto("60,00", Valor, 5000).is_err());
        assert!(ler_desconto("101", Percentual, 5000).is_err());
        assert!(ler_desconto("abc", Valor, 5000).is_err());
        assert!(ler_desconto("-1", Valor, 5000).is_err());
        assert!(ler_desconto("1,2,3", Percentual, 5000).is_err());
        assert_eq!(ler_desconto("100", Percentual, 2999), Ok(2999));
        // `Math.round` sobe no meio.
        assert_eq!(ler_desconto("50", Percentual, 1), Ok(1));
        assert_eq!(ler_desconto("%", Percentual, 1000), Ok(0));
    }

    fn pago(forma: FormaDePagamento, valor: i64) -> PagamentoLancado {
        PagamentoLancado {
            forma,
            valor,
            detalhe: String::new(),
        }
    }

    #[test]
    fn falta_receber_nao_fecha() {
        let c = conta_do_pagamento(2500, &[pago(FormaDePagamento::Pix, 2000)]);
        assert_eq!((c.falta, c.pronto, c.troco), (500, false, 0));
    }

    #[test]
    fn o_troco_sai_do_dinheiro() {
        let c = conta_do_pagamento(
            3500,
            &[
                pago(FormaDePagamento::Pix, 1000),
                pago(FormaDePagamento::Dinheiro, 5000),
            ],
        );
        assert_eq!((c.recebido, c.troco, c.pronto), (6000, 2500, true));
    }

    #[test]
    fn cartao_acima_do_total_nao_da_troco() {
        let c = conta_do_pagamento(2500, &[pago(FormaDePagamento::Credito, 3000)]);
        assert_eq!((c.excedente_fora_do_dinheiro, c.pronto), (500, false));
    }

    #[test]
    fn total_zero_fecha_sem_pagamento() {
        let c = conta_do_pagamento(0, &[]);
        assert_eq!((c.pronto, c.troco), (true, 0));
    }

    #[test]
    fn lancar_confere_valor_detalhe_e_monta_o_do_cartao() {
        use FormaDePagamento::*;
        assert!(lancar(Pix, "0", "", None).unwrap_err().contains("50,00"));
        assert!(lancar(Pix, "abc", "", None).is_err());
        assert_eq!(
            lancar(Voucher, "10,00", " ", None).unwrap_err(),
            "Informe: número do voucher."
        );
        assert_eq!(
            lancar(Credito, "10,00", "123", Some("visa")).unwrap(),
            PagamentoLancado {
                forma: Credito,
                valor: 1000,
                detalhe: "Visa · NSU 123".into()
            }
        );
        assert_eq!(lancar(Debito, "1", "", Some("elo")).unwrap().detalhe, "Elo");
        assert_eq!(lancar(Debito, "1", "9", None).unwrap().detalhe, "NSU 9");
        assert_eq!(lancar(Pix, "1", " id ", None).unwrap().detalhe, " id ");
        assert_eq!(bandeira_do_detalhe("Visa · NSU 123"), Some("visa"));
        assert_eq!(bandeira_do_detalhe("Elo"), Some("elo"));
        assert_eq!(bandeira_do_detalhe("NSU 9"), None);
    }

    #[test]
    fn montar_venda_leva_o_cupom_como_o_backend_espera() {
        let cupom = fechar_caixa(
            &[
                negociada("a", 0, Some(0), "Cortesia"),
                com(foto("b"), |f| f.ordem = 1),
            ],
            faixa,
            &nada(),
        );
        let v = montar_venda(
            "g",
            &cupom,
            ExtraDaVenda {
                desconto_no_total: 0,
                motivo_do_desconto: "ignorado".into(),
                pagamentos: vec![PagamentoLancado {
                    forma: FormaDePagamento::Dinheiro,
                    valor: 2500,
                    detalhe: "  ".into(),
                }],
                fotografo_id: "f-1".into(),
                atendente_id: "a-1".into(),
                auxiliar_id: "f-1".into(),
            },
        );
        assert_eq!(v.itens[0].foto_id, "a");
        assert_eq!((v.itens[0].cheio, v.itens[0].cobrado), (2500, 0));
        assert_eq!(v.itens[0].tipo_de_negociacao, Some("cortesia"));
        assert_eq!(v.itens[1].tipo_de_negociacao, None);
        assert_eq!(v.itens[1].cobrado, 2500);
        assert_eq!(v.motivo_do_desconto, None);
        assert_eq!(
            v.pagamentos,
            vec![PagamentoGravado {
                forma: FormaDePagamento::Dinheiro,
                valor: 2500,
                detalhe: None
            }]
        );
        assert_eq!(
            (
                v.fotografo_id.as_str(),
                v.atendente_id.as_str(),
                v.auxiliar_id.as_str()
            ),
            ("f-1", "a-1", "f-1")
        );
    }

    #[test]
    fn o_motivo_do_desconto_so_vai_com_desconto() {
        let extra = |desconto| ExtraDaVenda {
            desconto_no_total: desconto,
            motivo_do_desconto: " fidelidade ".into(),
            pagamentos: vec![],
            fotografo_id: String::new(),
            atendente_id: String::new(),
            auxiliar_id: String::new(),
        };
        let cupom = Cupom::default();
        assert_eq!(
            montar_venda("g", &cupom, extra(100))
                .motivo_do_desconto
                .as_deref(),
            Some("fidelidade")
        );
        assert_eq!(montar_venda("g", &cupom, extra(0)).motivo_do_desconto, None);
    }

    #[test]
    fn abrir_e_movimentar_recusam_como_o_site() {
        assert_eq!(ler_fundo_de_troco(""), Ok(0));
        assert_eq!(ler_fundo_de_troco("200,00"), Ok(20000));
        assert!(ler_fundo_de_troco("-1").is_err());
        assert!(ler_fundo_de_troco("x").is_err());
        assert_eq!(ler_movimento("100,00", "troco"), Ok(10000));
        assert_eq!(
            ler_movimento("0", "troco").unwrap_err(),
            "Valor inválido. Use o formato 100,00."
        );
        assert_eq!(ler_movimento("1", " ").unwrap_err(), "Diga o motivo.");
    }

    #[test]
    fn a_contagem_cega_deixa_o_vazio_de_fora_e_para_no_primeiro_erro() {
        use FormaDePagamento::*;
        let lido = ler_contagem(&[(Dinheiro, "200,00".into()), (Pix, " ".into())]).unwrap();
        assert_eq!(lido, PorForma::from([(Dinheiro, 20000)]));
        assert_eq!(
            ler_contagem(&[(Credito, "x".into()), (Pix, "-1".into())]).unwrap_err(),
            "Valor inválido em PIX."
        );
    }

    #[test]
    fn a_conferencia_diz_o_que_nao_bate() {
        use FormaDePagamento::*;
        assert_eq!(
            frase_da_conferencia(&PorForma::from([(Dinheiro, 0)])),
            (
                "A contagem bate com o esperado em todas as formas.".into(),
                false
            )
        );
        let (frase, alerta) = frase_da_conferencia(&PorForma::from([(Dinheiro, 500)]));
        assert!(alerta);
        assert_eq!(
            frase,
            "Dinheiro não bate (total +R$ 5,00). Se foi erro de digitação, corrija antes de fechar."
        );
        let (frase, _) = frase_da_conferencia(&PorForma::from([(Dinheiro, 500), (Pix, -700)]));
        assert!(frase.starts_with("Dinheiro, PIX não batem (total -R$ 2,00)."));
    }

    #[test]
    fn a_tabela_mostra_so_as_formas_com_valor_e_percebe_a_correcao() {
        use FormaDePagamento::*;
        let contado = PorForma::from([(Dinheiro, 100)]);
        let esperado = PorForma::from([(Pix, 200), (Credito, 0)]);
        let primeira = PorForma::from([(Voucher, 5), (Dinheiro, 100)]);
        assert_eq!(
            formas_da_tabela(&contado, &esperado, None),
            vec![Dinheiro, Pix]
        );
        assert_eq!(
            formas_da_tabela(&contado, &esperado, Some(&primeira)),
            vec![Dinheiro, Pix, Voucher]
        );
        assert!(contagem_corrigida(Some(&primeira), &contado));
        assert!(!contagem_corrigida(
            Some(&PorForma::from([(Dinheiro, 100), (Pix, 0)])),
            &contado
        ));
        assert!(!contagem_corrigida(None, &contado));
        assert_eq!(diferenca_escrita(100), "+R$ 1,00");
        assert_eq!(diferenca_escrita(-100), "-R$ 1,00");
        assert_eq!(diferenca_escrita(0), "R$ 0,00");
    }

    fn situacao(tem_estudio: bool, indisponivel: bool, aberto: bool) -> SituacaoDoPdv {
        SituacaoDoPdv {
            tem_estudio,
            indisponivel,
            aberto,
        }
    }

    #[test]
    fn as_teclas_do_caixa_pedem_estudio_resposta_e_caixa_aberto() {
        assert_eq!(
            precisa_caixa(situacao(false, false, true)),
            Desfecho::Erro(SEM_ESTUDIO.into())
        );
        assert!(matches!(
            precisa_caixa(situacao(true, true, true)),
            Desfecho::Erro(_)
        ));
        assert_eq!(
            precisa_caixa(situacao(true, false, false)),
            Desfecho::AbrirCaixa
        );
        assert_eq!(precisa_caixa(situacao(true, false, true)), Desfecho::Seguir);
        assert_eq!(
            abrir_ou_fechar(situacao(true, false, false)),
            Desfecho::Seguir
        );
        assert_eq!(
            abrir_ou_fechar(situacao(true, true, false)),
            Desfecho::Erro("O caixa não respondeu.".into())
        );
    }

    #[test]
    fn f4_pede_sessao_itens_e_nomes_nessa_ordem() {
        let ok = situacao(true, false, true);
        assert_eq!(
            finalizar(situacao(true, false, false), true, 1, true),
            Desfecho::AbrirCaixa
        );
        assert_eq!(
            finalizar(ok, false, 1, true),
            Desfecho::Aviso("Escolha a sessão a cobrar.".into())
        );
        assert_eq!(
            finalizar(ok, true, 0, true),
            Desfecho::Aviso("Nenhuma foto sinalizada para cobrar.".into())
        );
        assert_eq!(finalizar(ok, true, 1, false), Desfecho::PessoasAntes);
        assert_eq!(finalizar(ok, true, 1, true), Desfecho::Seguir);
    }

    #[test]
    fn as_pessoas_valem_so_para_quem_continua_ativo() {
        let p = Pessoas {
            fotografo: Some("a".into()),
            atendente: Some("b".into()),
            auxiliar: Some("a".into()),
        };
        assert!(p.conferir().is_ok());
        let so_a = p.so_ativos(&conjunto(&["a"]));
        assert_eq!(so_a.atendente, None);
        assert!(so_a.alguma() && !so_a.completas());
        assert!(so_a.conferir().is_err());
        assert!(!Pessoas::default().alguma());
    }

    fn sessao(id: &str, sinalizadas: i64, criada_em: &str, contato: Option<&str>) -> SessaoACobrar {
        SessaoACobrar {
            id: id.into(),
            titulo: format!("Ensaio {id}"),
            contato: contato.map(Into::into),
            criada_em: criada_em.into(),
            sinalizadas,
        }
    }

    #[test]
    fn as_sessoes_com_sinalizadas_vem_primeiro_e_depois_as_recentes() {
        let mut lista = vec![
            sessao("velha", 0, "2026-09-01T00:00:00Z", None),
            sessao("nova", 0, "2026-09-10T00:00:00Z", None),
            sessao("sinalizada-velha", 2, "2026-08-01T00:00:00Z", None),
            sessao("sinalizada-nova", 1, "2026-09-02T00:00:00Z", None),
        ];
        ordenar_sessoes(&mut lista);
        let ids: Vec<&str> = lista.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["sinalizada-nova", "sinalizada-velha", "nova", "velha"]
        );
    }

    #[test]
    fn a_busca_acha_por_titulo_ou_contato() {
        let lista = vec![
            sessao("Maria", 0, "", Some("maria@x.com")),
            sessao("João", 0, "", Some("5551999")),
        ];
        assert_eq!(filtrar_sessoes(&lista, "  ").len(), 2);
        assert_eq!(filtrar_sessoes(&lista, "MARIA")[0].id, "Maria");
        assert_eq!(filtrar_sessoes(&lista, "5551")[0].id, "João");
        assert!(filtrar_sessoes(&lista, "pedro").is_empty());
    }

    /// 🚨 A ordem inteira, com o estúdio da máquina entre a sessão e o chute.
    #[test]
    fn o_estudio_do_pedido_manda_depois_o_da_sessao_depois_o_da_maquina() {
        let ativos = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        // O pedido no endereço vence tudo.
        assert_eq!(
            escolher_estudio(&ativos, Some("b"), Some("a"), Some("c")).as_deref(),
            Some("b")
        );
        // Pedido que não existe mais cai para o da sessão.
        assert_eq!(
            escolher_estudio(&ativos, Some("x"), Some("b"), Some("c")).as_deref(),
            Some("b")
        );
        // Sem sessão com estúdio, vale o **desta máquina** — e não o primeiro.
        assert_eq!(
            escolher_estudio(&ativos, None, None, Some("c")).as_deref(),
            Some("c"),
            "o caixa abre no estúdio em que o operador disse estar"
        );
        assert_eq!(
            escolher_estudio(&ativos, None, Some("x"), Some("c")).as_deref(),
            Some("c")
        );
        // Sem nada escolhido, o chute: o primeiro ativo.
        assert_eq!(
            escolher_estudio(&ativos, None, None, None).as_deref(),
            Some("a")
        );
        // Estúdio de máquina que saiu do cadastro não vale.
        assert_eq!(
            escolher_estudio(&ativos, None, None, Some("sumiu")).as_deref(),
            Some("a")
        );
        assert_eq!(escolher_estudio(&[], Some("a"), None, Some("c")), None);
    }

    #[test]
    fn o_item_tem_codigo_e_lembra_a_negociacao_gravada() {
        let c = fechar_caixa(
            &[negociada("a", 6, Some(1500), "Desconto — combo")],
            faixa,
            &nada(),
        );
        let i = &c.itens[0];
        assert_eq!(i.codigo(), "#0007");
        assert!(i.negociado());
        assert_eq!(i.observacao.as_deref(), Some("Desconto — combo"));
        assert!(!fechar_caixa(&[foto("b")], faixa, &nada()).itens[0].negociado());
        assert_eq!(decimal_da_api(1500), "15.00");
        assert_eq!(decimal_da_api(5), "0.05");
    }

    #[test]
    fn as_formas_seguem_as_teclas_e_as_chaves_do_backend() {
        assert_eq!(
            FormaDePagamento::da_tecla(1),
            Some(FormaDePagamento::Dinheiro)
        );
        assert_eq!(FormaDePagamento::da_tecla(8), Some(FormaDePagamento::Outro));
        assert_eq!(FormaDePagamento::da_tecla(0), None);
        assert_eq!(FormaDePagamento::da_tecla(9), None);
        for f in FormaDePagamento::TODAS {
            assert_eq!(FormaDePagamento::da_chave(f.chave()), Some(f));
        }
        assert_eq!(
            formas_juntas([FormaDePagamento::Pix, FormaDePagamento::Credito]),
            "PIX + Cartão de crédito"
        );
        assert_eq!(formas_juntas([]), "sem pagamento");
        assert_eq!(itens(1), "1 item");
        assert_eq!(itens(0), "0 itens");
    }
}
