//! ⏱️ **Desempenho** — a ferramenta de medição do rodapé.
//!
//! # O pedido (dono, 2026-09-27)
//!
//! A renderização está lenta, especialmente no Windows, e a pergunta é *onde*:
//! quantos quadros a janela produziu, quais engasgaram e em qual etapa da CPU ou
//! da GPU o tempo foi gasto — e em que máquina, para separar defeito do app de
//! limite do hardware ("Nesse relatório traga informações do Hardware, sistema
//! operacional e drivers").
//!
//! # O que é medido, e como
//!
//! | O quê | Onde | Como |
//! |---|---|---|
//! | intervalo entre quadros | janela principal | fim da pintura de um quadro ao do seguinte ([`sentinela`]) |
//! | montagem da interface | thread da interface | `render` da raiz → fim da pintura |
//! | apresentação (aprox.) | thread da interface | fim da pintura → primeira tarefa depois do `present` |
//! | decodificação, recorte, histograma, conversão | CPU | [`medir`] em volta de cada trecho |
//! | redução, preparo, gravação, espera, leitura | thread do motor | `revelacao_core::TemposDoMotor` |
//! | máscaras, retoques, revelação | **GPU** | timestamp query (`revelacao_core::cronometro`) |
//! | foto na tela | ponta a ponta | do pedido ao palco |
//!
//! 🚨 **O FPS não é contagem de `render`.** Uma view pode renderizar sem que a
//! janela apresente, e o GPUI desenha a janela inteira quando qualquer view
//! avisa. O que conta é o quadro que **a janela concluiu**: o sentinela é um
//! elemento pintado por último em cada quadro, e o intervalo entre duas
//! pinturas dele é o intervalo entre quadros.
//!
//! 🚨 **CPU e GPU não se somam.** O motor revela numa thread própria enquanto a
//! interface desenha; a GPU trabalha enquanto a CPU espera. O "tempo do
//! quadro" é o observado na thread da interface, e cada etapa é mostrada ao
//! lado dele, não como parcela.
//!
//! # O que não dá para medir (e é dito assim na tela)
//!
//! - **O tempo de GPU do próprio GPUI** (DirectX 11 no Windows, Metal no macOS,
//!   Vulkan no Linux): o GPUI 0.3 não expõe instrumentação do renderizador. A
//!   subida das imagens para o atlas dele também fica dentro do `present`.
//! - **O fim exato do `present`**: sem gancho público, a "apresentação" é até a
//!   primeira tarefa que a thread roda depois do quadro — um teto, não o valor.
//! - **O tempo de GPU sem `TIMESTAMP_QUERY`**: sobram as etapas de CPU e a
//!   espera pela GPU, que inclui fila e driver.
//!
//! # Custo
//!
//! 🔑 **Desligada, cada ponto de medição é uma leitura atômica**, e o sentinela
//! nem entra na árvore. Ligada, cada quadro custa uma trava curta e uma tarefa;
//! nada toca o disco no caminho do quadro — salvar é um lote em segundo plano
//! ([`porta`]).

pub mod coletor;
pub mod maquina;
pub mod painel;
pub mod porta;
pub mod relatorio;
pub mod sentinela;
pub mod vigia;

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

pub use coletor::{Coletor, Imagem, Resumo, RevelacaoNoMotor};

// ── As operações ───────────────────────────────────────────────────────────

/// O que o operador está fazendo — a quem os quadros e as etapas pertencem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Operacao {
    Nenhuma,
    Rolagem,
    TrocaDeFoto,
    AberturaDaRevelacao,
    ArrastoDeSlider,
    Pincel,
    Gradiente,
    Laco,
    Clone,
    Heal,
    Preencher,
}

impl Operacao {
    pub const TODAS: [Operacao; 10] = [
        Operacao::Rolagem,
        Operacao::TrocaDeFoto,
        Operacao::AberturaDaRevelacao,
        Operacao::ArrastoDeSlider,
        Operacao::Pincel,
        Operacao::Gradiente,
        Operacao::Laco,
        Operacao::Clone,
        Operacao::Heal,
        Operacao::Preencher,
    ];

    /// O nome estável — é o que vai para o banco.
    pub fn nome(self) -> &'static str {
        match self {
            Operacao::Nenhuma => "nenhuma",
            Operacao::Rolagem => "rolagem",
            Operacao::TrocaDeFoto => "troca_de_foto",
            Operacao::AberturaDaRevelacao => "abertura_da_revelacao",
            Operacao::ArrastoDeSlider => "arrasto_de_slider",
            Operacao::Pincel => "pincel",
            Operacao::Gradiente => "gradiente",
            Operacao::Laco => "laco",
            Operacao::Clone => "clone",
            Operacao::Heal => "heal",
            Operacao::Preencher => "preencher",
        }
    }

    pub fn do_nome(nome: &str) -> Operacao {
        Self::TODAS
            .into_iter()
            .find(|o| o.nome() == nome)
            .unwrap_or(Operacao::Nenhuma)
    }

    /// Como a tela escreve.
    pub fn rotulo(self) -> &'static str {
        match self {
            Operacao::Nenhuma => "sem operação",
            Operacao::Rolagem => "rolagem",
            Operacao::TrocaDeFoto => "troca de foto",
            Operacao::AberturaDaRevelacao => "abertura da Revelação",
            Operacao::ArrastoDeSlider => "arrasto de slider",
            Operacao::Pincel => "pincel",
            Operacao::Gradiente => "gradiente",
            Operacao::Laco => "laço",
            Operacao::Clone => "clone (carimbo)",
            Operacao::Heal => "heal (band-aid)",
            Operacao::Preencher => "preencher (Content-Aware)",
        }
    }

    /// Um gesto contínuo: enquanto ele dura, a janela deveria produzir um
    /// quadro por batida do monitor.
    pub fn continua(self) -> bool {
        !matches!(
            self,
            Operacao::Nenhuma | Operacao::TrocaDeFoto | Operacao::AberturaDaRevelacao
        )
    }

    /// Quanto a operação dura depois do último sinal dela. Os gestos contínuos
    /// sinalizam a cada evento; a troca de foto e a abertura sinalizam uma vez
    /// e o trabalho que provocam (decodificar, revelar) chega depois.
    pub fn duracao_us(self) -> u64 {
        let ms = match self {
            Operacao::Nenhuma => 0,
            Operacao::Rolagem => 250,
            Operacao::ArrastoDeSlider => 400,
            Operacao::Pincel
            | Operacao::Gradiente
            | Operacao::Laco
            | Operacao::Clone
            | Operacao::Heal
            | Operacao::Preencher => 400,
            Operacao::TrocaDeFoto => 1_200,
            Operacao::AberturaDaRevelacao => 2_000,
        };
        ms * 1000
    }
}

// ── As etapas ──────────────────────────────────────────────────────────────

/// Onde uma etapa roda — o que diz se ela prende quadro ou só atrasa a foto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Onde {
    Quadro,
    CpuInterface,
    CpuFundo,
    Gpu,
    PontaAPonta,
}

impl Onde {
    pub fn nome(self) -> &'static str {
        match self {
            Onde::Quadro => "quadro",
            Onde::CpuInterface => "cpu_interface",
            Onde::CpuFundo => "cpu_fundo",
            Onde::Gpu => "gpu",
            Onde::PontaAPonta => "ponta_a_ponta",
        }
    }
    pub fn rotulo(self) -> &'static str {
        match self {
            Onde::Quadro => "quadro",
            Onde::CpuInterface => "CPU, thread da interface",
            Onde::CpuFundo => "CPU, em segundo plano",
            Onde::Gpu => "GPU",
            Onde::PontaAPonta => "ponta a ponta",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Etapa {
    IntervaloDoQuadro,
    TempoDoQuadro,
    MontagemDaInterface,
    Apresentacao,
    Decodificacao,
    PreparacaoDosAjustes,
    RecorteNaCpu,
    Histograma,
    ConversaoParaExibicao,
    ReducaoNoMotor,
    PreparoNoMotor,
    GravacaoNoMotor,
    EsperaPelaGpu,
    LeituraDaGpu,
    MotorTotal,
    GpuMascaras,
    GpuRetoques,
    GpuRevelacao,
    GpuMotorTotal,
    LatenciaDaFoto,
    EntradaAoQuadro,
}

/// As etapas que podem rodar na thread da interface e entram no quadro.
pub const ETAPAS_DA_INTERFACE: [Etapa; 5] = [
    Etapa::Decodificacao,
    Etapa::PreparacaoDosAjustes,
    Etapa::RecorteNaCpu,
    Etapa::Histograma,
    Etapa::ConversaoParaExibicao,
];

impl Etapa {
    pub const TODAS: [Etapa; 21] = [
        Etapa::IntervaloDoQuadro,
        Etapa::TempoDoQuadro,
        Etapa::MontagemDaInterface,
        Etapa::Apresentacao,
        Etapa::Decodificacao,
        Etapa::PreparacaoDosAjustes,
        Etapa::RecorteNaCpu,
        Etapa::Histograma,
        Etapa::ConversaoParaExibicao,
        Etapa::ReducaoNoMotor,
        Etapa::PreparoNoMotor,
        Etapa::GravacaoNoMotor,
        Etapa::EsperaPelaGpu,
        Etapa::LeituraDaGpu,
        Etapa::MotorTotal,
        Etapa::GpuMascaras,
        Etapa::GpuRetoques,
        Etapa::GpuRevelacao,
        Etapa::GpuMotorTotal,
        Etapa::LatenciaDaFoto,
        Etapa::EntradaAoQuadro,
    ];

    pub fn nome(self) -> &'static str {
        match self {
            Etapa::IntervaloDoQuadro => "intervalo_do_quadro",
            Etapa::TempoDoQuadro => "tempo_do_quadro",
            Etapa::MontagemDaInterface => "montagem_da_interface",
            Etapa::Apresentacao => "apresentacao",
            Etapa::Decodificacao => "decodificacao",
            Etapa::PreparacaoDosAjustes => "preparacao_dos_ajustes",
            Etapa::RecorteNaCpu => "recorte_na_cpu",
            Etapa::Histograma => "histograma",
            Etapa::ConversaoParaExibicao => "conversao_para_exibicao",
            Etapa::ReducaoNoMotor => "reducao_no_motor",
            Etapa::PreparoNoMotor => "preparo_no_motor",
            Etapa::GravacaoNoMotor => "gravacao_no_motor",
            Etapa::EsperaPelaGpu => "espera_pela_gpu",
            Etapa::LeituraDaGpu => "leitura_da_gpu",
            Etapa::MotorTotal => "motor_total",
            Etapa::GpuMascaras => "gpu_mascaras",
            Etapa::GpuRetoques => "gpu_retoques",
            Etapa::GpuRevelacao => "gpu_revelacao",
            Etapa::GpuMotorTotal => "gpu_motor_total",
            Etapa::LatenciaDaFoto => "latencia_da_foto",
            Etapa::EntradaAoQuadro => "entrada_ao_quadro",
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Etapa::IntervaloDoQuadro => "intervalo entre quadros",
            Etapa::TempoDoQuadro => "tempo observado do quadro",
            Etapa::MontagemDaInterface => "montagem da interface (render, layout, pintura)",
            Etapa::Apresentacao => "apresentação (aprox., até a tarefa seguinte)",
            Etapa::Decodificacao => "leitura/decodificação da foto",
            Etapa::PreparacaoDosAjustes => "preparação dos ajustes",
            Etapa::RecorteNaCpu => "recorte e giro na CPU",
            Etapa::Histograma => "histograma",
            Etapa::ConversaoParaExibicao => "conversão da imagem para exibição",
            Etapa::ReducaoNoMotor => "redução da foto para a tela",
            Etapa::PreparoNoMotor => "preparo no motor (upload, grades, uniformes)",
            Etapa::GravacaoNoMotor => "gravação e envio dos comandos",
            Etapa::EsperaPelaGpu => "espera pela GPU (fila, driver, shaders, cópia)",
            Etapa::LeituraDaGpu => "cópia da foto revelada para a CPU",
            Etapa::MotorTotal => "revelação no motor (total)",
            Etapa::GpuMascaras => "máscaras na GPU",
            Etapa::GpuRetoques => "retoques na GPU",
            Etapa::GpuRevelacao => "revelação na GPU",
            Etapa::GpuMotorTotal => "GPU do motor (1ª à última passada)",
            Etapa::LatenciaDaFoto => "do pedido à foto na tela",
            Etapa::EntradaAoQuadro => "do gesto processado ao quadro seguinte",
        }
    }

    pub fn onde(self) -> Onde {
        match self {
            Etapa::IntervaloDoQuadro
            | Etapa::TempoDoQuadro
            | Etapa::MontagemDaInterface
            | Etapa::Apresentacao => Onde::Quadro,
            Etapa::PreparacaoDosAjustes
            | Etapa::RecorteNaCpu
            | Etapa::Histograma
            | Etapa::ConversaoParaExibicao => Onde::CpuInterface,
            Etapa::Decodificacao
            | Etapa::ReducaoNoMotor
            | Etapa::PreparoNoMotor
            | Etapa::GravacaoNoMotor
            | Etapa::EsperaPelaGpu
            | Etapa::LeituraDaGpu
            | Etapa::MotorTotal => Onde::CpuFundo,
            Etapa::GpuMascaras
            | Etapa::GpuRetoques
            | Etapa::GpuRevelacao
            | Etapa::GpuMotorTotal => Onde::Gpu,
            Etapa::LatenciaDaFoto | Etapa::EntradaAoQuadro => Onde::PontaAPonta,
        }
    }

    pub fn indice_na_interface(self) -> Option<usize> {
        ETAPAS_DA_INTERFACE.iter().position(|e| *e == self)
    }

    /// Entra na disputa de "maior gargalo": as etapas que são parte do
    /// trabalho, e não os totais que já as contêm.
    pub fn candidata_a_gargalo(self) -> bool {
        !matches!(
            self,
            Etapa::IntervaloDoQuadro
                | Etapa::TempoDoQuadro
                | Etapa::MotorTotal
                | Etapa::GpuMotorTotal
                | Etapa::LatenciaDaFoto
                | Etapa::EntradaAoQuadro
        )
    }
}

// ── O estado global ────────────────────────────────────────────────────────

/// O contexto da captura: onde e em quê ela roda.
#[derive(Clone, Debug, Default)]
pub struct Contexto {
    pub janela_px: (u32, u32),
    pub escala: f32,
    pub hz: f32,
    /// `sistema`, `estimada` ou `padrao`.
    pub hz_origem: &'static str,
    /// A GPU que o motor abriu (quando já abriu).
    pub motor: Option<revelacao_core::InfoDoAdaptador>,
}

struct Captura {
    inicio: Instant,
    iniciada_em: chrono::DateTime<chrono::Utc>,
    id: String,
    coletor: Coletor,
    contexto: Contexto,
    thread_da_interface: ThreadId,
}

static ATIVA: AtomicBool = AtomicBool::new(false);
static CAPTURA: Mutex<Option<Captura>> = Mutex::new(None);

/// A captura está ligada. Uma leitura atômica — é o que cada ponto de medição
/// paga com a ferramenta desligada.
#[inline]
pub fn ativa() -> bool {
    ATIVA.load(Ordering::Relaxed)
}

fn com<R>(f: impl FnOnce(&mut Captura, u64) -> R) -> Option<R> {
    if !ativa() {
        return None;
    }
    let mut guarda = CAPTURA.lock();
    let c = guarda.as_mut()?;
    let t = c.inicio.elapsed().as_micros() as u64;
    Some(f(c, t))
}

/// Liga a captura. Chamado da thread da interface (o botão "Iniciar").
pub fn iniciar(contexto: Contexto) {
    let mut coletor = Coletor::novo(if contexto.hz > 0.0 { contexto.hz } else { 60.0 });
    coletor.tela("?");
    *CAPTURA.lock() = Some(Captura {
        inicio: Instant::now(),
        iniciada_em: chrono::Utc::now(),
        id: uuid::Uuid::new_v4().to_string(),
        coletor,
        contexto,
        thread_da_interface: std::thread::current().id(),
    });
    ATIVA.store(true, Ordering::Release);
    vigia::ligar();
}

/// Uma captura encerrada, pronta para virar relatório e ir ao banco.
pub struct Encerrada {
    pub id: String,
    pub iniciada_em: chrono::DateTime<chrono::Utc>,
    pub terminada_em: chrono::DateTime<chrono::Utc>,
    pub duracao: Duration,
    pub coletor: Coletor,
    pub contexto: Contexto,
    pub origem: &'static str,
}

/// Desliga e devolve o que foi capturado.
pub fn parar() -> Option<Encerrada> {
    ATIVA.store(false, Ordering::Release);
    vigia::desligar();
    let c = CAPTURA.lock().take()?;
    Some(Encerrada {
        id: c.id,
        iniciada_em: c.iniciada_em,
        terminada_em: chrono::Utc::now(),
        duracao: c.inicio.elapsed(),
        coletor: c.coletor,
        contexto: c.contexto,
        origem: "manual",
    })
}

/// Os números da captura em curso — para o painel ao vivo.
pub fn resumo() -> Option<Resumo> {
    com(|c, t| c.coletor.resumo(t))
}

/// Há quanto tempo a captura corre.
pub fn duracao() -> Option<Duration> {
    com(|c, _| c.inicio.elapsed())
}

/// Completa o contexto — a GPU do motor abre depois da primeira revelação, e a
/// taxa estimada fica boa com os quadros.
pub fn atualizar_contexto(f: impl FnOnce(&mut Contexto)) {
    com(|c, _| {
        f(&mut c.contexto);
        if c.contexto.hz > 0.0 {
            c.coletor.definir_hz(c.contexto.hz);
        }
    });
}

pub fn contexto() -> Option<Contexto> {
    com(|c, _| c.contexto.clone())
}

/// O operador está fazendo `operacao`.
#[inline]
pub fn operacao(operacao: Operacao) {
    com(|c, t| c.coletor.operacao(operacao, t));
}

/// A tela aberta na janela principal.
pub fn tela(nome: &'static str) {
    com(|c, _| c.coletor.tela(nome));
}

/// Uma etapa que já foi medida por quem chama.
pub fn etapa(etapa: Etapa, duracao: Duration) {
    let ms = duracao.as_secs_f32() * 1000.0;
    com(|c, t| {
        let na_interface = std::thread::current().id() == c.thread_da_interface;
        c.coletor.etapa(etapa, ms, na_interface, t)
    });
}

/// Mede do agora até o fim do escopo. Desligada, não lê nem o relógio.
#[must_use = "a medida vai até sair do escopo: guarde em `_m`"]
pub struct Medida(Option<(Etapa, Instant)>);

#[inline]
pub fn medir(e: Etapa) -> Medida {
    Medida(ativa().then(|| (e, Instant::now())))
}

impl Drop for Medida {
    fn drop(&mut self) {
        if let Some((e, inicio)) = self.0.take() {
            etapa(e, inicio.elapsed());
        }
    }
}

/// Uma revelação do motor (thread do motor).
pub fn revelacao_do_motor(mut r: RevelacaoNoMotor, info: Option<&revelacao_core::InfoDoAdaptador>) {
    com(|c, t| {
        r.em_us = t;
        if c.contexto.motor.is_none() {
            c.contexto.motor = info.cloned();
        }
        c.coletor.revelacao(r)
    });
}

pub fn imagem(imagem: Imagem) {
    com(|c, _| c.coletor.imagem(imagem));
}

// ── O quadro ──

/// O `render` da raiz começou — o começo do quadro.
#[inline]
pub fn quadro_comecou() {
    com(|c, t| c.coletor.quadro_comecou(t));
    vigia::bater();
}

/// A batida da thread da interface (a cada [`coletor::BATIDA_US`], da tarefa
/// do painel): é o que separa a janela presa do operador parado.
pub fn batida() {
    vigia::bater();
    com(|c, t| c.coletor.batida(t));
}

/// O sentinela pintou — o fim do quadro. Devolve o número dele.
pub fn quadro_pintado() -> Option<u64> {
    vigia::bater();
    com(|c, t| c.coletor.quadro_pintado(t))
}

pub fn quadro_apresentado(seq: u64) {
    com(|c, t| c.coletor.quadro_apresentado(seq, t));
}

/// A interface parou de responder por `duracao` (visto pelo vigia).
fn travamento(inicio: Instant, duracao: Duration) {
    com(|c, _| {
        let em = inicio.saturating_duration_since(c.inicio).as_micros() as u64;
        c.coletor.travamento(em, duracao.as_secs_f32() * 1000.0)
    });
}

/// O que o vigia grava em disco quando a interface trava — tirado **da
/// thread do vigia**, com a trava tentada por pouco tempo: se a interface
/// travou segurando a trava (não deveria: ela só a segura por microssegundos),
/// o vigia desiste em vez de travar junto.
fn fotografia_para_o_vigia() -> Option<relatorio::Fotografia> {
    let guarda = CAPTURA.try_lock_for(Duration::from_millis(200))?;
    let c = guarda.as_ref()?;
    Some(relatorio::fotografar(
        &c.id,
        c.iniciada_em,
        c.inicio.elapsed(),
        &c.coletor,
        &c.contexto,
    ))
}

/// O perfil com que o binário foi compilado — medida em `debug` não vale.
pub fn perfil_de_build() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "otimizado"
    }
}

// ── A foto na tela ──

/// Os pedidos ao motor à espera da foto — para a latência ponta a ponta.
static PEDIDOS: Mutex<std::collections::VecDeque<(u64, Instant)>> =
    Mutex::new(std::collections::VecDeque::new());

/// O pedido `id` foi para o motor (thread da interface).
pub fn pedido_ao_motor(id: u64) {
    if !ativa() {
        return;
    }
    let mut p = PEDIDOS.lock();
    if p.len() >= 64 {
        p.pop_front();
    }
    p.push_back((id, Instant::now()));
}

/// O resultado do pedido `id` entrou no palco. Os anteriores a ele foram
/// largados pelo motor (só o mais novo é atendido) e saem junto.
pub fn foto_na_tela(id: u64) {
    if !ativa() {
        return;
    }
    let pedido = {
        let mut p = PEDIDOS.lock();
        let achado = p.iter().find(|(i, _)| *i == id).map(|(_, t)| *t);
        p.retain(|(i, _)| *i > id);
        achado
    };
    if let Some(quando) = pedido {
        etapa(Etapa::LatenciaDaFoto, quando.elapsed());
    }
}

/// A taxa estimada pelos quadros da captura em curso.
pub fn estimar_hz() -> Option<f32> {
    com(|c, _| c.coletor.hz_estimada()).flatten()
}

/// A sessão em curso até agora, como iria para o banco — o "Copiar relatório"
/// com a captura ainda ligada.
pub fn sessao_ate_agora() -> Option<domain::desempenho::SessaoDeDesempenho> {
    com(|c, _| {
        relatorio::montar_sessao(
            &c.id,
            c.iniciada_em,
            c.inicio.elapsed(),
            &c.coletor,
            &c.contexto,
            "manual",
            maquina::pronta().as_deref(),
        )
    })
}

/// A tela aberta e o tamanho da janela principal — do `render` dela, a cada
/// quadro com a captura ligada.
pub fn janela_principal(nome: &'static str, janela_px: (u32, u32), escala: f32) {
    com(|c, _| {
        c.coletor.tela(nome);
        c.contexto.janela_px = janela_px;
        c.contexto.escala = escala;
    });
}
