//! A janela do editor em camadas — uma por foto.
//!
//! ```text
//! ┌ Arquivo Editar Camada Selecionar Filtro Visualizar Janela Ajuda            (menus.rs) ┐
//! ├ opções da ferramenta — altura fixa                                         (opcoes.rs) ┤
//! ├──┬ foto.jpg @ 32% (RGB/8) • × ─────────────────────┬ Cor | Amostras ───────────────────┤
//! │▣ │                                               │ Propriedades | Pincel | Histórico │
//! │▢◢│   a foto (vista em ladrilhos)                 │ Camadas       (area_de_trabalho.rs) │
//! │■□│  (ferramentas.rs)                             │                                   │
//! └──┴───────────────────────────────────────────────┴───────────────────────────────────┘
//!   zoom · 100% · Encaixar · medidas · aviso                        Salvo  Salvar (status.rs)
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
//! ampliam em torno do cursor, a roda e o Espaço + arrastar movem (Z é a Lupa),
//! `⌘=` `⌘−` `⌘0` `⌘⌥0`. Com a foto ampliada além da vista, a janela pede uma
//! **lupa**: a vista só do pedaço visível, montada fora da thread da tela, por
//! cima da vista inteira. A partir de 8 pixels do dispositivo por pixel, cada
//! pixel é um quadrado nítido (o `pixels_nitidos` da Revelação).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod aparencia;
pub mod area_de_trabalho;
mod camadas;
pub mod ferramentas;
pub mod filtro;
mod mascara_e_cadeados;
mod menus;
mod opcoes;
mod paineis;
mod painel_do_preenchimento;
mod preencher;
mod reguas;
mod status;
mod transferencia;
pub use area_de_trabalho::QualPainel;
pub use ferramentas::{letra_de, DefDeFerramenta, FERRAMENTAS};
pub use menus::atalho_da_acao as menus_atalho;
pub use painel_do_preenchimento::{
    AlvoDoPincel, EspacoDoPreenchimento, EstadoDoCalculo, FerramentaDoPreenchimento,
};

use editor_core::selecao::{caixa_do_arrasto, medida_da_caixa};
use editor_core::sessao::PedidoDeLupa;
use editor_core::vista::Vista as VistaDoEditor;
use editor_core::Ajuste;
use editor_core::{
    Acabamento, AmostraDaVarinha, BaseRef, Documento, Estilo, Ferramenta, Forma, Historico, Modo,
    OpcoesDaVarinha, Operacao, Retangulo, Sessao, Transformacao, VarinhaRecusada, VersaoEditada,
};
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme, Disableable, Sizable};
use gpui_kit::{
    canvas, div, img, prelude::*, px, AnyElement, Bounds, Context, Entity, EventEmitter,
    FocusHandle, Focusable, KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ObjectFit, PathBuilder, PinchEvent, Pixels, Point, RenderImage, ScrollWheelEvent,
    SharedString, Subscription, Task, Window,
};
use image::DynamicImage;

use super::giro;
use super::porta::{Abertura, Edicoes, FotoDoEditor};
use super::{
    Afastar, AlternarAntesDepois, AlternarCamada, AlternarInterface, AlternarMascaraDeCorte,
    AlternarPaineis, ApagarSelecao, AplicarTransformacao, Aproximar, CamadaDeBaixo, CamadaDeCima,
    CamadaViaRecorte, CancelarTransformacao, CoresPadrao, DescerCamada, DesfazerNoEditor,
    Desmarcar, DifundirSelecao, DuplicarCamada, DurezaMaior, DurezaMenor, Encaixar, FecharEditor,
    GrupoB, GrupoE, GrupoG, GrupoH, GrupoI, GrupoJ, GrupoL, GrupoM, GrupoO, GrupoR, GrupoS, GrupoV,
    GrupoW, GrupoZ, InverterSelecao, Liquidificar, MesclarParaBaixo, NovaCamada, PincelMaior,
    PincelMenor, PreencherPeloConteudo, PreencherSelecao, ProximaDoGrupoG, ProximaDoGrupoJ,
    ProximaDoGrupoL, ProximaDoGrupoM, ProximaDoGrupoO, RefazerNoEditor, SalvarNoEditor,
    SegurarAMao, SelecionarTudo, SubirCamada, TransformacaoLivre, TrocarCores, UmPorUm, CONTEXTO,
};
use super::{AjusteCurvas, AjusteMatiz, AjusteNiveis, AlternarReguas, Reselecionar};
use super::{
    AlternarRubi, BloquearTransparencia, CarimbarVisivel, Colar, ColarNoLugar, Copiar,
    CopiarMesclado, Inverter, Recortar,
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

/// As amostras em `Hsla`, para as cores em destaque dos seletores.
fn amostras_em_hsla() -> Vec<gpui_kit::Hsla> {
    AMOSTRAS.iter().map(|c| hsla_de(*c)).collect()
}

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
    /// A última montagem do palco girado (só os ladrilhos refeitos).
    pub palco_girado: Option<Duration>,
}

/// Um segmento da borda da seleção, em pixels da foto: `(x0, y0, x1, y1)`.
type Segmento = (u32, u32, u32, u32);

/// De que estado a prévia do carimbo é: versão da sessão, ponteiro (pixels da
/// foto), raio, mira e amostra.
type ChaveDaPrevia = (
    u64,
    (i32, i32),
    u32,
    (i32, i32),
    editor_core::AmostraDoCarimbo,
);

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
    /// J (⇧J até ele) — o Remendo: arrastar a seleção até a pele limpa refaz
    /// a área selecionada com a textura de lá e a cor daqui. Sem seleção (ou
    /// fora dela), o arrasto desenha a seleção à mão, como o laço.
    Remendo,
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
    /// R — girar a vista: arrastar gira a foto **na tela** (⇧ de 15 em 15
    /// graus); nenhum pixel muda. Ver `giro.rs`.
    GirarVista,
}

/// O atalho de mostrar e esconder a camada, como a plataforma escreve.
const MOSTRAR: &str = if cfg!(target_os = "macos") {
    "⌘,"
} else {
    "Ctrl+,"
};

/// Uma ferramenta da barra: as que pintam, as de seleção e as auxiliares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item {
    F(Ferramenta),
    S(TipoDeSelecao),
    A(Auxiliar),
}

impl Item {
    /// A mesma ferramenta, sem olhar a faixa de tons da subexposição.
    fn mesma(&self, outra: &Item) -> bool {
        match (self, outra) {
            (Item::F(a), Item::F(b)) => std::mem::discriminant(a) == std::mem::discriminant(b),
            _ => self == outra,
        }
    }
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

/// As ferramentas de seleção (M, ⇧M, L, ⇧L).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoDeSelecao {
    Retangulo,
    Elipse,
    Laco,
    /// Clique a clique: cada clique é um vértice; fecha clicando perto do
    /// primeiro, com duplo clique ou Enter; ⌫ tira o último, Esc cancela.
    LacoPoligonal,
}

impl TipoDeSelecao {
    pub const TODOS: [TipoDeSelecao; 4] = [
        TipoDeSelecao::Retangulo,
        TipoDeSelecao::Elipse,
        TipoDeSelecao::Laco,
        TipoDeSelecao::LacoPoligonal,
    ];

    fn indice(self) -> usize {
        self as usize
    }

    /// Retangular e elíptica têm estilo (proporção ou tamanho fixos).
    pub fn tem_estilo(self) -> bool {
        matches!(self, TipoDeSelecao::Retangulo | TipoDeSelecao::Elipse)
    }

    /// O retângulo é sempre exato: não tem antisserrilhado.
    pub fn suaviza(self) -> bool {
        self != TipoDeSelecao::Retangulo
    }
}

/// O tipo do estilo escolhido na barra (os números moram em [`OpcoesDaForma`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TipoDeEstilo {
    #[default]
    Normal,
    Proporcao,
    Tamanho,
}

impl TipoDeEstilo {
    const TODOS: [TipoDeEstilo; 3] = [
        TipoDeEstilo::Normal,
        TipoDeEstilo::Proporcao,
        TipoDeEstilo::Tamanho,
    ];

    fn chave(self) -> &'static str {
        match self {
            TipoDeEstilo::Normal => "normal",
            TipoDeEstilo::Proporcao => "proporcao",
            TipoDeEstilo::Tamanho => "tamanho",
        }
    }

    fn nome(self) -> &'static str {
        match self {
            TipoDeEstilo::Normal => "Normal",
            TipoDeEstilo::Proporcao => "Proporção fixa",
            TipoDeEstilo::Tamanho => "Tamanho fixo",
        }
    }
}

/// As opções de uma ferramenta de seleção — valem para a **próxima** seleção
/// e não entram no desfazer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpcoesDaForma {
    pub acabamento: Acabamento,
    pub estilo: TipoDeEstilo,
    /// A razão largura:altura da proporção fixa.
    pub proporcao: (f32, f32),
    /// Largura e altura do tamanho fixo, em pixels do documento.
    pub tamanho: (u32, u32),
}

impl Default for OpcoesDaForma {
    /// O padrão do Photoshop: Normal, difusão 0, antisserrilhado ligado,
    /// proporção 1:1 e tamanho 64×64.
    fn default() -> Self {
        Self {
            acabamento: Acabamento::default(),
            estilo: TipoDeEstilo::Normal,
            proporcao: (1.0, 1.0),
            tamanho: (64, 64),
        }
    }
}

impl OpcoesDaForma {
    pub fn estilo(&self) -> Estilo {
        match self.estilo {
            TipoDeEstilo::Normal => Estilo::Normal,
            TipoDeEstilo::Proporcao => Estilo::Proporcao {
                largura: self.proporcao.0,
                altura: self.proporcao.1,
            },
            TipoDeEstilo::Tamanho => Estilo::Tamanho {
                largura: self.tamanho.0,
                altura: self.tamanho.1,
            },
        }
    }
}

/// O que o menu "Modificar seleção" faz na seleção que já existe — um passo
/// do desfazer cada um, com o valor em pixels do documento.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modificacao {
    Difundir,
    Expandir,
    Contrair,
}

impl Modificacao {
    fn indice(self) -> usize {
        self as usize
    }

    fn titulo(self) -> &'static str {
        match self {
            Modificacao::Difundir => "Difundir seleção",
            Modificacao::Expandir => "Expandir seleção",
            Modificacao::Contrair => "Contrair seleção",
        }
    }

    fn rotulo(self) -> &'static str {
        match self {
            Modificacao::Difundir => "Raio da difusão",
            Modificacao::Expandir => "Expandir em",
            Modificacao::Contrair => "Contrair em",
        }
    }
}

/// O nome do modo na barra de opções.
pub fn nome_do_modo(operacao: Operacao) -> &'static str {
    match operacao {
        Operacao::Nova => "Nova",
        Operacao::Somar => "Adicionar",
        Operacao::Subtrair => "Subtrair",
        Operacao::Intersecao => "Intersectar",
    }
}

const MODOS: [Operacao; 4] = [
    Operacao::Nova,
    Operacao::Somar,
    Operacao::Subtrair,
    Operacao::Intersecao,
];

/// A distância, em pontos da tela, em que o clique "pega" o primeiro vértice
/// do laço poligonal e fecha.
const FECHAR_O_POLIGONO: f32 = 8.0;

/// Uma seleção sendo desenhada: os pontos em pixels da foto (no retângulo e
/// na elipse, o primeiro e o último são os cantos).
/// O que o arrasto faz na caixa da transformação livre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParteDaCaixa {
    Dentro,
    /// Uma das oito alças (`Transformacao::alcas`): canto proporcional (⇧
    /// solta), meio de lado só naquele eixo; a oposta fica parada, ou o ponto
    /// de referência com ⌥.
    Alca(usize),
    /// Fora da caixa: gira em volta do ponto de referência (⇧ de 15 em 15
    /// graus).
    Fora,
    /// O ponto de referência: arrastar o leva para outro lugar.
    Referencia,
}

/// O arrasto na malha do Deformar: um dos 16 pontos, ou um ponto da
/// superfície (`u`, `v`) puxado por dentro; de onde começou (pixels da foto) e a
/// malha de então.
#[derive(Clone, Copy)]
struct GestoDeMalha {
    pega: Option<editor_core::deformar::Pega>,
    uv: Option<(f32, f32)>,
    inicio: (f32, f32),
    malha: editor_core::Malha,
}

/// Um arrasto na caixa: o que começou a fazer, onde (pixels da foto), e a
/// transformação de então.
#[derive(Clone, Copy)]
struct GestoDeTransformacao {
    parte: ParteDaCaixa,
    inicio: (f32, f32),
    t: Transformacao,
    /// O ponto de referência de quando o arrasto começou (caixa de origem).
    referencia: (f32, f32),
}

struct GestoDeSelecao {
    tipo: TipoDeSelecao,
    /// O modo deste gesto: o da barra, ou o dos modificadores apertados no
    /// começo (só neste gesto — a barra não muda).
    operacao: Operacao,
    /// O estilo e o acabamento de quando o gesto começou.
    estilo: Estilo,
    acabamento: Acabamento,
    /// No laço poligonal, os vértices; nos outros, o começo e o ponto de agora
    /// (no laço livre, o caminho todo).
    pontos: Vec<(f32, f32)>,
    /// O laço poligonal: onde o ponteiro está — a prévia do próximo segmento.
    proximo: Option<(f32, f32)>,
    /// ⇧ segurado no arrasto: quadrado ou círculo.
    quadrado: bool,
    /// ⌥ segurado no arrasto: desenha a partir do centro.
    do_centro: bool,
}

impl GestoDeSelecao {
    /// Os dois cantos do retângulo (ou da caixa da elipse), com o estilo, ⇧ e
    /// ⌥ — "Constrain marquee to square" e "Draw marquee from center" da
    /// tabela da Adobe (`selecao::caixa_do_arrasto` tem a tabela da conversa
    /// deles com a proporção e o tamanho fixos).
    fn cantos(&self) -> ((f32, f32), (f32, f32)) {
        let a = self.pontos[0];
        let b = *self.pontos.last().unwrap_or(&a);
        caixa_do_arrasto(a, b, self.estilo, self.quadrado, self.do_centro)
    }

    /// Largura e altura da caixa em pixels do documento (retângulo e elipse) —
    /// o que a tela mostra junto do ponteiro.
    fn medida(&self) -> Option<(u32, u32)> {
        self.tipo.tem_estilo().then(|| {
            let (a, b) = self.cantos();
            medida_da_caixa(a, b)
        })
    }

    fn forma(&self) -> Forma {
        let (a, b) = self.cantos();
        let (x0, x1) = (a.0.min(b.0).round(), a.0.max(b.0).round());
        let (y0, y1) = (a.1.min(b.1).round(), a.1.max(b.1).round());
        match self.tipo {
            TipoDeSelecao::Retangulo => {
                let (cx0, cy0) = (x0.max(0.0), y0.max(0.0));
                Forma::Retangulo(Retangulo::novo(
                    cx0 as u32,
                    cy0 as u32,
                    (x1.max(0.0) - cx0) as u32,
                    (y1.max(0.0) - cy0) as u32,
                ))
            }
            // A caixa inteira, mesmo passando da foto: a elipse não achata.
            TipoDeSelecao::Elipse => Forma::ElipseNaCaixa(x0, y0, x1, y1),
            TipoDeSelecao::Laco | TipoDeSelecao::LacoPoligonal => Forma::Laco(self.pontos.clone()),
        }
    }

    /// O contorno para desenhar enquanto arrasta, em pixels da foto.
    fn contorno(&self) -> Vec<(f32, f32)> {
        if self.tipo == TipoDeSelecao::LacoPoligonal {
            // Os segmentos fixos e o próximo, até o ponteiro.
            let mut v = self.pontos.clone();
            v.extend(self.proximo);
            if v.len() == 1 {
                v.push(v[0]);
            }
            return v;
        }
        let (a, b) = self.cantos();
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
            _ => {
                let mut v = self.pontos.clone();
                v.push(self.pontos[0]);
                v
            }
        }
    }

    /// O clique cai perto do primeiro vértice do laço poligonal (`folga` em
    /// pixels da foto)?
    fn perto_do_primeiro(&self, p: (f32, f32), folga: f32) -> bool {
        self.pontos.len() >= 3 && (p.0 - self.pontos[0].0).hypot(p.1 - self.pontos[0].1) <= folga
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
    /// O modo escolhido na barra de opções da seleção. Os modificadores (⇧,
    /// ⌥, ⇧⌥) trocam só durante o gesto, sem mexer nele.
    modo_de_selecao: Operacao,
    /// As opções de cada ferramenta de seleção (`TipoDeSelecao::indice`).
    opcoes_das_formas: [OpcoesDaForma; 4],
    /// As opções da varinha (W): tolerância, contígua, antisserrilhado e de
    /// onde ela lê a cor.
    opcoes_da_varinha: OpcoesDaVarinha,
    /// Os campos da barra de opções: difusão, largura e altura (proporção ou
    /// tamanho fixo) e a tolerância da varinha.
    campo_da_difusao: Entity<InputState>,
    campo_da_largura: Entity<InputState>,
    campo_da_altura: Entity<InputState>,
    campo_da_tolerancia: Entity<InputState>,
    seletor_de_estilo: Entity<SelectState<Vec<Opcao>>>,
    seletor_de_amostra: Entity<SelectState<Vec<Opcao>>>,
    /// De que ferramenta e estilo os campos estão mostrando os valores — para
    /// só reescrevê-los quando a ferramenta ou o estilo mudam.
    campos_de: Option<(TipoDeSelecao, TipoDeEstilo)>,
    /// A amostra da varinha que o Select mostra.
    amostra_mostrada: Option<AmostraDaVarinha>,
    /// O modo da ferramenta (pincel e carimbo) e a amostra do carimbo, nos
    /// Selects do kit.
    seletor_do_modo_da_ferramenta: Entity<SelectState<Vec<Opcao>>>,
    seletor_da_amostra_do_carimbo: Entity<SelectState<Vec<Opcao>>>,
    /// "Mostrar sobreposição" do carimbo: a origem dentro do círculo do
    /// pincel (o "Show Overlay" do Photoshop, recortado no círculo).
    sobreposicao_do_carimbo: bool,
    /// A prévia da origem montada para o ponteiro de agora: de que estado ela
    /// é, o recorte da foto que ela cobre e a imagem.
    previa_do_carimbo: Option<(ChaveDaPrevia, Retangulo, Arc<RenderImage>)>,
    /// O "Modificar seleção" aberto: o comando e o campo do valor.
    modificando: Option<(Modificacao, Entity<InputState>)>,
    /// O último valor usado em cada comando de "Modificar seleção", em px.
    valores_da_modificacao: [u32; 3],
    modo: Entity<SelectState<Vec<Opcao>>>,
    /// O modo que o Select mostra — para só mexer nele quando mudar.
    modo_mostrado: Option<Modo>,
    /// A faixa de tons da subexposição e da superexposição (Select do kit).
    faixa: editor_core::pincel::Faixa,
    seletor_de_faixa: Entity<SelectState<Vec<Opcao>>>,
    /// O Preenchimento sensível ao conteúdo aberto (o painel no lugar das
    /// opções), e o que ele lembra entre aberturas: o método e o backend.
    area_do_preenchimento: Option<EspacoDoPreenchimento>,
    metodo_do_preenchimento: preenchimento::Metodo,
    backend_da_ia: ia_local::execucao::Backend,
    /// "Aplicar numa camada nova" — lembrado entre aberturas.
    /// As escolhas do Content-Aware que voltam na próxima abertura, e os
    /// controles delas no painel.
    preferencias_do_preenchimento: painel_do_preenchimento::PreferenciasDoPreenchimento,
    opacidade_da_amostragem: Entity<SliderState>,
    seletor_da_cor_da_amostragem: Entity<SelectState<Vec<Opcao>>>,
    seletor_da_saida: Entity<SelectState<Vec<Opcao>>>,
    seletor_de_metodo: Entity<SelectState<Vec<Opcao>>>,
    seletor_de_backend: Entity<SelectState<Vec<Opcao>>>,
    /// A margem de contexto da IA (%) e a suavização da borda (px).
    contexto_da_ia: Entity<SliderState>,
    suavizacao: Entity<SliderState>,
    tarefa_do_painel: Option<Task<()>>,
    vigia_do_painel: Option<Task<()>>,
    tarefa_do_modelo: Option<Task<()>>,
    /// O documento e a coluna dos painéis, com a borda arrastável do kit; a
    /// largura vai na área de trabalho (`area_de_trabalho.rs`).
    colunas: Entity<gpui_kit::component::resizable::ResizableState>,
    /// A área de trabalho: o dock dos painéis (montado no primeiro quadro) e
    /// a arrumação que vai para o disco.
    area_de_trabalho: Option<area_de_trabalho::AreaDeTrabalho>,
    arranjo: area_de_trabalho::Arranjo,
    _gravacao_da_area: Option<Task<()>>,
    gravacoes_da_area: usize,
    /// `Tab` (barra, opções e painéis) e `⇧Tab` (só os painéis): passageiros.
    interface_oculta: bool,
    paineis_ocultos: bool,
    /// A barra de ferramentas em duas colunas.
    barra_em_duas_colunas: bool,
    /// O que cada grupo da barra mostra, o flyout aberto, o aperto que pode
    /// virar flyout e as caixas dos botões (do último quadro).
    mostrada_na_barra: [Option<Item>; ferramentas::QUANTOS_GRUPOS],
    flyout: Option<ferramentas::Flyout>,
    aperto_na_barra: Option<usize>,
    abriu_pelo_aperto: bool,
    _tarefa_do_aperto: Option<Task<()>>,
    caixas_da_barra: [Bounds<Pixels>; ferramentas::QUANTOS_GRUPOS],
    /// A cor de fundo no seletor da barra (e qual cor o painel Cor edita).
    seletor_de_fundo: Entity<ColorPickerState>,
    fundo_mostrado: Option<[u8; 3]>,
    cor_do_painel_e_o_fundo: bool,
    rgb_do_painel: [Entity<SliderState>; 3],
    /// O zoom editável da barra de status, e o que ele mostra.
    campo_do_zoom: Entity<InputState>,
    zoom_mostrado: Option<String>,
    /// O ângulo da Girar vista na barra de opções.
    campo_do_giro: Entity<InputState>,
    giro_mostrado: Option<f32>,
    /// A opacidade da camada no painel Camadas (o número; o slider é o de
    /// sempre, no popover).
    campo_da_opacidade_da_camada: Entity<InputState>,
    opacidade_mostrada: Option<f32>,
    /// A Lupa em "Reduzir" (o ⌥ inverte no clique).
    lupa_reduz: bool,
    /// Largura e altura juntas na barra da transformação.
    proporcao_travada: bool,
    /// A Ajuda dos atalhos aberta.
    mostrando_atalhos: bool,
    /// O diálogo "Preencher" (⇧⌫) e o conteúdo dele.
    dialogo_de_preencher: bool,
    seletor_do_conteudo: Entity<SelectState<Vec<Opcao>>>,
    /// A camada sendo renomeada e o campo do nome.
    renomeando: Option<(usize, Entity<InputState>)>,
    medidas: Medidas,
    _assinaturas: Vec<Subscription>,
    _assinatura_do_nome: Option<Subscription>,
    _assinatura_da_modificacao: Option<Subscription>,
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
    /// O ponto de referência da caixa do ⌘T (na caixa de origem); `None` é o
    /// centro. Some ao abrir e ao fechar a caixa.
    referencia_da_caixa: Option<(f32, f32)>,
    /// Os campos X, Y, L, A e ângulo da barra da transformação, e se a janela
    /// está escrevendo neles (para a mudança não voltar como gesto).
    campos_da_transformacao: [Entity<InputState>; 5],
    escrevendo_os_campos: bool,
    /// De que números os campos estão mostrando.
    numeros_mostrados: Option<(f32, f32, f32, f32, f32)>,
    /// O traço do pincel de correção em curso (pixels da foto).
    traco_de_correcao: Option<Vec<(f32, f32)>>,
    /// O degradê sendo arrastado: começo e fim, em pixels da foto.
    degrade_em_curso: Option<((f32, f32), (f32, f32))>,
    /// A miniatura da máscara de cada camada (`None` sem máscara).
    miniaturas_das_mascaras: Vec<Option<Arc<RenderImage>>>,
    /// A miniatura da fotografia base — a linha "Fundo" das Camadas. Feita
    /// uma vez: a base não muda (C28).
    miniatura_do_fundo: Option<Arc<RenderImage>>,
    /// O diálogo de Filtro › Desfoque gaussiano / Máscara de nitidez.
    filtro: filtro::Estado,
    /// Um preenchimento por conteúdo calculando em segundo plano.
    preenchendo: bool,
    _tarefa_do_preenchimento: Option<Task<()>>,
    /// "Criar camada da fotografia base": a camada é montada em segundo plano
    /// (96 MB numa foto de 24 MP).
    _tarefa_da_fotografia: Option<Task<()>>,
    criando_a_fotografia: bool,
    /// Densidade (0–100) e difusão (px) da máscara escolhida — as
    /// Propriedades da máscara.
    densidade_da_mascara: Entity<SliderState>,
    difusao_da_mascara: Entity<SliderState>,
    /// O que ⌘C, ⇧⌘C e ⌘X guardaram (`transferencia.rs`).
    copiado: Option<transferencia::Copiado>,
    _tarefa_da_transferencia: Option<Task<()>>,
    /// A posição do histórico quando a linha da camada foi apertada — o
    /// arrasto dela no painel vira um passo só a partir daqui.
    historico_no_aperto_da_camada: Option<usize>,
    /// O "Carimbar visível" compondo em segundo plano.
    carimbando: bool,
    _tarefa_do_carimbo: Option<Task<()>>,
    /// O arrasto na malha do Deformar.
    gesto_de_malha: Option<GestoDeMalha>,
    /// A grade do Deformar à vista (os pontos ficam sempre).
    grade_visivel: bool,
    /// Curvas: o canal à vista (0 = RGB), o ponto escolhido, o gráfico medido
    /// no último quadro e o arrasto em curso.
    canal_da_curva: usize,
    ponto_da_curva: Option<usize>,
    /// O Liquidificar: onde o ponteiro estava no último evento do arrasto
    /// (pixels da foto), e o slider da Pressão.
    arrasto_do_liquido: Option<(f32, f32)>,
    /// O arrasto do Remendo: de onde e até onde (pixels da foto).
    arrasto_do_remendo: Option<((f32, f32), (f32, f32))>,
    pressao_do_liquido: Entity<SliderState>,
    caixa_da_curva: Bounds<Pixels>,
    arrasto_da_curva: Option<ArrastoDaCurva>,
    /// O giro da vista (R), em radianos — só a tela; e os ladrilhos do palco
    /// girado.
    giro: f32,
    palco_girado: giro::PalcoGirado,
    /// O arrasto da ferramenta Girar vista: o ângulo do ponteiro em volta do
    /// centro do palco e o giro de quando começou.
    gesto_de_giro: Option<(f32, f32)>,
    /// O giro guardado enquanto o preenchimento modal está aberto (ele
    /// desenha sem giro).
    giro_antes_do_preenchimento: Option<f32>,
    /// Fluxo, espaçamento e suavização do pincel, e a predefinição.
    fluxo: Entity<SliderState>,
    espacamento: Entity<SliderState>,
    suavizacao_do_pincel: Entity<SliderState>,
    /// A difusão do pincel de recuperação (1 a 7) — não é a da seleção.
    difusao_da_recuperacao: Entity<SliderState>,
    seletor_de_predefinicao: Entity<SelectState<Vec<Opcao>>>,
    /// Os números do teclado (opacidade; com ⇧, fluxo): o primeiro dígito e
    /// quando chegou — "4 e 5 em seguida = 45%", como no Photoshop.
    digito: Option<(Instant, u8, bool)>,
    /// O ajuste rápido de tamanho e dureza arrastando (⌃⌥ + arrasto no Mac,
    /// ⌥ + botão direito no Windows e no Linux): onde começou, o raio e a
    /// dureza de então.
    ajuste_rapido: Option<(Point<Pixels>, f32, f32)>,
    /// O arrasto do contorno da seleção (só a borda, sem os pixels): onde
    /// começou, em pixels da foto.
    arrasto_do_contorno: Option<(f32, f32)>,
    /// O Espaço segurado no meio do desenho de uma seleção reposiciona a forma
    /// (a tabela da Adobe): onde o ponteiro estava quando o Espaço desceu.
    reposicionando: Option<(f32, f32)>,
    /// A última ferramenta de cada grupo da barra — a letra volta a ela, e ⇧ +
    /// letra passa para a seguinte do grupo, como no Photoshop.
    ultima_do_grupo: HashMap<char, Item>,
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
        let fluxo = slider(1.0, 100.0, 1.0, pincel.fluxo * 100.0, cx);
        let espacamento = slider(1.0, 1000.0, 1.0, pincel.espacamento * 100.0, cx);
        let suavizacao_do_pincel = slider(0.0, 100.0, 1.0, pincel.suavizacao * 100.0, cx);
        let difusao_da_recuperacao = slider(1.0, 7.0, 1.0, pincel.difusao as f32, cx);
        let pressao_do_liquido = slider(1.0, 100.0, 1.0, 50.0, cx);
        let predefinicoes: Vec<Opcao> = editor_core::pincel::PREDEFINICOES
            .iter()
            .map(|p| Opcao::nova(p.nome, p.nome))
            .collect();
        let seletor_de_predefinicao =
            cx.new(|cx| SelectState::new(predefinicoes, None, window, cx));
        let opacidade_da_camada = slider(0.0, 100.0, 1.0, 100.0, cx);
        let campo_numerico =
            |valor: &str, min: f64, max: f64, window: &mut Window, cx: &mut Context<Self>| {
                let valor = valor.to_string();
                cx.new(move |cx| {
                    InputState::new(window, cx)
                        .step(1.0)
                        .min(min)
                        .max(max)
                        .default_value(valor)
                })
            };
        let campo_da_difusao = campo_numerico("0", 0.0, 250.0, window, cx);
        let campo_da_largura = campo_numerico("1", 0.0, 30000.0, window, cx);
        let campo_da_altura = campo_numerico("1", 0.0, 30000.0, window, cx);
        let campo_da_tolerancia = campo_numerico("32", 0.0, 255.0, window, cx);
        let estilos: Vec<Opcao> = TipoDeEstilo::TODOS
            .iter()
            .map(|e| Opcao::nova(e.chave(), e.nome()))
            .collect();
        let seletor_de_estilo = cx.new(|cx| SelectState::new(estilos, None, window, cx));
        seletor_de_estilo.update(cx, |s, cx| {
            s.set_selected_value(&TipoDeEstilo::Normal.chave().to_string(), window, cx)
        });
        let amostras = vec![
            Opcao::nova("camada", "Camada atual"),
            Opcao::nova("todas", "Todas as camadas"),
        ];
        let seletor_de_amostra = cx.new(|cx| SelectState::new(amostras, None, window, cx));
        seletor_de_amostra.update(cx, |s, cx| {
            s.set_selected_value(&"todas".to_string(), window, cx)
        });
        let ajustes: Vec<Entity<SliderState>> = PARAMETROS_DE_AJUSTE
            .iter()
            .map(|p| slider(p.min, p.max, p.passo, p.inicial, cx))
            .collect();
        let densidade_da_mascara = slider(0.0, 100.0, 1.0, 100.0, cx);
        let difusao_da_mascara = slider(0.0, editor_core::difusao::DIFUSAO_MAXIMA, 0.5, 0.0, cx);
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
        let seletor_de_fundo = cx.new(|cx| {
            ColorPickerState::new(window, cx).default_value(hsla_de(pincel.cor_de_fundo))
        });
        let campo_do_zoom = cx.new(|cx| InputState::new(window, cx));
        let campo_do_giro = campo_numerico("0", -180.0, 180.0, window, cx);
        let campo_da_opacidade_da_camada = campo_numerico("100", 0.0, 100.0, window, cx);
        let estado_do_filtro = filtro::Estado::novo(window, cx);
        let rgb_do_painel =
            [0u8, 1, 2].map(|i| slider(0.0, 255.0, 1.0, pincel.cor[i as usize] as f32, cx));
        let arranjo = area_de_trabalho::ler();
        let metodos: Vec<Opcao> = preenchimento::Metodo::disponiveis()
            .into_iter()
            .map(|m| Opcao::nova(m.chave(), m.nome()))
            .collect();
        // As últimas escolhas do preenchimento (`painel_do_preenchimento::Lembrado`).
        let conteudos: Vec<Opcao> = preencher::CONTEUDOS
            .iter()
            .map(|(c, n)| Opcao::nova(*c, *n))
            .collect();
        let seletor_do_conteudo = cx.new(|cx| SelectState::new(conteudos, None, window, cx));
        seletor_do_conteudo.update(cx, |s, cx| {
            s.set_selected_value(&preencher::CONTEUDOS[0].0.to_string(), window, cx)
        });
        let lembrado = painel_do_preenchimento::Lembrado::ler();
        let pref_do_preenchimento = lembrado.preferencias();
        let opacidade_da_amostragem = slider(
            0.0,
            100.0,
            5.0,
            pref_do_preenchimento.vista.opacidade * 100.0,
            cx,
        );
        let cores_da_amostragem: Vec<Opcao> = crate::editor::preenchimento::CORES_DA_SOBREPOSICAO
            .iter()
            .map(|(n, _)| Opcao::nova(*n, *n))
            .collect();
        let seletor_da_cor_da_amostragem =
            cx.new(|cx| SelectState::new(cores_da_amostragem, None, window, cx));
        if let Some((nome, _)) = crate::editor::preenchimento::CORES_DA_SOBREPOSICAO
            .iter()
            .find(|(_, c)| *c == pref_do_preenchimento.vista.cor)
        {
            let nome = nome.to_string();
            seletor_da_cor_da_amostragem
                .update(cx, |s, cx| s.set_selected_value(&nome, window, cx));
        }
        let saidas: Vec<Opcao> = painel_do_preenchimento::SAIDAS
            .iter()
            .map(|(_, c, n)| Opcao::nova(*c, *n))
            .collect();
        let seletor_da_saida = cx.new(|cx| SelectState::new(saidas, None, window, cx));
        {
            let chave =
                painel_do_preenchimento::chave_da_saida(pref_do_preenchimento.saida).to_string();
            seletor_da_saida.update(cx, |s, cx| s.set_selected_value(&chave, window, cx));
        }
        let seletor_de_metodo = cx.new(|cx| SelectState::new(metodos, None, window, cx));
        seletor_de_metodo.update(cx, |s, cx| {
            s.set_selected_value(&lembrado.metodo().chave().to_string(), window, cx)
        });
        // Os backends compilados, com o que a máquina não tem marcado.
        let backends: Vec<Opcao> = ia_local::execucao::Backend::compilados()
            .into_iter()
            .map(|b| {
                let nome = match (b, b.disponivel()) {
                    (ia_local::execucao::Backend::Automatico, _) => {
                        format!(
                            "Automático ({})",
                            preenchimento::lama::BACKEND_AUTOMATICO.nome()
                        )
                    }
                    // Medido num Mac: a LaMa leva 6× mais no CoreML (as FFTs
                    // voltam à CPU) — escolher continua possível.
                    (ia_local::execucao::Backend::CoreMl, Ok(())) => {
                        format!("{} — mais lento com este modelo (medido)", b.nome())
                    }
                    (_, Ok(())) => b.nome().to_string(),
                    (_, Err(_)) => format!("{} — indisponível aqui", b.nome()),
                };
                Opcao::nova(b.chave(), nome)
            })
            .collect();
        let seletor_de_backend = cx.new(|cx| SelectState::new(backends, None, window, cx));
        seletor_de_backend.update(cx, |s, cx| {
            s.set_selected_value(&lembrado.backend().chave().to_string(), window, cx)
        });
        let colunas = cx.new(|_| gpui_kit::component::resizable::ResizableState::default());
        let contexto_da_ia = slider(0.0, 200.0, 5.0, lembrado.contexto, cx);
        let suavizacao = slider(0.0, 30.0, 1.0, lembrado.suavizacao, cx);

        let mut assinaturas = Vec::new();
        assinaturas.push(cx.subscribe(
            &colunas,
            |ed: &mut Self, _, _: &gpui_kit::component::resizable::ResizablePanelEvent, cx| {
                ed.guardar_a_largura_do_painel(cx)
            },
        ));
        for (estado, qual) in [
            (&tamanho, 0u8),
            (&dureza, 1),
            (&opacidade, 2),
            (&fluxo, 3),
            (&espacamento, 4),
            (&suavizacao_do_pincel, 5),
            (&difusao_da_recuperacao, 6),
        ] {
            assinaturas.push(cx.subscribe_in(
                estado,
                window,
                move |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                    let (v, _) = valor(evento);
                    if let Some(s) = ed.sessao_mut() {
                        match qual {
                            0 => s.pincel.raio = v.max(1.0),
                            1 => s.pincel.dureza = v / 100.0,
                            2 => s.pincel.opacidade = v / 100.0,
                            3 => s.pincel.fluxo = v / 100.0,
                            4 => s.pincel.espacamento = v / 100.0,
                            5 => s.pincel.suavizacao = v / 100.0,
                            _ => s.pincel.difusao = v.round().clamp(1.0, 7.0) as u8,
                        }
                    }
                    cx.notify();
                },
            ));
        }
        assinaturas.push(cx.subscribe_in(
            &seletor_de_predefinicao,
            window,
            |ed: &mut Self, _e, evento: &SelectEvent<Vec<Opcao>>, window, cx| {
                if let SelectEvent::Confirm(Some(nome)) = evento {
                    ed.usar_predefinicao(nome, cx);
                }
                window.focus(&ed.foco, cx);
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &pressao_do_liquido,
            window,
            |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                let (v, _) = valor(evento);
                if let Some(s) = ed.sessao_mut() {
                    s.forca_do_liquido = v / 100.0;
                }
                cx.notify();
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &opacidade_da_camada,
            window,
            |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                let (v, soltou) = valor(evento);
                ed.mover_opacidade_da_camada(v / 100.0, soltou, cx);
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &densidade_da_mascara,
            window,
            |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                let (v, soltou) = valor(evento);
                ed.mover_densidade_da_mascara(v, soltou, cx);
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &difusao_da_mascara,
            window,
            |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                let (v, soltou) = valor(evento);
                ed.mover_difusao_da_mascara(v, soltou, cx);
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
                    if let Some(s) = ed.sessao_mut() {
                        s.definir_cor_de_frente(cor);
                    }
                    ed.cor_mostrada = ed.sessao().map(|s| s.pincel.cor);
                    cx.notify();
                }
            },
        ));

        assinaturas.push(cx.subscribe_in(
            &seletor_de_fundo,
            window,
            |ed: &mut Self, _e, evento: &ColorPickerEvent, _w, cx| {
                if let ColorPickerEvent::Change(Some(cor)) = evento {
                    let cor = rgb_de(*cor);
                    if let Some(s) = ed.sessao_mut() {
                        s.definir_cor_de_fundo(cor);
                    }
                    ed.fundo_mostrado = ed.sessao().map(|s| s.pincel.cor_de_fundo);
                    cx.notify();
                }
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &campo_do_zoom,
            window,
            |ed: &mut Self, campo, evento: &InputEvent, window, cx| {
                ed.campo_do_zoom_mudou(campo, evento, window, cx)
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &campo_do_giro,
            window,
            |ed: &mut Self, campo, evento: &InputEvent, _window, cx| {
                if matches!(evento, InputEvent::Change | InputEvent::PressEnter { .. }) {
                    if let Ok(graus) = campo
                        .read(cx)
                        .value()
                        .trim()
                        .replace(',', ".")
                        .parse::<f32>()
                    {
                        if graus.is_finite() && (graus.to_radians() - ed.giro).abs() > 1e-3 {
                            ed.girar_a_vista(graus.to_radians(), cx);
                        }
                    }
                }
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &campo_da_opacidade_da_camada,
            window,
            |ed: &mut Self, campo, evento: &InputEvent, _window, cx| {
                if matches!(
                    evento,
                    InputEvent::PressEnter { .. } | InputEvent::Blur | InputEvent::Change
                ) {
                    if let Ok(v) = campo
                        .read(cx)
                        .value()
                        .trim()
                        .replace(',', ".")
                        .parse::<f32>()
                    {
                        let v = v.clamp(0.0, 100.0);
                        if (v - ed.opacidade_da_camada() * 100.0).abs() > 0.4 {
                            let soltou = !matches!(evento, InputEvent::Change);
                            ed.mover_opacidade_da_camada(v, soltou, cx);
                        }
                    }
                }
            },
        ));
        for (i, estado) in rgb_do_painel.iter().enumerate() {
            assinaturas.push(cx.subscribe_in(
                estado,
                window,
                move |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                    ed.canal_do_painel_mudou(i, valor(evento).0, cx);
                },
            ));
        }
        assinaturas.push(cx.subscribe_in(
            &opacidade_da_amostragem,
            window,
            |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                let (v, _) = valor(evento);
                ed.mudar_vista_da_amostragem(|o| o.opacidade = v / 100.0, cx);
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &seletor_da_cor_da_amostragem,
            window,
            |ed: &mut Self, _e, evento: &SelectEvent<Vec<Opcao>>, window, cx| {
                if let SelectEvent::Confirm(Some(nome)) = evento {
                    if let Some((_, cor)) = crate::editor::preenchimento::CORES_DA_SOBREPOSICAO
                        .iter()
                        .find(|(n, _)| n == nome)
                    {
                        let cor = *cor;
                        ed.mudar_vista_da_amostragem(|o| o.cor = cor, cx);
                    }
                }
                window.focus(&ed.foco, cx);
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &seletor_da_saida,
            window,
            |ed: &mut Self, _e, evento: &SelectEvent<Vec<Opcao>>, window, cx| {
                if let SelectEvent::Confirm(Some(chave)) = evento {
                    if let Some(saida) = painel_do_preenchimento::saida_da_chave(chave) {
                        ed.mudar_saida_do_preenchimento(saida, cx);
                    }
                }
                window.focus(&ed.foco, cx);
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &seletor_de_metodo,
            window,
            |ed: &mut Self, _e, evento: &SelectEvent<Vec<Opcao>>, window, cx| {
                if let SelectEvent::Confirm(Some(chave)) = evento {
                    if let Some(m) = preenchimento::Metodo::da_chave(chave) {
                        ed.mudar_metodo_do_preenchimento(m, cx);
                    }
                }
                window.focus(&ed.foco, cx);
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &seletor_de_backend,
            window,
            |ed: &mut Self, _e, evento: &SelectEvent<Vec<Opcao>>, window, cx| {
                if let SelectEvent::Confirm(Some(chave)) = evento {
                    if let Some(b) = ia_local::execucao::Backend::da_chave(chave) {
                        ed.mudar_backend_da_ia(b, cx);
                    }
                }
                window.focus(&ed.foco, cx);
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &suavizacao,
            window,
            |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                if valor(evento).1 {
                    ed.suavizacao_mudou(cx);
                    ed.lembrar_o_preenchimento(cx);
                }
                cx.notify();
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &contexto_da_ia,
            window,
            |ed: &mut Self, _e, evento: &SliderEvent, _w, cx| {
                if valor(evento).1 {
                    if ed.metodo_do_preenchimento == preenchimento::Metodo::LaMa {
                        ed.invalidar_previa(cx);
                    }
                    ed.lembrar_o_preenchimento(cx);
                }
                cx.notify();
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

        // Os campos da barra de opções: cada mudança vira opção da ferramenta
        // (nunca um passo do desfazer); Enter devolve o foco ao palco.
        for (campo, qual) in [
            (&campo_da_difusao, 0u8),
            (&campo_da_largura, 1),
            (&campo_da_altura, 2),
            (&campo_da_tolerancia, 3),
        ] {
            assinaturas.push(cx.subscribe_in(
                campo,
                window,
                move |ed: &mut Self, estado, evento: &InputEvent, window, cx| match evento {
                    InputEvent::Change => {
                        let texto = estado.read(cx).value().to_string();
                        ed.campo_da_selecao_mudou(qual, &texto, cx);
                    }
                    InputEvent::PressEnter { .. } => window.focus(&ed.foco, cx),
                    _ => {}
                },
            ));
        }
        assinaturas.push(cx.subscribe_in(
            &seletor_de_estilo,
            window,
            |ed: &mut Self, _e, evento: &SelectEvent<Vec<Opcao>>, window, cx| {
                if let SelectEvent::Confirm(Some(chave)) = evento {
                    if let Some(e) = TipoDeEstilo::TODOS.into_iter().find(|e| e.chave() == chave) {
                        ed.escolher_estilo(e, cx);
                    }
                }
                window.focus(&ed.foco, cx);
            },
        ));
        assinaturas.push(cx.subscribe_in(
            &seletor_de_amostra,
            window,
            |ed: &mut Self, _e, evento: &SelectEvent<Vec<Opcao>>, window, cx| {
                if let SelectEvent::Confirm(Some(chave)) = evento {
                    ed.opcoes_da_varinha.amostra = if chave == "camada" {
                        AmostraDaVarinha::CamadaAtual
                    } else {
                        AmostraDaVarinha::Todas
                    };
                    cx.notify();
                }
                window.focus(&ed.foco, cx);
            },
        ));

        let campos_da_transformacao: [Entity<InputState>; 5] = std::array::from_fn(|i| {
            let (min, max, passo) = match i {
                0 | 1 => (-100000.0, 100000.0, 1.0),
                2 | 3 => (1.0, 10000.0, 1.0),
                _ => (-360.0, 360.0, 1.0),
            };
            cx.new(|cx| {
                InputState::new(window, cx)
                    .step(passo)
                    .min(min)
                    .max(max)
                    .default_value("0")
            })
        });
        for (qual, campo) in campos_da_transformacao.iter().enumerate() {
            assinaturas.push(cx.subscribe_in(
                campo,
                window,
                move |ed: &mut Self, estado, evento: &InputEvent, window, cx| match evento {
                    InputEvent::Change if !ed.escrevendo_os_campos => {
                        let texto = estado.read(cx).value().replace(',', ".");
                        if let Ok(v) = texto.trim().parse::<f32>() {
                            ed.numero_da_transformacao_mudou(qual as u8, v, cx);
                        }
                    }
                    InputEvent::PressEnter { .. } => window.focus(&ed.foco, cx),
                    _ => {}
                },
            ));
        }
        let modos_da_ferramenta: Vec<Opcao> = Modo::TODOS
            .iter()
            .map(|m| Opcao::nova(m.chave(), m.nome()))
            .collect();
        let seletor_do_modo_da_ferramenta =
            cx.new(|cx| SelectState::new(modos_da_ferramenta, None, window, cx));
        seletor_do_modo_da_ferramenta.update(cx, |s, cx| {
            s.set_selected_value(&Modo::Normal.chave().to_string(), window, cx)
        });
        assinaturas.push(cx.subscribe_in(
            &seletor_do_modo_da_ferramenta,
            window,
            |ed: &mut Self, _e, evento: &SelectEvent<Vec<Opcao>>, window, cx| {
                if let SelectEvent::Confirm(Some(chave)) = evento {
                    if let (Some(modo), Some(s)) = (Modo::da_chave(chave), ed.sessao_mut()) {
                        s.pincel.modo = modo;
                    }
                    cx.notify();
                }
                window.focus(&ed.foco, cx);
            },
        ));
        let amostras_do_carimbo: Vec<Opcao> = editor_core::AmostraDoCarimbo::TODAS
            .iter()
            .map(|a| Opcao::nova(a.chave(), a.nome()))
            .collect();
        let seletor_da_amostra_do_carimbo =
            cx.new(|cx| SelectState::new(amostras_do_carimbo, None, window, cx));
        seletor_da_amostra_do_carimbo.update(cx, |s, cx| {
            s.set_selected_value(
                &editor_core::AmostraDoCarimbo::AtualEAbaixo
                    .chave()
                    .to_string(),
                window,
                cx,
            )
        });
        assinaturas.push(cx.subscribe_in(
            &seletor_da_amostra_do_carimbo,
            window,
            |ed: &mut Self, _e, evento: &SelectEvent<Vec<Opcao>>, window, cx| {
                if let SelectEvent::Confirm(Some(chave)) = evento {
                    let amostra = editor_core::AmostraDoCarimbo::TODAS
                        .into_iter()
                        .find(|a| a.chave() == chave);
                    if let (Some(amostra), Some(s)) = (amostra, ed.sessao_mut()) {
                        s.carimbo.amostra = amostra;
                    }
                    cx.notify();
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
            modo_de_selecao: Operacao::Nova,
            opcoes_das_formas: [OpcoesDaForma::default(); 4],
            opcoes_da_varinha: OpcoesDaVarinha::default(),
            campo_da_difusao,
            campo_da_largura,
            campo_da_altura,
            campo_da_tolerancia,
            seletor_de_estilo,
            seletor_de_amostra,
            campos_de: None,
            amostra_mostrada: Some(AmostraDaVarinha::Todas),
            seletor_do_modo_da_ferramenta,
            seletor_da_amostra_do_carimbo,
            sobreposicao_do_carimbo: true,
            previa_do_carimbo: None,
            modificando: None,
            valores_da_modificacao: [5, 5, 5],
            modo,
            modo_mostrado: None,
            faixa: editor_core::pincel::Faixa::default(),
            seletor_de_faixa,
            area_do_preenchimento: None,
            metodo_do_preenchimento: lembrado.metodo(),
            backend_da_ia: lembrado.backend(),
            preferencias_do_preenchimento: lembrado.preferencias(),
            opacidade_da_amostragem,
            seletor_da_cor_da_amostragem,
            seletor_da_saida,
            seletor_de_metodo,
            seletor_de_backend,
            contexto_da_ia,
            suavizacao,
            tarefa_do_painel: None,
            vigia_do_painel: None,
            tarefa_do_modelo: None,
            colunas,
            area_de_trabalho: None,
            barra_em_duas_colunas: arranjo.barra_em_duas_colunas,
            arranjo,
            _gravacao_da_area: None,
            gravacoes_da_area: 0,
            interface_oculta: false,
            paineis_ocultos: false,
            mostrada_na_barra: [None; ferramentas::QUANTOS_GRUPOS],
            flyout: None,
            aperto_na_barra: None,
            abriu_pelo_aperto: false,
            _tarefa_do_aperto: None,
            caixas_da_barra: [Bounds::default(); ferramentas::QUANTOS_GRUPOS],
            seletor_de_fundo,
            fundo_mostrado: None,
            cor_do_painel_e_o_fundo: false,
            rgb_do_painel,
            campo_do_zoom,
            zoom_mostrado: None,
            campo_do_giro,
            giro_mostrado: None,
            campo_da_opacidade_da_camada,
            opacidade_mostrada: None,
            lupa_reduz: false,
            proporcao_travada: false,
            mostrando_atalhos: false,
            dialogo_de_preencher: false,
            seletor_do_conteudo,
            renomeando: None,
            medidas: Medidas::default(),
            _assinaturas: assinaturas,
            _assinatura_do_nome: None,
            _assinatura_da_modificacao: None,
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
            referencia_da_caixa: None,
            campos_da_transformacao,
            escrevendo_os_campos: false,
            numeros_mostrados: None,
            traco_de_correcao: None,
            degrade_em_curso: None,
            miniaturas_das_mascaras: Vec::new(),
            miniatura_do_fundo: None,
            filtro: estado_do_filtro,
            preenchendo: false,
            _tarefa_do_preenchimento: None,
            _tarefa_da_fotografia: None,
            criando_a_fotografia: false,
            densidade_da_mascara,
            difusao_da_mascara,
            copiado: None,
            _tarefa_da_transferencia: None,
            historico_no_aperto_da_camada: None,
            carimbando: false,
            _tarefa_do_carimbo: None,
            gesto_de_malha: None,
            grade_visivel: true,
            canal_da_curva: 0,
            ponto_da_curva: None,
            arrasto_do_liquido: None,
            arrasto_do_remendo: None,
            pressao_do_liquido,
            caixa_da_curva: Bounds::default(),
            arrasto_da_curva: None,
            giro: 0.0,
            palco_girado: giro::PalcoGirado::default(),
            gesto_de_giro: None,
            giro_antes_do_preenchimento: None,
            fluxo,
            espacamento,
            suavizacao_do_pincel,
            difusao_da_recuperacao,
            seletor_de_predefinicao,
            digito: None,
            ajuste_rapido: None,
            arrasto_do_contorno: None,
            reposicionando: None,
            ultima_do_grupo: HashMap::new(),
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
        if self.giro != 0.0 {
            // Girada, a caixa dos quatro cantos da foto.
            let visor = self.visor()?;
            let (x0, y0, x1, y1) = visor.caixa_na_tela(&Retangulo::inteiro(
                cena.janela.largura as u32,
                cena.janela.altura as u32,
            ));
            return Some(Bounds::new(
                self.palco.origin + gpui_kit::point(px(x0), px(y0)),
                gpui_kit::size(px(x1 - x0), px(y1 - y0)),
            ));
        }
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

    /// Um ponto da janela no palco, **sem o giro da vista** — o espaço em que
    /// o zoom, a mão e a conversão para a foto trabalham.
    fn ponto_no_palco(&self, posicao: Point<Pixels>) -> Ponto {
        let p = (
            f(posicao.x - self.palco.origin.x),
            f(posicao.y - self.palco.origin.y),
        );
        let (x, y) = giro::girar(
            p,
            (
                f(self.palco.size.width) / 2.0,
                f(self.palco.size.height) / 2.0,
            ),
            -self.giro,
        );
        Ponto { x, y }
    }

    /// Onde a foto está no palco, com o giro — o que o desenho usa.
    fn visor(&self) -> Option<giro::Visor> {
        let (cena, v) = self.vista_do_zoom()?;
        Some(giro::Visor {
            vx: v.x,
            vy: v.y,
            escala: v.escala,
            largura: cena.area.largura,
            altura: cena.area.altura,
            angulo: self.giro,
        })
    }

    /// O giro da vista, em radianos.
    pub fn giro_da_vista(&self) -> f32 {
        self.giro
    }

    /// Quantos ladrilhos o palco girado tem montados (0 sem giro).
    pub fn ladrilhos_do_palco_girado(&self) -> usize {
        self.palco_girado.ladrilhos.len()
    }

    /// Gira só a tela (R). Nenhum pixel nem dimensão da foto muda.
    pub fn girar_a_vista(&mut self, angulo: f32, cx: &mut Context<Self>) {
        let angulo = giro::normalizar(angulo);
        if (angulo - self.giro).abs() > 1e-6 {
            self.giro = if angulo.abs() < 1e-4 { 0.0 } else { angulo };
            cx.notify();
        }
    }

    /// O ângulo do ponteiro em volta do centro do palco.
    fn angulo_do_ponteiro(&self, posicao: Point<Pixels>) -> f32 {
        let c = self.palco.center();
        f(posicao.y - c.y).atan2(f(posicao.x - c.x))
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
        let (dx, dy) = giro::girar((dx, dy), (0.0, 0.0), -self.giro);
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
        // O Photoshop só tem a Mão no Espaço: tocar sem arrastar não mexe no
        // zoom (o que a Revelação faz não vale aqui).
        let _ = (desde, usado);
        self.reposicionando = None;
        cx.notify();
    }

    pub fn espaco_segurado(&self) -> bool {
        self.espaco.is_some()
    }

    fn ao_soltar_tecla(&mut self, evento: &KeyUpEvent, cx: &mut Context<Self>) {
        if evento.keystroke.key == "space" {
            self.espaco_solto(cx);
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
            // O arrasto na tela, tirado o giro: a foto segue a mão mesmo com a
            // vista girada.
            let (dx, dy) = giro::girar(
                (f(posicao.x - mao.inicio.x), f(posicao.y - mao.inicio.y)),
                (0.0, 0.0),
                -self.giro,
            );
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

    /// O modo de um gesto de seleção: o dos modificadores, se algum está
    /// apertado (⇧ soma, ⌥ tira, ⇧⌥ cruza); senão, o escolhido na barra. A
    /// barra não muda — o modificador vale só para este gesto.
    pub fn operacao_do_gesto(&self, modificadores: gpui_kit::Modifiers) -> Operacao {
        if modificadores.shift || modificadores.alt {
            operacao_dos(modificadores)
        } else {
            self.modo_de_selecao
        }
    }

    /// O modo da barra (os quatro botões).
    pub fn modo_de_selecao(&self) -> Operacao {
        self.modo_de_selecao
    }

    pub fn escolher_modo_de_selecao(&mut self, operacao: Operacao, cx: &mut Context<Self>) {
        self.modo_de_selecao = operacao;
        cx.notify();
    }

    /// O modo que a barra realça: o do gesto em curso (com o modificador que
    /// o trocou), ou o escolhido.
    fn modo_realcado(&self) -> Operacao {
        self.gesto_de_selecao
            .as_ref()
            .map_or(self.modo_de_selecao, |g| g.operacao)
    }

    /// As opções da ferramenta de seleção na mão (ou da retangular).
    pub fn opcoes_da_forma(&self, tipo: TipoDeSelecao) -> OpcoesDaForma {
        self.opcoes_das_formas[tipo.indice()]
    }

    pub fn opcoes_da_forma_mut(&mut self, tipo: TipoDeSelecao) -> &mut OpcoesDaForma {
        &mut self.opcoes_das_formas[tipo.indice()]
    }

    pub fn opcoes_da_varinha(&self) -> OpcoesDaVarinha {
        self.opcoes_da_varinha
    }

    pub fn opcoes_da_varinha_mut(&mut self) -> &mut OpcoesDaVarinha {
        &mut self.opcoes_da_varinha
    }

    /// O laço poligonal está aberto (com vértices à espera).
    pub fn poligono_aberto(&self) -> bool {
        self.gesto_de_selecao
            .as_ref()
            .is_some_and(|g| g.tipo == TipoDeSelecao::LacoPoligonal)
    }

    /// Os vértices do laço poligonal aberto.
    pub fn vertices_do_poligono(&self) -> Vec<(f32, f32)> {
        self.gesto_de_selecao
            .as_ref()
            .filter(|g| g.tipo == TipoDeSelecao::LacoPoligonal)
            .map(|g| g.pontos.clone())
            .unwrap_or_default()
    }

    /// Onde a prévia do próximo segmento do laço poligonal termina (o
    /// ponteiro), em pixels da foto.
    pub fn proximo_do_poligono(&self) -> Option<(f32, f32)> {
        self.gesto_de_selecao
            .as_ref()
            .filter(|g| g.tipo == TipoDeSelecao::LacoPoligonal)
            .and_then(|g| g.proximo)
    }

    /// Largura e altura (pixels do documento) da forma sendo desenhada.
    pub fn medida_do_gesto(&self) -> Option<(u32, u32)> {
        self.gesto_de_selecao
            .as_ref()
            .and_then(GestoDeSelecao::medida)
    }

    /// Fecha o laço poligonal: a seleção entra num passo só. Com menos de três
    /// vértices não há área — o gesto some e a seleção de antes fica.
    pub fn concluir_poligono(&mut self, cx: &mut Context<Self>) {
        let Some(gesto) = self.gesto_de_selecao.take() else {
            return;
        };
        let mut vertices = gesto.pontos;
        vertices.dedup_by(|a, b| (a.0 - b.0).hypot(a.1 - b.1) < 0.5);
        if vertices.len() >= 3 {
            let (operacao, acabamento) = (gesto.operacao, gesto.acabamento);
            if let Some(s) = self.sessao_mut() {
                s.selecionar_poligono(vertices, operacao, acabamento);
            }
        }
        cx.notify();
    }

    /// ⌫ no laço poligonal: tira o último vértice (o último que sobra cancela).
    pub fn tirar_o_ultimo_vertice(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(gesto) = self
            .gesto_de_selecao
            .as_mut()
            .filter(|g| g.tipo == TipoDeSelecao::LacoPoligonal)
        else {
            return false;
        };
        gesto.pontos.pop();
        if gesto.pontos.is_empty() {
            self.gesto_de_selecao = None;
        }
        cx.notify();
        true
    }

    /// Esc com uma seleção sendo desenhada: o gesto some e a seleção de antes
    /// fica como estava (nada foi mexido nela até o fim do gesto).
    pub fn cancelar_gesto_de_selecao(&mut self, cx: &mut Context<Self>) -> bool {
        if self.gesto_de_selecao.take().is_none() {
            return false;
        }
        self.reposicionando = None;
        cx.notify();
        true
    }

    /// O ponteiro desceu no palco com uma ferramenta de seleção: o gesto
    /// começa com o modo da barra (⇧ soma, ⌥ tira e ⇧⌥ cruza só neste gesto),
    /// mesmo fora da foto. `cliques` é a contagem do GPUI (2 = duplo clique,
    /// que fecha o laço poligonal).
    pub fn comecar_selecao(
        &mut self,
        ponto: Point<Pixels>,
        modificadores: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) {
        self.comecar_selecao_com_cliques(ponto, modificadores, 1, cx);
    }

    pub fn comecar_selecao_com_cliques(
        &mut self,
        ponto: Point<Pixels>,
        modificadores: gpui_kit::Modifiers,
        cliques: usize,
        cx: &mut Context<Self>,
    ) {
        let (Some(tipo), Some(p)) = (self.selecionando, self.na_foto_sem_limite(ponto)) else {
            return;
        };
        // O laço poligonal aberto: cada clique é um vértice; perto do primeiro
        // ou no duplo clique, fecha.
        let escala = self
            .vista_do_zoom()
            .map_or(1.0, |(_, v)| v.escala)
            .max(1e-3);
        if let Some(gesto) = self
            .gesto_de_selecao
            .as_mut()
            .filter(|g| g.tipo == TipoDeSelecao::LacoPoligonal)
        {
            if gesto.perto_do_primeiro(p, FECHAR_O_POLIGONO / escala) {
                self.concluir_poligono(cx);
                return;
            }
            gesto.pontos.push(p);
            gesto.proximo = Some(p);
            if cliques >= 2 {
                self.concluir_poligono(cx);
                return;
            }
            cx.notify();
            return;
        }
        let operacao = self.operacao_do_gesto(modificadores);
        let opcoes = self.opcoes_da_forma(tipo);
        // Sem modificador, arrastar por dentro da seleção move **só o
        // contorno** — o "Nova seleção" do Photoshop. No laço poligonal o
        // clique é sempre um vértice.
        let dentro = self
            .sessao()
            .and_then(Sessao::selecao)
            .filter(|_| operacao == Operacao::Nova && tipo != TipoDeSelecao::LacoPoligonal)
            .is_some_and(|sel| {
                p.0 >= 0.0
                    && p.1 >= 0.0
                    && p.0 < sel.largura() as f32
                    && p.1 < sel.altura() as f32
                    && sel.valor(p.0 as u32, p.1 as u32) >= 128
            });
        if dentro {
            if let Some(s) = self.sessao_mut() {
                if s.comecar_a_mover_o_contorno() {
                    self.arrasto_do_contorno = Some(p);
                }
            }
            cx.notify();
            return;
        }
        let pontos = if tipo == TipoDeSelecao::LacoPoligonal {
            vec![p]
        } else {
            vec![p, p]
        };
        self.gesto_de_selecao = Some(GestoDeSelecao {
            tipo,
            operacao,
            estilo: if tipo.tem_estilo() {
                opcoes.estilo()
            } else {
                Estilo::Normal
            },
            acabamento: Acabamento {
                suavizar: opcoes.acabamento.suavizar || !tipo.suaviza(),
                ..opcoes.acabamento
            },
            pontos,
            proximo: Some(p),
            quadrado: false,
            do_centro: false,
        });
        self.reposicionando = None;
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
        if self.area_do_preenchimento.is_some() {
            self.apertar_no_preenchimento(ponto, modificadores, cx);
            return;
        }
        let ferramenta = self.ferramenta();
        if self.liquidificando() {
            if let Some(p) = self.na_foto_sem_limite(ponto) {
                self.arrasto_do_liquido = Some(p);
                cx.notify();
            }
            return;
        }
        if self.sessao().is_some_and(Sessao::deformando) {
            self.comecar_gesto_na_malha(ponto, cx);
            return;
        }
        if self.sessao().is_some_and(Sessao::transformando) {
            self.comecar_gesto_na_caixa(ponto, cx);
            return;
        }
        if self.selecionando.is_some() {
            self.comecar_selecao(ponto, modificadores, cx);
            return;
        }
        match self.auxiliar {
            Some(Auxiliar::GirarVista) => {
                self.gesto_de_giro = Some((self.angulo_do_ponteiro(ponto), self.giro));
                cx.notify();
                return;
            }
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
                // O modo da barra de opções (Ampliar/Reduzir); o ⌥ inverte.
                let reduz = modificadores.alt != self.lupa_reduz;
                self.ampliar_em_torno(if reduz { 0.5 } else { 2.0 }, p, cx);
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
                    let operacao = self.operacao_do_gesto(modificadores);
                    self.varinha(x, y, operacao, cx);
                }
                return;
            }
            Some(Auxiliar::Remendo) => {
                if self.avisar_se_na_mascara(cx) {
                    return;
                }
                let Some(p) = self.na_foto_sem_limite(ponto) else {
                    return;
                };
                let dentro = self.sessao().and_then(Sessao::selecao).is_some_and(|sel| {
                    p.0 >= 0.0
                        && p.1 >= 0.0
                        && p.0 < sel.largura() as f32
                        && p.1 < sel.altura() as f32
                        && sel.valor(p.0 as u32, p.1 as u32) >= 128
                });
                if dentro {
                    self.arrasto_do_remendo = Some((p, p));
                } else {
                    // O laço à mão: a área com defeito.
                    let opcoes = self.opcoes_da_forma(TipoDeSelecao::Laco);
                    self.gesto_de_selecao = Some(GestoDeSelecao {
                        tipo: TipoDeSelecao::Laco,
                        operacao: self.operacao_do_gesto(modificadores),
                        estilo: Estilo::Normal,
                        acabamento: opcoes.acabamento,
                        pontos: vec![p, p],
                        proximo: Some(p),
                        quadrado: false,
                        do_centro: false,
                    });
                }
                cx.notify();
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
                Some(Ferramenta::Carimbo | Ferramenta::Recuperacao) => {
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
        // ⇧ + clique: uma reta desde o fim do traço anterior.
        self.apertar_e_tracar(ponto, modificadores.shift, cx);
    }

    /// Há um arrasto em curso no palco (pincel, seleção, mover, giro…): o
    /// movimento e o soltar são ouvidos na janela inteira.
    fn em_gesto(&self) -> bool {
        self.pintando
            || self.gesto_de_selecao.is_some()
            || self.pegando_cor
            || self.arrasto_do_mover.is_some()
            || self.gesto_de_transformacao.is_some()
            || self.gesto_de_malha.is_some()
            || self.arrasto_do_liquido.is_some()
            || self.arrasto_do_remendo.is_some()
            || self.traco_de_correcao.is_some()
            || self.degrade_em_curso.is_some()
            || self.gesto_de_giro.is_some()
            || self.ajuste_rapido.is_some()
            || self.arrasto_do_contorno.is_some()
            || self
                .area_do_preenchimento
                .as_ref()
                .is_some_and(|e| e.pincelando.is_some())
    }

    /// Uma ferramenta de pintura está na mão (o pincel de correção também).
    fn pinta(&self) -> bool {
        self.selecionando.is_none()
            && (self.auxiliar.is_none() || self.auxiliar == Some(Auxiliar::Correcao))
    }

    fn comecar_ajuste_rapido(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(p) = self.sessao().map(|s| s.pincel) {
            self.ajuste_rapido = Some((ponto, p.raio, p.dureza));
            cx.notify();
        }
    }

    /// O arrasto do ajuste rápido: um ponto da tela para a direita é um pixel
    /// a mais de raio na tela; 200 pontos para cima, a dureza inteira.
    fn arrastar_ajuste_rapido(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((inicio, raio, dureza)) = self.ajuste_rapido else {
            return;
        };
        let escala = self
            .vista_do_zoom()
            .map_or(1.0, |(_, v)| v.escala)
            .max(1e-3);
        let (dx, dy) = (f(ponto.x - inicio.x), f(ponto.y - inicio.y));
        if let Some(s) = self.sessao_mut() {
            s.pincel.raio = (raio + dx / escala).clamp(1.0, 800.0).round();
            s.pincel.dureza = (dureza - dy / 200.0).clamp(0.0, 1.0);
        }
        cx.notify();
    }

    // ---------------------------------------------- transformação livre

    /// ⌘T: a caixa aparece em volta do conteúdo da camada (ou da seleção).
    /// Os campos da barra da transformação acompanham a caixa (fora de um
    /// campo em edição), e o ponto de referência volta ao centro quando a
    /// caixa fecha. Chamado no `render`.
    fn sincronizar_a_transformacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(numeros) = self.numeros_da_transformacao() else {
            self.referencia_da_caixa = None;
            self.numeros_mostrados = None;
            return;
        };
        if self.numeros_mostrados == Some(numeros) {
            return;
        }
        self.numeros_mostrados = Some(numeros);
        let (x, y, l, a, g) = numeros;
        let textos = [
            format!("{x:.0}"),
            format!("{y:.0}"),
            format!("{l:.1}"),
            format!("{a:.1}"),
            format!("{g:.1}"),
        ];
        self.escrevendo_os_campos = true;
        for (campo, texto) in self.campos_da_transformacao.clone().iter().zip(textos) {
            if campo.focus_handle(cx).is_focused(window) {
                continue;
            }
            campo.update(cx, |c, cx| c.set_value(texto, window, cx));
        }
        self.escrevendo_os_campos = false;
    }

    pub fn transformar(&mut self, cx: &mut Context<Self>) {
        self.referencia_da_caixa = None;
        let Some(s) = self.sessao_mut() else {
            return;
        };
        if s.posicao_bloqueada() || s.pixels_bloqueados() {
            self.avisar_cadeado("transformar", cx);
            return;
        }
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
        self.gesto_de_malha = None;
        self.arrasto_do_liquido = None;
        if self.liquidificando() {
            self.na_sessao(cx, |s| {
                s.aplicar_liquidificacao();
            });
            return;
        }
        self.na_sessao(cx, |s| {
            s.aplicar_transformacao();
        });
    }

    /// Esc.
    pub fn cancelar_transformacao(&mut self, cx: &mut Context<Self>) {
        self.gesto_de_transformacao = None;
        self.gesto_de_malha = None;
        self.arrasto_do_liquido = None;
        if self.liquidificando() {
            self.na_sessao(cx, Sessao::cancelar_liquidificacao);
            return;
        }
        self.na_sessao(cx, Sessao::cancelar_transformacao);
    }

    pub fn transformando(&self) -> bool {
        self.sessao().is_some_and(Sessao::transformando)
    }

    // ------------------------------------------------------------- remendo

    /// O Remendo com a seleção levada `(dx, dy)` pixels da foto até a origem.
    pub fn remendar(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        let inicio = Instant::now();
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let feito = s.remendar(dx, dy);
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
        self.aviso = (!feito).then(|| {
            (
                SharedString::from(
                    "O Remendo precisa de uma seleção numa camada de pixels visível, com a origem dentro da foto",
                ),
                true,
            )
        });
        cx.notify();
    }

    // -------------------------------------------------------- liquidificar

    /// ⇧⌘X: o Liquidificar na camada escolhida (ou na máscara dela).
    pub fn liquidificar(&mut self, cx: &mut Context<Self>) {
        self.gesto_de_selecao = None;
        let Some(s) = self.sessao_mut() else {
            return;
        };
        self.aviso = (!s.comecar_a_liquidificar()).then(|| {
            (
                SharedString::from(
                    "Nada para liquidificar: a camada está escondida (ou é de ajuste — escolha a máscara dela)",
                ),
                true,
            )
        });
        cx.notify();
    }

    pub fn liquidificando(&self) -> bool {
        self.sessao().is_some_and(Sessao::liquidificando)
    }

    pub fn restaurar_liquidificacao(&mut self, cx: &mut Context<Self>) {
        self.arrasto_do_liquido = None;
        if let Some(s) = self.sessao_mut() {
            s.restaurar_liquidificacao();
        }
        cx.notify();
    }

    /// 🧪 O roteiro e os testes: um trecho de pincelada, em pixels da foto.
    pub fn liquidificar_trecho(&mut self, de: (f32, f32), ate: (f32, f32), cx: &mut Context<Self>) {
        let inicio = Instant::now();
        if let Some(s) = self.sessao_mut() {
            s.liquidificar(de, ate);
        }
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
        cx.notify();
    }

    // -------------------------------------------------------- antes/depois

    /// Y e o botão da barra: a foto como abriu nesta janela, só na tela.
    pub fn alternar_antes_depois(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let agora = s.mostrando_antes();
        self.aviso = (!s.mostrar_antes(!agora)).then(|| {
            (
                SharedString::from("Aplique ou cancele a transformação antes de comparar"),
                true,
            )
        });
        cx.notify();
    }

    pub fn mostrando_antes(&self) -> bool {
        self.sessao().is_some_and(Sessao::mostrando_antes)
    }

    // ------------------------------------------------------------ deformar

    /// "Deformar": a malha de 3 × 3 células sobre o conteúdo (do ⌘T aberto,
    /// ou começando um).
    pub fn deformar(&mut self, cx: &mut Context<Self>) {
        self.gesto_de_transformacao = None;
        let Some(s) = self.sessao_mut() else {
            return;
        };
        if s.posicao_bloqueada() || s.pixels_bloqueados() {
            self.avisar_cadeado("deformar", cx);
            return;
        }
        self.aviso = if s.comecar_a_deformar() {
            None
        } else if s.transformando_a_selecao() {
            Some((
                "Deformar leva pixels: aplique ou cancele o Transformar seleção antes".into(),
                true,
            ))
        } else {
            Some((
                "Nada para deformar: a camada está vazia ou escondida".into(),
                true,
            ))
        };
        cx.notify();
    }

    pub fn deformando(&self) -> bool {
        self.sessao().is_some_and(Sessao::deformando)
    }

    /// Do Deformar de volta à transformação livre (só com a malha intocada).
    pub fn voltar_a_transformacao_livre(&mut self, cx: &mut Context<Self>) {
        self.gesto_de_malha = None;
        if let Some(s) = self.sessao_mut() {
            if !s.voltar_a_transformacao_livre() {
                self.aviso = Some((
                    "A malha já foi deformada: Redefinir antes de voltar à transformação livre"
                        .into(),
                    true,
                ));
            }
        }
        cx.notify();
    }

    /// "Redefinir": a malha do começo, sem confirmar.
    pub fn redefinir_malha(&mut self, cx: &mut Context<Self>) {
        self.gesto_de_malha = None;
        self.na_sessao(cx, Sessao::redefinir_malha);
    }

    pub fn alternar_grade(&mut self, cx: &mut Context<Self>) {
        self.grade_visivel = !self.grade_visivel;
        cx.notify();
    }

    pub fn grade_visivel(&self) -> bool {
        self.grade_visivel
    }

    /// O apertar no Deformar: perto de um dos 16 pontos (9 pontos da tela),
    /// pega o ponto; dentro da malha, puxa a superfície; fora, nada.
    fn comecar_gesto_na_malha(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        let (Some(p), Some((_, malha))) = (
            self.na_foto_sem_limite(ponto),
            self.sessao().and_then(Sessao::malha),
        ) else {
            return;
        };
        let escala = self.vista_do_zoom().map_or(1.0, |(_, v)| v.escala);
        let pega = malha.pegar(p.0, p.1, 9.0 / escala);
        let uv = if pega.is_none() {
            malha.onde(p.0, p.1)
        } else {
            None
        };
        if pega.is_none() && uv.is_none() {
            return;
        }
        self.gesto_de_malha = Some(GestoDeMalha {
            pega,
            uv,
            inicio: p,
            malha,
        });
        cx.notify();
    }

    fn arrastar_na_malha(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        let (Some(g), Some(p)) = (self.gesto_de_malha, self.na_foto_sem_limite(ponto)) else {
            return;
        };
        let (dx, dy) = (p.0 - g.inicio.0, p.1 - g.inicio.1);
        let mut m = g.malha;
        match (g.pega, g.uv) {
            (Some(editor_core::deformar::Pega::Ponto(j, i)), _) => m.mover_ponto(j, i, dx, dy),
            (None, Some((u, v))) => m.puxar(u, v, dx, dy),
            _ => return,
        }
        let inicio = Instant::now();
        if let Some(s) = self.sessao_mut() {
            s.definir_malha(m);
        }
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
        cx.notify();
    }

    /// 🧪 O roteiro e os testes: um dos 16 pontos levado de `(dx, dy)` (pixels
    /// da foto), como um arrasto.
    pub fn mover_ponto_da_malha(
        &mut self,
        linha: usize,
        coluna: usize,
        dx: f32,
        dy: f32,
        cx: &mut Context<Self>,
    ) {
        if let Some(s) = self.sessao_mut() {
            if let Some((_, mut m)) = s.malha() {
                m.mover_ponto(linha, coluna, dx, dy);
                s.definir_malha(m);
            }
        }
        cx.notify();
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
        // A versão do instantâneo: o remendo só entra se o documento ainda
        // for este quando ele chegar.
        let versao = s.versao();
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
            // Toda a borda suave da seleção é refeita (e misturada pelo peso
            // ao colar): com ≥ 128, a rampa de uma seleção difundida ficava
            // fora do remendo e a borda saía dura.
            let no_buraco = |x: f32, y: f32| para_o_fundo.peso(x + ox, y + oy) > 0;
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
                let mut mudou = false;
                if let Some(s) = ed.sessao_mut() {
                    mudou = s.mudou_desde(versao);
                    if !mudou {
                        s.colar_remendo(camada, &ret, &r.rgba, &peso);
                    }
                }
                ed.aviso = mudou.then(|| {
                    (
                        "A foto mudou enquanto o preenchimento era calculado — o remendo foi descartado; refaça".into(),
                        true,
                    )
                });
                cx.notify();
            });
        }));
    }

    /// O ponto de referência da caixa, na caixa de origem: o escolhido, ou
    /// o centro.
    pub fn referencia_da_caixa(&self) -> Option<(f32, f32)> {
        let (caixa, _) = self.sessao()?.transformacao()?;
        Some(self.referencia_da_caixa.unwrap_or((
            caixa.x as f32 + caixa.largura as f32 / 2.0,
            caixa.y as f32 + caixa.altura as f32 / 2.0,
        )))
    }

    /// Em que parte da caixa o ponto (da foto) cai — o ponto de referência e
    /// as alças contam com uma folga de 8 pontos da tela.
    pub fn parte_da_caixa(&self, x: f32, y: f32) -> Option<ParteDaCaixa> {
        let (caixa, t) = self.sessao()?.transformacao()?;
        let escala = self.vista_do_zoom().map_or(1.0, |(_, v)| v.escala);
        let folga = 8.0 / escala;
        let perto = |q: (f32, f32)| (q.0 - x).hypot(q.1 - y) <= folga;
        let referencia = self.referencia_da_caixa()?;
        if perto(t.aplicar(&caixa, referencia.0, referencia.1)) {
            return Some(ParteDaCaixa::Referencia);
        }
        // Os cantos primeiro: numa caixa pequena eles ganham dos meios.
        let alcas = Transformacao::alcas(&caixa);
        for i in [0, 2, 4, 6, 1, 3, 5, 7] {
            if perto(t.aplicar(&caixa, alcas[i].0, alcas[i].1)) {
                return Some(ParteDaCaixa::Alca(i));
            }
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
        let (Some(parte), Some(referencia)) =
            (self.parte_da_caixa(p.0, p.1), self.referencia_da_caixa())
        else {
            return;
        };
        self.gesto_de_transformacao = Some(GestoDeTransformacao {
            parte,
            inicio: p,
            t,
            referencia,
        });
        cx.notify();
    }

    /// O arrasto na caixa: mover, escalar por uma alça, girar em volta do
    /// ponto de referência ou levar o ponto de referência. ⇧ solta a
    /// proporção nos cantos e prende o giro em 15°; ⌥ escala em volta do ponto
    /// de referência.
    fn arrastar_na_caixa(
        &mut self,
        ponto: Point<Pixels>,
        modificadores: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) {
        let (Some(g), Some(p), Some((caixa, _))) = (
            self.gesto_de_transformacao,
            self.na_foto_sem_limite(ponto),
            self.sessao().and_then(Sessao::transformacao),
        ) else {
            return;
        };
        let mut t = g.t;
        match g.parte {
            ParteDaCaixa::Dentro => {
                t.dx = g.t.dx + (p.0 - g.inicio.0).round();
                t.dy = g.t.dy + (p.1 - g.inicio.1).round();
            }
            ParteDaCaixa::Alca(i) => {
                let ancora = if modificadores.alt {
                    g.referencia
                } else {
                    Transformacao::alcas(&caixa)[(i + 4) % 8]
                };
                t = g.t.pela_alca(&caixa, i, p, ancora, !modificadores.shift);
            }
            ParteDaCaixa::Fora => {
                let r = g.t.aplicar(&caixa, g.referencia.0, g.referencia.1);
                let angulo = |q: (f32, f32)| (q.1 - r.1).atan2(q.0 - r.0);
                let mut final_ = g.t.angulo + angulo(p) - angulo(g.inicio);
                if modificadores.shift {
                    let passo = std::f32::consts::PI / 12.0;
                    final_ = (final_ / passo).round() * passo;
                }
                t =
                    g.t.girada_em_volta(&caixa, g.referencia, final_ - g.t.angulo);
            }
            ParteDaCaixa::Referencia => {
                self.referencia_da_caixa = Some(g.t.inversa(&caixa, p.0, p.1));
                cx.notify();
                return;
            }
        }
        self.definir_transformacao_medindo(t, cx);
    }

    /// A transformação nova, só se mudou (o soltar no mesmo ponto não é
    /// gesto), medindo o tempo.
    fn definir_transformacao_medindo(&mut self, t: Transformacao, cx: &mut Context<Self>) {
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

    /// Os números da barra da transformação: X e Y do ponto de referência
    /// (pixels da foto), largura e altura em % e o ângulo em graus.
    pub fn numeros_da_transformacao(&self) -> Option<(f32, f32, f32, f32, f32)> {
        let (caixa, t) = self.sessao()?.transformacao()?;
        let r = self.referencia_da_caixa()?;
        let (x, y) = t.aplicar(&caixa, r.0, r.1);
        Some((
            x,
            y,
            t.escala_x * 100.0,
            t.escala_y * 100.0,
            t.angulo.to_degrees(),
        ))
    }

    /// Um campo da barra da transformação mudou (0 X, 1 Y, 2 L%, 3 A%, 4
    /// ângulo): a escala e o ângulo em volta do ponto de referência.
    pub fn numero_da_transformacao_mudou(&mut self, qual: u8, valor: f32, cx: &mut Context<Self>) {
        let (Some((caixa, t)), Some(r)) = (
            self.sessao().and_then(Sessao::transformacao),
            self.referencia_da_caixa(),
        ) else {
            return;
        };
        if !valor.is_finite() {
            return;
        }
        let mut n = t;
        match qual {
            0 | 1 => {
                let (x, y) = t.aplicar(&caixa, r.0, r.1);
                if qual == 0 {
                    n.dx += valor - x;
                } else {
                    n.dy += valor - y;
                }
            }
            // Com a corrente da barra fechada, largura e altura andam juntas
            // (a proporção da caixa fica).
            2 | 3 if valor >= 1.0 && self.proporcao_travada => {
                let (ex, ey) = if qual == 2 {
                    (
                        valor / 100.0,
                        t.escala_y * (valor / 100.0) / t.escala_x.max(1e-6),
                    )
                } else {
                    (
                        t.escala_x * (valor / 100.0) / t.escala_y.max(1e-6),
                        valor / 100.0,
                    )
                };
                n = Transformacao {
                    escala_x: ex,
                    escala_y: ey,
                    ..t
                }
                .fixando(&caixa, r, &t)
            }
            2 if valor >= 1.0 => {
                n = Transformacao {
                    escala_x: valor / 100.0,
                    ..t
                }
                .fixando(&caixa, r, &t)
            }
            3 if valor >= 1.0 => {
                n = Transformacao {
                    escala_y: valor / 100.0,
                    ..t
                }
                .fixando(&caixa, r, &t)
            }
            4 => n = t.girada_em_volta(&caixa, r, valor.to_radians() - t.angulo),
            _ => return,
        }
        self.definir_transformacao_medindo(n, cx);
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
        if s.posicao_bloqueada() || (s.selecao().is_some() && s.pixels_bloqueados()) {
            self.avisar_cadeado("mover", cx);
            return;
        }
        if s.comecar_a_mover() {
            self.arrasto_do_mover = Some(p);
            self.aviso = None;
        } else {
            let nome = s.camada_ativa().nome.clone();
            self.aviso = Some((
                format!(
                    "{nome} está escondida — mostre a camada (o olho, ou {MOSTRAR}) para movê-la"
                )
                .into(),
                true,
            ));
        }
        cx.notify();
    }

    /// O ponteiro desceu na foto.
    pub fn apertar(&mut self, ponto: Point<Pixels>, cx: &mut Context<Self>) {
        self.apertar_e_tracar(ponto, false, cx);
    }

    /// O ponteiro desceu na foto; `em_reta` (⇧) liga ao fim do traço anterior.
    fn apertar_e_tracar(&mut self, ponto: Point<Pixels>, em_reta: bool, cx: &mut Context<Self>) {
        if self.selecionando.is_some() {
            self.comecar_selecao(ponto, gpui_kit::Modifiers::none(), cx);
            return;
        }
        let Some((x, y)) = self.na_foto(ponto) else {
            return;
        };
        let inicio = Instant::now();
        let escala = self.vista_do_zoom().map_or(1.0, |(_, v)| v.escala);
        let Some(s) = self.sessao_mut() else {
            return;
        };
        // A suavização é medida na tela: o zoom de agora.
        s.escala_da_tela = escala;
        let comecou = if em_reta {
            s.apertar_em_reta(x, y)
        } else {
            s.apertar(x, y)
        };
        if comecou {
            self.pintando = true;
        } else if s.pixels_bloqueados() {
            self.avisar_cadeado("pintar", cx);
            return;
        } else if s.pincel.ferramenta.copia_da_origem() && s.origem().is_none() {
            self.aviso = Some((
                if s.pincel.ferramenta == Ferramenta::Recuperacao {
                    "⌥ + clique na foto para escolher a pele limpa de onde a recuperação copia"
                } else {
                    "⌥ + clique na foto para escolher de onde o carimbo copia"
                }
                .into(),
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
                format!("{nome} está escondida — mostre a camada (o olho, ou {MOSTRAR}) para pintar nela").into(),
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
        if self.arrastar_no_preenchimento(ponto, modificadores, cx) {
            return;
        }
        if let Some((de, giro_de_antes)) = self.gesto_de_giro {
            let mut angulo = giro_de_antes + (self.angulo_do_ponteiro(ponto) - de);
            if modificadores.shift {
                angulo = giro::em_passos(angulo);
            }
            self.girar_a_vista(angulo, cx);
            return;
        }
        if self.ajuste_rapido.is_some() {
            self.arrastar_ajuste_rapido(ponto, cx);
            return;
        }
        if let Some(inicio) = self.arrasto_do_contorno {
            if let Some(p) = self.na_foto_sem_limite(ponto) {
                let (dx, dy) = (
                    (p.0 - inicio.0).round() as i64,
                    (p.1 - inicio.1).round() as i64,
                );
                if let Some(s) = self.sessao_mut() {
                    s.mover_o_contorno_por(dx, dy);
                }
            }
            cx.notify();
            return;
        }
        if self.gesto_de_malha.is_some() {
            self.arrastar_na_malha(ponto, cx);
            return;
        }
        if let Some((de, _)) = self.arrasto_do_remendo {
            if let Some(ate) = self.na_foto_sem_limite(ponto) {
                self.arrasto_do_remendo = Some((de, ate));
                cx.notify();
            }
            return;
        }
        if let Some(de) = self.arrasto_do_liquido {
            if let Some(ate) = self.na_foto_sem_limite(ponto) {
                self.arrasto_do_liquido = Some(ate);
                self.liquidificar_trecho(de, ate, cx);
            }
            return;
        }
        if self.gesto_de_transformacao.is_some() {
            self.arrastar_na_caixa(ponto, modificadores, cx);
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
        if self.poligono_aberto() {
            // O laço poligonal só mostra o próximo segmento até o ponteiro.
            let no_ponto = self.na_foto_sem_limite(ponto);
            if let Some(g) = self.gesto_de_selecao.as_mut() {
                g.proximo = no_ponto;
            }
            cx.notify();
            return;
        }
        if self.gesto_de_selecao.is_some() {
            let escala = self.vista_do_zoom().map_or(1.0, |(_, v)| v.escala);
            let no_ponto = self.na_foto_sem_limite(ponto);
            let espaco = self.espaco.is_some();
            // Espaço segurado no meio do desenho: a forma anda inteira com o
            // ponteiro ("Reposition marquee while selecting").
            if let (Some(p), true) = (no_ponto, espaco) {
                // O Espaço foi usado: soltar não alterna o zoom.
                if let Some((_, usado)) = self.espaco.as_mut() {
                    *usado = true;
                }
                let antes = self.reposicionando.replace(p);
                if let (Some(antes), Some(gesto)) = (antes, self.gesto_de_selecao.as_mut()) {
                    if gesto.tipo != TipoDeSelecao::Laco {
                        let (dx, dy) = (p.0 - antes.0, p.1 - antes.1);
                        for q in gesto.pontos.iter_mut() {
                            q.0 += dx;
                            q.1 += dy;
                        }
                    }
                }
                cx.notify();
                return;
            }
            self.reposicionando = None;
            if let (Some(p), Some(gesto)) = (no_ponto, self.gesto_de_selecao.as_mut()) {
                gesto.quadrado = modificadores.shift;
                gesto.do_centro = modificadores.alt;
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
        if self.soltar_no_preenchimento(cx) {
            return;
        }
        if self.gesto_de_giro.take().is_some() || self.ajuste_rapido.take().is_some() {
            cx.notify();
            return;
        }
        if self.arrasto_do_contorno.take().is_some() {
            if let Some(s) = self.sessao_mut() {
                s.terminar_de_mover_o_contorno();
            }
            cx.notify();
            return;
        }
        self.pegando_cor = false;
        if let Some((de, ate)) = self.arrasto_do_remendo.take() {
            let (dx, dy) = (ate.0 - de.0, ate.1 - de.1);
            if dx.hypot(dy) >= 1.0 {
                self.remendar(dx, dy, cx);
            }
            cx.notify();
            return;
        }
        if self.gesto_de_transformacao.take().is_some()
            || self.gesto_de_malha.take().is_some()
            || self.arrasto_do_liquido.take().is_some()
        {
            cx.notify();
            return;
        }
        if let Some((de, ate)) = self.degrade_em_curso.take() {
            if self.recusar_se_bloqueada("aplicar o degradê", cx) {
                return;
            }
            let escondida = self.sessao().is_some_and(|s| !s.camada_ativa().visivel);
            let inicio = Instant::now();
            self.na_sessao(cx, |s| {
                s.degrade(de, ate);
            });
            self.medidas.ultimo_gesto = Some(inicio.elapsed());
            if escondida {
                self.aviso = Some((
                    format!(
                        "A camada está escondida — mostre-a (o olho, ou {MOSTRAR}) para o degradê"
                    )
                    .into(),
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
        if self.poligono_aberto() {
            // O laço poligonal continua aberto entre os cliques.
            return;
        }
        if let Some(gesto) = self.gesto_de_selecao.take() {
            let forma = gesto.forma();
            if let Some(s) = self.sessao_mut() {
                s.selecionar_com(&forma, gesto.operacao, gesto.acabamento);
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
        if self.selecionando != Some(tipo) {
            self.gesto_de_selecao = None;
        }
        self.selecionando = Some(tipo);
        self.auxiliar = None;
        self.lembrar_na_barra(Item::S(tipo));
        cx.notify();
    }

    /// O conta-gotas (I) ou o Mover (V) na mão.
    pub fn usar_auxiliar(&mut self, auxiliar: Auxiliar, cx: &mut Context<Self>) {
        if self.area_do_preenchimento.is_some() {
            return;
        }
        self.auxiliar = Some(auxiliar);
        self.selecionando = None;
        self.gesto_de_selecao = None;
        self.lembrar_na_barra(Item::A(auxiliar));
        cx.notify();
    }

    /// Uma ferramenta da barra, de qualquer tipo.
    pub fn usar_item(&mut self, item: Item, cx: &mut Context<Self>) {
        match item {
            Item::F(f) => self.usar(f, cx),
            Item::S(t) => self.usar_selecao(t, cx),
            Item::A(a) => self.usar_auxiliar(a, cx),
        }
    }

    /// A ferramenta na mão, como item da barra.
    pub fn item_atual(&self) -> Option<Item> {
        if let Some(t) = self.selecionando {
            return Some(Item::S(t));
        }
        if let Some(a) = self.auxiliar {
            return Some(Item::A(a));
        }
        self.ferramenta().map(Item::F)
    }

    pub fn auxiliar(&self) -> Option<Auxiliar> {
        self.auxiliar
    }

    /// O nome da ferramenta na mão, com a letra ("Borracha (E)") — da
    /// tabela da barra.
    pub fn nome_da_ferramenta(&self) -> String {
        self.item_atual()
            .and_then(|i| ferramentas::def_de(&i))
            .map(ferramentas::nome_com_letra)
            .unwrap_or_default()
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
        if self.area_do_preenchimento.is_some() {
            return;
        }
        self.selecionando = None;
        self.auxiliar = None;
        self.lembrar_na_barra(Item::F(ferramenta));
        if let Some(s) = self.sessao_mut() {
            s.pincel.ferramenta = ferramenta;
        }
        cx.notify();
    }

    pub fn escolher_cor(&mut self, cor: [u8; 3], cx: &mut Context<Self>) {
        let com_o_pincel = self.selecionando.is_none()
            && self.auxiliar.is_none()
            && !self.ferramenta().is_some_and(Ferramenta::copia_da_origem);
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

    /// `{` e `}`: a dureza em passos de 25%, como no Photoshop.
    fn mudar_dureza(&mut self, passo: f32, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            let nova = ((s.pincel.dureza + passo) / 0.25).round() * 0.25;
            s.pincel.dureza = nova.clamp(0.0, 1.0);
        }
        cx.notify();
    }

    /// Uma predefinição de pincel pelo nome (o Select do painel).
    pub fn usar_predefinicao(&mut self, nome: &str, cx: &mut Context<Self>) {
        let Some(pre) = editor_core::pincel::PREDEFINICOES
            .iter()
            .find(|p| p.nome == nome)
        else {
            return;
        };
        if let Some(s) = self.sessao_mut() {
            s.pincel = pre.aplicada(s.pincel);
        }
        cx.notify();
    }

    /// Os números do teclado com uma ferramenta de pintura: a opacidade (com
    /// ⇧, o fluxo) — "1" = 10%, "0" = 100%, "4" e "5" em seguida = 45%, a regra
    /// da tabela da Adobe. Só com o editor focado, nunca num campo de texto.
    fn ao_apertar_tecla(&mut self, evento: &KeyDownEvent, window: &Window, cx: &mut Context<Self>) {
        // ↑ ↓ com o flyout da barra aberto (Enter e Esc vêm pelas ações).
        if self.flyout.is_some()
            && matches!(evento.keystroke.key.as_str(), "up" | "down")
            && self.tecla_no_flyout(&evento.keystroke.key, cx)
        {
            cx.stop_propagation();
            return;
        }
        let m = evento.keystroke.modifiers;
        if !self.foco.is_focused(window) || m.platform || m.control || m.alt || m.function {
            return;
        }
        // 🚨 No teclado real, ⇧3 chega como "#" (visto no app real): os
        // símbolos da fila dos números valem o número, com ⇧.
        let Some(d) = digito_da_tecla(&evento.keystroke.key) else {
            return;
        };
        let pinta = self.selecionando.is_none()
            && (self.auxiliar.is_none() || self.auxiliar == Some(Auxiliar::Correcao));
        if !pinta {
            return;
        }
        // O símbolo ("#") já é o ⇧: o GPUI do Mac o entrega sem o ⇧ nos
        // modificadores (visto no app real).
        let simbolo = !evento
            .keystroke
            .key
            .starts_with(|c: char| c.is_ascii_digit());
        let fluxo = m.shift || simbolo;
        let agora = Instant::now();
        let valor = match self.digito.take() {
            Some((quando, primeiro, f))
                if f == fluxo && agora.duration_since(quando) < Duration::from_millis(700) =>
            {
                (primeiro as f32 * 10.0 + d as f32) / 100.0
            }
            _ => {
                self.digito = Some((agora, d as u8, fluxo));
                if d == 0 {
                    1.0
                } else {
                    d as f32 / 10.0
                }
            }
        };
        let valor = valor.clamp(0.01, 1.0);
        if let Some(s) = self.sessao_mut() {
            if fluxo {
                s.pincel.fluxo = valor;
            } else {
                s.pincel.opacidade = valor;
            }
        }
        cx.notify();
    }

    /// Os sliders do pincel acompanham o pincel (predefinição, números,
    /// `{` `}`, o ajuste rápido arrastando).
    fn acompanhar_o_pincel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.sessao().map(|s| s.pincel) else {
            return;
        };
        for (estado, v) in [
            (self.tamanho.clone(), p.raio),
            (self.dureza.clone(), p.dureza * 100.0),
            (self.opacidade.clone(), p.opacidade * 100.0),
            (self.fluxo.clone(), p.fluxo * 100.0),
            (self.espacamento.clone(), p.espacamento * 100.0),
            (self.suavizacao_do_pincel.clone(), p.suavizacao * 100.0),
            (self.difusao_da_recuperacao.clone(), p.difusao as f32),
            (
                self.pressao_do_liquido.clone(),
                self.sessao().map_or(50.0, |s| s.forca_do_liquido * 100.0),
            ),
        ] {
            if (estado.read(cx).value().start() - v).abs() > 0.5 {
                estado.update(cx, |s, cx| s.set_value(v, window, cx));
            }
        }
        // O Select mostra a predefinição só enquanto o pincel é ela.
        let pre = editor_core::pincel::PREDEFINICOES
            .iter()
            .find(|pre| pre.e_a_de(&p))
            .map(|pre| pre.nome.to_string());
        let mostrada = self
            .seletor_de_predefinicao
            .read(cx)
            .selected_value()
            .cloned();
        if pre != mostrada {
            self.seletor_de_predefinicao.update(cx, |s, cx| match &pre {
                Some(nome) => s.set_selected_value(nome, window, cx),
                None => s.set_selected_index(None, window, cx),
            });
        }
    }

    // ------------------------------------------------------------ camadas

    /// Um gesto na sessão que pode ter mudado a pilha: limpa o aviso velho.
    fn na_sessao(&mut self, cx: &mut Context<Self>, fazer: impl FnOnce(&mut Sessao)) {
        // O preenchimento é modal: nenhum comando mexe no documento por baixo
        // dele (⌘Z, camadas, seleção) — o que ele aplicaria deixaria de ser o
        // que foi visualizado.
        if self.area_do_preenchimento.is_some() || self.filtro.aberto.is_some() {
            return;
        }
        // Um comando no meio de um laço poligonal aberto o descarta (a seleção
        // de antes fica).
        self.gesto_de_selecao = None;
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

    /// A varinha em `(x, y)`, pixels da foto, com as opções da barra.
    pub fn varinha(&mut self, x: f32, y: f32, operacao: Operacao, cx: &mut Context<Self>) {
        let opcoes = self.opcoes_da_varinha;
        let inicio = Instant::now();
        let mut recusa = None;
        self.na_sessao(cx, |s| {
            recusa = s.varinha_com(x, y, opcoes, operacao).err();
        });
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
        let texto = match recusa {
            Some(VarinhaRecusada::CamadaSemPixels) => Some(
                "A camada de ajuste não tem pixels: escolha a máscara dela ou amostre todas as camadas",
            ),
            Some(VarinhaRecusada::CamadaEscondida) => Some(
                "A camada atual está escondida: mostre-a ou amostre todas as camadas",
            ),
            _ => None,
        };
        if let Some(texto) = texto {
            self.aviso = Some((texto.into(), true));
            cx.notify();
        }
    }

    // ------------------------------------------------- opções da barra

    /// Um campo da barra de opções mudou (0 difusão, 1 largura, 2 altura, 3
    /// tolerância). Texto que não é número fica para o próximo — a opção só
    /// muda com um valor que serve. Nenhum passo do desfazer, e a seleção que
    /// existe não muda.
    fn campo_da_selecao_mudou(&mut self, qual: u8, texto: &str, cx: &mut Context<Self>) {
        let Ok(numero) = texto.trim().replace(',', ".").parse::<f32>() else {
            return;
        };
        if !numero.is_finite() || numero < 0.0 {
            return;
        }
        if qual == 3 {
            self.opcoes_da_varinha.tolerancia = numero.round().min(255.0) as u8;
            cx.notify();
            return;
        }
        let Some(tipo) = self.selecionando else {
            return;
        };
        let opcoes = self.opcoes_da_forma_mut(tipo);
        match (qual, opcoes.estilo) {
            (0, _) => opcoes.acabamento.difusao = numero.round().min(250.0) as u32,
            (1, TipoDeEstilo::Proporcao) if numero > 0.0 => opcoes.proporcao.0 = numero,
            (2, TipoDeEstilo::Proporcao) if numero > 0.0 => opcoes.proporcao.1 = numero,
            (1, TipoDeEstilo::Tamanho) if numero >= 1.0 => opcoes.tamanho.0 = numero.round() as u32,
            (2, TipoDeEstilo::Tamanho) if numero >= 1.0 => opcoes.tamanho.1 = numero.round() as u32,
            _ => {}
        }
        cx.notify();
    }

    /// O estilo da retangular ou da elíptica na mão.
    pub fn escolher_estilo(&mut self, estilo: TipoDeEstilo, cx: &mut Context<Self>) {
        if let Some(tipo) = self.selecionando.filter(|t| t.tem_estilo()) {
            self.opcoes_da_forma_mut(tipo).estilo = estilo;
        }
        cx.notify();
    }

    /// Uma proporção pronta (1:1, 3:2, 4:3, 16:9): o estilo vira proporção
    /// fixa com ela.
    pub fn usar_proporcao(&mut self, largura: f32, altura: f32, cx: &mut Context<Self>) {
        if let Some(tipo) = self.selecionando.filter(|t| t.tem_estilo()) {
            let o = self.opcoes_da_forma_mut(tipo);
            o.estilo = TipoDeEstilo::Proporcao;
            o.proporcao = (largura, altura);
        }
        cx.notify();
    }

    /// ⇄ troca largura e altura da proporção ou do tamanho fixo.
    pub fn trocar_largura_e_altura(&mut self, cx: &mut Context<Self>) {
        if let Some(tipo) = self.selecionando.filter(|t| t.tem_estilo()) {
            let o = self.opcoes_da_forma_mut(tipo);
            o.proporcao = (o.proporcao.1, o.proporcao.0);
            o.tamanho = (o.tamanho.1, o.tamanho.0);
            // Força os campos a mostrar os valores trocados.
            self.campos_de = None;
        }
        cx.notify();
    }

    /// Liga ou desliga o antisserrilhado da ferramenta na mão (elipse, laços,
    /// varinha).
    pub fn alternar_suavizar(&mut self, cx: &mut Context<Self>) {
        if self.auxiliar == Some(Auxiliar::Varinha) {
            self.opcoes_da_varinha.suavizar = !self.opcoes_da_varinha.suavizar;
        } else if let Some(tipo) = self.selecionando.filter(|t| t.suaviza()) {
            let a = &mut self.opcoes_da_forma_mut(tipo).acabamento;
            a.suavizar = !a.suavizar;
        }
        cx.notify();
    }

    pub fn alternar_varinha_contigua(&mut self, cx: &mut Context<Self>) {
        self.opcoes_da_varinha.contigua = !self.opcoes_da_varinha.contigua;
        cx.notify();
    }

    /// Os campos da barra acompanham a ferramenta e o estilo escolhidos (e o
    /// Select do estilo também). Chamado no `render`.
    fn sincronizar_os_campos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let amostra = self.opcoes_da_varinha.amostra;
        if self.amostra_mostrada != Some(amostra) {
            self.amostra_mostrada = Some(amostra);
            let chave = match amostra {
                AmostraDaVarinha::CamadaAtual => "camada",
                AmostraDaVarinha::Todas => "todas",
            }
            .to_string();
            self.seletor_de_amostra
                .update(cx, |s, cx| s.set_selected_value(&chave, window, cx));
        }
        let Some(tipo) = self.selecionando else {
            self.campos_de = None;
            return;
        };
        let o = self.opcoes_da_forma(tipo);
        if self.campos_de == Some((tipo, o.estilo)) {
            return;
        }
        self.campos_de = Some((tipo, o.estilo));
        let numero = |v: f32| {
            if v.fract() == 0.0 {
                format!("{v:.0}")
            } else {
                format!("{v}")
            }
        };
        let (l, a) = match o.estilo {
            TipoDeEstilo::Tamanho => (o.tamanho.0.to_string(), o.tamanho.1.to_string()),
            _ => (numero(o.proporcao.0), numero(o.proporcao.1)),
        };
        let difusao = o.acabamento.difusao.to_string();
        for (campo, texto) in [
            (&self.campo_da_difusao, difusao),
            (&self.campo_da_largura, l),
            (&self.campo_da_altura, a),
        ] {
            campo.update(cx, |c, cx| c.set_value(texto, window, cx));
        }
        let chave = o.estilo.chave().to_string();
        self.seletor_de_estilo
            .update(cx, |s, cx| s.set_selected_value(&chave, window, cx));
    }

    // ------------------------------------------------ modificar seleção

    /// Abre o "Modificar seleção" de um comando: o valor em pixels num campo.
    pub fn abrir_modificacao(
        &mut self,
        modificacao: Modificacao,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sessao().and_then(Sessao::selecao).is_none() {
            self.aviso = Some(("Não há seleção para modificar".into(), true));
            cx.notify();
            return;
        }
        let valor = self.valores_da_modificacao[modificacao.indice()].to_string();
        let campo = cx.new(|cx| {
            InputState::new(window, cx)
                .step(1.0)
                .min(1.0)
                .max(500.0)
                .default_value(valor)
        });
        let sub = cx.subscribe_in(
            &campo,
            window,
            |ed: &mut Self, _c, evento: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = evento {
                    ed.confirmar_modificacao(window, cx);
                }
            },
        );
        self._assinatura_da_modificacao = Some(sub);
        window.focus(&campo.focus_handle(cx), cx);
        self.modificando = Some((modificacao, campo));
        cx.notify();
    }

    /// OK no "Modificar seleção": um passo do desfazer.
    pub fn confirmar_modificacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((modificacao, campo)) = self.modificando.take() else {
            return;
        };
        window.focus(&self.foco, cx);
        let texto = campo.read(cx).value().to_string();
        match texto.trim().parse::<f32>() {
            Ok(v) if v >= 1.0 => self.modificar_selecao(modificacao, v.round() as u32, cx),
            _ => {
                self.aviso = Some(("Use um número de pixels a partir de 1".into(), true));
                cx.notify();
            }
        }
    }

    pub fn cancelar_modificacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.modificando = None;
        window.focus(&self.foco, cx);
        cx.notify();
    }

    /// Difundir, expandir ou contrair a seleção que existe em `px` pixels do
    /// documento — diferente da difusão da ferramenta, que vale para a próxima
    /// forma.
    pub fn modificar_selecao(&mut self, modificacao: Modificacao, px: u32, cx: &mut Context<Self>) {
        let px = px.max(1);
        self.valores_da_modificacao[modificacao.indice()] = px;
        let inicio = Instant::now();
        self.na_sessao(cx, |s| {
            match modificacao {
                Modificacao::Difundir => s.difundir_selecao(px),
                Modificacao::Expandir => s.expandir_selecao(px as i32),
                Modificacao::Contrair => s.expandir_selecao(-(px as i32)),
            };
        });
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
    }

    /// ⇧F6: o Difundir do "Modificar seleção".
    pub fn difundir_selecao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.abrir_modificacao(Modificacao::Difundir, window, cx);
    }

    /// "Transformar seleção": a caixa do ⌘T em volta da seleção, que só mexe
    /// no contorno.
    pub fn transformar_selecao(&mut self, cx: &mut Context<Self>) {
        self.referencia_da_caixa = None;
        let mut abriu = false;
        self.na_sessao(cx, |s| abriu = s.comecar_a_transformar_a_selecao());
        if !abriu {
            self.aviso = Some(("Não há seleção para transformar".into(), true));
            cx.notify();
        }
    }

    pub fn modificando(&self) -> Option<Modificacao> {
        self.modificando.as_ref().map(|(m, _)| *m)
    }

    // ------------------------------------------------ opções do carimbo

    pub fn alternar_carimbo_alinhado(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.carimbo.alinhado = !s.carimbo.alinhado;
        }
        cx.notify();
    }

    pub fn alternar_sobreposicao_do_carimbo(&mut self, cx: &mut Context<Self>) {
        self.sobreposicao_do_carimbo = !self.sobreposicao_do_carimbo;
        cx.notify();
    }

    /// A prévia da origem para o ponteiro de agora: só com o carimbo na mão,
    /// a sobreposição ligada, fora de um traço (ela some enquanto se pinta,
    /// como o "Ocultar automaticamente" do Photoshop) e sem a vista girada (o
    /// GPUI não gira imagem). Refeita só quando o estado muda.
    fn atualizar_a_previa_do_carimbo(&mut self) {
        let carimbo = self.selecionando.is_none()
            && self.auxiliar.is_none()
            && self.ferramenta().is_some_and(Ferramenta::copia_da_origem);
        let ponto = self
            .ponteiro
            .filter(|p| self.palco.contains(p))
            .and_then(|p| self.na_foto(p));
        let (Some((x, y)), true, true, false, true) = (
            ponto,
            carimbo,
            self.sobreposicao_do_carimbo,
            self.pintando,
            self.giro == 0.0,
        ) else {
            self.previa_do_carimbo = None;
            return;
        };
        let Some(s) = self.sessao() else {
            return;
        };
        let Some(mira) = s.mira_do_carimbo(x, y) else {
            self.previa_do_carimbo = None;
            return;
        };
        let raio = s.pincel.raio;
        let chave: ChaveDaPrevia = (
            s.versao(),
            (x.round() as i32, y.round() as i32),
            raio.round() as u32,
            (mira.0.round() as i32, mira.1.round() as i32),
            s.carimbo.amostra,
        );
        if self
            .previa_do_carimbo
            .as_ref()
            .is_some_and(|(c, _, _)| *c == chave)
        {
            return;
        }
        let Some((ret, rgba)) = s.previa_do_carimbo(x, y, raio) else {
            self.previa_do_carimbo = None;
            return;
        };
        let mut bytes = rgba.into_raw();
        for p in bytes.as_chunks_mut::<4>().0 {
            p.swap(0, 2);
        }
        self.previa_do_carimbo =
            crate::imagem::de_bgra(ret.largura, ret.altura, bytes).map(|img| (chave, ret, img));
    }

    /// A prévia da origem está montada (para os testes).
    pub fn tem_previa_do_carimbo(&self) -> bool {
        self.previa_do_carimbo.is_some()
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

    // ------------------------------------------------------------- curvas

    /// O ponto do gráfico das Curvas (0..=255, saída para cima) sob `p`
    /// (pontos da janela), e se ele está dentro do gráfico com folga.
    fn na_curva(&self, p: Point<Pixels>) -> (u8, u8, bool) {
        let c = self.caixa_da_curva;
        let (l, a) = (f(c.size.width).max(1.0), f(c.size.height).max(1.0));
        let fx = (f(p.x) - f(c.origin.x)) / l;
        let fy = 1.0 - (f(p.y) - f(c.origin.y)) / a;
        let folga = 24.0 / l;
        let dentro = (-folga..=1.0 + folga).contains(&fx) && (-folga..=1.0 + folga).contains(&fy);
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        (q(fx), q(fy), dentro)
    }

    pub fn escolher_canal_da_curva(&mut self, canal: usize, cx: &mut Context<Self>) {
        self.canal_da_curva = canal.min(3);
        self.ponto_da_curva = None;
        cx.notify();
    }

    pub fn canal_da_curva(&self) -> usize {
        self.canal_da_curva
    }

    /// Apertar no gráfico: perto de um ponto (8 pontos da tela), pega ele;
    /// senão cria um ponto ali (até 14) e pega o novo.
    fn apertar_na_curva(&mut self, p: Point<Pixels>, cx: &mut Context<Self>) {
        let canal = self.canal_da_curva;
        let Some(ajuste) = self.sessao().and_then(|s| s.camada_ativa().ajuste) else {
            return;
        };
        let Some(mut curva) = curva_do_canal(&ajuste, canal) else {
            return;
        };
        let (x, y, _) = self.na_curva(p);
        let escala = 255.0 / f(self.caixa_da_curva.size.width).max(1.0);
        let perto = curva
            .pontos()
            .iter()
            .position(|q| (q[0] as f32 - x as f32).hypot(q[1] as f32 - y as f32) <= 8.0 * escala);
        let indice = match perto {
            Some(i) => i,
            None => match curva.com_ponto(x, y) {
                Some(i) => {
                    if let Some(s) = self.sessao_mut() {
                        s.mover_ajuste(com_curva(ajuste, canal, curva));
                    }
                    i
                }
                None => {
                    self.aviso = Some(("As Curvas aceitam até 14 pontos".into(), true));
                    cx.notify();
                    return;
                }
            },
        };
        self.ponto_da_curva = Some(indice);
        self.arrasto_da_curva = Some(ArrastoDaCurva {
            canal,
            indice,
            curva,
        });
        cx.notify();
    }

    /// O arrasto do ponto: dentro do gráfico ele anda (entre os vizinhos);
    /// arrastado para fora, sai — como no Photoshop. Sempre a partir da curva
    /// do começo do arrasto.
    fn arrastar_na_curva(&mut self, p: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(g) = self.arrasto_da_curva else {
            return;
        };
        let (x, y, dentro) = self.na_curva(p);
        let mut curva = g.curva;
        let ultimo = curva.pontos().len() - 1;
        if !dentro && g.indice != 0 && g.indice != ultimo {
            curva.sem_ponto(g.indice);
            self.ponto_da_curva = None;
        } else {
            curva.mover(g.indice, x, y);
            self.ponto_da_curva = Some(g.indice);
        }
        let inicio = Instant::now();
        if let Some(s) = self.sessao_mut() {
            if let Some(a) = s.camada_ativa().ajuste {
                s.mover_ajuste(com_curva(a, g.canal, curva));
            }
        }
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
        cx.notify();
    }

    /// O soltar: o arrasto inteiro (ou o clique que criou o ponto) vira um
    /// passo do desfazer.
    fn soltar_na_curva(&mut self, cx: &mut Context<Self>) {
        if self.arrasto_da_curva.take().is_some() {
            if let Some(s) = self.sessao_mut() {
                s.confirmar_ajuste();
            }
            cx.notify();
        }
    }

    /// "Redefinir" do canal à vista: a reta, num passo.
    pub fn redefinir_curva(&mut self, cx: &mut Context<Self>) {
        let canal = self.canal_da_curva;
        self.ponto_da_curva = None;
        if let Some(s) = self.sessao_mut() {
            if let Some(a) = s.camada_ativa().ajuste {
                s.mover_ajuste(com_curva(
                    a,
                    canal,
                    editor_core::ajuste::Curva::identidade(),
                ));
                s.confirmar_ajuste();
            }
        }
        cx.notify();
    }

    /// 🧪 O roteiro e os testes: um ponto `(x, y)` posto no canal à vista,
    /// num passo (o clique sem arrasto).
    pub fn ponto_na_curva(&mut self, x: u8, y: u8, cx: &mut Context<Self>) {
        let canal = self.canal_da_curva;
        if let Some(s) = self.sessao_mut() {
            if let Some(a) = s.camada_ativa().ajuste {
                if let Some(mut c) = curva_do_canal(&a, canal) {
                    c.com_ponto(x, y);
                    s.mover_ajuste(com_curva(a, canal, c));
                    s.confirmar_ajuste();
                }
            }
        }
        cx.notify();
    }

    /// O editor das Curvas: o canal, o gráfico com a reta de referência, a
    /// grade em quartos, a curva e os pontos, e a entrada/saída do ponto
    /// escolhido.
    fn editor_de_curvas(&self, ajuste: Ajuste, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::button::{Button, ButtonGroup};
        let tema = cx.theme().clone();
        let canal = self.canal_da_curva;
        let Some(curva) = curva_do_canal(&ajuste, canal) else {
            return div().into_any_element();
        };
        let cor = match canal {
            1 => gpui_kit::hsla(0.0, 0.8, 0.55, 1.0),
            2 => gpui_kit::hsla(0.33, 0.7, 0.45, 1.0),
            3 => gpui_kit::hsla(0.6, 0.8, 0.6, 1.0),
            _ => tema.foreground,
        };
        let pontos: Vec<[u8; 2]> = curva.pontos().to_vec();
        let amostras: Vec<f32> = (0..=64)
            .map(|k| curva.valor(k as f32 * 255.0 / 64.0))
            .collect();
        let escolhido = self.ponto_da_curva.filter(|i| *i < pontos.len());
        let medidor = cx.entity();
        let ouvinte = cx.entity();
        let arrastando = self.arrasto_da_curva.is_some();
        let linha = tema.border;
        let grafico = canvas(
            move |bounds, _window, cx| {
                medidor.update(cx, |ed, _| ed.caixa_da_curva = bounds);
            },
            move |bounds, _, window, _| {
                let (ox, oy) = (f(bounds.origin.x), f(bounds.origin.y));
                let (l, a) = (f(bounds.size.width), f(bounds.size.height));
                let ponto = |x: f32, y: f32| {
                    gpui_kit::point(px(ox + x / 255.0 * l), px(oy + (1.0 - y / 255.0) * a))
                };
                // A grade em quartos e a reta de referência.
                for k in 1..4 {
                    let v = k as f32 * 255.0 / 4.0;
                    for (de, ate) in [((v, 0.0), (v, 255.0)), ((0.0, v), (255.0, v))] {
                        let mut t = PathBuilder::stroke(px(1.0));
                        t.move_to(ponto(de.0, de.1));
                        t.line_to(ponto(ate.0, ate.1));
                        if let Ok(c) = t.build() {
                            window.paint_path(c, linha);
                        }
                    }
                }
                let mut t = PathBuilder::stroke(px(1.0));
                t.move_to(ponto(0.0, 0.0));
                t.line_to(ponto(255.0, 255.0));
                if let Ok(c) = t.build() {
                    window.paint_path(c, linha);
                }
                // A curva.
                let mut t = PathBuilder::stroke(px(2.0));
                t.move_to(ponto(0.0, amostras[0]));
                for (k, y) in amostras.iter().enumerate().skip(1) {
                    t.line_to(ponto(k as f32 * 255.0 / 64.0, *y));
                }
                if let Ok(c) = t.build() {
                    window.paint_path(c, cor);
                }
                // 🔑 O arrasto é ouvido na janela: o ponto continua quando o
                // ponteiro sai do gráfico (e sai da curva lá fora).
                if !arrastando {
                    return;
                }
                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |e: &MouseMoveEvent, fase, _w, cx| {
                        if fase.bubble() {
                            esta.update(cx, |ed, cx| ed.arrastar_na_curva(e.position, cx));
                        }
                    }
                });
                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |e: &MouseUpEvent, fase, _w, cx| {
                        if fase.bubble() && e.button == MouseButton::Left {
                            esta.update(cx, |ed, cx| ed.soltar_na_curva(cx));
                        }
                    }
                });
            },
        )
        .absolute()
        .inset_0();
        let marcas = pontos.iter().enumerate().map(|(i, p)| {
            let (x, y) = (
                p[0] as f32 / 255.0 * LADO_DA_CURVA,
                (1.0 - p[1] as f32 / 255.0) * LADO_DA_CURVA,
            );
            div()
                .debug_selector(move || format!("editor-curva-ponto-{i}"))
                .absolute()
                .left(px(x - 4.0))
                .top(px(y - 4.0))
                .size(px(8.0))
                .border_1()
                .border_color(tema.foreground)
                .when(escolhido == Some(i), |d| d.bg(tema.foreground))
                .when(escolhido != Some(i), |d| d.bg(tema.background))
        });
        let leitura = match escolhido {
            Some(i) => format!("Entrada {} · Saída {}", pontos[i][0], pontos[i][1]),
            None => "Clique para pôr um ponto (até 14); arraste um ponto para fora do gráfico para tirá-lo".into(),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().debug_selector(|| "editor-curva-canal".into()).child(
                ButtonGroup::new("editor-curva-canais").xsmall().children(
                    CANAIS_DA_CURVA.iter().enumerate().map(|(k, nome)| {
                        let b = Button::new(("editor-curva-canal", k)).label(*nome);
                        let b = if k == canal { b.primary() } else { b.outline() };
                        b.on_click(
                            cx.listener(move |ed, _, _, cx| ed.escolher_canal_da_curva(k, cx)),
                        )
                    }),
                ),
            ))
            .child(
                div()
                    .id("editor-curva")
                    .debug_selector(|| "editor-curva".into())
                    .relative()
                    .size(px(LADO_DA_CURVA))
                    .bg(tema.muted)
                    .border_1()
                    .border_color(tema.border)
                    .cursor_crosshair()
                    .child(grafico)
                    .children(marcas)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|ed, e: &MouseDownEvent, _w, cx| {
                            cx.stop_propagation();
                            ed.apertar_na_curva(e.position, cx);
                        }),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "editor-curva-leitura".into())
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(leitura),
            )
            .child(
                div().flex().justify_end().child(
                    crate::estilo::botao_fantasma_pequeno("editor-curva-redefinir", cx)
                        .debug_selector(|| "editor-curva-redefinir".into())
                        .label("Redefinir")
                        .tooltip("O canal à vista volta à reta")
                        .disabled(curva.neutra())
                        .on_click(cx.listener(|ed, _, _, cx| ed.redefinir_curva(cx))),
                ),
            )
            .into_any_element()
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
                    .child(ajuste.nome().to_string()),
            );
        if let Ajuste::Curvas { .. } = ajuste {
            return coluna
                .child(self.editor_de_curvas(ajuste, cx))
                .into_any_element();
        }
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
        if self.recusar_se_bloqueada("preencher", cx) {
            return;
        }
        let escondida = self.sessao().is_some_and(|s| !s.camada_ativa().visivel);
        let inicio = Instant::now();
        self.na_sessao(cx, |s| {
            s.lata_de_tinta(x, y);
        });
        self.medidas.ultimo_gesto = Some(inicio.elapsed());
        if escondida {
            self.aviso = Some((
                format!(
                    "A camada está escondida — mostre-a (o olho, ou {MOSTRAR}) para pintar nela"
                )
                .into(),
                true,
            ));
        }
    }

    /// O degradê de `de` a `ate`, pixels da foto — o roteiro e os testes.
    pub fn degrade(&mut self, de: (f32, f32), ate: (f32, f32), cx: &mut Context<Self>) {
        if self.recusar_se_bloqueada("aplicar o degradê", cx) {
            return;
        }
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
        if self.recusar_se_bloqueada("recortar", cx) {
            return;
        }
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

    /// "Duplicar camada": a escolhida inteira, exata — mesmo com seleção (o
    /// ⌘J com seleção é a camada via cópia).
    pub fn duplicar_camada_inteira(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, Sessao::duplicar_camada);
    }

    /// "Criar camada da fotografia base": a camada opaca da base, logo acima
    /// dela, montada em segundo plano.
    pub fn criar_camada_da_fotografia(&mut self, cx: &mut Context<Self>) {
        if self.criando_a_fotografia || self.area_do_preenchimento.is_some() {
            return;
        }
        let Some(base) = self.sessao().map(|s| s.base().clone()) else {
            return;
        };
        self.criando_a_fotografia = true;
        self.aviso = Some(("Criando a camada da fotografia…".into(), false));
        cx.notify();
        let trabalho = cx
            .background_executor()
            .spawn(async move { editor_core::CamadaDePixels::da_imagem(&base) });
        self._tarefa_da_fotografia = Some(cx.spawn(async move |esta, cx| {
            let pixels = trabalho.await;
            let _ = esta.update(cx, |ed, cx| {
                ed.criando_a_fotografia = false;
                ed.na_sessao(cx, |s| {
                    s.criar_camada_da_fotografia(pixels);
                });
            });
        }));
    }

    pub fn criando_a_fotografia(&self) -> bool {
        self.criando_a_fotografia
    }

    /// ⌥ + clique na divisa entre a camada `indice` e a de baixo (e ⌥⌘G na
    /// escolhida): cria ou libera a máscara de corte.
    pub fn alternar_mascara_de_corte(&mut self, indice: usize, cx: &mut Context<Self>) {
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let feito = s.alternar_mascara_de_corte(indice);
        self.aviso = (!feito).then(|| {
            (
                SharedString::from(if indice == 0 {
                    "A camada de baixo de todas não tem base para recortar"
                } else {
                    "A máscara de corte precisa de uma camada de pixels embaixo"
                }),
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

    /// ⇧⌘D.
    pub fn reselecionar(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, Sessao::reselecionar);
    }

    /// ⌘L ⌘M ⌘U: a camada de ajuste daquele tipo (Imagem › Ajustes).
    pub fn ajuste_pela_chave(&mut self, chave: &str, cx: &mut Context<Self>) {
        if let Some(a) = editor_core::ajuste::Ajuste::da_chave(chave) {
            self.nova_camada_de_ajuste(a, cx);
        }
    }

    pub fn inverter_selecao(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, Sessao::inverter_selecao);
    }

    /// Delete: apaga a seleção na camada escolhida.
    pub fn apagar_selecao(&mut self, cx: &mut Context<Self>) {
        if self.recusar_se_bloqueada("apagar", cx) {
            return;
        }
        self.na_sessao(cx, |s| {
            s.apagar_selecao();
        });
    }

    /// ⌥Delete: preenche a seleção (ou a camada) com a cor do pincel.
    pub fn preencher_selecao(&mut self, cx: &mut Context<Self>) {
        if self.recusar_se_bloqueada("preencher", cx) {
            return;
        }
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let escondida = !s.camada_ativa().visivel;
        s.preencher_selecao();
        self.aviso = escondida.then(|| {
            (
                SharedString::from(format!(
                    "A camada está escondida — mostre-a (o olho, ou {MOSTRAR}) para preencher"
                )),
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
        if self.miniatura_do_fundo.is_none() {
            let reduzida = image::imageops::thumbnail(s.base().as_ref(), l, a);
            let rgba = reduzida
                .pixels()
                .flat_map(|p| [p[2], p[1], p[0], 255])
                .collect();
            self.miniatura_do_fundo = crate::imagem::de_bgra(l, a, rgba);
        }
        let Some(s) = self.sessao() else {
            return;
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
        if self.area_do_preenchimento.is_some() {
            // ⌘S com o modal aberto não grava; "Salvar" na pergunta de
            // fechar a janela larga o modal (o visualizado não entra) e grava.
            if !e_fechar {
                return;
            }
            self.area_do_preenchimento = None;
        }
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
            // janela apertar|soltar|clicar|duplo|mover x y [mods] — em pontos da
            // janela (a barra de opções, o painel, os diálogos).
            "janela" => {
                let tipo = partes.get(1).copied().unwrap_or("clicar");
                let mods = partes
                    .iter()
                    .skip(4)
                    .map(|n| match *n {
                        "shift" => 1 << 17,
                        "alt" => 1 << 19,
                        "cmd" => 1 << 20,
                        _ => 0,
                    })
                    .fold(0, |a, b| a | b);
                let r = crate::depuracao::mouse_nativo(window, tipo, numero(2), numero(3), mods);
                eprintln!(
                    "[roteiro] editor janela {tipo} ({}, {}): {r:?}",
                    numero(2),
                    numero(3)
                );
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
                    "duplicar-exata" => self.duplicar_camada_inteira(cx),
                    // camada fotografia — a camada da fotografia base (em
                    // segundo plano: um `espera` depois).
                    "fotografia" => self.criar_camada_da_fotografia(cx),
                    // camada recorte N — ⌥ + clique na divisa sobre N.
                    "recorte" => self.alternar_mascara_de_corte(indice, cx),
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
            // liquidificar comecar|aplicar|cancelar|restaurar|forca P|
            //              trecho x0 y0 x1 y1 (pixels da foto)
            "liquidificar" => match partes.get(1).copied().unwrap_or("comecar") {
                "comecar" => self.liquidificar(cx),
                "aplicar" => self.aplicar_transformacao(cx),
                "cancelar" => self.cancelar_transformacao(cx),
                "restaurar" => self.restaurar_liquidificacao(cx),
                "forca" => {
                    if let Some(s) = self.sessao_mut() {
                        s.forca_do_liquido = numero(2) / 100.0;
                    }
                    cx.notify();
                }
                "trecho" => {
                    self.liquidificar_trecho((numero(2), numero(3)), (numero(4), numero(5)), cx)
                }
                outro => eprintln!("[roteiro] editor liquidificar {outro}?"),
            },
            // remendo dx dy (pixels da foto)
            "remendo" => self.remendar(numero(1), numero(2), cx),
            // curva canal 0..3 | curva ponto x y | curva redefinir
            "curva" => match partes.get(1).copied().unwrap_or_default() {
                "canal" => self.escolher_canal_da_curva(numero(2) as usize, cx),
                "ponto" => self.ponto_na_curva(numero(2) as u8, numero(3) as u8, cx),
                "redefinir" => self.redefinir_curva(cx),
                outro => eprintln!("[roteiro] editor curva {outro}?"),
            },
            // deformar comecar|livre|redefinir|grade|aplicar|cancelar|
            //          ponto L C dx dy (pixels da foto)|estado
            "deformar" => match partes.get(1).copied().unwrap_or("comecar") {
                "comecar" => self.deformar(cx),
                "livre" => self.voltar_a_transformacao_livre(cx),
                "redefinir" => self.redefinir_malha(cx),
                "grade" => self.alternar_grade(cx),
                "aplicar" => self.aplicar_transformacao(cx),
                "cancelar" => self.cancelar_transformacao(cx),
                "ponto" => self.mover_ponto_da_malha(
                    numero(2) as usize,
                    numero(3) as usize,
                    numero(4),
                    numero(5),
                    cx,
                ),
                _ => eprintln!(
                    "[roteiro] editor deformar: malha={:?} tempo={:?}",
                    self.sessao()
                        .and_then(Sessao::malha)
                        .map(|(c, m)| (c, m.pontos)),
                    self.medidas.ultimo_gesto
                ),
            },
            // tamanho <largura> <altura> — a janela em pontos (1366 768 …).
            "tamanho" => {
                window.resize(gpui_kit::size(px(numero(1)), px(numero(2))));
                eprintln!(
                    "[roteiro] editor tamanho pedido {}×{}; escala {}",
                    numero(1),
                    numero(2),
                    window.scale_factor()
                );
            }
            // area tab|shift-tab|recolher|colunas|restaurar|mostrar <painel>|
            // esconder <painel>|flyout <grupo>|estado
            "area" => match partes.get(1).copied().unwrap_or_default() {
                "tab" => self.alternar_interface(cx),
                "shift-tab" => self.alternar_paineis(cx),
                "recolher" => self.alternar_recolhido(cx),
                "colunas" => self.alternar_colunas_da_barra(cx),
                "restaurar" => self.restaurar_area_de_trabalho(window, cx),
                "flyout" => self.abrir_flyout(numero(2) as usize, cx),
                "mostrar" | "esconder" => {
                    let nome = format!("editor:{}", partes.get(2).copied().unwrap_or_default());
                    if let Some(q) = QualPainel::do_nome(&nome) {
                        if partes[1] == "mostrar" {
                            self.mostrar_painel(q, window, cx)
                        } else {
                            self.definir_painel_oculto(q, true, cx)
                        }
                    }
                }
                _ => {
                    let v = window.viewport_size();
                    eprintln!(
                        "[roteiro] editor area: janela={:.0}×{:.0} escala={} palco={:?} interface_oculta={} paineis_ocultos={} recolhido={} ocultos={:?} duas_colunas={} flyout={:?} gravacoes={} ladrilhos={} largura={:.0}",
                        f(v.width),
                        f(v.height),
                        window.scale_factor(),
                        self.palco,
                        self.interface_oculta,
                        self.paineis_ocultos,
                        self.arranjo.recolhido,
                        self.arranjo.ocultos,
                        self.barra_em_duas_colunas,
                        self.flyout,
                        self.gravacoes_da_area,
                        self.ladrilhos.len(),
                        self.largura_do_painel(),
                    );
                }
            },
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
                    Some("cruzar") => Operacao::Intersecao,
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
                    // selecao difundir|expandir|contrair N — direto, sem o diálogo
                    "difundir" | "expandir" | "contrair" => {
                        let px = numero(2).round().max(1.0) as u32;
                        let m = match partes[1] {
                            "difundir" => Modificacao::Difundir,
                            "expandir" => Modificacao::Expandir,
                            _ => Modificacao::Contrair,
                        };
                        self.modificar_selecao(m, px, cx);
                        None
                    }
                    // selecao modificar difundir|expandir|contrair — abre o diálogo
                    "modificar" => {
                        let m = match partes.get(2).copied() {
                            Some("expandir") => Modificacao::Expandir,
                            Some("contrair") => Modificacao::Contrair,
                            _ => Modificacao::Difundir,
                        };
                        self.abrir_modificacao(m, window, cx);
                        None
                    }
                    "transformar" => {
                        self.transformar_selecao(cx);
                        None
                    }
                    // selecao modo nova|somar|subtrair|cruzar
                    "modo" => {
                        let m = match partes.get(2).copied() {
                            Some("somar") => Operacao::Somar,
                            Some("subtrair") => Operacao::Subtrair,
                            Some("cruzar") => Operacao::Intersecao,
                            _ => Operacao::Nova,
                        };
                        self.escolher_modo_de_selecao(m, cx);
                        None
                    }
                    // selecao estilo normal|proporcao L A|tamanho L A
                    "estilo" => {
                        if let Some(tipo) = self.selecionando {
                            let o = self.opcoes_da_forma_mut(tipo);
                            match partes.get(2).copied() {
                                Some("proporcao") => {
                                    o.estilo = TipoDeEstilo::Proporcao;
                                    o.proporcao = (numero(3), numero(4));
                                }
                                Some("tamanho") => {
                                    o.estilo = TipoDeEstilo::Tamanho;
                                    o.tamanho = (numero(3) as u32, numero(4) as u32);
                                }
                                _ => o.estilo = TipoDeEstilo::Normal,
                            }
                            self.campos_de = None;
                        }
                        None
                    }
                    // selecao difusao N | selecao suavizar sim|nao | selecao amostra camada|todas
                    "difusao" => {
                        if let Some(tipo) = self.selecionando {
                            self.opcoes_da_forma_mut(tipo).acabamento.difusao = numero(2) as u32;
                            self.campos_de = None;
                        }
                        None
                    }
                    "suavizar" => {
                        let sim = partes.get(2) != Some(&"nao");
                        if self.auxiliar == Some(Auxiliar::Varinha) {
                            self.opcoes_da_varinha.suavizar = sim;
                        } else if let Some(tipo) = self.selecionando {
                            self.opcoes_da_forma_mut(tipo).acabamento.suavizar = sim;
                        }
                        None
                    }
                    "amostra" => {
                        self.opcoes_da_varinha.amostra = if partes.get(2) == Some(&"camada") {
                            AmostraDaVarinha::CamadaAtual
                        } else {
                            AmostraDaVarinha::Todas
                        };
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
                        let v = numero(2).clamp(0.0, 255.0);
                        self.opcoes_da_varinha.tolerancia = v as u8;
                        self.campo_da_tolerancia
                            .update(cx, |c, cx| c.set_value(format!("{v:.0}"), window, cx));
                        None
                    }
                    "contigua" => {
                        self.opcoes_da_varinha.contigua = partes.get(2) != Some(&"nao");
                        None
                    }
                    "ferramenta" => {
                        match partes.get(2).copied() {
                            Some("retangulo") => self.usar_selecao(TipoDeSelecao::Retangulo, cx),
                            Some("elipse") => self.usar_selecao(TipoDeSelecao::Elipse, cx),
                            Some("laco") => self.usar_selecao(TipoDeSelecao::Laco, cx),
                            Some("poligonal") => {
                                self.usar_selecao(TipoDeSelecao::LacoPoligonal, cx)
                            }
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
            // preenchimento abrir|visualizar|aplicar|cancelar|original|redefinir |
            // metodo patchmatch|lama | backend CHAVE | contexto N | suavizar N |
            // pincel amostra|remover [excluir] | camada-nova sim|nao
            "preenchimento" => {
                use painel_do_preenchimento::AlvoDoPincel as Alvo;
                match partes.get(1).copied().unwrap_or_default() {
                    "abrir" => self.abrir_preenchimento(cx),
                    "aplicar" => self.aplicar_preenchimento(cx),
                    "visualizar" => self.visualizar_preenchimento(cx),
                    "cancelar" => self.cancelar_preenchimento(cx),
                    "original" => self.alternar_original(cx),
                    "redefinir" => self.redefinir_amostragem(cx),
                    "baixar" => self.baixar_modelo(cx),
                    "remover" => self.remover_modelo(cx),
                    "importar" => {
                        let caminho = partes.get(2..).map(|p| p.join(" ")).unwrap_or_default();
                        self.importar_modelo_de(caminho.into(), cx);
                    }
                    "metodo" => {
                        if let Some(m) = partes
                            .get(2)
                            .and_then(|c| preenchimento::Metodo::da_chave(c))
                        {
                            self.seletor_de_metodo.update(cx, |s, cx| {
                                s.set_selected_value(&m.chave().to_string(), window, cx)
                            });
                            self.mudar_metodo_do_preenchimento(m, cx);
                        }
                    }
                    "backend" => {
                        if let Some(b) = partes
                            .get(2)
                            .and_then(|c| ia_local::execucao::Backend::da_chave(c))
                        {
                            self.seletor_de_backend.update(cx, |s, cx| {
                                s.set_selected_value(&b.chave().to_string(), window, cx)
                            });
                            self.mudar_backend_da_ia(b, cx);
                        }
                    }
                    "contexto" => {
                        let v = numero(2);
                        self.contexto_da_ia
                            .update(cx, |s, cx| s.set_value(v, window, cx));
                        self.invalidar_previa(cx);
                    }
                    "suavizar" => {
                        let v = numero(2);
                        self.suavizacao
                            .update(cx, |s, cx| s.set_value(v, window, cx));
                        self.suavizacao_mudou(cx);
                    }
                    "pincel" => {
                        let incluir = partes.get(3) != Some(&"excluir");
                        let alvo = if partes.get(2) == Some(&"amostra") {
                            Alvo::Amostragem
                        } else {
                            Alvo::Destino
                        };
                        self.escolher_alvo_do_pincel(alvo, incluir, cx);
                    }
                    "camada-nova" => {
                        if let Some(e) = self.area_do_preenchimento.as_mut() {
                            e.saida = if partes.get(2) == Some(&"nao") {
                                editor_core::SaidaDoPreenchimento::CamadaAtual
                            } else {
                                painel_do_preenchimento::saida_da_chave(
                                    partes.get(2).copied().unwrap_or("nova"),
                                )
                                .unwrap_or_default()
                            };
                        }
                    }
                    outro => eprintln!("[roteiro] editor preenchimento {outro}?"),
                }
            }
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
                // pincel tamanho|dureza|opacidade|fluxo|espacamento|suavizacao V
                // (V em %, o tamanho em px), ou pincel predefinicao <nome…>
                if partes.get(1) == Some(&"predefinicao") {
                    let nome = partes[2..].join(" ");
                    self.usar_predefinicao(&nome, cx);
                    return;
                }
                if let Some(s) = self.sessao_mut() {
                    match partes.get(1).copied() {
                        Some("tamanho") => s.pincel.raio = v.max(1.0),
                        Some("dureza") => s.pincel.dureza = v / 100.0,
                        Some("fluxo") => s.pincel.fluxo = v / 100.0,
                        Some("espacamento") => s.pincel.espacamento = v / 100.0,
                        Some("suavizacao") => s.pincel.suavizacao = v / 100.0,
                        Some("difusao") => s.pincel.difusao = v as u8,
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
                "recuperacao" => self.usar(Ferramenta::Recuperacao, cx),
                "remendo" => self.usar_auxiliar(Auxiliar::Remendo, cx),
                "conta-gotas" => self.usar_auxiliar(Auxiliar::ContaGotas, cx),
                "mover" => self.usar_auxiliar(Auxiliar::Mover, cx),
                "correcao" => self.usar_auxiliar(Auxiliar::Correcao, cx),
                "mao" => self.usar_auxiliar(Auxiliar::Mao, cx),
                "lupa" => self.usar_auxiliar(Auxiliar::Zoom, cx),
                "degrade" => self.usar_auxiliar(Auxiliar::Degrade, cx),
                "lata" => self.usar_auxiliar(Auxiliar::Lata, cx),
                "varinha" => self.usar_auxiliar(Auxiliar::Varinha, cx),
                "girar" => self.usar_auxiliar(Auxiliar::GirarVista, cx),
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
            // giro <graus> — gira só a vista.
            "giro" => self.girar_a_vista(numero(1).to_radians(), cx),
            "espaco" => match partes.get(1).copied() {
                Some("segurar") => self.espaco_apertado(cx),
                _ => self.espaco_solto(cx),
            },
            // filtro desfoque|nitidez|raio V|quantidade V|limiar V|visualizar|ok|cancelar|estado
            "filtro" => match partes.get(1).copied().unwrap_or_default() {
                "desfoque" => self.abrir_filtro(filtro::Tipo::Desfoque, window, cx),
                "nitidez" => self.abrir_filtro(filtro::Tipo::Nitidez, window, cx),
                q @ ("raio" | "quantidade" | "limiar") => {
                    self.definir_controle_do_filtro(q, numero(2), window, cx)
                }
                "visualizar" => self.alternar_visualizar_o_filtro(cx),
                "ok" => self.confirmar_filtro(window, cx),
                "cancelar" => self.cancelar_o_filtro(window, cx),
                _ => eprintln!(
                    "[roteiro] editor filtro: aberto={:?} controles={:?} previa_pronta={}",
                    self.filtro_aberto(),
                    self.filtro_dos_controles(cx),
                    self.previa_do_filtro_pronta()
                ),
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
            // Etapa 16: as Propriedades da máscara, os cadeados e a área de
            // transferência (o mesmo caminho dos botões e das teclas).
            "mascara" => match partes.get(1).copied().unwrap_or_default() {
                "densidade" => self.mover_densidade_da_mascara(numero(2), true, cx),
                "difusao" => self.mover_difusao_da_mascara(numero(2), true, cx),
                "inverter" => self.inverter_mascara(cx),
                "aplicar" => self.aplicar_mascara(cx),
                "vinculo" => self.alternar_vinculo_de_ativa(cx),
                "ver" => {
                    if let Some(i) = self.sessao().map(Sessao::ativa) {
                        self.alternar_so_a_mascara(i, cx);
                    }
                }
                "rubi" => self.alternar_rubi(cx),
                outro => eprintln!("[roteiro] editor: mascara {outro}?"),
            },
            "cadeado" => {
                let cadeado = match partes.get(1).copied().unwrap_or_default() {
                    "transparencia" => editor_core::Cadeado::Transparencia,
                    "pixels" => editor_core::Cadeado::Pixels,
                    "posicao" => editor_core::Cadeado::Posicao,
                    _ => editor_core::Cadeado::Tudo,
                };
                if let Some(i) = self.sessao().map(Sessao::ativa) {
                    self.alternar_bloqueio_de(i, cadeado, cx);
                }
            }
            "copiar" => self.copiar(partes.get(1) == Some(&"mesclado"), cx),
            "recortar" => self.recortar(cx),
            "colar" => self.colar(partes.get(1) == Some(&"lugar"), cx),
            "carimbar" => self.carimbar_visivel(cx),
            "importar" => {
                if let Some(caminho) = partes.get(1) {
                    self.importar_arquivo(std::path::PathBuf::from(caminho), cx);
                }
            }
            "arrastar_camada" => self.arrastar_camada(numero(1) as usize, numero(2) as usize, cx),
            "estado" => {
                let camadas = self.sessao().map_or_else(String::new, |s| {
                    s.documento()
                        .camadas
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            format!(
                                "{}{}{}:{}:{}:{:.0}%:{}tiles{}",
                                if i == s.ativa() { "*" } else { "" },
                                if c.recortada { "↳" } else { "" },
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
                                        extras_da_mascara(m)
                                    ))
                                    + &if c.bloqueio.algum() {
                                        format!(":bloqueio{:?}", c.bloqueio)
                                    } else {
                                        String::new()
                                    }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" | ")
                });
                let selecao = self
                    .sessao()
                    .and_then(|s| s.selecao())
                    .map(|s| format!("{:?}", s.caixa_justa()));
                eprintln!(
                    "[roteiro] editor selecao: modo={:?} opcoes={:?} varinha={:?} poligono={:?} medida={:?} modificando={:?} transformando_selecao={}",
                    self.modo_de_selecao,
                    self.selecionando.map(|t| self.opcoes_da_forma(t)),
                    self.opcoes_da_varinha,
                    self.vertices_do_poligono(),
                    self.medida_do_gesto(),
                    self.modificando(),
                    self.sessao().is_some_and(Sessao::transformando_a_selecao),
                );
                let lupa = self
                    .sessao()
                    .and_then(|s| s.lupa())
                    .map(|l| format!("fator {} {:?}", l.fator(), l.regiao()));
                if let Some(e) = self.area_do_preenchimento.as_ref() {
                    eprintln!(
                        "[roteiro] preenchimento: metodo={:?} backend={:?} estado={:?} destino={:?} amostragem={:?} resultado={:?} tempo={:?} aviso_do_modelo={:?}",
                        e.metodo,
                        e.backend,
                        e.estado,
                        e.destino.caixa_justa(),
                        e.amostragem.caixa_justa(),
                        e.resultado.as_ref().map(|r| (r.ret, r.fator, r.reducao_do_motor, r.executado_em.clone())),
                        e.tempo,
                        e.aviso_do_modelo
                    );
                }
                let pincel = self.sessao().map(|s| {
                    let p = s.pincel;
                    format!(
                        "raio={:.0} dureza={:.2} opacidade={:.2} fluxo={:.2} espacamento={:.2} suavizacao={:.2}",
                        p.raio, p.dureza, p.opacidade, p.fluxo, p.espacamento, p.suavizacao
                    )
                });
                let historico = self.sessao().map(|s| {
                    s.historico()
                        .passos()
                        .iter()
                        .map(|p| p.descricao(s.documento()))
                        .collect::<Vec<_>>()
                        .join(" › ")
                });
                eprintln!(
                    "[roteiro] editor: ferramenta={:?} giro={:.1}° pincel={pincel:?} historico={historico:?}",
                    self.item_atual(),
                    self.giro.to_degrees(),
                );
                eprintln!(
                    "[roteiro] editor: exibicao={:?} copiado={:?} carimbando={}",
                    self.exibicao(),
                    self.copiado.as_ref().map(transferencia::Copiado::descricao),
                    self.carimbando,
                );
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
        // Por fora da trava do modal: o teste muda o documento "por baixo".
        if let Some(s) = self.sessao_mut() {
            fazer(s);
        }
        cx.notify();
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
        // O pedaço da foto que o palco mostra — girado, a caixa dos cantos do
        // palco levados à foto.
        let (x0, y0, x1, y1) = giro::Visor {
            vx: v.x,
            vy: v.y,
            escala: v.escala,
            largura: cena.area.largura,
            altura: cena.area.altura,
            angulo: self.giro,
        }
        .caixa_na_foto();
        let (x0, y0) = (x0.clamp(0.0, largura), y0.clamp(0.0, altura));
        let (x1, y1) = (x1.clamp(0.0, largura), y1.clamp(0.0, altura));
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
        let mut sujos = subir(sessao.vista_mut(), &mut self.ladrilhos);
        if let Some(lupa) = sessao.lupa_mut() {
            sujos.extend(subir(lupa, &mut self.ladrilhos_da_lupa));
        }
        self.medidas.ladrilhos_no_quadro = sujos.len();
        // 🔄 A vista girada: os ladrilhos do palco sobre o que sujou, ou todos
        // se o zoom, o giro, o palco ou a lupa mudaram.
        if self.giro == 0.0 {
            self.palco_girado.largar();
            return;
        }
        for r in sujos {
            self.palco_girado.sujar(r);
        }
        let Some(visor) = self.visor() else {
            return;
        };
        let Fase::Pronta(sessao) = &self.fase else {
            return;
        };
        let base = sessao.base();
        let montagem = giro::Montagem {
            visor,
            dpr: self.dpr,
            foto: (base.width(), base.height()),
            vista: giro::Fonte::da_vista(sessao.vista()),
            lupa: sessao.lupa().map(giro::Fonte::da_vista),
        };
        let inicio = Instant::now();
        if self.palco_girado.atualizar(&montagem) > 0 {
            self.medidas.palco_girado = Some(inicio.elapsed());
        }
    }

    fn palco(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let fator_da_tela = window.scale_factor().max(1.0);
        let medidor = cx.entity();
        let ouvinte = cx.entity();
        let pintando = self.em_gesto();
        let com_a_mao = self.mao.is_some();
        let medida = canvas(
            move |bounds, window, cx| {
                medidor.update(cx, |ed, cx| {
                    if ed.palco != bounds {
                        // 🔑 O palco andou (uma barra de opções apareceu ou
                        // sumiu): as marcas dele (caixa, malha, letreiro) foram
                        // desenhadas com a medida velha — mais um quadro.
                        if ed.palco.origin != bounds.origin {
                            cx.notify();
                        }
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
                        angulo: self.giro,
                    };
                    if self.giro != 0.0 {
                        // 🔄 Girada: os ladrilhos do palco montados na CPU.
                        palco = palco.children(tela.ladrilhos_girados(&self.palco_girado));
                    } else {
                        palco = palco.children(tela.ladrilhos(sessao.vista(), &self.ladrilhos));
                        if let Some(lupa) = sessao.lupa() {
                            palco = palco.children(tela.ladrilhos(lupa, &self.ladrilhos_da_lupa));
                            if let Some(nitidos) = tela.pixels_nitidos(lupa) {
                                palco = palco.child(nitidos);
                            }
                        }
                    }
                    // O preenchimento aberto: a sobreposição e a prévia, e o
                    // laço em curso.
                    palco = palco.children(self.elementos_do_preenchimento(&v, cx));
                    if let Some(pontos) = self
                        .area_do_preenchimento
                        .as_ref()
                        .and_then(|e| e.laco.clone())
                        .filter(|p| p.len() > 1)
                    {
                        palco = palco.child(tela.contorno(pontos));
                    }
                    // O Remendo em curso: a seleção levada até a origem.
                    if let (Some((de, ate)), Some(sel)) =
                        (self.arrasto_do_remendo, sessao.selecao())
                    {
                        let c = sel.caixa_justa();
                        let (dx, dy) = (ate.0 - de.0, ate.1 - de.1);
                        let (x0, y0) = (c.x as f32 + dx, c.y as f32 + dy);
                        let (x1, y1) = (c.direita() as f32 + dx, c.baixo() as f32 + dy);
                        palco = palco.child(tela.contorno(vec![
                            (x0, y0),
                            (x1, y0),
                            (x1, y1),
                            (x0, y1),
                            (x0, y0),
                        ]));
                    }
                    // O selo do Antes/Depois.
                    if sessao.mostrando_antes() {
                        palco = palco.child(
                            div()
                                .debug_selector(|| "editor-selo-antes".into())
                                .absolute()
                                .top(px(10.))
                                .left(px(10.))
                                .px(px(8.))
                                .py(px(3.))
                                .rounded(crate::tema::canto(4.))
                                .bg(gpui_kit::black().opacity(0.7))
                                .text_color(gpui_kit::white())
                                .text_xs()
                                .child("Antes — como abriu (Y volta)"),
                        );
                    }
                    // 🕸️ A malha do Deformar: as 8 linhas da grade de 3 × 3
                    // células (sobre a superfície, curvas) e os 16 pontos —
                    // cantos quadrados, alças e internos redondos, com a haste
                    // de cada alça até o canto dela.
                    if let Some((_, malha)) = sessao.malha() {
                        if self.grade_visivel {
                            for linha in malha.linhas_da_grade(24) {
                                palco = palco.child(tela.contorno(linha));
                            }
                        }
                        let p = malha.pontos;
                        for (canto, alca) in [
                            ((0, 0), (0, 1)),
                            ((0, 0), (1, 0)),
                            ((0, 3), (0, 2)),
                            ((0, 3), (1, 3)),
                            ((3, 0), (3, 1)),
                            ((3, 0), (2, 0)),
                            ((3, 3), (3, 2)),
                            ((3, 3), (2, 3)),
                        ] {
                            palco = palco
                                .child(tela.contorno(vec![p[canto.0][canto.1], p[alca.0][alca.1]]));
                        }
                        for (j, linha) in p.iter().enumerate() {
                            for (i, (x, y)) in linha.iter().enumerate() {
                                let canto = (j == 0 || j == 3) && (i == 0 || i == 3);
                                let (sx, sy) = tela.p(*x, *y);
                                let lado = if canto { 9.0 } else { 7.0 };
                                palco = palco.child(
                                    div()
                                        .debug_selector(move || format!("editor-malha-{j}-{i}"))
                                        .absolute()
                                        .left(px(sx - lado / 2.0))
                                        .top(px(sy - lado / 2.0))
                                        .size(px(lado))
                                        .when(!canto, |d| d.rounded_full())
                                        .bg(gpui_kit::white())
                                        .border_1()
                                        .border_color(gpui_kit::black()),
                                );
                            }
                        }
                    } else if let Some((caixa, t)) = sessao.transformacao() {
                        let cantos: Vec<(f32, f32)> =
                            t.cantos(&caixa).iter().map(|(x, y)| (*x, *y)).collect();
                        let mut fechado = cantos.clone();
                        fechado.push(cantos[0]);
                        palco = palco.child(tela.contorno(fechado));
                        // O ponto de referência: um círculo com a cruz.
                        if let Some(r) = self.referencia_da_caixa() {
                            let (qx, qy) = t.aplicar(&caixa, r.0, r.1);
                            let (rx, ry) = tela.p(qx, qy);
                            palco = palco.child(
                                div()
                                    .debug_selector(|| "editor-referencia-da-caixa".into())
                                    .absolute()
                                    .left(px(rx - 6.0))
                                    .top(px(ry - 6.0))
                                    .size(px(12.0))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(gpui_kit::white())
                                    .bg(gpui_kit::black().opacity(0.35)),
                            );
                            for (l, a) in [(16.0, 1.0), (1.0, 16.0)] {
                                palco = palco.child(
                                    div()
                                        .absolute()
                                        .left(px(rx - l / 2.0))
                                        .top(px(ry - a / 2.0))
                                        .w(px(l))
                                        .h(px(a))
                                        .bg(gpui_kit::white().opacity(0.9)),
                                );
                            }
                        }
                        let alcas: Vec<(f32, f32)> = Transformacao::alcas(&caixa)
                            .iter()
                            .map(|(x, y)| t.aplicar(&caixa, *x, *y))
                            .collect();
                        for (x, y) in alcas {
                            let (sx, sy) = tela.p(x, y);
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
                        let pontos: Vec<_> = traco.iter().map(|(x, y)| tela.p(*x, *y)).collect();
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
                        let a = tela.p(de.0, de.1);
                        let b = tela.p(ate.0, ate.1);
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
                        // Largura e altura em pixels do documento, junto do
                        // ponteiro — zoom e giro da vista não as mudam.
                        if let (Some((l, a)), Some(ponteiro)) = (gesto.medida(), self.ponteiro) {
                            let onde = ponteiro - self.palco.origin;
                            palco = palco.child(
                                div()
                                    .debug_selector(|| "editor-medida-da-selecao".into())
                                    .absolute()
                                    .left(onde.x + px(14.))
                                    .top(onde.y + px(14.))
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(crate::tema::canto(4.))
                                    .bg(gpui_kit::black().opacity(0.75))
                                    .text_color(gpui_kit::white())
                                    .text_xs()
                                    .child(format!("L: {l} px   A: {a} px")),
                            );
                        }
                    }
                    // O círculo do pincel, do tamanho que ele pinta.
                    let dentro = self.ponteiro.filter(|p| {
                        self.area_na_janela().is_some_and(|a| a.contains(p))
                            && self.palco.contains(p)
                    });
                    let com_pincel = self.area_do_preenchimento.is_some()
                        || (self.selecionando.is_none()
                            && self.auxiliar.is_none()
                            && !sessao.transformando())
                        || sessao.liquidificando();
                    // A prévia da origem dentro do círculo do pincel.
                    if let (Some(ponteiro), Some((_, ret, imagem))) =
                        (dentro, self.previa_do_carimbo.as_ref())
                    {
                        let raio = sessao.pincel.raio * v.escala;
                        let centro = ponteiro - self.palco.origin;
                        let (cx0, cy0) = (f(centro.x) - raio, f(centro.y) - raio);
                        let (ix, iy) = tela.p(ret.x as f32, ret.y as f32);
                        palco = palco.child(
                            div()
                                .debug_selector(|| "editor-previa-do-carimbo".into())
                                .absolute()
                                .left(px(cx0))
                                .top(px(cy0))
                                .size(px(raio * 2.0))
                                .rounded_full()
                                .overflow_hidden()
                                .opacity(0.75)
                                .child(
                                    img(imagem.clone())
                                        .absolute()
                                        .left(px(ix - cx0))
                                        .top(px(iy - cy0))
                                        .w(px(ret.largura as f32 * v.escala))
                                        .h(px(ret.altura as f32 * v.escala))
                                        .object_fit(ObjectFit::Fill),
                                ),
                        );
                    }
                    // A mira do carimbo: de onde ele copia para o ponteiro.
                    if let (
                        Some(ponteiro),
                        Some(Ferramenta::Carimbo | Ferramenta::Recuperacao),
                        true,
                    ) = (dentro, self.ferramenta(), com_pincel)
                    {
                        let mira = self
                            .na_foto_sem_limite(ponteiro)
                            .and_then(|(x, y)| sessao.mira_do_carimbo(x, y));
                        if let Some((mx, my)) = mira {
                            let (cx_, cy_) = tela.p(mx, my);
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
                } else if self.auxiliar == Some(Auxiliar::GirarVista) {
                    gpui_kit::CursorStyle::PointingHand
                } else if self.ajuste_rapido.is_some() {
                    gpui_kit::CursorStyle::ResizeLeftRight
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
                            } else if ed.selecionando.is_some()
                                && !ed.transformando()
                                && !ed.liquidificando()
                            {
                                // Com a caixa da transformação aberta (⌘T ou
                                // "Transformar seleção"), o clique é dela.
                                ed.comecar_selecao_com_cliques(
                                    evento.position,
                                    evento.modifiers,
                                    evento.click_count,
                                    cx,
                                );
                            } else {
                                ed.apertar_com(evento.position, evento.modifiers, cx);
                            }
                        }),
                    )
                    // O ajuste rápido de tamanho e dureza do Photoshop: ⌥ +
                    // botão direito arrastando (Windows e Linux) e ⌃⌥ +
                    // arrasto no Mac — 🚨 o GPUI do Mac **entrega o ⌃ +
                    // clique como botão direito, sem o ⌃**, então os dois
                    // chegam aqui como direito com ⌥.
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|ed, evento: &MouseDownEvent, window, cx| {
                            if evento.modifiers.alt && ed.pinta() {
                                window.focus(&ed.foco, cx);
                                ed.comecar_ajuste_rapido(evento.position, cx);
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
                    // 🚨 O soltar também no próprio palco: o ouvinte da janela
                    // só existe depois do quadro seguinte ao apertar, e um
                    // clique mais rápido que um quadro (visto no app real, o
                    // ⇧ + clique) perdia o soltar e deixava o traço aberto até
                    // o gesto seguinte. `soltar` repetido não faz nada.
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|ed, _: &MouseUpEvent, _w, cx| ed.soltar(cx)),
                    )
                    .on_mouse_up(
                        MouseButton::Right,
                        cx.listener(|ed, _: &MouseUpEvent, _w, cx| ed.soltar(cx)),
                    )
                    // 🚨 Com um gesto em curso, quem trata o movimento é o
                    // ouvinte da janela (com os modificadores). Tratar aqui
                    // também, sem eles, desfazia o ⇧ do giro de 15° em 15°.
                    .on_mouse_move(cx.listener(|ed, evento: &MouseMoveEvent, _w, cx| {
                        if !ed.em_gesto() && ed.mao.is_none() {
                            ed.mover(evento.position, cx);
                        }
                    }));
            }
        }
        // O botão direito no palco: o menu de contexto do Photoshop — o da
        // transformação com a caixa ou a malha abertas, o da seleção com uma
        // seleção. Com ⌥ é o ajuste rápido do pincel, e nada abre.
        let ed = cx.entity();
        palco
            .context_menu(move |menu, window, cx| {
                let menu = match window.focused(cx) {
                    Some(antes) => menu.action_context(antes),
                    None => menu,
                };
                if window.modifiers().alt {
                    return menu;
                }
                let (transformando, deformando, contorno, tem_selecao, intocada) = {
                    let e = ed.read(cx);
                    let s = e.sessao();
                    (
                        e.transformando(),
                        e.deformando(),
                        s.is_some_and(Sessao::transformando_a_selecao),
                        s.and_then(Sessao::selecao).is_some(),
                        s.is_some_and(Sessao::malha_intocada),
                    )
                };
                let item = |id: &'static str,
                            rotulo: &'static str,
                            ligado: bool,
                            fazer: fn(
                    &mut EditorDeFoto,
                    &mut Window,
                    &mut Context<EditorDeFoto>,
                )| {
                    let ed = ed.clone();
                    crate::estilo::item_de_menu(id, ferramentas::na_plataforma(rotulo), None)
                        .disabled(!ligado)
                        .on_click(move |_ev, window, cx| {
                            ed.update(cx, |ed, cx| fazer(ed, window, cx));
                        })
                };
                if transformando {
                    return menu
                        .item(item(
                            "editor-contexto-transformacao-livre",
                            "Transformação livre",
                            deformando && intocada,
                            |ed, _, cx| ed.voltar_a_transformacao_livre(cx),
                        ))
                        .item(item(
                            "editor-contexto-deformar",
                            "Deformar",
                            !deformando && !contorno,
                            |ed, _, cx| ed.deformar(cx),
                        ))
                        .item(item(
                            "editor-contexto-redefinir",
                            "Redefinir a malha",
                            deformando && !intocada,
                            |ed, _, cx| ed.redefinir_malha(cx),
                        ))
                        .separator()
                        .item(item(
                            "editor-contexto-aplicar",
                            "Aplicar  ↵",
                            true,
                            |ed, _, cx| ed.aplicar_transformacao(cx),
                        ))
                        .item(item(
                            "editor-contexto-cancelar",
                            "Cancelar  Esc",
                            true,
                            |ed, _, cx| ed.cancelar_transformacao(cx),
                        ));
                }
                if !tem_selecao {
                    return menu;
                }
                menu.item(item(
                    "editor-contexto-difundir",
                    "Difusão…  ⇧F6",
                    true,
                    |ed, w, cx| ed.abrir_modificacao(Modificacao::Difundir, w, cx),
                ))
                .item(item(
                    "editor-contexto-expandir",
                    "Expandir…",
                    true,
                    |ed, w, cx| ed.abrir_modificacao(Modificacao::Expandir, w, cx),
                ))
                .item(item(
                    "editor-contexto-contrair",
                    "Contrair…",
                    true,
                    |ed, w, cx| ed.abrir_modificacao(Modificacao::Contrair, w, cx),
                ))
                .separator()
                .item(item(
                    "editor-contexto-inverter",
                    "Inverter seleção  ⇧⌘I",
                    true,
                    |ed, _, cx| ed.inverter_selecao(cx),
                ))
                .item(item(
                    "editor-contexto-desmarcar",
                    "Desmarcar  ⌘D",
                    true,
                    |ed, _, cx| ed.desmarcar(cx),
                ))
                .separator()
                // O Content-Aware do Photoshop, à mão com a seleção feita.
                .item(item(
                    "editor-contexto-preenchimento",
                    "Preenchimento sensível ao conteúdo…",
                    true,
                    |ed, _, cx| ed.abrir_preenchimento(cx),
                ))
                .item(item(
                    "editor-contexto-preencher-conteudo",
                    "Preencher…  ⇧⌫",
                    true,
                    |ed, _, cx| ed.abrir_dialogo_de_preencher(cx),
                ))
                .separator()
                .item(item(
                    "editor-contexto-via-copia",
                    "Camada via cópia  ⌘J",
                    true,
                    |ed, _, cx| ed.duplicar_camada(cx),
                ))
                .item(item(
                    "editor-contexto-via-recorte",
                    "Camada via recorte  ⇧⌘J",
                    true,
                    |ed, _, cx| ed.camada_via_recorte(cx),
                ))
                .separator()
                .item(item(
                    "editor-contexto-transformar-selecao",
                    "Transformar seleção",
                    true,
                    |ed, _, cx| ed.transformar_selecao(cx),
                ))
                .item(item(
                    "editor-contexto-deformar-selecao",
                    "Deformar",
                    true,
                    |ed, _, cx| ed.deformar(cx),
                ))
            })
            .into_any_element()
    }

    /// O diálogo do "Modificar seleção": o valor em pixels do documento.
    fn dialogo_da_modificacao(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        use gpui_kit::component::input::NumberInput;
        let (modificacao, campo) = self.modificando.clone()?;
        let tema = cx.theme().clone();
        Some(
            gpui_kit::component::v_flex()
                .gap(px(16.))
                .debug_selector(|| "editor-dialogo-modificar".into())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(crate::estilo::cabecalho_do_dialogo(
                    modificacao.titulo(),
                    "Vale para a seleção que já existe, num passo do desfazer — a difusão da barra de opções é a da próxima seleção.",
                    None,
                    cx,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_sm()
                                .text_color(tema.muted_foreground)
                                .child(modificacao.rotulo()),
                        )
                        .child(
                            div()
                                .w(px(120.))
                                .debug_selector(|| "editor-valor-da-modificacao".into())
                                .child(crate::estilo::campo(
                                    NumberInput::new(&campo)
                                        .suffix(div().text_sm().child("px")),
                                )),
                        ),
                )
                .child(
                    crate::estilo::rodape_do_dialogo()
                        .child(
                            crate::estilo::botao_contorno("editor-cancelar-modificacao", cx)
                                .debug_selector(|| "editor-cancelar-modificacao".into())
                                .child("Cancelar")
                                .on_click(cx.listener(|ed, _, window, cx| {
                                    ed.cancelar_modificacao(window, cx)
                                })),
                        )
                        .child(
                            crate::estilo::botao_primario("editor-confirmar-modificacao", cx)
                                .debug_selector(|| "editor-confirmar-modificacao".into())
                                .child("OK")
                                .on_click(cx.listener(|ed, _, window, cx| {
                                    ed.confirmar_modificacao(window, cx)
                                })),
                        ),
                )
                .into_any_element(),
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
/// Sobe os ladrilhos sujos de uma vista e devolve onde eles estão na foto
/// (o palco girado refaz o que cai em cima).
fn subir(
    vista: &mut VistaDoEditor,
    imagens: &mut HashMap<(u32, u32), Arc<RenderImage>>,
) -> Vec<Retangulo> {
    let sujos = vista.levar_os_sujos();
    let (regiao, fator) = (vista.regiao(), vista.fator());
    let mut na_foto = Vec::with_capacity(sujos.len());
    for &ladrilho in &sujos {
        let (l, a, bytes) = vista.ladrilho_bgra_com_folga(ladrilho);
        if let Some(imagem) = crate::imagem::de_bgra(l, a, bytes) {
            imagens.insert(ladrilho, imagem);
        }
        let r = vista.retangulo_do_ladrilho(ladrilho);
        na_foto.push(Retangulo::novo(
            regiao.x + r.x * fator,
            regiao.y + r.y * fator,
            r.largura * fator,
            r.altura * fator,
        ));
    }
    na_foto
}

/// A operação da seleção pelos modificadores: ⇧ soma, ⌥ tira.
/// O número de uma tecla da fila dos números — o próprio, ou o símbolo que ⇧
/// dá nela no teclado americano e no ABNT2 (`!@#$%^¨&*()`).
/// As propriedades da máscara que não estão no padrão — o estado do roteiro.
fn extras_da_mascara(m: &editor_core::Mascara) -> String {
    let mut extras = String::new();
    if !m.ativa {
        extras.push_str(",desligada");
    }
    if !m.vinculada {
        extras.push_str(",solta");
    }
    if m.densidade < 1.0 {
        extras.push_str(&format!(",densidade={:.0}%", m.densidade * 100.0));
    }
    if m.difusao > 0.0 {
        extras.push_str(&format!(",difusao={:.1}", m.difusao));
    }
    extras
}

fn digito_da_tecla(tecla: &str) -> Option<u32> {
    let mut letras = tecla.chars();
    let c = letras.next()?;
    if letras.next().is_some() {
        return None;
    }
    if let Some(d) = c.to_digit(10) {
        return Some(d);
    }
    "!@#$%^&*()"
        .find(c)
        .map(|i| ((i + 1) % 10) as u32)
        .or((c == '¨').then_some(6))
}

/// A operação de seleção dos modificadores — a mesma no palco, na varinha e
/// na miniatura (⌘ + clique): ⇧ soma, ⌥ tira, ⇧⌥ cruza.
fn operacao_dos(m: gpui_kit::Modifiers) -> Operacao {
    Operacao::dos_modificadores(m.shift, m.alt)
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
        Ajuste::Inverter | Ajuste::Curvas { .. } => Vec::new(),
    }
}

/// A curva do canal `canal` (0 = RGB, 1 vermelho, 2 verde, 3 azul).
fn curva_do_canal(ajuste: &Ajuste, canal: usize) -> Option<editor_core::ajuste::Curva> {
    match *ajuste {
        Ajuste::Curvas {
            rgb,
            vermelho,
            verde,
            azul,
        } => Some([rgb, vermelho, verde, azul][canal.min(3)]),
        _ => None,
    }
}

/// As Curvas com a curva do canal `canal` trocada por `c`.
fn com_curva(ajuste: Ajuste, canal: usize, c: editor_core::ajuste::Curva) -> Ajuste {
    match ajuste {
        Ajuste::Curvas {
            mut rgb,
            mut vermelho,
            mut verde,
            mut azul,
        } => {
            match canal {
                1 => vermelho = c,
                2 => verde = c,
                3 => azul = c,
                _ => rgb = c,
            }
            Ajuste::Curvas {
                rgb,
                vermelho,
                verde,
                azul,
            }
        }
        outro => outro,
    }
}

/// O arrasto de um ponto da curva: o canal, o índice, e a curva do começo
/// (o arrasto parte sempre dela — ao voltar para dentro do gráfico, o ponto
/// tirado volta).
#[derive(Clone, Copy)]
struct ArrastoDaCurva {
    canal: usize,
    indice: usize,
    curva: editor_core::ajuste::Curva,
}

/// Os nomes dos canais das Curvas.
const CANAIS_DA_CURVA: [&str; 4] = ["RGB", "Vermelho", "Verde", "Azul"];

/// O lado do gráfico das Curvas, em pontos.
const LADO_DA_CURVA: f32 = 176.0;

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
    /// O giro da vista (R), em torno do centro do palco.
    angulo: f32,
}

impl Tela {
    /// Um pixel da foto no palco, com o giro.
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        giro::girar(
            (self.v.x + x * self.v.escala, self.v.y + y * self.v.escala),
            (self.area.largura / 2.0, self.area.altura / 2.0),
            self.angulo,
        )
    }

    /// Os ladrilhos do palco girado (`giro.rs`), na grade do dispositivo.
    fn ladrilhos_girados(&self, palco: &giro::PalcoGirado) -> Vec<AnyElement> {
        let dpr = self.fator_da_tela;
        palco
            .ladrilhos
            .iter()
            .map(|(&(lx, ly), imagem)| {
                let tamanho = imagem.size(0);
                let (l, a) = (
                    i32::from(tamanho.width) as f32,
                    i32::from(tamanho.height) as f32,
                );
                img(imagem.clone())
                    .object_fit(ObjectFit::Fill)
                    .absolute()
                    .left(px((lx * giro::LADO) as f32 / dpr))
                    .top(px((ly * giro::LADO) as f32 / dpr))
                    .w(px(l / dpr))
                    .h(px(a / dpr))
                    .into_any_element()
            })
            .collect()
    }

    /// O letreiro com a vista girada: as bordas viram segmentos em qualquer
    /// direção, recortados ao palco sem entortar, e os traços brancos seguem
    /// cada segmento.
    fn letreiro_girado(&self, bordas: Arc<Vec<Segmento>>) -> AnyElement {
        let (v, area, angulo) = (self.v, self.area, self.angulo);
        canvas(
            |_, _, _| {},
            move |limites, _, window, _| {
                let tela = Tela {
                    v,
                    area,
                    fator_da_tela: 1.0,
                    angulo,
                };
                let (ox, oy) = (f(limites.origin.x), f(limites.origin.y));
                let caixa = (-2.0, -2.0, area.largura + 2.0, area.altura + 2.0);
                let mut preto = PathBuilder::stroke(px(1.5));
                let mut branco = PathBuilder::stroke(px(1.));
                let mut algum = false;
                for &(x0, y0, x1, y1) in bordas.iter() {
                    let a = tela.p(x0 as f32, y0 as f32);
                    let b = tela.p(x1 as f32, y1 as f32);
                    let Some((a, b)) = giro::recortar(a, b, caixa) else {
                        continue;
                    };
                    algum = true;
                    preto.move_to(gpui_kit::point(px(ox + a.0), px(oy + a.1)));
                    preto.line_to(gpui_kit::point(px(ox + b.0), px(oy + b.1)));
                    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                    let comprimento = dx.hypot(dy);
                    if comprimento <= 0.0 {
                        continue;
                    }
                    let (ux, uy) = (dx / comprimento, dy / comprimento);
                    let mut t = 0.0;
                    while t < comprimento {
                        let fim = (t + 4.0).min(comprimento);
                        branco.move_to(gpui_kit::point(
                            px(ox + a.0 + ux * t),
                            px(oy + a.1 + uy * t),
                        ));
                        branco.line_to(gpui_kit::point(
                            px(ox + a.0 + ux * fim),
                            px(oy + a.1 + uy * fim),
                        ));
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

    /// O letreiro: a borda da seleção em preto, com traços brancos de 4 pontos
    /// por cima — lê sobre qualquer foto.
    fn letreiro(&self, bordas: Arc<Vec<Segmento>>) -> AnyElement {
        if self.angulo != 0.0 {
            return self.letreiro_girado(bordas);
        }
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
        let na_tela: Vec<(f32, f32)> = pontos.iter().map(|(x, y)| self.p(*x, *y)).collect();
        let pontos = na_tela;
        canvas(
            |_, _, _| {},
            move |limites, _, window, _| {
                let (ox, oy) = (f(limites.origin.x), f(limites.origin.y));
                let ponto = |p: &(f32, f32)| gpui_kit::point(px(ox + p.0), px(oy + p.1));
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
        if !window.is_window_active() && self.espaco.is_some() {
            self.espaco = None;
            self.mao = None;
        }
        // O preenchimento modal desenha sem giro; a vista volta girada depois.
        if self.area_do_preenchimento.is_some() && self.giro != 0.0 {
            self.giro_antes_do_preenchimento = Some(self.giro);
            self.giro = 0.0;
        } else if self.area_do_preenchimento.is_none() {
            if let Some(g) = self.giro_antes_do_preenchimento.take() {
                self.giro = g;
            }
        }
        self.acompanhar_o_pincel(window, cx);
        self.sincronizar_os_campos(window, cx);
        self.sincronizar_a_transformacao(window, cx);
        self.atualizar_a_previa_do_carimbo();
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
        // As Propriedades da máscara acompanham a escolhida e o desfazer.
        if let Some((densidade, difusao)) = self
            .sessao()
            .and_then(|s| s.camada_ativa().mascara.as_ref())
            .map(|m| (m.densidade * 100.0, m.difusao))
        {
            for (estado, v, folga) in [
                (self.densidade_da_mascara.clone(), densidade, 0.5),
                (self.difusao_da_mascara.clone(), difusao, 0.25),
            ] {
                if (estado.read(cx).value().start() - v).abs() > folga {
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
        // A de fundo também (X, D, a máscara em cinza).
        let fundo = self.sessao().map(|s| s.pincel.cor_de_fundo);
        if fundo.is_some() && fundo != self.fundo_mostrado {
            self.fundo_mostrado = fundo;
            let hsla = hsla_de(fundo.unwrap_or([255; 3]));
            self.seletor_de_fundo
                .update(cx, |s, cx| s.set_value(hsla, window, cx));
        }
        // R G B do painel Cor: a cor que ele edita.
        if let Some(s) = self.sessao() {
            let cor = if self.cor_do_painel_e_o_fundo {
                s.pincel.cor_de_fundo
            } else {
                s.pincel.cor
            };
            for (i, estado) in self.rgb_do_painel.clone().iter().enumerate() {
                if (estado.read(cx).value().start() - cor[i] as f32).abs() > 0.5 {
                    estado.update(cx, |e, cx| e.set_value(cor[i] as f32, window, cx));
                }
            }
        }
        self.sincronizar_o_campo_do_zoom(window, cx);
        // O ângulo da Girar vista e a opacidade da camada, nos campos (menos
        // enquanto se digita neles).
        let graus = (self.giro.to_degrees() * 10.0).round() / 10.0;
        if self.giro_mostrado != Some(graus)
            && !self
                .campo_do_giro
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        {
            self.giro_mostrado = Some(graus);
            self.campo_do_giro
                .update(cx, |c, cx| c.set_value(format!("{graus}"), window, cx));
        }
        let opacidade = (self.opacidade_da_camada() * 100.0).round();
        if self.opacidade_mostrada != Some(opacidade)
            && !self
                .campo_da_opacidade_da_camada
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        {
            self.opacidade_mostrada = Some(opacidade);
            self.campo_da_opacidade_da_camada.update(cx, |c, cx| {
                c.set_value(format!("{opacidade:.0}"), window, cx)
            });
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
        let modificacao = {
            let quer = self.modificando.is_some();
            crate::dialogo::desenhar(
                self,
                quer,
                crate::dialogo::Jeito::dialogo(400.),
                Self::dialogo_da_modificacao,
                |ed, window, cx| ed.cancelar_modificacao(window, cx),
                window,
                cx,
            )
        };
        let atalhos = {
            let quer = self.mostrando_atalhos;
            crate::dialogo::desenhar(
                self,
                quer,
                crate::dialogo::Jeito::dialogo(620.),
                Self::dialogo_dos_atalhos,
                |ed, _, cx| {
                    ed.mostrando_atalhos = false;
                    cx.notify();
                },
                window,
                cx,
            )
        };
        let flyout = self.flyout_da_barra(cx);
        let preencher = {
            let quer = self.dialogo_de_preencher;
            crate::dialogo::desenhar(
                self,
                quer,
                crate::dialogo::Jeito::dialogo(420.),
                Self::dialogo_do_preencher,
                |ed, window, cx| ed.cancelar_preencher(window, cx),
                window,
                cx,
            )
        };
        let tema = cx.theme().clone();
        // A moldura do `Root` não pode tomar o clique do conteúdo encostado na
        // borda com a janela maximizada (`janela::raiz_do_conteudo`).
        // 🪄 O Preenchimento sensível ao conteúdo é um espaço **modal**, como
        // no Photoshop (dono, 07/out/2026: *"deveria abrir um modal
        // separado"*): a foto com o pincel à esquerda, a Visualização no
        // meio, os ajustes à direita — e nada do resto do editor ao alcance.
        let (barra, corpo) = if self.area_do_preenchimento.is_some() {
            let largura = self.largura_do_painel();
            let corpo = div()
                .flex()
                .flex_1()
                .min_h(px(0.))
                .child(self.ferramentas_do_preenchimento(cx))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(240.))
                        .h_full()
                        .child(self.palco(window, cx)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(240.))
                        .h_full()
                        .child(self.visualizacao_do_preenchimento(cx)),
                )
                .child(
                    div()
                        .w(px(largura))
                        .flex_shrink_0()
                        .h_full()
                        .child(self.painel_do_preenchimento(cx)),
                )
                .into_any_element();
            (self.barra_do_preenchimento(window, cx), corpo)
        } else {
            // 🧭 A área de trabalho do Photoshop: menus; opções da ferramenta
            // (altura fixa); barra de ferramentas à esquerda; a aba do
            // documento e o palco no meio; as docas à direita; o status
            // embaixo. Tab esconde barra, opções e docas; ⇧Tab só as docas.
            self.garantir_a_area_de_trabalho(window, cx);
            let ferramentas_a_vista = !self.interface_oculta;
            let paineis_a_vista = !self.interface_oculta && !self.paineis_ocultos;
            let documento = div()
                .flex()
                .flex_col()
                .size_full()
                .min_w(px(0.))
                .child(self.aba_do_documento(cx))
                .child({
                    let palco = self.palco(window, cx);
                    div()
                        .flex()
                        .flex_1()
                        .min_h(px(0.))
                        .child(self.com_reguas(palco, cx))
                })
                .into_any_element();
            let meio: AnyElement = if !paineis_a_vista {
                documento
            } else if self.arranjo.recolhido {
                div()
                    .flex()
                    .size_full()
                    .child(div().flex_1().min_w(px(0.)).h_full().child(documento))
                    .child(self.coluna_de_paineis(cx))
                    .into_any_element()
            } else {
                use gpui_kit::component::resizable::{h_resizable, resizable_panel};
                let largura = self.largura_do_painel();
                let limites = area_de_trabalho::LIMITES_DA_COLUNA;
                h_resizable("editor-colunas")
                    .with_state(&self.colunas)
                    .child(
                        resizable_panel()
                            .size_range(px(320.)..gpui_kit::Pixels::MAX)
                            .child(documento),
                    )
                    .child(
                        // A largura como `flex_basis` (o padrão da tela do caixa):
                        // não cresce com a janela, encolhe até o mínimo.
                        resizable_panel()
                            .size_range(px(limites.minimo)..px(limites.maximo))
                            .flex_basis(px(largura))
                            .flex_grow_0()
                            .flex_shrink(1.)
                            .child(self.coluna_de_paineis(cx)),
                    )
                    .into_any_element()
            };
            let corpo = div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .when(ferramentas_a_vista, |d| d.child(self.barra_de_opcoes(cx)))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h(px(0.))
                        .when(ferramentas_a_vista, |d| {
                            d.child(self.barra_de_ferramentas(cx))
                        })
                        .child(div().flex_1().min_w(px(0.)).h_full().child(meio)),
                )
                .child(self.barra_de_status(cx))
                .into_any_element();
            (self.linha_de_menus(window, cx), corpo)
        };
        crate::janela::raiz_do_conteudo(div())
            .id("editor-de-foto")
            .key_context(if self.filtro.aberto.is_some() {
                super::CONTEXTO_DO_FILTRO
            } else {
                CONTEXTO
            })
            .track_focus(&self.foco)
            .size_full()
            .flex()
            .flex_col()
            .bg(tema.background)
            .text_color(tema.foreground)
            .on_key_up(
                cx.listener(|ed, evento: &KeyUpEvent, _w, cx| ed.ao_soltar_tecla(evento, cx)),
            )
            .on_key_down(cx.listener(|ed, evento: &KeyDownEvent, window, cx| {
                ed.ao_apertar_tecla(evento, window, cx)
            }))
            .on_action(cx.listener(|ed, _: &DesfazerNoEditor, _, cx| ed.desfazer(cx)))
            .on_action(cx.listener(|ed, _: &RefazerNoEditor, _, cx| ed.refazer(cx)))
            .on_action(
                cx.listener(|ed, _: &SalvarNoEditor, window, cx| ed.salvar(false, window, cx)),
            )
            .on_action(cx.listener(|ed, _: &FecharEditor, window, cx| ed.fechar(window, cx)))
            .on_action(cx.listener(|ed, _: &PincelMenor, window, cx| {
                ed.mudar_tamanho(1.0 / 1.25, window, cx)
            }))
            .on_action(
                cx.listener(|ed, _: &PincelMaior, window, cx| ed.mudar_tamanho(1.25, window, cx)),
            )
            .on_action(cx.listener(|ed, _: &DurezaMenor, _, cx| ed.mudar_dureza(-0.25, cx)))
            .on_action(cx.listener(|ed, _: &DurezaMaior, _, cx| ed.mudar_dureza(0.25, cx)))
            .on_action(cx.listener(|ed, _: &GrupoV, _, cx| ed.pela_letra('v', false, cx)))
            .on_action(cx.listener(|ed, _: &GrupoM, _, cx| ed.pela_letra('m', false, cx)))
            .on_action(cx.listener(|ed, _: &ProximaDoGrupoM, _, cx| ed.pela_letra('m', true, cx)))
            .on_action(cx.listener(|ed, _: &GrupoL, _, cx| ed.pela_letra('l', false, cx)))
            .on_action(cx.listener(|ed, _: &GrupoW, _, cx| ed.pela_letra('w', false, cx)))
            .on_action(cx.listener(|ed, _: &GrupoI, _, cx| ed.pela_letra('i', false, cx)))
            .on_action(cx.listener(|ed, _: &GrupoJ, _, cx| ed.pela_letra('j', false, cx)))
            .on_action(cx.listener(|ed, _: &ProximaDoGrupoJ, _, cx| ed.pela_letra('j', true, cx)))
            .on_action(
                cx.listener(|ed, _: &AlternarAntesDepois, _, cx| ed.alternar_antes_depois(cx)),
            )
            .on_action(cx.listener(|ed, _: &Liquidificar, _, cx| ed.liquidificar(cx)))
            .on_action(cx.listener(|ed, _: &AlternarMascaraDeCorte, _, cx| {
                if let Some(i) = ed.sessao().map(Sessao::ativa) {
                    ed.alternar_mascara_de_corte(i, cx);
                }
            }))
            .on_action(cx.listener(|ed, _: &GrupoB, _, cx| ed.pela_letra('b', false, cx)))
            .on_action(cx.listener(|ed, _: &GrupoS, _, cx| ed.pela_letra('s', false, cx)))
            .on_action(cx.listener(|ed, _: &GrupoE, _, cx| ed.pela_letra('e', false, cx)))
            .on_action(cx.listener(|ed, _: &GrupoG, _, cx| ed.pela_letra('g', false, cx)))
            .on_action(cx.listener(|ed, _: &ProximaDoGrupoG, _, cx| ed.pela_letra('g', true, cx)))
            .on_action(cx.listener(|ed, _: &GrupoO, _, cx| ed.pela_letra('o', false, cx)))
            .on_action(cx.listener(|ed, _: &ProximaDoGrupoO, _, cx| ed.pela_letra('o', true, cx)))
            .on_action(cx.listener(|ed, _: &GrupoH, _, cx| ed.pela_letra('h', false, cx)))
            .on_action(cx.listener(|ed, _: &GrupoR, _, cx| ed.pela_letra('r', false, cx)))
            .on_action(cx.listener(|ed, _: &AlternarCamada, _, cx| ed.alternar_visibilidade(cx)))
            .on_action(cx.listener(|ed, _: &NovaCamada, _, cx| ed.nova_camada(cx)))
            .on_action(cx.listener(|ed, _: &DuplicarCamada, _, cx| ed.duplicar_camada(cx)))
            .on_action(cx.listener(|ed, _: &CamadaViaRecorte, _, cx| ed.camada_via_recorte(cx)))
            .on_action(cx.listener(|ed, _: &TransformacaoLivre, _, cx| ed.transformar(cx)))
            .on_action(
                cx.listener(|ed, _: &PreencherPeloConteudo, _, cx| {
                    ed.abrir_dialogo_de_preencher(cx)
                }),
            )
            .on_action(cx.listener(|ed, _: &AplicarTransformacao, window, cx| {
                if ed.filtro.aberto.is_some() {
                    ed.confirmar_filtro(window, cx)
                } else if ed.tecla_no_flyout("enter", cx) {
                } else if ed.dialogo_de_preencher {
                    ed.confirmar_preencher(window, cx)
                } else if ed.area_do_preenchimento.is_some() {
                    ed.confirmar_preenchimento(cx)
                } else if ed.modificando.is_some() {
                    ed.confirmar_modificacao(window, cx)
                } else if ed.poligono_aberto() {
                    ed.concluir_poligono(cx)
                } else {
                    ed.aplicar_transformacao(cx)
                }
            }))
            .on_action(cx.listener(|ed, _: &CancelarTransformacao, window, cx| {
                if ed.filtro.aberto.is_some() {
                    ed.cancelar_o_filtro(window, cx)
                } else if ed.tecla_no_flyout("escape", cx) {
                } else if ed.dialogo_de_preencher {
                    ed.cancelar_preencher(window, cx)
                } else if ed.area_do_preenchimento.is_some() {
                    ed.cancelar_preenchimento(cx)
                } else if ed.modificando.is_some() {
                    ed.cancelar_modificacao(window, cx)
                } else if ed.cancelar_gesto_de_selecao(cx) {
                    // O gesto de seleção some; a seleção de antes fica.
                } else if ed.transformando() || ed.liquidificando() {
                    ed.cancelar_transformacao(cx)
                } else if ed.auxiliar == Some(Auxiliar::GirarVista) {
                    // Esc com a Girar vista na mão: a tela volta a 0°.
                    ed.girar_a_vista(0.0, cx)
                } else {
                    ed.cancelar_transformacao(cx)
                }
            }))
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
            .on_action(cx.listener(|ed, _: &GrupoZ, _, cx| ed.pela_letra('z', false, cx)))
            .on_action(cx.listener(|ed, _: &AlternarInterface, _, cx| ed.alternar_interface(cx)))
            .on_action(cx.listener(|ed, _: &AlternarPaineis, _, cx| ed.alternar_paineis(cx)))
            .on_action(cx.listener(|ed, _: &SegurarAMao, _, cx| ed.espaco_apertado(cx)))
            .on_action(
                cx.listener(|ed, _: &DifundirSelecao, window, cx| ed.difundir_selecao(window, cx)),
            )
            .on_action(cx.listener(|ed, _: &ProximaDoGrupoL, _, cx| ed.pela_letra('l', true, cx)))
            .on_action(cx.listener(|ed, _: &TrocarCores, _, cx| ed.trocar_cores(cx)))
            .on_action(cx.listener(|ed, _: &CoresPadrao, _, cx| ed.cores_padrao(cx)))
            .on_action(cx.listener(|ed, _: &SelecionarTudo, _, cx| ed.selecionar_tudo(cx)))
            .on_action(cx.listener(|ed, _: &Desmarcar, _, cx| ed.desmarcar(cx)))
            .on_action(cx.listener(|ed, _: &Reselecionar, _, cx| ed.reselecionar(cx)))
            .on_action(
                cx.listener(|ed, _: &AjusteNiveis, _, cx| ed.ajuste_pela_chave("niveis", cx)),
            )
            .on_action(
                cx.listener(|ed, _: &AjusteCurvas, _, cx| ed.ajuste_pela_chave("curvas", cx)),
            )
            .on_action(cx.listener(|ed, _: &AjusteMatiz, _, cx| ed.ajuste_pela_chave("matiz", cx)))
            .on_action(cx.listener(|ed, _: &AlternarReguas, _, cx| ed.alternar_reguas(cx)))
            .on_action(cx.listener(|ed, _: &InverterSelecao, _, cx| ed.inverter_selecao(cx)))
            .on_action(cx.listener(|ed, _: &ApagarSelecao, _, cx| {
                // ⌫ com o laço poligonal aberto tira o último vértice.
                if !ed.tirar_o_ultimo_vertice(cx) {
                    ed.apagar_selecao(cx)
                }
            }))
            .on_action(cx.listener(|ed, _: &PreencherSelecao, _, cx| ed.preencher_selecao(cx)))
            .on_action(cx.listener(|ed, _: &MesclarParaBaixo, _, cx| ed.mesclar_para_baixo(cx)))
            .on_action(cx.listener(|ed, _: &Copiar, _, cx| ed.copiar(false, cx)))
            .on_action(cx.listener(|ed, _: &CopiarMesclado, _, cx| ed.copiar(true, cx)))
            .on_action(cx.listener(|ed, _: &Recortar, _, cx| ed.recortar(cx)))
            .on_action(cx.listener(|ed, _: &Colar, _, cx| ed.colar(false, cx)))
            .on_action(cx.listener(|ed, _: &ColarNoLugar, _, cx| ed.colar(true, cx)))
            .on_action(cx.listener(|ed, _: &CarimbarVisivel, _, cx| ed.carimbar_visivel(cx)))
            .on_action(cx.listener(|ed, _: &Inverter, _, cx| ed.inverter(cx)))
            .on_action(cx.listener(|ed, _: &AlternarRubi, _, cx| ed.alternar_rubi(cx)))
            .on_action(
                cx.listener(|ed, _: &BloquearTransparencia, _, cx| ed.bloquear_transparencia(cx)),
            )
            .child(barra)
            .child(corpo)
            .children(flyout)
            .children(pergunta)
            .children(modificacao)
            .children(atalhos)
            .children(preencher)
            .children(self.caixa_do_filtro(window, cx))
    }
}

#[cfg(test)]
mod testes_das_teclas {
    use super::digito_da_tecla;

    #[test]
    fn o_numero_vem_do_digito_ou_do_simbolo_do_shift() {
        assert_eq!(digito_da_tecla("4"), Some(4));
        assert_eq!(digito_da_tecla("#"), Some(3), "⇧3 no teclado real");
        assert_eq!(digito_da_tecla(")"), Some(0));
        assert_eq!(digito_da_tecla("!"), Some(1));
        assert_eq!(digito_da_tecla("¨"), Some(6), "⇧6 no ABNT2");
        assert_eq!(digito_da_tecla("a"), None);
        assert_eq!(digito_da_tecla("f4"), None);
    }
}
