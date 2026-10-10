//! O estado e os gestos do painel do chatbot. O desenho mora em
//! [`super::desenho`].
//!
//! # Vive o tempo todo, e não só com a tela na frente
//!
//! O painel nasce com o app e passa a escutar os cinco fluxos assim que a conta
//! entra ([`Chatbot::definir_sessao`]). É o que faz o aviso chegar com o
//! operador na triagem, no caixa ou com o app na bandeja — que é o pedido
//! (dono, 2026-09-25: *"o operador … esteja antenado no bot de atendimento"*).
//!
//! O que só vale com a tela na frente é **reler**: com ela escondida, cada
//! evento só acende a novidade e marca "desatualizado"; a releitura acontece ao
//! voltar. É o que o site faz com a aba escondida.
//!
//! # O relógio da releitura (o `canal.tsx` do site)
//!
//! - evento → releitura em 400 ms, empurrada por eventos seguidos até 2 s da
//!   primeira pendente;
//! - `pronto` depois de reconectar e `sincronizar` → releitura na hora;
//! - de segurança: a cada 300 s conectado, a cada 15 s sem conexão.

use crate::campo::TrocarValor as _;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::modal::Modal;
use domain::services::pos_venda::Sessao;
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{prelude::*, Context, Entity, EventEmitter, SharedString, Task, Window};
use serde_json::Value;

use super::midia::{self, ClasseDoAnexo, ComoEnviar, Midia, Previa, TipoDeMidia};
use super::modelo::{
    self, Cadastro, Canal, Chave, Conversa, EventoDoChatbot, FiltroDeCanal, Mensagem,
    PaginaDoWhatsApp, Status, Urgencia, Voucher,
};
use super::pedidos::{self, MudancaDaUrgencia};
use crate::importacao::estado::Recado as RecadoDoSeletor;
use crate::importacao::explorador::SeletorDePasta;
use crate::pos_venda::porta::{PedidoCru, PedidoJson, Publicador, Recado};
use crate::tempo_real::preferencias::{self, Preferencias};
use crate::tempo_real::{Aviso, Escuta, EstadoDaConexao, Guarda, Sinal};

/// De quanto em quanto a tela olha os canais.
pub(crate) const PASSO: Duration = Duration::from_millis(100);
/// A espera depois de um evento, para uma rajada virar uma releitura só.
pub(crate) const ESPERA_DO_EVENTO: Duration = Duration::from_millis(400);
/// O teto dessa espera, contado da primeira pendente.
pub(crate) const TETO_DA_ESPERA: Duration = Duration::from_secs(2);
/// A releitura de segurança com o tempo real funcionando…
pub(crate) const RELEITURA_CONECTADO: Duration = Duration::from_secs(300);
/// …e sem ele: "Atualizando a cada 15s".
pub(crate) const RELEITURA_SEM_CONEXAO: Duration = Duration::from_secs(15);

/// O que o painel pede à raiz.
#[derive(Debug, Clone, PartialEq)]
pub enum PedidoDoChatbot {
    /// Mensagem nova de um cliente. A raiz decide: janela em foco vira toast;
    /// fora de foco vira aviso do sistema. `na_tela` é a conversa já aberta
    /// e visível — aí nenhum dos dois.
    Avisar {
        titulo: String,
        corpo: Option<String>,
        aviso: Aviso,
        na_tela: bool,
    },
    /// O resultado de um gesto ("Você assumiu a conversa…").
    Toast { texto: String, erro: bool },
    /// O operador clicou num aviso do sistema: a janela vem para a frente, na
    /// tela do chatbot.
    TrazerParaAFrente,
    /// Novidades ou urgências mudaram — o selo do menu lateral.
    Mudou,
}

impl EventEmitter<PedidoDoChatbot> for Chatbot {}

/// Uma resposta pronta (`GET /whatsapp/quick-responses`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespostaRapida {
    pub id: String,
    pub titulo: String,
    pub mensagem: String,
    pub usos: i64,
}

impl RespostaRapida {
    fn lista(valor: &Value) -> Vec<RespostaRapida> {
        valor
            .as_array()
            .map(|l| {
                l.iter()
                    .filter_map(|r| {
                        Some(RespostaRapida {
                            id: r.get("id")?.as_str()?.to_string(),
                            titulo: r.get("title")?.as_str()?.to_string(),
                            mensagem: r.get("message")?.as_str()?.to_string(),
                            usos: r.get("usage").and_then(Value::as_i64).unwrap_or(0),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Uma mensagem escrita que ainda não voltou do servidor — o balão
/// "enviando", ou "não enviada" com "Tentar de novo".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pendente {
    pub id: u64,
    pub chave: Chave,
    pub texto: String,
    pub resposta: Option<String>,
    pub erro: Option<String>,
    /// 📎 O anexo em voo. Fica aqui para o "Tentar de novo" mandar o **mesmo**
    /// arquivo, sem pedir o seletor de novo.
    pub anexo: Option<Anexo>,
}

/// 📎 O arquivo que o operador escolheu para mandar pela conversa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anexo {
    pub nome: String,
    pub bytes: Arc<Vec<u8>>,
    /// O que vai como legenda — vazio até o "Enviar", e sempre vazio no áudio.
    pub legenda: String,
    /// Como sai: foto, vídeo ou documento — a escolha da tira. Começa no
    /// jeito natural do arquivo (`ComoEnviar::opcoes`).
    pub como: Option<ComoEnviar>,
}

/// A foto de um balão: a caminho, na tela, ou sem conseguir.
#[derive(Clone)]
pub enum Miniatura {
    Baixando,
    Pronta(Arc<gpui_kit::RenderImage>),
    Falhou,
}

/// Para que o arquivo de uma mensagem está sendo baixado.
#[derive(Debug, Clone)]
enum Baixa {
    /// A foto que aparece dentro do balão.
    Miniatura,
    /// O clique: gravar e entregar ao programa do sistema.
    Abrir(Midia),
    /// 🎧 O MP3 do áudio, para tocar aqui.
    Tocar,
}

/// O aviso de que um áudio terminou — o id e, se não tocou, o motivo.
type FimDoAudio = (String, Option<String>);

/// Toca o MP3 até o fim, ou até `parar`. Avisa `fim` com o id ao terminar —
/// é o que apaga o "tocando" do balão.
#[cfg(not(test))]
fn tocar_mp3(id: String, bytes: Vec<u8>, parar: Arc<AtomicBool>, fim: Sender<FimDoAudio>) {
    let _ = std::thread::Builder::new()
        .name("audio-da-conversa".into())
        .spawn(move || {
            let desfecho = (|| -> Result<(), String> {
                let mut saida = rodio::DeviceSinkBuilder::open_default_sink()
                    .map_err(|e| format!("sem saída de áudio: {e}"))?;
                saida.log_on_drop(false);
                let tocador = rodio::Player::connect_new(saida.mixer());
                let decodificado = rodio::Decoder::new(std::io::Cursor::new(bytes))
                    .map_err(|e| format!("o áudio não decodificou: {e}"))?;
                tocador.append(decodificado);
                while !tocador.empty() && !parar.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(50));
                }
                tocador.stop();
                Ok(())
            })();
            let _ = fim.send((id, desfecho.err()));
        });
}

/// Nos testes não há alto-falante: o áudio "termina" na hora, e o que se
/// confere é o pedido da prévia e o estado do balão.
#[cfg(test)]
fn tocar_mp3(id: String, _bytes: Vec<u8>, _parar: Arc<AtomicBool>, fim: Sender<FimDoAudio>) {
    let _ = fim.send((id, None));
}

/// O lado maior da foto do balão, em pixels. O balão a desenha com metade
/// disto; o dobro é para a tela de alta densidade.
const LADO_DA_MINIATURA: u32 = 480;

/// A foto do balão, a partir do arquivo que a API devolveu. Roda fora da
/// thread da interface: decodificar um JPEG de 5 MB custa dezenas de
/// milissegundos.
///
/// 🔑 **Sai quadrada, cortada pelo meio.** O balão reserva um quadro fixo
/// antes de a foto chegar; com a proporção de cada foto, a conversa crescia
/// quando ela chegava e o fim do histórico saía da tela (visto no app de
/// verdade em 10/out/2026, com uma foto em pé). O clique abre a foto inteira.
fn miniatura_de(bytes: &[u8]) -> Option<Arc<gpui_kit::RenderImage>> {
    let imagem = image::load_from_memory(bytes).ok()?;
    Some(crate::imagem::para_gpui(imagem.resize_to_fill(
        LADO_DA_MINIATURA,
        LADO_DA_MINIATURA,
        image::imageops::FilterType::Triangle,
    )))
}

/// Grava o arquivo numa pasta temporária do app e o entrega ao programa do
/// sistema. Devolve onde ficou.
fn gravar_e_abrir(nome: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let pasta = std::env::temp_dir().join("vintagelightbox-midia");
    std::fs::create_dir_all(&pasta)
        .map_err(|e| format!("Não foi possível guardar o arquivo: {e}"))?;
    let caminho = pasta.join(nome);
    std::fs::write(&caminho, bytes)
        .map_err(|e| format!("Não foi possível guardar o arquivo: {e}"))?;
    abrir_no_sistema(&caminho)?;
    Ok(caminho)
}

/// O programa padrão do sistema para aquele arquivo — o do duplo clique.
#[cfg(not(test))]
fn abrir_no_sistema(caminho: &Path) -> Result<(), String> {
    open::that_detached(caminho).map_err(|e| format!("O sistema não abriu o arquivo: {e}"))
}

/// Nos testes nada abre: o que se confere é o que foi **gravado** e entregue
/// (`Chatbot::abertos`).
#[cfg(test)]
fn abrir_no_sistema(_caminho: &Path) -> Result<(), String> {
    Ok(())
}

/// O que está aberto por cima do painel.
#[derive(Debug, Clone, PartialEq)]
pub enum Dialogo {
    /// "🚨 Interações urgentes".
    Urgencias,
    /// "Resolver urgência", com as notas. Cancelar volta à lista.
    Resolver(Urgencia),
    /// "Descartar este alerta?".
    Descartar(Urgencia),
    /// "Quem está assumindo?" — só o canal do site.
    QuemAssume(Chave),
    /// "Excluir todo o histórico" — só WhatsApp.
    ExcluirHistorico(Chave),
}

/// A que gesto uma resposta pertence.
#[derive(Debug, Clone, PartialEq)]
enum Acao {
    Envio(u64),
    Alternar {
        chave: Chave,
        atender: bool,
    },
    Urgencia(MudancaDaUrgencia),
    ApagarHistorico,
    ContarUso,
    /// 🎤 "Transcrever", com o id da mensagem.
    Transcricao(String),
}

pub struct Chatbot {
    publicador: Arc<dyn Publicador>,
    escuta: Arc<dyn Escuta>,
    /// O seletor de arquivo do sistema — o mesmo da importação.
    seletor: Arc<dyn SeletorDePasta>,
    pub(crate) sessao: Option<Sessao>,
    guarda: Option<Guarda>,
    sinais: (Sender<Sinal>, Receiver<Sinal>),
    cliques: (Sender<String>, Receiver<String>),
    pub(crate) conexoes: HashMap<&'static str, EstadoDaConexao>,
    /// A tela está na frente.
    pub(crate) visivel: bool,
    /// Algo mudou desde a última leitura, com a tela escondida.
    desatualizado: bool,

    // ── O que a API respondeu ──
    pub(crate) pagina_do_whatsapp: PaginaDoWhatsApp,
    /// A conversa aberta que não está na página (veio de aviso ou urgência).
    fora_da_pagina: PaginaDoWhatsApp,
    pub(crate) outros: HashMap<Canal, Vec<Conversa>>,
    vouchers: HashMap<String, Voucher>,
    cadastros: HashMap<String, Cadastro>,
    pub(crate) historico_aberto: Vec<Mensagem>,
    pub(crate) carregando_historico: bool,
    pub(crate) urgencias: Vec<Urgencia>,
    pub(crate) respostas: Vec<RespostaRapida>,
    pub(crate) carregou: bool,
    pub(crate) falhou_a_carga: bool,

    // ── Filtros e seleção ──
    pub(crate) canal: FiltroDeCanal,
    pub(crate) status: Status,
    pub(crate) busca: Entity<InputState>,
    pub(crate) busca_aplicada: String,
    pub(crate) pagina: u32,
    pub(crate) aberta: Option<Chave>,
    /// "Carregar mensagens anteriores" foi clicado nesta conversa.
    pub(crate) tudo: bool,
    /// Conversas com mensagem nova ainda não aberta — o ponto verde.
    pub(crate) novidades: HashSet<Chave>,

    // ── A conversa aberta ──
    pub(crate) pendentes: Vec<Pendente>,
    proximo_pendente: u64,
    pub(crate) compositor: Entity<TextareaState>,
    pub(crate) respostas_abertas: bool,
    /// Trocou de conversa: o campo esvazia no próximo desenho (esvaziar pede a
    /// janela, que o `abrir` não tem).
    pub(crate) limpar_compositor: bool,
    /// O histórico rola até o fim quando a conversa ou o número de balões
    /// muda — e só então: quem subiu para ler não é puxado de volta a cada
    /// desenho.
    pub(crate) rolagem: gpui_kit::ScrollHandle,
    pub(crate) rolagem_vista: Option<(Chave, usize, usize)>,

    // ── 📎 A mídia da conversa aberta ──
    /// O arquivo escolhido, esperando o "Enviar".
    pub(crate) anexo: Option<Anexo>,
    /// O seletor do sistema está aberto.
    escolha: Option<Receiver<RecadoDoSeletor>>,
    /// A foto de cada balão, pelo id da mensagem.
    pub(crate) miniaturas: HashMap<String, Miniatura>,
    /// Há balão novo para conferir se falta foto.
    conferir_miniaturas: bool,
    baixas: Vec<(String, Baixa, Receiver<Recado>)>,
    decodificando: Vec<(String, Receiver<Option<Arc<gpui_kit::RenderImage>>>)>,
    /// As mensagens cujo arquivo está sendo baixado para abrir.
    pub(crate) abrindo: HashSet<String>,
    /// 🎤 Os áudios com a transcrição pedida.
    pub(crate) transcrevendo: HashSet<String>,
    /// 🎧 O áudio que está tocando (id da mensagem) e como pará-lo.
    tocando: Option<(String, Arc<AtomicBool>)>,
    /// Os áudios cujo MP3 está a caminho.
    pub(crate) baixando_audio: HashSet<String>,
    /// O aviso de que um áudio terminou (ou não conseguiu tocar).
    fim_da_reproducao: (Sender<FimDoAudio>, Receiver<FimDoAudio>),
    /// O que foi entregue ao programa do sistema, na ordem.
    pub(crate) abertos: Vec<PathBuf>,

    // ── Diálogos ──
    /// No contrato de [`Modal`]: "Quem assume", "Resolver" e "Excluir"
    /// têm campo, e fechar com o foco nele matava as teclas da tela.
    pub(crate) dialogo: Modal<Dialogo>,
    pub(crate) notas: Entity<TextareaState>,
    pub(crate) nome_do_atendente: Entity<InputState>,
    pub(crate) confirmacao: Entity<InputState>,
    pub(crate) em_acao: bool,

    pub(crate) preferencias: Preferencias,
    arquivo_de_preferencias: PathBuf,

    // ── As respostas a esperar ──
    carga: Option<Receiver<Recado>>,
    /// O remetente da mesma leva: os pedidos que dependem da página do
    /// WhatsApp (voucher, cadastro) entram nela, e uma leva nova descarta tudo
    /// o que a velha ainda ia responder.
    envia_da_carga: Option<Sender<Recado>>,
    pub(crate) carregando: usize,
    historico: Option<Receiver<Recado>>,
    acoes: Vec<(Acao, Receiver<Recado>)>,

    // ── O relógio ──
    releitura_em: Option<Instant>,
    primeira_pendente: Option<Instant>,
    ultima_carga: Option<Instant>,
    /// Já houve um `pronto`: o próximo é reconexão, e relê.
    ja_abriu: HashSet<&'static str>,
    _vigia: Option<Task<()>>,
    _assinaturas: Vec<gpui_kit::Subscription>,
}

impl Chatbot {
    pub fn novo(
        publicador: Arc<dyn Publicador>,
        escuta: Arc<dyn Escuta>,
        seletor: Arc<dyn SeletorDePasta>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let busca =
            cx.new(|cx| InputState::new(window, cx).placeholder("Buscar nos cinco canais…"));
        let compositor = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 6)
                .placeholder("Escrever para o cliente…")
        });
        let notas = cx.new(|cx| {
            TextareaState::new(window, cx).auto_grow(5, 8).placeholder(
                "Ex.: cliente atendido, dúvida sobre o voucher esclarecida, novo código enviado…",
            )
        });
        let nome_do_atendente = cx.new(|cx| InputState::new(window, cx).placeholder("Seu nome"));
        let confirmacao = cx.new(|cx| InputState::new(window, cx));
        let arquivo_de_preferencias = preferencias::arquivo();
        let assinaturas = vec![
            // O botão "Excluir histórico" acende com a frase certa.
            cx.subscribe_in(&confirmacao, window, |_, _, _: &InputEvent, _w, cx| {
                cx.notify()
            }),
            cx.subscribe_in(&notas, window, |_, _, _: &InputEvent, _w, cx| cx.notify()),
            cx.subscribe_in(&compositor, window, |_, _, _: &InputEvent, _w, cx| {
                cx.notify()
            }),
        ];
        Self {
            publicador,
            escuta,
            seletor,
            sessao: None,
            guarda: None,
            sinais: channel(),
            cliques: channel(),
            conexoes: HashMap::new(),
            visivel: false,
            desatualizado: true,
            pagina_do_whatsapp: PaginaDoWhatsApp::default(),
            fora_da_pagina: PaginaDoWhatsApp::default(),
            outros: HashMap::new(),
            vouchers: HashMap::new(),
            cadastros: HashMap::new(),
            historico_aberto: Vec::new(),
            carregando_historico: false,
            urgencias: Vec::new(),
            respostas: Vec::new(),
            carregou: false,
            falhou_a_carga: false,
            canal: FiltroDeCanal::Todas,
            status: Status::Todas,
            busca,
            busca_aplicada: String::new(),
            pagina: 1,
            aberta: None,
            tudo: false,
            novidades: HashSet::new(),
            pendentes: Vec::new(),
            proximo_pendente: 0,
            compositor,
            respostas_abertas: false,
            limpar_compositor: false,
            rolagem: gpui_kit::ScrollHandle::new(),
            rolagem_vista: None,
            anexo: None,
            escolha: None,
            miniaturas: HashMap::new(),
            conferir_miniaturas: false,
            baixas: Vec::new(),
            decodificando: Vec::new(),
            abrindo: HashSet::new(),
            transcrevendo: HashSet::new(),
            tocando: None,
            baixando_audio: HashSet::new(),
            fim_da_reproducao: channel(),
            abertos: Vec::new(),
            dialogo: Modal::default(),
            notas,
            nome_do_atendente,
            confirmacao,
            em_acao: false,
            preferencias: preferencias::ler(&arquivo_de_preferencias),
            arquivo_de_preferencias,
            carga: None,
            envia_da_carga: None,
            carregando: 0,
            historico: None,
            acoes: Vec::new(),
            releitura_em: None,
            primeira_pendente: None,
            ultima_carga: None,
            ja_abriu: HashSet::new(),
            _vigia: None,
            _assinaturas: assinaturas,
        }
    }

    // ── A conta ────────────────────────────────────────────────────────────

    /// A conta entrou: os cinco fluxos abrem, e o painel começa a vigiar.
    pub fn definir_sessao(&mut self, sessao: Sessao, cx: &mut Context<Self>) {
        let (envia, recebe) = channel();
        self.sinais = (envia.clone(), recebe);
        self.conexoes.clear();
        self.ja_abriu.clear();
        let fontes = Canal::TODOS
            .into_iter()
            .map(|canal| (canal.prefixo(), canal.fluxo().to_string()))
            .collect();
        self.guarda = Some(self.escuta.escutar(sessao.clone(), fontes, envia));
        self.sessao = Some(sessao);
        self.desatualizado = true;
        self.vigiar(cx);
        if self.visivel {
            self.recarregar(cx);
        }
    }

    /// A conta saiu: fecha os fluxos e esquece o que era dela.
    pub fn sair(&mut self, cx: &mut Context<Self>) {
        self.guarda = None;
        self.sessao = None;
        self._vigia = None;
        self.pagina_do_whatsapp = PaginaDoWhatsApp::default();
        self.fora_da_pagina = PaginaDoWhatsApp::default();
        self.outros.clear();
        self.vouchers.clear();
        self.cadastros.clear();
        self.historico_aberto.clear();
        self.urgencias.clear();
        self.respostas.clear();
        self.novidades.clear();
        self.pendentes.clear();
        self.conexoes.clear();
        self.aberta = None;
        self.dialogo.largar();
        self.carregou = false;
        self.carga = None;
        self.envia_da_carga = None;
        self.historico = None;
        self.acoes.clear();
        self.carregando = 0;
        self.largar_a_midia();
        cx.notify();
    }

    /// A tela entrou ou saiu da frente.
    pub fn mostrar(&mut self, visivel: bool, cx: &mut Context<Self>) {
        let voltou = visivel && !self.visivel;
        self.visivel = visivel;
        if voltou && (self.desatualizado || !self.carregou) {
            self.recarregar(cx);
        }
        cx.notify();
    }

    /// O canal por onde os avisos do sistema devolvem o clique.
    pub fn canal_dos_cliques(&self) -> Sender<String> {
        self.cliques.0.clone()
    }

    /// O sino é um só para as telas de atendimento: lido do arquivo a cada
    /// aviso, o que a agenda mudar vale aqui.
    pub fn avisos_ligados(&self) -> bool {
        preferencias::ler(&self.arquivo_de_preferencias).avisos_do_sistema
    }

    /// Onde a preferência mora — os testes apontam a agenda para o mesmo.
    #[cfg(test)]
    pub(crate) fn arquivo_de_preferencias(&self) -> PathBuf {
        self.arquivo_de_preferencias.clone()
    }

    /// Quantas conversas têm mensagem nova — o número no menu lateral.
    pub fn quantas_novidades(&self) -> usize {
        self.novidades.len()
    }

    /// Alguém pediu atendente e está esperando.
    pub fn alguem_esperando(&self) -> bool {
        self.urgencias.iter().any(Urgencia::pediu_atendente)
    }

    /// O estado de todas as fontes juntas (o indicador).
    pub fn conexao(&self) -> EstadoDaConexao {
        if self.conexoes.is_empty() {
            return EstadoDaConexao::Conectando;
        }
        let mut estados: Vec<EstadoDaConexao> = self.conexoes.values().copied().collect();
        // Fonte que ainda não disse nada conta como conectando.
        estados
            .extend((self.conexoes.len()..Canal::TODOS.len()).map(|_| EstadoDaConexao::Conectando));
        EstadoDaConexao::de_todas(estados)
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

    /// Uma volta do laço. Devolve se vale continuar.
    pub(crate) fn passo(&mut self, cx: &mut Context<Self>) -> bool {
        if self.sessao.is_none() {
            self._vigia = None;
            return false;
        }
        let agora = cx.background_executor().now();
        let mut mudou = self.colher_sinais(agora, cx);
        mudou |= self.colher_cliques(cx);
        mudou |= self.colher_respostas(cx);
        mudou |= self.colher_escolha(cx);
        mudou |= self.colher_a_midia(cx);
        mudou |= self.colher_o_fim_do_audio(cx);
        if std::mem::take(&mut self.conferir_miniaturas) {
            self.pedir_miniaturas();
        }

        if self.visivel {
            let vencida = self.releitura_em.is_some_and(|em| em <= agora);
            let intervalo = if self.conexao() == EstadoDaConexao::Conectado {
                RELEITURA_CONECTADO
            } else {
                RELEITURA_SEM_CONEXAO
            };
            let periodica = self
                .ultima_carga
                .is_some_and(|ultima| agora.duration_since(ultima) >= intervalo);
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

    fn agendar_releitura(&mut self, agora: Instant) {
        let primeira = *self.primeira_pendente.get_or_insert(agora);
        self.releitura_em = Some((agora + ESPERA_DO_EVENTO).min(primeira + TETO_DA_ESPERA));
    }

    fn releitura_ja(&mut self, agora: Instant) {
        self.primeira_pendente.get_or_insert(agora);
        self.releitura_em = Some(agora);
    }

    fn colher_sinais(&mut self, agora: Instant, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(sinal) = self.sinais.1.try_recv() {
            mudou = true;
            match sinal {
                Sinal::Conexao { fonte, estado } => {
                    self.conexoes.insert(fonte, estado);
                }
                Sinal::Pronto { fonte } => {
                    // O primeiro `pronto` é a abertura; os seguintes são a
                    // volta de uma queda — o que chegou no intervalo se perdeu.
                    if !self.ja_abriu.insert(fonte) {
                        self.marcar_desatualizado(agora, true);
                    }
                }
                Sinal::Sincronizar { .. } => self.marcar_desatualizado(agora, true),
                Sinal::Evento { fonte, dados } => {
                    let Some(canal) = Canal::da_fonte(fonte) else {
                        continue;
                    };
                    match EventoDoChatbot::ler(canal, &dados) {
                        Some(evento) => self.receber(evento, cx),
                        None => crate::telemetria::avisar!(
                            "⚠️ [Chatbot] evento ilegível de {fonte}: {dados}"
                        ),
                    }
                    self.marcar_desatualizado(agora, false);
                }
            }
        }
        mudou
    }

    fn marcar_desatualizado(&mut self, agora: Instant, ja: bool) {
        self.desatualizado = true;
        if !self.visivel {
            return;
        }
        if ja {
            self.releitura_ja(agora);
        } else {
            self.agendar_releitura(agora);
        }
    }

    /// Um evento do domínio: acende a novidade e avisa, se for mensagem do
    /// cliente numa conversa que o operador não está olhando.
    fn receber(&mut self, evento: EventoDoChatbot, cx: &mut Context<Self>) {
        if !evento.e_mensagem_recebida() {
            return;
        }
        let chave = evento.chave();
        let na_tela = self.visivel && self.aberta.as_ref() == Some(&chave);
        if !na_tela {
            self.novidades.insert(chave);
        }
        let (Some((titulo, corpo)), Some(aviso)) = (evento.toast(), evento.notificacao()) else {
            return;
        };
        cx.emit(PedidoDoChatbot::Avisar {
            titulo,
            corpo,
            aviso,
            na_tela,
        });
        cx.emit(PedidoDoChatbot::Mudou);
    }

    fn colher_cliques(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(destino) = self.cliques.1.try_recv() {
            let Some(chave) = destino
                .strip_prefix("chatbot:")
                .and_then(modelo::Chave::do_texto)
            else {
                continue;
            };
            mudou = true;
            cx.emit(PedidoDoChatbot::TrazerParaAFrente);
            self.abrir(chave, cx);
        }
        mudou
    }

    // ── A leitura ──────────────────────────────────────────────────────────

    fn pedir(&self, pedido: PedidoJson, canal: &Sender<Recado>) {
        if let Some(sessao) = self.sessao.clone() {
            self.publicador.pedir_json(sessao, pedido, canal.clone());
        }
    }

    /// Relê tudo o que a tela mostra — o `router.refresh()` do site.
    pub fn recarregar(&mut self, cx: &mut Context<Self>) {
        if self.sessao.is_none() {
            return;
        }
        let (envia, recebe) = channel();
        let mut pedidos = vec![pedidos::conversas_do_whatsapp(
            self.status,
            &self.busca_aplicada,
            self.pagina,
        )];
        // Os quatro canais **sempre**, mesmo com uma aba ativa: é a contagem
        // do outro lado que aparece na aba.
        pedidos.extend(
            [
                Canal::Instagram,
                Canal::Web,
                Canal::Telegram,
                Canal::Messenger,
            ]
            .map(pedidos::conversas_do_canal),
        );
        pedidos.push(pedidos::respostas_rapidas());
        pedidos.push(pedidos::urgencias());
        self.carregando = pedidos.len();
        for pedido in pedidos {
            self.pedir(pedido, &envia);
        }
        self.carga = Some(recebe);
        self.envia_da_carga = Some(envia);
        if let Some(chave) = self.aberta.clone() {
            if chave.canal != Canal::WhatsApp {
                self.pedir_historico(&chave);
            }
        }
        self.desatualizado = false;
        self.releitura_em = None;
        self.primeira_pendente = None;
        self.ultima_carga = Some(cx.background_executor().now());
        cx.notify();
    }

    fn pedir_historico(&mut self, chave: &Chave) {
        let (envia, recebe) = channel();
        self.pedir(pedidos::historico(chave), &envia);
        self.historico = Some(recebe);
        self.carregando_historico = true;
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
                self.ler_da_carga(rotulo, resultado, cx);
            }
        }
        if let Some(historico) = &self.historico {
            if let Ok(Recado::Json { rotulo, resultado }) = historico.try_recv() {
                mudou = true;
                self.historico = None;
                self.carregando_historico = false;
                match (rotulo, resultado) {
                    ("historico", Ok(valor)) => {
                        self.historico_aberto = modelo::mensagens_do_canal(&valor)
                    }
                    ("wa-fora", Ok(valor)) => {
                        self.fora_da_pagina = modelo::pagina_do_whatsapp(&valor)
                    }
                    (_, Err(erro)) => {
                        crate::telemetria::avisar!("⚠️ [Chatbot] histórico: {erro}");
                        cx.emit(PedidoDoChatbot::Toast {
                            texto: modelo::explicar(&erro, "Não foi possível carregar a conversa."),
                            erro: true,
                        });
                    }
                    _ => {}
                }
            }
        }
        let mut prontas = Vec::new();
        self.acoes.retain(|(acao, canal)| match canal.try_recv() {
            Ok(Recado::Json { resultado, .. }) => {
                prontas.push((acao.clone(), resultado));
                false
            }
            // 📎 O envio de anexo sobe em multipart e a resposta vem crua: é
            // o mesmo JSON do envio de texto, só por outro caminho.
            Ok(Recado::Cru { resultado, .. }) => {
                prontas.push((
                    acao.clone(),
                    resultado.map(|r| serde_json::from_slice(&r.bytes).unwrap_or(Value::Null)),
                ));
                false
            }
            Ok(_) => true,
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
        });
        for (acao, resultado) in prontas {
            mudou = true;
            self.concluir(acao, resultado, cx);
        }
        // O que chegou pode ter trazido balão com foto.
        self.conferir_miniaturas |= mudou;
        mudou
    }

    fn ler_da_carga(
        &mut self,
        rotulo: &'static str,
        resultado: Result<Value, String>,
        cx: &mut Context<Self>,
    ) {
        let valor = match resultado {
            Ok(valor) => valor,
            Err(erro) => {
                crate::telemetria::avisar!("⚠️ [Chatbot] {rotulo}: {erro}");
                if rotulo == "wa-conversas" {
                    self.falhou_a_carga = true;
                }
                return;
            }
        };
        match rotulo {
            "wa-conversas" => {
                self.pagina_do_whatsapp = modelo::pagina_do_whatsapp(&valor);
                self.falhou_a_carga = false;
                self.carregou = true;
                let contatos: Vec<String> = self
                    .pagina_do_whatsapp
                    .conversas
                    .iter()
                    .map(|c| c.chave.id.clone())
                    .collect();
                // Dependem da página: não entram na primeira leva.
                if !contatos.is_empty() {
                    if let Some(carga) = self.envia_da_carga.clone() {
                        self.carregando += 2;
                        self.pedir(pedidos::vouchers(&contatos), &carga);
                        self.pedir(pedidos::cadastros(&contatos), &carga);
                    }
                }
                self.buscar_a_aberta_fora_da_pagina();
            }
            "vouchers" => {
                self.vouchers = serde_json::from_value::<Vec<Voucher>>(valor)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|v| (v.contact_id.clone(), v))
                    .collect();
            }
            "cadastros" => {
                // A chave é o número como está no cadastro, como no site.
                self.cadastros = serde_json::from_value::<Vec<Cadastro>>(valor)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|c| (c.whatsapp.clone(), c))
                    .collect();
            }
            "respostas" => self.respostas = RespostaRapida::lista(&valor),
            "urgencias" => {
                self.urgencias = Urgencia::lista(&valor);
                cx.emit(PedidoDoChatbot::Mudou);
            }
            outro => {
                if let Some(canal) = pedidos::canal_da_lista(outro) {
                    self.outros
                        .insert(canal, modelo::conversas_do_canal(canal, &valor));
                }
            }
        }
    }

    /// A conversa aberta do WhatsApp que não está na página listada.
    fn buscar_a_aberta_fora_da_pagina(&mut self) {
        let Some(chave) = self.aberta.clone() else {
            return;
        };
        if chave.canal != Canal::WhatsApp
            || self
                .pagina_do_whatsapp
                .conversas
                .iter()
                .any(|c| c.chave == chave)
        {
            self.fora_da_pagina = PaginaDoWhatsApp::default();
            return;
        }
        let (envia, recebe) = channel();
        self.pedir(pedidos::conversa_fora_da_pagina(&chave.id), &envia);
        self.historico = Some(recebe);
    }

    // ── O que a lista mostra ───────────────────────────────────────────────

    /// Todas as conversas dos cinco canais, cruas, com voucher e cadastro
    /// pendurados — antes de filtro nenhum.
    pub(crate) fn todas(&self) -> Vec<Conversa> {
        let mut todas: Vec<Conversa> = self.pagina_do_whatsapp.conversas.clone();
        for conversa in &self.fora_da_pagina.conversas {
            if !todas.iter().any(|c| c.chave == conversa.chave) {
                todas.push(conversa.clone());
            }
        }
        for conversa in todas.iter_mut() {
            conversa.voucher = self.vouchers.get(&conversa.chave.id).cloned();
            conversa.cadastro = self.cadastros.get(&conversa.chave.id).cloned();
        }
        for canal in [
            Canal::Instagram,
            Canal::Web,
            Canal::Telegram,
            Canal::Messenger,
        ] {
            todas.extend(self.outros.get(&canal).cloned().unwrap_or_default());
        }
        todas
    }

    /// A lista da coluna da esquerda, com todos os filtros.
    pub fn visiveis(&self) -> Vec<Conversa> {
        let mut lista = modelo::conversas_unificadas(
            &self.todas(),
            self.canal,
            self.status,
            &self.busca_aplicada,
        );
        // A conversa que está fora da página aparece aberta, mas não entra na
        // lista: a lista é a página que se pediu.
        lista.retain(|c| {
            c.chave.canal != Canal::WhatsApp
                || self
                    .pagina_do_whatsapp
                    .conversas
                    .iter()
                    .any(|p| p.chave == c.chave)
        });
        lista
    }

    /// A conversa aberta, do dado **cru**: um filtro que não casa não pode
    /// esconder a conversa que alguém acabou de abrir.
    pub fn conversa_aberta(&self) -> Option<Conversa> {
        let chave = self.aberta.as_ref()?;
        Some(
            self.todas()
                .into_iter()
                .find(|c| &c.chave == chave)
                .unwrap_or_else(|| Conversa {
                    chave: chave.clone(),
                    nome: chave.id.clone(),
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
                }),
        )
    }

    /// As mensagens da conversa aberta que o histórico mostra.
    pub fn mensagens_da_aberta(&self) -> (Vec<Mensagem>, usize) {
        let Some(chave) = &self.aberta else {
            return (Vec::new(), 0);
        };
        if chave.canal == Canal::WhatsApp {
            let mut todas = self.pagina_do_whatsapp.mensagens.clone();
            todas.extend(self.fora_da_pagina.mensagens.iter().cloned());
            modelo::historico(&todas, chave, self.tudo)
        } else {
            modelo::historico(&self.historico_aberto, chave, self.tudo)
        }
    }

    /// Os balões pendentes da conversa aberta que ainda não voltaram na
    /// leitura (`pendentesVisiveis`).
    pub fn pendentes_da_aberta(&self) -> Vec<Pendente> {
        let Some(chave) = &self.aberta else {
            return Vec::new();
        };
        self.pendentes
            .iter()
            .filter(|p| &p.chave == chave)
            .cloned()
            .collect()
    }

    /// A janela de 24 h da conversa aberta, com o relógio de agora.
    pub fn janela_da_aberta(&self, agora: chrono::DateTime<chrono::Utc>) -> modelo::Janela {
        let Some(conversa) = self.conversa_aberta() else {
            return modelo::Janela::Desconhecida;
        };
        match conversa.chave.canal {
            Canal::WhatsApp => modelo::Janela::avaliar(conversa.ultima_entrada, agora),
            Canal::Instagram | Canal::Messenger if self.carregando_historico => {
                modelo::Janela::Desconhecida
            }
            Canal::Instagram | Canal::Messenger => {
                modelo::Janela::das_mensagens(&self.historico_aberto, agora)
            }
            Canal::Web | Canal::Telegram => modelo::Janela::Desconhecida,
        }
    }

    // ── Os gestos ──────────────────────────────────────────────────────────

    pub fn escolher_canal(&mut self, canal: FiltroDeCanal, cx: &mut Context<Self>) {
        self.canal = canal;
        cx.notify();
    }

    /// Trocar o status relê: no WhatsApp o filtro é do servidor.
    pub fn escolher_status(&mut self, status: Status, cx: &mut Context<Self>) {
        if self.status == status {
            return;
        }
        self.status = status;
        self.pagina = 1;
        self.recarregar(cx);
    }

    pub fn aplicar_busca(&mut self, cx: &mut Context<Self>) {
        let busca = self.busca.read(cx).value().trim().to_string();
        if busca == self.busca_aplicada {
            return;
        }
        self.busca_aplicada = busca;
        self.pagina = 1;
        self.recarregar(cx);
    }

    /// O X de "Limpar busca".
    pub fn limpar_busca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busca
            .update(cx, |campo, cx| campo.trocar_valor("", window, cx));
        self.aplicar_busca(cx);
    }

    pub fn paginas(&self) -> u32 {
        let total = self.pagina_do_whatsapp.total.max(0) as u32;
        total.div_ceil(pedidos::POR_PAGINA).max(1)
    }

    /// "Anterior" / "Próxima" — só o WhatsApp pagina.
    pub fn mudar_pagina(&mut self, delta: i32, cx: &mut Context<Self>) {
        let nova = (self.pagina as i32 + delta).clamp(1, self.paginas() as i32) as u32;
        if nova == self.pagina {
            return;
        }
        self.pagina = nova;
        self.recarregar(cx);
    }

    /// Abre uma conversa: a novidade apaga, o histórico vem, e o que estava
    /// sendo digitado para outra pessoa não vaza para esta.
    pub fn abrir(&mut self, chave: Chave, cx: &mut Context<Self>) {
        let mesma = self.aberta.as_ref() == Some(&chave);
        self.novidades.remove(&chave);
        if !mesma {
            self.tudo = false;
            self.respostas_abertas = false;
            self.historico_aberto.clear();
            self.fora_da_pagina = PaginaDoWhatsApp::default();
            self.limpar_compositor = true;
            // O anexo escolhido era para a outra conversa, e as fotos dela
            // não precisam ficar na memória.
            self.largar_a_midia();
        }
        self.conferir_miniaturas = true;
        self.aberta = Some(chave.clone());
        if chave.canal == Canal::WhatsApp {
            self.historico = None;
            self.carregando_historico = false;
            if self.carregou {
                self.buscar_a_aberta_fora_da_pagina();
            }
        } else if !mesma || self.historico_aberto.is_empty() {
            self.pedir_historico(&chave);
        }
        cx.emit(PedidoDoChatbot::Mudou);
        cx.notify();
    }

    pub fn carregar_anteriores(&mut self, cx: &mut Context<Self>) {
        self.tudo = true;
        self.conferir_miniaturas = true;
        cx.notify();
    }

    pub fn alternar_respostas(&mut self, cx: &mut Context<Self>) {
        self.respostas_abertas = !self.respostas_abertas;
        cx.notify();
    }

    /// O Enter do compositor, e o botão "Enviar".
    pub fn enviar_o_escrito(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let texto = self.compositor.read(cx).value().trim().to_string();
        if self.bloqueado_pela_janela() {
            return;
        }
        if let Some(mut anexo) = self.anexo.take() {
            // 🎤 Áudio não leva legenda (a Cloud API não tem o campo): ele sai
            // sozinho, e o que estava escrito fica no campo para a próxima.
            if midia::classe_do_anexo(&anexo.nome) != Some(ClasseDoAnexo::Audio) {
                anexo.legenda = texto;
                self.compositor
                    .update(cx, |campo, cx| campo.trocar_valor("", window, cx));
            }
            self.despachar_anexo(anexo, cx);
            return;
        }
        if texto.is_empty() {
            return;
        }
        // O campo esvazia na hora: o balão pendente já está na conversa.
        self.compositor
            .update(cx, |campo, cx| campo.trocar_valor("", window, cx));
        self.despachar(texto, None, cx);
    }

    fn bloqueado_pela_janela(&self) -> bool {
        let Some(chave) = &self.aberta else {
            return true;
        };
        modelo::decisao_do_compositor(chave.canal, self.janela_da_aberta(chrono::Utc::now()))
            .bloqueado
    }

    /// Uma resposta rápida sai direto, e conta um uso.
    pub fn enviar_resposta(&mut self, resposta: &RespostaRapida, cx: &mut Context<Self>) {
        self.respostas_abertas = false;
        self.despachar(resposta.mensagem.clone(), Some(resposta.id.clone()), cx);
    }

    fn despachar(&mut self, texto: String, resposta: Option<String>, cx: &mut Context<Self>) {
        let Some(chave) = self.aberta.clone() else {
            return;
        };
        self.proximo_pendente += 1;
        let pendente = Pendente {
            id: self.proximo_pendente,
            chave,
            texto,
            resposta,
            erro: None,
            anexo: None,
        };
        self.mandar(&pendente);
        self.pendentes.push(pendente);
        cx.notify();
    }

    fn despachar_anexo(&mut self, anexo: Anexo, cx: &mut Context<Self>) {
        let Some(chave) = self.aberta.clone() else {
            return;
        };
        self.proximo_pendente += 1;
        let pendente = Pendente {
            id: self.proximo_pendente,
            chave,
            texto: midia::texto_do_anexo_em_voo(&anexo.nome, &anexo.legenda),
            resposta: None,
            erro: None,
            anexo: Some(anexo),
        };
        self.mandar(&pendente);
        self.pendentes.push(pendente);
        cx.notify();
    }

    fn mandar(&mut self, pendente: &Pendente) {
        let (envia, recebe) = channel();
        match &pendente.anexo {
            Some(anexo) => self.pedir_cru(
                pedidos::enviar_anexo(
                    &pendente.chave.id,
                    &anexo.legenda,
                    &anexo.nome,
                    &anexo.bytes,
                    anexo.como,
                ),
                &envia,
            ),
            None => self.pedir(pedidos::enviar(&pendente.chave, &pendente.texto), &envia),
        }
        self.acoes.push((Acao::Envio(pendente.id), recebe));
    }

    // ── 📎 A mídia ─────────────────────────────────────────────────────────

    fn pedir_cru(&self, pedido: PedidoCru, canal: &Sender<Recado>) {
        if let Some(sessao) = self.sessao.clone() {
            self.publicador.pedir_cru(sessao, pedido, canal.clone());
        }
    }

    /// Esquece o anexo escolhido e as fotos da conversa que saiu da frente.
    fn largar_a_midia(&mut self) {
        self.anexo = None;
        self.escolha = None;
        self.miniaturas.clear();
        self.baixas.clear();
        self.decodificando.clear();
        self.abrindo.clear();
        self.transcrevendo.clear();
        self.parar_o_audio();
        self.baixando_audio.clear();
    }

    /// 🎧 O áudio que está tocando agora, se há.
    pub fn tocando(&self) -> Option<&str> {
        self.tocando.as_ref().map(|(id, _)| id.as_str())
    }

    fn parar_o_audio(&mut self) {
        if let Some((_, parar)) = self.tocando.take() {
            parar.store(true, Ordering::Relaxed);
        }
    }

    /// 🎧 O ▶ do balão: toca o áudio (pelo MP3 que o servidor prepara), ou
    /// para o que está tocando. Um de cada vez — é um balcão, não uma rádio.
    pub fn tocar_audio(&mut self, mensagem_id: &str, cx: &mut Context<Self>) {
        if self.tocando() == Some(mensagem_id) {
            self.parar_o_audio();
            cx.notify();
            return;
        }
        if !self.baixando_audio.insert(mensagem_id.to_string()) {
            return;
        }
        self.parar_o_audio();
        self.baixar(
            mensagem_id,
            Baixa::Tocar,
            pedidos::baixar_previa(mensagem_id, Previa::Mp3),
        );
        cx.notify();
    }

    fn colher_o_fim_do_audio(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok((id, erro)) = self.fim_da_reproducao.1.try_recv() {
            if self.tocando() == Some(id.as_str()) {
                self.tocando = None;
                mudou = true;
            }
            if let Some(erro) = erro {
                self.toast(format!("Não foi possível tocar o áudio: {erro}"), true, cx);
            }
        }
        mudou
    }

    /// A escolha da tira: como o anexo sai.
    pub fn escolher_como(&mut self, como: ComoEnviar, cx: &mut Context<Self>) {
        if let Some(anexo) = &mut self.anexo {
            anexo.como = Some(como);
        }
        cx.notify();
    }

    /// O clipe: abre o seletor de arquivo do sistema.
    ///
    /// 🔑 Só o WhatsApp anexa, e só com a janela de 24 h aberta — anexo é
    /// mensagem de sessão como o texto. O servidor confere de novo antes de
    /// subir o arquivo para a Meta.
    pub fn anexar(&mut self, cx: &mut Context<Self>) {
        let no_whatsapp = self
            .aberta
            .as_ref()
            .is_some_and(|chave| chave.canal == Canal::WhatsApp);
        if !no_whatsapp || self.bloqueado_pela_janela() || self.escolha.is_some() {
            return;
        }
        let (envia, recebe) = channel();
        self.seletor.escolher_arquivo(envia, cx);
        self.escolha = Some(recebe);
    }

    /// O "x" da tira do anexo.
    pub fn tirar_anexo(&mut self, cx: &mut Context<Self>) {
        self.anexo = None;
        cx.notify();
    }

    fn colher_escolha(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(escolha) = &self.escolha else {
            return false;
        };
        let recado = match escolha.try_recv() {
            Ok(recado) => recado,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => {
                self.escolha = None;
                return false;
            }
        };
        self.escolha = None;
        match recado {
            RecadoDoSeletor::ArquivoEscolhido(caminho) => {
                self.receber_o_escolhido(Path::new(&caminho), cx)
            }
            RecadoDoSeletor::Falhou(erro) => self.toast(erro, true, cx),
            // Desistir do seletor não é erro.
            _ => {}
        }
        true
    }

    /// Confere e carrega o arquivo escolhido. O que não serve é recusado com
    /// o motivo, **antes** de ler o arquivo inteiro para a memória.
    fn receber_o_escolhido(&mut self, caminho: &Path, cx: &mut Context<Self>) {
        let nome = caminho
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let tamanho = std::fs::metadata(caminho).map(|m| m.len()).unwrap_or(0);
        if let Some(recusa) = midia::recusa_do_anexo(&nome, tamanho) {
            self.toast(recusa, true, cx);
            return;
        }
        match std::fs::read(caminho) {
            Ok(bytes) => {
                let como = ComoEnviar::opcoes(&nome).first().copied();
                self.anexo = Some(Anexo {
                    nome,
                    bytes: Arc::new(bytes),
                    legenda: String::new(),
                    como,
                })
            }
            Err(erro) => self.toast(format!("Não foi possível ler o arquivo: {erro}"), true, cx),
        }
    }

    /// A mídia de uma mensagem da conversa aberta.
    fn midia_da_mensagem(&self, mensagem_id: &str) -> Option<Midia> {
        self.mensagens_da_aberta()
            .0
            .into_iter()
            .find(|m| m.id == mensagem_id)
            .and_then(|m| m.midia)
    }

    fn baixar(&mut self, mensagem_id: &str, baixa: Baixa, pedido: PedidoCru) {
        let (envia, recebe) = channel();
        self.pedir_cru(pedido, &envia);
        self.baixas.push((mensagem_id.to_string(), baixa, recebe));
    }

    /// Pede a foto de cada balão da conversa aberta que ainda não a tem.
    fn pedir_miniaturas(&mut self) {
        let faltam: Vec<(String, bool)> = self
            .mensagens_da_aberta()
            .0
            .into_iter()
            .filter_map(|m| {
                let midia = m.midia.as_ref()?;
                // Só a que já está guardada: a que a Meta ainda não entregou
                // espera o clique — ver `Midia::guardada`. O vídeo entra pela
                // capa, que o servidor prepara.
                let e_video = midia.tipo == TipoDeMidia::Video;
                ((midia.tipo.aparece_no_balao() || e_video)
                    && midia.guardada
                    && !self.miniaturas.contains_key(&m.id))
                .then_some((m.id, e_video))
            })
            .collect();
        for (id, e_video) in faltam {
            self.miniaturas.insert(id.clone(), Miniatura::Baixando);
            let pedido = if e_video {
                pedidos::baixar_previa(&id, Previa::Quadro)
            } else {
                pedidos::baixar_midia(&id)
            };
            self.baixar(&id, Baixa::Miniatura, pedido);
        }
    }

    /// O clique na foto ou no cartão do arquivo: baixa e abre no programa do
    /// sistema — o leitor de PDF, o tocador de áudio, o visualizador de fotos.
    pub fn abrir_midia(&mut self, mensagem_id: &str, cx: &mut Context<Self>) {
        if self.abrindo.contains(mensagem_id) {
            return;
        }
        let Some(midia) = self.midia_da_mensagem(mensagem_id) else {
            return;
        };
        self.abrindo.insert(mensagem_id.to_string());
        self.baixar(
            mensagem_id,
            Baixa::Abrir(midia),
            pedidos::baixar_midia(mensagem_id),
        );
        cx.notify();
    }

    /// 🎤 "Transcrever", no áudio que ainda não tem texto.
    pub fn transcrever(&mut self, mensagem_id: &str, cx: &mut Context<Self>) {
        if !self.transcrevendo.insert(mensagem_id.to_string()) {
            return;
        }
        let (envia, recebe) = channel();
        self.pedir(pedidos::transcrever(mensagem_id), &envia);
        self.acoes
            .push((Acao::Transcricao(mensagem_id.to_string()), recebe));
        cx.notify();
    }

    fn colher_a_midia(&mut self, cx: &mut Context<Self>) -> bool {
        let mut baixadas = Vec::new();
        self.baixas
            .retain(|(id, baixa, canal)| match canal.try_recv() {
                Ok(Recado::Cru { resultado, .. }) => {
                    baixadas.push((id.clone(), baixa.clone(), resultado));
                    false
                }
                Ok(_) | Err(TryRecvError::Empty) => true,
                Err(TryRecvError::Disconnected) => {
                    baixadas.push((id.clone(), baixa.clone(), Err("sem resposta".into())));
                    false
                }
            });
        let mut mudou = !baixadas.is_empty();
        for (id, baixa, resultado) in baixadas {
            match (baixa, resultado) {
                (Baixa::Miniatura, Ok(resposta)) => {
                    let (envia, recebe) = channel();
                    std::thread::spawn(move || {
                        let _ = envia.send(miniatura_de(&resposta.bytes));
                    });
                    self.decodificando.push((id, recebe));
                }
                (Baixa::Miniatura, Err(erro)) => {
                    // Sem toast: a foto que não veio vira o cartão de arquivo,
                    // e é o clique nele que diz o motivo.
                    crate::telemetria::avisar!("⚠️ [Chatbot] foto do balão: {erro}");
                    self.miniaturas.insert(id, Miniatura::Falhou);
                }
                (Baixa::Abrir(midia), Ok(resposta)) => {
                    self.abrindo.remove(&id);
                    let nome = midia.nome_para_abrir(&id, resposta.tipo.as_deref());
                    match gravar_e_abrir(&nome, &resposta.bytes) {
                        Ok(caminho) => self.abertos.push(caminho),
                        Err(erro) => self.toast(erro, true, cx),
                    }
                }
                (Baixa::Tocar, Ok(resposta)) => {
                    self.baixando_audio.remove(&id);
                    self.parar_o_audio();
                    let parar = Arc::new(AtomicBool::new(false));
                    tocar_mp3(
                        id.clone(),
                        resposta.bytes,
                        parar.clone(),
                        self.fim_da_reproducao.0.clone(),
                    );
                    self.tocando = Some((id, parar));
                }
                (Baixa::Tocar, Err(erro)) => {
                    self.baixando_audio.remove(&id);
                    self.toast(
                        modelo::explicar(&erro, "Não foi possível tocar o áudio."),
                        true,
                        cx,
                    );
                }
                (Baixa::Abrir(_), Err(erro)) => {
                    self.abrindo.remove(&id);
                    self.toast(
                        modelo::explicar(&erro, "Não foi possível abrir o arquivo."),
                        true,
                        cx,
                    );
                }
            }
        }

        let mut decodificadas = Vec::new();
        self.decodificando
            .retain(|(id, canal)| match canal.try_recv() {
                Ok(imagem) => {
                    decodificadas.push((id.clone(), imagem));
                    false
                }
                Err(TryRecvError::Empty) => true,
                Err(TryRecvError::Disconnected) => {
                    decodificadas.push((id.clone(), None));
                    false
                }
            });
        mudou |= !decodificadas.is_empty();
        for (id, imagem) in decodificadas {
            self.miniaturas
                .insert(id, imagem.map_or(Miniatura::Falhou, Miniatura::Pronta));
        }
        mudou
    }

    /// "Tentar de novo" no balão que não foi.
    pub fn reenviar(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(pendente) = self.pendentes.iter_mut().find(|p| p.id == id) else {
            return;
        };
        pendente.erro = None;
        let pendente = pendente.clone();
        self.mandar(&pendente);
        cx.notify();
    }

    /// "Descartar" no balão que não foi.
    pub fn descartar(&mut self, id: u64, cx: &mut Context<Self>) {
        self.pendentes.retain(|p| p.id != id);
        cx.notify();
    }

    /// "Assumir" ou "Devolver ao bot". No site o assumir pergunta antes quem
    /// está assumindo.
    pub fn alternar_atendimento(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(conversa) = self.conversa_aberta() else {
            return;
        };
        let atender = !conversa.atendimento_humano;
        if atender && conversa.chave.canal == Canal::Web {
            let nome = self.preferencias.atendente.clone();
            self.nome_do_atendente
                .update(cx, |campo, cx| campo.trocar_valor(nome, window, cx));
            self.dialogo
                .abrir(Dialogo::QuemAssume(conversa.chave), window, cx);
            cx.notify();
            return;
        }
        self.mandar_alternancia(conversa.chave, atender, None, cx);
    }

    /// "Assumir conversa" no "Quem está assumindo?".
    pub fn confirmar_quem_assume(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialogo::QuemAssume(chave)) = self.dialogo.aberto().cloned() else {
            return;
        };
        let nome = self.nome_do_atendente.read(cx).value().trim().to_string();
        if !nome.is_empty() {
            self.preferencias.atendente = nome.clone();
            preferencias::guardar(&self.arquivo_de_preferencias, &self.preferencias);
        }
        self.dialogo.fechar(window, cx);
        self.mandar_alternancia(chave, true, Some(nome), cx);
    }

    fn mandar_alternancia(
        &mut self,
        chave: Chave,
        atender: bool,
        atendente: Option<String>,
        cx: &mut Context<Self>,
    ) {
        // Otimista: o botão troca na hora, e volta se o servidor recusar.
        self.marcar_atendimento(&chave, atender);
        let (envia, recebe) = channel();
        self.pedir(
            pedidos::alternar(&chave, atender, atendente.as_deref()),
            &envia,
        );
        self.acoes.push((Acao::Alternar { chave, atender }, recebe));
        cx.notify();
    }

    fn marcar_atendimento(&mut self, chave: &Chave, atender: bool) {
        let listas = [
            &mut self.pagina_do_whatsapp.conversas,
            &mut self.fora_da_pagina.conversas,
        ];
        for lista in listas {
            for c in lista.iter_mut().filter(|c| &c.chave == chave) {
                c.atendimento_humano = atender;
            }
        }
        if let Some(lista) = self.outros.get_mut(&chave.canal) {
            for c in lista.iter_mut().filter(|c| &c.chave == chave) {
                c.atendimento_humano = atender;
            }
        }
    }

    /// "Copiar número" / "Copiar IGSID" / "Copiar ID da conversa".
    pub fn copiar_id(&mut self, cx: &mut Context<Self>) {
        let Some(chave) = self.aberta.clone() else {
            return;
        };
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(chave.id.clone()));
        let texto = match chave.canal {
            Canal::WhatsApp => "Número copiado.",
            Canal::Instagram => "IGSID copiado.",
            _ => "ID da conversa copiado.",
        };
        cx.emit(PedidoDoChatbot::Toast {
            texto: texto.into(),
            erro: false,
        });
        cx.notify();
    }

    pub fn pedir_exclusao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chave) = self.aberta.clone().filter(|c| c.canal == Canal::WhatsApp) else {
            return;
        };
        self.confirmacao
            .update(cx, |campo, cx| campo.trocar_valor("", window, cx));
        self.dialogo
            .abrir(Dialogo::ExcluirHistorico(chave), window, cx);
        cx.notify();
    }

    pub fn exclusao_confirmada(&self, cx: &gpui_kit::App) -> bool {
        self.confirmacao.read(cx).value().as_ref() == pedidos::CONFIRMACAO_DE_EXCLUSAO
    }

    /// "Excluir histórico", só com a frase letra por letra.
    pub fn excluir_historico(&mut self, cx: &mut Context<Self>) {
        let Some(Dialogo::ExcluirHistorico(chave)) = self.dialogo.aberto().cloned() else {
            return;
        };
        if !self.exclusao_confirmada(cx) || self.em_acao {
            return;
        }
        self.em_acao = true;
        let (envia, recebe) = channel();
        self.pedir(pedidos::apagar_historico(&chave.id), &envia);
        self.acoes.push((Acao::ApagarHistorico, recebe));
        cx.notify();
    }

    // ── Urgências ──────────────────────────────────────────────────────────

    pub fn abrir_urgencias(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialogo.abrir(Dialogo::Urgencias, window, cx);
        cx.notify();
    }

    pub fn fechar_dialogo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.em_acao {
            return;
        }
        // Resolver e Descartar abrem de dentro da lista: cancelar volta a ela.
        match self.dialogo.aberto() {
            Some(Dialogo::Resolver(_)) | Some(Dialogo::Descartar(_)) => {
                self.dialogo.abrir(Dialogo::Urgencias, window, cx)
            }
            _ => {
                self.dialogo.fechar(window, cx);
            }
        }
        cx.notify();
    }

    pub fn assumir_urgencia(&mut self, urgencia: &Urgencia, cx: &mut Context<Self>) {
        self.mudar_urgencia(urgencia, MudancaDaUrgencia::Assumir, cx);
    }

    pub fn pedir_resolucao(
        &mut self,
        urgencia: Urgencia,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.notas
            .update(cx, |campo, cx| campo.trocar_valor("", window, cx));
        self.dialogo.abrir(Dialogo::Resolver(urgencia), window, cx);
        cx.notify();
    }

    pub fn pedir_descarte(
        &mut self,
        urgencia: Urgencia,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dialogo.abrir(Dialogo::Descartar(urgencia), window, cx);
        cx.notify();
    }

    /// "Confirmar resolução" — as notas são obrigatórias.
    pub fn confirmar_resolucao(&mut self, cx: &mut Context<Self>) {
        let Some(Dialogo::Resolver(urgencia)) = self.dialogo.aberto().cloned() else {
            return;
        };
        if self.notas.read(cx).value().trim().is_empty() {
            return;
        }
        self.mudar_urgencia(&urgencia, MudancaDaUrgencia::Resolver, cx);
    }

    pub fn confirmar_descarte(&mut self, cx: &mut Context<Self>) {
        let Some(Dialogo::Descartar(urgencia)) = self.dialogo.aberto().cloned() else {
            return;
        };
        self.mudar_urgencia(&urgencia, MudancaDaUrgencia::Descartar, cx);
    }

    fn mudar_urgencia(
        &mut self,
        urgencia: &Urgencia,
        mudanca: MudancaDaUrgencia,
        cx: &mut Context<Self>,
    ) {
        if self.em_acao {
            return;
        }
        self.em_acao = true;
        let notas = self.notas.read(cx).value().to_string();
        let (envia, recebe) = channel();
        self.pedir(
            pedidos::mudar_urgencia(&urgencia.id, mudanca, &notas),
            &envia,
        );
        self.acoes.push((Acao::Urgencia(mudanca), recebe));
        cx.notify();
    }

    /// "Ver conversa" de uma urgência: as urgências são do WhatsApp.
    pub fn ver_conversa_da_urgencia(&mut self, urgencia: &Urgencia, cx: &mut Context<Self>) {
        self.dialogo.fechar_depois(cx);
        self.abrir(
            Chave::nova(Canal::WhatsApp, urgencia.contact_id.clone()),
            cx,
        );
    }

    /// O sino: liga e desliga os avisos do sistema.
    pub fn alternar_avisos(&mut self, cx: &mut Context<Self>) {
        self.preferencias = preferencias::ler(&self.arquivo_de_preferencias);
        self.preferencias.avisos_do_sistema = !self.preferencias.avisos_do_sistema;
        preferencias::guardar(&self.arquivo_de_preferencias, &self.preferencias);
        cx.notify();
    }

    // ── As respostas dos gestos ────────────────────────────────────────────

    fn toast(&self, texto: impl Into<String>, erro: bool, cx: &mut Context<Self>) {
        cx.emit(PedidoDoChatbot::Toast {
            texto: texto.into(),
            erro,
        });
    }

    fn concluir(&mut self, acao: Acao, resultado: Result<Value, String>, cx: &mut Context<Self>) {
        match acao {
            Acao::Envio(id) => match resultado {
                Ok(_) => {
                    let resposta = self
                        .pendentes
                        .iter()
                        .find(|p| p.id == id)
                        .and_then(|p| p.resposta.clone());
                    self.pendentes.retain(|p| p.id != id);
                    // Falhar em contar o uso não desfaz a mensagem que saiu.
                    if let Some(resposta) = resposta {
                        let (envia, recebe) = channel();
                        self.pedir(pedidos::contar_uso(&resposta), &envia);
                        self.acoes.push((Acao::ContarUso, recebe));
                    }
                    self.recarregar(cx);
                }
                Err(erro) => {
                    if let Some(p) = self.pendentes.iter_mut().find(|p| p.id == id) {
                        p.erro = Some(modelo::explicar(
                            &erro,
                            "Não foi possível enviar a mensagem.",
                        ));
                    }
                }
            },
            Acao::Transcricao(id) => {
                self.transcrevendo.remove(&id);
                match resultado {
                    // O texto chega com a releitura, no `midia` da mensagem.
                    Ok(_) => self.recarregar(cx),
                    Err(erro) => self.toast(
                        modelo::explicar(&erro, "Não foi possível transcrever o áudio."),
                        true,
                        cx,
                    ),
                }
            }
            Acao::Alternar { chave, atender } => match resultado {
                Ok(_) => {
                    self.toast(
                        if atender {
                            "Você assumiu a conversa — o bot está calado."
                        } else {
                            "Conversa devolvida ao bot."
                        },
                        false,
                        cx,
                    );
                    self.recarregar(cx);
                }
                Err(erro) => {
                    self.marcar_atendimento(&chave, !atender);
                    self.toast(modelo::explicar(&erro, "Erro ao executar ação"), true, cx);
                }
            },
            Acao::Urgencia(mudanca) => {
                self.em_acao = false;
                let (certo, errado) = match mudanca {
                    MudancaDaUrgencia::Assumir => {
                        ("Marcado como em atendimento", "Erro ao assumir a urgência")
                    }
                    MudancaDaUrgencia::Resolver => (
                        "Urgência resolvida com sucesso!",
                        "Erro ao resolver a urgência",
                    ),
                    MudancaDaUrgencia::Descartar => ("Urgência descartada", "Erro ao descartar"),
                };
                match resultado {
                    Ok(_) => {
                        self.toast(certo, false, cx);
                        if matches!(
                            self.dialogo.aberto(),
                            Some(Dialogo::Resolver(_)) | Some(Dialogo::Descartar(_))
                        ) {
                            // Continua aberto: a volta para a lista não mexe no foco guardado.
                            self.dialogo.abrir_sem_janela(Dialogo::Urgencias);
                        }
                        self.recarregar(cx);
                    }
                    Err(erro) => self.toast(modelo::explicar(&erro, errado), true, cx),
                }
            }
            Acao::ApagarHistorico => {
                self.em_acao = false;
                match resultado {
                    Ok(valor) => {
                        let apagadas = valor.as_i64().unwrap_or(0);
                        self.toast(
                            format!(
                                "Histórico de mensagens excluído com sucesso ({apagadas} mensagens)"
                            ),
                            false,
                            cx,
                        );
                        self.dialogo.fechar_depois(cx);
                        self.recarregar(cx);
                    }
                    Err(erro) => self.toast(
                        modelo::explicar(&erro, "Erro ao excluir mensagens"),
                        true,
                        cx,
                    ),
                }
            }
            Acao::ContarUso => {}
        }
    }

    /// O texto da tela, para o desenho.
    pub(crate) fn linha_de_resumo(&self) -> SharedString {
        let visiveis = self.visiveis();
        let com_atendente = visiveis.iter().filter(|c| c.atendimento_humano).count();
        format!(
            "{} conversas · {} com bot · {} em atendimento",
            visiveis.len(),
            visiveis.len() - com_atendente,
            com_atendente
        )
        .into()
    }
}
