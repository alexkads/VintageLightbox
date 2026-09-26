//! O estado e os gestos da agenda. O desenho mora em [`super::desenho`].
//!
//! Como o chatbot: nasce com o app, escuta `/bookings/agenda/eventos` assim
//! que a conta entra, e avisa de agendamento novo e cancelado **em qualquer
//! tela**. Com a tela escondida o evento só avisa e marca "desatualizado";
//! a releitura é ao voltar.

use crate::campo::TrocarValor as _;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::modal::Modal;
use chrono::{DateTime, NaiveDate, Utc};
use domain::services::pos_venda::Sessao;
use gpui_kit::component::input::{InputModeKind, InputState, TextareaState};
use gpui_kit::{prelude::*, Context, Entity, EventEmitter, FocusHandle, Task, Window};
use serde_json::Value;

use super::modelo::{self, Ensaio, Estudio, EventoDaAgenda, Indicadores, Visao};
use super::pedidos::{self, Atendimento};
use crate::pos_venda::porta::{PedidoJson, Publicador, Recado};
use crate::tempo_real::preferencias;
use crate::tempo_real::{Aviso, Escuta, EstadoDaConexao, Guarda, Sinal};

const PASSO: Duration = Duration::from_millis(100);
const ESPERA_DO_EVENTO: Duration = Duration::from_millis(400);
const TETO_DA_ESPERA: Duration = Duration::from_secs(2);
const RELEITURA_CONECTADO: Duration = Duration::from_secs(300);
const RELEITURA_SEM_CONEXAO: Duration = Duration::from_secs(15);
/// O rótulo da única fonte.
pub const FONTE: &str = "agenda";

/// O que a agenda pede à raiz — os mesmos quatro do chatbot.
#[derive(Debug, Clone, PartialEq)]
pub enum PedidoDaAgenda {
    Avisar {
        titulo: String,
        corpo: Option<String>,
        aviso: Aviso,
    },
    Toast {
        texto: String,
        erro: bool,
    },
    TrazerParaAFrente,
    Mudou,
}

impl EventEmitter<PedidoDaAgenda> for Agenda {}

/// O modo do diálogo de um agendamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    Detalhes,
    Reagendar,
    Atendimento,
    /// "Excluir o agendamento?" — a pergunta antes.
    Excluir,
}

#[derive(Debug, Clone, PartialEq)]
enum Acao {
    Reagendar,
    Atendimento,
    Excluir,
    Descancelar,
}

pub struct Agenda {
    publicador: Arc<dyn Publicador>,
    escuta: Arc<dyn Escuta>,
    pub(crate) sessao: Option<Sessao>,
    guarda: Option<Guarda>,
    sinais: (Sender<Sinal>, Receiver<Sinal>),
    cliques: (Sender<String>, Receiver<String>),
    pub(crate) conexao: EstadoDaConexao,
    pub(crate) visivel: bool,
    desatualizado: bool,
    ja_abriu: bool,

    // ── O que a API respondeu ──
    pub(crate) estudios: Vec<Estudio>,
    pub(crate) ensaios: Vec<Ensaio>,
    pub(crate) de_hoje: Vec<Ensaio>,
    pub(crate) indicadores: Option<Indicadores>,
    pub(crate) falhou_a_agenda: bool,
    pub(crate) falhou_os_indicadores: bool,
    pub(crate) carregou: bool,

    // ── O calendário ──
    pub(crate) visao: Visao,
    pub(crate) dia: NaiveDate,
    pub(crate) estudio: Option<String>,
    pub(crate) escolhendo_estudio: bool,
    /// O "+ N mais" de um dia do mês.
    pub(crate) dia_aberto: Option<NaiveDate>,

    // ── O diálogo do agendamento ──
    /// No contrato de [`Modal`]: o formulário de reagendar tem campos, e
    /// fechar com o foco num deles matava o `Esc` da agenda.
    pub(crate) aberto: Modal<Ensaio>,
    pub(crate) modo: Modo,
    /// Veio de um aviso: abre quando a leitura trouxer o ensaio.
    abrir_quando_chegar: Option<String>,
    pub(crate) inicio: Entity<InputState>,
    pub(crate) fim: Entity<InputState>,
    pub(crate) valor: Entity<InputState>,
    pub(crate) entrada: Entity<InputState>,
    pub(crate) saida: Entity<InputState>,
    /// Várias linhas: no gpui-kit 0.6 o campo que cresce é um `TextareaState`,
    /// e não mais um `InputState` com `auto_grow`.
    pub(crate) observacoes: Entity<TextareaState>,
    pub(crate) erro_do_formulario: Option<String>,
    pub(crate) em_acao: bool,
    /// Os campos pedem a janela para serem preenchidos: o `abrir` marca e o
    /// próximo desenho preenche.
    preencher: bool,

    /// 🔑 As teclas só casam contexto no caminho até quem tem o foco: sem um
    /// foco da tela, o Esc do diálogo nunca chegaria a ela.
    pub(crate) foco: FocusHandle,
    arquivo_de_preferencias: PathBuf,
    carga: Option<Receiver<Recado>>,
    pub(crate) carregando: usize,
    acoes: Vec<(Acao, Receiver<Recado>)>,
    releitura_em: Option<Instant>,
    primeira_pendente: Option<Instant>,
    ultima_carga: Option<Instant>,
    _vigia: Option<Task<()>>,
}

impl Agenda {
    pub fn nova(
        publicador: Arc<dyn Publicador>,
        escuta: Arc<dyn Escuta>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let campo = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        Self {
            publicador,
            escuta,
            sessao: None,
            guarda: None,
            sinais: channel(),
            cliques: channel(),
            conexao: EstadoDaConexao::Conectando,
            visivel: false,
            desatualizado: true,
            ja_abriu: false,
            estudios: Vec::new(),
            ensaios: Vec::new(),
            de_hoje: Vec::new(),
            indicadores: None,
            falhou_a_agenda: false,
            falhou_os_indicadores: false,
            carregou: false,
            visao: Visao::Mes,
            dia: modelo::hoje(Utc::now()),
            estudio: None,
            escolhendo_estudio: false,
            dia_aberto: None,
            aberto: Modal::default(),
            modo: Modo::Detalhes,
            abrir_quando_chegar: None,
            inicio: campo("dd/mm/aaaa hh:mm", window, cx),
            fim: campo("dd/mm/aaaa hh:mm", window, cx),
            valor: campo("0,00", window, cx),
            entrada: campo("dd/mm/aaaa hh:mm", window, cx),
            saida: campo("dd/mm/aaaa hh:mm", window, cx),
            observacoes: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(3, 6)
                    .placeholder("Ex: cliente chegou atrasado, pagou em dinheiro…")
            }),
            erro_do_formulario: None,
            em_acao: false,
            preencher: false,
            foco: cx.focus_handle(),
            arquivo_de_preferencias: preferencias::arquivo(),
            carga: None,
            carregando: 0,
            acoes: Vec::new(),
            releitura_em: None,
            primeira_pendente: None,
            ultima_carga: None,
            _vigia: None,
        }
    }

    // ── A conta ────────────────────────────────────────────────────────────

    pub fn definir_sessao(&mut self, sessao: Sessao, cx: &mut Context<Self>) {
        let (envia, recebe) = channel();
        self.sinais = (envia.clone(), recebe);
        self.conexao = EstadoDaConexao::Conectando;
        self.ja_abriu = false;
        self.guarda = Some(self.escuta.escutar(
            sessao.clone(),
            vec![(FONTE, "/bookings/agenda/eventos")],
            envia,
        ));
        self.sessao = Some(sessao);
        self.desatualizado = true;
        self.vigiar(cx);
        if self.visivel {
            self.recarregar(cx);
        }
    }

    pub fn sair(&mut self, cx: &mut Context<Self>) {
        self.guarda = None;
        self.sessao = None;
        self._vigia = None;
        self.estudios.clear();
        self.ensaios.clear();
        self.de_hoje.clear();
        self.indicadores = None;
        self.aberto.largar();
        self.carregou = false;
        self.carga = None;
        self.acoes.clear();
        self.carregando = 0;
        cx.notify();
    }

    pub fn mostrar(&mut self, visivel: bool, cx: &mut Context<Self>) {
        let voltou = visivel && !self.visivel;
        self.visivel = visivel;
        if !visivel {
            self.escolhendo_estudio = false;
        }
        if voltou && (self.desatualizado || !self.carregou) {
            self.recarregar(cx);
        }
        cx.notify();
    }

    pub fn canal_dos_cliques(&self) -> Sender<String> {
        self.cliques.0.clone()
    }

    /// O sino é um só para as telas de atendimento: lido do arquivo a cada
    /// aviso, o que o chatbot mudar vale aqui.
    pub fn avisos_ligados(&self) -> bool {
        preferencias::ler(&self.arquivo_de_preferencias).avisos_do_sistema
    }

    #[cfg(test)]
    pub(crate) fn usar_preferencias(&mut self, arquivo: PathBuf) {
        self.arquivo_de_preferencias = arquivo;
    }

    // ── O laço ─────────────────────────────────────────────────────────────

    fn vigiar(&mut self, cx: &mut Context<Self>) {
        if self._vigia.is_some() {
            return;
        }
        self._vigia = Some(cx.spawn(async move |tela, cx| loop {
            cx.background_executor().timer(PASSO).await;
            let continua = tela.update(cx, |tela, cx| tela.passo(cx)).unwrap_or(false);
            if !continua {
                break;
            }
        }));
    }

    pub(crate) fn passo(&mut self, cx: &mut Context<Self>) -> bool {
        if self.sessao.is_none() {
            self._vigia = None;
            return false;
        }
        let agora = cx.background_executor().now();
        let mut mudou = self.colher_sinais(agora, cx);
        while let Ok(destino) = self.cliques.1.try_recv() {
            if let Some(id) = destino.strip_prefix("agenda:") {
                mudou = true;
                cx.emit(PedidoDaAgenda::TrazerParaAFrente);
                self.abrir_pelo_id(id.to_string(), cx);
            }
        }
        mudou |= self.colher_respostas(cx);
        if self.visivel {
            let vencida = self.releitura_em.is_some_and(|em| em <= agora);
            let intervalo = if self.conexao == EstadoDaConexao::Conectado {
                RELEITURA_CONECTADO
            } else {
                RELEITURA_SEM_CONEXAO
            };
            let periodica = self
                .ultima_carga
                .is_some_and(|u| agora.duration_since(u) >= intervalo);
            if (vencida || periodica) && self.carregando == 0 {
                self.recarregar(cx);
                mudou = true;
            }
        }
        if mudou {
            cx.notify();
        }
        true
    }

    fn marcar_desatualizado(&mut self, agora: Instant, ja: bool) {
        self.desatualizado = true;
        if !self.visivel {
            return;
        }
        let primeira = *self.primeira_pendente.get_or_insert(agora);
        self.releitura_em = Some(if ja {
            agora
        } else {
            (agora + ESPERA_DO_EVENTO).min(primeira + TETO_DA_ESPERA)
        });
    }

    fn colher_sinais(&mut self, agora: Instant, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(sinal) = self.sinais.1.try_recv() {
            mudou = true;
            match sinal {
                Sinal::Conexao { estado, .. } => self.conexao = estado,
                Sinal::Pronto { .. } => {
                    if self.ja_abriu {
                        self.marcar_desatualizado(agora, true);
                    }
                    self.ja_abriu = true;
                }
                Sinal::Sincronizar { .. } => self.marcar_desatualizado(agora, true),
                Sinal::Evento { dados, .. } => match EventoDaAgenda::ler(&dados) {
                    Some(evento) => {
                        if let Some((titulo, corpo)) = evento.aviso() {
                            cx.emit(PedidoDaAgenda::Avisar {
                                aviso: Aviso {
                                    titulo: titulo.clone(),
                                    corpo: corpo.clone().unwrap_or_default(),
                                    destino: format!("agenda:{}", evento.ensaio_id),
                                },
                                titulo,
                                corpo,
                            });
                        }
                        let (de, ate) = modelo::periodo_visivel(self.visao, self.dia);
                        let hoje = modelo::hoje(Utc::now());
                        let conta = evento.relevante(de, ate, self.estudio.as_deref())
                            || evento.relevante(hoje, hoje, self.estudio.as_deref());
                        if conta {
                            self.marcar_desatualizado(agora, false);
                        }
                    }
                    None => {
                        eprintln!("⚠️ [Agenda] evento ilegível: {dados}");
                        self.marcar_desatualizado(agora, false);
                    }
                },
            }
        }
        mudou
    }

    // ── A leitura ──────────────────────────────────────────────────────────

    fn pedir(&self, pedido: PedidoJson, canal: &Sender<Recado>) {
        if let Some(sessao) = self.sessao.clone() {
            self.publicador.pedir_json(sessao, pedido, canal.clone());
        }
    }

    /// Relê os quatro — o `revalidatePath` do site.
    pub fn recarregar(&mut self, cx: &mut Context<Self>) {
        if self.sessao.is_none() {
            return;
        }
        let (envia, recebe) = channel();
        let (de, ate) = modelo::periodo_visivel(self.visao, self.dia);
        let estudio = self.estudio.as_deref();
        let pedidos = [
            pedidos::estudios(),
            pedidos::agenda(de, ate, estudio),
            pedidos::hoje(modelo::hoje(Utc::now()), estudio),
            pedidos::indicadores(),
        ];
        self.carregando = pedidos.len();
        for pedido in pedidos {
            self.pedir(pedido, &envia);
        }
        self.carga = Some(recebe);
        self.desatualizado = false;
        self.releitura_em = None;
        self.primeira_pendente = None;
        self.ultima_carga = Some(cx.background_executor().now());
        cx.notify();
    }

    fn colher_respostas(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        let mut recados = Vec::new();
        if let Some(carga) = &self.carga {
            while let Ok(recado) = carga.try_recv() {
                recados.push(recado);
            }
        }
        for recado in recados {
            if let Recado::Json { rotulo, resultado } = recado {
                mudou = true;
                self.carregando = self.carregando.saturating_sub(1);
                self.ler_da_carga(rotulo, resultado);
            }
        }
        let mut prontas = Vec::new();
        self.acoes.retain(|(acao, canal)| match canal.try_recv() {
            Ok(Recado::Json { resultado, .. }) => {
                prontas.push((acao.clone(), resultado));
                false
            }
            Ok(_) | Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
        });
        for (acao, resultado) in prontas {
            mudou = true;
            self.concluir(acao, resultado, cx);
        }
        mudou
    }

    fn ler_da_carga(&mut self, rotulo: &'static str, resultado: Result<Value, String>) {
        match (rotulo, resultado) {
            ("estudios", Ok(v)) => self.estudios = Estudio::lista_ativa(&v),
            ("agenda", Ok(v)) => {
                self.ensaios = Ensaio::lista(&v);
                self.falhou_a_agenda = false;
                self.carregou = true;
                if let Some(id) = self.abrir_quando_chegar.take() {
                    match self.ensaios.iter().find(|e| e.id == id).cloned() {
                        Some(ensaio) => self.abrir(ensaio),
                        None => self.abrir_quando_chegar = Some(id),
                    }
                }
                // O que está aberto acompanha a leitura.
                if let Some(aberto) = self.aberto.aberto_mut() {
                    if let Some(novo) = self.ensaios.iter().find(|e| e.id == aberto.id) {
                        *aberto = novo.clone();
                    }
                }
            }
            ("hoje", Ok(v)) => {
                self.de_hoje = Ensaio::lista(&v);
                if let Some(id) = self.abrir_quando_chegar.take() {
                    match self.de_hoje.iter().find(|e| e.id == id).cloned() {
                        Some(ensaio) => self.abrir(ensaio),
                        None => self.abrir_quando_chegar = Some(id),
                    }
                }
            }
            ("indicadores", Ok(v)) => {
                self.indicadores = Indicadores::ler(&v);
                self.falhou_os_indicadores = self.indicadores.is_none();
            }
            ("agenda", Err(erro)) => {
                eprintln!("⚠️ [Agenda] {erro}");
                self.falhou_a_agenda = true;
            }
            ("indicadores", Err(erro)) => {
                eprintln!("⚠️ [Agenda] indicadores: {erro}");
                self.falhou_os_indicadores = true;
            }
            (rotulo, Err(erro)) => eprintln!("⚠️ [Agenda] {rotulo}: {erro}"),
            _ => {}
        }
    }

    // ── O calendário ───────────────────────────────────────────────────────

    pub fn escolher_visao(&mut self, visao: Visao, cx: &mut Context<Self>) {
        if self.visao == visao {
            return;
        }
        self.visao = visao;
        self.dia_aberto = None;
        self.recarregar(cx);
    }

    /// "Anterior" (−1), "Próximo" (+1).
    pub fn andar(&mut self, passo: i32, cx: &mut Context<Self>) {
        self.dia = modelo::navegar(self.visao, self.dia, passo);
        self.dia_aberto = None;
        self.recarregar(cx);
    }

    /// "Hoje".
    pub fn ir_para_hoje(&mut self, cx: &mut Context<Self>) {
        let hoje = modelo::hoje(Utc::now());
        if self.dia == hoje {
            return;
        }
        self.dia = hoje;
        self.dia_aberto = None;
        self.recarregar(cx);
    }

    /// O dia clicado nas visões de ano e estação abre a visão de dia; o título
    /// do mês, a de mês.
    pub fn abrir_dia(&mut self, dia: NaiveDate, visao: Visao, cx: &mut Context<Self>) {
        self.dia = dia;
        self.visao = visao;
        self.dia_aberto = None;
        self.recarregar(cx);
    }

    /// "+ N mais".
    pub fn mostrar_o_dia(&mut self, dia: Option<NaiveDate>, cx: &mut Context<Self>) {
        self.dia_aberto = dia;
        cx.notify();
    }

    pub fn alternar_seletor_de_estudio(&mut self, cx: &mut Context<Self>) {
        self.escolhendo_estudio = !self.escolhendo_estudio;
        cx.notify();
    }

    /// "Todos os Estúdios" (`None`) ou um. Não recorta os indicadores.
    pub fn escolher_estudio(&mut self, estudio: Option<String>, cx: &mut Context<Self>) {
        self.escolhendo_estudio = false;
        if self.estudio == estudio {
            cx.notify();
            return;
        }
        self.estudio = estudio;
        self.recarregar(cx);
    }

    pub fn nome_do_estudio_escolhido(&self) -> String {
        self.estudio
            .as_ref()
            .and_then(|id| self.estudios.iter().find(|e| &e.id == id))
            .map(|e| e.nome.clone())
            .unwrap_or_else(|| "Todos os Estúdios".into())
    }

    // ── O diálogo ──────────────────────────────────────────────────────────

    /// Clicar num evento ou no "Abrir" da lista de hoje.
    /// O clique no evento: o diálogo abre e o foco vai para a agenda, onde
    /// mora o `Esc` dele — e é para lá que volta ao fechar.
    pub fn abrir_ensaio(&mut self, ensaio: Ensaio, window: &mut Window, cx: &mut Context<Self>) {
        self.dia_aberto = None;
        self.abrir(ensaio);
        self.aberto.focar(&self.foco.clone(), window, cx);
        cx.notify();
    }

    /// Sem janela: o aviso que abre o ensaio quando a leitura chegar.
    fn abrir(&mut self, ensaio: Ensaio) {
        self.aberto.abrir_sem_janela(ensaio);
        self.modo = Modo::Detalhes;
        self.erro_do_formulario = None;
        self.em_acao = false;
    }

    /// O clique num aviso: o ensaio pode não estar no período — a tela vai
    /// para o dia dele se o evento trouxe a data, e abre quando chegar.
    fn abrir_pelo_id(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(ensaio) = self
            .ensaios
            .iter()
            .chain(self.de_hoje.iter())
            .find(|e| e.id == id)
            .cloned()
        {
            self.abrir(ensaio);
            return;
        }
        self.abrir_quando_chegar = Some(id);
        self.recarregar(cx);
    }

    pub fn fechar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.em_acao {
            return;
        }
        self.aberto.fechar(window, cx);
        self.modo = Modo::Detalhes;
        cx.notify();
    }

    /// O Esc do site: formulário → detalhes → fechar.
    pub fn voltar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.em_acao {
            return;
        }
        if self.dia_aberto.is_some() {
            self.dia_aberto = None;
        } else if self.aberto.esta_aberto() && self.modo != Modo::Detalhes {
            self.modo = Modo::Detalhes;
            self.erro_do_formulario = None;
            // O formulário sumiu com o campo focado: o foco volta à agenda.
            let foco = self.foco.clone();
            self.aberto.focar(&foco, window, cx);
        } else {
            self.aberto.fechar(window, cx);
        }
        cx.notify();
    }

    pub fn entrar_no_modo(&mut self, modo: Modo, cx: &mut Context<Self>) {
        if !self.aberto.esta_aberto() {
            return;
        }
        self.modo = modo;
        self.erro_do_formulario = None;
        self.preencher = true;
        cx.notify();
    }

    /// Os campos do formulário com o que o ensaio já tem.
    pub(crate) fn preencher_se_preciso(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.preencher) {
            return;
        }
        let Some(e) = self.aberto.aberto().cloned() else {
            return;
        };
        let pares: [(&Entity<InputState>, String); 5] = [
            (&self.inicio, modelo::para_o_campo(e.inicio)),
            (&self.fim, modelo::para_o_campo(e.fim)),
            (
                &self.valor,
                e.valor_pago
                    .map(|v| format!("{v:.2}").replace('.', ","))
                    .unwrap_or_default(),
            ),
            (
                &self.entrada,
                e.hora_entrada.map(modelo::para_o_campo).unwrap_or_default(),
            ),
            (
                &self.saida,
                e.hora_saida.map(modelo::para_o_campo).unwrap_or_default(),
            ),
        ];
        for (campo, valor) in pares {
            campo.update(cx, |c, cx| c.trocar_valor(valor, window, cx));
        }
        let observacoes = e.observacoes_atendimento.clone().unwrap_or_default();
        self.observacoes
            .update(cx, |c, cx| c.trocar_valor(observacoes, window, cx));
    }

    fn valor_do<M: InputModeKind>(
        &self,
        campo: &Entity<gpui_kit::base::input::InputBaseState<M>>,
        cx: &gpui_kit::App,
    ) -> String {
        campo.read(cx).value().trim().to_string()
    }

    /// As duas datas do reagendamento, ou a frase do que está errado.
    pub fn datas_do_reagendamento(
        &self,
        cx: &gpui_kit::App,
    ) -> Result<(DateTime<Utc>, DateTime<Utc>), &'static str> {
        let inicio = modelo::do_campo(&self.valor_do(&self.inicio, cx));
        let fim = modelo::do_campo(&self.valor_do(&self.fim, cx));
        let (Some(inicio), Some(fim)) = (inicio, fim) else {
            return Err("Informe as duas datas.");
        };
        if inicio >= fim {
            return Err("A data de término deve ser após a data de início.");
        }
        Ok((inicio, fim))
    }

    /// "Confirmar reagendamento".
    pub fn confirmar_reagendamento(&mut self, cx: &mut Context<Self>) {
        let Some(ensaio) = self.aberto.aberto().cloned() else {
            return;
        };
        if self.em_acao {
            return;
        }
        match self.datas_do_reagendamento(cx) {
            Err(erro) => self.erro_do_formulario = Some(erro.into()),
            Ok((inicio, fim)) => {
                self.erro_do_formulario = None;
                self.mandar(pedidos::reagendar(&ensaio.id, inicio, fim), Acao::Reagendar);
            }
        }
        cx.notify();
    }

    /// O que o "Registrar atendimento" vai mandar, ou a frase do erro.
    pub fn dados_do_atendimento(&self, cx: &gpui_kit::App) -> Result<Atendimento, String> {
        let valor_pago = modelo::ler_valor(&self.valor_do(&self.valor, cx))?;
        let texto_entrada = self.valor_do(&self.entrada, cx);
        let texto_saida = self.valor_do(&self.saida, cx);
        let hora_entrada = modelo::do_campo(&texto_entrada);
        let hora_saida = modelo::do_campo(&texto_saida);
        if (!texto_entrada.is_empty() && hora_entrada.is_none())
            || (!texto_saida.is_empty() && hora_saida.is_none())
        {
            return Err("Use o formato dd/mm/aaaa hh:mm.".into());
        }
        if let (Some(entrada), Some(saida)) = (hora_entrada, hora_saida) {
            if entrada >= saida {
                return Err("A hora de saída deve ser após a hora de entrada.".into());
            }
        }
        let observacoes = Some(self.valor_do(&self.observacoes, cx)).filter(|o| !o.is_empty());
        Ok(Atendimento {
            valor_pago,
            hora_entrada,
            hora_saida,
            observacoes,
        })
    }

    /// "Confirmar atendimento".
    pub fn confirmar_atendimento(&mut self, cx: &mut Context<Self>) {
        let Some(ensaio) = self.aberto.aberto().cloned() else {
            return;
        };
        if self.em_acao {
            return;
        }
        match self.dados_do_atendimento(cx) {
            Err(erro) => self.erro_do_formulario = Some(erro),
            Ok(dados) => {
                self.erro_do_formulario = None;
                self.mandar(pedidos::atendimento(&ensaio.id, &dados), Acao::Atendimento);
            }
        }
        cx.notify();
    }

    /// "Excluir" na pergunta.
    pub fn confirmar_exclusao(&mut self, cx: &mut Context<Self>) {
        let Some(ensaio) = self.aberto.aberto().cloned() else {
            return;
        };
        if self.em_acao || self.modo != Modo::Excluir {
            return;
        }
        self.mandar(pedidos::excluir(&ensaio.id), Acao::Excluir);
        cx.notify();
    }

    /// "Descancelar" — só no cancelado.
    pub fn descancelar(&mut self, ensaio: &Ensaio, cx: &mut Context<Self>) {
        if self.em_acao || !ensaio.status.cancelado() {
            return;
        }
        self.mandar(pedidos::descancelar(&ensaio.id), Acao::Descancelar);
        cx.notify();
    }

    fn mandar(&mut self, pedido: PedidoJson, acao: Acao) {
        self.em_acao = true;
        let (envia, recebe) = channel();
        self.pedir(pedido, &envia);
        self.acoes.push((acao, recebe));
    }

    /// O `explicar` do `actions.ts` da agenda.
    fn explicar(erro: &str, conflito: &str) -> String {
        let status = erro
            .split("o site respondeu ")
            .nth(1)
            .and_then(|r| r.split_whitespace().next())
            .map(|s| s.trim_end_matches(':'))
            .unwrap_or_default();
        match status {
            "409" => conflito.into(),
            "404" => "Esse agendamento não existe mais.".into(),
            "400" | "422" => erro
                .split_once(": ")
                .map(|(_, m)| m.trim().to_string())
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| "Não foi possível concluir. Tente de novo.".into()),
            _ => "Não foi possível concluir. Tente de novo.".into(),
        }
    }

    fn concluir(&mut self, acao: Acao, resultado: Result<Value, String>, cx: &mut Context<Self>) {
        self.em_acao = false;
        let (certo, conflito) = match acao {
            Acao::Reagendar => (
                "Agendamento reagendado.",
                "Já há ensaio nesse horário. Escolha outro para reagendar.",
            ),
            Acao::Atendimento => (
                "Atendimento registrado.",
                "Não foi possível registrar o atendimento.",
            ),
            Acao::Excluir => ("Agendamento excluído.", "Não foi possível excluir."),
            Acao::Descancelar => (
                "Agendamento restaurado.",
                "O horário já foi ocupado. Remarque para outro.",
            ),
        };
        match resultado {
            Ok(_) => {
                cx.emit(PedidoDaAgenda::Toast {
                    texto: certo.into(),
                    erro: false,
                });
                self.aberto.fechar_depois(cx);
                self.modo = Modo::Detalhes;
                self.recarregar(cx);
            }
            Err(erro) => {
                let frase = Self::explicar(&erro, conflito);
                if matches!(acao, Acao::Reagendar | Acao::Atendimento) {
                    self.erro_do_formulario = Some(frase.clone());
                }
                cx.emit(PedidoDaAgenda::Toast {
                    texto: frase,
                    erro: true,
                });
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_erro_da_api_vira_a_frase_do_site() {
        let conflito = "Já há ensaio nesse horário. Escolha outro para reagendar.";
        assert_eq!(
            Agenda::explicar("o site respondeu 409: Horário sem vaga (1/1)", conflito),
            conflito
        );
        assert_eq!(
            Agenda::explicar("o site respondeu 404: nada", conflito),
            "Esse agendamento não existe mais."
        );
        assert_eq!(
            Agenda::explicar(
                "o site respondeu 400: Horário de início (07:00) é antes do horário de abertura do estúdio (09:00)",
                conflito
            ),
            "Horário de início (07:00) é antes do horário de abertura do estúdio (09:00)"
        );
        assert_eq!(
            Agenda::explicar("o site respondeu 500: pane", conflito),
            "Não foi possível concluir. Tente de novo."
        );
        assert_eq!(
            Agenda::explicar("sem resposta do site: timeout", conflito),
            "Não foi possível concluir. Tente de novo."
        );
    }
}
