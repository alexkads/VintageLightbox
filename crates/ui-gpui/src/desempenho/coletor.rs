//! O coletor: quadros, etapas e revelações de uma captura, em memória limitada.
//!
//! Lógica pura — o tempo entra como microssegundos desde o começo da captura,
//! e é isso que deixa testar sem janela nem relógio. Quem o alimenta e o
//! protege com trava é o [módulo](super).
//!
//! # A memória tem teto
//!
//! 🔑 **Uma captura de uma hora ocupa o mesmo que uma de um minuto.** Os quadros
//! individuais ficam num anel ([`CAPACIDADE_DE_QUADROS`]); os lentos, noutro
//! ([`CAPACIDADE_DE_LENTOS`]), com os piores guardados à parte para nunca
//! sumirem; as revelações do motor, noutro. O que cresce sem teto são só as
//! **contas** — as distribuições por operação × etapa, de tamanho fixo por
//! par —, e é delas que saem mediana, p95 e pior da sessão inteira, mesmo
//! depois de o anel ter dado a volta.

use std::collections::{HashMap, VecDeque};

use super::{Etapa, Operacao, ETAPAS_DA_INTERFACE};

/// Quadros individuais guardados (≈ 5 min a 120 Hz de interação contínua).
pub const CAPACIDADE_DE_QUADROS: usize = 36_000;
/// Quadros lentos guardados, os mais recentes.
pub const CAPACIDADE_DE_LENTOS: usize = 3_000;
/// Os piores de todos, que nunca saem.
pub const PIORES_GUARDADOS: usize = 50;
/// Revelações do motor guardadas.
pub const CAPACIDADE_DE_REVELACOES: usize = 5_000;
pub const CAPACIDADE_DE_TRAVAMENTOS: usize = 200;
pub const CAPACIDADE_DE_IMAGENS: usize = 50;
/// O teto de memória que os testes prendem.
pub const TETO_DE_MEMORIA: usize = 8 * 1024 * 1024;

/// Um intervalo só conta como "quadro de interação" se o anterior aconteceu
/// dentro da mesma operação e há menos que isto: uma janela parada não produz
/// quadros, e o intervalo que atravessa o repouso não é engasgo.
const INTERVALO_MAXIMO_US: u64 = 1_000_000;

/// A batida da thread da interface durante a captura (ver [`Coletor::batida`]).
pub const BATIDA_US: u64 = 16_000;

// ── A distribuição ─────────────────────────────────────────────────────────

/// Baldes de 0,1 ms até 50 ms, de 2 ms até 1 s, de 100 ms até 60 s.
const BALDES_FINOS: usize = 500;
const BALDES_MEDIOS: usize = 475;
const BALDES_GROSSOS: usize = 590;
const BALDES: usize = BALDES_FINOS + BALDES_MEDIOS + BALDES_GROSSOS;

fn balde(ms: f32) -> usize {
    let ms = ms.max(0.0);
    if ms < 50.0 {
        (ms * 10.0) as usize
    } else if ms < 1000.0 {
        BALDES_FINOS + ((ms - 50.0) / 2.0) as usize
    } else {
        (BALDES_FINOS + BALDES_MEDIOS + ((ms - 1000.0) / 100.0) as usize).min(BALDES - 1)
    }
}

fn meio_do_balde(b: usize) -> f32 {
    if b < BALDES_FINOS {
        b as f32 / 10.0 + 0.05
    } else if b < BALDES_FINOS + BALDES_MEDIOS {
        50.0 + (b - BALDES_FINOS) as f32 * 2.0 + 1.0
    } else {
        1000.0 + (b - BALDES_FINOS - BALDES_MEDIOS) as f32 * 100.0 + 50.0
    }
}

/// Um histograma de durações: percentil com erro de meio balde (0,05 ms até
/// 50 ms), soma e pior exatos. Tamanho fixo (~6 KB), qualquer que seja o
/// número de amostras.
#[derive(Clone, Debug)]
pub struct Distribuicao {
    baldes: Box<[u32; BALDES]>,
    pub amostras: u64,
    pub soma_ms: f64,
    pub pior_ms: f32,
}

impl Default for Distribuicao {
    fn default() -> Self {
        Self {
            baldes: Box::new([0; BALDES]),
            amostras: 0,
            soma_ms: 0.0,
            pior_ms: 0.0,
        }
    }
}

impl Distribuicao {
    pub fn anotar(&mut self, ms: f32) {
        if !ms.is_finite() {
            return;
        }
        let b = balde(ms);
        self.baldes[b] = self.baldes[b].saturating_add(1);
        self.amostras += 1;
        self.soma_ms += f64::from(ms);
        self.pior_ms = self.pior_ms.max(ms);
    }

    /// `p` de 0 a 1. Zero sem amostras.
    pub fn percentil(&self, p: f32) -> f32 {
        if self.amostras == 0 {
            return 0.0;
        }
        let alvo = ((p.clamp(0.0, 1.0) as f64) * self.amostras as f64)
            .ceil()
            .max(1.0) as u64;
        let mut acumulado = 0u64;
        for (b, &n) in self.baldes.iter().enumerate() {
            acumulado += u64::from(n);
            if acumulado >= alvo {
                return meio_do_balde(b).min(self.pior_ms);
            }
        }
        self.pior_ms
    }

    pub fn media_ms(&self) -> f32 {
        if self.amostras == 0 {
            0.0
        } else {
            (self.soma_ms / self.amostras as f64) as f32
        }
    }

    pub fn resumo(&self) -> Estatistica {
        Estatistica {
            amostras: self.amostras,
            mediana_ms: self.percentil(0.5),
            p95_ms: self.percentil(0.95),
            pior_ms: self.pior_ms,
            media_ms: self.media_ms(),
            soma_ms: self.soma_ms,
        }
    }
}

/// O que uma distribuição diz, em números.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Estatistica {
    pub amostras: u64,
    pub mediana_ms: f32,
    pub p95_ms: f32,
    pub pior_ms: f32,
    pub media_ms: f32,
    pub soma_ms: f64,
}

// ── Os registros ───────────────────────────────────────────────────────────

/// Um quadro concluído pela janela.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quadro {
    pub seq: u64,
    /// Fim da pintura, desde o começo da captura.
    pub em_us: u64,
    /// Desde a pintura anterior. Zero se o anterior foi antes da operação ou
    /// há mais de um segundo (repouso, não engasgo) — ver [`Quadro::conta`].
    pub intervalo_us: u32,
    /// `render` da raiz → fim da pintura.
    pub montagem_us: u32,
    /// Fim da pintura → primeira tarefa depois do `present` (0 = não veio).
    pub apresentacao_us: u32,
    pub operacao: Operacao,
    pub tela: u8,
    /// O intervalo entra na conta do FPS de interação.
    pub conta: bool,
    pub lento: bool,
    /// O que a thread da interface fez entre o quadro anterior e este, por
    /// etapa ([`ETAPAS_DA_INTERFACE`]), em centésimos de milissegundo.
    pub etapas: [u16; ETAPAS_DA_INTERFACE.len()],
}

impl Quadro {
    pub fn intervalo_ms(&self) -> f32 {
        self.intervalo_us as f32 / 1000.0
    }
    /// O tempo total observado do quadro: `render` da raiz → depois do
    /// `present`. Sem a apresentação, só a montagem.
    pub fn duracao_ms(&self) -> f32 {
        (self.montagem_us + self.apresentacao_us) as f32 / 1000.0
    }
    pub fn etapa_ms(&self, i: usize) -> f32 {
        f32::from(self.etapas[i]) / 100.0
    }
}

/// Uma revelação do motor, etapa por etapa (thread do motor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RevelacaoNoMotor {
    pub em_us: u64,
    pub operacao: Operacao,
    pub largura: u32,
    pub altura: u32,
    pub rascunho: bool,
    pub reducao_ms: f32,
    pub tempos: revelacao_core::TemposDoMotor,
    pub total_ms: f32,
}

/// A interface parou de responder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Travamento {
    pub em_us: u64,
    pub duracao_ms: f32,
    pub operacao: Operacao,
}

/// As características técnicas de uma foto revelada — nunca o conteúdo.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize)]
pub struct Imagem {
    pub largura: u32,
    pub altura: u32,
    pub formato: String,
    pub mascaras: u32,
    pub retoques: u32,
}

// ── A operação em curso ────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
struct EmCurso {
    operacao: Operacao,
    desde_us: u64,
    ate_us: u64,
}

// ── O coletor ──────────────────────────────────────────────────────────────

pub struct Coletor {
    periodo_ms: f32,
    em_curso: Option<EmCurso>,
    /// O primeiro sinal de operação depois do último quadro, e a última batida
    /// da thread antes dele — ver [`Coletor::quadro_pintado`].
    primeiro_sinal: Option<(u64, Option<u64>)>,
    ultima_batida_us: Option<u64>,
    inicio_do_quadro_us: Option<u64>,
    ultima_pintura_us: Option<u64>,
    pendentes: [f32; ETAPAS_DA_INTERFACE.len()],
    seq: u64,
    quadros: VecDeque<Quadro>,
    lentos: VecDeque<Quadro>,
    piores: Vec<Quadro>,
    revelacoes: VecDeque<RevelacaoNoMotor>,
    travamentos: Vec<Travamento>,
    imagens: Vec<Imagem>,
    pub telas: Vec<&'static str>,
    tela: u8,
    /// As contas da sessão inteira.
    distribuicoes: HashMap<(Operacao, Etapa), Distribuicao>,
    intervalos: Distribuicao,
    quadros_total: u64,
    acima_do_orcamento: u64,
    acima_por_operacao: HashMap<Operacao, u64>,
    /// Por operação: (quadros de interação, tempo somado dos intervalos).
    ritmo: HashMap<Operacao, (u64, u64)>,
    /// A GPU mediu alguma coisa nesta captura.
    pub gpu_medida: bool,
}

impl Coletor {
    /// `hz`: a taxa do monitor, que dá o orçamento de um quadro.
    pub fn novo(hz: f32) -> Self {
        Self {
            periodo_ms: 1000.0 / hz.clamp(20.0, 500.0),
            em_curso: None,
            primeiro_sinal: None,
            ultima_batida_us: None,
            inicio_do_quadro_us: None,
            ultima_pintura_us: None,
            pendentes: [0.0; ETAPAS_DA_INTERFACE.len()],
            seq: 0,
            // 🔑 Reservados de uma vez, com o tamanho exato: um `VecDeque` que
            // cresce dobrando passaria do teto no último dobro.
            quadros: VecDeque::with_capacity(CAPACIDADE_DE_QUADROS),
            lentos: VecDeque::with_capacity(CAPACIDADE_DE_LENTOS),
            piores: Vec::new(),
            revelacoes: VecDeque::with_capacity(CAPACIDADE_DE_REVELACOES),
            travamentos: Vec::new(),
            imagens: Vec::new(),
            telas: Vec::new(),
            tela: 0,
            distribuicoes: HashMap::new(),
            intervalos: Distribuicao::default(),
            quadros_total: 0,
            acima_do_orcamento: 0,
            acima_por_operacao: HashMap::new(),
            ritmo: HashMap::new(),
            gpu_medida: false,
        }
    }

    pub fn orcamento_ms(&self) -> f32 {
        self.periodo_ms
    }

    /// Troca a taxa do monitor (a estimada fica mais certa com os quadros).
    pub fn definir_hz(&mut self, hz: f32) {
        self.periodo_ms = 1000.0 / hz.clamp(20.0, 500.0);
    }

    /// Acima disto, o intervalo perdeu pelo menos uma batida do monitor.
    pub fn limite_de_lento_ms(&self) -> f32 {
        self.periodo_ms * 1.5
    }

    fn anotar(&mut self, operacao: Operacao, etapa: Etapa, ms: f32) {
        self.distribuicoes
            .entry((operacao, etapa))
            .or_default()
            .anotar(ms);
    }

    // ── A operação ──

    pub fn operacao_em(&self, t: u64) -> Operacao {
        match self.em_curso {
            Some(e) if t <= e.ate_us => e.operacao,
            _ => Operacao::Nenhuma,
        }
    }

    fn desde_em(&self, t: u64) -> Option<u64> {
        self.em_curso.filter(|e| t <= e.ate_us).map(|e| e.desde_us)
    }

    /// O operador está fazendo `operacao` agora. Repetir estica a janela.
    /// A thread da interface estava livre em `t` (uma tarefa dela rodou).
    pub fn batida(&mut self, t: u64) {
        self.ultima_batida_us = Some(t);
    }

    pub fn operacao(&mut self, operacao: Operacao, t: u64) {
        if self.primeiro_sinal.is_none() {
            self.primeiro_sinal = Some((t, self.ultima_batida_us));
        }
        let fica = operacao.duracao_us();
        match &mut self.em_curso {
            Some(e) if e.operacao == operacao && t <= e.ate_us => e.ate_us = t + fica,
            // 🔑 A troca de foto que a abertura da Revelação provoca é parte
            // da abertura, e não uma operação nova.
            Some(e)
                if e.operacao == Operacao::AberturaDaRevelacao
                    && operacao == Operacao::TrocaDeFoto
                    && t <= e.ate_us => {}
            _ => {
                self.em_curso = Some(EmCurso {
                    operacao,
                    desde_us: t,
                    ate_us: t + fica,
                })
            }
        }
    }

    pub fn tela(&mut self, nome: &'static str) {
        let i = match self.telas.iter().position(|t| *t == nome) {
            Some(i) => i,
            None => {
                self.telas.push(nome);
                self.telas.len() - 1
            }
        };
        self.tela = i.min(u8::MAX as usize) as u8;
    }

    // ── O quadro ──

    pub fn quadro_comecou(&mut self, t: u64) {
        self.inicio_do_quadro_us = Some(t);
    }

    /// O fim da pintura. Devolve o número do quadro, para a apresentação.
    pub fn quadro_pintado(&mut self, t: u64) -> u64 {
        let operacao = self.operacao_em(t);
        let desde = self.desde_em(t);
        let intervalo = self.ultima_pintura_us.map(|u| t.saturating_sub(u));
        // 🔑 **Quando o intervalo é engasgo.** Só nos gestos contínuos (rolar,
        // arrastar, pintar): a troca de foto e a abertura esperam disco e GPU
        // com a janela parada, e são julgadas pelo tempo do próprio quadro e
        // pela latência até a foto. E só se o intervalo teve demanda — um
        // sinal do gesto desde o quadro anterior.
        //
        // ⚠️ O sinal é anotado quando a thread **processa** o evento, não quando
        // o sistema o gerou. Um gesto que pausa (o mouse parado com o botão
        // apertado) e volta pareceria um engasgo do tamanho da pausa. O que
        // separa os dois é a batida: se a thread bateu logo antes de processar
        // o sinal (e depois do quadro anterior), ela estava livre — era o
        // operador parado, não a janela presa.
        let sinal = self.primeiro_sinal.take();
        let ocioso_antes_do_sinal = match (sinal, self.ultima_pintura_us) {
            (Some((s, Some(b))), Some(u)) => {
                b > u && s.saturating_sub(b) <= 2 * BATIDA_US && s.saturating_sub(u) > 2 * BATIDA_US
            }
            _ => false,
        };
        if let Some((s, _)) = sinal {
            if operacao != Operacao::Nenhuma {
                self.anotar(
                    operacao,
                    Etapa::EntradaAoQuadro,
                    t.saturating_sub(s) as f32 / 1000.0,
                );
            }
        }
        let conta = operacao.continua()
            && sinal.is_some()
            && !ocioso_antes_do_sinal
            && matches!((intervalo, self.ultima_pintura_us, desde),
                (Some(i), Some(u), Some(d)) if i < INTERVALO_MAXIMO_US && u >= d);
        let montagem = self
            .inicio_do_quadro_us
            .take()
            .filter(|&i| i <= t && self.ultima_pintura_us.is_none_or(|u| i >= u))
            .map_or(0, |i| t - i);
        self.ultima_pintura_us = Some(t);
        self.seq += 1;
        self.quadros_total += 1;

        let mut etapas = [0u16; ETAPAS_DA_INTERFACE.len()];
        for (i, ms) in self.pendentes.iter_mut().enumerate() {
            etapas[i] = (*ms * 100.0).round().min(u16::MAX as f32) as u16;
            *ms = 0.0;
        }
        let intervalo_us = if conta {
            intervalo.unwrap_or(0).min(u64::from(u32::MAX)) as u32
        } else {
            0
        };
        let mut q = Quadro {
            seq: self.seq,
            em_us: t,
            intervalo_us,
            montagem_us: montagem.min(u64::from(u32::MAX)) as u32,
            apresentacao_us: 0,
            operacao,
            tela: self.tela,
            conta,
            lento: false,
            etapas,
        };
        if conta {
            let ms = q.intervalo_ms();
            self.intervalos.anotar(ms);
            self.anotar(operacao, Etapa::IntervaloDoQuadro, ms);
            let r = self.ritmo.entry(operacao).or_default();
            r.0 += 1;
            r.1 += u64::from(intervalo_us);
            if ms > self.limite_de_lento_ms() {
                q.lento = true;
                self.acima_do_orcamento += 1;
                *self.acima_por_operacao.entry(operacao).or_default() += 1;
            }
        }
        if operacao != Operacao::Nenhuma {
            self.anotar(
                operacao,
                Etapa::MontagemDaInterface,
                q.montagem_us as f32 / 1000.0,
            );
        }
        if self.quadros.len() == CAPACIDADE_DE_QUADROS {
            self.quadros.pop_front();
        }
        self.quadros.push_back(q);
        if q.lento {
            self.guardar_lento(q);
        }
        self.seq
    }

    /// A primeira tarefa da thread depois do `present` do quadro `seq`.
    pub fn quadro_apresentado(&mut self, seq: u64, t: u64) {
        let orcamento = self.periodo_ms;
        let Some(q) = self.quadros.iter_mut().rev().take(8).find(|q| q.seq == seq) else {
            return;
        };
        q.apresentacao_us = t.saturating_sub(q.em_us).min(u64::from(u32::MAX)) as u32;
        let q = *q;
        if q.operacao != Operacao::Nenhuma {
            self.anotar(
                q.operacao,
                Etapa::Apresentacao,
                q.apresentacao_us as f32 / 1000.0,
            );
            self.anotar(q.operacao, Etapa::TempoDoQuadro, q.duracao_ms());
        }
        // Um quadro que sozinho passou do orçamento também é lento, mesmo que
        // o intervalo não tenha contado (o primeiro de um gesto, por exemplo).
        if !q.lento && q.operacao != Operacao::Nenhuma && q.duracao_ms() > orcamento {
            if let Some(g) = self.quadros.iter_mut().rev().take(8).find(|x| x.seq == seq) {
                g.lento = true;
            }
            let mut q = q;
            q.lento = true;
            self.guardar_lento(q);
        } else if q.lento {
            if let Some(g) = self.lentos.iter_mut().rev().find(|x| x.seq == seq) {
                g.apresentacao_us = q.apresentacao_us;
            }
            if let Some(g) = self.piores.iter_mut().find(|x| x.seq == seq) {
                g.apresentacao_us = q.apresentacao_us;
            }
        }
    }

    fn guardar_lento(&mut self, q: Quadro) {
        if self.lentos.len() == CAPACIDADE_DE_LENTOS {
            self.lentos.pop_front();
        }
        self.lentos.push_back(q);
        let chave = |q: &Quadro| q.intervalo_us.max(q.montagem_us + q.apresentacao_us);
        if self.piores.len() < PIORES_GUARDADOS {
            self.piores.push(q);
        } else if let Some((i, menor)) = self
            .piores
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| chave(p))
            .map(|(i, p)| (i, chave(p)))
        {
            if chave(&q) > menor {
                self.piores[i] = q;
            }
        }
    }

    // ── As etapas ──

    /// Uma etapa medida. `na_interface`: rodou na thread da interface, e entra
    /// no quadro seguinte como "o que a thread fez entre dois quadros".
    pub fn etapa(&mut self, etapa: Etapa, ms: f32, na_interface: bool, t: u64) {
        let operacao = self.operacao_em(t);
        self.anotar(operacao, etapa, ms);
        if etapa.onde() == super::Onde::Gpu {
            self.gpu_medida = true;
        }
        if na_interface {
            if let Some(i) = etapa.indice_na_interface() {
                self.pendentes[i] += ms;
            }
        }
    }

    pub fn revelacao(&mut self, mut r: RevelacaoNoMotor) {
        r.operacao = self.operacao_em(r.em_us);
        let t = r.em_us;
        let tempos = r.tempos;
        for (etapa, ms) in [
            (Etapa::ReducaoNoMotor, Some(r.reducao_ms)),
            (Etapa::PreparoNoMotor, Some(tempos.preparo_ms)),
            (Etapa::GravacaoNoMotor, Some(tempos.gravacao_ms)),
            (Etapa::EsperaPelaGpu, Some(tempos.espera_ms)),
            (Etapa::LeituraDaGpu, Some(tempos.leitura_ms)),
            (Etapa::MotorTotal, Some(r.total_ms)),
            (Etapa::GpuMascaras, tempos.gpu.and_then(|g| g.mascaras_ms)),
            (Etapa::GpuRetoques, tempos.gpu.and_then(|g| g.retoques_ms)),
            (Etapa::GpuRevelacao, tempos.gpu.and_then(|g| g.revelacao_ms)),
            (Etapa::GpuMotorTotal, tempos.gpu.and_then(|g| g.total_ms)),
        ] {
            if let Some(ms) = ms {
                self.etapa(etapa, ms, false, t);
            }
        }
        if self.revelacoes.len() == CAPACIDADE_DE_REVELACOES {
            self.revelacoes.pop_front();
        }
        self.revelacoes.push_back(r);
    }

    pub fn travamento(&mut self, em_us: u64, duracao_ms: f32) {
        if self.travamentos.len() < CAPACIDADE_DE_TRAVAMENTOS {
            let operacao = self.operacao_em(em_us);
            self.travamentos.push(Travamento {
                em_us,
                duracao_ms,
                operacao,
            });
        }
    }

    pub fn imagem(&mut self, imagem: Imagem) {
        if self.imagens.len() < CAPACIDADE_DE_IMAGENS && !self.imagens.contains(&imagem) {
            self.imagens.push(imagem);
        }
    }

    // ── A leitura ──

    pub fn quadros(&self) -> &VecDeque<Quadro> {
        &self.quadros
    }
    pub fn lentos(&self) -> &VecDeque<Quadro> {
        &self.lentos
    }
    pub fn piores(&self) -> &[Quadro] {
        &self.piores
    }
    pub fn revelacoes(&self) -> &VecDeque<RevelacaoNoMotor> {
        &self.revelacoes
    }
    pub fn travamentos(&self) -> &[Travamento] {
        &self.travamentos
    }
    pub fn imagens(&self) -> &[Imagem] {
        &self.imagens
    }
    pub fn distribuicoes(&self) -> &HashMap<(Operacao, Etapa), Distribuicao> {
        &self.distribuicoes
    }

    /// A taxa do monitor estimada pelos quadros: os intervalos mais curtos de
    /// uma interação contínua são uma batida do monitor. Só com amostra
    /// suficiente, e encostada numa taxa comum quando está perto de uma.
    pub fn hz_estimada(&self) -> Option<f32> {
        if self.intervalos.amostras < 120 {
            return None;
        }
        let batida = self.intervalos.percentil(0.2);
        if batida < 2.0 {
            return None;
        }
        let hz = 1000.0 / batida;
        const COMUNS: [f32; 15] = [
            30.0, 48.0, 50.0, 60.0, 72.0, 75.0, 90.0, 100.0, 120.0, 144.0, 165.0, 170.0, 180.0,
            200.0, 240.0,
        ];
        Some(
            COMUNS
                .into_iter()
                .find(|c| (hz - c).abs() / c < 0.08)
                .unwrap_or(hz),
        )
    }

    /// O que a captura ocupa, em bytes — o que os testes prendem abaixo de
    /// [`TETO_DE_MEMORIA`].
    pub fn memoria_estimada(&self) -> usize {
        use std::mem::size_of;
        self.quadros.capacity() * size_of::<Quadro>()
            + self.lentos.capacity() * size_of::<Quadro>()
            + self.piores.capacity() * size_of::<Quadro>()
            + self.revelacoes.capacity() * size_of::<RevelacaoNoMotor>()
            + self.travamentos.capacity() * size_of::<Travamento>()
            + (self.distribuicoes.len() + 1) * (BALDES * 4 + size_of::<Distribuicao>())
            + self.imagens.capacity() * (size_of::<Imagem>() + 16)
    }

    /// Os números da captura até `t`.
    pub fn resumo(&self, t: u64) -> Resumo {
        let mut operacoes: Vec<Operacao> = self
            .distribuicoes
            .keys()
            .map(|(o, _)| *o)
            .filter(|o| *o != Operacao::Nenhuma)
            .collect();
        operacoes.sort();
        operacoes.dedup();
        let por_operacao = operacoes
            .into_iter()
            .map(|operacao| {
                let (quadros, soma_us) = self.ritmo.get(&operacao).copied().unwrap_or_default();
                let intervalo = self
                    .distribuicoes
                    .get(&(operacao, Etapa::IntervaloDoQuadro))
                    .map(Distribuicao::resumo)
                    .unwrap_or_default();
                let mut etapas: Vec<(Etapa, Estatistica)> = self
                    .distribuicoes
                    .iter()
                    .filter(|((o, e), _)| *o == operacao && *e != Etapa::IntervaloDoQuadro)
                    .map(|((_, e), d)| (*e, d.resumo()))
                    .collect();
                etapas.sort_by_key(|(e, _)| *e);
                let gargalo = gargalo(&etapas);
                ResumoDaOperacao {
                    operacao,
                    quadros,
                    fps: fps(quadros, soma_us),
                    intervalo,
                    acima_do_orcamento: self
                        .acima_por_operacao
                        .get(&operacao)
                        .copied()
                        .unwrap_or(0),
                    etapas,
                    gargalo,
                }
            })
            .collect();
        let (quadros_interacao, soma_us) = self
            .ritmo
            .values()
            .fold((0, 0), |(a, b), (q, s)| (a + q, b + s));
        let geral = self.intervalos.resumo();
        Resumo {
            duracao_ms: t as f64 / 1000.0,
            orcamento_ms: self.periodo_ms,
            limite_de_lento_ms: self.limite_de_lento_ms(),
            quadros_total: self.quadros_total,
            quadros_interacao,
            fps_interacao: fps(quadros_interacao, soma_us),
            mediana_ms: geral.mediana_ms,
            p95_ms: geral.p95_ms,
            pior_ms: geral.pior_ms,
            acima_do_orcamento: self.acima_do_orcamento,
            operacao_atual: self.operacao_em(t),
            por_operacao,
            lentos_recentes: self.lentos.iter().rev().take(240).rev().copied().collect(),
            travamentos: self.travamentos.clone(),
            gpu_medida: self.gpu_medida,
            memoria_bytes: self.memoria_estimada(),
            telas: self.telas.clone(),
        }
    }
}

fn fps(quadros: u64, soma_us: u64) -> f32 {
    if quadros == 0 || soma_us == 0 {
        0.0
    } else {
        (quadros as f64 * 1e6 / soma_us as f64) as f32
    }
}

/// A etapa de maior p95 entre as que são **parte** do trabalho — não o
/// tempo total do quadro nem os totais que já contêm as outras.
pub fn gargalo(etapas: &[(Etapa, Estatistica)]) -> Option<(Etapa, Estatistica)> {
    etapas
        .iter()
        .filter(|(e, s)| e.candidata_a_gargalo() && s.amostras > 0)
        .max_by(|a, b| a.1.p95_ms.total_cmp(&b.1.p95_ms))
        .copied()
}

#[derive(Clone, Debug)]
pub struct ResumoDaOperacao {
    pub operacao: Operacao,
    pub quadros: u64,
    pub fps: f32,
    pub intervalo: Estatistica,
    pub acima_do_orcamento: u64,
    pub etapas: Vec<(Etapa, Estatistica)>,
    pub gargalo: Option<(Etapa, Estatistica)>,
}

#[derive(Clone, Debug)]
pub struct Resumo {
    pub duracao_ms: f64,
    pub orcamento_ms: f32,
    pub limite_de_lento_ms: f32,
    pub quadros_total: u64,
    pub quadros_interacao: u64,
    pub fps_interacao: f32,
    pub mediana_ms: f32,
    pub p95_ms: f32,
    pub pior_ms: f32,
    pub acima_do_orcamento: u64,
    pub operacao_atual: Operacao,
    pub por_operacao: Vec<ResumoDaOperacao>,
    pub lentos_recentes: Vec<Quadro>,
    pub travamentos: Vec<Travamento>,
    pub gpu_medida: bool,
    pub memoria_bytes: usize,
    pub telas: Vec<&'static str>,
}

#[cfg(test)]
mod testes {
    use super::*;

    const MS: u64 = 1000;

    #[test]
    fn o_percentil_erra_menos_que_meio_balde() {
        let mut d = Distribuicao::default();
        for i in 1..=1000 {
            d.anotar(i as f32 / 20.0); // 0,05 … 50 ms
        }
        assert!(
            (d.percentil(0.5) - 25.0).abs() <= 0.1,
            "{}",
            d.percentil(0.5)
        );
        assert!(
            (d.percentil(0.95) - 47.5).abs() <= 0.1,
            "{}",
            d.percentil(0.95)
        );
        assert_eq!(d.pior_ms, 50.0);
        assert_eq!(d.amostras, 1000);
        d.anotar(5_000.0);
        assert_eq!(d.pior_ms, 5_000.0);
        assert!(d.percentil(1.0) <= 5_000.0);
    }

    /// O arrasto a 60 Hz com um engasgo: o FPS vem dos intervalos entre
    /// pinturas, o engasgo é lento, e o repouso antes do gesto não conta.
    #[test]
    fn so_o_intervalo_dentro_da_operacao_conta() {
        let mut c = Coletor::new_60();
        // Um quadro solto, sem operação, e cinco segundos parados.
        c.quadro_comecou(0);
        c.quadro_pintado(2 * MS);
        c.operacao(Operacao::ArrastoDeSlider, 5_000 * MS);
        let mut t = 5_000 * MS;
        for i in 0..60 {
            t += if i == 30 { 50 * MS } else { 16_667 };
            c.operacao(Operacao::ArrastoDeSlider, t - 1);
            c.quadro_comecou(t - 4 * MS);
            let seq = c.quadro_pintado(t);
            c.quadro_apresentado(seq, t + MS);
        }
        let r = c.resumo(t);
        let arrasto = &r.por_operacao[0];
        assert_eq!(arrasto.operacao, Operacao::ArrastoDeSlider);
        // O primeiro do gesto não conta (o anterior foi antes do gesto).
        assert_eq!(arrasto.quadros, 59);
        assert_eq!(r.acima_do_orcamento, 1, "só o engasgo de 50 ms");
        assert!((arrasto.intervalo.mediana_ms - 16.7).abs() < 0.1);
        assert!(arrasto.intervalo.pior_ms >= 49.9);
        assert!(arrasto.fps > 50.0 && arrasto.fps < 60.0, "{}", arrasto.fps);
        assert_eq!(r.lentos_recentes.len(), 1);
        let montagem = arrasto
            .etapas
            .iter()
            .find(|(e, _)| *e == Etapa::MontagemDaInterface)
            .unwrap()
            .1;
        assert!((montagem.mediana_ms - 4.0).abs() < 0.1);
        let total = arrasto
            .etapas
            .iter()
            .find(|(e, _)| *e == Etapa::TempoDoQuadro)
            .unwrap()
            .1;
        assert!(
            (total.mediana_ms - 5.0).abs() < 0.1,
            "montagem + apresentação"
        );
    }

    #[test]
    fn a_etapa_da_interface_entra_no_quadro_seguinte_e_da_o_gargalo() {
        let mut c = Coletor::new_60();
        c.operacao(Operacao::ArrastoDeSlider, 0);
        c.quadro_pintado(MS);
        c.operacao(Operacao::ArrastoDeSlider, 4 * MS);
        c.etapa(Etapa::Histograma, 3.0, true, 5 * MS);
        c.etapa(Etapa::ConversaoParaExibicao, 22.0, true, 6 * MS);
        c.etapa(Etapa::EsperaPelaGpu, 9.0, false, 6 * MS);
        c.quadro_pintado(30 * MS);
        let q = c.quadros().back().copied().unwrap();
        let i = Etapa::ConversaoParaExibicao.indice_na_interface().unwrap();
        assert_eq!(q.etapa_ms(i), 22.0);
        assert!(q.lento);
        let r = c.resumo(30 * MS);
        let (etapa, s) = r.por_operacao[0].gargalo.unwrap();
        assert_eq!(etapa, Etapa::ConversaoParaExibicao);
        assert!((s.p95_ms - 22.0).abs() < 0.1);
        // O quadro seguinte começa zerado.
        c.quadro_pintado(40 * MS);
        assert_eq!(
            c.quadros().back().unwrap().etapas,
            [0; ETAPAS_DA_INTERFACE.len()]
        );
    }

    #[test]
    fn a_abertura_engole_a_troca_de_foto_que_ela_mesma_provoca() {
        let mut c = Coletor::new_60();
        c.operacao(Operacao::AberturaDaRevelacao, 0);
        c.operacao(Operacao::TrocaDeFoto, 10 * MS);
        assert_eq!(c.operacao_em(20 * MS), Operacao::AberturaDaRevelacao);
        c.operacao(Operacao::ArrastoDeSlider, 30 * MS);
        assert_eq!(c.operacao_em(31 * MS), Operacao::ArrastoDeSlider);
        assert_eq!(c.operacao_em(10_000 * MS), Operacao::Nenhuma, "expirou");
    }

    /// 🔑 Uma captura enorme fica abaixo do teto de memória, e as contas
    /// continuam valendo para a sessão inteira.
    #[test]
    fn a_captura_enorme_respeita_o_teto_de_memoria() {
        let mut c = Coletor::new_60();
        let mut t = 0;
        let operacoes = Operacao::TODAS;
        for i in 0..400_000u64 {
            t += if i % 97 == 0 { 40 * MS } else { 16_667 };
            let op = operacoes[(i / 5_000) as usize % operacoes.len()];
            c.operacao(op, t - 1);
            c.etapa(Etapa::Histograma, 1.0, true, t - 1);
            let seq = c.quadro_pintado(t);
            c.quadro_apresentado(seq, t + 500);
            if i % 3 == 0 {
                c.revelacao(RevelacaoNoMotor {
                    em_us: t,
                    operacao: Operacao::Nenhuma,
                    largura: 1920,
                    altura: 1280,
                    rascunho: true,
                    reducao_ms: 1.0,
                    tempos: Default::default(),
                    total_ms: 5.0,
                });
            }
        }
        assert_eq!(c.quadros().len(), CAPACIDADE_DE_QUADROS);
        assert_eq!(c.lentos().len(), CAPACIDADE_DE_LENTOS);
        assert!(
            c.memoria_estimada() <= TETO_DE_MEMORIA,
            "{} bytes",
            c.memoria_estimada()
        );
        let r = c.resumo(t);
        // Oito das dez operações são contínuas: ~320 mil intervalos contam.
        assert!(r.quadros_interacao > 300_000, "as contas cobrem tudo");
        assert!(r.acima_do_orcamento > 3_000);
    }

    /// O operador parou o mouse no meio do arrasto: a thread bateu antes de o
    /// gesto voltar, então o intervalo longo é pausa, não engasgo. Sem batida
    /// (a thread presa), o mesmo intervalo é engasgo.
    #[test]
    fn a_pausa_do_operador_nao_e_engasgo_e_a_thread_presa_e() {
        let mut c = Coletor::novo(60.0);
        c.operacao(Operacao::ArrastoDeSlider, 0);
        c.quadro_pintado(MS);
        c.operacao(Operacao::ArrastoDeSlider, 2 * MS);
        c.quadro_pintado(18 * MS);
        // Pausa de 300 ms com a thread livre, batendo.
        for b in (20..320).step_by(16) {
            c.batida(b * MS);
        }
        c.operacao(Operacao::ArrastoDeSlider, 322 * MS);
        c.quadro_pintado(330 * MS);
        assert_eq!(c.resumo(330 * MS).acima_do_orcamento, 0, "pausa");
        // Agora a thread presa 80 ms: nenhuma batida entre o quadro e o gesto.
        c.operacao(Operacao::ArrastoDeSlider, 410 * MS);
        c.quadro_pintado(415 * MS);
        let r = c.resumo(415 * MS);
        assert_eq!(r.acima_do_orcamento, 1, "presa");
        let entrada = r.por_operacao[0]
            .etapas
            .iter()
            .find(|(e, _)| *e == Etapa::EntradaAoQuadro)
            .unwrap()
            .1;
        assert_eq!(entrada.amostras, 4);
    }

    /// Na troca de foto a janela espera o disco parada: o intervalo não é
    /// engasgo, mas o quadro que passa do orçamento sozinho é.
    #[test]
    fn a_troca_de_foto_e_julgada_pelo_proprio_quadro() {
        let mut c = Coletor::novo(60.0);
        c.operacao(Operacao::TrocaDeFoto, 0);
        c.quadro_pintado(MS);
        c.quadro_comecou(300 * MS);
        let s = c.quadro_pintado(330 * MS);
        c.quadro_apresentado(s, 331 * MS);
        let r = c.resumo(400 * MS);
        assert_eq!(r.quadros_interacao, 0);
        assert_eq!(r.lentos_recentes.len(), 1, "31 ms de quadro");
    }

    #[test]
    fn a_taxa_estimada_encosta_na_comum() {
        let mut c = Coletor::novo(60.0);
        c.operacao(Operacao::Rolagem, 0);
        let mut t = 0;
        for _ in 0..200 {
            t += 8_400; // ~119 Hz
            c.operacao(Operacao::Rolagem, t - 1);
            c.quadro_pintado(t);
        }
        assert_eq!(c.hz_estimada(), Some(120.0));
    }

    impl Coletor {
        fn new_60() -> Self {
            Coletor::novo(60.0)
        }
    }
}
