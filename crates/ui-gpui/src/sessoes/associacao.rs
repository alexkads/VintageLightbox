//! 🔗 **Os campos de associação** — agendamento, voucher, compra antecipada e
//! parceiro: o cartão ("Buscar…" quando vazio, "Trocar"/"Remover" quando há),
//! o modal de busca e o cadastro rápido de parceiro. É o `associacoes/` do
//! site, que o assistente e a gaveta do atendimento usam **os dois**.
//!
//! 🔑 **Um componente, e não uma cópia por tela.** No site a gaveta do
//! atendimento abre os mesmos modais do assistente (`atendimento-da-sessao.tsx`:
//! *"os modais são os mesmos do assistente"*). Aqui a gaveta nasceu como uma
//! lista de texto só de leitura, e a diferença saltava à vista (dono,
//! 2026-09-26: *"o modal de atendimento da web também tá muito diferente no
//! desktop"*). O assistente ainda tem a própria cópia em `nova/` — levá-lo
//! para este componente é o passo seguinte.
//!
//! O componente **não grava nada** na sessão: ele busca, cadastra parceiro e
//! avisa quem escolheu ([`EventoDaAssociacao`]). Quem guarda o formulário
//! decide o que fazer com a escolha — o assistente põe no rascunho, a gaveta
//! manda um `PATCH`.

use crate::campo::TrocarValor as _;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use domain::services::pos_venda::Sessao;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon};
use gpui_kit::{
    div, prelude::*, px, AnyElement, Context, Div, Entity, EventEmitter, FontWeight, Hsla, Pixels,
    SharedString, Subscription, Task, Window,
};

use super::nova::associacoes::{
    self as assoc, AgendamentoEscolhido, CompraEscolhida, ParceiroEscolhido, VoucherEscolhido,
};
use super::nova::estado;
use super::nova::tela::{ItemDaBusca, TipoDeBusca};
use crate::estilo;
use crate::modal::Modal;
use crate::pos_venda::porta::{PedidoJson, Publicador, Recado};
use crate::recursos::Icone;

const VERMELHO: u32 = 0xdc2626;
const ESMERALDA: u32 = 0x059669;
const ESPERA_DA_BUSCA: Duration = Duration::from_millis(300);
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(50);
/// A observação do cadastro, com as palavras do dono (2026-09-13).
const SO_RECORRENTE: &str = "Cadastre apenas se for um parceiro recorrente";

fn cor(hex: u32) -> Hsla {
    gpui_kit::rgb(hex).into()
}

// ── O cartão ─────────────────────────────────────────────────────────────

/// O que o cartão de uma associação mostra: o título, o selo ao lado e até
/// duas linhas apagadas — o `CampoDeAssociacao` do site.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cartao {
    pub titulo: String,
    pub selo: Option<String>,
    pub linhas: Vec<String>,
}

impl Cartao {
    pub fn do_agendamento(a: &AgendamentoEscolhido) -> Self {
        Self {
            titulo: a.nome.clone().unwrap_or_else(|| "Sem nome".into()),
            selo: (!a.status.trim().is_empty())
                .then(|| assoc::rotulo_do_status_do_agendamento(&a.status)),
            linhas: vec![
                assoc::juntar(&[a.quando.as_deref(), a.estudio_nome.as_deref()]),
                assoc::juntar(&[a.whatsapp.as_deref(), a.email.as_deref()]),
            ],
        }
    }

    pub fn do_voucher(v: &VoucherEscolhido) -> Self {
        Self {
            titulo: format!(
                "Nº {} · {}",
                v.numero,
                v.nome.clone().unwrap_or_else(|| "Sem nome".into())
            ),
            selo: Some(match &v.utilizado_em {
                Some(quando) => format!("usado em {quando}"),
                None => "não usado".into(),
            }),
            linhas: vec![
                assoc::juntar(&[v.parceiro.as_deref(), v.agendamento.as_deref()]),
                assoc::juntar(&[v.whatsapp.as_deref(), v.email.as_deref()]),
            ],
        }
    }

    pub fn da_compra(c: &CompraEscolhida) -> Self {
        Self {
            titulo: format!(
                "{} · {}",
                c.comprador_nome
                    .clone()
                    .unwrap_or_else(|| "Comprador sem nome".into()),
                c.total
                    .map(biblioteca_core::dinheiro::formatar)
                    .unwrap_or_default()
            ),
            selo: None,
            linhas: vec![
                assoc::juntar(&[
                    c.pago_em
                        .as_ref()
                        .map(|p| format!("paga em {p}"))
                        .as_deref(),
                    Some(assoc::resumo_dos_itens(&c.itens, 3).as_str()),
                ]),
                assoc::juntar(&[c.comprador_email.as_deref(), c.whatsapp.as_deref()]),
            ],
        }
    }

    pub fn do_parceiro(p: &ParceiroEscolhido) -> Self {
        Self {
            titulo: p.nome.clone(),
            selo: (!p.tipo.trim().is_empty()).then(|| assoc::rotulo_do_tipo_de_parceiro(&p.tipo)),
            linhas: vec![assoc::juntar(&[p.whatsapp.as_deref(), p.email.as_deref()])],
        }
    }

    pub fn do_item(item: &ItemDaBusca) -> Self {
        match item {
            ItemDaBusca::Agendamento(a) => Self::do_agendamento(a),
            ItemDaBusca::Voucher(v) => Self::do_voucher(v),
            ItemDaBusca::Compra(c) => Self::da_compra(c),
            ItemDaBusca::Parceiro(p) => Self::do_parceiro(p),
        }
    }
}

/// O ícone e a frase do estado vazio de cada associação — os do site.
pub fn icone_e_vazio(tipo: TipoDeBusca) -> (Icone, &'static str) {
    match tipo {
        TipoDeBusca::Agendamento => (Icone::CalendarCheck, "Nenhum agendamento associado."),
        TipoDeBusca::Voucher => (Icone::Ticket, "Nenhum voucher associado."),
        TipoDeBusca::Compra => (Icone::ShoppingBag, "Nenhuma compra antecipada associada."),
        TipoDeBusca::Parceiro => (Icone::Handshake, "Nenhum parceiro escolhido."),
    }
}

fn chave(tipo: TipoDeBusca) -> &'static str {
    match tipo {
        TipoDeBusca::Agendamento => "agendamento",
        TipoDeBusca::Voucher => "voucher",
        TipoDeBusca::Compra => "compra",
        TipoDeBusca::Parceiro => "parceiro",
    }
}

fn rotulo_do_pedido(tipo: TipoDeBusca) -> &'static str {
    match tipo {
        TipoDeBusca::Agendamento => "associacao-busca-agendamentos",
        TipoDeBusca::Voucher => "associacao-busca-vouchers",
        TipoDeBusca::Compra => "associacao-busca-compras",
        TipoDeBusca::Parceiro => "associacao-busca-parceiros",
    }
}

fn o_que(tipo: TipoDeBusca) -> &'static str {
    match tipo {
        TipoDeBusca::Agendamento => "agendamentos",
        TipoDeBusca::Voucher => "vouchers",
        TipoDeBusca::Compra => "compras antecipadas",
        TipoDeBusca::Parceiro => "parceiros",
    }
}

/// A frase de uma busca que falhou.
pub fn mensagem_da_busca(erro: &str, o_que: &str) -> String {
    let (status, frase) = estado::ler_erro_do_site(erro);
    match status {
        Some(403) => format!("Sua conta não pode ver {o_que}."),
        Some(404) => format!("A busca de {o_que} ainda não está disponível na API."),
        Some(400) => frase,
        _ => format!("Não foi possível buscar {o_que}. Tente de novo."),
    }
}

/// As quatro colunas de uma linha do modal.
pub fn colunas_do_item(item: &ItemDaBusca) -> [String; 4] {
    match item {
        ItemDaBusca::Agendamento(a) => [
            a.nome.clone().unwrap_or_else(|| "Sem nome".into()),
            assoc::juntar(&[a.whatsapp.as_deref(), a.email.as_deref()]),
            assoc::juntar(&[a.quando.as_deref(), a.estudio_nome.as_deref()]),
            assoc::rotulo_do_status_do_agendamento(&a.status),
        ],
        ItemDaBusca::Voucher(v) => [
            format!(
                "Nº {} · {}",
                v.numero,
                v.nome.clone().unwrap_or_else(|| "Sem nome".into())
            ),
            assoc::juntar(&[v.whatsapp.as_deref(), v.email.as_deref()]),
            assoc::juntar(&[v.criado_em.as_deref(), v.agendamento.as_deref()]),
            match &v.utilizado_em {
                Some(quando) => format!("usado em {quando}"),
                None => "não usado".into(),
            },
        ],
        ItemDaBusca::Compra(c) => [
            assoc::juntar(&[
                Some(
                    c.comprador_nome
                        .clone()
                        .unwrap_or_else(|| "Comprador sem nome".into())
                        .as_str(),
                ),
                Some(assoc::resumo_dos_itens(&c.itens, 3).as_str()),
            ]),
            assoc::juntar(&[c.comprador_email.as_deref(), c.whatsapp.as_deref()]),
            match &c.pago_em {
                Some(p) => format!("paga em {p}"),
                None => "não paga".into(),
            },
            format!(
                "{} · {}",
                assoc::rotulo_do_status_da_compra(&c.status),
                c.total
                    .map(biblioteca_core::dinheiro::formatar)
                    .unwrap_or_default()
            ),
        ],
        ItemDaBusca::Parceiro(p) => [
            p.nome.clone(),
            assoc::juntar(&[p.whatsapp.as_deref(), p.email.as_deref()]),
            assoc::rotulo_do_tipo_de_parceiro(&p.tipo),
            if p.ativo { "ativo" } else { "inativo" }.into(),
        ],
    }
}

// ── O componente ─────────────────────────────────────────────────────────

/// O que o componente avisa a quem o usa.
#[derive(Debug, Clone, PartialEq)]
pub enum EventoDaAssociacao {
    /// Uma linha da busca, ou o parceiro recém-cadastrado.
    Escolheu(Box<ItemDaBusca>),
    /// O "Remover" do cartão.
    Removeu(TipoDeBusca),
}

/// Um tipo de parceiro no `Select` do cadastro.
#[derive(Clone)]
struct TipoDeParceiro {
    valor: String,
    rotulo: SharedString,
}

impl SelectItem for TipoDeParceiro {
    type Value = String;

    fn title(&self) -> SharedString {
        self.rotulo.clone()
    }

    fn value(&self) -> &String {
        &self.valor
    }
}

struct Busca {
    tipo: TipoDeBusca,
    campo: Entity<InputState>,
    carregando: bool,
    erro: Option<String>,
    itens: Vec<ItemDaBusca>,
    com_texto: bool,
    enviados: u32,
    recebidos: u32,
    agendada: Option<Instant>,
    _assinatura: Subscription,
}

struct Cadastro {
    nome: Entity<InputState>,
    whatsapp: Entity<InputState>,
    email: Entity<InputState>,
    tipo: Entity<SelectState<SearchableVec<TipoDeParceiro>>>,
    tipo_escolhido: Option<String>,
    enviando: bool,
    erro: Option<String>,
    existente: Option<ParceiroEscolhido>,
    _assinatura: Subscription,
}

pub struct Associador {
    publicador: Arc<dyn Publicador>,
    sessao: Option<Sessao>,
    recados: (Sender<Recado>, Receiver<Recado>),
    busca: Modal<Busca>,
    cadastro: Option<Cadastro>,
    /// Os ids dos elementos começam por aqui — duas telas com o componente
    /// não dividem id.
    prefixo: &'static str,
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

impl EventEmitter<EventoDaAssociacao> for Associador {}

impl Associador {
    pub fn novo(publicador: Arc<dyn Publicador>, prefixo: &'static str) -> Self {
        Self {
            publicador,
            sessao: None,
            recados: channel(),
            busca: Modal::default(),
            cadastro: None,
            prefixo,
            colhendo: false,
            _colheita: None,
        }
    }

    fn id(&self, resto: impl std::fmt::Display) -> SharedString {
        SharedString::from(format!("{}-{resto}", self.prefixo))
    }

    pub fn definir_sessao(&mut self, sessao: Option<Sessao>) {
        self.sessao = sessao;
    }

    /// Fecha o que estiver aberto sem devolver o foco — quem chama já cuida
    /// dele (a gaveta que fecha, a sessão que troca).
    pub fn largar(&mut self) {
        self.busca.largar();
        self.cadastro = None;
    }

    pub fn busca_aberta(&self) -> bool {
        self.busca.esta_aberto()
    }

    /// 🧪 O tipo da busca aberta.
    pub fn tipo_da_busca(&self) -> Option<TipoDeBusca> {
        self.busca.aberto().map(|b| b.tipo)
    }

    pub fn cadastro_aberto(&self) -> bool {
        self.cadastro.is_some()
    }

    // ── A busca ──────────────────────────────────────────────────────────

    pub fn abrir_busca(&mut self, tipo: TipoDeBusca, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = match tipo {
            TipoDeBusca::Agendamento => "Nome, WhatsApp, e-mail ou estúdio…",
            TipoDeBusca::Voucher => "Número, nome, WhatsApp, e-mail ou parceiro…",
            TipoDeBusca::Compra => "Nome, e-mail, WhatsApp ou número do pedido…",
            TipoDeBusca::Parceiro => "Nome do hotel, pousada, guia…",
        };
        let campo = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let assinatura = cx.subscribe_in(
            &campo,
            window,
            |tela, _, evento: &InputEvent, window, cx| match evento {
                InputEvent::Change => {
                    if let Some(busca) = tela.busca.aberto_mut() {
                        busca.agendada = Some(Instant::now() + ESPERA_DA_BUSCA);
                    }
                    tela.acompanhar(cx);
                }
                InputEvent::PressEnter { .. } => {
                    let primeiro = tela.busca.aberto().and_then(|b| b.itens.first().cloned());
                    if let Some(item) = primeiro {
                        tela.escolher_da_busca(item, window, cx);
                    }
                }
                _ => {}
            },
        );
        // Abre **antes** de focar o campo: é aí que o contrato guarda quem
        // tinha o foco, para devolver ao fechar.
        self.busca.abrir(
            Busca {
                tipo,
                campo,
                carregando: false,
                erro: None,
                itens: Vec::new(),
                com_texto: false,
                enviados: 0,
                recebidos: 0,
                agendada: None,
                _assinatura: assinatura,
            },
            window,
            cx,
        );
        if let Some(busca) = self.busca.aberto() {
            busca.campo.update(cx, |c, cx| c.focus(window, cx));
        }
        self.buscar(cx);
        self.acompanhar(cx);
        cx.notify();
    }

    pub fn fechar_busca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busca.fechar(window, cx);
        // O cadastro aberto no rodapé do modal sai junto com ele.
        self.cadastro = None;
        cx.notify();
    }

    fn buscar(&mut self, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let Some(busca) = self.busca.aberto_mut() else {
            return;
        };
        let texto = busca.campo.read(cx).value().trim().to_string();
        busca.agendada = None;
        busca.carregando = true;
        busca.erro = None;
        busca.com_texto = !texto.is_empty();
        busca.enviados += 1;
        let limite = Some(assoc::LIMITE_DA_BUSCA.to_string());
        let com_texto = (!texto.is_empty()).then_some(texto);
        let caminho = match busca.tipo {
            TipoDeBusca::Agendamento => {
                // Sem texto, a janela em volta de hoje; com texto, qualquer data.
                let janela = com_texto.is_none().then(|| {
                    let hoje = chrono::Utc::now();
                    (
                        (hoje - chrono::Duration::days(assoc::DIAS_ATRAS))
                            .format("%Y-%m-%d")
                            .to_string(),
                        (hoje + chrono::Duration::days(assoc::DIAS_A_FRENTE))
                            .format("%Y-%m-%d")
                            .to_string(),
                    )
                });
                format!(
                    "/pos-venda/busca/agendamentos{}",
                    assoc::consulta(&[
                        ("busca", com_texto),
                        ("de", janela.as_ref().map(|j| j.0.clone())),
                        ("ate", janela.map(|j| j.1)),
                        ("limite", limite),
                    ])
                )
            }
            TipoDeBusca::Voucher => format!(
                "/pos-venda/busca/vouchers{}",
                assoc::consulta(&[("busca", com_texto), ("limite", limite)])
            ),
            TipoDeBusca::Compra => format!(
                "/pos-venda/busca/compras-antecipadas{}",
                assoc::consulta(&[("busca", com_texto), ("limite", limite)])
            ),
            TipoDeBusca::Parceiro => format!(
                "/pos-venda/parceiros{}",
                assoc::consulta(&[("busca", com_texto)])
            ),
        };
        self.publicador.pedir_json(
            sessao,
            PedidoJson::ler(rotulo_do_pedido(busca.tipo), caminho),
            self.recados.0.clone(),
        );
    }

    fn receber_busca(&mut self, tipo: TipoDeBusca, resultado: Result<serde_json::Value, String>) {
        let Some(busca) = self.busca.aberto_mut().filter(|b| b.tipo == tipo) else {
            return;
        };
        busca.recebidos += 1;
        // Só a resposta do último pedido vale: as anteriores são de um texto
        // que o operador já mudou.
        if busca.recebidos < busca.enviados {
            return;
        }
        busca.carregando = false;
        match resultado {
            Ok(valor) => {
                busca.itens = match tipo {
                    TipoDeBusca::Agendamento => {
                        let mut lista = assoc::agendamentos_da_api(&valor);
                        assoc::ordenar_agendamentos(&mut lista, chrono::Utc::now().timestamp());
                        lista.into_iter().map(ItemDaBusca::Agendamento).collect()
                    }
                    TipoDeBusca::Voucher => {
                        let mut lista = assoc::vouchers_da_api(&valor);
                        assoc::ordenar_vouchers(&mut lista);
                        lista.into_iter().map(ItemDaBusca::Voucher).collect()
                    }
                    TipoDeBusca::Compra => {
                        let mut lista = assoc::compras_da_api(&valor);
                        assoc::ordenar_compras(&mut lista);
                        lista.into_iter().map(ItemDaBusca::Compra).collect()
                    }
                    TipoDeBusca::Parceiro => assoc::parceiros_da_api(&valor)
                        .into_iter()
                        .map(ItemDaBusca::Parceiro)
                        .collect(),
                };
            }
            Err(erro) => {
                busca.itens.clear();
                busca.erro = Some(mensagem_da_busca(&erro, o_que(tipo)));
            }
        }
    }

    pub fn escolher_da_busca(
        &mut self,
        item: ItemDaBusca,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.busca.fechar(window, cx);
        self.cadastro = None;
        cx.emit(EventoDaAssociacao::Escolheu(Box::new(item)));
        cx.notify();
    }

    /// 🧪 Os itens da busca aberta.
    pub fn itens_da_busca(&self) -> Vec<ItemDaBusca> {
        self.busca
            .aberto()
            .map(|b| b.itens.clone())
            .unwrap_or_default()
    }

    // ── O cadastro de parceiro ───────────────────────────────────────────

    pub fn abrir_cadastro(
        &mut self,
        nome: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let campo =
            |window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx));
        let nome_campo = campo(window, cx);
        if let Some(nome) = nome {
            nome_campo.update(cx, |c, cx| c.trocar_valor(nome, window, cx));
        }
        let opcoes: Vec<TipoDeParceiro> = assoc::TIPOS_DE_PARCEIRO
            .iter()
            .map(|(v, r)| TipoDeParceiro {
                valor: v.to_string(),
                rotulo: (*r).into(),
            })
            .collect();
        let tipo = cx.new(|cx| SelectState::new(SearchableVec::new(opcoes), None, window, cx));
        let assinatura = cx.subscribe_in(
            &tipo,
            window,
            |tela, _, evento: &SelectEvent<SearchableVec<TipoDeParceiro>>, _, cx| {
                let SelectEvent::Confirm(valor) = evento;
                if let Some(cadastro) = tela.cadastro.as_mut() {
                    cadastro.tipo_escolhido = valor.clone();
                    cx.notify();
                }
            },
        );
        nome_campo.update(cx, |c, cx| c.focus(window, cx));
        self.cadastro = Some(Cadastro {
            nome: nome_campo,
            whatsapp: campo(window, cx),
            email: campo(window, cx),
            tipo,
            tipo_escolhido: None,
            enviando: false,
            erro: None,
            existente: None,
            _assinatura: assinatura,
        });
        cx.notify();
    }

    pub fn fechar_cadastro(&mut self, cx: &mut Context<Self>) {
        self.cadastro = None;
        cx.notify();
    }

    /// 🧪 Escolhe o tipo sem abrir o `Select`.
    pub fn escolher_tipo_do_parceiro(&mut self, tipo: &str, cx: &mut Context<Self>) {
        if let Some(cadastro) = self.cadastro.as_mut() {
            cadastro.tipo_escolhido = Some(tipo.to_string());
            cx.notify();
        }
    }

    /// "Cadastrar e usar".
    pub fn cadastrar_parceiro(&mut self, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        let Some(cadastro) = self.cadastro.as_mut() else {
            return;
        };
        if cadastro.enviando {
            return;
        }
        let nome = cadastro.nome.read(cx).value().trim().to_string();
        let ou = |e: &Entity<InputState>| {
            let v = e.read(cx).value().trim().to_string();
            (!v.is_empty()).then_some(v)
        };
        let (whatsapp, email) = (ou(&cadastro.whatsapp), ou(&cadastro.email));
        cadastro.existente = None;
        if nome.is_empty() {
            cadastro.erro = Some("Informe o nome do parceiro.".into());
            cx.notify();
            return;
        }
        let Some(tipo) = cadastro.tipo_escolhido.clone() else {
            cadastro.erro = Some("Escolha o tipo do parceiro.".into());
            cx.notify();
            return;
        };
        cadastro.erro = None;
        cadastro.enviando = true;
        self.publicador.pedir_json(
            sessao,
            PedidoJson::gravar(
                "associacao-parceiro-criado",
                "POST",
                "/pos-venda/parceiros",
                serde_json::json!({
                    "nome": nome,
                    "tipo": tipo,
                    "whatsapp": whatsapp,
                    "email": email,
                }),
            ),
            self.recados.0.clone(),
        );
        self.acompanhar(cx);
        cx.notify();
    }

    /// O `409`: o existente é procurado pelo nome, para "Usar este".
    fn receber_cadastro(
        &mut self,
        resultado: Result<serde_json::Value, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(cadastro) = self.cadastro.as_mut() else {
            return;
        };
        cadastro.enviando = false;
        match resultado {
            Ok(valor) => match assoc::parceiro_da_api(&valor) {
                Some(parceiro) => {
                    self.cadastro = None;
                    self.busca.fechar_depois(cx);
                    cx.emit(EventoDaAssociacao::Escolheu(Box::new(
                        ItemDaBusca::Parceiro(parceiro),
                    )));
                }
                None => {
                    cadastro.erro =
                        Some("Não foi possível cadastrar o parceiro. Tente de novo.".into());
                }
            },
            Err(erro) => {
                let (status, frase) = estado::ler_erro_do_site(&erro);
                cadastro.erro = Some(match status {
                    Some(409) => {
                        let nome = cadastro.nome.read(cx).value().trim().to_string();
                        if let Some(sessao) = self.sessao.clone() {
                            self.publicador.pedir_json(
                                sessao,
                                PedidoJson::ler(
                                    "associacao-parceiro-existente",
                                    format!(
                                        "/pos-venda/parceiros{}",
                                        assoc::consulta(&[
                                            ("busca", Some(nome)),
                                            ("incluir_inativos", Some("true".into())),
                                        ])
                                    ),
                                ),
                                self.recados.0.clone(),
                            );
                        }
                        "Já existe um parceiro com este nome.".into()
                    }
                    Some(403) => "Sua conta não pode cadastrar parceiros.".into(),
                    Some(400) => frase,
                    _ => "Não foi possível cadastrar o parceiro. Tente de novo.".into(),
                });
            }
        }
    }

    pub fn usar_existente(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(parceiro) = self.cadastro.as_ref().and_then(|c| c.existente.clone()) else {
            return;
        };
        self.escolher_da_busca(ItemDaBusca::Parceiro(parceiro), window, cx);
    }

    // ── A colheita ───────────────────────────────────────────────────────

    fn acompanhar(&mut self, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;
        self._colheita = Some(cx.spawn(async move |esta, cx| loop {
            cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
            let Ok(continua) = esta.update(cx, |tela, cx| tela.colher(cx)) else {
                break;
            };
            if !continua {
                break;
            }
        }));
    }

    /// Drena as respostas e dispara a busca que esperou o fim da digitação.
    /// Devolve se vale continuar acordando.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            let Recado::Json { rotulo, resultado } = recado else {
                continue;
            };
            match rotulo {
                "associacao-parceiro-criado" => self.receber_cadastro(resultado, cx),
                "associacao-parceiro-existente" => {
                    if let (Some(cadastro), Ok(valor)) = (self.cadastro.as_mut(), resultado) {
                        let nome = cadastro.nome.read(cx).value().to_string();
                        cadastro.existente = assoc::achar_parceiro_pelo_nome(
                            &assoc::parceiros_da_api(&valor),
                            &nome,
                        )
                        .cloned();
                    }
                }
                outro => {
                    for tipo in [
                        TipoDeBusca::Agendamento,
                        TipoDeBusca::Voucher,
                        TipoDeBusca::Compra,
                        TipoDeBusca::Parceiro,
                    ] {
                        if rotulo_do_pedido(tipo) == outro {
                            self.receber_busca(tipo, resultado);
                            break;
                        }
                    }
                }
            }
        }
        if let Some(busca) = self.busca.aberto() {
            if busca
                .agendada
                .is_some_and(|quando| Instant::now() >= quando)
            {
                self.buscar(cx);
            }
        }
        if mudou {
            cx.notify();
        }
        let esperando = self
            .busca
            .aberto()
            .is_some_and(|b| b.carregando || b.agendada.is_some())
            || self.cadastro.as_ref().is_some_and(|c| c.enviando);
        if !esperando {
            self.colhendo = false;
        }
        esperando
    }

    // ── O desenho ────────────────────────────────────────────────────────

    /// O cartão de uma associação — o `CampoDeAssociacao` do site: vazio com
    /// "Buscar…", preenchido com "Trocar" e "Remover".
    pub fn campo(
        &self,
        tipo: TipoDeBusca,
        cartao: Option<Cartao>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme();
        let (icone, vazio) = icone_e_vazio(tipo);
        let chave = chave(tipo);
        let buscar = estilo::botao_contorno(self.id(format!("buscar-{chave}")), cx)
            .on_click(cx.listener(move |tela, _, window, cx| tela.abrir_busca(tipo, window, cx)));
        match cartao {
            None => h_flex()
                .gap(px(12.))
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_dashed()
                .border_color(tema.border)
                .child(
                    Icon::new(icone)
                        .size(px(18.))
                        .text_color(tema.muted_foreground),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_sm()
                        .text_color(tema.muted_foreground)
                        .child(vazio),
                )
                .child(
                    buscar
                        .child(Icon::new(Icone::Search).size(px(16.)))
                        .child("Buscar…"),
                )
                .into_any_element(),
            Some(cartao) => h_flex()
                .items_start()
                .gap(px(12.))
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(tema.border)
                .bg(tema.muted.opacity(0.2))
                .child(
                    Icon::new(icone)
                        .size(px(18.))
                        .mt(px(2.))
                        .text_color(cor(ESMERALDA)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.))
                        .gap(px(2.))
                        .child(
                            h_flex()
                                .min_w(px(0.))
                                .gap(px(8.))
                                .child(
                                    div()
                                        .min_w(px(0.))
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .truncate()
                                        .child(cartao.titulo),
                                )
                                .when_some(cartao.selo, |c, selo| {
                                    c.child(estilo::selo_contorno(cx).child(selo))
                                }),
                        )
                        .children(
                            cartao
                                .linhas
                                .into_iter()
                                .filter(|l| !l.trim().is_empty())
                                .take(2)
                                .map(|l| {
                                    div()
                                        .text_xs()
                                        .text_color(tema.muted_foreground)
                                        .truncate()
                                        .child(l)
                                }),
                        ),
                )
                .child(
                    h_flex()
                        .flex_none()
                        .gap(px(4.))
                        .child(
                            estilo::botao_fantasma(self.id(format!("trocar-{chave}")), cx)
                                .child("Trocar")
                                .on_click(cx.listener(move |tela, _, window, cx| {
                                    tela.abrir_busca(tipo, window, cx)
                                })),
                        )
                        .child(
                            estilo::botao_fantasma(self.id(format!("remover-{chave}")), cx)
                                .text_color(tema.muted_foreground)
                                .child("Remover")
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(EventoDaAssociacao::Removeu(tipo));
                                })),
                        ),
                )
                .into_any_element(),
        }
    }

    /// O cadastro rápido de parceiro — embaixo do campo (`no_modal = false`)
    /// ou no rodapé da busca.
    pub fn cadastro_de_parceiro(&self, no_modal: bool, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let Some(cadastro) = self.cadastro.as_ref() else {
            let texto_da_busca = self
                .busca
                .aberto()
                .map(|b| b.campo.read(cx).value().trim().to_string())
                .filter(|t| !t.is_empty());
            let rotulo = match (&texto_da_busca, no_modal) {
                (Some(t), true) => format!("Cadastrar \"{t}\""),
                _ => "Cadastrar novo parceiro".into(),
            };
            return h_flex()
                .gap(px(12.))
                .child(
                    estilo::botao_contorno(self.id("abrir-cadastro"), cx)
                        .child(Icon::new(Icone::Plus).size(px(16.)))
                        .child(rotulo)
                        .on_click(cx.listener(move |tela, _, window, cx| {
                            tela.abrir_cadastro(texto_da_busca.clone(), window, cx)
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(tema.muted_foreground)
                        .child(SO_RECORRENTE),
                )
                .into_any_element();
        };
        let pronto = !cadastro.nome.read(cx).value().trim().is_empty()
            && cadastro.tipo_escolhido.is_some()
            && !cadastro.enviando;
        let campo = |rotulo: &'static str, corpo: AnyElement| {
            v_flex()
                .flex_1()
                .min_w(px(0.))
                .gap(px(6.))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(rotulo),
                )
                .child(corpo)
        };
        v_flex()
            .gap(px(12.))
            .p(px(16.))
            .rounded(px(8.))
            .border_1()
            .border_color(tema.border)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Novo parceiro"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(SO_RECORRENTE),
            )
            .child(
                h_flex()
                    .items_start()
                    .gap(px(12.))
                    .child(campo(
                        "Nome",
                        Input::new(&cadastro.nome).w_full().into_any_element(),
                    ))
                    .child(campo(
                        "Tipo",
                        Select::new(&cadastro.tipo)
                            .placeholder("Escolha…")
                            .w_full()
                            .into_any_element(),
                    )),
            )
            .child(
                h_flex()
                    .items_start()
                    .gap(px(12.))
                    .child(campo(
                        "WhatsApp (opcional)",
                        Input::new(&cadastro.whatsapp).w_full().into_any_element(),
                    ))
                    .child(campo(
                        "E-mail (opcional)",
                        Input::new(&cadastro.email).w_full().into_any_element(),
                    )),
            )
            .when_some(cadastro.erro.clone(), |c, erro| {
                c.child(
                    h_flex()
                        .gap(px(8.))
                        .child(div().text_sm().text_color(cor(VERMELHO)).child(erro))
                        .when_some(cadastro.existente.clone(), |c, p| {
                            c.child(
                                estilo::botao_contorno(self.id("usar-existente"), cx)
                                    .child(format!(
                                        "Usar \"{}\" ({})",
                                        p.nome,
                                        assoc::rotulo_do_tipo_de_parceiro(&p.tipo)
                                    ))
                                    .on_click(cx.listener(|tela, _, window, cx| {
                                        tela.usar_existente(window, cx)
                                    })),
                            )
                        }),
                )
            })
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(
                        estilo::desligado(
                            estilo::botao_primario(self.id("cadastrar-parceiro"), cx).child(
                                if cadastro.enviando {
                                    "Cadastrando…"
                                } else {
                                    "Cadastrar e usar"
                                },
                            ),
                            !pronto,
                        )
                        .when(pronto, |b| {
                            b.on_click(cx.listener(|tela, _, _, cx| tela.cadastrar_parceiro(cx)))
                        }),
                    )
                    .child(
                        estilo::botao_fantasma(self.id("cancelar-cadastro"), cx)
                            .child("Cancelar")
                            .on_click(cx.listener(|tela, _, _, cx| tela.fechar_cadastro(cx))),
                    ),
            )
            .into_any_element()
    }

    /// O miolo do `modal-de-busca.tsx`: véu, caixa, X e `Esc` são do `Dialog`
    /// do gpui-kit (`crate::dialogo`, no `render`).
    fn modal_de_busca(&self, busca: &Busca, altura: Pixels, cx: &mut Context<Self>) -> Div {
        let tema = cx.theme().clone();
        let (titulo, descricao, colunas): (&str, &str, [&str; 4]) = match busca.tipo {
            TipoDeBusca::Agendamento => (
                "Associar agendamento",
                "Sem busca, os mais perto de hoje. Com busca, qualquer data.",
                ["Nome", "Contato", "Quando", "Situação"],
            ),
            TipoDeBusca::Voucher => (
                "Associar voucher",
                "Os não usados primeiro, do mais recente ao mais antigo.",
                ["Voucher", "Contato", "Emitido / agendado", "Uso"],
            ),
            TipoDeBusca::Compra => (
                "Associar compra antecipada",
                "As pagas mais recentes primeiro.",
                ["Comprador e itens", "Contato", "Pagamento", "Situação"],
            ),
            TipoDeBusca::Parceiro => (
                "Parceiro que indicou",
                "Não achou? Cadastre abaixo — nome e tipo bastam.",
                ["Parceiro", "Contato", "Tipo", "Situação"],
            ),
        };
        let vazio = match (busca.tipo, busca.com_texto) {
            (TipoDeBusca::Agendamento, true) => "Nenhum agendamento bate com a busca.",
            (TipoDeBusca::Agendamento, false) => "Nenhum agendamento perto de hoje.",
            (TipoDeBusca::Voucher, true) => "Nenhum voucher bate com a busca.",
            (TipoDeBusca::Voucher, false) => "Nenhum voucher encontrado.",
            (TipoDeBusca::Compra, true) => "Nenhuma compra bate com a busca.",
            (TipoDeBusca::Compra, false) => "Nenhuma compra antecipada encontrada.",
            (TipoDeBusca::Parceiro, true) => "Nenhum parceiro com esse nome.",
            (TipoDeBusca::Parceiro, false) => "Nenhum parceiro cadastrado ainda.",
        };
        let celula = |texto: String, forte: bool| {
            div()
                .flex_1()
                .min_w(px(0.))
                .text_sm()
                .when(forte, |d| d.font_weight(FontWeight::MEDIUM))
                .when(!forte, |d| d.text_color(tema.muted_foreground))
                .truncate()
                .child(texto)
        };
        let linhas: Vec<AnyElement> = busca
            .itens
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let [a, b, c, d] = colunas_do_item(item);
                let item = item.clone();
                h_flex()
                    .id(self.id(format!("resultado-{i}")))
                    .gap(px(12.))
                    .px(px(12.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(tema.border)
                    .cursor_pointer()
                    .hover(|h| h.bg(tema.accent))
                    .child(celula(a, true))
                    .child(celula(b, false))
                    .child(celula(c, false))
                    .child(celula(d, false))
                    .on_click(cx.listener(move |tela, _, window, cx| {
                        tela.escolher_da_busca(item.clone(), window, cx)
                    }))
                    .into_any_element()
            })
            .collect();

        // `max-h-[calc(100dvh-2rem)]`, menos o respiro de 16 da caixa: a lista
        // é quem encolhe, e o campo de busca não sai da vista.
        v_flex()
            .max_h(altura - px(64.))
            .gap(px(12.))
            .child(
                div()
                    .pr(px(24.))
                    .text_size(px(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(titulo),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(tema.muted_foreground)
                    .child(descricao),
            )
            .child(
                Input::new(&busca.campo)
                    .w_full()
                    .prefix(Icon::new(Icone::Search).size(px(16.))),
            )
            .child(
                v_flex()
                    .id(self.id("resultados"))
                    .flex_1()
                    .min_h(px(160.))
                    .overflow_y_scroll()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(tema.border)
                    .child(
                        h_flex()
                            .gap(px(12.))
                            .px(px(12.))
                            .py(px(8.))
                            .border_b_1()
                            .border_color(tema.border)
                            .bg(tema.muted)
                            .children(colunas.iter().map(|c| {
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(tema.muted_foreground)
                                    .child(*c)
                            })),
                    )
                    .map(|lista| {
                        if busca.carregando && busca.itens.is_empty() {
                            lista.child(div().p(px(16.)).text_sm().child("Carregando…"))
                        } else if let Some(erro) = &busca.erro {
                            lista.child(
                                div()
                                    .p(px(16.))
                                    .text_sm()
                                    .text_color(cor(VERMELHO))
                                    .child(erro.clone()),
                            )
                        } else if busca.itens.is_empty() {
                            lista.child(
                                div()
                                    .p(px(16.))
                                    .text_sm()
                                    .text_color(tema.muted_foreground)
                                    .child(vazio),
                            )
                        } else {
                            lista.children(linhas)
                        }
                    }),
            )
            .when(busca.tipo == TipoDeBusca::Parceiro, |c| {
                c.child(self.cadastro_de_parceiro(true, cx))
            })
    }
}

impl Render for Associador {
    /// Só o modal da busca — o cartão e o cadastro são desenhados por quem usa
    /// o componente, no lugar dele ([`Associador::campo`]).
    ///
    /// 🪟 **Por cima da janela inteira**, e não só de quem o contém: o
    /// associador vive dentro da gaveta do atendimento (o `Sheet` do
    /// gpui-kit), e a busca de 768 px não cabe nos 576 dela. O `Dialog` do kit
    /// se ancora no canto da janela, esteja onde estiver na árvore.
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let altura = window.viewport_size().height;
        let miolo = self
            .busca
            .aberto()
            .map(|busca| self.modal_de_busca(busca, altura, cx).into_any_element());
        crate::dialogo::desenhar_conteudo(
            miolo,
            None,
            crate::dialogo::Jeito::dialogo(768.),
            |tela, window, cx| tela.fechar_busca(window, cx),
            window,
            cx,
        )
        .unwrap_or_else(|| div().into_any_element())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_cartao_do_agendamento_leva_o_status_ao_selo() {
        let a = AgendamentoEscolhido {
            id: "a1".into(),
            nome: Some("Maria".into()),
            whatsapp: Some("5199".into()),
            email: None,
            inicio_iso: None,
            quando: Some("13/09/2026 14:00".into()),
            estudio_id: None,
            estudio_nome: Some("Gramado".into()),
            status: "confirmado".into(),
        };
        let c = Cartao::do_agendamento(&a);
        assert_eq!(c.titulo, "Maria");
        assert!(c.selo.is_some());
        assert_eq!(c.linhas[0], "13/09/2026 14:00 · Gramado");
    }

    #[test]
    fn o_parceiro_sem_tipo_nao_tem_selo() {
        let p = ParceiroEscolhido {
            id: "p1".into(),
            nome: "Hotel Serra".into(),
            tipo: String::new(),
            whatsapp: None,
            email: None,
            ativo: true,
        };
        assert_eq!(Cartao::do_parceiro(&p).selo, None);
    }
}
