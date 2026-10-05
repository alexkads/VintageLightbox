//! A janela do editor em camadas — uma por foto.
//!
//! ```text
//! ┌ ✎ img0042.jpg  • Alterações não salvas   Encaixar 1:1 25%   ↶ ↷  [Salvar] [Fechar] ┐
//! ├────────────────────────────────────────────────────────────────┬──────────────────┤
//! │                                                                │ Ferramenta       │
//! │                  a foto (vista em ladrilhos)                   │ Tamanho …        │
//! │                                                                │ Camadas          │
//! │                                                                │  [Normal ▾] op.  │
//! │                                                                │  👁 Camada 2     │
//! │                                                                │  👁 Pintura      │
//! │                                                                │  + ⧉ ↑ ↓ 🗑      │
//! └────────────────────────────────────────────────────────────────┴──────────────────┘
//! ```
//!
//! 🔑 **A janela não sabe da Revelação.** Ela carrega a base neutra, pinta,
//! salva pela porta e anuncia [`EventoDoEditor::Salva`]; quem troca a fonte da
//! Revelação é a raiz (`docs/editor-em-camadas/02-CONTRATO.md`).
//!
//! 🔑 **A foto pinta em resolução cheia e a tela vê a vista reduzida**, em
//! ladrilhos de 256 px: cada gesto só reenvia à GPU os ladrilhos que sujou
//! (`editor-core/src/vista.rs`) — é o que deixa o pincel acompanhar a mão numa
//! foto de 24 MP.
//!
//! 🔍 **O zoom é o da Revelação** (`revelacao::zoom`, com o "1:1" de um pixel
//! da foto num pixel do dispositivo) e os gestos também: pinça e `⌘`/`⌥` + roda
//! ampliam em torno do cursor, a roda e o Espaço + arrastar movem, `Z` alterna,
//! `⌘=` `⌘−` `⌘0` `⌘⌥0`. Com a foto ampliada além da vista, a janela pede uma
//! **lupa**: a vista só do pedaço visível, montada fora da thread da tela, por
//! cima da vista inteira. A partir de 8 pixels do dispositivo por pixel, cada
//! pixel é um quadrado nítido (o `pixels_nitidos` da Revelação).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use editor_core::sessao::PedidoDeLupa;
use editor_core::vista::Vista as VistaDoEditor;
use editor_core::{
    BaseRef, Documento, Ferramenta, Forma, Historico, Modo, Operacao, Retangulo, Sessao,
    VersaoEditada,
};
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme, Disableable, Sizable};
use gpui_kit::{
    canvas, div, img, prelude::*, px, AnyElement, Bounds, Context, Entity, EventEmitter,
    FocusHandle, Focusable, KeyUpEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ObjectFit, PathBuilder, PinchEvent, Pixels, Point, RenderImage, ScrollWheelEvent, SharedString,
    Subscription, Task, Window,
};
use image::DynamicImage;

use super::porta::{Abertura, Edicoes, FotoDoEditor};
use super::{
    Afastar, AlternarCamada, AlternarZoom, ApagarSelecao, Aproximar, CamadaDeBaixo, CamadaDeCima,
    DescerCamada, DesfazerNoEditor, Desmarcar, DuplicarCamada, Encaixar, FecharEditor,
    InverterSelecao, MesclarParaBaixo, NovaCamada, PincelMaior, PincelMenor, PreencherSelecao,
    RefazerNoEditor, SalvarNoEditor, SegurarAMao, SelecaoEliptica, SelecaoLaco, SelecaoRetangular,
    SelecionarTudo, SubirCamada, UmPorUm, UsarBorracha, UsarPincel, CONTEXTO,
};
use crate::campo::TrocarValor as _;
use crate::recursos::Icone;
use crate::revelacao::zoom::{self, Cena, EstadoDoZoom, Medidas as MedidasDaCena, Nivel, Ponto};
use crate::sessoes::filtros_da_lista::Opcao;
use crate::tema;

/// O maior lado da vista reduzida, em pixels: o dobro de uma tela de ~1000 pt,
/// que é o que a janela mostra numa tela retina.
pub const LADO_DA_VISTA: u32 = 2048;

/// As cores das amostras do pincel.
const AMOSTRAS: [[u8; 3]; 10] = [
    [0, 0, 0],
    [255, 255, 255],
    [128, 128, 128],
    [200, 30, 30],
    [240, 140, 20],
    [240, 220, 40],
    [40, 170, 70],
    [40, 120, 220],
    [140, 60, 200],
    [120, 80, 50],
];

/// O que a janela anuncia.
#[derive(Clone, Debug)]
pub enum EventoDoEditor {
    /// A edição foi salva e confirmada no catálogo. `versao` é `None` quando o
    /// projeto não muda a foto (C30): a Revelação volta ao bruto.
    Salva {
        foto: FotoDoEditor,
        versao: Option<VersaoEditada>,
    },
}

impl EventEmitter<EventoDoEditor> for EditorDeFoto {}

enum Fase {
    Carregando,
    Falhou(String),
    Pronta(Box<Sessao>),
}

/// O que a medida da responsividade guarda (`medir-editor` e o roteiro).
#[derive(Default, Clone, Copy, Debug)]
pub struct Medidas {
    /// O último gesto do pincel: da chegada do evento à vista refeita.
    pub ultimo_gesto: Option<Duration>,
    /// Quantos ladrilhos subiram para a GPU no último quadro.
    pub ladrilhos_no_quadro: usize,
    /// O último salvamento inteiro (compor, codificar, gravar, confirmar).
    pub ultimo_salvamento: Option<Duration>,
    /// A última lupa: do pedido à chegada.
    pub ultima_lupa: Option<Duration>,
}

/// Um segmento da borda da seleção, em pixels da foto: `(x0, y0, x1, y1)`.
type Segmento = (u32, u32, u32, u32);

/// De que versão da sessão e de que lupa (região, fator) a borda é.
type ChaveDasBordas = (u64, Option<(Retangulo, u32)>);

/// As ferramentas de seleção (M, ⇧M, L).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoDeSelecao {
    Retangulo,
    Elipse,
    Laco,
}

/// Uma seleção sendo desenhada: os pontos em pixels da foto (no retângulo e
/// na elipse, o primeiro e o último são os cantos).
struct GestoDeSelecao {
    tipo: TipoDeSelecao,
    operacao: Operacao,
    pontos: Vec<(f32, f32)>,
}

impl GestoDeSelecao {
    fn forma(&self) -> Forma {
        let (a, b) = (
            self.pontos[0],
            *self.pontos.last().unwrap_or(&self.pontos[0]),
        );
        let x0 = a.0.min(b.0).max(0.0);
        let y0 = a.1.min(b.1).max(0.0);
        let caixa = Retangulo::novo(
            x0.round() as u32,
            y0.round() as u32,
            (a.0.max(b.0).max(0.0) - x0).round() as u32,
            (a.1.max(b.1).max(0.0) - y0).round() as u32,
        );
        match self.tipo {
            TipoDeSelecao::Retangulo => Forma::Retangulo(caixa),
            TipoDeSelecao::Elipse => Forma::Elipse(caixa),
            TipoDeSelecao::Laco => Forma::Laco(self.pontos.clone()),
        }
    }

    /// O contorno para desenhar enquanto arrasta, em pixels da foto.
    fn contorno(&self) -> Vec<(f32, f32)> {
        let (a, b) = (
            self.pontos[0],
            *self.pontos.last().unwrap_or(&self.pontos[0]),
        );
        match self.tipo {
            TipoDeSelecao::Retangulo => vec![a, (b.0, a.1), b, (a.0, b.1), a],
            TipoDeSelecao::Elipse => {
                let (cx, cy) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                let (rx, ry) = ((b.0 - a.0).abs() / 2.0, (b.1 - a.1).abs() / 2.0);
                (0..=64)
                    .map(|i| {
                        let t = i as f32 / 64.0 * std::f32::consts::TAU;
                        (cx + rx * t.cos(), cy + ry * t.sin())
                    })
                    .collect()
            }
            TipoDeSelecao::Laco => {
                let mut v = self.pontos.clone();
                v.push(a);
                v
            }
        }
    }
}

/// A mão: onde o arrasto começou e a vista de então.
#[derive(Clone, Copy)]
struct Mao {
    inicio: Point<Pixels>,
    vista: zoom::Vista,
}

pub struct EditorDeFoto {
    foto: FotoDoEditor,
    edicoes: Arc<dyn Edicoes>,
    fase: Fase,
    ladrilhos: HashMap<(u32, u32), Arc<RenderImage>>,
    ladrilhos_da_lupa: HashMap<(u32, u32), Arc<RenderImage>>,
    /// Onde a foto está desenhada, medida no quadro anterior.
    palco: Bounds<Pixels>,
    /// Pixels do dispositivo por ponto, do último quadro.
    dpr: f32,
    ponteiro: Option<Point<Pixels>>,
    pintando: bool,
    zoom: EstadoDoZoom,
    /// O nível a que o `Z` volta quando a foto está encaixada.
    alvo_do_z: Nivel,
    z_desde: Option<Instant>,
    /// O Espaço segurado: quando desceu, e se já arrastou com ele.
    espaco: Option<(Instant, bool)>,
    mao: Option<Mao>,
    salvando: bool,
    /// O aviso da barra: o texto, e se é erro.
    aviso: Option<(SharedString, bool)>,
    /// "Fechar" com alterações pendentes: a pergunta está aberta.
    perguntando: bool,
    /// Fechar sem perguntar (depois de Salvar ou Descartar na pergunta).
    liberada: bool,
    foco: FocusHandle,
    tamanho: Entity<SliderState>,
    dureza: Entity<SliderState>,
    opacidade: Entity<SliderState>,
    opacidade_da_camada: Entity<SliderState>,
    modo: Entity<SelectState<Vec<Opcao>>>,
    /// O modo que o Select mostra — para só mexer nele quando mudar.
    modo_mostrado: Option<Modo>,
    /// A camada sendo renomeada e o campo do nome.
    renomeando: Option<(usize, Entity<InputState>)>,
    medidas: Medidas,
    _assinaturas: Vec<Subscription>,
    _assinatura_do_nome: Option<Subscription>,
    _tarefa: Option<Task<()>>,
    _tarefa_da_lupa: Option<Task<()>>,
    /// A ferramenta de seleção, quando é ela que está na mão.
    selecionando: Option<TipoDeSelecao>,
    gesto_de_selecao: Option<GestoDeSelecao>,
    /// A borda da seleção em segmentos da foto, e de que versão/lupa ela é.
    bordas: Option<(ChaveDasBordas, Arc<Vec<Segmento>>)>,
    /// A miniatura de cada camada e a versão da sessão em que foi feita.
    miniaturas: Vec<Arc<RenderImage>>,
    versao_das_miniaturas: Option<u64>,
}

fn slider(
    min: f32,
    max: f32,
    passo: f32,
    valor: f32,
    cx: &mut Context<EditorDeFoto>,
) -> Entity<SliderState> {
    cx.new(|_| {
        SliderState::new()
            .min(min)
            .max(max)
            .step(passo)
            .default_value(valor)
    })
}

fn valor(evento: &SliderEvent) -> (f32, bool) {
    match evento {
        SliderEvent::Change(v) => (v.start(), false),
        SliderEvent::Release(v) => (v.start(), true),
    }
}

fn f(p: Pixels) -> f32 {
    f32::from(p)
}

impl EditorDeFoto {
    /// Abre a janela e começa a carregar a base (`carregar_base`, no executor
    /// de fundo) e o projeto salvo, se houver.
    pub fn novo(
        foto: FotoDoEditor,
        edicoes: Arc<dyn Edicoes>,
        carregar_base: impl FnOnce() -> Result<DynamicImage, String> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let pincel = editor_core::Pincel::default();
        let tamanho = slider(1.0, 800.0, 1.0, pincel.raio, cx);
        let dureza = slider(0.0, 100.0, 1.0, pincel.dureza * 100.0, cx);
        let opacidade = slider(1.0, 100.0, 1.0, pincel.opacidade * 100.0, cx);
        let opacidade_da_camada = slider(0.0, 100.0, 1.0, 100.0, cx);
        let opcoes: Vec<Opcao> = Modo::TODOS
            .iter()
            .map(|m| Opcao::nova(m.chave(), m.nome()))
            .collect();
        let modo = cx.new(|cx| SelectState::new(opcoes, None, window, cx));

        let mut assinaturas = Vec::new();
        for (estado, qual) in [(&tamanho, 0u8), (&dureza, 1), (&opacidade, 2)] {
            assinaturas.push(cx.subscribe_in(
                estado,
                window,
                move |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                    let (v, _) = valor(evento);
                    if let Some(s) = ed.sessao_mut() {
                        match qual {
                            0 => s.pincel.raio = v.max(1.0),
                            1 => s.pincel.dureza = v / 100.0,
                            _ => s.pincel.opacidade = v / 100.0,
                        }
                    }
                    cx.notify();
                },
            ));
        }
        assinaturas.push(cx.subscribe_in(
            &opacidade_da_camada,
            window,
            |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                let (v, soltou) = valor(evento);
                ed.mover_opacidade_da_camada(v / 100.0, soltou, cx);
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &modo,
            window,
            |ed: &mut Self, _e, evento: &SelectEvent<Vec<Opcao>>, window, cx| {
                let SelectEvent::Confirm(Some(chave)) = evento else {
                    return;
                };
                if let Some(modo) = Modo::da_chave(chave) {
                    ed.mudar_modo(modo, cx);
                }
                window.focus(&ed.foco, cx);
            },
        ));

        let foco = cx.focus_handle();
        window.focus(&foco, cx);

        // 🚨 Fechar pelo X da janela com alterações pergunta antes, como o
        // botão "Fechar".
        let fraca = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            fraca
                .update(cx, |ed, cx| ed.pode_fechar(window, cx))
                .unwrap_or(true)
        });

        let mut editor = Self {
            foto,
            edicoes,
            fase: Fase::Carregando,
            ladrilhos: HashMap::new(),
            ladrilhos_da_lupa: HashMap::new(),
            palco: Bounds::default(),
            dpr: window.scale_factor().max(1.0),
            ponteiro: None,
            pintando: false,
            zoom: EstadoDoZoom::default(),
            alvo_do_z: Nivel::Razao(1.0),
            z_desde: None,
            espaco: None,
            mao: None,
            salvando: false,
            aviso: None,
            perguntando: false,
            liberada: false,
            foco,
            tamanho,
            dureza,
            opacidade,
            opacidade_da_camada,
            modo,
            modo_mostrado: None,
            renomeando: None,
            medidas: Medidas::default(),
            _assinaturas: assinaturas,
            _assinatura_do_nome: None,
            _tarefa: None,
            _tarefa_da_lupa: None,
            selecionando: None,
            gesto_de_selecao: None,
            bordas: None,
            miniaturas: Vec::new(),
            versao_das_miniaturas: None,
        };
        editor.carregar(carregar_base, cx);
        editor
    }

    fn carregar(
        &mut self,
        carregar_base: impl FnOnce() -> Result<DynamicImage, String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let edicoes = self.edicoes.clone();
        let foto = self.foto.clone();
        let trabalho = cx.background_executor().spawn(async move {
            // C28: a base é a base neutra do bruto — nunca uma prévia.
            let base = Arc::new(carregar_base()?.to_rgb8());
            let (documento, historico) = match edicoes.abrir(&foto, &base)? {
                Abertura::Nova => (
                    Documento::novo(BaseRef::da_imagem(&base)),
                    Historico::novo(),
                ),
                Abertura::Existente {
                    documento,
                    historico,
                } => (documento, historico),
            };
            Ok::<_, String>(Sessao::nova(base, documento, historico, LADO_DA_VISTA))
        });
        self._tarefa = Some(cx.spawn(async move |esta, cx| {
            let resultado = trabalho.await;
            let _ = esta.update(cx, |ed, cx| {
                ed.fase = match resultado {
                    Ok(sessao) => Fase::Pronta(Box::new(sessao)),
                    Err(erro) => Fase::Falhou(erro),
                };
                cx.notify();
            });
        }));
    }

    // ------------------------------------------------------------ consultas

    pub fn foto(&self) -> &FotoDoEditor {
        &self.foto
    }

    pub fn sessao(&self) -> Option<&Sessao> {
        match &self.fase {
            Fase::Pronta(s) => Some(s),
            _ => None,
        }
    }

    fn sessao_mut(&mut self) -> Option<&mut Sessao> {
        match &mut self.fase {
            Fase::Pronta(s) => Some(s),
            _ => None,
        }
    }

    /// Onde a foto está desenhada, em pontos da janela — o roteiro mira nela.
    /// Ampliada, passa das bordas do palco.
    pub fn area_na_janela(&self) -> Option<Bounds<Pixels>> {
        let (cena, v) = self.vista_do_zoom()?;
        Some(Bounds::new(
            self.palco.origin + gpui_kit::point(px(v.x), px(v.y)),
            gpui_kit::size(
                px(cena.janela.largura * v.escala),
                px(cena.janela.altura * v.escala),
            ),
        ))
    }

    pub fn pronta(&self) -> bool {
        self.sessao().is_some()
    }

    pub fn falha(&self) -> Option<&str> {
        match &self.fase {
            Fase::Falhou(e) => Some(e),
            _ => None,
        }
    }

    /// Há alterações que não foram salvas.
    pub fn alterado(&self) -> bool {
        self.sessao().is_some_and(Sessao::alterado)
    }

    pub fn salvando(&self) -> bool {
        self.salvando
    }

    pub fn aviso(&self) -> Option<(&str, bool)> {
        self.aviso.as_ref().map(|(t, e)| (t.as_ref(), *e))
    }

    pub fn perguntando(&self) -> bool {
        self.perguntando
    }

    pub fn medidas(&self) -> Medidas {
        self.medidas
    }

    /// O título da janela: o arquivo e o ponto das alterações.
    pub fn titulo(&self) -> String {
        format!(
            "Editar — {}{}",
            self.foto.nome,
            if self.alterado() { " •" } else { "" }
        )
    }

    // ---------------------------------------------------------------- zoom

    /// A cena do zoom: a foto inteira, em pixels, no palco medido.
    fn cena(&self) -> Option<Cena> {
        let base = self.sessao()?.base();
        let (pl, pa) = (f(self.palco.size.width), f(self.palco.size.height));
        if pl < 2.0 || pa < 2.0 {
            return None;
        }
        Some(Cena {
            janela: MedidasDaCena {
                largura: base.width() as f32,
                altura: base.height() as f32,
            },
            area: MedidasDaCena {
                largura: pl,
                altura: pa,
            },
            dpr: self.dpr,
            fator_do_bruto: 1.0,
        })
    }

    fn vista_do_zoom(&self) -> Option<(Cena, zoom::Vista)> {
        let cena = self.cena()?;
        Some((cena, zoom::vista_do_zoom(self.zoom, &cena)))
    }

    pub fn nivel_do_zoom(&self) -> Nivel {
        self.zoom.nivel
    }

    /// Pixels do dispositivo por pixel da foto (o "1:1" é 1).
    pub fn razao_do_zoom(&self) -> Option<f32> {
        self.vista_do_zoom()
            .map(|(cena, v)| zoom::razao_da_escala(v.escala, &cena))
    }

    fn ponto_no_palco(&self, posicao: Point<Pixels>) -> Ponto {
        Ponto {
            x: f(posicao.x - self.palco.origin.x),
            y: f(posicao.y - self.palco.origin.y),
        }
    }

    pub fn ir_para_nivel(&mut self, nivel: Nivel, ponto: Option<Ponto>, cx: &mut Context<Self>) {
        let Some((cena, vista)) = self.vista_do_zoom() else {
            self.zoom.nivel = nivel;
            cx.notify();
            return;
        };
        let escala = zoom::escala_do_nivel(nivel, &cena);
        let centro = match ponto {
            Some(p) => zoom::centro_em_torno_de(&vista, escala, p, &cena),
            None => vista.centro,
        };
        self.zoom = EstadoDoZoom { nivel, centro };
        cx.notify();
    }

    /// O `Z` e o Espaço tocado: ampliada volta ao encaixe; encaixada vai ao
    /// último zoom, em torno do ponteiro.
    pub fn alternar_zoom(&mut self, cx: &mut Context<Self>) {
        let ponto = self.ponteiro.map(|p| self.ponto_no_palco(p));
        if self.zoom.nivel != Nivel::Encaixar {
            self.alvo_do_z = self.zoom.nivel;
            self.ir_para_nivel(Nivel::Encaixar, None, cx);
        } else {
            self.ir_para_nivel(self.alvo_do_z, ponto, cx);
        }
    }

    /// `⌘=` (1) e `⌘−` (-1).
    pub fn passo_de_zoom(&mut self, direcao: i32, cx: &mut Context<Self>) {
        let Some((cena, vista)) = self.vista_do_zoom() else {
            return;
        };
        self.zoom = EstadoDoZoom {
            nivel: zoom::proxima_parada(vista.escala, direcao, &cena),
            centro: vista.centro,
        };
        cx.notify();
    }

    /// Amplia `fator` vezes em torno de um ponto do palco.
    pub fn ampliar_em_torno(&mut self, fator: f32, ponto: Ponto, cx: &mut Context<Self>) {
        let Some((cena, vista)) = self.vista_do_zoom() else {
            return;
        };
        let escala = zoom::limitar_escala(vista.escala * fator, &cena);
        self.zoom = EstadoDoZoom {
            nivel: zoom::nivel_da_escala(escala, &cena),
            centro: zoom::centro_em_torno_de(&vista, escala, ponto, &cena),
        };
        cx.notify();
    }

    /// Move a foto ampliada `dx, dy` pontos (a roda e o roteiro).
    pub fn mover_a_foto(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        let Some((cena, vista)) = self.vista_do_zoom() else {
            return;
        };
        if !zoom::passa_da_area(&vista, &cena) {
            return;
        }
        self.zoom.centro = zoom::centro_arrastado(&vista, dx, dy, &cena);
        cx.notify();
    }

    /// A pinça do trackpad. No meio de uma pincelada não: a foto andaria
    /// debaixo do traço.
    fn ao_pincar(&mut self, evento: &PinchEvent, cx: &mut Context<Self>) {
        if self.pintando {
            return;
        }
        let ponto = self.ponto_no_palco(evento.position);
        self.ampliar_em_torno(zoom::fator_da_pinca(evento.delta), ponto, cx);
    }

    fn ao_rolar(&mut self, evento: &ScrollWheelEvent, cx: &mut Context<Self>) {
        if self.pintando {
            return;
        }
        let delta = evento.delta.pixel_delta(px(16.));
        let m = evento.modifiers;
        if m.platform || m.control || m.alt {
            let ponto = self.ponto_no_palco(evento.position);
            self.ampliar_em_torno(zoom::fator_da_roda(-f(delta.y)), ponto, cx);
            return;
        }
        // A roda do GPUI já vem no sentido do conteúdo: somar move a foto.
        self.mover_a_foto(f(delta.x), f(delta.y), cx);
    }

    /// `Z` desceu (repetição não conta): alterna.
    pub fn z_apertado(&mut self, cx: &mut Context<Self>) {
        if self.z_desde.is_some() {
            return;
        }
        self.z_desde = Some(Instant::now());
        self.alternar_zoom(cx);
    }

    /// `Z` subiu: segurado mais de 400 ms era espiar, e volta.
    pub fn z_solto(&mut self, cx: &mut Context<Self>) {
        if let Some(desde) = self.z_desde.take() {
            if desde.elapsed() > Duration::from_millis(400) {
                self.alternar_zoom(cx);
            }
        }
    }

    /// Espaço desceu: a mão, enquanto segurado.
    pub fn espaco_apertado(&mut self, cx: &mut Context<Self>) {
        if self.espaco.is_none() {
            self.espaco = Some((Instant::now(), false));
            cx.notify();
        }
    }

    /// Espaço subiu: tocado sem arrastar, alterna o zoom (como na Revelação).
    pub fn espaco_solto(&mut self, cx: &mut Context<Self>) {
        let Some((desde, usado)) = self.espaco.take() else {
            return;
        };
        if !usado && self.mao.is_none() && desde.elapsed() < Duration::from_millis(500) {
            self.alternar_zoom(cx);
        }
        cx.notify();
    }

    pub fn espaco_segurado(&self) -> bool {
        self.espaco.is_some()
    }

    fn ao_soltar_tecla(&mut self, evento: &KeyUpEvent, cx: &mut Context<Self>) {
        match evento.keystroke.key.as_str() {
            "space" => self.espaco_solto(cx),
            "z" => self.z_solto(cx),
            _ => {}
        }
    }

    fn pegar_com_a_mao(&mut self, posicao: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((_, vista)) = self.vista_do_zoom() else {
            return;
        };
        if let Some((_, usado)) = self.espaco.as_mut() {
            *usado = true;
        }
        self.mao = Some(Mao {
            inicio: posicao,
            vista,
        });
        cx.notify();
    }

    fn arrastar_com_a_mao(&mut self, posicao: Point<Pixels>, cx: &mut Context<Self>) {
        let (Some(mao), Some(cena)) = (self.mao, self.cena()) else {
            return;
        };
        // 🔑 Da vista do começo do gesto: somar sobre a de agora perderia
        // movimento no arrasto rápido.
        if zoom::passa_da_area(&mao.vista, &cena) {
            let (dx, dy) = (f(posicao.x - mao.inicio.x), f(posicao.y - mao.inicio.y));
            self.zoom.centro = zoom::centro_arrastado(&mao.vista, dx, dy, &cena);
        }
        cx.notify();
    }

    // ------------------------------------------------------------ o pincel

    /// Converte um ponto da janela em pixel da foto, dentro dela ou não.
    fn na_foto_sem_limite(&self, ponto: Point<Pixels>) -> Option<(f32, f32)> {
        let (_, v) = self.vista_do_zoom()?;
        let p = self.ponto_no_palco(ponto);
        Some(((p.x - v.x) / v.escala, (p.y - v.y) / v.escala))
    }

    /// Converte um ponto da janela em pixel da foto, se ele cai na foto.
    fn na_foto(&self, ponto: Point<Pixels>) -> Option<(f32, f32)> {
        let base = self.sessao()?.base();
        let (x, y) = self.na_foto_sem_limite(ponto)?;
        (x >= 0.0 && y >= 0.0 && x < base.width() as f32 && y < base.height() as f32)
            .then_some((x, y))
    }

    /// O ponteiro desceu no palco com uma ferramenta de seleção: o gesto
    /// começa (⇧ soma, ⌥ subtrai), mesmo fora da foto.
    pub fn comecar_selecao(
        &mut self,
        ponto: Point<Pixels>,
        modificadores: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) {
        let (Some(tipo), Some(p)) = (self.selecionando, self.na_foto_sem_limite(ponto)) else {
            return;
        };
        let operacao = if modificadores.shift {
            Operacao::Somar
        } else if modificadores.alt {
            Operacao::Subtrair
        } else {
            Operacao::Nova
        };
        self.gesto_de_selecao = Some(GestoDeSelecao {
            tipo,
            operacao,
            pontos: vec![p, p],
        });
        cx.notify();
    }

    /// O ponteiro desceu na foto.
    pub fn apertar(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        if self.selecionando.is_some() {
            self.comecar_selecao(ponto, gpui_kit::Modifiers::none(), cx);
            return;
        }
        let Some((x, y)) = self.na_foto(ponto) else {
            return;
        };
        let inicio = Instant::now();
        let Some(s) = self.sessao_mut() else {
            return;
        };
        if s.apertar(x, y) {
            self.pintando = true;
        } else {
            let nome = s.camada_ativa().nome.clone();
            self.aviso = Some((
                format!("{nome} está escondida — mostre a camada (H) para pintar nela").into(),
                true,
            ));
        }
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
        cx.notify();
    }

    /// O ponteiro andou — pinta se estiver apertado; sempre move o círculo.
    pub fn mover(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        self.ponteiro = Some(ponto);
        if self.gesto_de_selecao.is_some() {
            let escala = self.vista_do_zoom().map_or(1.0, |(_, v)| v.escala);
            let no_ponto = self.na_foto_sem_limite(ponto);
            if let (Some(p), Some(gesto)) = (no_ponto, self.gesto_de_selecao.as_mut()) {
                match gesto.tipo {
                    TipoDeSelecao::Laco => {
                        // Um ponto a cada 2 pontos da tela: o laço segue a mão
                        // sem guardar cada evento.
                        let ultimo = *gesto.pontos.last().unwrap();
                        if (p.0 - ultimo.0).hypot(p.1 - ultimo.1) * escala >= 2.0 {
                            gesto.pontos.push(p);
                        }
                    }
                    _ => {
                        if let Some(fim) = gesto.pontos.last_mut() {
                            *fim = p;
                        }
                    }
                }
            }
            cx.notify();
            return;
        }
        if self.pintando {
            // Fora da foto o traço continua (a conta dá coordenada negativa ou
            // além da borda, e o pincel corta) — como no Photoshop.
            if let Some((x, y)) = self.na_foto_sem_limite(ponto) {
                let inicio = Instant::now();
                if let Some(s) = self.sessao_mut() {
                    s.arrastar(x, y);
                }
                self.medidas.ultimo_gesto = Some(inicio.elapsed());
            }
        }
        cx.notify();
    }

    /// O ponteiro subiu: o traço vira um passo do desfazer.
    pub fn soltar(&mut self, cx: &mut Context<Self>) {
        if self.mao.take().is_some() {
            cx.notify();
        }
        if let Some(gesto) = self.gesto_de_selecao.take() {
            let forma = gesto.forma();
            if let Some(s) = self.sessao_mut() {
                s.selecionar(&forma, gesto.operacao);
            }
            cx.notify();
            return;
        }
        if !std::mem::take(&mut self.pintando) {
            return;
        }
        if let Some(s) = self.sessao_mut() {
            s.soltar();
        }
        self.aviso = None;
        cx.notify();
    }

    pub fn ferramenta(&self) -> Option<Ferramenta> {
        self.sessao().map(|s| s.pincel.ferramenta)
    }

    /// A ferramenta de seleção na mão (`None`: volta ao pincel/borracha).
    pub fn selecionando(&self) -> Option<TipoDeSelecao> {
        self.selecionando
    }

    /// Quantas miniaturas o painel tem prontas (uma por camada).
    pub fn quantas_miniaturas(&self) -> usize {
        self.miniaturas.len()
    }

    pub fn usar_selecao(&mut self, tipo: TipoDeSelecao, cx: &mut Context<Self>) {
        self.selecionando = Some(tipo);
        cx.notify();
    }

    pub fn usar(&mut self, ferramenta: Ferramenta, cx: &mut Context<Self>) {
        self.selecionando = None;
        if let Some(s) = self.sessao_mut() {
            s.pincel.ferramenta = ferramenta;
        }
        cx.notify();
    }

    pub fn escolher_cor(&mut self, cor: [u8; 3], cx: &mut Context<Self>) {
        let com_o_pincel = self.selecionando.is_none();
        if let Some(s) = self.sessao_mut() {
            s.pincel.cor = cor;
            if com_o_pincel {
                s.pincel.ferramenta = Ferramenta::Pincel;
            }
        }
        cx.notify();
    }

    fn mudar_tamanho(&mut self, fator: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let novo = (s.pincel.raio * fator).clamp(1.0, 800.0).round();
        s.pincel.raio = novo;
        self.tamanho
            .update(cx, |estado, cx| estado.set_value(novo, window, cx));
        cx.notify();
    }

    // ------------------------------------------------------------ camadas

    /// Um gesto na sessão que pode ter mudado a pilha: limpa o aviso velho.
    fn na_sessao(&mut self, cx: &mut Context<Self>, fazer: impl FnOnce(&mut Sessao)) {
        if let Some(s) = self.sessao_mut() {
            fazer(s);
            self.aviso = None;
        }
        cx.notify();
    }

    pub fn alternar_visibilidade(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, Sessao::alternar_visibilidade);
    }

    pub fn alternar_visibilidade_de(&mut self, indice: usize, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| s.alternar_visibilidade_de(indice));
    }

    pub fn escolher_camada(&mut self, indice: usize, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| s.escolher_camada(indice));
    }

    /// `⌥]` (1) e `⌥[` (-1): a camada de cima ou de baixo passa a ser a escolhida.
    pub fn escolher_vizinha(&mut self, direcao: i32, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            let alvo = s.ativa() as i64 + direcao as i64;
            if alvo >= 0 {
                s.escolher_camada(alvo as usize);
            }
        });
    }

    pub fn nova_camada(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, Sessao::nova_camada);
    }

    pub fn duplicar_camada(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, Sessao::duplicar_camada);
    }

    pub fn excluir_camada(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.excluir_camada();
        });
    }

    pub fn mover_camada(&mut self, direcao: i32, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.mover_camada(direcao);
        });
    }

    pub fn mudar_modo(&mut self, modo: Modo, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| s.mudar_modo(modo));
    }

    /// ⌘E — com o motivo na barra quando não dá.
    pub fn mesclar_para_baixo(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.sessao_mut() else {
            return;
        };
        self.aviso = s
            .mesclar_para_baixo()
            .err()
            .map(|motivo| (SharedString::from(motivo), true));
        cx.notify();
    }

    pub fn selecionar_tudo(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, Sessao::selecionar_tudo);
    }

    pub fn desmarcar(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, Sessao::desmarcar);
    }

    pub fn inverter_selecao(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, Sessao::inverter_selecao);
    }

    /// Delete: apaga a seleção na camada escolhida.
    pub fn apagar_selecao(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.apagar_selecao();
        });
    }

    /// ⌥Delete: preenche a seleção (ou a camada) com a cor do pincel.
    pub fn preencher_selecao(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let escondida = !s.camada_ativa().visivel;
        s.preencher_selecao();
        self.aviso = escondida.then(|| {
            (
                SharedString::from("A camada está escondida — mostre-a (H) para preencher"),
                true,
            )
        });
        cx.notify();
    }

    /// Refaz a borda da seleção quando ela, a pilha ou a lupa mudaram. Com a
    /// lupa, a borda é a do pedaço dela no fator dela (fina no zoom); sem, a da
    /// foto inteira no fator da vista.
    fn atualizar_as_bordas(&mut self) {
        let Some(s) = self.sessao() else {
            self.bordas = None;
            return;
        };
        let Some(selecao) = s.selecao() else {
            self.bordas = None;
            return;
        };
        let lupa = s.lupa().map(|l| (l.regiao(), l.fator()));
        let chave = (s.versao(), lupa);
        if self.bordas.as_ref().is_some_and(|(c, _)| *c == chave) {
            return;
        }
        let (regiao, passo) = lupa.unwrap_or((
            Retangulo::inteiro(selecao.largura(), selecao.altura()),
            s.vista().fator(),
        ));
        self.bordas = Some((chave, Arc::new(selecao.bordas(&regiao, passo))));
    }

    /// As miniaturas do painel, refeitas quando a sessão mudou e não há traço
    /// em curso (não a cada movimento do pincel).
    fn atualizar_as_miniaturas(&mut self) {
        let Some(s) = self.sessao() else {
            return;
        };
        if s.tracando()
            || (self.versao_das_miniaturas == Some(s.versao())
                && self.miniaturas.len() == s.documento().camadas.len())
        {
            return;
        }
        let (lf, af) = (s.base().width().max(1), s.base().height().max(1));
        let (l, a) = if lf >= af {
            (LADO_DA_MINIATURA, (LADO_DA_MINIATURA * af / lf).max(1))
        } else {
            ((LADO_DA_MINIATURA * lf / af).max(1), LADO_DA_MINIATURA)
        };
        let versao = s.versao();
        let miniaturas = s
            .documento()
            .camadas
            .iter()
            .filter_map(|c| {
                let rgba = editor_core::operacoes::miniatura(&c.pixels, l, a);
                crate::imagem::de_bgra(l, a, sobre_xadrez(&rgba, l))
            })
            .collect();
        self.miniaturas = miniaturas;
        self.versao_das_miniaturas = Some(versao);
    }

    pub fn renomear_camada(&mut self, indice: usize, nome: &str, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.renomear_camada(indice, nome);
        });
    }

    /// Duplo clique no nome: o campo aparece no lugar, com o nome selecionado.
    pub fn comecar_a_renomear(
        &mut self,
        indice: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(nome) = self
            .sessao()
            .and_then(|s| s.documento().camadas.get(indice))
            .map(|c| c.nome.clone())
        else {
            return;
        };
        let campo = cx.new(|cx| InputState::new(window, cx));
        campo.update(cx, |campo, cx| campo.trocar_valor(nome, window, cx));
        let focar = campo.clone();
        window.defer(cx, move |window, cx| {
            focar.update(cx, |campo, cx| campo.focus(window, cx));
        });
        // O nome inteiro selecionado: digitar já troca (o renome da guia).
        let foco_do_campo = Focusable::focus_handle(campo.read(cx), cx);
        cx.spawn_in(window, async move |_ed, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
            let _ = cx.update(|window, cx| {
                foco_do_campo.dispatch_action(&gpui_kit::component::input::SelectAll, window, cx);
            });
        })
        .detach();
        self._assinatura_do_nome = Some(cx.subscribe_in(
            &campo,
            window,
            |ed: &mut Self, _e, evento: &InputEvent, window, cx| match evento {
                InputEvent::PressEnter { .. } => ed.terminar_de_renomear(true, window, cx),
                InputEvent::Blur => ed.terminar_de_renomear(true, window, cx),
                _ => {}
            },
        ));
        self.renomeando = Some((indice, campo));
        cx.notify();
    }

    fn terminar_de_renomear(
        &mut self,
        confirmar: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((indice, campo)) = self.renomeando.take() else {
            return;
        };
        self._assinatura_do_nome = None;
        if confirmar {
            let nome = campo.read(cx).value().to_string();
            self.renomear_camada(indice, &nome, cx);
        }
        window.focus(&self.foco, cx);
        cx.notify();
    }

    pub fn mover_opacidade_da_camada(&mut self, valor: f32, soltou: bool, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.mover_opacidade(valor);
            if soltou {
                s.confirmar_opacidade();
            }
        }
        cx.notify();
    }

    pub fn camada_visivel(&self) -> bool {
        self.sessao().is_some_and(|s| s.camada_ativa().visivel)
    }

    pub fn opacidade_da_camada(&self) -> f32 {
        self.sessao().map_or(1.0, |s| s.camada_ativa().opacidade)
    }

    // ---------------------------------------------------------- desfazer

    pub fn desfazer(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.desfazer();
        });
    }

    pub fn refazer(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.refazer();
        });
    }

    // ------------------------------------------------------------ salvar

    /// Salva em segundo plano. `e_fechar`: fecha a janela quando der certo (o
    /// "Salvar" da pergunta de fechar).
    pub fn salvar(&mut self, e_fechar: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.salvando {
            return;
        }
        let Some(sessao) = self.sessao_mut() else {
            return;
        };
        let (documento, historico) = sessao.instantaneo();
        let base = sessao.base().clone();
        let edicoes = self.edicoes.clone();
        let foto = self.foto.clone();
        self.salvando = true;
        self.aviso = Some(("Salvando…".into(), false));
        cx.notify();

        let inicio = Instant::now();
        let para_gravar = (foto.clone(), documento, historico.clone());
        let trabalho = cx.background_executor().spawn(async move {
            let (foto, documento, historico) = para_gravar;
            edicoes.salvar(&foto, &base, &documento, &historico)
        });
        let janela = window.window_handle();
        self._tarefa = Some(cx.spawn(async move |esta, cx| {
            let resultado = trabalho.await;
            let _ = esta.update(cx, |ed, cx| {
                ed.salvando = false;
                ed.medidas.ultimo_salvamento = Some(inicio.elapsed());
                match resultado {
                    Ok(versao) => {
                        if let Some(s) = ed.sessao_mut() {
                            s.salvo(&historico);
                        }
                        ed.aviso = Some((
                            match &versao {
                                Some(v) => format!("Salvo — revisão {}", v.revisao),
                                None => "Salvo — sem efeito, a foto fica como o bruto".into(),
                            }
                            .into(),
                            false,
                        ));
                        cx.emit(EventoDoEditor::Salva { foto, versao });
                        if e_fechar {
                            ed.liberada = true;
                            let _ = janela.update(cx, |_, window, _| window.remove_window());
                        }
                    }
                    Err(erro) => {
                        ed.aviso = Some((format!("Não foi possível salvar: {erro}").into(), true));
                    }
                }
                cx.notify();
            });
        }));
    }

    // ------------------------------------------------------------ fechar

    /// O X da janela, o `Cmd+W` e o botão "Fechar": com alterações, pergunta.
    pub fn pode_fechar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.liberada || !self.alterado() {
            return true;
        }
        self.perguntando = true;
        cx.notify();
        false
    }

    pub fn fechar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pode_fechar(window, cx) {
            window.remove_window();
        }
    }

    /// "Descartar" na pergunta: fecha sem salvar — o projeto salvo continua
    /// como estava.
    pub fn descartar_e_fechar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.liberada = true;
        self.perguntando = false;
        cx.notify();
        window.remove_window();
    }

    pub fn cancelar_fechar(&mut self, cx: &mut Context<Self>) {
        self.perguntando = false;
        cx.notify();
    }

    /// Um gesto do roteiro de depuração nesta janela — o do app inteiro
    /// (`editor …` depois de `tira editar N`) e o do editor avulso (`bin/editor.rs`).
    ///
    /// - `mouse apertar|arrastar|soltar|clicar fx fy` — evento **real** do
    ///   AppKit, numa fração da foto (ampliada, a fração da foto inteira);
    /// - `tecla <keyCode> [shift|ctrl|alt|cmd…]` — tecla física;
    /// - `camada nova|duplicar|excluir|subir|descer|escolher N|olho N|modo <chave>|renomear N <nome>|opacidade 0–100`;
    /// - `zoom encaixar|1:1|mais|menos|alternar|razao R [fx fy]|mover dx dy`;
    /// - `espaco segurar|soltar` — a mão, para o `mouse` arrastar a foto;
    /// - `foto <nome>` — a janela em PNG, em `pasta`;
    /// - `estado` — uma linha no stderr.
    pub fn seguir_o_roteiro(
        &mut self,
        gesto: &str,
        pasta: Option<&std::path::Path>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let partes: Vec<&str> = gesto.split_whitespace().collect();
        let numero = |i: usize| {
            partes
                .get(i)
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.0)
        };
        match partes.first().copied().unwrap_or_default() {
            "mouse" => {
                let tipo = partes.get(1).copied().unwrap_or("clicar");
                let Some(area) = self.area_na_janela() else {
                    eprintln!("[roteiro] editor: a foto ainda não está desenhada");
                    return;
                };
                let x = f(area.origin.x) + f(area.size.width) * numero(2);
                let y = f(area.origin.y) + f(area.size.height) * numero(3);
                let r = crate::depuracao::mouse_nativo(window, tipo, x, y, 0);
                eprintln!("[roteiro] editor mouse {tipo} ({x:.0}, {y:.0}): {r:?}");
            }
            "tecla" => {
                let codigo = numero(1) as u16;
                let mods = partes
                    .iter()
                    .skip(2)
                    .map(|n| match *n {
                        "shift" => 1 << 17,
                        "ctrl" => 1 << 18,
                        "alt" => 1 << 19,
                        "cmd" => 1 << 20,
                        _ => 0,
                    })
                    .fold(0, |a, b| a | b);
                let r = crate::depuracao::tecla_nativa(window, codigo, mods);
                eprintln!("[roteiro] editor tecla {codigo} mods={mods:#x}: {r:?}");
            }
            "camada" => {
                let indice = numero(2) as usize;
                match partes.get(1).copied().unwrap_or_default() {
                    "nova" => self.nova_camada(cx),
                    "duplicar" => self.duplicar_camada(cx),
                    "excluir" => self.excluir_camada(cx),
                    "mesclar" => self.mesclar_para_baixo(cx),
                    "subir" => self.mover_camada(1, cx),
                    "descer" => self.mover_camada(-1, cx),
                    "escolher" => self.escolher_camada(indice, cx),
                    "olho" => self.alternar_visibilidade_de(indice, cx),
                    "opacidade" => self.mover_opacidade_da_camada(numero(2) / 100.0, true, cx),
                    "modo" => match partes.get(2).and_then(|c| Modo::da_chave(c)) {
                        Some(modo) => self.mudar_modo(modo, cx),
                        None => {
                            eprintln!("[roteiro] editor: modo desconhecido {:?}", partes.get(2))
                        }
                    },
                    "renomear" => {
                        let nome = partes.get(3..).map(|p| p.join(" ")).unwrap_or_default();
                        self.renomear_camada(indice, &nome, cx);
                    }
                    outro => eprintln!("[roteiro] editor camada {outro}?"),
                }
            }
            "zoom" => {
                let ponto_da_fracao = |ed: &Self, fx: f32, fy: f32| {
                    ed.area_na_janela().map(|a| Ponto {
                        x: f(a.origin.x - ed.palco.origin.x) + f(a.size.width) * fx,
                        y: f(a.origin.y - ed.palco.origin.y) + f(a.size.height) * fy,
                    })
                };
                match partes.get(1).copied().unwrap_or_default() {
                    "encaixar" => self.ir_para_nivel(Nivel::Encaixar, None, cx),
                    "1:1" => self.ir_para_nivel(Nivel::Razao(1.0), None, cx),
                    "mais" => self.passo_de_zoom(1, cx),
                    "menos" => self.passo_de_zoom(-1, cx),
                    "alternar" => self.alternar_zoom(cx),
                    "razao" => {
                        let ponto = (partes.len() >= 5)
                            .then(|| ponto_da_fracao(self, numero(3), numero(4)))
                            .flatten();
                        self.ir_para_nivel(Nivel::Razao(numero(2)), ponto, cx);
                    }
                    "mover" => self.mover_a_foto(numero(2), numero(3), cx),
                    outro => eprintln!("[roteiro] editor zoom {outro}?"),
                }
            }
            "selecao" => {
                // Frações da foto inteira, como o `mouse`.
                let (largura, altura) = self.sessao().map_or((1.0, 1.0), |s| {
                    (s.base().width() as f32, s.base().height() as f32)
                });
                let ponto = |i: usize| (numero(i) * largura, numero(i + 1) * altura);
                let operacao = match partes.last().copied() {
                    Some("somar") => Operacao::Somar,
                    Some("subtrair") => Operacao::Subtrair,
                    _ => Operacao::Nova,
                };
                let caixa = || {
                    let (a, b) = (ponto(2), ponto(4));
                    Retangulo::novo(
                        a.0.min(b.0) as u32,
                        a.1.min(b.1) as u32,
                        (a.0 - b.0).abs() as u32,
                        (a.1 - b.1).abs() as u32,
                    )
                };
                let forma = match partes.get(1).copied().unwrap_or_default() {
                    "retangulo" => Some(Forma::Retangulo(caixa())),
                    "elipse" => Some(Forma::Elipse(caixa())),
                    "laco" => {
                        let numeros = partes
                            .iter()
                            .skip(2)
                            .filter_map(|v| v.parse::<f32>().ok())
                            .collect::<Vec<_>>();
                        Some(Forma::Laco(
                            numeros
                                .as_chunks::<2>()
                                .0
                                .iter()
                                .map(|[x, y]| (x * largura, y * altura))
                                .collect(),
                        ))
                    }
                    "tudo" => {
                        self.selecionar_tudo(cx);
                        None
                    }
                    "desmarcar" => {
                        self.desmarcar(cx);
                        None
                    }
                    "inverter" => {
                        self.inverter_selecao(cx);
                        None
                    }
                    "apagar" => {
                        self.apagar_selecao(cx);
                        None
                    }
                    "preencher" => {
                        self.preencher_selecao(cx);
                        None
                    }
                    "ferramenta" => {
                        match partes.get(2).copied() {
                            Some("retangulo") => self.usar_selecao(TipoDeSelecao::Retangulo, cx),
                            Some("elipse") => self.usar_selecao(TipoDeSelecao::Elipse, cx),
                            Some("laco") => self.usar_selecao(TipoDeSelecao::Laco, cx),
                            _ => self.usar(Ferramenta::Pincel, cx),
                        }
                        None
                    }
                    outro => {
                        eprintln!("[roteiro] editor selecao {outro}?");
                        None
                    }
                };
                if let Some(forma) = forma {
                    self.na_sessao(cx, |s| s.selecionar(&forma, operacao));
                }
            }
            "espaco" => match partes.get(1).copied() {
                Some("segurar") => self.espaco_apertado(cx),
                _ => self.espaco_solto(cx),
            },
            "foto" => {
                let Some(pasta) = pasta else {
                    eprintln!("[roteiro] editor foto: sem VLB_FOTOS");
                    return;
                };
                let destino = pasta.join(format!("{}.png", partes.get(1).unwrap_or(&"editor")));
                let r = crate::depuracao::fotografar(window, &destino);
                eprintln!("[foto] {}: {r:?}", destino.display());
            }
            "estado" => {
                let camadas = self.sessao().map_or_else(String::new, |s| {
                    s.documento()
                        .camadas
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            format!(
                                "{}{}:{}:{}:{:.0}%:{}tiles",
                                if i == s.ativa() { "*" } else { "" },
                                c.nome,
                                c.modo.chave(),
                                if c.visivel { "vis" } else { "oculta" },
                                c.opacidade * 100.0,
                                c.pixels.quantos()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" | ")
                });
                let selecao = self
                    .sessao()
                    .and_then(|s| s.selecao())
                    .map(|s| format!("{:?}", s.limites()));
                let lupa = self
                    .sessao()
                    .and_then(|s| s.lupa())
                    .map(|l| format!("fator {} {:?}", l.fator(), l.regiao()));
                eprintln!(
                    "[roteiro] editor: foto={} pronta={} falha={:?} alterado={} salvando={} aviso={:?} passos={} camadas=[{camadas}] zoom={} razao={:?} lupa={lupa:?} selecao={selecao:?} medidas={:?}",
                    self.foto.id,
                    self.pronta(),
                    self.falha(),
                    self.alterado(),
                    self.salvando,
                    self.aviso(),
                    self.sessao().map_or(0, |s| s.historico().posicao()),
                    zoom::rotulo_do_nivel(self.zoom.nivel),
                    self.razao_do_zoom(),
                    self.medidas,
                );
            }
            outro => eprintln!("[roteiro] gesto do editor desconhecido: {outro}"),
        }
    }

    /// 🧪 Um traço de `de` a `ate`, em pixels da foto — o que o ponteiro faz,
    /// sem precisar do palco medido.
    #[cfg(test)]
    pub fn tracar_para_teste(&mut self, de: (f32, f32), ate: (f32, f32), cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.pincel.dureza = 1.0;
            s.pincel.raio = 4.0;
            s.apertar(de.0, de.1);
            s.arrastar(ate.0, ate.1);
            s.soltar();
        }
        cx.notify();
    }

    // ------------------------------------------------------------- a lupa

    /// Pede, troca ou larga a lupa conforme o zoom de agora.
    ///
    /// 🔑 **Só um pedido por vez.** Enquanto a lupa monta, a vista inteira
    /// (ampliada, borrada) continua na tela; quando ela chega, a próxima
    /// passada confere se ainda serve e pede outra se o operador já andou.
    fn atualizar_a_lupa(&mut self, cx: &mut Context<Self>) {
        let Some((cena, v)) = self.vista_do_zoom() else {
            return;
        };
        let dpr = self.dpr;
        let Fase::Pronta(sessao) = &mut self.fase else {
            return;
        };
        // Quantos pixels da foto cabem num pixel do dispositivo.
        let por_pixel = 1.0 / (v.escala * dpr).max(1e-6);
        let fator = (por_pixel.floor() as u32).max(1);
        // 🔑 **No encaixe não há lupa** (a regra do `precisa_do_bruto` da
        // Revelação): com a foto inteira na tela, a lupa seria uma segunda
        // vista da foto toda, e cada pincelada pagaria as duas — medido no app
        // real, 21 ms por gesto numa foto de 14 MP.
        if fator >= sessao.vista().fator() || self.zoom.nivel == Nivel::Encaixar {
            if sessao.lupa().is_some() || sessao.lupa_a_caminho() {
                sessao.largar_lupa();
                self.ladrilhos_da_lupa.clear();
            }
            return;
        }
        let (largura, altura) = (cena.janela.largura, cena.janela.altura);
        let x0 = (-v.x / v.escala).clamp(0.0, largura);
        let y0 = (-v.y / v.escala).clamp(0.0, altura);
        let x1 = ((cena.area.largura - v.x) / v.escala).clamp(0.0, largura);
        let y1 = ((cena.area.altura - v.y) / v.escala).clamp(0.0, altura);
        let visivel = Retangulo::novo(
            x0.floor() as u32,
            y0.floor() as u32,
            (x1.ceil() - x0.floor()) as u32,
            (y1.ceil() - y0.floor()) as u32,
        );
        if visivel.vazio() {
            return;
        }
        if sessao
            .lupa()
            .is_some_and(|l| l.fator() == fator && contem(&l.regiao(), &visivel))
        {
            return;
        }
        if sessao.lupa_a_caminho() {
            return;
        }
        // Um oitavo de folga de cada lado: o arrasto curto não pede outra.
        let (mx, my) = (visivel.largura / 8, visivel.altura / 8);
        let pedido = Retangulo::novo(
            visivel.x.saturating_sub(mx),
            visivel.y.saturating_sub(my),
            visivel.largura + 2 * mx,
            visivel.altura + 2 * my,
        );
        let pedido: PedidoDeLupa = sessao.pedir_lupa(pedido, fator);
        let inicio = Instant::now();
        let trabalho = cx
            .background_executor()
            .spawn(async move { pedido.montar() });
        self._tarefa_da_lupa = Some(cx.spawn(async move |esta, cx| {
            let (id, lupa) = trabalho.await;
            let _ = esta.update(cx, |ed, cx| {
                if let Some(s) = ed.sessao_mut() {
                    if s.receber_lupa(id, lupa) {
                        ed.ladrilhos_da_lupa.clear();
                        ed.medidas.ultima_lupa = Some(inicio.elapsed());
                    }
                }
                cx.notify();
            });
        }));
    }

    // ------------------------------------------------------------ desenho

    /// Sobe para a GPU os ladrilhos que o último gesto sujou — da vista e da
    /// lupa.
    fn subir_os_ladrilhos(&mut self) {
        let Fase::Pronta(sessao) = &mut self.fase else {
            return;
        };
        let mut subidos = subir(sessao.vista_mut(), &mut self.ladrilhos);
        if let Some(lupa) = sessao.lupa_mut() {
            subidos += subir(lupa, &mut self.ladrilhos_da_lupa);
        }
        self.medidas.ladrilhos_no_quadro = subidos;
    }

    fn palco(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let fator_da_tela = window.scale_factor().max(1.0);
        let medidor = cx.entity();
        let ouvinte = cx.entity();
        let pintando = self.pintando || self.gesto_de_selecao.is_some();
        let com_a_mao = self.mao.is_some();
        let medida = canvas(
            move |bounds, window, cx| {
                medidor.update(cx, |ed, _cx| {
                    if ed.palco != bounds {
                        ed.palco = bounds;
                    }
                    ed.dpr = window.scale_factor().max(1.0);
                });
            },
            move |bounds, _prepaint, window, _cx| {
                // 🚨 **A pinça é ouvida na janela** (a lição da Revelação,
                // 27/09): o `on_pinch` do elemento exige o palco "sob o mouse",
                // e o GPUI desliga o hover depois de uma tecla.
                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |evento: &PinchEvent, fase, _window, cx| {
                        if fase.bubble() && bounds.contains(&evento.position) {
                            esta.update(cx, |ed, cx| ed.ao_pincar(evento, cx));
                        }
                    }
                });
                // 🔑 O arrasto é ouvido na janela: o traço (e a mão) continua
                // quando o ponteiro sai da foto, e o soltar fora dela fecha.
                if !pintando && !com_a_mao {
                    return;
                }
                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |evento: &MouseMoveEvent, fase, _window, cx| {
                        if fase.bubble() {
                            esta.update(cx, |ed, cx| {
                                if ed.mao.is_some() {
                                    ed.arrastar_com_a_mao(evento.position, cx);
                                } else {
                                    ed.mover(evento.position, cx);
                                }
                            });
                        }
                    }
                });
                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |_evento: &MouseUpEvent, fase, _window, cx| {
                        if fase.bubble() {
                            esta.update(cx, |ed, cx| ed.soltar(cx));
                        }
                    }
                });
            },
        )
        .absolute()
        .size_full();

        let mut palco = div()
            .id("palco-do-editor")
            .debug_selector(|| "palco-do-editor".into())
            .relative()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(tema::cores::poco())
            .child(medida);

        match &self.fase {
            Fase::Carregando => {
                palco = palco.child(aviso_centrado("Abrindo a foto em resolução cheia…", cx));
            }
            Fase::Falhou(erro) => {
                palco = palco.child(aviso_centrado(&format!("A foto não abriu: {erro}"), cx));
            }
            Fase::Pronta(sessao) => {
                if let Some((cena, v)) = self.vista_do_zoom() {
                    let tela = Tela {
                        v,
                        area: cena.area,
                        fator_da_tela,
                    };
                    palco = palco.children(tela.ladrilhos(sessao.vista(), &self.ladrilhos));
                    if let Some(lupa) = sessao.lupa() {
                        palco = palco.children(tela.ladrilhos(lupa, &self.ladrilhos_da_lupa));
                        if let Some(nitidos) = tela.pixels_nitidos(lupa) {
                            palco = palco.child(nitidos);
                        }
                    }
                    // O letreiro da seleção e a forma sendo desenhada.
                    if let Some((_, bordas)) = &self.bordas {
                        palco = palco.child(tela.letreiro(bordas.clone()));
                    }
                    if let Some(gesto) = &self.gesto_de_selecao {
                        palco = palco.child(tela.contorno(gesto.contorno()));
                    }
                    // O círculo do pincel, do tamanho que ele pinta.
                    let dentro = self.ponteiro.filter(|p| {
                        self.area_na_janela().is_some_and(|a| a.contains(p))
                            && self.palco.contains(p)
                    });
                    let com_pincel = self.selecionando.is_none();
                    if let (Some(ponteiro), None, true) = (dentro, self.espaco, com_pincel) {
                        let raio = sessao.pincel.raio * v.escala;
                        let centro = ponteiro - self.palco.origin;
                        palco = palco.child(
                            div()
                                .absolute()
                                .left(centro.x - px(raio))
                                .top(centro.y - px(raio))
                                .size(px(raio * 2.0))
                                .rounded_full()
                                .border_1()
                                .border_color(gpui_kit::white().opacity(0.85)),
                        );
                    }
                }
                let cursor = if self.mao.is_some() {
                    gpui_kit::CursorStyle::ClosedHand
                } else if self.espaco.is_some() {
                    gpui_kit::CursorStyle::OpenHand
                } else {
                    gpui_kit::CursorStyle::Crosshair
                };
                palco = palco
                    .cursor(cursor)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|ed, evento: &MouseDownEvent, window, cx| {
                            window.focus(&ed.foco, cx);
                            if ed.espaco.is_some() {
                                ed.pegar_com_a_mao(evento.position, cx);
                            } else if ed.selecionando.is_some() {
                                ed.comecar_selecao(evento.position, evento.modifiers, cx);
                            } else {
                                ed.apertar(evento.position, cx);
                            }
                        }),
                    )
                    // O botão do meio é a mão também, sem o Espaço.
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(|ed, evento: &MouseDownEvent, _w, cx| {
                            ed.pegar_com_a_mao(evento.position, cx);
                        }),
                    )
                    .on_scroll_wheel(
                        cx.listener(|ed, e: &ScrollWheelEvent, _w, cx| ed.ao_rolar(e, cx)),
                    )
                    .on_mouse_move(cx.listener(|ed, evento: &MouseMoveEvent, _w, cx| {
                        if !ed.pintando && ed.mao.is_none() && ed.gesto_de_selecao.is_none() {
                            ed.mover(evento.position, cx);
                        }
                    }));
            }
        }
        palco.into_any_element()
    }

    fn barra(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let pronta = self.pronta();
        let (pode_desfazer, pode_refazer) = self
            .sessao()
            .map(|s| (s.historico().pode_desfazer(), s.historico().pode_refazer()))
            .unwrap_or((false, false));
        let dica = |passo: Option<&editor_core::Comando>, verbo: &str| -> SharedString {
            match (self.sessao(), passo) {
                (Some(s), Some(p)) => format!("{verbo} {}", p.descricao(s.documento())).into(),
                _ => verbo.to_string().into(),
            }
        };
        let dica_desfazer = dica(
            self.sessao().and_then(|s| s.historico().a_desfazer()),
            "Desfazer",
        );
        let dica_refazer = dica(
            self.sessao().and_then(|s| s.historico().a_refazer()),
            "Refazer",
        );
        let rotulo_do_zoom = match self.zoom.nivel {
            Nivel::Encaixar => self
                .razao_do_zoom()
                .map(zoom::porcentagem)
                .unwrap_or_default(),
            nivel => zoom::rotulo_do_nivel(nivel),
        };
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(48.))
            .px(px(12.))
            .border_b_1()
            .border_color(tema.border)
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .child(format!("Editar — {}", self.foto.nome)),
            )
            .when(self.alterado(), |barra| {
                barra.child(
                    div()
                        .id("editor-alterado")
                        .debug_selector(|| "editor-alterado".into())
                        .text_xs()
                        .text_color(tema::cores::quente())
                        .child("• Alterações não salvas"),
                )
            })
            .when_some(self.aviso.clone(), |barra, (texto, erro)| {
                barra.child(
                    div()
                        .text_xs()
                        .text_color(if erro {
                            tema.danger
                        } else {
                            tema.muted_foreground
                        })
                        .child(texto),
                )
            })
            .child(div().flex_1())
            .child(
                crate::estilo::botao_fantasma_pequeno("editor-encaixar", cx)
                    .debug_selector(|| "editor-encaixar".into())
                    .label("Encaixar")
                    .tooltip("Encaixar a foto na janela (⌘0)")
                    .disabled(!pronta)
                    .on_click(
                        cx.listener(|ed, _, _, cx| ed.ir_para_nivel(Nivel::Encaixar, None, cx)),
                    ),
            )
            .child(
                crate::estilo::botao_fantasma_pequeno("editor-1-1", cx)
                    .debug_selector(|| "editor-1-1".into())
                    .label("1:1")
                    .tooltip("Um pixel da foto num pixel da tela (⌘⌥0)")
                    .disabled(!pronta)
                    .on_click(
                        cx.listener(|ed, _, _, cx| ed.ir_para_nivel(Nivel::Razao(1.0), None, cx)),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "editor-zoom".into())
                    .w(px(56.))
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(rotulo_do_zoom),
            )
            .child(
                crate::estilo::botao_fantasma("editor-desfazer", cx)
                    .debug_selector(|| "editor-desfazer".into())
                    .child("↶")
                    .tooltip(dica_desfazer)
                    .disabled(!pode_desfazer)
                    .on_click(cx.listener(|ed, _, _, cx| ed.desfazer(cx))),
            )
            .child(
                crate::estilo::botao_fantasma("editor-refazer", cx)
                    .debug_selector(|| "editor-refazer".into())
                    .child("↷")
                    .tooltip(dica_refazer)
                    .disabled(!pode_refazer)
                    .on_click(cx.listener(|ed, _, _, cx| ed.refazer(cx))),
            )
            .child(
                crate::estilo::botao_primario("editor-salvar", cx)
                    .debug_selector(|| "editor-salvar".into())
                    .child(if self.salvando {
                        "Salvando…"
                    } else {
                        "Salvar"
                    })
                    .disabled(!pronta || self.salvando)
                    .on_click(cx.listener(|ed, _, window, cx| ed.salvar(false, window, cx))),
            )
            .child(
                crate::estilo::botao_contorno("editor-fechar", cx)
                    .debug_selector(|| "editor-fechar".into())
                    .child("Fechar")
                    .on_click(cx.listener(|ed, _, window, cx| ed.fechar(window, cx))),
            )
    }

    fn painel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let ferramenta = self.ferramenta().filter(|_| self.selecionando.is_none());
        let selecionando = self.selecionando;
        let cor_atual = self.sessao().map(|s| s.pincel.cor);
        let rotulo = |texto: &'static str| {
            div()
                .text_xs()
                .text_color(tema.muted_foreground)
                .child(texto)
        };
        let botao_ferramenta =
            |id: &'static str, nome: &'static str, qual: Ferramenta, cx: &mut Context<Self>| {
                let ativa = ferramenta == Some(qual);
                let botao = if ativa {
                    crate::estilo::botao_primario(id, cx)
                } else {
                    crate::estilo::botao_contorno(id, cx)
                };
                botao
                    .debug_selector(move || id.into())
                    .child(nome)
                    .on_click(cx.listener(move |ed, _, _, cx| ed.usar(qual, cx)))
            };
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .w(px(260.))
            .h_full()
            .p(px(12.))
            .border_l_1()
            .border_color(tema.border)
            .child(rotulo("Ferramenta"))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .child(botao_ferramenta(
                        "editor-pincel",
                        "Pincel (B)",
                        Ferramenta::Pincel,
                        cx,
                    ))
                    .child(botao_ferramenta(
                        "editor-borracha",
                        "Borracha (E)",
                        Ferramenta::Borracha,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .children(
                        [
                            (TipoDeSelecao::Retangulo, Icone::Square, "editor-selecao-retangulo", "Seleção retangular (M) — ⇧ soma, ⌥ tira"),
                            (TipoDeSelecao::Elipse, Icone::CircleDashed, "editor-selecao-elipse", "Seleção elíptica (⇧M) — ⇧ soma, ⌥ tira"),
                            (TipoDeSelecao::Laco, Icone::Lasso, "editor-selecao-laco", "Laço (L) — ⇧ soma, ⌥ tira"),
                        ]
                        .into_iter()
                        .map(|(tipo, icone, id, dica)| {
                            let botao = crate::estilo::botao_icone_padrao(id, icone)
                                .debug_selector(move || id.into())
                                .tooltip(dica)
                                .on_click(cx.listener(move |ed, _, _, cx| ed.usar_selecao(tipo, cx)));
                            if selecionando == Some(tipo) {
                                botao.primary()
                            } else {
                                botao
                            }
                        }),
                    )
                    .child(div().flex_1())
                    .child(
                        crate::estilo::botao_fantasma_pequeno("editor-desmarcar", cx)
                            .debug_selector(|| "editor-desmarcar".into())
                            .label("Desmarcar")
                            .tooltip("Tirar a seleção (⌘D) · ⌘A tudo · ⇧⌘I inverter · Delete apaga · ⌥Delete preenche")
                            .disabled(self.sessao().and_then(Sessao::selecao).is_none())
                            .on_click(cx.listener(|ed, _, _, cx| ed.desmarcar(cx))),
                    ),
            )
            .child(rotulo("Tamanho  [  ]"))
            .child(div().h(px(20.)).child(crate::estilo::slider(&self.tamanho)))
            .child(rotulo("Dureza"))
            .child(div().h(px(20.)).child(crate::estilo::slider(&self.dureza)))
            .child(rotulo("Opacidade do pincel"))
            .child(
                div()
                    .h(px(20.))
                    .child(crate::estilo::slider(&self.opacidade)),
            )
            .child(rotulo("Cor"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .children(AMOSTRAS.iter().enumerate().map(|(i, cor)| {
                        let cor = *cor;
                        let escolhida = cor_atual == Some(cor);
                        div()
                            .id(("editor-cor", i))
                            .size(px(22.))
                            .rounded(crate::tema::canto(4.))
                            .border_2()
                            .border_color(if escolhida { tema.ring } else { tema.border })
                            .bg(gpui_kit::rgb(
                                (cor[0] as u32) << 16 | (cor[1] as u32) << 8 | cor[2] as u32,
                            ))
                            .cursor_pointer()
                            .on_click(cx.listener(move |ed, _, _, cx| ed.escolher_cor(cor, cx)))
                    })),
            )
            .child(div().h(px(1.)).bg(tema.border))
            .child(self.painel_de_camadas(cx))
    }

    /// O painel Camadas do Photoshop: o modo e a opacidade da escolhida, a
    /// pilha de cima para baixo, e os botões embaixo.
    fn painel_de_camadas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let (camadas, ativa, pode_desfazer_alguma) = match self.sessao() {
            Some(s) => (
                s.documento()
                    .camadas
                    .iter()
                    .map(|c| (c.nome.clone(), c.visivel, c.modo))
                    .collect::<Vec<_>>(),
                s.ativa(),
                true,
            ),
            None => (Vec::new(), 0, false),
        };
        let quantas = camadas.len();
        let renomeando = self.renomeando.clone();
        let miniaturas = self.miniaturas.clone();
        let linhas = camadas
            .into_iter()
            .enumerate()
            .rev()
            .map(|(i, (nome, visivel, modo))| {
                let escolhida = i == ativa;
                let id_do_olho: SharedString = format!("editor-olho-{i}").into();
                let olho = crate::estilo::botao_icone_pequeno(
                    id_do_olho.clone(),
                    if visivel { Icone::Eye } else { Icone::EyeOff },
                )
                .debug_selector(move || id_do_olho.to_string())
                .tooltip(if visivel {
                    "Esconder a camada (H)"
                } else {
                    "Mostrar a camada (H)"
                })
                .on_click(cx.listener(move |ed, _, _, cx| ed.alternar_visibilidade_de(i, cx)));
                let texto: AnyElement = match &renomeando {
                    Some((j, campo)) if *j == i => div()
                        .flex_1()
                        .child(crate::estilo::campo_pequeno(Input::new(campo).xsmall()))
                        .into_any_element(),
                    _ => div()
                        .flex_1()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_sm()
                        .when(!visivel, |d| d.text_color(tema.muted_foreground))
                        .child(nome)
                        .into_any_element(),
                };
                div()
                    .id(("editor-camada", i))
                    .debug_selector(move || format!("editor-camada-{i}"))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(crate::tema::canto(4.))
                    .cursor_pointer()
                    .when(escolhida, |d| {
                        d.bg(tema.accent).text_color(tema.accent_foreground)
                    })
                    .when(!escolhida, |d| d.hover(|d| d.bg(tema.muted)))
                    // O olho não escolhe a camada, como no Photoshop.
                    .child(
                        div()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(olho),
                    )
                    .when_some(miniaturas.get(i).cloned(), |d, m| {
                        d.child(
                            img(m)
                                .object_fit(ObjectFit::Contain)
                                .w(px(LADO_DA_MINIATURA as f32 * 0.75))
                                .h(px(LADO_DA_MINIATURA as f32 * 0.75))
                                .flex_none(),
                        )
                    })
                    .child(texto)
                    .when(modo != Modo::Normal, |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(tema.muted_foreground)
                                .child(modo.nome()),
                        )
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ed, evento: &MouseDownEvent, window, cx| {
                            if evento.click_count >= 2 {
                                ed.comecar_a_renomear(i, window, cx);
                            } else {
                                if ed.renomeando.as_ref().is_some_and(|(j, _)| *j != i) {
                                    ed.terminar_de_renomear(true, window, cx);
                                }
                                ed.escolher_camada(i, cx);
                                if ed.renomeando.is_none() {
                                    window.focus(&ed.foco, cx);
                                }
                            }
                        }),
                    )
            });
        let botao = |id: &'static str, icone: Icone, dica: &'static str, ligado: bool| {
            crate::estilo::botao_icone_pequeno(id, icone)
                .debug_selector(move || id.into())
                .tooltip(dica)
                .disabled(!ligado)
        };
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .flex_1()
            .min_h(px(0.))
            .child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child("Camadas"),
            )
            .child(
                div()
                    .debug_selector(|| "editor-modo".into())
                    .child(crate::estilo::campo_pequeno(
                        Select::new(&self.modo)
                            .xsmall()
                            .disabled(!pode_desfazer_alguma),
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child("Opacidade da camada"),
            )
            .child(
                div()
                    .h(px(20.))
                    .child(crate::estilo::slider(&self.opacidade_da_camada)),
            )
            .child(
                div()
                    .id("editor-camadas")
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(linhas),
            )
            .child(
                div()
                    .flex()
                    .gap(px(4.))
                    .child(
                        botao(
                            "editor-camada-nova",
                            Icone::Plus,
                            "Nova camada (⇧⌘N)",
                            pode_desfazer_alguma,
                        )
                        .on_click(cx.listener(|ed, _, _, cx| ed.nova_camada(cx))),
                    )
                    .child(
                        botao(
                            "editor-camada-duplicar",
                            Icone::Copy,
                            "Duplicar a camada (⌘J)",
                            pode_desfazer_alguma,
                        )
                        .on_click(cx.listener(|ed, _, _, cx| ed.duplicar_camada(cx))),
                    )
                    .child(
                        botao(
                            "editor-camada-subir",
                            Icone::ArrowUp,
                            "Subir a camada (⌘])",
                            ativa + 1 < quantas,
                        )
                        .on_click(cx.listener(|ed, _, _, cx| ed.mover_camada(1, cx))),
                    )
                    .child(
                        botao(
                            "editor-camada-descer",
                            Icone::ArrowDown,
                            "Descer a camada (⌘[)",
                            ativa > 0 && quantas > 1,
                        )
                        .on_click(cx.listener(|ed, _, _, cx| ed.mover_camada(-1, cx))),
                    )
                    .child(
                        botao(
                            "editor-camada-mesclar",
                            Icone::Layers,
                            "Mesclar para baixo (⌘E)",
                            ativa > 0 && quantas > 1,
                        )
                        .on_click(cx.listener(|ed, _, _, cx| ed.mesclar_para_baixo(cx))),
                    )
                    .child(div().flex_1())
                    .child(
                        botao(
                            "editor-camada-excluir",
                            Icone::Trash2,
                            "Excluir a camada",
                            quantas > 1,
                        )
                        .on_click(cx.listener(|ed, _, _, cx| ed.excluir_camada(cx))),
                    ),
            )
    }

    fn pergunta_de_fechar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        Some(
            gpui_kit::component::v_flex()
                .gap(px(16.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(crate::estilo::cabecalho_do_dialogo(
                    "Salvar a edição antes de fechar?",
                    SharedString::from(format!(
                        "{} tem alterações que ainda não foram salvas. Descartar fecha a janela e mantém a última versão salva.",
                        self.foto.nome
                    )),
                    None,
                    cx,
                ))
                .child(
                    crate::estilo::rodape_do_dialogo()
                        .child(
                            crate::estilo::botao_contorno("editor-cancelar-fechar", cx)
                                .debug_selector(|| "editor-cancelar-fechar".into())
                                .child("Cancelar")
                                .on_click(cx.listener(|ed, _, _, cx| ed.cancelar_fechar(cx))),
                        )
                        .child(
                            crate::estilo::botao_perigo("editor-descartar", cx)
                                .debug_selector(|| "editor-descartar".into())
                                .child("Descartar")
                                .on_click(cx.listener(|ed, _, window, cx| ed.descartar_e_fechar(window, cx))),
                        )
                        .child(
                            crate::estilo::botao_primario("editor-salvar-e-fechar", cx)
                                .debug_selector(|| "editor-salvar-e-fechar".into())
                                .child("Salvar")
                                .on_click(cx.listener(|ed, _, window, cx| {
                                    ed.perguntando = false;
                                    ed.salvar(true, window, cx);
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}

/// Os ladrilhos sujos de uma vista, para a GPU. Devolve quantos subiram.
fn subir(vista: &mut VistaDoEditor, imagens: &mut HashMap<(u32, u32), Arc<RenderImage>>) -> usize {
    let sujos = vista.levar_os_sujos();
    for &ladrilho in &sujos {
        let (l, a, bytes) = vista.ladrilho_bgra(ladrilho);
        if let Some(imagem) = crate::imagem::de_bgra(l, a, bytes) {
            imagens.insert(ladrilho, imagem);
        }
    }
    sujos.len()
}

/// O lado maior da miniatura da camada, em pixels.
const LADO_DA_MINIATURA: u32 = 48;

/// A miniatura RGBA sobre o xadrez do transparente, em BGRA opaco.
fn sobre_xadrez(rgba: &[u8], largura: u32) -> Vec<u8> {
    let mut saida = Vec::with_capacity(rgba.len());
    for (k, p) in rgba.as_chunks::<4>().0.iter().enumerate() {
        let (x, y) = (k as u32 % largura, k as u32 / largura);
        let fundo: f32 = if (x / 6 + y / 6) % 2 == 0 {
            204.0
        } else {
            153.0
        };
        let a = p[3] as f32 / 255.0;
        let c = |v: u8| (v as f32 * a + fundo * (1.0 - a)).round() as u8;
        saida.extend_from_slice(&[c(p[2]), c(p[1]), c(p[0]), 255]);
    }
    saida
}

/// `dentro` cabe inteiro em `fora`.
fn contem(fora: &Retangulo, dentro: &Retangulo) -> bool {
    dentro.x >= fora.x
        && dentro.y >= fora.y
        && dentro.direita() <= fora.direita()
        && dentro.baixo() <= fora.baixo()
}

/// Onde a foto está no palco, para desenhar as vistas.
struct Tela {
    v: zoom::Vista,
    area: MedidasDaCena,
    fator_da_tela: f32,
}

impl Tela {
    /// O letreiro: a borda da seleção em preto, com traços brancos de 4 pontos
    /// por cima — lê sobre qualquer foto.
    fn letreiro(&self, bordas: Arc<Vec<Segmento>>) -> AnyElement {
        let (vx, vy, esc) = (self.v.x, self.v.y, self.v.escala);
        let area = self.area;
        canvas(
            |_, _, _| {},
            move |limites, _, window, _| {
                let (ox, oy) = (f(limites.origin.x), f(limites.origin.y));
                let mut preto = PathBuilder::stroke(px(1.5));
                let mut branco = PathBuilder::stroke(px(1.));
                let mut algum = false;
                for &(x0, y0, x1, y1) in bordas.iter() {
                    let (ax, ay) = (vx + x0 as f32 * esc, vy + y0 as f32 * esc);
                    let (bx, by) = (vx + x1 as f32 * esc, vy + y1 as f32 * esc);
                    if ax.max(bx) < 0.0
                        || ay.max(by) < 0.0
                        || ax.min(bx) > area.largura
                        || ay.min(by) > area.altura
                    {
                        continue;
                    }
                    // Só o pedaço dentro do palco (com folga): ampliada, uma
                    // borda pode ter milhares de pontos de comprimento.
                    let (ax, bx) = (
                        ax.clamp(-2.0, area.largura + 2.0),
                        bx.clamp(-2.0, area.largura + 2.0),
                    );
                    let (ay, by) = (
                        ay.clamp(-2.0, area.altura + 2.0),
                        by.clamp(-2.0, area.altura + 2.0),
                    );
                    algum = true;
                    preto.move_to(gpui_kit::point(px(ox + ax), px(oy + ay)));
                    preto.line_to(gpui_kit::point(px(ox + bx), px(oy + by)));
                    let comprimento = (bx - ax).abs() + (by - ay).abs();
                    let (dx, dy) = ((bx - ax).signum(), (by - ay).signum());
                    // Os traços alinhados à grade da tela, para os vizinhos
                    // emendarem no mesmo compasso.
                    let inicio = if dx != 0.0 { ax.min(bx) } else { ay.min(by) };
                    let mut t = (inicio / 8.0).ceil() * 8.0 - inicio;
                    let (sx, sy) = (ax.min(bx), ay.min(by));
                    while t < comprimento {
                        let fim = (t + 4.0).min(comprimento);
                        let (px0, py0) = (sx + dx.abs() * t, sy + dy.abs() * t);
                        let (px1, py1) = (sx + dx.abs() * fim, sy + dy.abs() * fim);
                        branco.move_to(gpui_kit::point(px(ox + px0), px(oy + py0)));
                        branco.line_to(gpui_kit::point(px(ox + px1), px(oy + py1)));
                        t += 8.0;
                    }
                }
                if !algum {
                    return;
                }
                if let Ok(caminho) = preto.build() {
                    window.paint_path(caminho, gpui_kit::black());
                }
                if let Ok(caminho) = branco.build() {
                    window.paint_path(caminho, gpui_kit::white());
                }
            },
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    /// A forma sendo desenhada (pixels da foto), fechada.
    fn contorno(&self, pontos: Vec<(f32, f32)>) -> AnyElement {
        let (vx, vy, esc) = (self.v.x, self.v.y, self.v.escala);
        canvas(
            |_, _, _| {},
            move |limites, _, window, _| {
                let (ox, oy) = (f(limites.origin.x), f(limites.origin.y));
                let ponto = |p: &(f32, f32)| {
                    gpui_kit::point(px(ox + vx + p.0 * esc), px(oy + vy + p.1 * esc))
                };
                for (largura, cor) in [(2.0, gpui_kit::black()), (1.0, gpui_kit::white())] {
                    let mut traco = PathBuilder::stroke(px(largura));
                    let Some(primeiro) = pontos.first() else {
                        return;
                    };
                    traco.move_to(ponto(primeiro));
                    for p in &pontos[1..] {
                        traco.line_to(ponto(p));
                    }
                    if let Ok(caminho) = traco.build() {
                        window.paint_path(caminho, cor);
                    }
                }
            },
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    /// Os ladrilhos de uma vista (a inteira ou a lupa) que caem no palco.
    fn ladrilhos(
        &self,
        vista: &VistaDoEditor,
        imagens: &HashMap<(u32, u32), Arc<RenderImage>>,
    ) -> Vec<AnyElement> {
        let regiao = vista.regiao();
        let escala = self.v.escala * vista.fator() as f32;
        let dpr = self.fator_da_tela;
        // 🚨 **Alinhado aos pixels da tela, e sobrando um.** Com a posição
        // fracionária, a borda de dois ladrilhos vizinhos caía no meio de um
        // pixel e abria uma fresta escura (visto no app real, 27/set/2026).
        // Início para baixo, fim para cima e um pixel do dispositivo a mais: o
        // vizinho cobre.
        let alinhar = |v: f32, cima: bool| {
            let d = v * dpr;
            (if cima { d.ceil() } else { d.floor() }) / dpr
        };
        let ox = self.v.x + regiao.x as f32 * self.v.escala;
        let oy = self.v.y + regiao.y as f32 * self.v.escala;
        let mut saida = Vec::new();
        for ly in 0..vista.linhas() {
            for lx in 0..vista.colunas() {
                let Some(imagem) = imagens.get(&(lx, ly)) else {
                    continue;
                };
                let r = vista.retangulo_do_ladrilho((lx, ly));
                let x0 = ox + r.x as f32 * escala;
                let y0 = oy + r.y as f32 * escala;
                let (x1, y1) = (
                    x0 + r.largura as f32 * escala,
                    y0 + r.altura as f32 * escala,
                );
                // Fora do palco não desenha: ampliada, quase toda a vista
                // inteira fica de fora.
                if x1 < 0.0 || y1 < 0.0 || x0 > self.area.largura || y0 > self.area.altura {
                    continue;
                }
                let (esq, topo) = (alinhar(x0, false), alinhar(y0, false));
                let dir = alinhar(x1, true) + 1.0 / dpr;
                let baixo = alinhar(y1, true) + 1.0 / dpr;
                saida.push(
                    img(imagem.clone())
                        .object_fit(ObjectFit::Fill)
                        .absolute()
                        .left(px(esq))
                        .top(px(topo))
                        .w(px(dir - esq))
                        .h(px(baixo - topo))
                        .into_any_element(),
                );
            }
        }
        saida
    }

    /// A partir de 8 pixels do dispositivo por pixel da foto, cada pixel da
    /// lupa vira um quadrado nítido por cima da textura (que o GPU amplia
    /// borrando) — o `pixels_nitidos` da Revelação, que dá para contar os
    /// vizinhos de um pixel na hora do retoque fino.
    fn pixels_nitidos(&self, lupa: &VistaDoEditor) -> Option<AnyElement> {
        if lupa.fator() != 1
            || self.v.escala * self.fator_da_tela < zoom::PIXELS_NITIDOS_A_PARTIR_DE
        {
            return None;
        }
        let regiao = lupa.regiao();
        let esc = self.v.escala;
        let x0 = ((-self.v.x / esc).floor().max(0.0) as u32).max(regiao.x);
        let y0 = ((-self.v.y / esc).floor().max(0.0) as u32).max(regiao.y);
        let x1 =
            (((self.area.largura - self.v.x) / esc).ceil().max(0.0) as u32).min(regiao.direita());
        let y1 = (((self.area.altura - self.v.y) / esc).ceil().max(0.0) as u32).min(regiao.baixo());
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let imagem = lupa.imagem();
        let mut pixels = Vec::with_capacity(((x1 - x0) * (y1 - y0)) as usize);
        for y in y0..y1 {
            for x in x0..x1 {
                pixels.push(imagem.get_pixel(x - regiao.x, y - regiao.y).0);
            }
        }
        let (vx, vy, dpr) = (self.v.x, self.v.y, self.fator_da_tela);
        let largura = (x1 - x0) as usize;
        Some(
            canvas(
                |_, _, _| {},
                move |limites, _, window, _| {
                    let borda =
                        |inicio: f32, n: u32| ((inicio + n as f32 * esc) * dpr).round() / dpr;
                    let (ox, oy) = (f(limites.origin.x), f(limites.origin.y));
                    for (k, p) in pixels.iter().enumerate() {
                        let (x, y) = (x0 + (k % largura) as u32, y0 + (k / largura) as u32);
                        let (xa, xb) = (borda(vx, x), borda(vx, x + 1));
                        let (ya, yb) = (borda(vy, y), borda(vy, y + 1));
                        window.paint_quad(gpui_kit::fill(
                            Bounds::new(
                                gpui_kit::point(px(ox + xa), px(oy + ya)),
                                gpui_kit::size(px(xb - xa), px(yb - ya)),
                            ),
                            gpui_kit::Rgba {
                                r: p[0] as f32 / 255.,
                                g: p[1] as f32 / 255.,
                                b: p[2] as f32 / 255.,
                                a: 1.,
                            },
                        ));
                    }
                },
            )
            .absolute()
            .inset_0()
            .into_any_element(),
        )
    }
}

fn aviso_centrado(texto: &str, cx: &mut Context<EditorDeFoto>) -> impl IntoElement {
    div()
        .absolute()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(texto.to_string())
}

impl Focusable for EditorDeFoto {
    fn focus_handle(&self, _cx: &gpui_kit::App) -> FocusHandle {
        self.foco.clone()
    }
}

impl Render for EditorDeFoto {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 🚨 A tecla solta fora da janela nunca chega: sem foco, larga tudo.
        if !window.is_window_active() && (self.espaco.is_some() || self.z_desde.is_some()) {
            self.espaco = None;
            self.z_desde = None;
        }
        self.atualizar_a_lupa(cx);
        self.subir_os_ladrilhos();
        self.atualizar_as_bordas();
        self.atualizar_as_miniaturas();
        window.set_window_title(&self.titulo());
        // O slider e o modo acompanham a camada escolhida, o desfazer e a
        // reabertura.
        let opacidade = self.opacidade_da_camada() * 100.0;
        if (self.opacidade_da_camada.read(cx).value().start() - opacidade).abs() > 0.5 {
            self.opacidade_da_camada
                .update(cx, |s, cx| s.set_value(opacidade, window, cx));
        }
        let modo = self.sessao().map(|s| s.camada_ativa().modo);
        if modo.is_some() && modo != self.modo_mostrado {
            self.modo_mostrado = modo;
            let chave = modo.unwrap_or_default().chave().to_string();
            self.modo
                .update(cx, |s, cx| s.set_selected_value(&chave, window, cx));
        }
        let pergunta = {
            let quer = self.perguntando;
            crate::dialogo::desenhar(
                self,
                quer,
                crate::dialogo::Jeito::alerta(480.),
                Self::pergunta_de_fechar,
                |ed, _, cx| ed.cancelar_fechar(cx),
                window,
                cx,
            )
        };
        let tema = cx.theme().clone();
        div()
            .id("editor-de-foto")
            .key_context(CONTEXTO)
            .track_focus(&self.foco)
            .size_full()
            .flex()
            .flex_col()
            .bg(tema.background)
            .text_color(tema.foreground)
            .on_key_up(
                cx.listener(|ed, evento: &KeyUpEvent, _w, cx| ed.ao_soltar_tecla(evento, cx)),
            )
            .on_action(cx.listener(|ed, _: &DesfazerNoEditor, _, cx| ed.desfazer(cx)))
            .on_action(cx.listener(|ed, _: &RefazerNoEditor, _, cx| ed.refazer(cx)))
            .on_action(
                cx.listener(|ed, _: &SalvarNoEditor, window, cx| ed.salvar(false, window, cx)),
            )
            .on_action(cx.listener(|ed, _: &FecharEditor, window, cx| ed.fechar(window, cx)))
            .on_action(cx.listener(|ed, _: &UsarPincel, _, cx| ed.usar(Ferramenta::Pincel, cx)))
            .on_action(cx.listener(|ed, _: &UsarBorracha, _, cx| ed.usar(Ferramenta::Borracha, cx)))
            .on_action(cx.listener(|ed, _: &PincelMenor, window, cx| {
                ed.mudar_tamanho(1.0 / 1.25, window, cx)
            }))
            .on_action(
                cx.listener(|ed, _: &PincelMaior, window, cx| ed.mudar_tamanho(1.25, window, cx)),
            )
            .on_action(cx.listener(|ed, _: &AlternarCamada, _, cx| ed.alternar_visibilidade(cx)))
            .on_action(cx.listener(|ed, _: &NovaCamada, _, cx| ed.nova_camada(cx)))
            .on_action(cx.listener(|ed, _: &DuplicarCamada, _, cx| ed.duplicar_camada(cx)))
            .on_action(cx.listener(|ed, _: &SubirCamada, _, cx| ed.mover_camada(1, cx)))
            .on_action(cx.listener(|ed, _: &DescerCamada, _, cx| ed.mover_camada(-1, cx)))
            .on_action(cx.listener(|ed, _: &CamadaDeCima, _, cx| ed.escolher_vizinha(1, cx)))
            .on_action(cx.listener(|ed, _: &CamadaDeBaixo, _, cx| ed.escolher_vizinha(-1, cx)))
            .on_action(cx.listener(|ed, _: &Aproximar, _, cx| ed.passo_de_zoom(1, cx)))
            .on_action(cx.listener(|ed, _: &Afastar, _, cx| ed.passo_de_zoom(-1, cx)))
            .on_action(
                cx.listener(|ed, _: &Encaixar, _, cx| ed.ir_para_nivel(Nivel::Encaixar, None, cx)),
            )
            .on_action(cx.listener(|ed, _: &UmPorUm, _, cx| {
                let ponto = ed.ponteiro.map(|p| ed.ponto_no_palco(p));
                ed.ir_para_nivel(Nivel::Razao(1.0), ponto, cx)
            }))
            .on_action(cx.listener(|ed, _: &AlternarZoom, _, cx| ed.z_apertado(cx)))
            .on_action(cx.listener(|ed, _: &SegurarAMao, _, cx| ed.espaco_apertado(cx)))
            .on_action(cx.listener(|ed, _: &SelecaoRetangular, _, cx| {
                ed.usar_selecao(TipoDeSelecao::Retangulo, cx)
            }))
            .on_action(cx.listener(|ed, _: &SelecaoEliptica, _, cx| {
                ed.usar_selecao(TipoDeSelecao::Elipse, cx)
            }))
            .on_action(
                cx.listener(|ed, _: &SelecaoLaco, _, cx| ed.usar_selecao(TipoDeSelecao::Laco, cx)),
            )
            .on_action(cx.listener(|ed, _: &SelecionarTudo, _, cx| ed.selecionar_tudo(cx)))
            .on_action(cx.listener(|ed, _: &Desmarcar, _, cx| ed.desmarcar(cx)))
            .on_action(cx.listener(|ed, _: &InverterSelecao, _, cx| ed.inverter_selecao(cx)))
            .on_action(cx.listener(|ed, _: &ApagarSelecao, _, cx| ed.apagar_selecao(cx)))
            .on_action(cx.listener(|ed, _: &PreencherSelecao, _, cx| ed.preencher_selecao(cx)))
            .on_action(cx.listener(|ed, _: &MesclarParaBaixo, _, cx| ed.mesclar_para_baixo(cx)))
            .child(self.barra(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(self.palco(window, cx))
                    .child(self.painel(cx)),
            )
            .children(pergunta)
    }
}
