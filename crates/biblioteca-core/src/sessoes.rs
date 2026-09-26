//! A lista de sessões fotográficas — situação, busca, contagens e o gráfico.
//!
//! # Por que isto é conta, e não tela
//!
//! A lista do painel (`/dashboard/sessoes-fotograficas`) e a do desktop
//! respondem à mesma pergunta com o mesmo operador na frente: *"cadê a galeria
//! da Maria?"*. Escritas duas vezes, elas divergem — e a divergência aqui não
//! aparece como erro, aparece como **a busca achando num app e não no outro**.
//!
//! Estas regras nasceram em TypeScript (`apresentacao.ts`, com 215 linhas de
//! teste ao lado) e foram portadas para cá em 6/set/2026, reescritas e não
//! copiadas, para que os dois lados passem a ler daqui.
//!
//! # A ordem da situação não é arbitrária
//!
//! É a ordem em que ela importa para quem opera: **vencida vence tudo** (não
//! adianta subir foto numa galeria que expirou), depois "sem fotos" (o próximo
//! passo é subir), e só então se o cliente já entrou ou ainda não.
//!
//! # Sem dependência nenhuma, inclusive de data
//!
//! Este crate compila para `wasm32` porque não puxa nada — e data seria a
//! primeira tentação. `expira_em` chega como **segundos desde a época** e a
//! comparação é aritmética; o agrupamento do gráfico lê a data ISO como texto e
//! conta os dias com o algoritmo civil, que cabe em dez linhas.

use std::collections::BTreeMap;

/// O que a lista diz de cada sessão, num olhar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Situacao {
    SemFotos,
    AguardandoCliente,
    AbertaPeloCliente,
    Vencida,
}

impl Situacao {
    /// Na ordem em que a barra de filtros as mostra.
    pub const TODAS: [Situacao; 4] = [
        Situacao::SemFotos,
        Situacao::AguardandoCliente,
        Situacao::AbertaPeloCliente,
        Situacao::Vencida,
    ];

    pub fn rotulo(self) -> &'static str {
        match self {
            Situacao::SemFotos => "Sem fotos",
            Situacao::AguardandoCliente => "Aguardando o cliente",
            Situacao::AbertaPeloCliente => "Cliente já abriu",
            Situacao::Vencida => "Vencida",
        }
    }

    /// O nome que atravessa a fronteira (JSON do site, wasm, teste).
    pub fn como_texto(self) -> &'static str {
        match self {
            Situacao::SemFotos => "sem_fotos",
            Situacao::AguardandoCliente => "aguardando_cliente",
            Situacao::AbertaPeloCliente => "aberta_pelo_cliente",
            Situacao::Vencida => "vencida",
        }
    }
}

/// Quantas fotos a sessão tem, por estado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContagemDeFotos {
    pub levadas_no_balcao: u32,
    pub disponiveis: u32,
    pub compradas: u32,
    pub apagadas: u32,
}

impl ContagemDeFotos {
    /// As que ainda existem. ⚠️ A apagada **não** conta: o arquivo sumiu pela
    /// retenção, e uma galeria só com apagadas não tem o que mostrar ao cliente.
    pub fn vivas(&self) -> u32 {
        self.levadas_no_balcao + self.disponiveis + self.compradas
    }
}

/// Quanto a sessão rendeu, por porta, em centavos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totais {
    pub balcao: i64,
    pub pos_venda: i64,
}

/// O que o **caixa do balcão** cobrou de uma sessão.
///
/// 🔑 **`vendas: 0` é um fato, e não um vazio**: a sessão não passou pelo
/// caixa, e é isso que faz a lista oferecer o caminho para fechar a venda.
/// Quem não sabe é o `Option` de fora — a API que não respondeu, ou que é
/// anterior ao campo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PagoNoCaixa {
    pub vendas: u32,
    /// Em centavos, como o resto do caixa — e ao contrário de [`Totais`], que
    /// chega da API em reais e é convertido na borda.
    pub bruto_centavos: i64,
    pub estornado_centavos: i64,
    /// `bruto − estornado`, calculado no servidor: a tela mostra, não recompõe.
    pub liquido_centavos: i64,
    /// 💳 O líquido por forma de pagamento, com a chave do site (`"pix"`).
    /// Soma o `liquido_centavos`. Ver [`rotulo_da_forma`].
    pub por_forma: BTreeMap<String, i64>,
}

/// O que a coluna "Caixa (PDV)" da lista tem a dizer sobre uma sessão.
///
/// 🚨 **Três dos quatro casos mostrariam `R$ 0,00`** se a coluna fosse só um
/// número, e eles pedem coisas opostas de quem opera: um manda perguntar de
/// novo, outro não tem nada a cobrar, o terceiro é uma venda esquecida e o
/// quarto já está fechado. É por isso que a decisão é um tipo, e não um `if`
/// dentro do desenho.
///
/// ⚠️ **A web tem esta mesma regra em TypeScript**
/// (`sessoes-fotograficas/caixa-na-lista.ts`), porque a lista de lá é React e
/// não chama este wasm. São duas escritas da mesma decisão, cada uma com o
/// teste que a prende — ao mudar uma, mudar a outra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoNoCaixa {
    /// A API não soube dizer (ou é anterior ao campo). Mostra `?`.
    NaoSei,
    /// Passou ou não pelo caixa, mas não há foto levada a cobrar. Mostra `—`.
    NadaACobrar,
    /// 💵 **Foto levada no balcão e nenhuma venda no PDV**: é a venda que
    /// ficou aberta, e a razão de a coluna existir.
    FecharVenda,
    /// Passou pelo caixa: o líquido, e quanto voltou em estorno.
    Cobrado {
        liquido_centavos: i64,
        estornado_centavos: i64,
    },
}

/// A decisão da coluna do caixa. Ver [`EstadoNoCaixa`].
pub fn estado_no_caixa(caixa: Option<&PagoNoCaixa>, levadas_no_balcao: u32) -> EstadoNoCaixa {
    let Some(pago) = caixa else {
        return EstadoNoCaixa::NaoSei;
    };
    if pago.vendas == 0 {
        // 🔑 Sem foto levada não há venda a fechar: oferecer o caixa em toda
        // sessão vazia encheria a coluna de convites que não levam a nada.
        return if levadas_no_balcao == 0 {
            EstadoNoCaixa::NadaACobrar
        } else {
            EstadoNoCaixa::FecharVenda
        };
    }
    EstadoNoCaixa::Cobrado {
        liquido_centavos: pago.liquido_centavos,
        estornado_centavos: pago.estornado_centavos,
    }
}

/// Uma sessão fotográfica como a lista precisa vê-la.
///
/// 🔑 O nome é `SessaoFotografica` e não `Sessao` de propósito: no desktop já
/// existe uma `Sessao` que é o **login** do operador, e as duas juntas num
/// mesmo `use` seriam duas coisas sem relação com o mesmo nome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessaoFotografica {
    pub id: String,
    pub titulo: String,
    pub email: Option<String>,
    pub whatsapp: Option<String>,
    /// `"2026-09-03"`, no fuso do estúdio — o carimbo do eixo do gráfico.
    pub criada_em_iso: String,
    /// Segundos desde a época. `None` = não expira.
    pub expira_em: Option<i64>,
    /// Preenchido quando o cliente já criou conta pelo link.
    pub user_id: Option<String>,
    pub fotos: ContagemDeFotos,
    /// `None` na sessão vinda de uma API anterior ao campo — e isso é dito na
    /// soma, em vez de virar zero calado.
    pub totais: Option<Totais>,
    /// 💵 **O que o caixa cobrou desta sessão** (dono, 20/set/2026). `None` =
    /// não se sabe; `Some` com `vendas: 0` = não passou pelo caixa, e a lista
    /// mostra o caminho para fechar a venda.
    pub caixa: Option<PagoNoCaixa>,
    /// 🏢 **A sessão não tem estúdio definido.**
    ///
    /// 🚨 **A grade precisa mostrar isso** (dono, 18/set/2026: *"tem sessões
    /// que foram criadas sem definição de estúdio… atrapalha até o fechamento
    /// de caixa"*): sem estúdio, a venda não entra no caixa de lugar nenhum, e
    /// a sessão só aparece em "Todos os estúdios" — onde ninguém a procura.
    pub sem_estudio: bool,
}

impl SessaoFotografica {
    /// A situação, na ordem em que ela importa para quem opera.
    pub fn situacao(&self, agora: i64) -> Situacao {
        if self.expira_em.is_some_and(|quando| quando < agora) {
            return Situacao::Vencida;
        }
        if self.fotos.vivas() == 0 {
            return Situacao::SemFotos;
        }
        if self.user_id.is_some() {
            return Situacao::AbertaPeloCliente;
        }
        Situacao::AguardandoCliente
    }
}

/// O que a busca procura.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Criterio {
    pub busca: String,
    /// `None` é "todas".
    pub situacao: Option<Situacao>,
    /// O período de criação, em datas `YYYY-MM-DD` do fuso do estúdio.
    /// `None` é "todo o período" — o arquivo inteiro.
    ///
    /// 🔑 **A lista abre em hoje** (dono, 2026-09-18: *"na listagem de sessões
    /// por padrão deve estar filtrado como hoje, mas com opção de selecionar o
    /// dia ou mesmo range de um período"*). O balcão trabalha o dia; a lista
    /// inteira é o arquivo.
    ///
    /// ⚠️ **Texto, e não data**: `YYYY-MM-DD` comparado como texto é comparação
    /// de data sem fuso, sem horário e sem biblioteca — e é o mesmo que o site
    /// faz (`periodo-da-lista.ts`). Quem converte para esse texto já lidou com
    /// o fuso.
    pub periodo: Option<FaixaDeDatas>,
}

/// Uma faixa de datas fechada, em `YYYY-MM-DD`.
///
/// ⚠️ **Não confundir com [`Periodo`]**, que é um degrau do gráfico: aquele é
/// "o que entrou no dia 3", este é "de 1 a 7".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FaixaDeDatas {
    pub de: String,
    pub ate: String,
}

impl FaixaDeDatas {
    /// Um dia só.
    pub fn no_dia(dia: impl Into<String>) -> Self {
        let dia = dia.into();
        Self {
            de: dia.clone(),
            ate: dia,
        }
    }

    /// Em ordem — quem escolheu o fim antes do começo quis o mesmo intervalo.
    pub fn em_ordem(&self) -> (&str, &str) {
        if self.de <= self.ate {
            (&self.de, &self.ate)
        } else {
            (&self.ate, &self.de)
        }
    }

    pub fn contem(&self, dia: &str) -> bool {
        let (de, ate) = self.em_ordem();
        dia >= de && dia <= ate
    }
}

/// O dia de uma data ISO — `2026-09-18T12:00:00Z` vira `2026-09-18`.
///
/// ⚠️ **Corta, não converte.** A API devolve a criação em UTC; o balcão é
/// GMT-3, e uma sessão criada às 21h de lá é do dia seguinte aqui. Isso é a
/// divergência que o site também tem (`criadaEmISO` sai de `paraDataISO`, que
/// usa o fuso) — e é por isso que este corte existe num lugar só, com nome: o
/// dia que sobe do corte é o que a coluna "Criada" já mostra.
pub(crate) fn dia_da_criacao(iso: &str) -> &str {
    iso.split('T').next().unwrap_or(iso)
}

/// Minúsculas e sem acento: "Joao" acha "João" e vice-versa.
///
/// ⚠️ **A tabela é do português, e é de propósito.** Normalizar Unicode inteiro
/// exigiria uma tabela de decomposição — dezenas de KB no `.wasm` que o
/// navegador baixa — para acertar alfabetos que este balcão não atende. O que
/// não estiver aqui passa direto, em minúsculas.
pub fn normalizar(texto: &str) -> String {
    texto
        .trim()
        .chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ý' | 'ÿ' => 'y',
            outro => outro,
        })
        .collect()
}

fn so_digitos(texto: &str) -> String {
    texto.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Quantos dígitos a busca precisa ter para valer como telefone.
///
/// Menos que isto casaria com quase todo número da lista, e o operador que
/// digita "12" procurando um título veria as galerias erradas aparecerem.
const DIGITOS_PARA_VALER_TELEFONE: usize = 3;

/// Os dígitos do telefone buscado, sem o `55` do Brasil na frente.
///
/// Quem cola `+55 (47) 99999-8888` procura o mesmo cliente que foi salvo como
/// `(47) 99999-8888`; com o DDI, os dígitos não caberiam dentro do número salvo.
/// Só corta com 12 dígitos ou mais — DDI + DDD + número —, para `5599…` (DDD 55)
/// continuar sendo DDD.
fn digitos_do_telefone(busca: &str) -> String {
    let digitos = so_digitos(busca);
    match digitos.strip_prefix("55") {
        Some(resto) if digitos.len() >= 12 => resto.to_string(),
        _ => digitos,
    }
}

/// Busca por título, e-mail ou telefone, e filtro por situação.
///
/// O telefone compara **só dígitos**: quem digita `99999` acha
/// `(47) 99999-8888` — a máscara é apresentação, não dado.
///
/// 🔎 **Com texto na busca, o período não recorta** (dono, 2026-09-26: *"tinha
/// que ter uma busca ativa por título, email e telefone"*). A lista abre em
/// hoje, e o cliente que volta ao balcão quase nunca é de hoje: buscar só dentro
/// do dia respondia "nada" para quem existe. A busca procura no arquivo inteiro,
/// e apagar o texto devolve o período escolhido. A situação continua valendo —
/// é um chip que o operador acendeu de propósito.
///
/// ⚠️ **O site tem a mesma regra** (`sessoes-fotograficas/lista.tsx`,
/// `apresentacao.ts`): ao mudar uma, mudar a outra.
pub fn filtrar<'a>(
    sessoes: &'a [SessaoFotografica],
    criterio: &Criterio,
    agora: i64,
) -> Vec<&'a SessaoFotografica> {
    let termo = normalizar(&criterio.busca);
    let digitos = digitos_do_telefone(&criterio.busca);

    sessoes
        .iter()
        .filter(|sessao| {
            // 📅 O período é o recorte mais grosso, e vem primeiro — menos
            // quando há busca, que procura em todas as datas.
            if termo.is_empty()
                && criterio
                    .periodo
                    .as_ref()
                    .is_some_and(|p| !p.contem(dia_da_criacao(&sessao.criada_em_iso)))
            {
                return false;
            }
            if criterio
                .situacao
                .is_some_and(|querida| sessao.situacao(agora) != querida)
            {
                return false;
            }
            if termo.is_empty() {
                return true;
            }
            if normalizar(&sessao.titulo).contains(&termo) {
                return true;
            }
            if sessao
                .email
                .as_deref()
                .is_some_and(|email| normalizar(email).contains(&termo))
            {
                return true;
            }
            digitos.len() >= DIGITOS_PARA_VALER_TELEFONE
                && sessao
                    .whatsapp
                    .as_deref()
                    .is_some_and(|zap| so_digitos(zap).contains(&digitos))
        })
        .collect()
}

/// Quantas sessões há em cada situação, para os contadores dos filtros.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Contagens {
    pub todas: usize,
    pub sem_fotos: usize,
    pub aguardando_cliente: usize,
    pub aberta_pelo_cliente: usize,
    pub vencida: usize,
}

impl Contagens {
    pub fn de(&self, situacao: Situacao) -> usize {
        match situacao {
            Situacao::SemFotos => self.sem_fotos,
            Situacao::AguardandoCliente => self.aguardando_cliente,
            Situacao::AbertaPeloCliente => self.aberta_pelo_cliente,
            Situacao::Vencida => self.vencida,
        }
    }
}

pub fn contar_por_situacao(sessoes: &[SessaoFotografica], agora: i64) -> Contagens {
    let mut contagens = Contagens {
        todas: sessoes.len(),
        ..Contagens::default()
    };
    for sessao in sessoes {
        match sessao.situacao(agora) {
            Situacao::SemFotos => contagens.sem_fotos += 1,
            Situacao::AguardandoCliente => contagens.aguardando_cliente += 1,
            Situacao::AbertaPeloCliente => contagens.aberta_pelo_cliente += 1,
            Situacao::Vencida => contagens.vencida += 1,
        }
    }
    contagens
}

/// A soma do **recorte visível**, e o que ela não pôde somar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Soma {
    pub balcao: i64,
    pub pos_venda: i64,
    /// Quantas sessões entraram sem totais — de uma API anterior ao campo.
    ///
    /// 🔑 **Contadas, e não silenciadas.** Sem isto o rodapé anunciaria um total
    /// menor que o real sem nada dizendo por quê.
    pub sem_totais: usize,
    /// 💵 O que o **caixa (PDV)** registrou no recorte — o cartão "Vendido no
    /// caixa".
    pub caixa: SomaDoCaixa,
}

/// O caixa do recorte: o que o PDV cobrou, o que voltou, e por qual forma.
///
/// 🔑 **É o número do caixa, e não o do preço das fotos** (dono, 2026-09-26:
/// *"tinha que ser o total vendido com as informações do caixa"*). O `balcao`
/// de [`Soma`] conta a foto levada pelo preço dela, tenha ou não passado pela
/// gaveta; este é o dinheiro que o caixa registrou.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SomaDoCaixa {
    pub vendas: u32,
    pub bruto_centavos: i64,
    pub estornado_centavos: i64,
    pub liquido_centavos: i64,
    /// As "moedas": o líquido por forma de pagamento, na ordem das teclas do
    /// PDV. Só as que ficaram com valor.
    pub por_forma: Vec<(String, i64)>,
    /// Sessões com foto levada no balcão e nenhuma venda no PDV — dinheiro
    /// que saiu do balcão sem passar pela gaveta.
    pub a_fechar: usize,
    /// Sessões de que a API não soube dizer o caixa. Como o `sem_totais`:
    /// contadas, para o número não se apresentar como inteiro.
    pub sem_caixa: usize,
}

/// As formas de pagamento do caixa do site, na ordem das teclas do PDV (1 a
/// 8), com o nome que o operador lê — os mesmos de `ROTULO_DA_FORMA`, no
/// `lib/schemas/caixa.ts` do site.
///
/// ⚠️ **A lista é do site** (`domain::pos_venda::caixa::FormaDePagamento`, no
/// e-commerce). Uma forma que chegue fora dela vai para o fim, com a chave
/// como veio — some da tela é que ela não pode.
pub const FORMAS_DE_PAGAMENTO: [(&str, &str); 8] = [
    ("dinheiro", "Dinheiro"),
    ("pix", "PIX"),
    ("debito", "Cartão de débito"),
    ("credito", "Cartão de crédito"),
    ("voucher", "Voucher"),
    ("parceiro", "Site parceiro"),
    ("transferencia", "Transferência"),
    ("outro", "Outro"),
];

/// O nome de uma forma de pagamento para a tela.
pub fn rotulo_da_forma(chave: &str) -> &str {
    FORMAS_DE_PAGAMENTO
        .iter()
        .find(|(k, _)| *k == chave)
        .map(|(_, rotulo)| *rotulo)
        .unwrap_or(chave)
}

/// ⚠️ **É a soma do que está na tela.** Com busca ou filtro ativo o número é do
/// recorte, e a tela precisa dizer isso: número parcial que se apresenta como
/// total é a mesma armadilha que o contador da grade evita do outro lado.
pub fn somar_totais(sessoes: &[&SessaoFotografica]) -> Soma {
    let mut soma = Soma::default();
    let mut formas: BTreeMap<String, i64> = BTreeMap::new();
    for sessao in sessoes {
        match sessao.totais {
            Some(totais) => {
                soma.balcao += totais.balcao;
                soma.pos_venda += totais.pos_venda;
            }
            None => soma.sem_totais += 1,
        }
        match estado_no_caixa(sessao.caixa.as_ref(), sessao.fotos.levadas_no_balcao) {
            EstadoNoCaixa::NaoSei => soma.caixa.sem_caixa += 1,
            EstadoNoCaixa::FecharVenda => soma.caixa.a_fechar += 1,
            EstadoNoCaixa::NadaACobrar | EstadoNoCaixa::Cobrado { .. } => {}
        }
        if let Some(pago) = &sessao.caixa {
            soma.caixa.vendas += pago.vendas;
            soma.caixa.bruto_centavos += pago.bruto_centavos;
            soma.caixa.estornado_centavos += pago.estornado_centavos;
            soma.caixa.liquido_centavos += pago.liquido_centavos;
            for (forma, valor) in &pago.por_forma {
                *formas.entry(forma.clone()).or_insert(0) += valor;
            }
        }
    }
    formas.retain(|_, valor| *valor != 0);
    // Na ordem das teclas do PDV; a forma desconhecida vai para o fim.
    let posicao = |chave: &str| {
        FORMAS_DE_PAGAMENTO
            .iter()
            .position(|(k, _)| *k == chave)
            .unwrap_or(FORMAS_DE_PAGAMENTO.len())
    };
    let mut por_forma: Vec<(String, i64)> = formas.into_iter().collect();
    por_forma.sort_by_key(|(chave, _)| posicao(chave));
    soma.caixa.por_forma = por_forma;
    soma
}

/// Um degrau do gráfico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Periodo {
    /// `"2026-09-03"` por dia, `"2026-09"` por mês.
    pub chave: String,
    pub balcao: i64,
    pub pos_venda: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Passo {
    Dia,
    Mes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serie {
    pub periodos: Vec<Periodo>,
    pub passo: Passo,
}

/// Acima disto o eixo por dia vira uma tarja; abaixo, o mês esconde a forma.
const DIAS_ATE_AGRUPAR_POR_MES: i64 = 45;

/// O que entrou por período — **pelo dia em que a sessão foi criada**, que é o
/// único carimbo de tempo que a lista tem.
///
/// ⚠️ **Não é faturamento por data de pagamento.** Uma foto comprada hoje numa
/// galeria de dois meses atrás entra no degrau de dois meses atrás. É a leitura
/// honesta do que a lista sabe, e a tela tem de dizer isso: chamar de "vendas do
/// mês" seria um número que responde outra pergunta.
///
/// 🔑 **Período sem sessão nenhuma não vira degrau vazio.** O eixo é categórico
/// — o que aconteceu —, e inventar zeros para dias sem atendimento desenharia
/// uma queda que não houve.
pub fn agrupar_por_periodo(sessoes: &[&SessaoFotografica]) -> Serie {
    if sessoes.is_empty() {
        return Serie {
            periodos: Vec::new(),
            passo: Passo::Dia,
        };
    }

    let mut dias: Vec<&str> = sessoes.iter().map(|s| s.criada_em_iso.as_str()).collect();
    dias.sort_unstable();
    let vao = match (dia_civil(dias[0]), dia_civil(dias[dias.len() - 1])) {
        (Some(primeiro), Some(ultimo)) => ultimo - primeiro,
        // Data ilegível não decide o passo: por dia é o que preserva a forma.
        _ => 0,
    };
    let passo = if vao > DIAS_ATE_AGRUPAR_POR_MES {
        Passo::Mes
    } else {
        Passo::Dia
    };

    // `BTreeMap` porque a saída é ordenada por chave, e a chave ISO ordena como
    // texto exatamente como ordena no tempo.
    let mut por_chave: BTreeMap<String, Periodo> = BTreeMap::new();
    for sessao in sessoes {
        let chave = match passo {
            Passo::Mes => sessao
                .criada_em_iso
                .get(..7)
                .unwrap_or_default()
                .to_string(),
            Passo::Dia => sessao.criada_em_iso.clone(),
        };
        let degrau = por_chave.entry(chave.clone()).or_insert(Periodo {
            chave,
            balcao: 0,
            pos_venda: 0,
        });
        if let Some(totais) = sessao.totais {
            degrau.balcao += totais.balcao;
            degrau.pos_venda += totais.pos_venda;
        }
    }

    Serie {
        periodos: por_chave.into_values().collect(),
        passo,
    }
}

/// O número do dia desde uma época qualquer, a partir de `"AAAA-MM-DD"`.
///
/// 🔑 É o algoritmo civil de Howard Hinnant, e ele está aqui por uma razão só:
/// medir o **vão** entre duas datas sem trazer um crate de calendário para um
/// crate que precisa continuar sem dependência nenhuma. Não faz fuso, não faz
/// hora, não valida mês 13 — mede distância entre dois dias.
fn dia_civil(iso: &str) -> Option<i64> {
    let mut partes = iso.split('-');
    let ano: i64 = partes.next()?.parse().ok()?;
    let mes: i64 = partes.next()?.parse().ok()?;
    let dia: i64 = partes.next()?.get(..2)?.parse().ok()?;

    let ano = if mes <= 2 { ano - 1 } else { ano };
    let era = if ano >= 0 { ano } else { ano - 399 } / 400;
    let ano_da_era = ano - era * 400;
    let dia_do_ano = (153 * (if mes > 2 { mes - 3 } else { mes + 9 }) + 2) / 5 + dia - 1;
    let dia_da_era = ano_da_era * 365 + ano_da_era / 4 - ano_da_era / 100 + dia_do_ano;
    Some(era * 146_097 + dia_da_era - 719_468)
}

/// O inverso de [`dia_civil`]: dias desde 1970-01-01 → `"YYYY-MM-DD"`.
fn data_do_dia(dias: i64) -> String {
    let z = dias + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let dia_da_era = z - era * 146_097;
    let ano_da_era =
        (dia_da_era - dia_da_era / 1460 + dia_da_era / 36_524 - dia_da_era / 146_096) / 365;
    let dia_do_ano = dia_da_era - (365 * ano_da_era + ano_da_era / 4 - ano_da_era / 100);
    let mp = (5 * dia_do_ano + 2) / 153;
    let dia = dia_do_ano - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = ano_da_era + era * 400 + i64::from(mes <= 2);
    format!("{ano:04}-{mes:02}-{dia:02}")
}

/// Quantos dias o gráfico de área mostra.
pub const DIAS_DO_GRAFICO: i64 = 30;

/// Um dia do gráfico de área: o que o caixa registrou e o que o pós-venda
/// vendeu nas sessões criadas nele.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiaDoGrafico {
    /// `"2026-09-03"`.
    pub chave: String,
    /// O líquido do caixa (PDV), em centavos.
    pub caixa: i64,
    /// O pós-venda, em centavos.
    pub pos_venda: i64,
}

/// 📈 **Os últimos dias até `fim`, um a um** — o gráfico de área da lista
/// (dono, 2026-09-26: *"nessa tela de sessões coloque gráficos"*).
///
/// # Por que uma janela, e não o recorte
///
/// A lista abre em hoje: um gráfico do recorte seria um ponto só, e área de um
/// ponto não desenha nada. A janela termina no fim do período escolhido (ou
/// hoje) e olha para trás; **a busca, a situação e os filtros de coluna
/// valem**, só o período não — quem chama passa as sessões já filtradas sem
/// ele.
///
/// # Aqui o dia vazio é zero
///
/// Ao contrário de [`agrupar_por_periodo`], o eixo é o **calendário**: a
/// janela tem sempre os mesmos dias, e um dia sem sessão é um dia em que não
/// entrou nada — zero é o fato, e não uma queda inventada.
///
/// ⚠️ **Pelo dia em que a sessão foi criada**, como o resto da lista. O site
/// faz a mesma conta (`serie-dos-graficos.ts`, `ultimosDias`).
pub fn ultimos_dias(sessoes: &[&SessaoFotografica], fim: &str, dias: i64) -> Vec<DiaDoGrafico> {
    let Some(ultimo) = dia_civil(fim) else {
        return Vec::new();
    };
    let primeiro = ultimo - (dias.max(1) - 1);
    let mut serie: Vec<DiaDoGrafico> = (primeiro..=ultimo)
        .map(|d| DiaDoGrafico {
            chave: data_do_dia(d),
            caixa: 0,
            pos_venda: 0,
        })
        .collect();
    for sessao in sessoes {
        let Some(d) = dia_civil(dia_da_criacao(&sessao.criada_em_iso)) else {
            continue;
        };
        if d < primeiro || d > ultimo {
            continue;
        }
        let dia = &mut serie[(d - primeiro) as usize];
        if let Some(pago) = &sessao.caixa {
            dia.caixa += pago.liquido_centavos;
        }
        if let Some(totais) = sessao.totais {
            dia.pos_venda += totais.pos_venda;
        }
    }
    serie
}

/// `"2026-09-03"` → `"03/09"`, o rótulo do eixo.
pub fn dia_curto(chave: &str) -> String {
    let mut partes = chave.split('-').skip(1);
    match (partes.next(), partes.next()) {
        (Some(mes), Some(dia)) => format!("{dia}/{mes}"),
        _ => chave.to_string(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 📈 Trinta dias até o fim, o dia vazio em zero, e a sessão fora da
    /// janela não entra.
    #[test]
    fn os_ultimos_dias_cobrem_a_janela_inteira() {
        let pago = |liquido: i64| PagoNoCaixa {
            vendas: 1,
            bruto_centavos: liquido,
            liquido_centavos: liquido,
            ..PagoNoCaixa::default()
        };
        let em = |dia: &str, caixa: i64, pos_venda: i64| SessaoFotografica {
            criada_em_iso: dia.into(),
            caixa: Some(pago(caixa)),
            totais: Some(Totais {
                balcao: 0,
                pos_venda,
            }),
            ..sessao(dia)
        };
        let sessoes = [
            em("2026-09-01T10:00:00Z", 4_000, 500),
            em("2026-09-01", 1_000, 0),
            em("2026-08-02", 9_999, 0),
            em("2026-09-03", 0, 700),
        ];
        let refs: Vec<&SessaoFotografica> = sessoes.iter().collect();

        let serie = ultimos_dias(&refs, "2026-09-03", DIAS_DO_GRAFICO);

        assert_eq!(serie.len(), 30);
        assert_eq!(serie[0].chave, "2026-08-05", "atravessa o mês");
        assert_eq!(serie[29].chave, "2026-09-03");
        assert_eq!(serie[27].caixa, 5_000, "duas sessões do mesmo dia somam");
        assert_eq!(serie[27].pos_venda, 500);
        assert_eq!(
            serie[28],
            DiaDoGrafico {
                chave: "2026-09-02".into(),
                caixa: 0,
                pos_venda: 0,
            }
        );
        assert_eq!(serie[29].pos_venda, 700);
        let total: i64 = serie.iter().map(|d| d.caixa).sum();
        assert_eq!(total, 5_000, "a de agosto ficou fora da janela");
        assert_eq!(data_do_dia(dia_civil("2024-02-29").unwrap()), "2024-02-29");
        assert_eq!(dia_curto("2026-09-03"), "03/09");
    }

    /// 12h de 3/set/2026, em segundos — o "agora" de todos os testes.
    const AGORA: i64 = 1_788_609_600;
    const UM_DIA: i64 = 86_400;

    fn sessao(titulo: &str) -> SessaoFotografica {
        SessaoFotografica {
            sem_estudio: false,
            id: titulo.to_string(),
            titulo: titulo.to_string(),
            email: None,
            whatsapp: None,
            criada_em_iso: "2026-09-03".into(),
            expira_em: None,
            user_id: None,
            fotos: ContagemDeFotos {
                disponiveis: 1,
                ..ContagemDeFotos::default()
            },
            totais: None,
            caixa: None,
        }
    }

    /// 💵 A coluna do caixa: os quatro estados, e os três que mostrariam zero.
    #[test]
    fn a_coluna_do_caixa_separa_o_que_falta_cobrar_do_que_ja_foi() {
        // Sem resposta da API não é "não foi cobrada".
        assert_eq!(estado_no_caixa(None, 3), EstadoNoCaixa::NaoSei);

        // Não passou pelo caixa, com foto levada: é a venda esquecida.
        let sem_venda = PagoNoCaixa::default();
        assert_eq!(
            estado_no_caixa(Some(&sem_venda), 3),
            EstadoNoCaixa::FecharVenda
        );
        // Sem foto levada não há o que cobrar — e o convite não aparece.
        assert_eq!(
            estado_no_caixa(Some(&sem_venda), 0),
            EstadoNoCaixa::NadaACobrar
        );

        // 🚨 Vendida e estornada por inteiro sobra zero, e **não** volta a ser
        // "fechar venda": ela já passou pelo caixa.
        let estornada = PagoNoCaixa {
            vendas: 1,
            bruto_centavos: 9_000,
            estornado_centavos: 9_000,
            liquido_centavos: 0,
            ..PagoNoCaixa::default()
        };
        assert_eq!(
            estado_no_caixa(Some(&estornada), 2),
            EstadoNoCaixa::Cobrado {
                liquido_centavos: 0,
                estornado_centavos: 9_000
            }
        );
    }

    /// 💳 O cartão do caixa soma o PDV do recorte: o líquido, as formas na
    /// ordem das teclas, a venda por fechar e a sessão de que não se sabe.
    #[test]
    fn a_soma_do_caixa_junta_as_formas_na_ordem_do_pdv() {
        let pago = |liquido: i64, formas: &[(&str, i64)]| PagoNoCaixa {
            vendas: 1,
            bruto_centavos: liquido,
            estornado_centavos: 0,
            liquido_centavos: liquido,
            por_forma: formas.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        };
        let com = |titulo: &str, caixa: Option<PagoNoCaixa>, levadas: u32| SessaoFotografica {
            caixa,
            fotos: ContagemDeFotos {
                levadas_no_balcao: levadas,
                ..ContagemDeFotos::default()
            },
            ..sessao(titulo)
        };
        let sessoes = [
            com(
                "a",
                Some(pago(5_000, &[("pix", 3_000), ("dinheiro", 2_000)])),
                2,
            ),
            com(
                "b",
                Some(pago(4_000, &[("credito", 1_000), ("dinheiro", 3_000)])),
                2,
            ),
            com("c", Some(PagoNoCaixa::default()), 3),
            com("d", None, 1),
        ];
        let refs: Vec<&SessaoFotografica> = sessoes.iter().collect();

        let caixa = somar_totais(&refs).caixa;

        assert_eq!(caixa.liquido_centavos, 9_000);
        assert_eq!(caixa.vendas, 2);
        assert_eq!(
            caixa.por_forma,
            vec![
                ("dinheiro".to_string(), 5_000),
                ("pix".to_string(), 3_000),
                ("credito".to_string(), 1_000),
            ]
        );
        assert_eq!(
            caixa.por_forma.iter().map(|(_, v)| v).sum::<i64>(),
            caixa.liquido_centavos,
            "as formas fecham com o líquido"
        );
        assert_eq!(caixa.a_fechar, 1, "a sessão com foto levada e sem venda");
        assert_eq!(caixa.sem_caixa, 1);
        assert_eq!(rotulo_da_forma("credito"), "Cartão de crédito");
        assert_eq!(
            rotulo_da_forma("cripto"),
            "cripto",
            "a desconhecida aparece como veio"
        );
    }

    /// 🚨 A ordem da situação é a ordem em que ela importa para quem opera.
    #[test]
    fn vencida_vence_tudo_e_sem_foto_vem_antes_do_cliente() {
        let com_cliente_e_sem_foto = SessaoFotografica {
            user_id: Some("u1".into()),
            fotos: ContagemDeFotos::default(),
            ..sessao("a")
        };
        assert_eq!(
            com_cliente_e_sem_foto.situacao(AGORA),
            Situacao::SemFotos,
            "sem foto viva não há o que o cliente tenha aberto"
        );

        assert_eq!(sessao("b").situacao(AGORA), Situacao::AguardandoCliente);

        let aberta = SessaoFotografica {
            user_id: Some("u1".into()),
            ..sessao("c")
        };
        assert_eq!(aberta.situacao(AGORA), Situacao::AbertaPeloCliente);

        let vencida = SessaoFotografica {
            expira_em: Some(AGORA - UM_DIA),
            user_id: Some("u1".into()),
            ..sessao("d")
        };
        assert_eq!(
            vencida.situacao(AGORA),
            Situacao::Vencida,
            "não adianta subir foto nem esperar o cliente"
        );

        let no_prazo = SessaoFotografica {
            expira_em: Some(AGORA + UM_DIA),
            ..sessao("e")
        };
        assert_eq!(no_prazo.situacao(AGORA), Situacao::AguardandoCliente);
    }

    /// ⚠️ A apagada não conta como viva: o arquivo sumiu pela retenção.
    #[test]
    fn so_apagadas_e_o_mesmo_que_sem_fotos() {
        let so_apagadas = SessaoFotografica {
            fotos: ContagemDeFotos {
                apagadas: 12,
                ..ContagemDeFotos::default()
            },
            ..sessao("a")
        };
        assert_eq!(so_apagadas.situacao(AGORA), Situacao::SemFotos);
    }

    #[test]
    fn normalizar_tira_acento_e_caixa() {
        assert_eq!(normalizar("  JOÃO Gonçalves "), "joao goncalves");
        assert_eq!(normalizar("Ângela"), "angela");
        assert_eq!(normalizar("Müller"), "muller");
    }

    #[test]
    fn a_busca_acha_por_titulo_email_e_telefone() {
        let sessoes = vec![
            SessaoFotografica {
                email: Some("joao@exemplo.com".into()),
                whatsapp: Some("(47) 99999-8888".into()),
                ..sessao("Ensaio do João")
            },
            SessaoFotografica {
                email: Some("maria@outro.com".into()),
                ..sessao("Casamento da Maria")
            },
        ];

        let todas = filtrar(&sessoes, &Criterio::default(), AGORA);
        assert_eq!(todas.len(), 2, "sem critério devolve tudo");

        let por_titulo = filtrar(
            &sessoes,
            &Criterio {
                busca: "joao".into(),
                situacao: None,
                ..Default::default()
            },
            AGORA,
        );
        assert_eq!(por_titulo.len(), 1, "'joao' acha 'João'");
        assert_eq!(por_titulo[0].titulo, "Ensaio do João");

        let por_email = filtrar(
            &sessoes,
            &Criterio {
                busca: "outro.com".into(),
                situacao: None,
                ..Default::default()
            },
            AGORA,
        );
        assert_eq!(por_email.len(), 1);

        let por_telefone = filtrar(
            &sessoes,
            &Criterio {
                busca: "99999".into(),
                situacao: None,
                ..Default::default()
            },
            AGORA,
        );
        assert_eq!(
            por_telefone.len(),
            1,
            "a máscara é apresentação: compara só dígitos"
        );
    }

    /// 🚨 Dois dígitos soltos não acham telefone — seriam todos.
    #[test]
    fn poucos_digitos_nao_valem_como_telefone() {
        let sessoes = vec![SessaoFotografica {
            whatsapp: Some("(47) 99999-8888".into()),
            ..sessao("Ensaio")
        }];
        let achadas = filtrar(
            &sessoes,
            &Criterio {
                busca: "47".into(),
                situacao: None,
                ..Default::default()
            },
            AGORA,
        );
        assert!(achadas.is_empty(), "'47' não pode achar todo mundo");
    }

    #[test]
    fn o_filtro_de_situacao_corta_antes_da_busca() {
        let sessoes = vec![
            sessao("Ensaio do João"),
            SessaoFotografica {
                fotos: ContagemDeFotos::default(),
                ..sessao("Ensaio da Joana")
            },
        ];
        let achadas = filtrar(
            &sessoes,
            &Criterio {
                busca: "ensaio".into(),
                situacao: Some(Situacao::SemFotos),
                ..Default::default()
            },
            AGORA,
        );
        assert_eq!(achadas.len(), 1);
        assert_eq!(achadas[0].titulo, "Ensaio da Joana");
    }

    #[test]
    fn as_contagens_batem_com_o_total() {
        let sessoes = vec![
            sessao("a"),
            SessaoFotografica {
                fotos: ContagemDeFotos::default(),
                ..sessao("b")
            },
            SessaoFotografica {
                user_id: Some("u".into()),
                ..sessao("c")
            },
            SessaoFotografica {
                expira_em: Some(AGORA - UM_DIA),
                ..sessao("d")
            },
        ];
        let contagens = contar_por_situacao(&sessoes, AGORA);
        assert_eq!(contagens.todas, 4);
        assert_eq!(contagens.de(Situacao::AguardandoCliente), 1);
        assert_eq!(contagens.de(Situacao::SemFotos), 1);
        assert_eq!(contagens.de(Situacao::AbertaPeloCliente), 1);
        assert_eq!(contagens.de(Situacao::Vencida), 1);
    }

    /// 🔑 A sessão sem totais é **contada à parte**, e não somada como zero.
    #[test]
    fn somar_separa_as_portas_e_conta_quem_nao_tem_totais() {
        let com = SessaoFotografica {
            totais: Some(Totais {
                balcao: 5_000,
                pos_venda: 1_990,
            }),
            ..sessao("a")
        };
        let sem = sessao("b");
        let lista = vec![&com, &sem];

        let soma = somar_totais(&lista);
        assert_eq!(soma.balcao, 5_000);
        assert_eq!(soma.pos_venda, 1_990);
        assert_eq!(soma.sem_totais, 1);

        assert_eq!(somar_totais(&[]), Soma::default(), "lista vazia é zero");
    }

    #[test]
    fn o_grafico_agrupa_por_dia_quando_a_historia_e_curta() {
        let a = SessaoFotografica {
            criada_em_iso: "2026-09-01".into(),
            totais: Some(Totais {
                balcao: 100,
                pos_venda: 0,
            }),
            ..sessao("a")
        };
        let b = SessaoFotografica {
            criada_em_iso: "2026-09-01".into(),
            totais: Some(Totais {
                balcao: 50,
                pos_venda: 20,
            }),
            ..sessao("b")
        };
        let c = SessaoFotografica {
            criada_em_iso: "2026-09-05".into(),
            totais: Some(Totais {
                balcao: 0,
                pos_venda: 7,
            }),
            ..sessao("c")
        };

        // Fora de ordem de propósito: o degrau sai cronológico assim mesmo.
        let serie = agrupar_por_periodo(&[&c, &a, &b]);
        assert_eq!(serie.passo, Passo::Dia);
        assert_eq!(serie.periodos.len(), 2, "dia sem atendimento não vira zero");
        assert_eq!(serie.periodos[0].chave, "2026-09-01");
        assert_eq!(serie.periodos[0].balcao, 150);
        assert_eq!(serie.periodos[0].pos_venda, 20);
        assert_eq!(serie.periodos[1].chave, "2026-09-05");
    }

    /// Com mais de 45 dias de história o eixo por dia vira tarja.
    #[test]
    fn o_grafico_passa_a_agrupar_por_mes_quando_o_intervalo_cresce() {
        let antiga = SessaoFotografica {
            criada_em_iso: "2026-06-10".into(),
            ..sessao("antiga")
        };
        let nova = SessaoFotografica {
            criada_em_iso: "2026-09-05".into(),
            ..sessao("nova")
        };
        let serie = agrupar_por_periodo(&[&antiga, &nova]);
        assert_eq!(serie.passo, Passo::Mes);
        assert_eq!(serie.periodos[0].chave, "2026-06");
        assert_eq!(serie.periodos[1].chave, "2026-09");
    }

    #[test]
    fn sem_sessao_nenhuma_o_grafico_nao_tem_degrau() {
        let serie = agrupar_por_periodo(&[]);
        assert!(serie.periodos.is_empty());
        assert_eq!(serie.passo, Passo::Dia);
    }

    /// O algoritmo civil mede o vão, e é só para isso que ele está aqui.
    #[test]
    fn o_dia_civil_mede_a_distancia_entre_datas() {
        let um = dia_civil("2026-09-01").expect("data boa");
        let outro = dia_civil("2026-09-05").expect("data boa");
        assert_eq!(outro - um, 4);

        // Ano bissexto, e a virada de fevereiro.
        let fim_de_fevereiro = dia_civil("2024-02-28").expect("data boa");
        let primeiro_de_marco = dia_civil("2024-03-01").expect("data boa");
        assert_eq!(primeiro_de_marco - fim_de_fevereiro, 2, "2024 tem 29/02");

        assert_eq!(dia_civil("nada disso"), None);
    }
}

#[cfg(test)]
mod testes_do_periodo {
    use super::*;

    fn sessao(id: &str, criada_em_iso: &str) -> SessaoFotografica {
        SessaoFotografica {
            sem_estudio: false,
            id: id.into(),
            titulo: format!("Ensaio {id}"),
            email: None,
            whatsapp: None,
            criada_em_iso: criada_em_iso.into(),
            expira_em: None,
            user_id: None,
            fotos: ContagemDeFotos::default(),
            totais: None,
            caixa: None,
        }
    }

    /// 📅 **A lista abre em hoje** e a faixa recorta pelas pontas, inclusive.
    #[test]
    fn o_periodo_recorta_pela_data_de_criacao() {
        let sessoes = vec![
            sessao("a", "2026-09-17T14:00:00Z"),
            sessao("b", "2026-09-18T09:30:00Z"),
            sessao("c", "2026-09-19T23:59:00Z"),
        ];
        let com = |de: &str, ate: &str| Criterio {
            periodo: Some(FaixaDeDatas {
                de: de.into(),
                ate: ate.into(),
            }),
            ..Default::default()
        };

        let hoje: Vec<&str> = filtrar(&sessoes, &com("2026-09-18", "2026-09-18"), 0)
            .iter()
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(hoje, vec!["b"], "um dia só");

        let faixa: Vec<&str> = filtrar(&sessoes, &com("2026-09-17", "2026-09-19"), 0)
            .iter()
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(faixa, vec!["a", "b", "c"], "as pontas entram");

        // Sem período, o arquivo inteiro.
        assert_eq!(filtrar(&sessoes, &Criterio::default(), 0).len(), 3);
    }

    /// A faixa invertida é a mesma faixa — quem clicou no fim antes quis isso.
    #[test]
    fn a_faixa_invertida_vale_igual() {
        let faixa = FaixaDeDatas {
            de: "2026-09-20".into(),
            ate: "2026-09-17".into(),
        };

        assert!(faixa.contem("2026-09-18"));
        assert_eq!(faixa.em_ordem(), ("2026-09-17", "2026-09-20"));
        assert!(!faixa.contem("2026-09-21"));
    }

    /// 🔎 **A busca procura em todas as datas**: com a lista em hoje, o
    /// cliente de ontem aparece por título, e-mail ou telefone — e apagar a
    /// busca devolve o dia.
    #[test]
    fn a_busca_procura_fora_do_periodo() {
        let mut ontem = sessao("a", "2026-09-17T14:00:00Z");
        ontem.titulo = "Leslye e Ricardo".into();
        ontem.email = Some("leslye@yahoo.com".into());
        ontem.whatsapp = Some("(51) 99876-5432".into());
        let sessoes = vec![ontem, sessao("b", "2026-09-18T09:30:00Z")];
        let buscando = |busca: &str| Criterio {
            busca: busca.into(),
            periodo: Some(FaixaDeDatas::no_dia("2026-09-18")),
            ..Default::default()
        };
        let achadas = |busca: &str| -> Vec<String> {
            filtrar(&sessoes, &buscando(busca), 0)
                .iter()
                .map(|s| s.id.clone())
                .collect()
        };

        assert_eq!(achadas("ricardo"), vec!["a"], "pelo título");
        assert_eq!(achadas("LESLYE@"), vec!["a"], "pelo e-mail");
        assert_eq!(achadas("98765"), vec!["a"], "pelo telefone");
        assert_eq!(
            achadas("+55 51 99876-5432"),
            vec!["a"],
            "com o DDI na frente"
        );
        assert_eq!(achadas(""), vec!["b"], "sem busca, o dia volta");
    }

    /// 🚨 A hora não entra na conta: o que se compara é o dia.
    #[test]
    fn a_hora_da_criacao_nao_entra_na_conta() {
        let sessoes = vec![sessao("a", "2026-09-18T23:59:59Z")];
        let criterio = Criterio {
            periodo: Some(FaixaDeDatas::no_dia("2026-09-18")),
            ..Default::default()
        };

        assert_eq!(filtrar(&sessoes, &criterio, 0).len(), 1);
    }
}
