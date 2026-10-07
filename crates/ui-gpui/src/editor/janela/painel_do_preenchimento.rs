//! 🪄 O Preenchimento sensível ao conteúdo na janela do editor: o painel, a
//! sobreposição no palco e a orquestração do cálculo.
//!
//! O documento **não muda** enquanto o painel está aberto: a prévia é uma
//! imagem por cima do palco. Aplicar cria a camada de retoque num passo só
//! do desfazer, e só se o documento ainda for o do instantâneo. Fechar a
//! janela (ou cancelar) derruba o cálculo em curso.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::component::progress::Progress;
use ia_local::execucao::Backend;
use ia_local::modelos::{self, Estado as EstadoDoModelo};
use preenchimento::{lama, Controle, Erro, Metodo, Motor, Progresso};

use editor_core::Selecao;
use gpui_kit::App;

use super::*;
use crate::editor::preenchimento::{self as calculo, Resultado};

/// O lado maior das imagens da Visualização (a janela do meio).
const LADO_DA_VISUALIZACAO: u32 = 1400;

/// O que o pincel pinta enquanto o painel está aberto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlvoDoPincel {
    /// De onde os pedaços podem vir (só para quem amostra).
    Amostragem,
    /// A área a remover (a seleção do painel).
    Destino,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EstadoDoCalculo {
    Ocioso,
    Calculando { final_: bool },
    Pronto { final_: bool },
    Falhou(String),
}

pub struct Download {
    pub cancelar: Arc<AtomicBool>,
    pub progresso: Arc<Mutex<(u64, u64)>>,
}

pub struct EspacoDoPreenchimento {
    pub metodo: Metodo,
    pub backend: Backend,
    /// A seleção a remover (editável com o pincel no alvo Destino).
    pub selecao: Selecao,
    /// Derivados da seleção e da suavização: o que o motor reconstrói e o
    /// peso com que o remendo entra.
    pub destino: Selecao,
    pub peso: Selecao,
    pub amostragem: Selecao,
    /// A amostragem foi mexida à mão (não volta à automática sozinha).
    pub amostragem_manual: bool,
    /// O instantâneo: a composição até a camada escolhida, e a versão do
    /// documento quando ele foi tirado.
    pub foto: Arc<image::RgbImage>,
    pub versao: u64,
    pub alvo: AlvoDoPincel,
    pub incluir: bool,
    pub ver_original: bool,
    pub em_camada_nova: bool,
    pub pedido: u64,
    pub cancelar: Arc<AtomicBool>,
    pub progresso: Arc<Mutex<Option<Progresso>>>,
    pub estado: EstadoDoCalculo,
    pub resultado: Option<Resultado>,
    /// A janela da Visualização: o recorte em volta do destino, antes e
    /// depois (este só com um resultado).
    pub visualizacao_antes: Option<Arc<RenderImage>>,
    pub visualizacao_depois: Option<Arc<RenderImage>>,
    pub sobreposicao: Option<Arc<RenderImage>>,
    pub pincelando: Option<(f32, f32)>,
    /// Já pediu Visualizar alguma vez (a dica muda para "visualize de novo").
    pub visualizou: bool,
    pub download: Option<Download>,
    pub aviso_do_modelo: Option<String>,
    pub tempo: Option<Duration>,
}

impl Drop for EspacoDoPreenchimento {
    /// Fechar o painel (ou a janela) derruba o cálculo e o download.
    fn drop(&mut self) {
        self.cancelar.store(true, Ordering::Relaxed);
        if let Some(d) = &self.download {
            d.cancelar.store(true, Ordering::Relaxed);
        }
    }
}

/// A suavização padrão da borda, em pixels.
pub const SUAVIZACAO_PADRAO: f32 = 2.0;
/// A margem de contexto padrão da IA (fração do lado do destino).
pub const CONTEXTO_PADRAO: f32 = 0.6;

/// 💾 As últimas escolhas do preenchimento — método, backend, margem de
/// contexto, suavização e "camada nova" —, de volta na próxima vez (dono,
/// 07/out/2026: *"deixe as últimas configurações gravadas"*). Gravadas a cada
/// mudança em `preenchimento.json`, ao lado das docas; o que faltar ou não
/// valer mais (um backend que esta versão não compila) volta ao padrão.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Lembrado {
    pub metodo: String,
    pub backend: String,
    /// Em porcentagem, como no slider.
    pub contexto: f32,
    pub suavizacao: f32,
    pub camada_nova: bool,
}

impl Default for Lembrado {
    fn default() -> Self {
        Self {
            metodo: Metodo::PatchMatch.chave().into(),
            backend: Backend::Automatico.chave().into(),
            contexto: CONTEXTO_PADRAO * 100.0,
            suavizacao: SUAVIZACAO_PADRAO,
            camada_nova: true,
        }
    }
}

impl Lembrado {
    /// Nos testes, em lugar nenhum: a suíte não mexe no que o dono escolheu.
    #[cfg(not(test))]
    fn arquivo() -> Option<std::path::PathBuf> {
        Some(infrastructure::paths::AppPaths::catalog_root().join("preenchimento.json"))
    }

    #[cfg(test)]
    fn arquivo() -> Option<std::path::PathBuf> {
        None
    }

    pub fn ler() -> Self {
        Self::arquivo()
            .and_then(|c| std::fs::read_to_string(c).ok())
            .map(|t| Self::do_texto(&t))
            .unwrap_or_default()
    }

    /// O JSON gravado, com o que não vale mais trocado pelo padrão.
    pub fn do_texto(texto: &str) -> Self {
        let mut l: Self = serde_json::from_str(texto).unwrap_or_default();
        let padrao = Self::default();
        if Metodo::da_chave(&l.metodo).is_none() {
            l.metodo = padrao.metodo;
        }
        if !Backend::da_chave(&l.backend).is_some_and(|b| Backend::compilados().contains(&b)) {
            l.backend = padrao.backend;
        }
        if !l.contexto.is_finite() {
            l.contexto = padrao.contexto;
        }
        if !l.suavizacao.is_finite() {
            l.suavizacao = padrao.suavizacao;
        }
        l.contexto = l.contexto.clamp(0.0, 200.0);
        l.suavizacao = l.suavizacao.clamp(0.0, 30.0);
        l
    }

    pub fn metodo(&self) -> Metodo {
        Metodo::da_chave(&self.metodo).unwrap_or(Metodo::PatchMatch)
    }

    pub fn backend(&self) -> Backend {
        Backend::da_chave(&self.backend).unwrap_or(Backend::Automatico)
    }

    fn gravar(&self) {
        let Some(caminho) = Self::arquivo() else {
            return;
        };
        let Ok(texto) = serde_json::to_string_pretty(self) else {
            return;
        };
        if let Some(pasta) = caminho.parent() {
            let _ = std::fs::create_dir_all(pasta);
        }
        if let Err(erro) = std::fs::write(&caminho, texto) {
            crate::telemetria::avisar!(
                "⚠️ as escolhas do preenchimento não foram gravadas: {erro}"
            );
        }
    }
}

impl EditorDeFoto {
    /// Grava as escolhas de agora (ver [`Lembrado`]).
    pub(super) fn lembrar_o_preenchimento(&self, cx: &App) {
        Lembrado {
            metodo: self.metodo_do_preenchimento.chave().into(),
            backend: self.backend_da_ia.chave().into(),
            // Um décimo basta: o slider anda de 5 em 5 e de 1 em 1.
            contexto: (self.contexto_da_ia.read(cx).value().start() * 10.0).round() / 10.0,
            suavizacao: (self.suavizacao.read(cx).value().start() * 10.0).round() / 10.0,
            camada_nova: self.camada_nova_do_preenchimento,
        }
        .gravar();
    }

    pub fn preenchendo_pelo_painel(&self) -> bool {
        self.area_do_preenchimento.is_some()
    }

    pub fn espaco_do_preenchimento(&self) -> Option<&EspacoDoPreenchimento> {
        self.area_do_preenchimento.as_ref()
    }

    /// Abre o painel: o instantâneo, o destino (a seleção) e a amostragem
    /// automática. **Não calcula**: o operador ajusta e pede Visualizar.
    pub fn abrir_preenchimento(&mut self, cx: &mut Context<Self>) {
        if self.area_do_preenchimento.is_some() || self.avisar_se_na_mascara(cx) {
            return;
        }
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let (largura, altura) = (s.base().width(), s.base().height());
        let selecao = s
            .selecao()
            .cloned()
            .unwrap_or_else(|| Selecao::vazia(largura, altura));
        let foto = Arc::new(s.foto_ate_a_ativa(&Retangulo::inteiro(largura, altura)));
        let versao = s.versao();
        let metodo = self.metodo_do_preenchimento;
        let raio = self.suavizacao_do_preenchimento(cx);
        let (destino, peso) = calculo::destino_e_peso(&selecao, raio);
        let amostragem = calculo::amostragem_automatica(&destino);
        self.area_do_preenchimento = Some(EspacoDoPreenchimento {
            metodo,
            backend: self.backend_da_ia,
            selecao,
            destino,
            peso,
            amostragem,
            amostragem_manual: false,
            foto,
            versao,
            alvo: AlvoDoPincel::Amostragem,
            incluir: true,
            ver_original: false,
            em_camada_nova: self.camada_nova_do_preenchimento,
            pedido: 0,
            cancelar: Arc::new(AtomicBool::new(false)),
            progresso: Arc::new(Mutex::new(None)),
            estado: EstadoDoCalculo::Ocioso,
            resultado: None,
            visualizacao_antes: None,
            visualizacao_depois: None,
            sobreposicao: None,
            pincelando: None,
            visualizou: false,
            download: None,
            aviso_do_modelo: None,
            tempo: None,
        });
        self.refazer_a_sobreposicao();
        self.refazer_a_visualizacao();
        self.aviso = None;
        cx.notify();
    }

    /// Refaz as imagens da Visualização com o destino e o resultado de agora.
    pub(super) fn refazer_a_visualizacao(&mut self) {
        let Some(e) = self.area_do_preenchimento.as_mut() else {
            return;
        };
        let (w, h, bgra) =
            calculo::visualizacao(&e.foto, &e.destino, &e.peso, None, LADO_DA_VISUALIZACAO);
        e.visualizacao_antes = crate::imagem::de_bgra(w, h, bgra);
        e.visualizacao_depois = e.resultado.as_ref().and_then(|r| {
            let (w, h, bgra) =
                calculo::visualizacao(&e.foto, &e.destino, &e.peso, Some(r), LADO_DA_VISUALIZACAO);
            crate::imagem::de_bgra(w, h, bgra)
        });
    }

    /// Visualizar (ou Enter sem prévia): calcula com os ajustes de agora.
    ///
    /// # 🚨 Por que não calcula sozinho
    ///
    /// *"Quando você entra na ferramenta de Preenchimento sensível ao
    /// conteúdo, logo de cara o efeito já aplicado. Precisa ter um botão de
    /// visualizar, pois nem temos oportunidade de mexer nas configurações"*
    /// (dono, 07/out/2026). Até a 0.1.107 o painel calculava ao abrir e a cada
    /// ajuste — com a IA, segundos de CPU por mudança que o operador ainda
    /// nem terminou. Agora abrir e ajustar só preparam; o cálculo é pedido.
    pub fn visualizar_preenchimento(&mut self, cx: &mut Context<Self>) {
        if let Some(e) = self.area_do_preenchimento.as_mut() {
            e.visualizou = true;
        }
        self.agendar_preenchimento(Duration::ZERO, cx);
    }

    /// Um ajuste mudou (ou Parar): o cálculo em andamento é derrubado e a
    /// prévia sai — ela não mostra mais o que Aplicar gravaria. Volta com
    /// Visualizar.
    pub fn invalidar_previa(&mut self, cx: &mut Context<Self>) {
        let Some(e) = self.area_do_preenchimento.as_mut() else {
            return;
        };
        e.cancelar.store(true, Ordering::Relaxed);
        e.cancelar = Arc::new(AtomicBool::new(false));
        e.pedido += 1;
        e.resultado = None;
        e.tempo = None;
        e.estado = EstadoDoCalculo::Ocioso;
        e.visualizacao_depois = None;
        *e.progresso.lock().unwrap_or_else(|p| p.into_inner()) = None;
        cx.notify();
    }

    /// Enter com o painel aberto: aplica a prévia final; sem ela, visualiza.
    pub fn confirmar_preenchimento(&mut self, cx: &mut Context<Self>) {
        let Some(e) = self.area_do_preenchimento.as_ref() else {
            return;
        };
        match e.estado {
            EstadoDoCalculo::Pronto { final_: true } => self.aplicar_preenchimento(cx),
            EstadoDoCalculo::Calculando { .. } => {}
            _ => self.visualizar_preenchimento(cx),
        }
    }

    /// Esc / Cancelar: o documento fica como estava.
    pub fn cancelar_preenchimento(&mut self, cx: &mut Context<Self>) {
        self.area_do_preenchimento = None;
        cx.notify();
    }

    fn suavizacao_do_preenchimento(&self, cx: &App) -> u32 {
        self.suavizacao.read(cx).value().start().round().max(0.0) as u32
    }

    fn contexto_do_preenchimento(&self, cx: &App) -> f32 {
        self.contexto_da_ia.read(cx).value().start() / 100.0
    }

    /// A seleção ou a suavização mudaram: refaz o destino e o peso (e a
    /// amostragem automática, se ninguém a mexeu).
    fn refazer_o_destino(&mut self, cx: &mut Context<Self>) {
        let raio = self.suavizacao_do_preenchimento(cx);
        let Some(e) = self.area_do_preenchimento.as_mut() else {
            return;
        };
        let (destino, peso) = calculo::destino_e_peso(&e.selecao, raio);
        e.destino = destino;
        e.peso = peso;
        if !e.amostragem_manual {
            e.amostragem = calculo::amostragem_automatica(&e.destino);
        }
        self.refazer_a_sobreposicao();
        self.refazer_a_visualizacao();
    }

    pub(super) fn refazer_a_sobreposicao(&mut self) {
        let Some(e) = self.area_do_preenchimento.as_mut() else {
            return;
        };
        let amostra = e.metodo.capacidades().amostragem;
        let vazia = Selecao::vazia(e.destino.largura(), e.destino.altura());
        let (w, h, bgra) = calculo::sobreposicao(
            &e.destino,
            if amostra { &e.amostragem } else { &vazia },
            1024,
        );
        e.sobreposicao = crate::imagem::de_bgra(w, h, bgra);
    }

    /// Pede um cálculo novo depois de `atraso` (o agrupamento das mudanças
    /// rápidas do pincel). O pedido anterior é cancelado; a resposta dele, se
    /// chegar, é descartada pelo número.
    pub fn agendar_preenchimento(&mut self, atraso: Duration, cx: &mut Context<Self>) {
        let contexto = self.contexto_do_preenchimento(cx);
        let Some(e) = self.area_do_preenchimento.as_mut() else {
            return;
        };
        e.cancelar.store(true, Ordering::Relaxed);
        e.cancelar = Arc::new(AtomicBool::new(false));
        e.pedido += 1;
        e.resultado = None;
        e.visualizacao_depois = None;
        e.tempo = None;
        *e.progresso.lock().unwrap_or_else(|p| p.into_inner()) = None;
        let pedido = e.pedido;
        let metodo = e.metodo;
        if metodo.capacidades().precisa_de_modelo
            && !matches!(
                modelos::estado(&modelos::pasta_padrao(), &lama::MODELO),
                EstadoDoModelo::Instalado { .. }
            )
        {
            e.estado = EstadoDoCalculo::Falhou(Erro::ModeloAusente.mensagem());
            cx.notify();
            return;
        }
        e.estado = EstadoDoCalculo::Calculando { final_: false };
        let foto = e.foto.clone();
        let destino = e.destino.clone();
        let amostragem = e.amostragem.clone();
        let cancelar = e.cancelar.clone();
        let progresso = e.progresso.clone();
        let backend = e.backend;
        let caixa = destino.caixa_justa();
        let semente = ((caixa.x as u64) << 48)
            ^ ((caixa.y as u64) << 32)
            ^ ((caixa.largura as u64) << 16)
            ^ caixa.altura as u64;
        // A provisória só para quem trabalha na resolução da foto: a IA já
        // reduz para a dela, e uma segunda passada reduzida não ajuda.
        let fator_provisorio = if metodo.capacidades().lado_nativo.is_none() {
            calculo::fator_da_previa(&destino)
        } else {
            1
        };
        let rodar = move |fator: u32| {
            let motor: Box<dyn Motor> = match metodo {
                Metodo::PatchMatch => Box::new(preenchimento::patchmatch::PatchMatch),
                Metodo::LaMa => Box::new(lama::LaMa {
                    pasta: modelos::pasta_padrao(),
                    backend,
                }),
            };
            let avisar = |p: Progresso| {
                *progresso.lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
            };
            let controle = Controle {
                cancelado: &cancelar,
                progresso: &avisar,
            };
            let inicio = std::time::Instant::now();
            let r = calculo::calcular(
                &foto,
                &destino,
                &amostragem,
                motor.as_ref(),
                contexto,
                fator,
                semente,
                &controle,
            );
            (r, inicio.elapsed())
        };
        let rodar = Arc::new(rodar);
        self.tarefa_do_painel = Some(cx.spawn(async move |esta, cx| {
            if !atraso.is_zero() {
                cx.background_executor().timer(atraso).await;
            }
            let vigente = |ed: &EditorDeFoto| {
                ed.area_do_preenchimento
                    .as_ref()
                    .is_some_and(|e| e.pedido == pedido)
            };
            if !esta.update(cx, |ed, _| vigente(ed)).unwrap_or(false) {
                return;
            }
            let mut passos = vec![(fator_provisorio, false)];
            if fator_provisorio > 1 {
                passos.push((1, true));
            } else {
                passos[0].1 = true;
            }
            for (fator, final_) in passos {
                let _ = esta.update(cx, |ed, cx| {
                    if let Some(e) = ed
                        .area_do_preenchimento
                        .as_mut()
                        .filter(|e| e.pedido == pedido)
                    {
                        e.estado = EstadoDoCalculo::Calculando { final_ };
                    }
                    ed.vigiar_o_progresso(cx);
                });
                let r = rodar.clone();
                let (resultado, tempo) = cx
                    .background_executor()
                    .spawn(async move { r(fator) })
                    .await;
                let continuar = esta
                    .update(cx, |ed, cx| {
                        ed.receber_preenchimento(pedido, resultado, tempo, final_, cx)
                    })
                    .unwrap_or(false);
                if !continuar {
                    return;
                }
            }
        }));
        cx.notify();
    }

    /// 🧪 Entrega um resultado como se viesse do cálculo do pedido `pedido`.
    #[cfg(test)]
    pub fn receber_para_teste(
        &mut self,
        pedido: u64,
        r: Resultado,
        cx: &mut Context<Self>,
    ) -> bool {
        self.receber_preenchimento(pedido, Ok(r), Duration::ZERO, true, cx)
    }

    /// Um resultado chegou. Devolve se o pedido continua (para o final).
    fn receber_preenchimento(
        &mut self,
        pedido: u64,
        resultado: Result<Resultado, Erro>,
        tempo: Duration,
        final_: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(e) = self
            .area_do_preenchimento
            .as_mut()
            .filter(|e| e.pedido == pedido)
        else {
            // De um pedido antigo (ou do painel já fechado): descartado.
            return false;
        };
        let continua = match resultado {
            Ok(r) => {
                e.resultado = Some(r);
                e.tempo = Some(tempo);
                e.estado = EstadoDoCalculo::Pronto { final_ };
                let (w, h, bgra) = calculo::visualizacao(
                    &e.foto,
                    &e.destino,
                    &e.peso,
                    e.resultado.as_ref(),
                    LADO_DA_VISUALIZACAO,
                );
                e.visualizacao_depois = crate::imagem::de_bgra(w, h, bgra);
                !final_
            }
            Err(Erro::Cancelado) => false,
            Err(erro) => {
                e.estado = EstadoDoCalculo::Falhou(erro.mensagem());
                false
            }
        };
        if final_ {
            self.medidas.ultimo_preenchimento = Some(tempo);
        }
        cx.notify();
        continua
    }

    /// Enquanto calcula ou baixa, refaz a tela a cada 120 ms (o progresso
    /// mora num `Mutex` que o cálculo escreve).
    fn vigiar_o_progresso(&mut self, cx: &mut Context<Self>) {
        if self.vigia_do_painel.is_some() {
            return;
        }
        self.vigia_do_painel = Some(cx.spawn(async move |esta, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            let segue = esta
                .update(cx, |ed, cx| {
                    let ativo = ed.area_do_preenchimento.as_ref().is_some_and(|e| {
                        matches!(e.estado, EstadoDoCalculo::Calculando { .. })
                            || e.download.is_some()
                    });
                    cx.notify();
                    if !ativo {
                        ed.vigia_do_painel = None;
                    }
                    ativo
                })
                .unwrap_or(false);
            if !segue {
                return;
            }
        }));
    }

    /// Enter / Aplicar: o resultado final entra numa camada nova (ou na
    /// escolhida), num passo do desfazer — se o documento não mudou.
    pub fn aplicar_preenchimento(&mut self, cx: &mut Context<Self>) {
        let Some(e) = self.area_do_preenchimento.as_ref() else {
            return;
        };
        let (Some(r), EstadoDoCalculo::Pronto { final_: true }) = (&e.resultado, &e.estado) else {
            return;
        };
        let (versao, ret, rgba, peso, nova) = (
            e.versao,
            r.ret,
            r.rgba.clone(),
            e.peso.clone(),
            e.em_camada_nova,
        );
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let pesar = |x: u32, y: u32| peso.valor(x, y);
        match s.aplicar_preenchimento(versao, &ret, &rgba, &pesar, nova) {
            Ok(()) => {
                s.desmarcar();
                self.area_do_preenchimento = None;
                self.aviso = None;
            }
            Err(motivo) => {
                if let Some(e) = self.area_do_preenchimento.as_mut() {
                    e.estado = EstadoDoCalculo::Falhou(motivo.into());
                }
            }
        }
        cx.notify();
    }

    // ------------------------------------------------------------ pincel

    /// O ponteiro desceu no palco com o painel aberto: o pincel pinta a
    /// amostragem (ou a área a remover); ⌥ inverte incluir e excluir.
    pub(super) fn apertar_no_preenchimento(
        &mut self,
        ponto: Point<Pixels>,
        modificadores: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) {
        let Some(p) = self.na_foto_sem_limite(ponto) else {
            return;
        };
        if let Some(e) = self.area_do_preenchimento.as_mut() {
            e.pincelando = Some(p);
            if e.alvo == AlvoDoPincel::Amostragem && !e.metodo.capacidades().amostragem {
                e.pincelando = None;
                return;
            }
        }
        self.pintar_no_preenchimento(p, p, modificadores.alt, cx);
    }

    pub(super) fn arrastar_no_preenchimento(
        &mut self,
        ponto: Point<Pixels>,
        modificadores: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(de) = self
            .area_do_preenchimento
            .as_ref()
            .and_then(|e| e.pincelando)
        else {
            return false;
        };
        if let Some(p) = self.na_foto_sem_limite(ponto) {
            self.pintar_no_preenchimento(de, p, modificadores.alt, cx);
            if let Some(e) = self.area_do_preenchimento.as_mut() {
                e.pincelando = Some(p);
            }
        }
        true
    }

    pub(super) fn soltar_no_preenchimento(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(e) = self.area_do_preenchimento.as_mut() else {
            return false;
        };
        if e.pincelando.take().is_none() {
            return false;
        }
        if e.alvo == AlvoDoPincel::Destino {
            self.refazer_o_destino(cx);
        }
        self.invalidar_previa(cx);
        true
    }

    fn pintar_no_preenchimento(
        &mut self,
        de: (f32, f32),
        ate: (f32, f32),
        alt: bool,
        cx: &mut Context<Self>,
    ) {
        let raio = self.sessao().map_or(20.0, |s| s.pincel.raio);
        let Some(e) = self.area_do_preenchimento.as_mut() else {
            return;
        };
        let incluir = e.incluir != alt;
        let alvo = match e.alvo {
            AlvoDoPincel::Amostragem => {
                e.amostragem_manual = true;
                &mut e.amostragem
            }
            AlvoDoPincel::Destino => &mut e.selecao,
        };
        // Discos a cada meio raio ao longo do traço.
        let d = (ate.0 - de.0).hypot(ate.1 - de.1);
        let passos = (d / (raio * 0.5).max(1.0)).ceil().max(1.0) as u32;
        for k in 0..=passos {
            let t = k as f32 / passos as f32;
            alvo.pintar_disco(
                de.0 + (ate.0 - de.0) * t,
                de.1 + (ate.1 - de.1) * t,
                raio,
                incluir,
            );
        }
        if e.alvo == AlvoDoPincel::Destino {
            // A sobreposição acompanha o traço na hora; o resto, ao soltar.
            e.destino = e.selecao.clone();
        }
        self.refazer_a_sobreposicao();
        cx.notify();
    }

    pub(super) fn suavizacao_mudou(&mut self, cx: &mut Context<Self>) {
        if self.area_do_preenchimento.is_some() {
            self.refazer_o_destino(cx);
            self.invalidar_previa(cx);
        }
    }

    pub fn redefinir_amostragem(&mut self, cx: &mut Context<Self>) {
        if let Some(e) = self.area_do_preenchimento.as_mut() {
            e.amostragem_manual = false;
        }
        self.refazer_o_destino(cx);
        self.invalidar_previa(cx);
    }

    pub fn mudar_metodo_do_preenchimento(&mut self, metodo: Metodo, cx: &mut Context<Self>) {
        self.metodo_do_preenchimento = metodo;
        self.lembrar_o_preenchimento(cx);
        if let Some(e) = self.area_do_preenchimento.as_mut() {
            e.metodo = metodo;
            if !metodo.capacidades().amostragem {
                e.alvo = AlvoDoPincel::Destino;
            }
        }
        self.refazer_a_sobreposicao();
        self.invalidar_previa(cx);
    }

    pub fn mudar_backend_da_ia(&mut self, backend: Backend, cx: &mut Context<Self>) {
        self.backend_da_ia = backend;
        self.lembrar_o_preenchimento(cx);
        if let Some(e) = self.area_do_preenchimento.as_mut() {
            e.backend = backend;
        }
        if self.metodo_do_preenchimento == Metodo::LaMa {
            self.invalidar_previa(cx);
        }
    }

    pub fn alternar_original(&mut self, cx: &mut Context<Self>) {
        if let Some(e) = self.area_do_preenchimento.as_mut() {
            e.ver_original = !e.ver_original;
        }
        cx.notify();
    }

    pub fn escolher_alvo_do_pincel(
        &mut self,
        alvo: AlvoDoPincel,
        incluir: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(e) = self.area_do_preenchimento.as_mut() {
            e.alvo = alvo;
            e.incluir = incluir;
        }
        cx.notify();
    }

    // ------------------------------------------------------------ modelos

    pub fn baixar_modelo(&mut self, cx: &mut Context<Self>) {
        let Some(e) = self.area_do_preenchimento.as_mut() else {
            return;
        };
        if e.download.is_some() {
            return;
        }
        let cancelar = Arc::new(AtomicBool::new(false));
        let progresso = Arc::new(Mutex::new((0u64, lama::MODELO.bytes)));
        e.download = Some(Download {
            cancelar: cancelar.clone(),
            progresso: progresso.clone(),
        });
        e.aviso_do_modelo = None;
        self.vigiar_o_progresso(cx);
        let trabalho = cx.background_executor().spawn(async move {
            modelos::baixar(
                &modelos::pasta_padrao(),
                &lama::MODELO,
                lama::MODELO.url,
                &|r, t| *progresso.lock().unwrap_or_else(|e| e.into_inner()) = (r, t),
                &cancelar,
            )
        });
        self.tarefa_do_modelo = Some(cx.spawn(async move |esta, cx| {
            let r = trabalho.await;
            let _ = esta.update(cx, |ed, cx| {
                ed.modelo_chegou(r.map(|_| "Modelo baixado e verificado".to_string()), cx)
            });
        }));
        cx.notify();
    }

    pub fn cancelar_download(&mut self, cx: &mut Context<Self>) {
        if let Some(d) = self
            .area_do_preenchimento
            .as_ref()
            .and_then(|e| e.download.as_ref())
        {
            d.cancelar.store(true, Ordering::Relaxed);
        }
        cx.notify();
    }

    pub fn importar_modelo(&mut self, cx: &mut Context<Self>) {
        let escolha = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Importar o modelo LaMa (.onnx)".into()),
        });
        self.tarefa_do_modelo = Some(cx.spawn(async move |esta, cx| {
            let Ok(Ok(Some(caminhos))) = escolha.await else {
                return;
            };
            let Some(origem) = caminhos.into_iter().next() else {
                return;
            };
            let _ = esta.update(cx, |ed, cx| ed.importar_modelo_de(origem, cx));
        }));
    }

    /// Importa um arquivo de modelo (sem o diálogo: o roteiro, os testes).
    pub fn importar_modelo_de(&mut self, origem: std::path::PathBuf, cx: &mut Context<Self>) {
        if let Some(e) = self.area_do_preenchimento.as_mut() {
            e.aviso_do_modelo = Some("Conferindo o arquivo…".into());
        }
        cx.notify();
        let trabalho = cx.background_executor().spawn(async move {
            modelos::importar(
                &modelos::pasta_padrao(),
                &lama::MODELO,
                &origem,
                &lama::conferir_assinatura,
            )
        });
        self.tarefa_do_modelo = Some(cx.spawn(async move |esta, cx| {
            let r = trabalho.await;
            let _ = esta.update(cx, |ed, cx| {
                ed.modelo_chegou(r.map(|_| "Modelo importado".to_string()), cx)
            });
        }));
    }

    fn modelo_chegou(&mut self, r: Result<String, modelos::ErroDeModelo>, cx: &mut Context<Self>) {
        let ok = r.is_ok();
        if let Some(e) = self.area_do_preenchimento.as_mut() {
            e.download = None;
            e.aviso_do_modelo = Some(match r {
                Ok(m) => m,
                Err(erro) => erro.mensagem(),
            });
        }
        if ok && self.metodo_do_preenchimento == Metodo::LaMa {
            ia_local::execucao::liberar();
            self.invalidar_previa(cx);
        }
        cx.notify();
    }

    pub fn remover_modelo(&mut self, cx: &mut Context<Self>) {
        ia_local::execucao::liberar();
        let r = modelos::remover(&modelos::pasta_padrao(), &lama::MODELO);
        if let Some(e) = self.area_do_preenchimento.as_mut() {
            e.aviso_do_modelo = Some(match r {
                Ok(()) => "Modelo removido".into(),
                Err(erro) => erro.mensagem(),
            });
        }
        if self.metodo_do_preenchimento == Metodo::LaMa {
            self.invalidar_previa(cx);
        }
        cx.notify();
    }

    // ------------------------------------------------------------ desenho

    /// O que vai por cima das imagens da foto no palco: a sobreposição
    /// (vermelho = a remover, verde = de onde amostrar), a moldura do
    /// contexto da IA e a prévia.
    pub(super) fn elementos_do_preenchimento(&self, v: &zoom::Vista, cx: &App) -> Vec<AnyElement> {
        let Some(e) = self.area_do_preenchimento.as_ref() else {
            return Vec::new();
        };
        let mut elementos = Vec::new();
        let (l, a) = (e.foto.width() as f32, e.foto.height() as f32);
        let caixa = |r: Retangulo| {
            div()
                .absolute()
                .left(px(v.x + r.x as f32 * v.escala))
                .top(px(v.y + r.y as f32 * v.escala))
                .w(px(r.largura as f32 * v.escala))
                .h(px(r.altura as f32 * v.escala))
        };
        // 🖼️ Como no Photoshop, a foto da esquerda mostra só o que se pinta
        // (vermelho = a remover, verde = de onde amostrar); o resultado mora
        // na janela da Visualização, ao lado.
        if let Some(s) = &e.sobreposicao {
            elementos.push(
                caixa(Retangulo::novo(0, 0, l as u32, a as u32))
                    .child(img(s.clone()).size_full().object_fit(ObjectFit::Fill))
                    .into_any_element(),
            );
        }
        if !e.metodo.capacidades().amostragem {
            let contexto = self.contexto_do_preenchimento(cx);
            let r = calculo::regiao_de_trabalho(&e.destino, &e.amostragem, e.metodo, contexto);
            elementos.push(
                caixa(r)
                    .border_1()
                    .border_dashed()
                    .border_color(gpui_kit::white().opacity(0.8))
                    .into_any_element(),
            );
        }
        elementos
    }

    /// O painel da direita com o preenchimento aberto, em seções do kit
    /// (`GroupBox`): só os controles que o método escolhido tem de verdade.
    pub(super) fn painel_do_preenchimento(&self, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::alert::Alert;
        use gpui_kit::component::button::ButtonGroup;
        use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants as _};
        use gpui_kit::component::spinner::Spinner;
        use gpui_kit::component::switch::Switch;
        use gpui_kit::component::tag::Tag;
        use gpui_kit::component::{h_flex, v_flex, Selectable as _};

        let tema = cx.theme().clone();
        let Some(e) = self.area_do_preenchimento.as_ref() else {
            return div().into_any_element();
        };
        let cap = e.metodo.capacidades();
        let apagado = |t: String| div().text_xs().text_color(tema.muted_foreground).child(t);
        let secao = |titulo: &'static str| {
            GroupBox::new().outline().title(
                div()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(titulo),
            )
        };
        // Um rótulo com o valor à direita, sobre o slider.
        let linha_de_valor = |rotulo: &'static str, valor: String| {
            h_flex()
                .justify_between()
                .text_xs()
                .child(div().text_color(tema.muted_foreground).child(rotulo))
                .child(div().font_weight(gpui_kit::FontWeight::MEDIUM).child(valor))
        };

        // --- Método
        let metodo = secao("Método").child(
            v_flex()
                .gap(px(6.))
                .child(
                    div()
                        .debug_selector(|| "editor-preenchimento-metodo".into())
                        .child(crate::estilo::campo_pequeno(Select::new(&self.seletor_de_metodo).xsmall())),
                )
                .child(apagado(match e.metodo {
                    Metodo::PatchMatch => {
                        "Copia pedaços da própria foto. Respeita a área verde de amostragem e trabalha na resolução da foto.".into()
                    }
                    Metodo::LaMa => {
                        "Rede neural no seu computador. Vê o contexto em volta; trabalha em 512 px.".into()
                    }
                })),
        );

        // --- O modelo (só a IA)
        let modelo = cap.precisa_de_modelo.then(|| {
            let m = &lama::MODELO;
            let estado = modelos::estado(&modelos::pasta_padrao(), m);
            let tamanho = format!("{:.0} MB", m.bytes as f64 / 1e6);
            let (selo, corpo): (Tag, AnyElement) = match (&e.download, &estado) {
                (Some(d), _) => {
                    let (r, t) = *d.progresso.lock().unwrap_or_else(|p| p.into_inner());
                    (
                        Tag::info().xsmall().child("Baixando"),
                        v_flex()
                            .gap(px(6.))
                            .child(apagado(format!(
                                "{:.0} de {:.0} MB",
                                r as f64 / 1e6,
                                t as f64 / 1e6
                            )))
                            .child(
                                Progress::new("editor-modelo-progresso")
                                    .value(r as f32 / t.max(1) as f32 * 100.0)
                                    .h(px(6.)),
                            )
                            .child(
                                crate::estilo::botao_fantasma_pequeno("editor-modelo-cancelar", cx)
                                    .label("Cancelar o download")
                                    .on_click(cx.listener(|ed, _, _, cx| ed.cancelar_download(cx))),
                            )
                            .into_any_element(),
                    )
                }
                (None, EstadoDoModelo::Instalado { registro, .. }) => (
                    if registro.verificado {
                        Tag::success().xsmall().child("Instalado")
                    } else {
                        Tag::warning().xsmall().child("Importado")
                    },
                    v_flex()
                        .gap(px(6.))
                        .child(apagado(if registro.verificado {
                            "Verificado pelo hash publicado. Funciona sem internet.".into()
                        } else {
                            "Arquivo compatível, mas diferente do publicado.".into()
                        }))
                        .child(
                            crate::estilo::botao_fantasma_pequeno("editor-modelo-remover", cx)
                                .label("Remover o modelo")
                                .on_click(cx.listener(|ed, _, _, cx| ed.remover_modelo(cx))),
                        )
                        .into_any_element(),
                ),
                (None, estado) => (
                    if matches!(estado, EstadoDoModelo::Corrompido { .. }) {
                        Tag::danger().xsmall().child("Incompleto")
                    } else {
                        Tag::warning().xsmall().child("Não instalado")
                    },
                    v_flex()
                        .gap(px(6.))
                        .child(apagado(format!(
                            "Baixado uma vez ({tamanho}); depois funciona sem internet."
                        )))
                        .child(
                            h_flex()
                                .gap(px(4.))
                                .flex_wrap()
                                .child(
                                    crate::estilo::botao_primario_pequeno(
                                        "editor-modelo-baixar",
                                        cx,
                                    )
                                    .label(format!("Baixar ({tamanho})"))
                                    .on_click(cx.listener(|ed, _, _, cx| ed.baixar_modelo(cx))),
                                )
                                .child(
                                    crate::estilo::botao_fantasma_pequeno(
                                        "editor-modelo-importar",
                                        cx,
                                    )
                                    .label("Importar arquivo…")
                                    .on_click(cx.listener(|ed, _, _, cx| ed.importar_modelo(cx))),
                                ),
                        )
                        .into_any_element(),
                ),
            };
            secao("Modelo de IA").child(
                v_flex()
                    .gap(px(6.))
                    .child(
                        h_flex()
                            .justify_between()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                                    .child("LaMa · big-lama"),
                            )
                            .child(selo),
                    )
                    .child(corpo)
                    .when_some(e.aviso_do_modelo.clone(), |d, a| d.child(apagado(a)))
                    .child(apagado(format!(
                        "{} · {} · a foto não sai do computador",
                        m.licenca, m.versao
                    ))),
            )
        });

        // --- O pincel: a área a remover sempre; a amostragem, só para quem amostra.
        let alternancia = |id: &'static str,
                           ativo_incluir: bool,
                           ativo_excluir: bool,
                           alvo: AlvoDoPincel,
                           cx: &mut Context<Self>| {
            ButtonGroup::new(id)
                .outline()
                .xsmall()
                .child(
                    crate::estilo::botao_contorno_pequeno(
                        SharedString::from(format!("{id}-incluir")),
                        cx,
                    )
                    .label("Incluir")
                    .selected(ativo_incluir),
                )
                .child(
                    crate::estilo::botao_contorno_pequeno(
                        SharedString::from(format!("{id}-excluir")),
                        cx,
                    )
                    .label("Excluir")
                    .selected(ativo_excluir),
                )
                .on_click(cx.listener(move |ed, cliques: &Vec<usize>, _, cx| {
                    ed.escolher_alvo_do_pincel(alvo, cliques.first() != Some(&1), cx)
                }))
        };
        let mut pincel = v_flex().gap(px(8.));
        if cap.amostragem {
            let (inc, exc) = (
                e.alvo == AlvoDoPincel::Amostragem && e.incluir,
                e.alvo == AlvoDoPincel::Amostragem && !e.incluir,
            );
            pincel = pincel.child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .gap(px(6.))
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child(
                                div()
                                    .size(px(9.))
                                    .rounded_full()
                                    .bg(gpui_kit::rgb(0x3cc850)),
                            )
                            .child(div().text_xs().child("De onde amostrar")),
                    )
                    .child(alternancia(
                        "editor-amostra",
                        inc,
                        exc,
                        AlvoDoPincel::Amostragem,
                        cx,
                    )),
            );
        }
        let (inc, exc) = (
            e.alvo == AlvoDoPincel::Destino && e.incluir,
            e.alvo == AlvoDoPincel::Destino && !e.incluir,
        );
        pincel = pincel
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .gap(px(6.))
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child(
                                div()
                                    .size(px(9.))
                                    .rounded_full()
                                    .bg(gpui_kit::rgb(0xe62828)),
                            )
                            .child(div().text_xs().child("Área a remover")),
                    )
                    .child(alternancia(
                        "editor-destino",
                        inc,
                        exc,
                        AlvoDoPincel::Destino,
                        cx,
                    )),
            )
            .child(linha_de_valor(
                "Tamanho do pincel  [ ]",
                format!(
                    "{:.0} px",
                    self.sessao().map_or(0.0, |s| s.pincel.raio * 2.0)
                ),
            ))
            .child(div().h(px(20.)).child(crate::estilo::slider(&self.tamanho)))
            .child(apagado(if cap.amostragem {
                "⌥ inverte incluir e excluir enquanto pinta.".into()
            } else {
                "A IA vê o contexto da moldura tracejada; não há área de amostragem.".into()
            }));
        if cap.amostragem {
            pincel = pincel.child(
                crate::estilo::botao_fantasma_pequeno("editor-amostra-redefinir", cx)
                    .label("Voltar à amostragem automática")
                    .on_click(cx.listener(|ed, _, _, cx| ed.redefinir_amostragem(cx))),
            );
        }
        let pincel = secao("Pincel").child(pincel);

        // --- Ajustes
        let mut ajustes = v_flex().gap(px(8.));
        if cap.contexto_ajustavel {
            ajustes = ajustes
                .child(linha_de_valor(
                    "Margem de contexto",
                    format!("{:.0}%", self.contexto_da_ia.read(cx).value().start()),
                ))
                .child(
                    div()
                        .h(px(20.))
                        .debug_selector(|| "editor-preenchimento-contexto".into())
                        .child(crate::estilo::slider(&self.contexto_da_ia)),
                );
        }
        ajustes = ajustes
            .child(linha_de_valor(
                "Suavização da borda",
                format!("{:.0} px", self.suavizacao.read(cx).value().start()),
            ))
            .child(
                div()
                    .h(px(20.))
                    .debug_selector(|| "editor-preenchimento-suavizacao".into())
                    .child(crate::estilo::slider(&self.suavizacao)),
            );
        if cap.precisa_de_modelo {
            ajustes = ajustes
                .child(
                    div()
                        .text_xs()
                        .text_color(tema.muted_foreground)
                        .child("Processamento"),
                )
                .child(
                    div()
                        .debug_selector(|| "editor-preenchimento-backend".into())
                        .child(crate::estilo::campo_pequeno(
                            Select::new(&self.seletor_de_backend).xsmall(),
                        )),
                );
        }
        let ajustes = secao("Ajustes").child(ajustes);

        // --- Saída
        let saida = secao("Resultado").child(
            v_flex().gap(px(8.)).child(
                Switch::new("editor-camada-nova-preenchimento")
                    .xsmall()
                    .label("Aplicar numa camada nova")
                    .checked(e.em_camada_nova)
                    .on_click(cx.listener(|ed, marcado: &bool, _, cx| {
                        if let Some(e) = ed.area_do_preenchimento.as_mut() {
                            e.em_camada_nova = *marcado;
                        }
                        ed.camada_nova_do_preenchimento = *marcado;
                        ed.lembrar_o_preenchimento(cx);
                        cx.notify();
                    })),
            ),
        );

        // --- O estado do cálculo
        let progresso = e
            .progresso
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let estado: AnyElement = match &e.estado {
            EstadoDoCalculo::Calculando { final_ } => {
                let etapa = progresso
                    .as_ref()
                    .map(|p| p.etapa.clone())
                    .unwrap_or_else(|| "Preparando".into());
                v_flex()
                    .gap(px(6.))
                    .child(h_flex().gap(px(8.)).child(Spinner::new().xsmall()).child(
                        div().text_xs().child(format!(
                            "{} — {etapa}",
                            if *final_ {
                                "Resultado final"
                            } else {
                                "Prévia provisória"
                            }
                        )),
                    ))
                    .when_some(progresso.and_then(|p| p.fracao), |d, f| {
                        d.child(
                            Progress::new("editor-preenchimento-progresso")
                                .value(f * 100.0)
                                .h(px(6.)),
                        )
                    })
                    .into_any_element()
            }
            EstadoDoCalculo::Pronto { final_ } => {
                let r = e.resultado.as_ref();
                let mut detalhes = Vec::new();
                if let Some(r) = r {
                    let tempo = e
                        .tempo
                        .map(|t| format!("{:.1} s", t.as_secs_f32()))
                        .unwrap_or_default();
                    detalhes.push(format!("{tempo} · {}", r.executado_em));
                    if r.reducao_do_motor > 1.01 {
                        detalhes.push(format!(
                            "A IA trabalha em 512 px: a área foi reduzida 1:{:.1} e ampliada de volta.",
                            r.reducao_do_motor
                        ));
                    }
                }
                let alerta = if *final_ {
                    Alert::success("editor-preenchimento-pronto", detalhes.join("\n"))
                        .title("Resultado final — é o que será gravado")
                } else {
                    Alert::info("editor-preenchimento-pronto", detalhes.join("\n")).title(format!(
                        "Prévia provisória (reduzida 1:{})",
                        r.map_or(1, |r| r.fator)
                    ))
                };
                alerta.xsmall().into_any_element()
            }
            EstadoDoCalculo::Falhou(m) => Alert::error("editor-preenchimento-falhou", m.clone())
                .xsmall()
                .into_any_element(),
            EstadoDoCalculo::Ocioso => Alert::info(
                "editor-preenchimento-ocioso",
                if e.visualizou {
                    "Os ajustes mudaram desde a última visualização."
                } else {
                    "Pinte a área e escolha os ajustes; o cálculo só roda quando você pedir."
                },
            )
            .title(if e.visualizou {
                "Visualize de novo (Enter)"
            } else {
                "Clique em Visualizar (Enter)"
            })
            .xsmall()
            .into_any_element(),
        };
        let calculando = matches!(e.estado, EstadoDoCalculo::Calculando { .. });

        let pronto = matches!(e.estado, EstadoDoCalculo::Pronto { final_: true });
        v_flex()
            .id("editor-preenchimento")
            .debug_selector(|| "editor-preenchimento".into())
            .size_full()
            .border_l_1()
            .border_color(tema.border)
            .bg(tema.background)
            // As seções, rolando.
            .child(
                v_flex()
                    .id("editor-preenchimento-opcoes")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .gap(px(10.))
                    .p(px(12.))
                    .child(metodo)
                    .children(modelo)
                    .child(pincel)
                    .child(ajustes)
                    .child(saida)
                    .child(
                        div()
                            .debug_selector(|| "editor-preenchimento-estado".into())
                            .child(estado),
                    ),
            )
            // O rodapé: cancelar, visualizar e aplicar, sempre à vista.
            .child(
                h_flex()
                    .gap(px(8.))
                    .p(px(12.))
                    .border_t_1()
                    .border_color(tema.border)
                    .child(
                        crate::estilo::botao_contorno("editor-preenchimento-cancelar", cx)
                            .flex_1()
                            .child("Cancelar")
                            .tooltip("Esc — a foto fica como estava")
                            .on_click(cx.listener(|ed, _, _, cx| ed.cancelar_preenchimento(cx))),
                    )
                    .child(if calculando {
                        crate::estilo::botao_secundario("editor-preenchimento-visualizar", cx)
                            .flex_1()
                            .child("Parar")
                            .tooltip("Interrompe o cálculo; os ajustes ficam")
                            .on_click(cx.listener(|ed, _, _, cx| ed.invalidar_previa(cx)))
                    } else {
                        crate::estilo::botao_secundario("editor-preenchimento-visualizar", cx)
                            .flex_1()
                            .child("Visualizar")
                            .tooltip("Enter — calcula com os ajustes de agora; a foto não muda")
                            .on_click(cx.listener(|ed, _, _, cx| ed.visualizar_preenchimento(cx)))
                    })
                    .child(
                        crate::estilo::botao_primario("editor-preenchimento-aplicar", cx)
                            .flex_1()
                            .child("Aplicar")
                            .tooltip(if pronto {
                                "Enter — grava o visualizado numa camada de retoque"
                            } else {
                                "Visualize primeiro: só se aplica o que foi visto"
                            })
                            .disabled(!pronto)
                            .on_click(cx.listener(|ed, _, _, cx| ed.aplicar_preenchimento(cx))),
                    ),
            )
            .into_any_element()
    }
}

impl EditorDeFoto {
    /// 🖼️ A barra do espaço modal: no lugar da do editor (Salvar, zoom,
    /// desfazer ficam fora de alcance enquanto ele está aberto, como no
    /// Photoshop). Também é a barra de título no GNOME.
    pub(super) fn barra_do_preenchimento(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme().clone();
        let fraca = cx.entity().downgrade();
        let controles = crate::janela::controles_com_fechar(
            "janela-editor",
            tema.foreground,
            window,
            cx,
            move |window, cx| {
                let _ = fraca.update(cx, |ed, cx| ed.fechar(window, cx));
            },
        );
        crate::janela::como_barra_de_titulo(div(), "barra-do-preenchimento", window, cx)
            .debug_selector(|| "editor-barra-do-preenchimento".into())
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(48.))
            .pl(px(12.))
            .pr(px(4.))
            .border_b_1()
            .border_color(tema.border)
            .child(gpui_kit::component::Icon::new(Icone::Sparkles).size_4())
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child("Preenchimento sensível ao conteúdo"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(format!("{} · a foto só muda ao aplicar", self.foto.nome)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child("Esc cancela · Enter visualiza ou aplica"),
            )
            .child(controles)
            .into_any_element()
    }

    /// 🖼️ A janela da Visualização (o meio do espaço modal): o recorte em
    /// volta da área, antes ou depois.
    pub(super) fn visualizacao_do_preenchimento(&self, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::button::ButtonGroup;
        use gpui_kit::component::spinner::Spinner;
        use gpui_kit::component::{h_flex, v_flex, Selectable as _};

        let tema = cx.theme().clone();
        let Some(e) = self.area_do_preenchimento.as_ref() else {
            return div().into_any_element();
        };
        let ha_depois = e.visualizacao_depois.is_some();
        let depois = ha_depois && !e.ver_original;
        let (imagem, seletor) = if depois {
            (e.visualizacao_depois.clone(), "editor-visualizacao-depois")
        } else {
            (e.visualizacao_antes.clone(), "editor-visualizacao-antes")
        };
        let calculando = matches!(e.estado, EstadoDoCalculo::Calculando { .. });
        let legenda = if calculando {
            "Calculando…"
        } else if !ha_depois {
            "Antes — clique em Visualizar para ver o resultado"
        } else if depois {
            "Depois — é o que Aplicar grava"
        } else {
            "Antes — a foto como está"
        };
        v_flex()
            .id("editor-visualizacao")
            .debug_selector(|| "editor-visualizacao".into())
            .size_full()
            .border_l_1()
            .border_color(tema.border)
            .bg(tema.secondary)
            .child(
                h_flex()
                    .gap(px(8.))
                    .h(px(40.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(tema.border)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child("Visualização"),
                    )
                    .when(calculando, |d| d.child(Spinner::new().xsmall()))
                    .child(div().flex_1())
                    .child(
                        ButtonGroup::new("editor-ver-original")
                            .outline()
                            .xsmall()
                            .child(
                                crate::estilo::botao_contorno_pequeno("editor-ver-antes", cx)
                                    .label("Antes")
                                    .selected(!depois),
                            )
                            .child(
                                crate::estilo::botao_contorno_pequeno("editor-ver-depois", cx)
                                    .label("Depois")
                                    .selected(depois)
                                    .disabled(!ha_depois),
                            )
                            .on_click(cx.listener(|ed, cliques: &Vec<usize>, _, cx| {
                                if let Some(e) = ed.area_do_preenchimento.as_mut() {
                                    e.ver_original = cliques.first() != Some(&1);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .p(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when_some(imagem, |d, imagem| {
                        d.child(
                            div()
                                .debug_selector(move || seletor.into())
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    img(imagem)
                                        .max_w_full()
                                        .max_h_full()
                                        .object_fit(ObjectFit::Contain),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .border_t_1()
                    .border_color(tema.border)
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(legenda),
            )
            .into_any_element()
    }
}

impl Drop for EditorDeFoto {
    /// A janela fechou: o painel (e o cálculo dele) cai junto, e a sessão da
    /// IA (~1 GB carregada) é solta — reabrir carrega de novo.
    fn drop(&mut self) {
        self.area_do_preenchimento = None;
        ia_local::execucao::liberar();
    }
}

#[cfg(test)]
mod testes_do_lembrado {
    use super::*;

    #[test]
    fn o_lembrado_volta_inteiro_e_o_que_nao_vale_cai_no_padrao() {
        let l = Lembrado {
            metodo: Metodo::LaMa.chave().into(),
            backend: Backend::Cpu.chave().into(),
            contexto: 85.0,
            suavizacao: 7.0,
            camada_nova: false,
        };
        let texto = serde_json::to_string(&l).unwrap();
        assert_eq!(Lembrado::do_texto(&texto), l);
        assert_eq!(Lembrado::do_texto(&texto).metodo(), Metodo::LaMa);
        // Campo a menos, método e backend desconhecidos, número fora da faixa.
        let estranho = Lembrado::do_texto(
            r#"{"metodo":"photoshop","backend":"tpu","contexto":999,"camada_nova":false}"#,
        );
        assert_eq!(estranho.metodo(), Metodo::PatchMatch);
        assert_eq!(estranho.backend(), Backend::Automatico);
        assert_eq!(estranho.contexto, 200.0);
        assert_eq!(estranho.suavizacao, SUAVIZACAO_PADRAO);
        assert!(!estranho.camada_nova);
        assert_eq!(Lembrado::do_texto("lixo"), Lembrado::default());
    }
}
