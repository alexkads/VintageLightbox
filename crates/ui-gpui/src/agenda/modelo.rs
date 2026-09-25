//! O modelo da agenda — as regras do `/dashboard/agendamentos` do site, em
//! funções puras.
//!
//! | Aqui | No site |
//! |---|---|
//! | [`Visao`], [`periodo_visivel`], [`inicio_da_estacao`] | `periodo.ts` |
//! | [`StatusDoEnsaio`] | `calendario/evento.ts` (`APARENCIA`, `contaNoDia`) |
//! | [`nome_da_origem`], [`origem_no_aviso`] | `lib/origem.ts` |
//! | [`EventoDaAgenda`] (`relevante`, `toast`) | `tempo-real-da-agenda.ts` |
//! | [`formatar_duracao`], [`dinheiro`] | `lib/datas.ts`, `lib/money.ts` |
//!
//! 🔑 **O dia é o do relógio do estúdio.** Um ensaio às 22:30 de São Paulo é
//! 01:30 UTC do dia seguinte: agrupar pelo UTC poria o ensaio no dia errado do
//! calendário. Tudo aqui passa por [`dia_no_estudio`].

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde_json::Value;

use crate::chatbot::modelo::fuso_do_estudio;

// ── O ensaio ───────────────────────────────────────────────────────────────

/// Os sete status do agendamento, com o rótulo e a cor do site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusDoEnsaio {
    Pendente,
    Confirmado,
    Cancelado,
    CanceladoUltimaHora,
    Reagendado,
    Concluido,
    NaoCompareceu,
}

impl StatusDoEnsaio {
    pub const TODOS: [StatusDoEnsaio; 7] = [
        StatusDoEnsaio::Pendente,
        StatusDoEnsaio::Confirmado,
        StatusDoEnsaio::Cancelado,
        StatusDoEnsaio::CanceladoUltimaHora,
        StatusDoEnsaio::Reagendado,
        StatusDoEnsaio::Concluido,
        StatusDoEnsaio::NaoCompareceu,
    ];

    /// O `status` do JSON (`Pending`, `Confirmed`…). Desconhecido é pendente:
    /// melhor mostrar amarelo do que sumir com o ensaio.
    pub fn da_api(texto: &str) -> StatusDoEnsaio {
        match texto {
            "Confirmed" => StatusDoEnsaio::Confirmado,
            "Cancelled" => StatusDoEnsaio::Cancelado,
            "CancelledLastMinute" => StatusDoEnsaio::CanceladoUltimaHora,
            "Rescheduled" => StatusDoEnsaio::Reagendado,
            "Completed" => StatusDoEnsaio::Concluido,
            "NoShow" => StatusDoEnsaio::NaoCompareceu,
            _ => StatusDoEnsaio::Pendente,
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            StatusDoEnsaio::Pendente => "Pendente",
            StatusDoEnsaio::Confirmado => "Confirmado",
            StatusDoEnsaio::Cancelado => "Cancelado",
            StatusDoEnsaio::CanceladoUltimaHora => "Cancelado (última hora)",
            StatusDoEnsaio::Reagendado => "Reagendado",
            StatusDoEnsaio::Concluido => "Concluído",
            StatusDoEnsaio::NaoCompareceu => "Não compareceu",
        }
    }

    /// A cor do evento no calendário, em RGB.
    pub fn cor(self) -> u32 {
        match self {
            StatusDoEnsaio::Pendente => 0xeab308,
            StatusDoEnsaio::Confirmado => 0x22c55e,
            StatusDoEnsaio::Cancelado | StatusDoEnsaio::CanceladoUltimaHora => 0xef4444,
            StatusDoEnsaio::Reagendado => 0x3b82f6,
            StatusDoEnsaio::Concluido => 0xa855f7,
            StatusDoEnsaio::NaoCompareceu => 0x6b7280,
        }
    }

    pub fn cancelado(self) -> bool {
        matches!(
            self,
            StatusDoEnsaio::Cancelado | StatusDoEnsaio::CanceladoUltimaHora
        )
    }

    /// Conta para o pontinho das visões de ano e estação (`contaNoDia`).
    pub fn conta_no_dia(self) -> bool {
        !self.cancelado()
    }
}

/// Um agendamento (`Booking` da API), com o que a tela usa.
#[derive(Debug, Clone, PartialEq)]
pub struct Ensaio {
    pub id: String,
    pub status: StatusDoEnsaio,
    pub inicio: DateTime<Utc>,
    pub fim: DateTime<Utc>,
    pub nome_familia: Option<String>,
    pub whatsapp: Option<String>,
    pub estudio_id: Option<String>,
    /// O **nome** do estúdio, como a API manda.
    pub estudio: Option<String>,
    pub cidade: Option<String>,
    pub pessoas: Option<i64>,
    pub tipo_ensaio: Option<String>,
    pub origem: Option<String>,
    pub notas: Option<String>,
    /// Em reais — a API manda o decimal como texto (`"250.00"`).
    pub valor_pago: Option<f64>,
    pub hora_entrada: Option<DateTime<Utc>>,
    pub hora_saida: Option<DateTime<Utc>>,
    pub observacoes_atendimento: Option<String>,
}

fn texto(v: &Value, campo: &str) -> Option<String> {
    v.get(campo)?
        .as_str()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

fn instante(v: &Value, campo: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(v.get(campo)?.as_str()?)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

impl Ensaio {
    pub fn ler(v: &Value) -> Option<Ensaio> {
        let valor_pago = match v.get("valor_pago") {
            Some(Value::String(t)) => t.trim().parse::<f64>().ok(),
            Some(Value::Number(n)) => n.as_f64(),
            _ => None,
        };
        Some(Ensaio {
            id: texto(v, "id")?,
            status: StatusDoEnsaio::da_api(v.get("status")?.as_str()?),
            inicio: instante(v, "start_time")?,
            fim: instante(v, "end_time")?,
            nome_familia: texto(v, "nome_familia"),
            whatsapp: texto(v, "whatsapp_phone"),
            estudio_id: texto(v, "studio_id"),
            estudio: texto(v, "estudio"),
            cidade: texto(v, "cidade"),
            pessoas: v.get("quantidade_de_pessoas").and_then(Value::as_i64),
            tipo_ensaio: texto(v, "tipo_ensaio"),
            origem: texto(v, "origem"),
            notas: texto(v, "notes"),
            valor_pago,
            hora_entrada: instante(v, "hora_entrada"),
            hora_saida: instante(v, "hora_saida"),
            observacoes_atendimento: texto(v, "observacoes_atendimento"),
        })
    }

    /// A lista de `GET /bookings/agenda`. Linha torta é descartada, e a
    /// lista sai em ordem de início (o backend já manda assim).
    pub fn lista(v: &Value) -> Vec<Ensaio> {
        let mut lista: Vec<Ensaio> = v
            .as_array()
            .map(|l| l.iter().filter_map(Ensaio::ler).collect())
            .unwrap_or_default();
        lista.sort_by_key(|e| e.inicio);
        lista
    }

    /// O título do evento no calendário.
    pub fn titulo(&self) -> String {
        self.nome_familia
            .clone()
            .unwrap_or_else(|| "Agendamento".into())
    }

    /// O nome na lista de hoje.
    pub fn nome_na_lista(&self) -> String {
        self.nome_familia
            .clone()
            .unwrap_or_else(|| "Sem nome".into())
    }

    pub fn estudio_ou_padrao(&self) -> String {
        self.estudio
            .clone()
            .unwrap_or_else(|| "Estúdio não definido".into())
    }

    /// Algum dado do atendimento já foi registrado.
    pub fn tem_atendimento(&self) -> bool {
        self.valor_pago.is_some()
            || self.hora_entrada.is_some()
            || self.hora_saida.is_some()
            || self.observacoes_atendimento.is_some()
    }

    pub fn duracao_em_minutos(&self) -> i64 {
        (self.fim - self.inicio).num_minutes().max(0)
    }

    /// O dia do calendário onde o ensaio cai.
    pub fn dia(&self) -> NaiveDate {
        dia_no_estudio(self.inicio)
    }

    /// `https://wa.me/<dígitos>` — o "Mensagem" e o link do WhatsApp.
    pub fn link_do_whatsapp(&self) -> Option<String> {
        let digitos: String = self
            .whatsapp
            .as_deref()?
            .chars()
            .filter(char::is_ascii_digit)
            .collect();
        (!digitos.is_empty()).then(|| format!("https://wa.me/{digitos}"))
    }
}

/// Os números de `GET /bookings/stats` — de hoje em diante, todos os estúdios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Indicadores {
    pub total: i64,
    pub pendentes: i64,
    pub confirmados: i64,
    pub cancelados: i64,
    pub concluidos: i64,
    pub ocupacao_hoje: i64,
    pub total_da_semana: i64,
}

impl Indicadores {
    pub fn ler(v: &Value) -> Option<Indicadores> {
        let n = |c: &str| v.get(c).and_then(Value::as_i64);
        Some(Indicadores {
            total: n("total")?,
            pendentes: n("pending").unwrap_or(0),
            confirmados: n("confirmed").unwrap_or(0),
            cancelados: n("cancelled").unwrap_or(0),
            concluidos: n("completed").unwrap_or(0),
            ocupacao_hoje: n("ocupacao_hoje").unwrap_or(0),
            total_da_semana: n("total_da_semana").unwrap_or(0),
        })
    }

    /// `round(confirmados / total × 100)`, e 0 sem ensaio nenhum.
    pub fn taxa_de_confirmacao(&self) -> i64 {
        if self.total <= 0 {
            return 0;
        }
        ((self.confirmados as f64 / self.total as f64) * 100.0).round() as i64
    }
}

/// Um estúdio do seletor (`GET /bookings/studios`, só os ativos).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estudio {
    pub id: String,
    pub nome: String,
}

impl Estudio {
    pub fn lista_ativa(v: &Value) -> Vec<Estudio> {
        v.as_array()
            .map(|l| {
                l.iter()
                    .filter(|e| e.get("is_active").and_then(Value::as_bool).unwrap_or(true))
                    .filter_map(|e| {
                        Some(Estudio {
                            id: texto(e, "id")?,
                            nome: texto(e, "name")?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ── O relógio do estúdio ───────────────────────────────────────────────────

pub fn dia_no_estudio(instante: DateTime<Utc>) -> NaiveDate {
    instante.with_timezone(&fuso_do_estudio()).date_naive()
}

/// Hoje, no estúdio.
pub fn hoje(agora: DateTime<Utc>) -> NaiveDate {
    dia_no_estudio(agora)
}

/// `14:05` no fuso do estúdio.
pub fn hora(instante: DateTime<Utc>) -> String {
    instante
        .with_timezone(&fuso_do_estudio())
        .format("%H:%M")
        .to_string()
}

/// `dd/mm/aaaa hh:mm` — o que os campos de data e hora mostram e leem.
pub fn para_o_campo(instante: DateTime<Utc>) -> String {
    instante
        .with_timezone(&fuso_do_estudio())
        .format("%d/%m/%Y %H:%M")
        .to_string()
}

/// O caminho de volta de [`para_o_campo`]: a hora do estúdio vira UTC.
/// Aceita também `aaaa-mm-ddThh:mm` (o `datetime-local` do site).
pub fn do_campo(texto: &str) -> Option<DateTime<Utc>> {
    let texto = texto.trim();
    let ingenuo = ["%d/%m/%Y %H:%M", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M"]
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(texto, f).ok())?;
    fuso_do_estudio()
        .from_local_datetime(&ingenuo)
        .single()
        .map(|d| d.with_timezone(&Utc))
}

const DIAS: [&str; 7] = [
    "domingo",
    "segunda-feira",
    "terça-feira",
    "quarta-feira",
    "quinta-feira",
    "sexta-feira",
    "sábado",
];
const MESES: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];

pub fn nome_do_mes(mes0: u32) -> &'static str {
    MESES[(mes0 as usize) % 12]
}

/// `sexta-feira, 25 de setembro` — o subtítulo do detalhe.
pub fn data_longa(dia: NaiveDate) -> String {
    format!(
        "{}, {} de {}",
        DIAS[dia.weekday().num_days_from_sunday() as usize],
        dia.day(),
        nome_do_mes(dia.month0())
    )
}

/// `25/09/2026`.
pub fn data_curta(dia: NaiveDate) -> String {
    dia.format("%d/%m/%Y").to_string()
}

/// `1h`, `45min`, `1h30` (`formatarDuracao`).
pub fn formatar_duracao(minutos: i64) -> String {
    let (h, m) = (minutos / 60, minutos % 60);
    match (h, m) {
        (0, m) => format!("{m}min"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h{m:02}"),
    }
}

/// `R$ 1.250,50` — o `Intl.NumberFormat("pt-BR", {currency: "BRL"})`.
pub fn dinheiro(reais: f64) -> String {
    let centavos = (reais * 100.0).round() as i64;
    let negativo = centavos < 0;
    let centavos = centavos.abs();
    let inteiro = (centavos / 100).to_string();
    let mut agrupado = String::new();
    for (i, c) in inteiro.chars().enumerate() {
        if i > 0 && (inteiro.len() - i).is_multiple_of(3) {
            agrupado.push('.');
        }
        agrupado.push(c);
    }
    format!(
        "{}R$ {agrupado},{:02}",
        if negativo { "-" } else { "" },
        centavos % 100
    )
}

/// O valor digitado: `250`, `250,5`, `1.250,50` ou `250.50`. `Err` é o
/// "Valor pago inválido." do site; vazio é `Ok(None)`.
pub fn ler_valor(texto: &str) -> Result<Option<f64>, &'static str> {
    let t = texto.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let normalizado = if t.contains(',') {
        t.replace('.', "").replace(',', ".")
    } else {
        t.to_string()
    };
    match normalizado.parse::<f64>() {
        Ok(v) if v.is_finite() && v >= 0.0 => Ok(Some(v)),
        _ => Err("Valor pago inválido."),
    }
}

// ── O calendário ───────────────────────────────────────────────────────────

/// As seis visões do calendário do site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visao {
    #[default]
    Mes,
    Semana,
    Dia,
    Agenda,
    Ano,
    Estacao,
}

impl Visao {
    pub const TODAS: [Visao; 6] = [
        Visao::Mes,
        Visao::Semana,
        Visao::Dia,
        Visao::Agenda,
        Visao::Ano,
        Visao::Estacao,
    ];

    pub fn rotulo(self) -> &'static str {
        match self {
            Visao::Mes => "Mês",
            Visao::Semana => "Semana",
            Visao::Dia => "Dia",
            Visao::Agenda => "Agenda",
            Visao::Ano => "Ano",
            Visao::Estacao => "Estação",
        }
    }
}

fn primeiro_do_mes(ano: i32, mes0: u32) -> NaiveDate {
    let (ano, mes0) = (ano + (mes0 / 12) as i32, mes0 % 12);
    NaiveDate::from_ymd_opt(ano, mes0 + 1, 1).expect("dia 1 existe")
}

/// Soma meses a um primeiro-do-mês (negativo volta).
fn somar_meses(dia: NaiveDate, meses: i32) -> NaiveDate {
    let total = dia.year() * 12 + dia.month0() as i32 + meses;
    primeiro_do_mes(total.div_euclid(12), total.rem_euclid(12) as u32)
}

fn ultimo_do_mes(dia: NaiveDate) -> NaiveDate {
    somar_meses(primeiro_do_mes(dia.year(), dia.month0()), 1) - Duration::days(1)
}

/// O domingo da semana do dia — a semana do site começa no domingo.
pub fn domingo_da_semana(dia: NaiveDate) -> NaiveDate {
    dia - Duration::days(dia.weekday().num_days_from_sunday() as i64)
}

/// O começo da estação: verão = dez–fev, outono = mar–mai, inverno = jun–ago,
/// primavera = set–nov (`inicioDaEstacao`).
pub fn inicio_da_estacao(dia: NaiveDate) -> NaiveDate {
    let (ano, mes0) = (dia.year(), dia.month0());
    match mes0 {
        11 => primeiro_do_mes(ano, 11),
        0 | 1 => primeiro_do_mes(ano - 1, 11),
        m => primeiro_do_mes(ano, m - ((m - 2) % 3)),
    }
}

/// "Verão", "Outono", "Inverno" ou "Primavera" do mês (0–11).
pub fn nome_da_estacao(mes0: u32) -> &'static str {
    match mes0 {
        11 | 0 | 1 => "Verão",
        2..=4 => "Outono",
        5..=7 => "Inverno",
        _ => "Primavera",
    }
}

/// O período que a tela pede à API (`periodoVisivel`), de–até inclusive.
pub fn periodo_visivel(visao: Visao, dia: NaiveDate) -> (NaiveDate, NaiveDate) {
    match visao {
        Visao::Dia => (dia, dia),
        Visao::Semana => {
            let domingo = domingo_da_semana(dia);
            (domingo, domingo + Duration::days(6))
        }
        Visao::Agenda => (dia, dia + Duration::days(30)),
        Visao::Ano => (
            NaiveDate::from_ymd_opt(dia.year(), 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(dia.year(), 12, 31).unwrap(),
        ),
        Visao::Estacao => {
            let inicio = inicio_da_estacao(dia);
            (inicio, ultimo_do_mes(somar_meses(inicio, 2)))
        }
        // O mês leva uma semana de folga de cada lado: é o que a grade mostra
        // do mês vizinho.
        Visao::Mes => {
            let primeiro = primeiro_do_mes(dia.year(), dia.month0());
            (
                primeiro - Duration::days(7),
                ultimo_do_mes(primeiro) + Duration::days(7),
            )
        }
    }
}

/// "Anterior" (`-1`) e "Próximo" (`+1`) de cada visão.
pub fn navegar(visao: Visao, dia: NaiveDate, passo: i32) -> NaiveDate {
    let mesmo_dia_do_mes = |meses: i32| {
        let destino = somar_meses(primeiro_do_mes(dia.year(), dia.month0()), meses);
        destino
            .with_day(dia.day().min(ultimo_do_mes(destino).day()))
            .unwrap_or(destino)
    };
    match visao {
        Visao::Dia => dia + Duration::days(passo as i64),
        Visao::Semana => dia + Duration::days(7 * passo as i64),
        Visao::Agenda => dia + Duration::days(30 * passo as i64),
        Visao::Mes => mesmo_dia_do_mes(passo),
        Visao::Ano => mesmo_dia_do_mes(12 * passo),
        Visao::Estacao => mesmo_dia_do_mes(3 * passo),
    }
}

/// O título do período no alto do calendário.
pub fn titulo_do_periodo(visao: Visao, dia: NaiveDate) -> String {
    match visao {
        Visao::Mes => format!("{} de {}", nome_do_mes(dia.month0()), dia.year()),
        Visao::Dia => data_longa(dia),
        Visao::Semana => {
            let (de, ate) = periodo_visivel(visao, dia);
            if de.month() == ate.month() {
                format!(
                    "{} – {} de {} de {}",
                    de.day(),
                    ate.day(),
                    nome_do_mes(ate.month0()),
                    ate.year()
                )
            } else {
                format!(
                    "{} de {} – {} de {} de {}",
                    de.day(),
                    nome_do_mes(de.month0()),
                    ate.day(),
                    nome_do_mes(ate.month0()),
                    ate.year()
                )
            }
        }
        Visao::Agenda => {
            let (de, ate) = periodo_visivel(visao, dia);
            format!("{} – {}", data_curta(de), data_curta(ate))
        }
        Visao::Ano => dia.year().to_string(),
        Visao::Estacao => {
            let inicio = inicio_da_estacao(dia);
            if inicio.month0() == 11 {
                format!("Verão {}/{}", inicio.year(), inicio.year() + 1)
            } else {
                format!("{} {}", nome_da_estacao(inicio.month0()), inicio.year())
            }
        }
    }
}

/// As semanas da grade de um mês: de domingo a sábado, cobrindo o mês inteiro.
pub fn semanas_do_mes(dia: NaiveDate) -> Vec<[NaiveDate; 7]> {
    let primeiro = primeiro_do_mes(dia.year(), dia.month0());
    let ultimo = ultimo_do_mes(primeiro);
    let mut inicio = domingo_da_semana(primeiro);
    let mut semanas = Vec::new();
    while inicio <= ultimo {
        semanas.push(std::array::from_fn(|i| inicio + Duration::days(i as i64)));
        inicio += Duration::days(7);
    }
    semanas
}

/// Os meses que a visão de ano (12) ou de estação (3) mostra.
pub fn meses_da_visao(visao: Visao, dia: NaiveDate) -> Vec<NaiveDate> {
    match visao {
        Visao::Ano => (0..12).map(|m| primeiro_do_mes(dia.year(), m)).collect(),
        Visao::Estacao => {
            let inicio = inicio_da_estacao(dia);
            (0..3).map(|m| somar_meses(inicio, m)).collect()
        }
        _ => Vec::new(),
    }
}

/// Os ensaios de um dia, em ordem de início.
pub fn ensaios_do_dia(ensaios: &[Ensaio], dia: NaiveDate) -> Vec<Ensaio> {
    ensaios.iter().filter(|e| e.dia() == dia).cloned().collect()
}

/// O dia tem ensaio que conta (o pontinho das visões de ano e estação).
pub fn dia_com_ponto(ensaios: &[Ensaio], dia: NaiveDate) -> bool {
    ensaios
        .iter()
        .any(|e| e.dia() == dia && e.status.conta_no_dia())
}

// ── Origem e tipo ──────────────────────────────────────────────────────────

fn normalizar(origem: &str) -> String {
    crate::chatbot::modelo::normalizar(origem.trim())
}

/// "WhatsApp", "Site", "Telefone", "Indicação", "Manual" — o resto como veio.
pub fn nome_da_origem(origem: &str) -> Option<String> {
    let limpo = origem.trim();
    if limpo.is_empty() {
        return None;
    }
    Some(
        match normalizar(limpo).as_str() {
            "whatsapp" => "WhatsApp",
            "site" => "Site",
            "telefone" => "Telefone",
            "indicacao" => "Indicação",
            "manual" => "Manual",
            _ => return Some(limpo.to_string()),
        }
        .to_string(),
    )
}

/// O pedaço do título do aviso: " pelo WhatsApp", " pelo site", " por
/// telefone" — e nada para os outros.
pub fn origem_no_aviso(origem: Option<&str>) -> &'static str {
    match origem.map(normalizar).as_deref() {
        Some("whatsapp") => " pelo WhatsApp",
        Some("site") => " pelo site",
        Some("telefone") => " por telefone",
        _ => "",
    }
}

pub fn tipo_de_ensaio(tipo: &str) -> String {
    match tipo {
        "FAMILIA" => "Família",
        "GESTANTE" => "Gestante",
        "NEWBORN" => "Newborn",
        "CASAL" => "Casal",
        "INFANTIL" => "Infantil",
        "INDIVIDUAL" => "Individual",
        "SMASH_THE_CAKE" => "Smash the Cake",
        outro => return outro.to_string(),
    }
    .to_string()
}

// ── O tempo real ───────────────────────────────────────────────────────────

/// Um evento de `/bookings/agenda/eventos`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventoDaAgenda {
    pub tipo: String,
    pub ensaio_id: String,
    pub quando: Option<DateTime<Utc>>,
    pub estudio_id: Option<String>,
    pub origem: Option<String>,
    pub cliente: Option<String>,
}

impl EventoDaAgenda {
    pub fn ler(v: &Value) -> Option<EventoDaAgenda> {
        Some(EventoDaAgenda {
            tipo: texto(v, "tipo")?,
            ensaio_id: texto(v, "ensaio_id")?,
            quando: instante(v, "quando"),
            estudio_id: texto(v, "estudio_id"),
            origem: texto(v, "origem"),
            cliente: texto(v, "cliente"),
        })
    }

    /// Muda o que a tela mostra? (`relevante`). Remarcação sempre relê (o
    /// ensaio pode ter saído do período); sem data, também.
    ///
    /// ⚠️ O dia é o do **estúdio**, e não o recorte UTC do site — lá, com a
    /// visão de dia, um ensaio das 22:00 era comparado com o dia seguinte.
    pub fn relevante(&self, de: NaiveDate, ate: NaiveDate, estudio: Option<&str>) -> bool {
        if self.tipo == "ensaio_remarcado" {
            return true;
        }
        let Some(quando) = self.quando else {
            return true;
        };
        if let (Some(filtro), Some(dele)) = (estudio, self.estudio_id.as_deref()) {
            if filtro != dele {
                return false;
            }
        }
        let dia = dia_no_estudio(quando);
        dia >= de && dia <= ate
    }

    /// O toast (e o aviso do sistema): título e descrição, só para ensaio
    /// criado e cancelado.
    pub fn aviso(&self) -> Option<(String, Option<String>)> {
        let titulo = match self.tipo.as_str() {
            "ensaio_criado" => format!(
                "Novo agendamento{}",
                origem_no_aviso(self.origem.as_deref())
            ),
            "ensaio_cancelado" => "Agendamento cancelado".into(),
            _ => return None,
        };
        let quando = self.quando.map(|q| {
            q.with_timezone(&fuso_do_estudio())
                .format("%d/%m, %H:%M")
                .to_string()
        });
        let partes: Vec<String> = [self.cliente.clone(), quando]
            .into_iter()
            .flatten()
            .collect();
        let descricao = (!partes.is_empty()).then(|| partes.join(" · "));
        Some((titulo, descricao))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    fn d(a: i32, m: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(a, m, dia).unwrap()
    }

    /// Terça, 15 de setembro de 2026 — a âncora do `periodo.test.ts`.
    const TERCA: (i32, u32, u32) = (2026, 9, 15);

    fn terca() -> NaiveDate {
        d(TERCA.0, TERCA.1, TERCA.2)
    }

    #[test]
    fn o_periodo_visivel_de_cada_visao_e_o_do_site() {
        assert_eq!(periodo_visivel(Visao::Dia, terca()), (terca(), terca()));
        assert_eq!(
            periodo_visivel(Visao::Semana, terca()),
            (d(2026, 9, 13), d(2026, 9, 19)),
            "domingo a sábado"
        );
        assert_eq!(
            periodo_visivel(Visao::Mes, terca()),
            (d(2026, 8, 25), d(2026, 10, 7)),
            "uma semana de folga de cada lado"
        );
        assert_eq!(
            periodo_visivel(Visao::Ano, terca()),
            (d(2026, 1, 1), d(2026, 12, 31))
        );
        assert_eq!(
            periodo_visivel(Visao::Estacao, terca()),
            (d(2026, 9, 1), d(2026, 11, 30)),
            "setembro é primavera"
        );
        assert_eq!(
            periodo_visivel(Visao::Estacao, d(2026, 1, 20)),
            (d(2025, 12, 1), d(2026, 2, 28)),
            "o verão atravessa a virada"
        );
        assert_eq!(
            periodo_visivel(Visao::Estacao, d(2026, 12, 20)),
            (d(2026, 12, 1), d(2027, 2, 28))
        );
        assert_eq!(
            periodo_visivel(Visao::Agenda, terca()),
            (terca(), d(2026, 10, 15))
        );
        // Nenhuma visão passa do teto de um ano da rota.
        for visao in Visao::TODAS {
            let (de, ate) = periodo_visivel(visao, terca());
            assert!((ate - de).num_days() <= 366, "{visao:?}");
        }
    }

    #[test]
    fn as_estacoes_do_hemisferio_sul() {
        let nomes: Vec<&str> = (0..12).map(nome_da_estacao).collect();
        assert_eq!(
            nomes,
            [
                "Verão",
                "Verão",
                "Outono",
                "Outono",
                "Outono",
                "Inverno",
                "Inverno",
                "Inverno",
                "Primavera",
                "Primavera",
                "Primavera",
                "Verão"
            ]
        );
        assert_eq!(inicio_da_estacao(d(2026, 1, 5)), d(2025, 12, 1));
        assert_eq!(inicio_da_estacao(d(2026, 4, 30)), d(2026, 3, 1));
        assert_eq!(inicio_da_estacao(d(2026, 8, 1)), d(2026, 6, 1));
    }

    #[test]
    fn anterior_e_proximo_andam_o_tamanho_da_visao() {
        assert_eq!(navegar(Visao::Dia, terca(), 1), d(2026, 9, 16));
        assert_eq!(navegar(Visao::Semana, terca(), -1), d(2026, 9, 8));
        assert_eq!(
            navegar(Visao::Mes, d(2026, 1, 31), 1),
            d(2026, 2, 28),
            "31 vira o último"
        );
        assert_eq!(navegar(Visao::Mes, d(2026, 1, 15), -1), d(2025, 12, 15));
        assert_eq!(navegar(Visao::Ano, terca(), 1), d(2027, 9, 15));
        assert_eq!(navegar(Visao::Estacao, terca(), 1), d(2026, 12, 15));
        assert_eq!(navegar(Visao::Agenda, terca(), 1), d(2026, 10, 15));
    }

    #[test]
    fn os_titulos_do_periodo() {
        assert_eq!(titulo_do_periodo(Visao::Mes, terca()), "setembro de 2026");
        assert_eq!(
            titulo_do_periodo(Visao::Semana, terca()),
            "13 – 19 de setembro de 2026"
        );
        assert_eq!(
            titulo_do_periodo(Visao::Semana, d(2026, 9, 29)),
            "27 de setembro – 3 de outubro de 2026"
        );
        assert_eq!(
            titulo_do_periodo(Visao::Dia, terca()),
            "terça-feira, 15 de setembro"
        );
        assert_eq!(
            titulo_do_periodo(Visao::Agenda, terca()),
            "15/09/2026 – 15/10/2026"
        );
        assert_eq!(titulo_do_periodo(Visao::Ano, terca()), "2026");
        assert_eq!(titulo_do_periodo(Visao::Estacao, terca()), "Primavera 2026");
        assert_eq!(
            titulo_do_periodo(Visao::Estacao, d(2026, 1, 3)),
            "Verão 2025/2026"
        );
    }

    #[test]
    fn a_grade_do_mes_comeca_no_domingo_e_cobre_o_mes() {
        let semanas = semanas_do_mes(terca());
        assert_eq!(semanas.first().unwrap()[0], d(2026, 8, 30));
        assert_eq!(semanas.last().unwrap()[6], d(2026, 10, 3));
        assert_eq!(semanas.len(), 5);
        assert_eq!(meses_da_visao(Visao::Ano, terca()).len(), 12);
        assert_eq!(
            meses_da_visao(Visao::Estacao, d(2026, 1, 9)),
            vec![d(2025, 12, 1), d(2026, 1, 1), d(2026, 2, 1)]
        );
        assert!(meses_da_visao(Visao::Mes, terca()).is_empty());
    }

    fn ensaio(status: &str, inicio: &str) -> Value {
        json!({
            "id": "e1", "user_id": "u", "status": status, "start_time": inicio,
            "end_time": "2026-09-15T15:30:00Z", "created_at": inicio, "updated_at": inicio,
            "studio_id": "s1", "estudio": "Estúdio Gramado", "nome_familia": "Família Souza",
            "whatsapp_phone": "+55 (54) 99999-1234", "valor_pago": "250.00",
            "quantidade_de_pessoas": 4, "tipo_ensaio": "FAMILIA", "origem": "WhatsApp",
            "notes": null, "hora_entrada": null, "hora_saida": null,
            "observacoes_atendimento": null
        })
    }

    #[test]
    fn o_ensaio_le_o_contrato_da_api() {
        let e = Ensaio::ler(&ensaio("Confirmed", "2026-09-15T14:00:00Z")).unwrap();
        assert_eq!(e.status, StatusDoEnsaio::Confirmado);
        assert_eq!(e.valor_pago, Some(250.0), "o decimal vem como texto");
        assert_eq!(e.duracao_em_minutos(), 90);
        assert_eq!(formatar_duracao(e.duracao_em_minutos()), "1h30");
        assert_eq!(
            e.link_do_whatsapp().as_deref(),
            Some("https://wa.me/5554999991234")
        );
        assert!(e.tem_atendimento());
        assert_eq!(e.titulo(), "Família Souza");
        assert_eq!(tipo_de_ensaio(e.tipo_ensaio.as_deref().unwrap()), "Família");

        let mut sem_nome = ensaio("Pending", "2026-09-15T14:00:00Z");
        sem_nome["nome_familia"] = Value::Null;
        sem_nome["valor_pago"] = Value::Null;
        sem_nome["estudio"] = json!("  ");
        let e = Ensaio::ler(&sem_nome).unwrap();
        assert_eq!(e.titulo(), "Agendamento");
        assert_eq!(e.nome_na_lista(), "Sem nome");
        assert_eq!(e.estudio_ou_padrao(), "Estúdio não definido");
        assert!(!e.tem_atendimento());

        let lista = Ensaio::lista(&json!([
            ensaio("Pending", "2026-09-15T18:00:00Z"),
            {"id": "torto"},
            ensaio("Pending", "2026-09-15T12:00:00Z")
        ]));
        assert_eq!(lista.len(), 2);
        assert!(lista[0].inicio < lista[1].inicio, "em ordem de início");
    }

    #[test]
    fn o_dia_e_o_do_relogio_do_estudio() {
        // 22:30 em Gramado é 01:30 UTC do dia seguinte.
        let tarde = Ensaio::ler(&ensaio("Confirmed", "2026-09-16T01:30:00Z")).unwrap();
        assert_eq!(tarde.dia(), d(2026, 9, 15));
        assert_eq!(hora(tarde.inicio), "22:30");
        assert_eq!(para_o_campo(tarde.inicio), "15/09/2026 22:30");
        assert_eq!(do_campo("15/09/2026 22:30"), Some(tarde.inicio));
        assert_eq!(
            do_campo("2026-09-15T22:30"),
            Some(tarde.inicio),
            "o formato do site"
        );
        assert_eq!(do_campo("amanhã"), None);
        assert_eq!(data_longa(d(2026, 9, 25)), "sexta-feira, 25 de setembro");
    }

    #[test]
    fn os_sete_status_e_o_pontinho() {
        for status in StatusDoEnsaio::TODOS {
            assert!(!status.rotulo().is_empty());
        }
        assert_eq!(
            StatusDoEnsaio::Cancelado.cor(),
            StatusDoEnsaio::CanceladoUltimaHora.cor()
        );
        assert_ne!(
            StatusDoEnsaio::Cancelado.rotulo(),
            StatusDoEnsaio::CanceladoUltimaHora.rotulo()
        );
        let fora: Vec<StatusDoEnsaio> = StatusDoEnsaio::TODOS
            .into_iter()
            .filter(|s| !s.conta_no_dia())
            .collect();
        assert_eq!(
            fora,
            [
                StatusDoEnsaio::Cancelado,
                StatusDoEnsaio::CanceladoUltimaHora
            ]
        );
        assert_eq!(
            StatusDoEnsaio::da_api("Inventado"),
            StatusDoEnsaio::Pendente
        );

        let ensaios = Ensaio::lista(&json!([ensaio("Cancelled", "2026-09-15T14:00:00Z")]));
        assert!(
            !dia_com_ponto(&ensaios, terca()),
            "cancelado não pinta o dia"
        );
        let ensaios = Ensaio::lista(&json!([ensaio("NoShow", "2026-09-15T14:00:00Z")]));
        assert!(dia_com_ponto(&ensaios, terca()));
        assert_eq!(ensaios_do_dia(&ensaios, terca()).len(), 1);
        assert!(ensaios_do_dia(&ensaios, d(2026, 9, 16)).is_empty());
    }

    #[test]
    fn dinheiro_e_valor_digitado() {
        assert_eq!(dinheiro(250.0), "R$ 250,00");
        assert_eq!(dinheiro(1250.5), "R$ 1.250,50");
        assert_eq!(dinheiro(1234567.891), "R$ 1.234.567,89");
        assert_eq!(dinheiro(0.0), "R$ 0,00");
        assert_eq!(ler_valor(""), Ok(None));
        assert_eq!(ler_valor("250"), Ok(Some(250.0)));
        assert_eq!(ler_valor("250,5"), Ok(Some(250.5)));
        assert_eq!(ler_valor("1.250,50"), Ok(Some(1250.5)));
        assert_eq!(ler_valor("250.50"), Ok(Some(250.5)));
        assert_eq!(ler_valor("-3"), Err("Valor pago inválido."));
        assert_eq!(ler_valor("dez"), Err("Valor pago inválido."));
        assert_eq!(formatar_duracao(45), "45min");
        assert_eq!(formatar_duracao(120), "2h");
    }

    #[test]
    fn a_origem_como_o_site() {
        assert_eq!(nome_da_origem("WHATSAPP").as_deref(), Some("WhatsApp"));
        assert_eq!(nome_da_origem(" indicação ").as_deref(), Some("Indicação"));
        assert_eq!(nome_da_origem("Instagram").as_deref(), Some("Instagram"));
        assert_eq!(nome_da_origem("  "), None);
        assert_eq!(origem_no_aviso(Some("SITE")), " pelo site");
        assert_eq!(origem_no_aviso(Some("Manual")), "");
        assert_eq!(origem_no_aviso(None), "");
    }

    fn evento(campos: Value) -> EventoDaAgenda {
        let mut base = json!({
            "tipo": "ensaio_criado", "ensaio_id": "e1", "quando": "2026-08-15T14:00:00Z",
            "estudio_id": null, "origem": null, "cliente": null, "em": "2026-08-01T10:00:00Z"
        });
        for (k, v) in campos.as_object().unwrap() {
            base[k] = v.clone();
        }
        EventoDaAgenda::ler(&base).unwrap()
    }

    #[test]
    fn a_relevancia_do_evento_e_a_do_site() {
        let (de, ate) = (d(2026, 8, 1), d(2026, 8, 31));
        assert!(evento(json!({})).relevante(de, ate, None));
        assert!(!evento(json!({"quando": "2026-11-03T14:00:00Z"})).relevante(de, ate, None));
        assert!(evento(json!({"quando": "2026-08-01T08:00:00Z"})).relevante(de, ate, None));
        assert!(evento(json!({"quando": "2026-08-31T20:00:00Z"})).relevante(de, ate, None));
        let um = Some("estudio-1");
        assert!(evento(json!({"estudio_id": "estudio-1"})).relevante(de, ate, um));
        assert!(!evento(json!({"estudio_id": "estudio-2"})).relevante(de, ate, um));
        assert!(evento(json!({"estudio_id": null})).relevante(de, ate, um));
        assert!(
            evento(json!({"tipo": "ensaio_remarcado", "quando": "2027-01-01T10:00:00Z"}))
                .relevante(de, ate, None)
        );
        assert!(evento(json!({"quando": null})).relevante(de, ate, None));
        // 01:00 UTC do dia 1º de setembro é 31 de agosto no estúdio.
        assert!(evento(json!({"quando": "2026-09-01T01:00:00Z"})).relevante(de, ate, None));
        assert!(EventoDaAgenda::ler(&json!({"tipo": "x"})).is_none());
    }

    #[test]
    fn o_aviso_diz_de_onde_veio_quem_e_quando() {
        let criado = evento(json!({"origem": "WHATSAPP", "cliente": "Família Souza"}));
        assert_eq!(
            criado.aviso(),
            Some((
                "Novo agendamento pelo WhatsApp".into(),
                Some("Família Souza · 15/08, 11:00".into())
            )),
            "a hora sai no fuso do estúdio"
        );
        assert_eq!(
            evento(json!({"origem": "SITE"})).aviso().unwrap().0,
            "Novo agendamento pelo site"
        );
        assert_eq!(
            evento(json!({"origem": "MANUAL"})).aviso().unwrap().0,
            "Novo agendamento"
        );
        let cancelado = evento(json!({"tipo": "ensaio_cancelado", "quando": null}));
        assert_eq!(
            cancelado.aviso(),
            Some(("Agendamento cancelado".into(), None))
        );
        for tipo in [
            "ensaio_remarcado",
            "ensaio_atualizado",
            "ensaio_removido",
            "ensaio_restaurado",
        ] {
            assert!(evento(json!({"tipo": tipo})).aviso().is_none(), "{tipo}");
        }
    }

    #[test]
    fn os_indicadores_e_a_taxa() {
        let i = Indicadores::ler(&json!({
            "total": 8, "pending": 2, "confirmed": 5, "cancelled": 1, "rescheduled": 0,
            "completed": 0, "ocupacao_hoje": 67, "total_da_semana": 3
        }))
        .unwrap();
        assert_eq!(i.taxa_de_confirmacao(), 63);
        assert_eq!(Indicadores::default().taxa_de_confirmacao(), 0);
        assert!(Indicadores::ler(&json!({})).is_none());
        let estudios = Estudio::lista_ativa(&json!([
            {"id": "s1", "name": "Gramado", "is_active": true},
            {"id": "s2", "name": "Canela", "is_active": false}
        ]));
        assert_eq!(
            estudios,
            vec![Estudio {
                id: "s1".into(),
                nome: "Gramado".into()
            }]
        );
    }
}
