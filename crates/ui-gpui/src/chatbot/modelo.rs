//! O modelo do painel do chatbot — as regras do `/dashboard/chatbot` do site,
//! em funções puras.
//!
//! | Aqui | No site |
//! |---|---|
//! | [`Canal`], [`FiltroDeCanal`], [`Status`] | `modelo.ts` |
//! | [`conversas_unificadas`] | `conversasUnificadas` |
//! | [`historico`] | `historicoDaConversa` + `mensagensDoContato` |
//! | [`Janela`], [`decisao_do_compositor`] | `lib/janela-de-atendimento.ts` |
//! | [`hora`], [`rotulo_do_dia`], [`carimbo`] | `conversas-do-whatsapp.ts` |
//! | [`EventoDoChatbot`], [`EventoDoChatbot::notificacao`] | `tempo-real.ts` |
//! | [`Urgencia`] | `urgencias.tsx` |
//!
//! 🔑 **Nada aqui sabe de tela.** São as decisões que mudam o que o atendente
//! vê — qual conversa entra na lista, em que ordem, se dá para escrever — e
//! nenhuma delas se confere olhando um componente.
//!
//! Os textos são os do site, letra por letra: o operador que usa os dois não
//! pode ter de aprender duas línguas.

use chrono::{DateTime, Duration, FixedOffset, Utc};
use serde::Deserialize;
use serde_json::Value;

/// O relógio do estúdio (Gramado e Canela): UTC−3, sem horário de verão
/// desde 2019 — o mesmo `America/Sao_Paulo` que o site usa.
pub fn fuso_do_estudio() -> FixedOffset {
    FixedOffset::west_opt(3 * 3600).expect("−3 h é um fuso válido")
}

// ── Canais ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Canal {
    WhatsApp,
    Instagram,
    /// O chat do site (LexDesk). Na tela é "Site".
    Web,
    Telegram,
    Messenger,
}

impl Canal {
    pub const TODOS: [Canal; 5] = [
        Canal::WhatsApp,
        Canal::Instagram,
        Canal::Messenger,
        Canal::Telegram,
        Canal::Web,
    ];

    /// O prefixo da chave (`wa:5554…`), o mesmo do `?conversa=` do site.
    pub fn prefixo(self) -> &'static str {
        match self {
            Canal::WhatsApp => "wa",
            Canal::Instagram => "ig",
            Canal::Web => "web",
            Canal::Telegram => "tg",
            Canal::Messenger => "ms",
        }
    }

    /// O rótulo da aba.
    pub fn rotulo(self) -> &'static str {
        match self {
            Canal::WhatsApp => "WhatsApp",
            Canal::Instagram => "Instagram",
            Canal::Web => "Site",
            Canal::Telegram => "Telegram",
            Canal::Messenger => "Messenger",
        }
    }

    /// A raiz das rotas do painel deste canal na API. O WhatsApp tem rotas
    /// próprias (inglês, legado) e não passa por aqui.
    pub fn raiz(self) -> &'static str {
        match self {
            Canal::WhatsApp => "/whatsapp",
            Canal::Instagram => "/instagram",
            Canal::Web => "/lexdesk",
            Canal::Telegram => "/telegram",
            Canal::Messenger => "/messenger",
        }
    }

    /// O fluxo de eventos do canal — os cinco que o proxy do site funde.
    pub fn fluxo(self) -> &'static str {
        match self {
            Canal::WhatsApp => "/whatsapp/eventos",
            Canal::Instagram => "/instagram/eventos",
            Canal::Web => "/lexdesk/eventos",
            Canal::Telegram => "/telegram/eventos",
            Canal::Messenger => "/messenger/eventos",
        }
    }

    /// O canal de uma fonte do tempo real (o rótulo é o [`Canal::prefixo`]).
    pub fn da_fonte(fonte: &str) -> Option<Canal> {
        Canal::TODOS.into_iter().find(|c| c.prefixo() == fonte)
    }

    /// A ordem em que o site junta as listas (`[...whatsapp, ...instagram,
    /// ...web, ...telegram, ...messenger]`) — decide o empate.
    fn ordem_na_fusao(self) -> u8 {
        match self {
            Canal::WhatsApp => 0,
            Canal::Instagram => 1,
            Canal::Web => 2,
            Canal::Telegram => 3,
            Canal::Messenger => 4,
        }
    }

    /// Só os canais da Meta têm a janela de 24 horas.
    pub fn sujeito_a_janela(self) -> bool {
        matches!(self, Canal::WhatsApp | Canal::Instagram | Canal::Messenger)
    }
}

/// A chave de uma conversa: canal + id do contato naquele canal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chave {
    pub canal: Canal,
    pub id: String,
}

impl Chave {
    pub fn nova(canal: Canal, id: impl Into<String>) -> Self {
        Self {
            canal,
            id: id.into(),
        }
    }

    /// `wa:5554…` — o destino do aviso e do "Ver conversa".
    pub fn texto(&self) -> String {
        format!("{}:{}", self.canal.prefixo(), self.id)
    }

    /// O caminho de volta, como `conversaDaChave`: torto vira `None`, não erro.
    pub fn do_texto(texto: &str) -> Option<Chave> {
        let (prefixo, id) = texto.split_once(':')?;
        if id.is_empty() {
            return None;
        }
        let canal = Canal::TODOS.into_iter().find(|c| c.prefixo() == prefixo)?;
        Some(Chave::nova(canal, id))
    }
}

/// As abas: "Todas" ou um canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FiltroDeCanal {
    #[default]
    Todas,
    So(Canal),
}

impl FiltroDeCanal {
    pub fn aceita(self, canal: Canal) -> bool {
        match self {
            FiltroDeCanal::Todas => true,
            FiltroDeCanal::So(c) => c == canal,
        }
    }
}

/// Os filtros de status da fita.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Todas,
    Bot,
    /// "Em atendimento" — o `manual` do WhatsApp e o "assumida" dos outros.
    Humano,
    /// "Não lidas" — só o WhatsApp conta; os outros canais saem da lista.
    NaoLidas,
}

impl Status {
    pub const TODOS: [Status; 4] = [Status::Todas, Status::Bot, Status::Humano, Status::NaoLidas];

    pub fn rotulo(self) -> &'static str {
        match self {
            Status::Todas => "Todas",
            Status::Bot => "Bot ativo",
            Status::Humano => "Em atendimento",
            Status::NaoLidas => "Não lidas",
        }
    }

    /// O `status_filter` que a API do WhatsApp entende (`manual`, não
    /// `humano`); `None` é não mandar o parâmetro.
    pub fn para_o_whatsapp(self) -> Option<&'static str> {
        match self {
            Status::Todas => None,
            Status::Bot => Some("bot"),
            Status::Humano => Some("manual"),
            Status::NaoLidas => Some("unread"),
        }
    }
}

// ── Conversas ──────────────────────────────────────────────────────────────

/// O voucher de um contato do WhatsApp (`POST /whatsapp/vouchers/info`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(default)]
pub struct Voucher {
    pub contact_id: String,
    pub has_voucher: bool,
    pub voucher_code: Option<String>,
    pub voucher_used: bool,
    pub reminders_sent: i64,
}

/// O cadastro do contato no site (`POST /crm/contacts/by-whatsapp`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(default)]
pub struct Cadastro {
    pub whatsapp: String,
    pub abandoned_carts: i64,
    pub completed_orders: i64,
}

/// Uma conversa de qualquer canal, no formato que a lista desenha.
#[derive(Debug, Clone, PartialEq)]
pub struct Conversa {
    pub chave: Chave,
    pub nome: String,
    pub previa: Option<String>,
    pub quando: Option<DateTime<Utc>>,
    /// O bot está calado e uma pessoa responde (pausado / assumida).
    pub atendimento_humano: bool,
    /// Mensagens do cliente sem resposta. Só o WhatsApp mede: nos outros é 0,
    /// e 0 ali quer dizer "não medido", não "em dia".
    pub sem_resposta: i64,
    /// WhatsApp: quantas mensagens a conversa tem (a legenda da linha).
    pub mensagens: Option<i64>,
    /// WhatsApp: só disparo automático, nenhuma escrita por gente.
    pub so_automaticas: bool,
    /// WhatsApp: quando o cliente escreveu pela última vez — abre a janela.
    pub ultima_entrada: Option<DateTime<Utc>>,
    /// Site: o visitante está com a página aberta.
    pub na_pagina: bool,
    pub saiu_ha_minutos: Option<i64>,
    pub voucher: Option<Voucher>,
    pub cadastro: Option<Cadastro>,
}

impl Conversa {
    fn vazia(chave: Chave, nome: String) -> Self {
        Self {
            chave,
            nome,
            previa: None,
            quando: None,
            atendimento_humano: false,
            sem_resposta: 0,
            mensagens: None,
            so_automaticas: false,
            ultima_entrada: None,
            na_pagina: false,
            saiu_ha_minutos: None,
            voucher: None,
            cadastro: None,
        }
    }

    /// "na página agora", "saiu há N min" ou "pelo site" — o que o cabeçalho
    /// mostra no lugar do número, no canal do site (`textoDaPresenca`).
    pub fn presenca(&self) -> String {
        if self.na_pagina {
            return "na página agora".into();
        }
        match self.saiu_ha_minutos {
            Some(minutos) => format!("saiu há {minutos} min"),
            None => "pelo site".into(),
        }
    }

    /// A cor do avatar no WhatsApp: o volume sem resposta (`prioridadeDaConversa`).
    pub fn prioridade(&self) -> Prioridade {
        if self.sem_resposta > 5 {
            Prioridade::Alta
        } else if self.sem_resposta > 2 {
            Prioridade::Media
        } else {
            Prioridade::Baixa
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prioridade {
    Alta,
    Media,
    Baixa,
}

fn data(valor: Option<&Value>) -> Option<DateTime<Utc>> {
    let texto = valor?.as_str()?;
    DateTime::parse_from_rfc3339(texto)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn texto(valor: &Value, campo: &str) -> Option<String> {
    valor.get(campo)?.as_str().map(str::to_string)
}

/// A página do WhatsApp: as conversas, as mensagens de todas elas, o total e
/// se há mais (`GET /whatsapp/conversations`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PaginaDoWhatsApp {
    pub conversas: Vec<Conversa>,
    pub mensagens: Vec<Mensagem>,
    pub total: i64,
}

pub fn pagina_do_whatsapp(valor: &Value) -> PaginaDoWhatsApp {
    let conversas = valor
        .get("conversations")
        .and_then(Value::as_array)
        .map(|lista| {
            lista
                .iter()
                .filter_map(|c| {
                    let id = texto(c, "contact_id")?;
                    let nome = texto(c, "profile_name").unwrap_or_else(|| id.clone());
                    let mut conversa = Conversa::vazia(Chave::nova(Canal::WhatsApp, id), nome);
                    conversa.previa = texto(c, "last_message");
                    conversa.quando = data(c.get("last_message_time"));
                    conversa.atendimento_humano =
                        c.get("is_paused").and_then(Value::as_bool).unwrap_or(false);
                    conversa.sem_resposta =
                        c.get("unread_count").and_then(Value::as_i64).unwrap_or(0);
                    conversa.mensagens = c.get("message_count").and_then(Value::as_i64);
                    conversa.so_automaticas = c
                        .get("has_only_automated_messages")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    conversa.ultima_entrada = data(c.get("ultima_entrada"));
                    Some(conversa)
                })
                .collect()
        })
        .unwrap_or_default();
    let mensagens = valor
        .get("all_messages")
        .and_then(Value::as_array)
        .map(|lista| lista.iter().filter_map(mensagem_do_whatsapp).collect())
        .unwrap_or_default();
    PaginaDoWhatsApp {
        conversas,
        mensagens,
        total: valor
            .get("total_count")
            .and_then(Value::as_i64)
            .unwrap_or(0),
    }
}

/// A lista de um dos outros quatro canais (`GET /<canal>/conversas`).
pub fn conversas_do_canal(canal: Canal, valor: &Value) -> Vec<Conversa> {
    valor
        .as_array()
        .map(|lista| {
            lista
                .iter()
                .filter_map(|c| {
                    let id = texto(c, "contato")?;
                    let nome = texto(c, "nome").unwrap_or_else(|| id.clone());
                    let mut conversa = Conversa::vazia(Chave::nova(canal, id), nome);
                    conversa.previa = texto(c, "ultima_mensagem");
                    conversa.quando = data(c.get("ultima_atividade"));
                    conversa.atendimento_humano = c
                        .get("assumida_por_humano")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    conversa.na_pagina =
                        c.get("na_pagina").and_then(Value::as_bool).unwrap_or(false);
                    conversa.saiu_ha_minutos = c.get("saiu_ha_minutos").and_then(Value::as_i64);
                    Some(conversa)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Sem acento e em minúsculas — "joao" acha "João", como no site.
pub fn normalizar(texto: &str) -> String {
    texto
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            outro => outro,
        })
        .collect()
}

/// A lista que o painel mostra: os cinco canais fundidos, filtrados e do mais
/// recente para o mais antigo (`conversasUnificadas`).
///
/// # A assimetria é herdada, não inventada
///
/// - **Busca**: no WhatsApp ela já aconteceu no servidor; aqui só os outros
///   canais são peneirados, porque a lista deles veio inteira.
/// - **"Não lidas"**: só o WhatsApp conta não-respondidas — os outros canais
///   saem da lista em vez de fingirem que 0 é "em dia".
/// - **Sem data** vai para o fim: sem recência, não fura a fila de quem tem.
pub fn conversas_unificadas(
    todas: &[Conversa],
    canal: FiltroDeCanal,
    status: Status,
    busca: &str,
) -> Vec<Conversa> {
    let alvo = normalizar(busca.trim());
    let mut lista: Vec<Conversa> = todas
        .iter()
        .filter(|c| canal.aceita(c.chave.canal))
        .filter(|c| {
            if c.chave.canal == Canal::WhatsApp {
                return true;
            }
            if status == Status::NaoLidas {
                return false;
            }
            alvo.is_empty()
                || [
                    c.nome.as_str(),
                    c.chave.id.as_str(),
                    c.previa.as_deref().unwrap_or(""),
                ]
                .iter()
                .any(|campo| normalizar(campo).contains(&alvo))
        })
        .filter(|c| match status {
            Status::Todas => true,
            Status::Bot => !c.atendimento_humano,
            Status::Humano => c.atendimento_humano,
            Status::NaoLidas => c.sem_resposta > 0,
        })
        .cloned()
        .collect();
    // O site concatena os canais nesta ordem e depois ordena com `sort`, que é
    // estável: no empate vale a ordem da concatenação. `sort_by` também é
    // estável, e a ordem de entrada aqui não é contrato — por isso o canal
    // entra na conta.
    lista.sort_by(|a, b| {
        let recencia = match (a.quando, b.quando) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(x), Some(y)) => y.cmp(&x),
        };
        recencia.then(
            a.chave
                .canal
                .ordem_na_fusao()
                .cmp(&b.chave.canal.ordem_na_fusao()),
        )
    });
    lista
}

/// Quantas conversas cada aba tem, contadas sobre a lista **sem** o recorte
/// de aba — é o que diz "tem gente esperando ali" sem obrigar a trocar.
pub fn contagem_por_canal(todas: &[Conversa], status: Status, busca: &str) -> [(Canal, usize); 5] {
    let lista = conversas_unificadas(todas, FiltroDeCanal::Todas, status, busca);
    Canal::TODOS.map(|canal| {
        (
            canal,
            lista.iter().filter(|c| c.chave.canal == canal).count(),
        )
    })
}

// ── Mensagens ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Mensagem {
    pub id: String,
    /// De quem é, no WhatsApp (as mensagens da página vêm misturadas).
    pub contato: Option<String>,
    pub texto: String,
    pub quando: Option<DateTime<Utc>>,
    /// Do estúdio (bot ou operador) para o cliente.
    pub saida: bool,
    pub automatica: bool,
    pub tipo_da_automacao: Option<String>,
}

impl Mensagem {
    /// "resposta do bot", "lembrete de voucher"… (`rotuloDaAutomacao`).
    pub fn rotulo_da_automacao(&self) -> Option<String> {
        if !self.automatica {
            return None;
        }
        Some(match self.tipo_da_automacao.as_deref() {
            None | Some("") => "automática".into(),
            Some("bot") => "resposta do bot".into(),
            Some("voucher-reminder") => "lembrete de voucher".into(),
            Some("booking-reminder") => "lembrete de agendamento".into(),
            Some("voucher-generation") => "voucher emitido".into(),
            Some(outro) => format!("automática ({outro})"),
        })
    }
}

fn mensagem_do_whatsapp(m: &Value) -> Option<Mensagem> {
    Some(Mensagem {
        id: texto(m, "id")?,
        contato: texto(m, "recipient_id"),
        texto: texto(m, "message").unwrap_or_default(),
        quando: data(m.get("timestamp")),
        // `eDeSaida`: tudo que não é `received` saiu daqui — enviada,
        // entregue, lida ou falhada.
        saida: m.get("event").and_then(Value::as_str) != Some("received"),
        automatica: m
            .get("is_automated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        tipo_da_automacao: texto(m, "automation_type"),
    })
}

/// O histórico de um dos outros canais (`GET /<canal>/conversas/{c}/mensagens`).
pub fn mensagens_do_canal(valor: &Value) -> Vec<Mensagem> {
    valor
        .as_array()
        .map(|lista| {
            lista
                .iter()
                .filter_map(|m| {
                    Some(Mensagem {
                        id: texto(m, "id")?,
                        contato: None,
                        texto: texto(m, "conteudo").unwrap_or_default(),
                        quando: data(m.get("em")),
                        saida: texto(m, "direcao")
                            .is_some_and(|d| d.eq_ignore_ascii_case("OUTBOUND")),
                        automatica: false,
                        tipo_da_automacao: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Quantas mensagens a conversa mostra antes do "Carregar anteriores".
pub const MENSAGENS_POR_PADRAO: usize = 50;

/// As mensagens visíveis da conversa aberta e quantas ficaram escondidas.
///
/// No WhatsApp as mensagens da página chegam misturadas: separa as do
/// contato e ordena pelo instante. Nos outros canais o histórico já é dele,
/// em ordem.
pub fn historico(mensagens: &[Mensagem], aberta: &Chave, tudo: bool) -> (Vec<Mensagem>, usize) {
    let mut dele: Vec<Mensagem> = if aberta.canal == Canal::WhatsApp {
        let mut dele: Vec<Mensagem> = mensagens
            .iter()
            .filter(|m| m.contato.as_deref() == Some(aberta.id.as_str()))
            .cloned()
            .collect();
        dele.sort_by_key(|m| m.quando);
        dele
    } else {
        mensagens.to_vec()
    };
    if tudo || dele.len() <= MENSAGENS_POR_PADRAO {
        return (dele, 0);
    }
    let escondidas = dele.len() - MENSAGENS_POR_PADRAO;
    (dele.split_off(escondidas), escondidas)
}

// ── A janela de 24 horas da Meta ───────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Janela {
    /// O contato nunca escreveu.
    NuncaAbriu,
    Aberta {
        ultima_entrada: DateTime<Utc>,
        restante: Duration,
    },
    Fechada {
        ultima_entrada: DateTime<Utc>,
    },
    /// Ainda não se sabe (o histórico está chegando): não trava ninguém.
    Desconhecida,
}

impl Janela {
    pub fn avaliar(ultima_entrada: Option<DateTime<Utc>>, agora: DateTime<Utc>) -> Janela {
        let Some(entrada) = ultima_entrada else {
            return Janela::NuncaAbriu;
        };
        let restante = entrada + Duration::hours(24) - agora;
        if restante > Duration::zero() {
            Janela::Aberta {
                ultima_entrada: entrada,
                restante,
            }
        } else {
            Janela::Fechada {
                ultima_entrada: entrada,
            }
        }
    }

    /// A janela a partir do histórico: a última mensagem **do cliente**.
    pub fn das_mensagens(mensagens: &[Mensagem], agora: DateTime<Utc>) -> Janela {
        let ultima = mensagens
            .iter()
            .filter(|m| !m.saida)
            .filter_map(|m| m.quando)
            .max();
        Janela::avaliar(ultima, agora)
    }
}

/// "3h 20min", "45min", "menos de 1min" (`formatarRestante`).
pub fn formatar_restante(restante: Duration) -> String {
    if restante <= Duration::zero() {
        return "encerrada".into();
    }
    let minutos = restante.num_minutes();
    if minutos < 1 {
        return "menos de 1min".into();
    }
    let (horas, resto) = (minutos / 60, minutos % 60);
    match (horas, resto) {
        (0, m) => format!("{m}min"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}min"),
    }
}

/// Abaixo disto, a janela aberta ganha a faixa de aviso.
const AVISAR_ABAIXO_DE: i64 = 3;

/// O que o compositor faz: some e dá lugar ao motivo, ou fica com um aviso.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DecisaoDoCompositor {
    pub bloqueado: bool,
    pub motivo: Option<String>,
    pub aviso: Option<String>,
}

pub fn decisao_do_compositor(canal: Canal, janela: Janela) -> DecisaoDoCompositor {
    if !canal.sujeito_a_janela() {
        return DecisaoDoCompositor::default();
    }
    match janela {
        Janela::Desconhecida => DecisaoDoCompositor::default(),
        Janela::Aberta { restante, .. } => DecisaoDoCompositor {
            bloqueado: false,
            motivo: None,
            aviso: (restante < Duration::hours(AVISAR_ABAIXO_DE)).then(|| {
                format!(
                    "Resta {} para responder dentro da janela de 24h da Meta.",
                    formatar_restante(restante)
                )
            }),
        },
        Janela::NuncaAbriu => DecisaoDoCompositor {
            bloqueado: true,
            motivo: Some(
                "Este contato nunca enviou mensagem. A Meta só entrega mensagem de sessão para \
                 quem escreveu nas últimas 24 horas."
                    .into(),
            ),
            aviso: None,
        },
        Janela::Fechada { ultima_entrada } => DecisaoDoCompositor {
            bloqueado: true,
            motivo: Some(format!(
                "A janela de 24 horas da Meta fechou — a última mensagem do cliente foi em {}. \
                 Só dá para responder depois que ele escrever de novo.",
                ultima_entrada
                    .with_timezone(&fuso_do_estudio())
                    .format("%d/%m, %H:%M")
            )),
            aviso: None,
        },
    }
}

// ── Relógio ────────────────────────────────────────────────────────────────

/// `14:05`, no fuso do estúdio.
pub fn hora(quando: DateTime<Utc>) -> String {
    quando
        .with_timezone(&fuso_do_estudio())
        .format("%H:%M")
        .to_string()
}

fn dia(quando: DateTime<Utc>) -> chrono::NaiveDate {
    quando.with_timezone(&fuso_do_estudio()).date_naive()
}

/// A primeira mensagem de outro dia ganha separador.
pub fn mudou_o_dia(quando: DateTime<Utc>, anterior: Option<DateTime<Utc>>) -> bool {
    anterior.is_none_or(|a| dia(a) != dia(quando))
}

/// "Hoje", "Ontem" ou `dd/mm/aaaa`.
pub fn rotulo_do_dia(quando: DateTime<Utc>, agora: DateTime<Utc>) -> String {
    let d = dia(quando);
    if d == dia(agora) {
        return "Hoje".into();
    }
    if d == dia(agora - Duration::hours(24)) {
        return "Ontem".into();
    }
    d.format("%d/%m/%Y").to_string()
}

/// O canto da linha: a hora se for hoje, senão o rótulo do dia.
pub fn carimbo(quando: DateTime<Utc>, agora: DateTime<Utc>) -> String {
    let rotulo = rotulo_do_dia(quando, agora);
    if rotulo == "Hoje" {
        hora(quando)
    } else {
        rotulo
    }
}

/// A letra do avatar: a primeira, se for letra ASCII; senão `?`.
pub fn inicial(nome: &str) -> String {
    match nome.trim().chars().next() {
        Some(c) if c.is_ascii_alphabetic() => c.to_ascii_uppercase().to_string(),
        _ => "?".into(),
    }
}

/// `(54) 99999-1234` para número brasileiro; o resto como veio.
pub fn formatar_whatsapp(numero: &str) -> String {
    let digitos: String = numero.chars().filter(char::is_ascii_digit).collect();
    let Some(nacional) = digitos.strip_prefix("55") else {
        return numero.into();
    };
    match nacional.len() {
        11 => format!(
            "({}) {}-{}",
            &nacional[..2],
            &nacional[2..7],
            &nacional[7..]
        ),
        10 => format!(
            "({}) {}-{}",
            &nacional[..2],
            &nacional[2..6],
            &nacional[6..]
        ),
        _ => numero.into(),
    }
}

// ── O tempo real ───────────────────────────────────────────────────────────

/// Um evento de qualquer um dos cinco fluxos, já com o canal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventoDoChatbot {
    pub canal: Canal,
    pub tipo: String,
    pub contato: String,
    pub nome: Option<String>,
    pub previa: Option<String>,
}

impl EventoDoChatbot {
    pub fn ler(canal: Canal, dados: &Value) -> Option<Self> {
        Some(Self {
            canal,
            tipo: texto(dados, "tipo")?,
            contato: texto(dados, "contato")?,
            nome: texto(dados, "nome"),
            previa: texto(dados, "previa"),
        })
    }

    pub fn chave(&self) -> Chave {
        Chave::nova(self.canal, self.contato.clone())
    }

    /// Mensagem do cliente: acende a linha e pode avisar.
    pub fn e_mensagem_recebida(&self) -> bool {
        self.tipo == "mensagem_recebida"
    }

    fn quem(&self) -> String {
        self.nome
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| match self.canal {
                Canal::Web => "Visitante".into(),
                _ => self.contato.clone(),
            })
    }

    /// O toast de dentro do app (`avisar` do `tempo-real.ts`): título e
    /// descrição. `None` quando não é mensagem do cliente.
    pub fn toast(&self) -> Option<(String, Option<String>)> {
        if !self.e_mensagem_recebida() {
            return None;
        }
        let titulo = match self.canal {
            Canal::WhatsApp => format!("{} no WhatsApp", self.quem()),
            Canal::Web => format!("{} no site", self.quem()),
            Canal::Messenger => "Nova mensagem no Messenger".into(),
            Canal::Telegram => "Nova mensagem no Telegram".into(),
            Canal::Instagram => "Nova mensagem no Instagram".into(),
        };
        Some((titulo, self.previa.clone()))
    }

    /// O aviso do sistema (`notificar` do `tempo-real.ts`).
    pub fn notificacao(&self) -> Option<crate::tempo_real::Aviso> {
        if !self.e_mensagem_recebida() {
            return None;
        }
        let titulo = match self.canal {
            Canal::WhatsApp => format!("{} no WhatsApp", self.quem()),
            Canal::Web => format!("{} está no site", self.quem()),
            Canal::Telegram => "Telegram".into(),
            Canal::Messenger => "Messenger".into(),
            Canal::Instagram => "Instagram Direct".into(),
        };
        Some(crate::tempo_real::Aviso {
            titulo,
            corpo: self.previa.clone().unwrap_or_default(),
            destino: format!("chatbot:{}", self.chave().texto()),
        })
    }
}

// ── Urgências ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Urgencia {
    pub id: String,
    pub contact_id: String,
    #[serde(default)]
    pub profile_name: String,
    pub reason: String,
    #[serde(default)]
    pub urgency_score: i64,
    #[serde(default)]
    pub context_summary: Option<String>,
    pub status: String,
    #[serde(default)]
    pub waiting_time_minutes: i64,
    #[serde(default)]
    pub priority_level: String,
    #[serde(default)]
    pub message_count: i64,
}

impl Urgencia {
    pub fn lista(valor: &Value) -> Vec<Urgencia> {
        valor
            .as_array()
            .map(|l| {
                l.iter()
                    .filter_map(|u| serde_json::from_value(u.clone()).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// O cliente **disse** que quer uma pessoa — é o que pulsa.
    pub fn pediu_atendente(&self) -> bool {
        self.reason == "TRANSFER_TO_HUMAN"
    }

    pub fn pendente(&self) -> bool {
        self.status == "PENDING"
    }

    pub fn em_atendimento(&self) -> bool {
        self.status == "IN_PROGRESS"
    }

    /// O motivo em português (`MOTIVOS`); o desconhecido vai cru.
    pub fn motivo(&self) -> String {
        match self.reason.as_str() {
            "TRANSFER_TO_HUMAN" => "pediu para falar com uma pessoa",
            "FRUSTRATION_DETECTED" => "frustração detectada",
            "MULTIPLE_UNANSWERED" => "várias mensagens sem resposta",
            "CRITICAL_KEYWORDS" => "palavra-chave crítica",
            "CONVERSATION_LOOP" => "conversa em looping",
            "PAYMENT_ISSUE" => "problema no pagamento",
            "VOUCHER_ISSUE" => "problema com voucher",
            "BOOKING_ISSUE" => "problema no agendamento",
            "MANUAL_FLAG" => "marcado à mão",
            outro => outro,
        }
        .into()
    }

    /// O selo da espera (`ROTULO_DA_PRIORIDADE`): mede tempo, não gravidade.
    pub fn rotulo_da_prioridade(&self) -> &'static str {
        match self.priority_level.as_str() {
            "CRITICAL" => "espera +1h",
            "HIGH" => "espera +30min",
            "MEDIUM" => "espera +15min",
            _ => "chegou agora",
        }
    }

    /// "1h 5min" ou "12min".
    pub fn espera(&self) -> String {
        let (h, m) = (
            self.waiting_time_minutes / 60,
            self.waiting_time_minutes % 60,
        );
        if h > 0 {
            format!("{h}h {m}min")
        } else {
            format!("{m}min")
        }
    }
}

/// A frase de erro da API, como o `explicar` do `actions.ts` do site: `403` e
/// `404` têm frase própria; `400`, `409`, `422` e `503` repassam a do
/// servidor (é ele quem sabe que a janela fechou); o resto é o `padrao`.
///
/// O erro chega do cliente HTTP como `o site respondeu 409: <mensagem>`.
pub fn explicar(erro: &str, padrao: &str) -> String {
    let Some(resto) = erro.split("o site respondeu ").nth(1) else {
        return padrao.into();
    };
    let (status, mensagem) = resto.split_once(": ").unwrap_or((resto, ""));
    let status = status.split_whitespace().next().unwrap_or_default();
    match status {
        "403" => "Sua conta não pode fazer isso.".into(),
        "404" => "Esse registro não existe mais.".into(),
        "400" | "409" | "422" | "503" if !mensagem.trim().is_empty() => mensagem.trim().into(),
        _ => padrao.into(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    fn t(texto: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(texto)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn conversa(canal: Canal, id: &str, nome: &str, quando: Option<&str>) -> Conversa {
        let mut c = Conversa::vazia(Chave::nova(canal, id), nome.into());
        c.quando = quando.map(t);
        c
    }

    #[test]
    fn a_chave_vai_e_volta_e_a_torta_vira_nada() {
        let chave = Chave::nova(Canal::Web, "sess-1");
        assert_eq!(chave.texto(), "web:sess-1");
        assert_eq!(Chave::do_texto("web:sess-1"), Some(chave));
        assert_eq!(
            Chave::do_texto("wa:5554:9"),
            Some(Chave::nova(Canal::WhatsApp, "5554:9")),
            "só o primeiro `:` separa"
        );
        for torta in ["", "wa", "wa:", "xx:1", ":1"] {
            assert_eq!(Chave::do_texto(torta), None, "{torta:?}");
        }
        for canal in Canal::TODOS {
            assert_eq!(Canal::da_fonte(canal.prefixo()), Some(canal));
        }
    }

    #[test]
    fn so_os_canais_da_meta_tem_janela() {
        assert!(Canal::WhatsApp.sujeito_a_janela());
        assert!(Canal::Instagram.sujeito_a_janela());
        assert!(Canal::Messenger.sujeito_a_janela());
        assert!(!Canal::Telegram.sujeito_a_janela());
        assert!(!Canal::Web.sujeito_a_janela());
    }

    #[test]
    fn a_pagina_do_whatsapp_le_o_contrato_da_api() {
        let pagina = pagina_do_whatsapp(&json!({
            "conversations": [{
                "contact_id": "5554999991234", "profile_name": "Ana", "last_message": "oi",
                "last_message_time": "2026-09-25T13:00:00Z", "message_count": 7,
                "unread_count": 3, "is_paused": true, "has_only_automated_messages": false,
                "ultima_entrada": "2026-09-25T12:59:00Z"
            }, {"sem_contact_id": true}],
            "total_count": 31, "has_more": true,
            "all_messages": [{
                "id": "m1", "recipient_id": "5554999991234", "event": "received",
                "profile_name": "Ana", "message_id": "wamid", "message": "oi",
                "timestamp": "2026-09-25T13:00:00Z", "is_edited": false,
                "is_automated": false, "automation_type": null,
                "created_at": "2026-09-25T13:00:00Z"
            }, {
                "id": "m2", "recipient_id": "5554999991234", "event": "delivered",
                "message": "olá!", "timestamp": "2026-09-25T13:01:00Z",
                "is_automated": true, "automation_type": "bot"
            }]
        }));
        assert_eq!(pagina.total, 31);
        assert_eq!(
            pagina.conversas.len(),
            1,
            "linha sem contato é descartada, não derruba"
        );
        let ana = &pagina.conversas[0];
        assert_eq!(ana.chave, Chave::nova(Canal::WhatsApp, "5554999991234"));
        assert!(ana.atendimento_humano);
        assert_eq!(ana.sem_resposta, 3);
        assert_eq!(ana.mensagens, Some(7));
        assert_eq!(ana.ultima_entrada, Some(t("2026-09-25T12:59:00Z")));
        assert_eq!(ana.prioridade(), Prioridade::Media);
        assert!(!pagina.mensagens[0].saida, "received é do cliente");
        assert!(pagina.mensagens[1].saida, "delivered saiu daqui");
        assert_eq!(
            pagina.mensagens[1].rotulo_da_automacao().as_deref(),
            Some("resposta do bot")
        );
        assert_eq!(
            pagina_do_whatsapp(&json!("lixo")),
            PaginaDoWhatsApp::default()
        );
    }

    #[test]
    fn os_outros_canais_leem_o_contrato_em_portugues() {
        let lista = conversas_do_canal(
            Canal::Web,
            &json!([{
                "contato": "sess-9", "nome": "Visitante 12", "ultima_mensagem": "tem horário?",
                "ultima_atividade": "2026-09-25T10:00:00Z", "assumida_por_humano": false,
                "sessao_id": "x", "na_pagina": false, "saiu_ha_minutos": 4
            }]),
        );
        assert_eq!(lista[0].chave, Chave::nova(Canal::Web, "sess-9"));
        assert_eq!(lista[0].previa.as_deref(), Some("tem horário?"));
        assert_eq!(lista[0].presenca(), "saiu há 4 min");
        let mut na_pagina = lista[0].clone();
        na_pagina.na_pagina = true;
        assert_eq!(na_pagina.presenca(), "na página agora");
        na_pagina.na_pagina = false;
        na_pagina.saiu_ha_minutos = None;
        assert_eq!(na_pagina.presenca(), "pelo site");

        let mensagens = mensagens_do_canal(&json!([
            {"id": "1", "message_id": "a", "direcao": "INBOUND", "conteudo": "oi", "em": "2026-09-25T10:00:00Z"},
            {"id": "2", "message_id": "b", "direcao": "outbound", "conteudo": "olá", "em": "2026-09-25T10:01:00Z"}
        ]));
        assert!(!mensagens[0].saida);
        assert!(mensagens[1].saida, "a direção é conferida sem caixa");
    }

    #[test]
    fn a_lista_funde_ordena_e_poe_os_sem_data_no_fim() {
        let todas = vec![
            conversa(Canal::Instagram, "ig1", "Bia", None),
            conversa(Canal::WhatsApp, "wa1", "Ana", Some("2026-09-25T10:00:00Z")),
            conversa(Canal::Web, "w1", "Visitante", Some("2026-09-25T12:00:00Z")),
            conversa(Canal::Telegram, "tg1", "Caio", Some("2026-09-24T12:00:00Z")),
        ];
        let lista = conversas_unificadas(&todas, FiltroDeCanal::Todas, Status::Todas, "");
        let ids: Vec<&str> = lista.iter().map(|c| c.chave.id.as_str()).collect();
        assert_eq!(ids, ["w1", "wa1", "tg1", "ig1"]);

        let so_site =
            conversas_unificadas(&todas, FiltroDeCanal::So(Canal::Web), Status::Todas, "");
        assert_eq!(so_site.len(), 1);
    }

    #[test]
    fn a_busca_peneira_os_outros_canais_sem_acento_e_nao_o_whatsapp() {
        let mut joao = conversa(Canal::Instagram, "ig1", "João", None);
        joao.previa = Some("quero agendar".into());
        let todas = vec![
            joao,
            conversa(Canal::Messenger, "ms1", "Maria", None),
            // O WhatsApp já veio buscado do servidor: não é peneirado de novo.
            conversa(Canal::WhatsApp, "wa1", "Zé", None),
        ];
        let achados: Vec<String> =
            conversas_unificadas(&todas, FiltroDeCanal::Todas, Status::Todas, "  JOAO ")
                .into_iter()
                .map(|c| c.chave.id)
                .collect();
        assert_eq!(achados, ["wa1", "ig1"]);
        let pela_previa =
            conversas_unificadas(&todas, FiltroDeCanal::Todas, Status::Todas, "agendar");
        assert_eq!(pela_previa.len(), 2);
        let pelo_id = conversas_unificadas(&todas, FiltroDeCanal::Todas, Status::Todas, "ms1");
        assert!(pelo_id.iter().any(|c| c.chave.id == "ms1"));
    }

    #[test]
    fn os_filtros_de_status_e_a_assimetria_das_nao_lidas() {
        let mut humano = conversa(Canal::Instagram, "ig1", "A", None);
        humano.atendimento_humano = true;
        let mut com_pendencia = conversa(Canal::WhatsApp, "wa1", "B", None);
        com_pendencia.sem_resposta = 2;
        let em_dia = conversa(Canal::WhatsApp, "wa2", "C", None);
        let todas = vec![
            humano,
            com_pendencia,
            em_dia,
            conversa(Canal::Web, "w", "D", None),
        ];

        let ids = |s| -> Vec<String> {
            conversas_unificadas(&todas, FiltroDeCanal::Todas, s, "")
                .into_iter()
                .map(|c| c.chave.id)
                .collect()
        };
        assert_eq!(ids(Status::Humano), ["ig1"]);
        assert_eq!(ids(Status::Bot), ["wa1", "wa2", "w"]);
        assert_eq!(
            ids(Status::NaoLidas),
            ["wa1"],
            "os outros canais não medem: saem, em vez de fingir 'em dia'"
        );

        let contagem = contagem_por_canal(&todas, Status::Todas, "");
        assert_eq!(contagem[0], (Canal::WhatsApp, 2));
        assert_eq!(contagem[1], (Canal::Instagram, 1));
        assert_eq!(contagem[4], (Canal::Web, 1));
        assert_eq!(Status::Humano.para_o_whatsapp(), Some("manual"));
        assert_eq!(Status::Todas.para_o_whatsapp(), None);
    }

    fn msg(id: &str, contato: &str, quando: &str) -> Mensagem {
        Mensagem {
            id: id.into(),
            contato: Some(contato.into()),
            texto: id.into(),
            quando: Some(t(quando)),
            saida: false,
            automatica: false,
            tipo_da_automacao: None,
        }
    }

    #[test]
    fn o_historico_do_whatsapp_separa_o_contato_ordena_e_corta_pelo_fim() {
        let mut todas: Vec<Mensagem> = (0..60)
            .map(|i| {
                msg(
                    &format!("a{i:02}"),
                    "ana",
                    &format!("2026-09-25T10:{i:02}:00Z"),
                )
            })
            .collect();
        todas.reverse();
        todas.push(msg("b1", "bia", "2026-09-25T09:00:00Z"));
        let ana = Chave::nova(Canal::WhatsApp, "ana");

        let (visiveis, escondidas) = historico(&todas, &ana, false);
        assert_eq!(escondidas, 10);
        assert_eq!(visiveis.len(), MENSAGENS_POR_PADRAO);
        assert_eq!(visiveis.first().unwrap().id, "a10", "as 50 mais recentes");
        assert_eq!(visiveis.last().unwrap().id, "a59");

        let (tudo, nenhuma) = historico(&todas, &ana, true);
        assert_eq!((tudo.len(), nenhuma), (60, 0));
        assert_eq!(
            historico(&todas, &Chave::nova(Canal::WhatsApp, "bia"), false)
                .0
                .len(),
            1
        );
    }

    #[test]
    fn a_janela_de_24_horas_decide_o_compositor() {
        let agora = t("2026-09-25T15:00:00Z");
        let aberta = Janela::avaliar(Some(t("2026-09-25T14:00:00Z")), agora);
        assert_eq!(
            decisao_do_compositor(Canal::WhatsApp, aberta),
            DecisaoDoCompositor::default()
        );

        let quase = Janela::avaliar(Some(t("2026-09-24T16:30:00Z")), agora);
        let decisao = decisao_do_compositor(Canal::Instagram, quase);
        assert!(!decisao.bloqueado);
        assert_eq!(
            decisao.aviso.as_deref(),
            Some("Resta 1h 30min para responder dentro da janela de 24h da Meta.")
        );

        let fechada = Janela::avaliar(Some(t("2026-09-24T14:59:00Z")), agora);
        let decisao = decisao_do_compositor(Canal::Messenger, fechada);
        assert!(decisao.bloqueado);
        assert_eq!(
            decisao.motivo.as_deref(),
            Some(
                "A janela de 24 horas da Meta fechou — a última mensagem do cliente foi em \
                 24/09, 11:59. Só dá para responder depois que ele escrever de novo."
            ),
            "a hora sai no fuso do estúdio"
        );

        let nunca = decisao_do_compositor(Canal::WhatsApp, Janela::avaliar(None, agora));
        assert!(nunca.bloqueado);
        assert!(nunca
            .motivo
            .unwrap()
            .starts_with("Este contato nunca enviou mensagem."));

        // Telegram e site não são da Meta; e o desconhecido não trava ninguém.
        assert!(!decisao_do_compositor(Canal::Telegram, fechada).bloqueado);
        assert!(!decisao_do_compositor(Canal::Web, Janela::NuncaAbriu).bloqueado);
        assert!(!decisao_do_compositor(Canal::WhatsApp, Janela::Desconhecida).bloqueado);
    }

    #[test]
    fn a_janela_das_mensagens_olha_so_o_que_o_cliente_escreveu() {
        let agora = t("2026-09-25T15:00:00Z");
        let mut enviada = msg("2", "x", "2026-09-25T14:00:00Z");
        enviada.saida = true;
        let recebida = msg("1", "x", "2026-09-24T10:00:00Z");
        assert_eq!(
            Janela::das_mensagens(&[recebida.clone(), enviada.clone()], agora),
            Janela::Fechada {
                ultima_entrada: t("2026-09-24T10:00:00Z")
            },
            "a resposta do estúdio não reabre a janela"
        );
        assert_eq!(Janela::das_mensagens(&[enviada], agora), Janela::NuncaAbriu);
    }

    #[test]
    fn o_restante_se_escreve_como_no_site() {
        assert_eq!(formatar_restante(Duration::zero()), "encerrada");
        assert_eq!(formatar_restante(Duration::seconds(30)), "menos de 1min");
        assert_eq!(formatar_restante(Duration::minutes(45)), "45min");
        assert_eq!(formatar_restante(Duration::minutes(120)), "2h");
        assert_eq!(formatar_restante(Duration::minutes(125)), "2h 5min");
    }

    #[test]
    fn o_relogio_fala_no_fuso_do_estudio() {
        let agora = t("2026-09-25T15:00:00Z"); // 12:00 no estúdio
        assert_eq!(hora(t("2026-09-25T13:05:00Z")), "10:05");
        assert_eq!(rotulo_do_dia(t("2026-09-25T03:30:00Z"), agora), "Hoje");
        // 02:59 UTC do dia 25 ainda é dia 24 no estúdio.
        assert_eq!(rotulo_do_dia(t("2026-09-25T02:59:00Z"), agora), "Ontem");
        assert_eq!(
            rotulo_do_dia(t("2026-09-20T12:00:00Z"), agora),
            "20/09/2026"
        );
        assert_eq!(carimbo(t("2026-09-25T13:05:00Z"), agora), "10:05");
        assert_eq!(carimbo(t("2026-09-23T13:05:00Z"), agora), "23/09/2026");
        assert!(mudou_o_dia(t("2026-09-25T13:00:00Z"), None));
        assert!(!mudou_o_dia(
            t("2026-09-25T13:00:00Z"),
            Some(t("2026-09-25T03:00:00Z"))
        ));
        assert!(mudou_o_dia(
            t("2026-09-25T03:00:00Z"),
            Some(t("2026-09-25T02:00:00Z"))
        ));
    }

    #[test]
    fn inicial_e_telefone() {
        assert_eq!(inicial("  ana"), "A");
        assert_eq!(inicial("Émerson"), "?", "como o site: só letra ASCII");
        assert_eq!(inicial(""), "?");
        assert_eq!(formatar_whatsapp("5554999991234"), "(54) 99999-1234");
        assert_eq!(formatar_whatsapp("555433331234"), "(54) 3333-1234");
        assert_eq!(formatar_whatsapp("14155550100"), "14155550100");
        assert_eq!(formatar_whatsapp("55123"), "55123");
    }

    #[test]
    fn o_evento_vira_toast_e_aviso_com_os_textos_do_site() {
        let evento = |canal, dados| EventoDoChatbot::ler(canal, &dados).unwrap();
        let wa = evento(
            Canal::WhatsApp,
            json!({"tipo": "mensagem_recebida", "contato": "5554", "nome": " Ana ", "previa": "oi", "em": "x"}),
        );
        assert_eq!(
            wa.toast(),
            Some(("Ana no WhatsApp".into(), Some("oi".into())))
        );
        let aviso = wa.notificacao().unwrap();
        assert_eq!(aviso.titulo, "Ana no WhatsApp");
        assert_eq!(aviso.corpo, "oi");
        assert_eq!(aviso.destino, "chatbot:wa:5554");

        let sem_nome = evento(
            Canal::WhatsApp,
            json!({"tipo": "mensagem_recebida", "contato": "5554", "nome": null}),
        );
        assert_eq!(sem_nome.toast().unwrap().0, "5554 no WhatsApp");

        let site = evento(
            Canal::Web,
            json!({"tipo": "mensagem_recebida", "contato": "s1", "nome": ""}),
        );
        assert_eq!(site.toast().unwrap().0, "Visitante no site");
        assert_eq!(site.notificacao().unwrap().titulo, "Visitante está no site");

        for (canal, toast, aviso) in [
            (
                Canal::Instagram,
                "Nova mensagem no Instagram",
                "Instagram Direct",
            ),
            (Canal::Telegram, "Nova mensagem no Telegram", "Telegram"),
            (Canal::Messenger, "Nova mensagem no Messenger", "Messenger"),
        ] {
            let e = evento(canal, json!({"tipo": "mensagem_recebida", "contato": "c"}));
            assert_eq!(e.toast().unwrap().0, toast);
            assert_eq!(e.notificacao().unwrap().titulo, aviso);
        }

        // Só mensagem do cliente avisa: a nossa saída, pausa e retomada não.
        for tipo in [
            "mensagem_enviada",
            "bot_pausado",
            "conversa_assumida",
            "conversa_atualizada",
        ] {
            let e = evento(Canal::WhatsApp, json!({"tipo": tipo, "contato": "c"}));
            assert!(e.toast().is_none() && e.notificacao().is_none(), "{tipo}");
        }
        assert!(EventoDoChatbot::ler(Canal::WhatsApp, &json!({"tipo": "x"})).is_none());
    }

    #[test]
    fn a_urgencia_le_a_api_e_fala_portugues() {
        let lista = Urgencia::lista(&json!([{
            "id": "u1", "contact_id": "5554", "profile_name": "Ana", "session_id": null,
            "reason": "TRANSFER_TO_HUMAN", "urgency_score": 9, "context_summary": "quer falar",
            "status": "PENDING", "waiting_time_minutes": 65, "priority_level": "CRITICAL",
            "message_count": 4, "already_transferred": false, "created_at": "2026-09-25T10:00:00Z"
        }, {"id": "sem campos obrigatórios"}]));
        assert_eq!(lista.len(), 1);
        let u = &lista[0];
        assert!(u.pediu_atendente() && u.pendente() && !u.em_atendimento());
        assert_eq!(u.motivo(), "pediu para falar com uma pessoa");
        assert_eq!(u.rotulo_da_prioridade(), "espera +1h");
        assert_eq!(u.espera(), "1h 5min");
        let mut outra = u.clone();
        outra.reason = "NOVO_MOTIVO".into();
        outra.waiting_time_minutes = 12;
        outra.priority_level = "LOW".into();
        assert_eq!(outra.motivo(), "NOVO_MOTIVO");
        assert_eq!(outra.espera(), "12min");
        assert_eq!(outra.rotulo_da_prioridade(), "chegou agora");
    }

    #[test]
    fn o_erro_da_api_vira_frase() {
        let padrao = "Não foi possível enviar a mensagem.";
        assert_eq!(
            explicar("o site respondeu 409: Janela de 24h fechada", padrao),
            "Janela de 24h fechada",
            "a frase do servidor passa"
        );
        assert_eq!(
            explicar("o site respondeu 422: inválido", padrao),
            "inválido"
        );
        assert_eq!(
            explicar("o site respondeu 403: proibido", padrao),
            "Sua conta não pode fazer isso."
        );
        assert_eq!(
            explicar("o site respondeu 404: x", padrao),
            "Esse registro não existe mais."
        );
        assert_eq!(
            explicar("o site respondeu 500: pane", padrao),
            padrao,
            "500 não vaza"
        );
        assert_eq!(explicar("o site respondeu 409: ", padrao), padrao);
        assert_eq!(explicar("sem resposta do site: timeout", padrao), padrao);
    }
}
