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
use editor_core::Ajuste;
use editor_core::{
    BaseRef, Documento, Ferramenta, Forma, Historico, Modo, Operacao, Retangulo, Sessao,
    Transformacao, VersaoEditada,
};
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::DropdownMenu as _;
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
    Afastar, AlternarCamada, AlternarZoom, ApagarSelecao, AplicarTransformacao, Aproximar,
    CamadaDeBaixo, CamadaDeCima, CamadaViaRecorte, CancelarTransformacao, DescerCamada,
    DesfazerNoEditor, Desmarcar, DuplicarCamada, Encaixar, FecharEditor, InverterSelecao,
    MesclarParaBaixo, NovaCamada, PincelMaior, PincelMenor, PreencherPeloConteudo,
    PreencherSelecao, RefazerNoEditor, SalvarNoEditor, SegurarAMao, SelecaoEliptica, SelecaoLaco,
    SelecaoRetangular, SelecionarTudo, SubirCamada, TransformacaoLivre, UmPorUm, UsarBorracha,
    UsarCarimbo, UsarContaGotas, UsarCorrecao, UsarMover, UsarPincel, CONTEXTO,
};
use super::{CoresPadrao, DifundirSelecao, TrocarCores, UsarVarinha};
use super::{
    UsarDegrade, UsarDesfoque, UsarLata, UsarNitidez, UsarSubexposicao, UsarSuperexposicao,
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
    /// A última borda da seleção montada (o letreiro).
    pub ultima_borda: Option<Duration>,
    /// O último preenchimento por conteúdo, do pedido ao remendo pronto.
    pub ultimo_preenchimento: Option<Duration>,
}

/// Um segmento da borda da seleção, em pixels da foto: `(x0, y0, x1, y1)`.
type Segmento = (u32, u32, u32, u32);

/// De que versão da sessão e de que lupa (região, fator) a borda é.
type ChaveDasBordas = (u64, Option<(Retangulo, u32)>);

/// As ferramentas que não pintam nem selecionam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Auxiliar {
    /// I — a cor da foto vira a cor do pincel.
    ContaGotas,
    /// V — arrasta o conteúdo da camada escolhida.
    Mover,
    /// J — o pincel de correção para manchas: pinta por cima, e ao soltar a
    /// área é refeita pelo que está em volta.
    Correcao,
    /// A mão da barra: arrastar move a foto ampliada (o Espaço segurado, sem
    /// segurar nada).
    Mao,
    /// A lupa da barra: clicar amplia em torno do ponto; ⌥ + clique afasta.
    Zoom,
    /// G — o degradê: arrastar do começo ao fim (⇧ prende em 45°).
    Degrade,
    /// ⇧G — a lata de tinta: um clique pinta a área parecida em volta.
    Lata,
    /// W — a varinha mágica: um clique seleciona a cor parecida (⇧ soma, ⌥
    /// tira).
    Varinha,
}

/// O buraco de um preenchimento por conteúdo.
#[derive(Clone)]
enum Buraco {
    /// O traço do pincel de correção: os pontos (pixels da foto) e o raio.
    Traco(Vec<(f32, f32)>, f32),
    /// A seleção (⇧⌫).
    Selecao(Arc<editor_core::Selecao>),
}

impl Buraco {
    /// Quanto do remendo entra no pixel `(x, y)`: 255 dentro, uma rampa de um
    /// pixel na borda.
    fn peso(&self, x: f32, y: f32) -> u8 {
        match self {
            Buraco::Traco(pontos, raio) => {
                let d = distancia_ao_traco(pontos, x, y);
                ((raio + 0.5 - d).clamp(0.0, 1.0) * 255.0).round() as u8
            }
            Buraco::Selecao(s) => s.valor(x as u32, y as u32),
        }
    }

    /// A caixa `(x0, y0, x1, y1)` do buraco, na foto.
    fn caixa(&self, largura: u32, altura: u32) -> Option<(u32, u32, u32, u32)> {
        let (x0, y0, x1, y1) = match self {
            Buraco::Traco(pontos, raio) => {
                let (mut a, mut b, mut c, mut d) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for (x, y) in pontos {
                    a = a.min(*x);
                    b = b.min(*y);
                    c = c.max(*x);
                    d = d.max(*y);
                }
                let r = raio + 1.0;
                (
                    (a - r).floor(),
                    (b - r).floor(),
                    (c + r).ceil(),
                    (d + r).ceil(),
                )
            }
            Buraco::Selecao(s) => {
                let l = s.limites();
                (l.x as f32, l.y as f32, l.direita() as f32, l.baixo() as f32)
            }
        };
        let caixa = (
            x0.max(0.0) as u32,
            y0.max(0.0) as u32,
            (x1.max(0.0) as u32).min(largura),
            (y1.max(0.0) as u32).min(altura),
        );
        (caixa.2 > caixa.0 && caixa.3 > caixa.1).then_some(caixa)
    }
}

/// A distância de um ponto ao traço (a linha quebrada pelos pontos).
fn distancia_ao_traco(pontos: &[(f32, f32)], x: f32, y: f32) -> f32 {
    let trechos: Vec<((f32, f32), (f32, f32))> = if pontos.len() == 1 {
        vec![(pontos[0], pontos[0])]
    } else {
        pontos.windows(2).map(|q| (q[0], q[1])).collect()
    };
    trechos
        .iter()
        .map(|(a, b)| {
            let ab = (b.0 - a.0, b.1 - a.1);
            let ap = (x - a.0, y - a.1);
            let l2 = ab.0 * ab.0 + ab.1 * ab.1;
            let t = if l2 > 1e-9 {
                ((ap.0 * ab.0 + ap.1 * ab.1) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (ap.0 - ab.0 * t).hypot(ap.1 - ab.1 * t)
        })
        .fold(f32::MAX, f32::min)
}

/// `[r, g, b]` → a cor do kit.
fn hsla_de(cor: [u8; 3]) -> gpui_kit::Hsla {
    gpui_kit::rgb((cor[0] as u32) << 16 | (cor[1] as u32) << 8 | cor[2] as u32).into()
}

/// A cor do kit → `[r, g, b]`.
fn rgb_de(cor: gpui_kit::Hsla) -> [u8; 3] {
    let c = cor.to_rgb();
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [q(c.r), q(c.g), q(c.b)]
}

/// As ferramentas de seleção (M, ⇧M, L).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoDeSelecao {
    Retangulo,
    Elipse,
    Laco,
}

/// Uma seleção sendo desenhada: os pontos em pixels da foto (no retângulo e
/// na elipse, o primeiro e o último são os cantos).
/// O que o arrasto faz na caixa da transformação livre.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ParteDaCaixa {
    Dentro,
    /// Um canto: muda o tamanho (proporcional; ⇧ solta).
    Canto,
    /// Fora da caixa: gira (⇧ de 15 em 15 graus).
    Fora,
}

/// Um arrasto na caixa: o que começou a fazer, onde (pixels da foto), e a
/// transformação de então.
#[derive(Clone, Copy)]
struct GestoDeTransformacao {
    parte: ParteDaCaixa,
    inicio: (f32, f32),
    t: Transformacao,
}

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
    /// Um slider por parâmetro de ajuste ([`PARAMETROS_DE_AJUSTE`]); o painel
    /// mostra os do ajuste escolhido.
    ajustes: Vec<Entity<SliderState>>,
    /// A tolerância da varinha (0–255; 32 no Photoshop) e se ela é contígua.
    tolerancia_da_varinha: Entity<SliderState>,
    varinha_contigua: bool,
    /// O raio, em pixels, da difusão, do expandir e do contrair.
    raio_da_selecao: Entity<SliderState>,
    modo: Entity<SelectState<Vec<Opcao>>>,
    /// O modo que o Select mostra — para só mexer nele quando mudar.
    modo_mostrado: Option<Modo>,
    /// A faixa de tons da subexposição e da superexposição (Select do kit).
    faixa: editor_core::pincel::Faixa,
    seletor_de_faixa: Entity<SelectState<Vec<Opcao>>>,
    /// A aba de baixo do painel: 0 = Camadas, 1 = Histórico.
    aba_do_painel: usize,
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
    auxiliar: Option<Auxiliar>,
    /// O conta-gotas com o botão apertado: arrastar continua pegando.
    pegando_cor: bool,
    /// O Mover: onde o arrasto começou, em pixels da foto.
    arrasto_do_mover: Option<(f32, f32)>,
    seletor_de_cor: Entity<ColorPickerState>,
    /// A cor que o seletor mostra — para só mexer nele quando mudar.
    cor_mostrada: Option<[u8; 3]>,
    gesto_de_transformacao: Option<GestoDeTransformacao>,
    /// O traço do pincel de correção em curso (pixels da foto).
    traco_de_correcao: Option<Vec<(f32, f32)>>,
    /// O degradê sendo arrastado: começo e fim, em pixels da foto.
    degrade_em_curso: Option<((f32, f32), (f32, f32))>,
    /// A miniatura da máscara de cada camada (`None` sem máscara).
    miniaturas_das_mascaras: Vec<Option<Arc<RenderImage>>>,
    /// Um preenchimento por conteúdo calculando em segundo plano.
    preenchendo: bool,
    _tarefa_do_preenchimento: Option<Task<()>>,
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
        let tolerancia_da_varinha = slider(0.0, 255.0, 1.0, 32.0, cx);
        let raio_da_selecao = slider(1.0, 100.0, 1.0, 5.0, cx);
        let ajustes: Vec<Entity<SliderState>> = PARAMETROS_DE_AJUSTE
            .iter()
            .map(|p| slider(p.min, p.max, p.passo, p.inicial, cx))
            .collect();
        let opcoes: Vec<Opcao> = Modo::TODOS
            .iter()
            .map(|m| Opcao::nova(m.chave(), m.nome()))
            .collect();
        let modo = cx.new(|cx| SelectState::new(opcoes, None, window, cx));
        let faixas: Vec<Opcao> = editor_core::pincel::Faixa::TODAS
            .iter()
            .map(|f| Opcao::nova(f.nome(), f.nome()))
            .collect();
        let seletor_de_faixa = cx.new(|cx| SelectState::new(faixas, None, window, cx));
        seletor_de_faixa.update(cx, |s, cx| {
            s.set_selected_value(
                &editor_core::pincel::Faixa::default().nome().to_string(),
                window,
                cx,
            )
        });
        let seletor_de_cor =
            cx.new(|cx| ColorPickerState::new(window, cx).default_value(hsla_de(pincel.cor)));

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
        for (qual, estado) in ajustes.iter().enumerate() {
            assinaturas.push(cx.subscribe_in(
                estado,
                window,
                move |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                    let (v, soltou) = valor(evento);
                    ed.mover_parametro_do_ajuste(qual, v, soltou, cx);
                },
            ));
        }
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

        assinaturas.push(cx.subscribe_in(
            &seletor_de_cor,
            window,
            |ed: &mut Self, _e, evento: &ColorPickerEvent, _w, cx| {
                if let ColorPickerEvent::Change(Some(cor)) = evento {
                    let cor = rgb_de(*cor);
                    ed.cor_mostrada = Some(cor);
                    if let Some(s) = ed.sessao_mut() {
                        s.pincel.cor = cor;
                    }
                    cx.notify();
                }
            },
        ));

        assinaturas.push(cx.subscribe_in(
            &seletor_de_faixa,
            window,
            |ed: &mut Self, _e, evento: &SelectEvent<Vec<Opcao>>, window, cx| {
                let SelectEvent::Confirm(Some(nome)) = evento else {
                    return;
                };
                if let Some(faixa) = editor_core::pincel::Faixa::TODAS
                    .into_iter()
                    .find(|f| f.nome() == nome)
                {
                    ed.escolher_faixa(faixa, cx);
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
            ajustes,
            tolerancia_da_varinha,
            varinha_contigua: true,
            raio_da_selecao,
            modo,
            modo_mostrado: None,
            faixa: editor_core::pincel::Faixa::default(),
            seletor_de_faixa,
            aba_do_painel: 0,
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
            auxiliar: None,
            pegando_cor: false,
            arrasto_do_mover: None,
            seletor_de_cor,
            cor_mostrada: None,
            gesto_de_transformacao: None,
            traco_de_correcao: None,
            degrade_em_curso: None,
            miniaturas_das_mascaras: Vec::new(),
            preenchendo: false,
            _tarefa_do_preenchimento: None,
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

    /// O botão esquerdo desceu no palco, com os modificadores: decide qual
    /// ferramenta recebe o gesto.
    ///
    /// - conta-gotas (I), ou ⌥ com o pincel: pega a cor;
    /// - Mover (V): começa a arrastar a camada;
    /// - ⌥ com o carimbo: escolhe a origem;
    /// - seleção: começa a forma;
    /// - o resto: o traço.
    pub fn apertar_com(
        &mut self,
        ponto: Point<Pixels>,
        modificadores: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) {
        let ferramenta = self.ferramenta();
        if self.sessao().is_some_and(Sessao::transformando) {
            self.comecar_gesto_na_caixa(ponto, cx);
            return;
        }
        if self.selecionando.is_some() {
            self.comecar_selecao(ponto, modificadores, cx);
            return;
        }
        match self.auxiliar {
            Some(Auxiliar::ContaGotas) => {
                self.pegando_cor = true;
                self.pegar_cor(ponto, cx);
                return;
            }
            Some(Auxiliar::Mover) => {
                self.comecar_a_mover(ponto, cx);
                return;
            }
            Some(Auxiliar::Mao) => {
                self.pegar_com_a_mao(ponto, cx);
                return;
            }
            Some(Auxiliar::Zoom) => {
                let p = self.ponto_no_palco(ponto);
                self.ampliar_em_torno(if modificadores.alt { 0.5 } else { 2.0 }, p, cx);
                return;
            }
            Some(Auxiliar::Degrade) => {
                if let Some(p) = self.na_foto_sem_limite(ponto) {
                    self.degrade_em_curso = Some((p, p));
                    cx.notify();
                }
                return;
            }
            Some(Auxiliar::Lata) => {
                if let Some((x, y)) = self.na_foto(ponto) {
                    self.lata_de_tinta(x, y, cx);
                }
                return;
            }
            Some(Auxiliar::Varinha) => {
                if let Some((x, y)) = self.na_foto(ponto) {
                    self.varinha(x, y, operacao_dos(modificadores), cx);
                }
                return;
            }
            Some(Auxiliar::Correcao) => {
                if self.avisar_se_na_mascara(cx) {
                    return;
                }
                if let Some(p) = self.na_foto(ponto) {
                    self.traco_de_correcao = Some(vec![p]);
                    cx.notify();
                }
                return;
            }
            None => {}
        }
        if modificadores.alt {
            match ferramenta {
                Some(Ferramenta::Carimbo) => {
                    if let (Some((x, y)), Some(s)) = (self.na_foto(ponto), self.sessao_mut()) {
                        s.definir_origem(x, y);
                    }
                    self.aviso = None;
                    cx.notify();
                    return;
                }
                Some(Ferramenta::Pincel) => {
                    self.pegando_cor = true;
                    self.pegar_cor(ponto, cx);
                    return;
                }
                _ => {}
            }
        }
        self.apertar(ponto, cx);
    }

    // ---------------------------------------------- transformação livre

    /// ⌘T: a caixa aparece em volta do conteúdo da camada (ou da seleção).
    pub fn transformar(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.sessao_mut() else {
            return;
        };
        self.aviso = if s.comecar_a_transformar() {
            None
        } else {
            Some((
                "Nada para transformar: a camada está vazia ou escondida".into(),
                true,
            ))
        };
        cx.notify();
    }

    /// Enter.
    pub fn aplicar_transformacao(&mut self, cx: &mut Context<Self>) {
        self.gesto_de_transformacao = None;
        self.na_sessao(cx, |s| {
            s.aplicar_transformacao();
        });
    }

    /// Esc.
    pub fn cancelar_transformacao(&mut self, cx: &mut Context<Self>) {
        self.gesto_de_transformacao = None;
        self.na_sessao(cx, Sessao::cancelar_transformacao);
    }

    pub fn transformando(&self) -> bool {
        self.sessao().is_some_and(Sessao::transformando)
    }

    // -------------------------------------------- preenchimento por conteúdo

    /// ⇧⌫: a seleção refeita pelo que está em volta dela.
    pub fn preencher_a_selecao_pelo_conteudo(&mut self, cx: &mut Context<Self>) {
        match self.sessao().and_then(|s| s.selecao().cloned()) {
            Some(s) => self.preencher_pelo_conteudo(Buraco::Selecao(Arc::new(s)), cx),
            None => {
                self.aviso = Some((
                    "Selecione a área a refazer pelo conteúdo em volta".into(),
                    true,
                ));
                cx.notify();
            }
        }
    }

    pub fn preenchendo(&self) -> bool {
        self.preenchendo
    }

    /// O preenchimento por conteúdo (o mesmo PatchMatch da Revelação): a foto
    /// até a camada escolhida, num recorte com a margem de trabalho, vai para o
    /// executor de fundo; o remendo volta e entra na camada como um passo do
    /// desfazer.
    fn preencher_pelo_conteudo(&mut self, buraco: Buraco, cx: &mut Context<Self>) {
        if self.preenchendo || self.avisar_se_na_mascara(cx) {
            return;
        }
        let Some(s) = self.sessao() else {
            return;
        };
        let (largura, altura) = (s.base().width(), s.base().height());
        let Some(caixa) = buraco.caixa(largura, altura) else {
            return;
        };
        let raio = match &buraco {
            Buraco::Traco(_, r) => *r,
            Buraco::Selecao(_) => 0.0,
        };
        let margem = revelacao_core::preenchimento::margem_de_trabalho(caixa, raio);
        let (rx, ry) = (
            caixa.0.saturating_sub(margem),
            caixa.1.saturating_sub(margem),
        );
        let regiao = Retangulo::novo(
            rx,
            ry,
            (caixa.2 + margem).min(largura) - rx,
            (caixa.3 + margem).min(altura) - ry,
        );
        let camada = s.ativa();
        let foto = s.foto_ate_a_ativa(&regiao);
        self.preenchendo = true;
        self.aviso = Some(("Refazendo pelo conteúdo em volta…".into(), false));
        cx.notify();

        let semente = ((caixa.0 as u64) << 48)
            ^ ((caixa.1 as u64) << 32)
            ^ ((caixa.2 as u64) << 16)
            ^ caixa.3 as u64;
        let para_o_fundo = buraco.clone();
        let inicio = Instant::now();
        let trabalho = cx.background_executor().spawn(async move {
            let rgba: Vec<u8> = foto
                .pixels()
                .flat_map(|p| [p.0[0], p.0[1], p.0[2], 255])
                .collect();
            let (ox, oy) = (regiao.x as f32, regiao.y as f32);
            let no_buraco = |x: f32, y: f32| para_o_fundo.peso(x + ox, y + oy) >= 128;
            let relativa = (
                caixa.0 - regiao.x,
                caixa.1 - regiao.y,
                caixa.2 - regiao.x,
                caixa.3 - regiao.y,
            );
            revelacao_core::preenchimento::preencher_buraco(
                &rgba,
                regiao.largura,
                regiao.altura,
                relativa,
                &no_buraco,
                semente,
            )
        });
        self._tarefa_do_preenchimento = Some(cx.spawn(async move |esta, cx| {
            let remendo = trabalho.await;
            let _ = esta.update(cx, |ed, cx| {
                ed.preenchendo = false;
                ed.medidas.ultimo_preenchimento = Some(inicio.elapsed());
                let Some(r) = remendo else {
                    ed.aviso = Some((
                        "Não há em volta de onde tirar o preenchimento — selecione uma área menor"
                            .into(),
                        true,
                    ));
                    cx.notify();
                    return;
                };
                let ret = Retangulo::novo(regiao.x + r.x0, regiao.y + r.y0, r.largura, r.altura);
                let peso = |x: u32, y: u32| buraco.peso(x as f32 + 0.5, y as f32 + 0.5);
                if let Some(s) = ed.sessao_mut() {
                    s.colar_remendo(camada, &ret, &r.rgba, &peso);
                }
                ed.aviso = None;
                cx.notify();
            });
        }));
    }

    /// Em que parte da caixa o ponto (da foto) cai — o canto conta com uma
    /// folga de 8 pontos da tela.
    fn parte_da_caixa(&self, x: f32, y: f32) -> Option<ParteDaCaixa> {
        let (caixa, t) = self.sessao()?.transformacao()?;
        let escala = self.vista_do_zoom().map_or(1.0, |(_, v)| v.escala);
        let folga = 8.0 / escala;
        if t.cantos(&caixa)
            .iter()
            .any(|(cx_, cy_)| (cx_ - x).hypot(cy_ - y) <= folga)
        {
            return Some(ParteDaCaixa::Canto);
        }
        let (u, v) = t.inversa(&caixa, x, y);
        let dentro = u >= caixa.x as f32
            && v >= caixa.y as f32
            && u <= caixa.direita() as f32
            && v <= caixa.baixo() as f32;
        Some(if dentro {
            ParteDaCaixa::Dentro
        } else {
            ParteDaCaixa::Fora
        })
    }

    fn comecar_gesto_na_caixa(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        let (Some(p), Some((_, t))) = (
            self.na_foto_sem_limite(ponto),
            self.sessao().and_then(Sessao::transformacao),
        ) else {
            return;
        };
        let Some(parte) = self.parte_da_caixa(p.0, p.1) else {
            return;
        };
        self.gesto_de_transformacao = Some(GestoDeTransformacao {
            parte,
            inicio: p,
            t,
        });
        cx.notify();
    }

    /// O arrasto na caixa: mover, escalar (em volta do centro) ou girar.
    fn arrastar_na_caixa(&mut self, ponto: Point<Pixels>, livre: bool, cx: &mut Context<Self>) {
        let (Some(g), Some(p), Some((caixa, _))) = (
            self.gesto_de_transformacao,
            self.na_foto_sem_limite(ponto),
            self.sessao().and_then(Sessao::transformacao),
        ) else {
            return;
        };
        let mut t = g.t;
        let centro = (
            caixa.x as f32 + caixa.largura as f32 / 2.0 + g.t.dx,
            caixa.y as f32 + caixa.altura as f32 / 2.0 + g.t.dy,
        );
        match g.parte {
            ParteDaCaixa::Dentro => {
                t.dx = g.t.dx + (p.0 - g.inicio.0).round();
                t.dy = g.t.dy + (p.1 - g.inicio.1).round();
            }
            ParteDaCaixa::Canto => {
                // O ponteiro no referencial da caixa (sem o giro).
                let (s, c) = g.t.angulo.sin_cos();
                let local = |q: (f32, f32)| {
                    let (x, y) = (q.0 - centro.0, q.1 - centro.1);
                    (x * c + y * s, -x * s + y * c)
                };
                let (meia_l, meia_a) = (caixa.largura as f32 / 2.0, caixa.altura as f32 / 2.0);
                let (lx, ly) = local(p);
                let (sx, sy) = ((lx / meia_l).abs().max(0.01), (ly / meia_a).abs().max(0.01));
                if livre {
                    t.escala_x = sx;
                    t.escala_y = sy;
                } else {
                    // Proporcional: a distância ao centro ao longo da diagonal.
                    let (ix, iy) = local(g.inicio);
                    let antes = ix.hypot(iy).max(1.0);
                    let fator = lx.hypot(ly) / antes;
                    t.escala_x = (g.t.escala_x * fator).max(0.01);
                    t.escala_y = (g.t.escala_y * fator).max(0.01);
                }
            }
            ParteDaCaixa::Fora => {
                let angulo = |q: (f32, f32)| (q.1 - centro.1).atan2(q.0 - centro.0);
                let mut a = g.t.angulo + angulo(p) - angulo(g.inicio);
                if livre {
                    let passo = std::f32::consts::PI / 12.0;
                    a = (a / passo).round() * passo;
                }
                t.angulo = a;
            }
        }
        // Só mede o que mudou: o soltar no mesmo ponto não é gesto.
        if self
            .sessao()
            .and_then(Sessao::transformacao)
            .map(|(_, t0)| t0)
            != Some(t)
        {
            let inicio = Instant::now();
            if let Some(s) = self.sessao_mut() {
                s.definir_transformacao(t);
            }
            self.medidas.ultimo_gesto = Some(inicio.elapsed());
        }
        cx.notify();
    }

    /// O conta-gotas: a cor da foto (como ela aparece) no ponto.
    pub fn pegar_cor(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((x, y)) = self.na_foto(ponto) else {
            return;
        };
        if let Some(s) = self.sessao_mut() {
            if let Some(cor) = s.cor_em(x, y) {
                s.pincel.cor = cor;
            }
        }
        cx.notify();
    }

    fn comecar_a_mover(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(p) = self.na_foto_sem_limite(ponto) else {
            return;
        };
        let Some(s) = self.sessao_mut() else {
            return;
        };
        if s.comecar_a_mover() {
            self.arrasto_do_mover = Some(p);
            self.aviso = None;
        } else {
            let nome = s.camada_ativa().nome.clone();
            self.aviso = Some((
                format!("{nome} está escondida — mostre a camada (H) para movê-la").into(),
                true,
            ));
        }
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
        } else if s.pincel.ferramenta == Ferramenta::Carimbo && s.origem().is_none() {
            self.aviso = Some((
                "⌥ + clique na foto para escolher de onde o carimbo copia".into(),
                true,
            ));
        } else if s.na_mascara() && s.pincel.ferramenta.le_a_foto() {
            self.aviso = Some((
                "Na máscara se pinta com o pincel e a borracha: preto esconde, branco revela"
                    .into(),
                true,
            ));
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
        self.mover_com(ponto, gpui_kit::Modifiers::none(), cx);
    }

    /// O ponteiro andou, com os modificadores de agora (⇧ na transformação).
    pub fn mover_com(
        &mut self,
        ponto: Point<Pixels>,
        modificadores: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) {
        self.ponteiro = Some(ponto);
        if self.gesto_de_transformacao.is_some() {
            self.arrastar_na_caixa(ponto, modificadores.shift, cx);
            return;
        }
        if let Some((de, _)) = self.degrade_em_curso {
            if let Some(mut p) = self.na_foto_sem_limite(ponto) {
                if modificadores.shift {
                    // ⇧ prende o ângulo em múltiplos de 45°, como no Photoshop.
                    let (dx, dy) = (p.0 - de.0, p.1 - de.1);
                    let passo = std::f32::consts::FRAC_PI_4;
                    let angulo = (dy.atan2(dx) / passo).round() * passo;
                    let r = dx.hypot(dy);
                    p = (de.0 + r * angulo.cos(), de.1 + r * angulo.sin());
                }
                self.degrade_em_curso = Some((de, p));
            }
            cx.notify();
            return;
        }
        if self.traco_de_correcao.is_some() {
            let escala = self.vista_do_zoom().map_or(1.0, |(_, v)| v.escala);
            if let (Some(p), Some(traco)) = (
                self.na_foto_sem_limite(ponto),
                self.traco_de_correcao.as_mut(),
            ) {
                let ultimo = *traco.last().unwrap_or(&p);
                if (p.0 - ultimo.0).hypot(p.1 - ultimo.1) * escala >= 2.0 {
                    traco.push(p);
                }
            }
            cx.notify();
            return;
        }
        if self.pegando_cor {
            self.pegar_cor(ponto, cx);
            return;
        }
        if let Some(inicio) = self.arrasto_do_mover {
            if let Some(p) = self.na_foto_sem_limite(ponto) {
                let (dx, dy) = (
                    (p.0 - inicio.0).round() as i64,
                    (p.1 - inicio.1).round() as i64,
                );
                let comeco = Instant::now();
                if let Some(s) = self.sessao_mut() {
                    s.mover_por(dx, dy);
                }
                self.medidas.ultimo_gesto = Some(comeco.elapsed());
            }
            cx.notify();
            return;
        }
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
        self.pegando_cor = false;
        if self.gesto_de_transformacao.take().is_some() {
            cx.notify();
            return;
        }
        if let Some((de, ate)) = self.degrade_em_curso.take() {
            let escondida = self.sessao().is_some_and(|s| !s.camada_ativa().visivel);
            let inicio = Instant::now();
            self.na_sessao(cx, |s| {
                s.degrade(de, ate);
            });
            self.medidas.ultimo_gesto = Some(inicio.elapsed());
            if escondida {
                self.aviso = Some((
                    "A camada está escondida — mostre-a (H) para o degradê".into(),
                    true,
                ));
            }
            return;
        }
        if let Some(traco) = self.traco_de_correcao.take() {
            let raio = self.sessao().map_or(10.0, |s| s.pincel.raio);
            self.preencher_pelo_conteudo(Buraco::Traco(traco, raio), cx);
            return;
        }
        if self.arrasto_do_mover.take().is_some() {
            if let Some(s) = self.sessao_mut() {
                s.terminar_de_mover();
            }
            cx.notify();
            return;
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
        self.auxiliar = None;
        cx.notify();
    }

    /// O conta-gotas (I) ou o Mover (V) na mão.
    pub fn usar_auxiliar(&mut self, auxiliar: Auxiliar, cx: &mut Context<Self>) {
        self.auxiliar = Some(auxiliar);
        self.selecionando = None;
        cx.notify();
    }

    pub fn auxiliar(&self) -> Option<Auxiliar> {
        self.auxiliar
    }

    /// O nome da ferramenta na mão, com o atalho — o título das opções no
    /// painel da direita.
    pub fn nome_da_ferramenta(&self) -> &'static str {
        if let Some(tipo) = self.selecionando {
            return match tipo {
                TipoDeSelecao::Retangulo => "Seleção retangular (M)",
                TipoDeSelecao::Elipse => "Seleção elíptica (⇧M)",
                TipoDeSelecao::Laco => "Laço (L)",
            };
        }
        if let Some(a) = self.auxiliar {
            return match a {
                Auxiliar::ContaGotas => "Conta-gotas (I)",
                Auxiliar::Mover => "Mover (V)",
                Auxiliar::Correcao => "Pincel de correção (J)",
                Auxiliar::Degrade => "Degradê (G) — arraste do começo ao fim",
                Auxiliar::Lata => "Lata de tinta (⇧G)",
                Auxiliar::Varinha => "Varinha mágica (W) — ⇧ soma, ⌥ tira",
                Auxiliar::Mao => "Mão (Espaço)",
                Auxiliar::Zoom => "Zoom — clique amplia, ⌥ + clique afasta",
            };
        }
        match self.ferramenta() {
            Some(Ferramenta::Borracha) => "Borracha (E)",
            Some(Ferramenta::Carimbo) => "Carimbo (S) — ⌥ + clique na origem",
            Some(Ferramenta::Subexposicao(_)) => "Subexposição (O)",
            Some(Ferramenta::Superexposicao(_)) => "Superexposição (⇧O)",
            Some(Ferramenta::Desfoque) => "Desfoque (R)",
            Some(Ferramenta::Nitidez) => "Nitidez (⇧R)",
            _ => "Pincel (B)",
        }
    }

    /// A barra de ferramentas vertical do Photoshop, à esquerda do palco: as
    /// ferramentas na ordem de lá, em grupos, e a cor atual embaixo.
    fn barra_de_ferramentas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let pincel_na_mao = self.selecionando.is_none() && self.auxiliar.is_none();
        let ferramenta = self.ferramenta().filter(|_| pincel_na_mao);
        let mesma = |a: Ferramenta| {
            ferramenta.is_some_and(|f| std::mem::discriminant(&f) == std::mem::discriminant(&a))
        };
        #[derive(Clone, Copy)]
        enum Item {
            F(Ferramenta),
            S(TipoDeSelecao),
            A(Auxiliar),
        }
        let faixa = self.faixa;
        let grupos: [&[(Item, Icone, &'static str, &'static str)]; 8] = [
            &[(
                Item::A(Auxiliar::Mover),
                Icone::Move,
                "editor-mover",
                "Mover (V)",
            )],
            &[
                (
                    Item::S(TipoDeSelecao::Retangulo),
                    Icone::Square,
                    "editor-selecao-retangulo",
                    "Seleção retangular (M) — ⇧ soma, ⌥ tira",
                ),
                (
                    Item::S(TipoDeSelecao::Elipse),
                    Icone::CircleDashed,
                    "editor-selecao-elipse",
                    "Seleção elíptica (⇧M)",
                ),
                (
                    Item::S(TipoDeSelecao::Laco),
                    Icone::Lasso,
                    "editor-selecao-laco",
                    "Laço (L)",
                ),
                (
                    Item::A(Auxiliar::Varinha),
                    Icone::WandSparkles,
                    "editor-varinha",
                    "Varinha mágica (W) — clique na cor; ⇧ soma, ⌥ tira",
                ),
            ],
            &[(
                Item::A(Auxiliar::ContaGotas),
                Icone::Pipette,
                "editor-conta-gotas",
                "Conta-gotas (I) — com o pincel, ⌥ + clique",
            )],
            &[
                (
                    Item::A(Auxiliar::Correcao),
                    Icone::Bandage,
                    "editor-correcao",
                    "Pincel de correção (J)",
                ),
                (
                    Item::F(Ferramenta::Pincel),
                    Icone::Paintbrush,
                    "editor-pincel",
                    "Pincel (B)",
                ),
                (
                    Item::F(Ferramenta::Borracha),
                    Icone::Eraser,
                    "editor-borracha",
                    "Borracha (E)",
                ),
                (
                    Item::F(Ferramenta::Carimbo),
                    Icone::Stamp,
                    "editor-carimbo",
                    "Carimbo (S) — ⌥ + clique escolhe a origem",
                ),
            ],
            &[
                (
                    Item::A(Auxiliar::Degrade),
                    Icone::Gradient,
                    "editor-degrade",
                    "Degradê (G) — arraste; ⇧ prende em 45°. Na máscara, preto → branco",
                ),
                (
                    Item::A(Auxiliar::Lata),
                    Icone::PaintBucket,
                    "editor-lata",
                    "Lata de tinta (⇧G) — pinta a área parecida em volta do clique",
                ),
            ],
            &[
                (
                    Item::F(Ferramenta::Desfoque),
                    Icone::Droplet,
                    "editor-desfoque",
                    "Desfoque (R)",
                ),
                (
                    Item::F(Ferramenta::Nitidez),
                    Icone::Triangle,
                    "editor-nitidez",
                    "Nitidez (⇧R)",
                ),
            ],
            &[
                (
                    Item::F(Ferramenta::Subexposicao(faixa)),
                    Icone::Sun,
                    "editor-subexposicao",
                    "Subexposição (O) — clareia",
                ),
                (
                    Item::F(Ferramenta::Superexposicao(faixa)),
                    Icone::Moon,
                    "editor-superexposicao",
                    "Superexposição (⇧O) — escurece",
                ),
            ],
            &[
                (
                    Item::A(Auxiliar::Mao),
                    Icone::Hand,
                    "editor-mao",
                    "Mão — arrasta a foto ampliada (ou segure o Espaço)",
                ),
                (
                    Item::A(Auxiliar::Zoom),
                    Icone::ZoomIn,
                    "editor-lupa",
                    "Zoom — clique amplia, ⌥ + clique afasta (⌘= ⌘− ⌘0)",
                ),
            ],
        ];
        let mut barra = div()
            .id("editor-barra-de-ferramentas")
            .debug_selector(|| "editor-barra-de-ferramentas".into())
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.))
            .w(px(44.))
            .h_full()
            .py(px(8.))
            .border_r_1()
            .border_color(tema.border)
            .bg(tema.background);
        for (n, grupo) in grupos.iter().enumerate() {
            if n > 0 {
                barra = barra.child(div().my(px(4.)).h(px(1.)).w(px(24.)).bg(tema.border));
            }
            for &(item, icone, id, dica) in grupo.iter() {
                let ativa = match item {
                    Item::F(f) => mesma(f),
                    Item::S(t) => self.selecionando == Some(t),
                    Item::A(a) => self.auxiliar == Some(a),
                };
                let botao = crate::estilo::botao_icone_padrao(id, icone)
                    .debug_selector(move || id.into())
                    .tooltip(dica)
                    .on_click(cx.listener(move |ed, _, _, cx| match item {
                        Item::F(f) => ed.usar(f, cx),
                        Item::S(t) => ed.usar_selecao(t, cx),
                        Item::A(a) => ed.usar_auxiliar(a, cx),
                    }));
                barra = barra.child(if ativa { botao.primary() } else { botao });
            }
        }
        // As cores de frente e de fundo do Photoshop: a de frente abre o
        // seletor; a de fundo, embaixo e à direita, troca com ela no clique
        // (X); D volta a preto e branco.
        let fundo = self.sessao().map_or([255; 3], |s| s.pincel.cor_de_fundo);
        barra.child(div().flex_1()).child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(2.))
                .child(
                    div()
                        .debug_selector(|| "editor-seletor-de-cor".into())
                        .child(ColorPicker::new(&self.seletor_de_cor)),
                )
                .child(
                    div()
                        .id("editor-cor-de-fundo")
                        .debug_selector(|| "editor-cor-de-fundo".into())
                        .size(px(16.))
                        .ml(px(12.))
                        .rounded(crate::tema::canto(3.))
                        .border_1()
                        .border_color(tema.border)
                        .bg(gpui_kit::rgb(
                            (fundo[0] as u32) << 16 | (fundo[1] as u32) << 8 | fundo[2] as u32,
                        ))
                        .cursor_pointer()
                        .tooltip(|window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(
                                "Cor de fundo — clique ou X troca com a de frente; D volta a preto e branco",
                            )
                            .build(window, cx)
                        })
                        .on_click(cx.listener(|ed, _, _, cx| ed.trocar_cores(cx))),
                ),
        )
    }

    /// X.
    pub fn trocar_cores(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, Sessao::trocar_cores);
    }

    /// D.
    pub fn cores_padrao(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, Sessao::cores_padrao);
    }

    /// A faixa de tons da subexposição e da superexposição; a ferramenta na
    /// mão, se for uma delas, passa a usá-la.
    pub fn escolher_faixa(&mut self, faixa: editor_core::pincel::Faixa, cx: &mut Context<Self>) {
        self.faixa = faixa;
        if let Some(s) = self.sessao_mut() {
            s.pincel.ferramenta = match s.pincel.ferramenta {
                Ferramenta::Subexposicao(_) => Ferramenta::Subexposicao(faixa),
                Ferramenta::Superexposicao(_) => Ferramenta::Superexposicao(faixa),
                outra => outra,
            };
        }
        cx.notify();
    }

    /// O painel Histórico: volta ou avança até `posicao` passos.
    pub fn ir_para_no_historico(&mut self, posicao: usize, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.ir_para(posicao);
        });
    }

    pub fn usar(&mut self, ferramenta: Ferramenta, cx: &mut Context<Self>) {
        self.selecionando = None;
        self.auxiliar = None;
        if let Some(s) = self.sessao_mut() {
            s.pincel.ferramenta = ferramenta;
        }
        cx.notify();
    }

    pub fn escolher_cor(&mut self, cor: [u8; 3], cx: &mut Context<Self>) {
        let com_o_pincel = self.selecionando.is_none()
            && self.auxiliar.is_none()
            && self.ferramenta() != Some(Ferramenta::Carimbo);
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

    // ----------------------------------------------------------- seleção

    /// A varinha em `(x, y)`, pixels da foto.
    pub fn varinha(&mut self, x: f32, y: f32, operacao: Operacao, cx: &mut Context<Self>) {
        let tolerancia = self.tolerancia_da_varinha.read(cx).value().start().round() as u8;
        let contigua = self.varinha_contigua;
        let inicio = Instant::now();
        self.na_sessao(cx, |s| {
            s.varinha(x, y, tolerancia, contigua, operacao);
        });
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
    }

    pub fn alternar_varinha_contigua(&mut self, cx: &mut Context<Self>) {
        self.varinha_contigua = !self.varinha_contigua;
        cx.notify();
    }

    /// O raio de agora para difundir, expandir e contrair.
    fn raio_da_selecao(&self, cx: &Context<Self>) -> u32 {
        self.raio_da_selecao
            .read(cx)
            .value()
            .start()
            .round()
            .max(1.0) as u32
    }

    /// ⇧F6 e o botão Difundir.
    pub fn difundir_selecao(&mut self, cx: &mut Context<Self>) {
        let raio = self.raio_da_selecao(cx);
        let inicio = Instant::now();
        self.na_sessao(cx, |s| {
            s.difundir_selecao(raio);
        });
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
    }

    /// Expandir (`sinal` 1) ou contrair (−1) pelo raio de agora.
    pub fn expandir_selecao(&mut self, sinal: i32, cx: &mut Context<Self>) {
        let px = self.raio_da_selecao(cx) as i32 * sinal.signum();
        self.na_sessao(cx, |s| {
            s.expandir_selecao(px);
        });
    }

    /// ⌘ + clique numa miniatura: a seleção da camada (ou da máscara).
    pub fn selecionar_da_camada(
        &mut self,
        indice: usize,
        da_mascara: bool,
        operacao: Operacao,
        cx: &mut Context<Self>,
    ) {
        self.na_sessao(cx, |s| {
            s.selecionar_da_camada(indice, da_mascara, operacao);
        });
    }

    // ------------------------------------------------------------ ajuste

    /// O menu de ajustes: uma camada de ajuste acima da escolhida.
    pub fn nova_camada_de_ajuste(&mut self, ajuste: Ajuste, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| s.nova_camada_de_ajuste(ajuste));
    }

    /// O ajuste da camada escolhida, se ela for de ajuste.
    pub fn ajuste_da_camada(&self) -> Option<Ajuste> {
        self.sessao().and_then(|s| s.camada_ativa().ajuste)
    }

    /// O slider `qual` do ajuste andou (ou soltou: vira um passo).
    pub fn mover_parametro_do_ajuste(
        &mut self,
        qual: usize,
        valor: f32,
        soltou: bool,
        cx: &mut Context<Self>,
    ) {
        let inicio = Instant::now();
        if let Some(s) = self.sessao_mut() {
            if let Some(a) = s.camada_ativa().ajuste {
                s.mover_ajuste(ajuste_com(a, qual, valor));
                if soltou {
                    s.confirmar_ajuste();
                }
            }
        }
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
        cx.notify();
    }

    /// As Propriedades do Photoshop para a camada de ajuste escolhida: um
    /// slider por parâmetro, com o valor ao lado.
    fn propriedades_do_ajuste(&self, ajuste: Ajuste, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let parametros = parametros_do_ajuste(&ajuste);
        let mut coluna = div()
            .debug_selector(|| "editor-propriedades".into())
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(format!("Propriedades — {}", ajuste.nome())),
            );
        if parametros.is_empty() {
            coluna = coluna.child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child("Sem controles: as cores de baixo viram o negativo"),
            );
        }
        for (qual, v) in parametros {
            let p = &PARAMETROS_DE_AJUSTE[qual];
            let texto = if p.passo < 1.0 {
                format!("{v:.2}")
            } else if p.min < 0.0 && v > 0.0 {
                format!("+{v:.0}")
            } else {
                format!("{v:.0}")
            };
            coluna = coluna
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_xs()
                        .text_color(tema.muted_foreground)
                        .child(p.nome)
                        .child(texto),
                )
                .child(
                    div()
                        .h(px(20.))
                        .debug_selector(move || format!("editor-ajuste-{}", p.chave))
                        .child(crate::estilo::slider(&self.ajustes[qual])),
                );
        }
        coluna.into_any_element()
    }

    // ----------------------------------------------------------- máscara

    /// O botão da máscara: revela tudo (com ⌥, esconde tudo); com seleção, a
    /// máscara nasce dela. O pincel passa a pintar na máscara.
    pub fn adicionar_mascara(&mut self, esconder: bool, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.adicionar_mascara(esconder);
        });
    }

    /// A miniatura da máscara clicada: o pincel pinta nela.
    pub fn escolher_mascara(&mut self, indice: usize, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.escolher_mascara(indice);
        });
    }

    /// ⇧ + clique na miniatura da máscara: liga ou desliga.
    pub fn alternar_mascara_de(&mut self, indice: usize, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.alternar_mascara_de(indice);
        });
    }

    pub fn excluir_mascara(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.excluir_mascara();
        });
    }

    /// O pincel está na máscara da escolhida.
    pub fn na_mascara(&self) -> bool {
        self.sessao().is_some_and(Sessao::na_mascara)
    }

    /// O que refaz a foto (o pincel de correção, o preenchimento por conteúdo)
    /// não pinta máscara: avisa e devolve `true` quando o pincel está nela.
    fn avisar_se_na_mascara(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.na_mascara() {
            return false;
        }
        self.aviso = Some((
            "Isto refaz a foto: clique na miniatura da camada, e não na da máscara".into(),
            true,
        ));
        cx.notify();
        true
    }

    /// A lata de tinta em `(x, y)`, pixels da foto.
    pub fn lata_de_tinta(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let escondida = self.sessao().is_some_and(|s| !s.camada_ativa().visivel);
        let inicio = Instant::now();
        self.na_sessao(cx, |s| {
            s.lata_de_tinta(x, y);
        });
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
        if escondida {
            self.aviso = Some((
                "A camada está escondida — mostre-a (H) para pintar nela".into(),
                true,
            ));
        }
    }

    /// O degradê de `de` a `ate`, pixels da foto — o roteiro e os testes.
    pub fn degrade(&mut self, de: (f32, f32), ate: (f32, f32), cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.degrade(de, ate);
        });
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

    /// ⌘J: com seleção, uma camada só com o selecionado; sem, a camada
    /// inteira duplicada.
    pub fn duplicar_camada(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.camada_via_copia(false);
        });
    }

    /// ⇧⌘J: o selecionado vai para uma camada nova e sai da de origem.
    pub fn camada_via_recorte(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.sessao_mut() else {
            return;
        };
        self.aviso = (!s.camada_via_copia(true)).then(|| {
            (
                SharedString::from("Selecione o pedaço antes de recortá-lo para outra camada"),
                true,
            )
        });
        cx.notify();
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
        let chave = (s.versao_da_selecao(), lupa);
        if self.bordas.as_ref().is_some_and(|(c, _)| *c == chave) {
            return;
        }
        let (regiao, passo) = lupa.unwrap_or((
            Retangulo::inteiro(selecao.largura(), selecao.altura()),
            s.vista().fator(),
        ));
        let inicio = Instant::now();
        let bordas = Arc::new(selecao.bordas(&regiao, passo));
        self.medidas.ultima_borda = Some(inicio.elapsed());
        self.bordas = Some((chave, bordas));
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
        let das_mascaras = s
            .documento()
            .camadas
            .iter()
            .map(|c| {
                let m = c.mascara.as_ref()?;
                let rgba = editor_core::operacoes::miniatura_da_mascara(m, l, a);
                crate::imagem::de_bgra(l, a, sobre_xadrez(&rgba, l))
            })
            .collect();
        self.miniaturas = miniaturas;
        self.miniaturas_das_mascaras = das_mascaras;
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
                let mods = partes
                    .iter()
                    .skip(4)
                    .map(|n| match *n {
                        "shift" => 1 << 17,
                        "ctrl" => 1 << 18,
                        "alt" => 1 << 19,
                        "cmd" => 1 << 20,
                        _ => 0,
                    })
                    .fold(0, |a, b| a | b);
                let r = crate::depuracao::mouse_nativo(window, tipo, x, y, mods);
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
                    "recortar" => self.camada_via_recorte(cx),
                    "subir" => self.mover_camada(1, cx),
                    "descer" => self.mover_camada(-1, cx),
                    "escolher" => self.escolher_camada(indice, cx),
                    "olho" => self.alternar_visibilidade_de(indice, cx),
                    // camada mascara [esconder] | mascara-escolher N |
                    // mascara-alternar N | mascara-excluir
                    "mascara" => self.adicionar_mascara(partes.get(2) == Some(&"esconder"), cx),
                    "mascara-escolher" => self.escolher_mascara(indice, cx),
                    "mascara-alternar" => self.alternar_mascara_de(indice, cx),
                    "mascara-excluir" => self.excluir_mascara(cx),
                    // camada ajuste brilho|niveis|matiz|inverter
                    "ajuste" => match partes.get(2).and_then(|c| Ajuste::da_chave(c)) {
                        Some(a) => self.nova_camada_de_ajuste(a, cx),
                        None => {
                            eprintln!("[roteiro] editor: ajuste desconhecido {:?}", partes.get(2))
                        }
                    },
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
                    // selecao varinha fx fy [somar|subtrair]
                    "varinha" => {
                        let (x, y) = ponto(2);
                        self.varinha(x, y, operacao, cx);
                        None
                    }
                    // selecao difundir|expandir|contrair N
                    "difundir" | "expandir" | "contrair" => {
                        let raio = numero(2);
                        self.raio_da_selecao
                            .update(cx, |s, cx| s.set_value(raio, window, cx));
                        match partes[1] {
                            "difundir" => self.difundir_selecao(cx),
                            "expandir" => self.expandir_selecao(1, cx),
                            _ => self.expandir_selecao(-1, cx),
                        }
                        None
                    }
                    // selecao camada N [mascara] [somar|subtrair]
                    "camada" => {
                        let mascara = partes.get(3) == Some(&"mascara");
                        self.selecionar_da_camada(numero(2) as usize, mascara, operacao, cx);
                        None
                    }
                    // selecao tolerancia N | selecao contigua sim|nao
                    "tolerancia" => {
                        let v = numero(2);
                        self.tolerancia_da_varinha
                            .update(cx, |s, cx| s.set_value(v, window, cx));
                        None
                    }
                    "contigua" => {
                        self.varinha_contigua = partes.get(2) != Some(&"nao");
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
            "conteudo" => self.preencher_a_selecao_pelo_conteudo(cx),
            "transformar" => match partes.get(1).copied().unwrap_or_default() {
                "comecar" => self.transformar(cx),
                "aplicar" => self.aplicar_transformacao(cx),
                "cancelar" => self.cancelar_transformacao(cx),
                // `transformar definir dx dy escala graus`
                "definir" => {
                    let t = Transformacao {
                        dx: numero(2),
                        dy: numero(3),
                        escala_x: numero(4).max(0.01),
                        escala_y: numero(4).max(0.01),
                        angulo: numero(5).to_radians(),
                    };
                    self.na_sessao(cx, |s| s.definir_transformacao(t));
                }
                outro => eprintln!("[roteiro] editor transformar {outro}?"),
            },
            // ajuste brilho|contraste|preto|gama|branco|matiz|saturacao|luminosidade V
            // [arrastando] — sem `arrastando`, solta (um passo do desfazer)
            "ajuste" => {
                let chave = partes.get(1).copied().unwrap_or_default();
                match PARAMETROS_DE_AJUSTE.iter().position(|p| p.chave == chave) {
                    Some(qual) => {
                        let soltou = partes.get(3) != Some(&"arrastando");
                        self.mover_parametro_do_ajuste(qual, numero(2), soltou, cx)
                    }
                    None => eprintln!("[roteiro] editor ajuste {chave}?"),
                }
            }
            // pincel tamanho|dureza|opacidade V (dureza e opacidade em %)
            "pincel" => {
                let v = numero(2);
                let (estado, valor) = match partes.get(1).copied() {
                    Some("tamanho") => (self.tamanho.clone(), v),
                    Some("dureza") => (self.dureza.clone(), v),
                    _ => (self.opacidade.clone(), v),
                };
                estado.update(cx, |s, cx| s.set_value(valor, window, cx));
                if let Some(s) = self.sessao_mut() {
                    match partes.get(1).copied() {
                        Some("tamanho") => s.pincel.raio = v.max(1.0),
                        Some("dureza") => s.pincel.dureza = v / 100.0,
                        _ => s.pincel.opacidade = v / 100.0,
                    }
                }
            }
            // cores trocar|padrao
            "cores" => match partes.get(1).copied() {
                Some("padrao") => self.cores_padrao(cx),
                _ => self.trocar_cores(cx),
            },
            // cor R G B
            "cor" => self.escolher_cor([numero(1) as u8, numero(2) as u8, numero(3) as u8], cx),
            "ferramenta" => match partes.get(1).copied().unwrap_or_default() {
                "pincel" => self.usar(Ferramenta::Pincel, cx),
                "borracha" => self.usar(Ferramenta::Borracha, cx),
                "carimbo" => self.usar(Ferramenta::Carimbo, cx),
                "conta-gotas" => self.usar_auxiliar(Auxiliar::ContaGotas, cx),
                "mover" => self.usar_auxiliar(Auxiliar::Mover, cx),
                "correcao" => self.usar_auxiliar(Auxiliar::Correcao, cx),
                "mao" => self.usar_auxiliar(Auxiliar::Mao, cx),
                "lupa" => self.usar_auxiliar(Auxiliar::Zoom, cx),
                "degrade" => self.usar_auxiliar(Auxiliar::Degrade, cx),
                "lata" => self.usar_auxiliar(Auxiliar::Lata, cx),
                "varinha" => self.usar_auxiliar(Auxiliar::Varinha, cx),
                "subexposicao" => {
                    let faixa = self.faixa;
                    self.usar(Ferramenta::Subexposicao(faixa), cx)
                }
                "superexposicao" => {
                    let faixa = self.faixa;
                    self.usar(Ferramenta::Superexposicao(faixa), cx)
                }
                "desfoque" => self.usar(Ferramenta::Desfoque, cx),
                "nitidez" => self.usar(Ferramenta::Nitidez, cx),
                "historico" => {
                    let alvo = partes.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);
                    self.ir_para_no_historico(alvo, cx)
                }
                outra => eprintln!("[roteiro] editor ferramenta {outra}?"),
            },
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
                                "{}{}:{}:{}:{:.0}%:{}tiles{}",
                                if i == s.ativa() { "*" } else { "" },
                                c.nome,
                                c.modo.chave(),
                                if c.visivel { "vis" } else { "oculta" },
                                c.opacidade * 100.0,
                                c.pixels.quantos(),
                                c.ajuste.map_or(String::new(), |a| format!(":{a:?}"))
                                    + &c.mascara.as_ref().map_or(String::new(), |m| format!(
                                        ":mascara({}fundo={},{}tiles{})",
                                        if i == s.ativa() && s.na_mascara() {
                                            "*"
                                        } else {
                                            ""
                                        },
                                        m.fundo,
                                        m.pixels.quantos(),
                                        if m.ativa { "" } else { ",desligada" }
                                    ))
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

    /// 🧪 Prepara a sessão (a cor, uma camada pintada) sem passar pela tela.
    #[cfg(test)]
    pub fn na_sessao_para_teste(
        &mut self,
        cx: &mut Context<Self>,
        fazer: impl FnOnce(&mut Sessao),
    ) {
        self.na_sessao(cx, fazer);
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
        let pintando = self.pintando
            || self.gesto_de_selecao.is_some()
            || self.pegando_cor
            || self.arrasto_do_mover.is_some()
            || self.gesto_de_transformacao.is_some()
            || self.traco_de_correcao.is_some()
            || self.degrade_em_curso.is_some();
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
                                    ed.mover_com(evento.position, evento.modifiers, cx);
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
                    // A caixa da transformação livre, com as quatro alças.
                    if let Some((caixa, t)) = sessao.transformacao() {
                        let cantos: Vec<(f32, f32)> =
                            t.cantos(&caixa).iter().map(|(x, y)| (*x, *y)).collect();
                        let mut fechado = cantos.clone();
                        fechado.push(cantos[0]);
                        palco = palco.child(tela.contorno(fechado));
                        for (x, y) in cantos {
                            let (sx, sy) = (v.x + x * v.escala, v.y + y * v.escala);
                            palco = palco.child(
                                div()
                                    .absolute()
                                    .left(px(sx - 4.0))
                                    .top(px(sy - 4.0))
                                    .size(px(8.0))
                                    .bg(gpui_kit::white())
                                    .border_1()
                                    .border_color(gpui_kit::black()),
                            );
                        }
                    }
                    // O traço do pincel de correção, por onde o remendo vai passar.
                    if let Some(traco) = &self.traco_de_correcao {
                        let largura = sessao.pincel.raio * 2.0 * v.escala;
                        let pontos: Vec<_> = traco
                            .iter()
                            .map(|(x, y)| (v.x + x * v.escala, v.y + y * v.escala))
                            .collect();
                        palco = palco.child(
                            canvas(
                                |_, _, _| {},
                                move |limites, _, window, _| {
                                    let (ox, oy) = (f(limites.origin.x), f(limites.origin.y));
                                    let mut caminho = PathBuilder::stroke(px(largura.max(1.0)));
                                    let p0 = pontos[0];
                                    caminho.move_to(gpui_kit::point(px(ox + p0.0), px(oy + p0.1)));
                                    // Um ponto só ainda precisa de um traço visível.
                                    caminho.line_to(gpui_kit::point(
                                        px(ox + p0.0 + 0.01),
                                        px(oy + p0.1),
                                    ));
                                    for (x, y) in &pontos[1..] {
                                        caminho.line_to(gpui_kit::point(px(ox + x), px(oy + y)));
                                    }
                                    if let Ok(c) = caminho.build() {
                                        window.paint_path(c, gpui_kit::white().opacity(0.45));
                                    }
                                },
                            )
                            .absolute()
                            .inset_0(),
                        );
                    }
                    // A linha do degradê, do começo ao fim.
                    if let Some((de, ate)) = self.degrade_em_curso {
                        let a = (v.x + de.0 * v.escala, v.y + de.1 * v.escala);
                        let b = (v.x + ate.0 * v.escala, v.y + ate.1 * v.escala);
                        palco = palco.child(
                            canvas(
                                |_, _, _| {},
                                move |limites, _, window, _| {
                                    let (ox, oy) = (f(limites.origin.x), f(limites.origin.y));
                                    for (largura, cor) in [
                                        (3.0, gpui_kit::black().opacity(0.6)),
                                        (1.5, gpui_kit::white()),
                                    ] {
                                        let mut caminho = PathBuilder::stroke(px(largura));
                                        caminho
                                            .move_to(gpui_kit::point(px(ox + a.0), px(oy + a.1)));
                                        caminho.line_to(gpui_kit::point(
                                            px(ox + b.0 + 0.01),
                                            px(oy + b.1),
                                        ));
                                        if let Ok(c) = caminho.build() {
                                            window.paint_path(c, cor);
                                        }
                                    }
                                },
                            )
                            .absolute()
                            .inset_0(),
                        );
                        for (x, y) in [a, b] {
                            palco = palco.child(
                                div()
                                    .absolute()
                                    .left(px(x - 3.5))
                                    .top(px(y - 3.5))
                                    .size(px(7.0))
                                    .rounded_full()
                                    .bg(gpui_kit::white())
                                    .border_1()
                                    .border_color(gpui_kit::black()),
                            );
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
                    let com_pincel = self.selecionando.is_none()
                        && self.auxiliar.is_none()
                        && !sessao.transformando();
                    // A mira do carimbo: de onde ele copia para o ponteiro.
                    if let (Some(ponteiro), Some(Ferramenta::Carimbo), true) =
                        (dentro, self.ferramenta(), com_pincel)
                    {
                        let mira = self
                            .na_foto_sem_limite(ponteiro)
                            .and_then(|(x, y)| sessao.mira_do_carimbo(x, y));
                        if let Some((mx, my)) = mira {
                            let (cx_, cy_) = (v.x + mx * v.escala, v.y + my * v.escala);
                            for (l, a) in [(14.0, 1.5), (1.5, 14.0)] {
                                palco = palco.child(
                                    div()
                                        .absolute()
                                        .left(px(cx_ - l / 2.0))
                                        .top(px(cy_ - a / 2.0))
                                        .w(px(l))
                                        .h(px(a))
                                        .bg(gpui_kit::white().opacity(0.9))
                                        .border_1()
                                        .border_color(gpui_kit::black().opacity(0.6)),
                                );
                            }
                        }
                    }
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
                } else if self.espaco.is_some() || self.auxiliar == Some(Auxiliar::Mao) {
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
                                ed.apertar_com(evento.position, evento.modifiers, cx);
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
            .when(self.transformando(), |barra| {
                barra
                    .child(
                        div()
                            .text_xs()
                            .text_color(tema.muted_foreground)
                            .child("Transformar — arraste dentro, nos cantos (⇧ livre) ou fora para girar (⇧ de 15°)"),
                    )
                    .child(
                        crate::estilo::botao_primario_pequeno("editor-aplicar-transformacao", cx)
                            .debug_selector(|| "editor-aplicar-transformacao".into())
                            .label("Aplicar")
                            .tooltip("Enter")
                            .on_click(cx.listener(|ed, _, _, cx| ed.aplicar_transformacao(cx))),
                    )
                    .child(
                        crate::estilo::botao_contorno_pequeno("editor-cancelar-transformacao", cx)
                            .debug_selector(|| "editor-cancelar-transformacao".into())
                            .label("Cancelar")
                            .tooltip("Esc")
                            .on_click(cx.listener(|ed, _, _, cx| ed.cancelar_transformacao(cx))),
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
        let ferramenta = self
            .ferramenta()
            .filter(|_| self.selecionando.is_none() && self.auxiliar.is_none());
        let cor_atual = self.sessao().map(|s| s.pincel.cor);
        // Como no Photoshop, as opções seguem a ferramenta: tamanho, dureza e
        // força só para quem pinta (o pincel de correção também).
        let pinta = ferramenta.is_some() || self.auxiliar == Some(Auxiliar::Correcao);
        let rotulo = |texto: &'static str| {
            div()
                .text_xs()
                .text_color(tema.muted_foreground)
                .child(texto)
        };
        // As opções da ferramenta (e as Propriedades do ajuste) rolam num
        // bloco que vai até pouco mais da metade da coluna: as camadas
        // embaixo sempre ficam à vista.
        let opcoes = div()
            .id("editor-opcoes")
            .flex()
            .flex_col()
            .gap(px(12.))
            .flex_shrink_0()
            .max_h(gpui_kit::relative(0.55))
            .overflow_y_scroll()
            .child(rotulo(self.nome_da_ferramenta()))
            .when_some(self.ajuste_da_camada(), |painel, a| {
                painel.child(self.propriedades_do_ajuste(a, cx))
            })
            .when(self.na_mascara(), |painel| {
                let nome = self
                    .sessao()
                    .map(|s| s.camada_ativa().nome.clone())
                    .unwrap_or_default();
                painel.child(
                    div()
                        .debug_selector(|| "editor-na-mascara".into())
                        .text_xs()
                        .px(px(6.))
                        .py(px(4.))
                        .rounded(crate::tema::canto(4.))
                        .bg(tema.muted)
                        .child(format!(
                            "Pintando na máscara de {nome}: preto esconde, branco revela. X troca as cores, D volta a preto e branco"
                        )),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(rotulo("Seleção"))
                    .child(div().flex_1())
                    .child(
                        crate::estilo::botao_icone_padrao("editor-preencher-conteudo", Icone::Sparkles)
                            .debug_selector(|| "editor-preencher-conteudo".into())
                            .tooltip("Refazer a seleção pelo conteúdo em volta (⇧⌫)")
                            .disabled(self.preenchendo || self.sessao().and_then(Sessao::selecao).is_none())
                            .on_click(cx.listener(|ed, _, _, cx| ed.preencher_a_selecao_pelo_conteudo(cx))),
                    )
                    .child(
                        crate::estilo::botao_fantasma_pequeno("editor-desmarcar", cx)
                            .debug_selector(|| "editor-desmarcar".into())
                            .label("Desmarcar")
                            .tooltip("Tirar a seleção (⌘D) · ⌘A tudo · ⇧⌘I inverter · Delete apaga · ⌥Delete preenche")
                            .disabled(self.sessao().and_then(Sessao::selecao).is_none())
                            .on_click(cx.listener(|ed, _, _, cx| ed.desmarcar(cx))),
                    ),
            )
            .when(self.auxiliar == Some(Auxiliar::Varinha), |painel| {
                let tolerancia = self.tolerancia_da_varinha.read(cx).value().start();
                painel
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_xs()
                            .text_color(tema.muted_foreground)
                            .child("Tolerância")
                            .child(format!("{tolerancia:.0}")),
                    )
                    .child(
                        div()
                            .h(px(20.))
                            .debug_selector(|| "editor-tolerancia".into())
                            .child(crate::estilo::slider(&self.tolerancia_da_varinha)),
                    )
                    .child(
                        div().debug_selector(|| "editor-contigua".into()).child(
                            gpui_kit::component::checkbox::Checkbox::new("editor-contigua")
                                .label("Contígua")
                                .checked(self.varinha_contigua)
                                .on_click(cx.listener(|ed, _: &bool, _, cx| {
                                    ed.alternar_varinha_contigua(cx)
                                })),
                        ),
                    )
            })
            // Modificar a seleção: o raio e os três comandos do Photoshop.
            .when(self.sessao().and_then(Sessao::selecao).is_some(), |painel| {
                let raio = self.raio_da_selecao.read(cx).value().start();
                painel
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_xs()
                            .text_color(tema.muted_foreground)
                            .child("Raio para modificar")
                            .child(format!("{raio:.0} px")),
                    )
                    .child(
                        div()
                            .h(px(20.))
                            .debug_selector(|| "editor-raio-da-selecao".into())
                            .child(crate::estilo::slider(&self.raio_da_selecao)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(
                                crate::estilo::botao_fantasma_pequeno("editor-difundir", cx)
                                    .debug_selector(|| "editor-difundir".into())
                                    .label("Difundir")
                                    .tooltip("Suavizar a borda pelo raio (⇧F6)")
                                    .on_click(cx.listener(|ed, _, _, cx| ed.difundir_selecao(cx))),
                            )
                            .child(
                                crate::estilo::botao_fantasma_pequeno("editor-expandir", cx)
                                    .debug_selector(|| "editor-expandir".into())
                                    .label("Expandir")
                                    .on_click(cx.listener(|ed, _, _, cx| ed.expandir_selecao(1, cx))),
                            )
                            .child(
                                crate::estilo::botao_fantasma_pequeno("editor-contrair", cx)
                                    .debug_selector(|| "editor-contrair".into())
                                    .label("Contrair")
                                    .on_click(cx.listener(|ed, _, _, cx| ed.expandir_selecao(-1, cx))),
                            ),
                    )
            })
            .when(
                matches!(
                    ferramenta,
                    Some(Ferramenta::Subexposicao(_) | Ferramenta::Superexposicao(_))
                ),
                |painel| {
                    painel.child(
                        div()
                            .debug_selector(|| "editor-faixa".into())
                            .child(crate::estilo::campo_pequeno(
                                Select::new(&self.seletor_de_faixa).xsmall(),
                            )),
                    )
                },
            )
            .when(pinta, |painel| {
                painel
                    .child(rotulo("Tamanho  [  ]"))
                    .child(
                        div()
                            .h(px(20.))
                            .debug_selector(|| "editor-tamanho".into())
                            .child(crate::estilo::slider(&self.tamanho)),
                    )
                    .child(rotulo("Dureza"))
                    .child(div().h(px(20.)).child(crate::estilo::slider(&self.dureza)))
                    .child(rotulo(match ferramenta {
                        Some(Ferramenta::Subexposicao(_) | Ferramenta::Superexposicao(_)) => {
                            "Exposição"
                        }
                        Some(Ferramenta::Desfoque | Ferramenta::Nitidez) => "Força",
                        _ => "Opacidade do pincel",
                    }))
                    .child(
                        div()
                            .h(px(20.))
                            .child(crate::estilo::slider(&self.opacidade)),
                    )
            })
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
;
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .w(px(260.))
            .h_full()
            .p(px(12.))
            .border_l_1()
            .border_color(tema.border)
            .child(opcoes)
            .child(div().h(px(1.)).bg(tema.border))
            .child({
                // 🗂️ Camadas e Histórico dividem o resto da coluna em abas, como os
                // painéis agrupados do Photoshop: um embaixo do outro, o
                // Histórico ficava abaixo da borda da janela.
                let esta = cx.entity();
                gpui_kit::component::tab::TabBar::new("editor-abas")
                    .segmented()
                    .small()
                    .selected_index(self.aba_do_painel)
                    .on_click(move |indice, _, cx| {
                        let indice = *indice;
                        esta.update(cx, |ed, cx| {
                            ed.aba_do_painel = indice;
                            cx.notify();
                        });
                    })
                    .child(
                        gpui_kit::component::tab::Tab::new()
                            .debug_selector(|| "editor-aba-camadas".into())
                            .label("Camadas"),
                    )
                    .child(
                        gpui_kit::component::tab::Tab::new()
                            .debug_selector(|| "editor-aba-historico".into())
                            .label("Histórico"),
                    )
            })
            .map(|painel| {
                if self.aba_do_painel == 1 {
                    painel.child(self.painel_do_historico(cx).into_any_element())
                } else {
                    painel.child(self.painel_de_camadas(cx).into_any_element())
                }
            })
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
                    .map(|c| {
                        (
                            c.nome.clone(),
                            c.visivel,
                            c.modo,
                            c.mascara.as_ref().map(|m| m.ativa),
                            c.ajuste.is_some(),
                        )
                    })
                    .collect::<Vec<_>>(),
                s.ativa(),
                true,
            ),
            None => (Vec::new(), 0, false),
        };
        let na_mascara = self.na_mascara();
        let tem_mascara = camadas.get(ativa).is_some_and(|c| c.3.is_some());
        let quantas = camadas.len();
        let renomeando = self.renomeando.clone();
        let miniaturas = self.miniaturas.clone();
        let das_mascaras = self.miniaturas_das_mascaras.clone();
        let lado = px(LADO_DA_MINIATURA as f32 * 0.75);
        // A moldura do alvo na cor de destaque do tema: em volta de uma
        // máscara branca, a do texto sumia.
        let cor_da_moldura = tema.ring;
        let cor_do_icone_de_ajuste = tema.muted;
        let linhas = camadas
            .into_iter()
            .enumerate()
            .rev()
            .map(|(i, (nome, visivel, modo, mascara, de_ajuste))| {
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
                    // A camada de ajuste não tem pixels: o ícone dela, como no
                    // Photoshop.
                    .when(de_ajuste, |d| {
                        d.child(
                            moldura(
                                div()
                                    .debug_selector(move || format!("editor-miniatura-{i}"))
                                    .size(lado)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(cor_do_icone_de_ajuste)
                                    .child(
                                        gpui_kit::component::Icon::new(Icone::Contrast)
                                            .size_4()
                                            .text_color(cor_da_moldura),
                                    ),
                                false,
                                cor_da_moldura,
                            ),
                        )
                    })
                    .when_some(
                        miniaturas.get(i).cloned().filter(|_| !de_ajuste),
                        |d, m| {
                            d.child(
                                moldura(
                                    div().debug_selector(move || format!("editor-miniatura-{i}")),
                                    escolhida && mascara.is_some() && !na_mascara,
                                    cor_da_moldura,
                                )
                                .child(img(m).object_fit(ObjectFit::Contain).w(lado).h(lado)),
                            )
                        },
                    )
                    // A máscara: clique escolhe, ⇧ + clique liga e desliga.
                    .when_some(
                        mascara.zip(das_mascaras.get(i).cloned().flatten()),
                        |d, (ligada, m)| {
                            d.child(
                                moldura(
                                    div()
                                        .id(("editor-mascara", i))
                                        .debug_selector(move || format!("editor-mascara-{i}"))
                                        .relative()
                                        .tooltip(move |window, cx| {
                                            gpui_kit::component::tooltip::Tooltip::new(if ligada {
                                                "Máscara — clique para pintar nela; ⇧ + clique desliga"
                                            } else {
                                                "Máscara desligada — ⇧ + clique liga"
                                            })
                                            .build(window, cx)
                                        }),
                                    escolhida && na_mascara,
                                    cor_da_moldura,
                                )
                                .child(img(m).object_fit(ObjectFit::Contain).w(lado).h(lado))
                                .when(!ligada, |d| {
                                    d.child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(gpui_kit::red())
                                            .text_lg()
                                            .child("✕"),
                                    )
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |ed, e: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        if e.modifiers.secondary() {
                                            ed.selecionar_da_camada(
                                                i,
                                                true,
                                                operacao_dos(e.modifiers),
                                                cx,
                                            );
                                        } else if e.modifiers.shift {
                                            ed.alternar_mascara_de(i, cx);
                                        } else {
                                            ed.escolher_mascara(i, cx);
                                        }
                                        window.focus(&ed.foco, cx);
                                    }),
                                ),
                            )
                        },
                    )
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
                            if evento.modifiers.secondary() {
                                // ⌘ + clique: a seleção do que a camada tem,
                                // sem trocar a escolhida (⇧ soma, ⌥ tira).
                                ed.selecionar_da_camada(i, false, operacao_dos(evento.modifiers), cx);
                            } else if evento.click_count >= 2 {
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
                    .child({
                        let ed = cx.entity();
                        botao(
                            "editor-camada-ajuste",
                            Icone::Contrast,
                            "Nova camada de ajuste",
                            pode_desfazer_alguma,
                        )
                        .dropdown_menu_with_anchor(
                            gpui_kit::Anchor::BottomLeft,
                            move |mut menu, _window, _cx| {
                                for a in editor_core::ajuste::TODOS {
                                    let ed = ed.clone();
                                    let id: &'static str = match a.chave() {
                                        "brilho" => "editor-ajuste-novo-brilho",
                                        "niveis" => "editor-ajuste-novo-niveis",
                                        "matiz" => "editor-ajuste-novo-matiz",
                                        _ => "editor-ajuste-novo-inverter",
                                    };
                                    menu = menu.item(
                                        crate::estilo::item_de_menu(id, a.nome(), None).on_click(
                                            move |_ev, _window, cx| {
                                                ed.update(cx, |ed, cx| {
                                                    ed.nova_camada_de_ajuste(a, cx)
                                                });
                                            },
                                        ),
                                    );
                                }
                                menu
                            },
                        )
                    })
                    .child(
                        botao(
                            "editor-camada-mascara",
                            Icone::LayerMask,
                            "Adicionar máscara (⌥ esconde tudo; com seleção, nasce dela)",
                            pode_desfazer_alguma && !tem_mascara,
                        )
                        .on_click(cx.listener(
                            |ed, e: &gpui_kit::ClickEvent, _, cx| {
                                ed.adicionar_mascara(e.modifiers().alt, cx)
                            },
                        )),
                    )
                    .child(
                        botao(
                            "editor-camada-nova",
                            Icone::Plus,
                            "Nova camada (⇧⌘N)",
                            pode_desfazer_alguma,
                        )
                        .on_click(cx.listener(|ed, _, _, cx| ed.nova_camada(cx))),
                    )
                    // O resto do menu de camadas do Photoshop, com os atalhos.
                    .child({
                        let ed = cx.entity();
                        let (pode_subir, pode_descer) =
                            (ativa + 1 < quantas, ativa > 0 && quantas > 1);
                        botao(
                            "editor-camada-mais",
                            Icone::EllipsisVertical,
                            "Mais: duplicar, subir, descer, mesclar",
                            pode_desfazer_alguma,
                        )
                        .dropdown_menu_with_anchor(
                            gpui_kit::Anchor::BottomLeft,
                            move |menu, _window, _cx| {
                                let item = |id: &'static str,
                                            rotulo: &'static str,
                                            ligado: bool,
                                            fazer: fn(
                                    &mut EditorDeFoto,
                                    &mut Context<EditorDeFoto>,
                                )| {
                                    let ed = ed.clone();
                                    crate::estilo::item_de_menu(id, rotulo, None)
                                        .disabled(!ligado)
                                        .on_click(move |_ev, _window, cx| {
                                            ed.update(cx, fazer);
                                        })
                                };
                                menu.item(item(
                                    "editor-camada-duplicar",
                                    "Duplicar a camada  ⌘J",
                                    true,
                                    |ed, cx| ed.duplicar_camada(cx),
                                ))
                                .item(item(
                                    "editor-camada-subir",
                                    "Subir a camada  ⌘]",
                                    pode_subir,
                                    |ed, cx| ed.mover_camada(1, cx),
                                ))
                                .item(item(
                                    "editor-camada-descer",
                                    "Descer a camada  ⌘[",
                                    pode_descer,
                                    |ed, cx| ed.mover_camada(-1, cx),
                                ))
                                .item(item(
                                    "editor-camada-mesclar",
                                    "Mesclar para baixo  ⌘E",
                                    pode_descer,
                                    |ed, cx| ed.mesclar_para_baixo(cx),
                                ))
                            },
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        botao(
                            "editor-camada-excluir",
                            Icone::Trash2,
                            if na_mascara {
                                "Excluir a máscara"
                            } else {
                                "Excluir a camada"
                            },
                            quantas > 1 || na_mascara,
                        )
                        .on_click(cx.listener(move |ed, _, _, cx| {
                            if na_mascara {
                                ed.excluir_mascara(cx)
                            } else {
                                ed.excluir_camada(cx)
                            }
                        })),
                    ),
            )
    }

    /// O painel Histórico do Photoshop: a abertura e cada passo, do mais
    /// antigo ao mais novo; o vigente realçado, os desfeitos apagados. Clicar
    /// num passo volta (ou avança) até ele.
    fn painel_do_historico(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let (nomes, posicao) = match self.sessao() {
            Some(s) => (
                std::iter::once("Abertura".to_string())
                    .chain(
                        s.historico()
                            .passos()
                            .iter()
                            .map(|p| p.descricao(s.documento())),
                    )
                    .collect::<Vec<_>>(),
                s.historico().posicao(),
            ),
            None => (Vec::new(), 0),
        };
        let linhas = nomes.into_iter().enumerate().map(|(i, nome)| {
            let vigente = i == posicao;
            let desfeito = i > posicao;
            div()
                .id(("editor-historico", i))
                .debug_selector(move || format!("editor-historico-{i}"))
                .px(px(6.))
                .py(px(2.))
                .rounded(crate::tema::canto(4.))
                .text_sm()
                .cursor_pointer()
                .when(vigente, |d| {
                    d.bg(tema.accent).text_color(tema.accent_foreground)
                })
                .when(desfeito, |d| {
                    d.text_color(tema.muted_foreground).opacity(0.6)
                })
                .when(!vigente, |d| d.hover(|d| d.bg(tema.muted)))
                .child(nome)
                .on_click(cx.listener(move |ed, _, _, cx| ed.ir_para_no_historico(i, cx)))
        });
        div()
            .id("editor-historico")
            .flex()
            .flex_col()
            .gap(px(1.))
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .children(linhas)
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
        let (l, a, bytes) = vista.ladrilho_bgra_com_folga(ladrilho);
        if let Some(imagem) = crate::imagem::de_bgra(l, a, bytes) {
            imagens.insert(ladrilho, imagem);
        }
    }
    sujos.len()
}

/// A operação da seleção pelos modificadores: ⇧ soma, ⌥ tira.
fn operacao_dos(m: gpui_kit::Modifiers) -> Operacao {
    if m.alt {
        Operacao::Subtrair
    } else if m.shift {
        Operacao::Somar
    } else {
        Operacao::Nova
    }
}

/// Um parâmetro de ajuste com o slider dele.
struct Parametro {
    chave: &'static str,
    nome: &'static str,
    min: f32,
    max: f32,
    passo: f32,
    inicial: f32,
}

/// Os parâmetros de todos os ajustes, com os limites do Photoshop. O índice é
/// o do slider em `EditorDeFoto::ajustes`.
const PARAMETROS_DE_AJUSTE: [Parametro; 8] = [
    Parametro {
        chave: "brilho",
        nome: "Brilho",
        min: -150.0,
        max: 150.0,
        passo: 1.0,
        inicial: 0.0,
    },
    Parametro {
        chave: "contraste",
        nome: "Contraste",
        min: -50.0,
        max: 100.0,
        passo: 1.0,
        inicial: 0.0,
    },
    Parametro {
        chave: "preto",
        nome: "Preto de entrada",
        min: 0.0,
        max: 253.0,
        passo: 1.0,
        inicial: 0.0,
    },
    Parametro {
        chave: "gama",
        nome: "Meios-tons (gama)",
        min: 0.1,
        max: 3.0,
        passo: 0.01,
        inicial: 1.0,
    },
    Parametro {
        chave: "branco",
        nome: "Branco de entrada",
        min: 2.0,
        max: 255.0,
        passo: 1.0,
        inicial: 255.0,
    },
    Parametro {
        chave: "matiz",
        nome: "Matiz",
        min: -180.0,
        max: 180.0,
        passo: 1.0,
        inicial: 0.0,
    },
    Parametro {
        chave: "saturacao",
        nome: "Saturação",
        min: -100.0,
        max: 100.0,
        passo: 1.0,
        inicial: 0.0,
    },
    Parametro {
        chave: "luminosidade",
        nome: "Luminosidade",
        min: -100.0,
        max: 100.0,
        passo: 1.0,
        inicial: 0.0,
    },
];

/// Os parâmetros do ajuste, como `(índice do slider, valor)`.
fn parametros_do_ajuste(ajuste: &Ajuste) -> Vec<(usize, f32)> {
    match *ajuste {
        Ajuste::BrilhoContraste { brilho, contraste } => vec![(0, brilho), (1, contraste)],
        Ajuste::Niveis {
            preto,
            gama,
            branco,
        } => vec![(2, preto), (3, gama), (4, branco)],
        Ajuste::MatizSaturacao {
            matiz,
            saturacao,
            luminosidade,
        } => vec![(5, matiz), (6, saturacao), (7, luminosidade)],
        Ajuste::Inverter => Vec::new(),
    }
}

/// O ajuste com o parâmetro `qual` trocado por `v`.
fn ajuste_com(ajuste: Ajuste, qual: usize, v: f32) -> Ajuste {
    match (ajuste, qual) {
        (Ajuste::BrilhoContraste { contraste, .. }, 0) => Ajuste::BrilhoContraste {
            brilho: v,
            contraste,
        },
        (Ajuste::BrilhoContraste { brilho, .. }, 1) => Ajuste::BrilhoContraste {
            brilho,
            contraste: v,
        },
        (Ajuste::Niveis { gama, branco, .. }, 2) => Ajuste::Niveis {
            preto: v,
            gama,
            branco,
        },
        (Ajuste::Niveis { preto, branco, .. }, 3) => Ajuste::Niveis {
            preto,
            gama: v,
            branco,
        },
        (Ajuste::Niveis { preto, gama, .. }, 4) => Ajuste::Niveis {
            preto,
            gama,
            branco: v,
        },
        (
            Ajuste::MatizSaturacao {
                saturacao,
                luminosidade,
                ..
            },
            5,
        ) => Ajuste::MatizSaturacao {
            matiz: v,
            saturacao,
            luminosidade,
        },
        (
            Ajuste::MatizSaturacao {
                matiz,
                luminosidade,
                ..
            },
            6,
        ) => Ajuste::MatizSaturacao {
            matiz,
            saturacao: v,
            luminosidade,
        },
        (
            Ajuste::MatizSaturacao {
                matiz, saturacao, ..
            },
            7,
        ) => Ajuste::MatizSaturacao {
            matiz,
            saturacao,
            luminosidade: v,
        },
        (outro, _) => outro,
    }
}

/// A miniatura onde o pincel pinta leva a moldura, como no Photoshop.
fn moldura<E: Styled>(elemento: E, alvo: bool, cor: gpui_kit::Hsla) -> E {
    elemento
        .flex_none()
        .p(px(1.))
        .border_2()
        .border_color(if alvo {
            cor
        } else {
            gpui_kit::transparent_black()
        })
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
                    // 🚨 Horizontal ou vertical, e nada de `signum`: no Rust
                    // `0.0_f32.signum()` é 1, e cada traço saía em diagonal
                    // (visto no app real).
                    let horizontal = (by - ay).abs() < (bx - ax).abs();
                    let (dx, dy) = if horizontal { (1.0, 0.0) } else { (0.0, 1.0) };
                    // Os traços alinhados à grade da tela, para os vizinhos
                    // emendarem no mesmo compasso.
                    let inicio = if horizontal { ax.min(bx) } else { ay.min(by) };
                    let mut t = (inicio / 8.0).ceil() * 8.0 - inicio;
                    let (sx, sy) = (ax.min(bx), ay.min(by));
                    while t < comprimento {
                        let fim = (t + 4.0).min(comprimento);
                        let (px0, py0) = (sx + dx * t, sy + dy * t);
                        let (px1, py1) = (sx + dx * fim, sy + dy * fim);
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
        // 🚨 **A emenda entre dois ladrilhos** (achada duas vezes no app real).
        // Com a posição fracionária, a borda caía no meio de um pixel e abria
        // uma fresta escura (27/set/2026); a sobra de um pixel que a corrigiu
        // fez outra linha, e mesmo na grade do dispositivo ela voltava
        // (05/out/2026): ampliada, o GPU lê meio texel além da borda da
        // textura, e ali está o vizinho dela **no atlas**, que muda a cada
        // ladrilho reenviado. Agora cada textura traz um pixel de folga (os
        // pixels de verdade do vizinho na vista) e é desenhada recortada
        // (`overflow_hidden`) no retângulo exato do ladrilho, com as bordas
        // arredondadas pela mesma conta dos dois lados: o meio texel lido além
        // da borda é foto, e a sobra fica fora do recorte.
        let alinhar = |v: f32| (v * dpr).round() / dpr;
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
                    ox + r.direita() as f32 * escala,
                    oy + r.baixo() as f32 * escala,
                );
                // Fora do palco não desenha: ampliada, quase toda a vista
                // inteira fica de fora.
                if x1 < 0.0 || y1 < 0.0 || x0 > self.area.largura || y0 > self.area.altura {
                    continue;
                }
                let (esq, topo, dir, baixo) = (alinhar(x0), alinhar(y0), alinhar(x1), alinhar(y1));
                saida.push(
                    div()
                        .absolute()
                        .left(px(esq))
                        .top(px(topo))
                        .w(px(dir - esq))
                        .h(px(baixo - topo))
                        .overflow_hidden()
                        .child(
                            img(imagem.clone())
                                .object_fit(ObjectFit::Fill)
                                .absolute()
                                .left(px(x0 - escala - esq))
                                .top(px(y0 - escala - topo))
                                .w(px((r.largura + 2) as f32 * escala))
                                .h(px((r.altura + 2) as f32 * escala)),
                        )
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
        // Os sliders do ajuste acompanham a camada escolhida e o desfazer.
        if let Some(a) = self.ajuste_da_camada() {
            for (qual, v) in parametros_do_ajuste(&a) {
                let estado = &self.ajustes[qual];
                if (estado.read(cx).value().start() - v).abs() > 0.004 {
                    estado.update(cx, |s, cx| s.set_value(v, window, cx));
                }
            }
        }
        // O seletor de cor acompanha a cor do pincel (amostras, conta-gotas).
        let cor = self.sessao().map(|s| s.pincel.cor);
        if cor.is_some() && cor != self.cor_mostrada {
            self.cor_mostrada = cor;
            let hsla = hsla_de(cor.unwrap_or_default());
            self.seletor_de_cor
                .update(cx, |s, cx| s.set_value(hsla, window, cx));
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
            .on_action(cx.listener(|ed, _: &CamadaViaRecorte, _, cx| ed.camada_via_recorte(cx)))
            .on_action(cx.listener(|ed, _: &TransformacaoLivre, _, cx| ed.transformar(cx)))
            .on_action(
                cx.listener(|ed, _: &UsarCorrecao, _, cx| ed.usar_auxiliar(Auxiliar::Correcao, cx)),
            )
            .on_action(cx.listener(|ed, _: &PreencherPeloConteudo, _, cx| {
                ed.preencher_a_selecao_pelo_conteudo(cx)
            }))
            .on_action(
                cx.listener(|ed, _: &AplicarTransformacao, _, cx| ed.aplicar_transformacao(cx)),
            )
            .on_action(
                cx.listener(|ed, _: &CancelarTransformacao, _, cx| ed.cancelar_transformacao(cx)),
            )
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
            .on_action(cx.listener(|ed, _: &UsarCarimbo, _, cx| ed.usar(Ferramenta::Carimbo, cx)))
            .on_action(cx.listener(|ed, _: &UsarContaGotas, _, cx| {
                ed.usar_auxiliar(Auxiliar::ContaGotas, cx)
            }))
            .on_action(
                cx.listener(|ed, _: &UsarMover, _, cx| ed.usar_auxiliar(Auxiliar::Mover, cx)),
            )
            .on_action(cx.listener(|ed, _: &UsarSubexposicao, _, cx| {
                let faixa = ed.faixa;
                ed.usar(Ferramenta::Subexposicao(faixa), cx)
            }))
            .on_action(cx.listener(|ed, _: &UsarSuperexposicao, _, cx| {
                let faixa = ed.faixa;
                ed.usar(Ferramenta::Superexposicao(faixa), cx)
            }))
            .on_action(cx.listener(|ed, _: &UsarDesfoque, _, cx| ed.usar(Ferramenta::Desfoque, cx)))
            .on_action(cx.listener(|ed, _: &UsarNitidez, _, cx| ed.usar(Ferramenta::Nitidez, cx)))
            .on_action(
                cx.listener(|ed, _: &UsarDegrade, _, cx| ed.usar_auxiliar(Auxiliar::Degrade, cx)),
            )
            .on_action(cx.listener(|ed, _: &UsarLata, _, cx| ed.usar_auxiliar(Auxiliar::Lata, cx)))
            .on_action(
                cx.listener(|ed, _: &UsarVarinha, _, cx| ed.usar_auxiliar(Auxiliar::Varinha, cx)),
            )
            .on_action(cx.listener(|ed, _: &DifundirSelecao, _, cx| ed.difundir_selecao(cx)))
            .on_action(cx.listener(|ed, _: &TrocarCores, _, cx| ed.trocar_cores(cx)))
            .on_action(cx.listener(|ed, _: &CoresPadrao, _, cx| ed.cores_padrao(cx)))
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
                    .child(self.barra_de_ferramentas(cx))
                    .child(self.palco(window, cx))
                    .child(self.painel(cx)),
            )
            .children(pergunta)
    }
}
