//! A **Revelação local** na tela: as máscaras (pincel, gradientes, laço) com a
//! exposição delas, e o retoque (carimbo, band-aid, Content-Aware).
//!
//! O motor, a revelação e a matemática moram no `revelacao-core` (`locais.rs`,
//! `mascaras.rs`, `retoque.rs`, `preenchimento.rs`) e são os mesmos do site. O
//! que mora aqui é o que só a tela tem: a ferramenta escolhida, o gesto em
//! curso, a conta do ponteiro à foto, as marcações por cima da foto e o painel.
//!
//! # A revelação é a fonte de verdade
//!
//! [`Revelacao::locais`] é a revelação da foto aberta, e é ela que o histórico,
//! a gravação e a GPU recebem. O gesto em curso é **provisório**: a GPU o
//! revela a cada movimento (`locais_na_tela`), mas ele só entra na revelação no
//! fim do gesto — um `⌘Z` por pincelada, como os sliders.
//!
//! # Coordenadas
//!
//! O ponteiro chega em pontos da área do palco; a revelação quer a foto inteira
//! de pé, antes do enquadramento. O caminho é a vista do zoom (área → quadro
//! exibido) e o `do_quadro_para_a_foto` do motor (quadro → foto), a mesma conta
//! que o `aplicar` usa para enquadrar — o teste dela está no `revelacao-core`.

use std::sync::Arc;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Selectable, Sizable};
use gpui_kit::{
    canvas, div, point, prelude::*, px, AnyElement, Context, Entity, Hsla, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels, Point, SharedString, Subscription, Window,
};
use revelacao_core::locais::{
    self, AjustesLocais, BrushStroke, Camada, Carimbo, Componente, Forma, GradienteLinear,
    GradienteRadial, Laco, Modo, ParametrosLocais, Preenchimento, Retoque,
};
use revelacao_core::transformacao;

use super::Revelacao;
use crate::recursos::Icone;
use crate::revelacao::zoom::Ponto;

/// As ferramentas, na ordem da barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ferramenta {
    Pincel,
    Linear,
    Radial,
    Laco,
    Carimbo,
    BandAid,
    Preencher,
}

impl Ferramenta {
    /// O gesto desta ferramenta, para a ferramenta de desempenho.
    pub fn operacao_de_desempenho(self) -> crate::desempenho::Operacao {
        use crate::desempenho::Operacao;
        match self {
            Ferramenta::Pincel => Operacao::Pincel,
            Ferramenta::Linear | Ferramenta::Radial => Operacao::Gradiente,
            Ferramenta::Laco => Operacao::Laco,
            Ferramenta::Carimbo => Operacao::Clone,
            Ferramenta::BandAid => Operacao::Heal,
            Ferramenta::Preencher => Operacao::Preencher,
        }
    }

    pub const MASCARA: [Ferramenta; 4] = [
        Ferramenta::Pincel,
        Ferramenta::Linear,
        Ferramenta::Radial,
        Ferramenta::Laco,
    ];
    pub const RETOQUE: [Ferramenta; 3] = [
        Ferramenta::Carimbo,
        Ferramenta::BandAid,
        Ferramenta::Preencher,
    ];

    pub fn de_mascara(self) -> bool {
        Self::MASCARA.contains(&self)
    }

    pub fn nome(self) -> &'static str {
        match self {
            Ferramenta::Pincel => "Pincel",
            Ferramenta::Linear => "Gradiente linear",
            Ferramenta::Radial => "Gradiente radial",
            Ferramenta::Laco => "Laço",
            Ferramenta::Carimbo => "Carimbo",
            Ferramenta::BandAid => "Band-aid",
            Ferramenta::Preencher => "Content-Aware",
        }
    }

    fn atalho(self) -> &'static str {
        match self {
            Ferramenta::Pincel => "K",
            Ferramenta::Linear => "M",
            Ferramenta::Radial => "⇧M",
            Ferramenta::Laco => "L",
            Ferramenta::Carimbo => "S",
            Ferramenta::BandAid => "J",
            Ferramenta::Preencher => "⇧J",
        }
    }

    fn icone(self) -> Icone {
        match self {
            Ferramenta::Pincel => Icone::Paintbrush,
            Ferramenta::Linear => Icone::RectangleHorizontal,
            Ferramenta::Radial => Icone::CircleDashed,
            Ferramenta::Laco => Icone::Lasso,
            Ferramenta::Carimbo => Icone::Stamp,
            Ferramenta::BandAid => Icone::Bandage,
            Ferramenta::Preencher => Icone::Sparkles,
        }
    }

    fn usa_laco(self) -> bool {
        matches!(self, Ferramenta::Laco)
    }

    fn circular(self) -> bool {
        matches!(
            self,
            Ferramenta::Pincel | Ferramenta::Carimbo | Ferramenta::BandAid | Ferramenta::Preencher
        )
    }
}

/// O que se arrasta num componente de máscara que já existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlcaDeMascara {
    /// As pontas do gradiente linear: onde começa (100%) e onde acaba (0%).
    Inicio,
    Fim,
    /// O centro do radial, ou o meio do linear e do laço: move o todo.
    Mover,
    /// A borda do radial nos dois eixos.
    RaioX,
    RaioY,
}

/// O que se arrasta num retoque que já existe.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Parte {
    /// O círculo tracejado: muda a amostragem.
    Origem,
    /// O destino inteiro: move o remendo.
    Destino,
    /// A borda do círculo: muda o raio.
    Raio,
    /// O canto da caixa do laço: escala o laço.
    Escala,
}

/// O gesto em curso.
#[derive(Debug, Clone)]
enum Gesto {
    /// Um componente de máscara sendo desenhado — `nova`: numa máscara nova.
    Componente { nova: bool, componente: Componente },
    /// Um retoque sendo desenhado.
    Retoque(Retoque),
    /// Um componente de máscara que já existe, pela alça (linear, radial,
    /// laço) — ao vivo na GPU, um passo ao soltar.
    Mascara {
        camada: usize,
        componente: usize,
        alca: AlcaDeMascara,
        de: [f32; 2],
        original: Componente,
        valor: Componente,
    },
    /// Um retoque existente sendo mudado.
    Editando {
        indice: usize,
        parte: Parte,
        de: [f32; 2],
        original: Retoque,
        valor: Retoque,
    },
}

/// O estado da Revelação local na tela.
pub(in crate::revelacao) struct Local {
    pub ferramenta: Option<Ferramenta>,
    /// A máscara que recebe os componentes novos — `None` com `criando`.
    pub mascara_sel: Option<usize>,
    /// O próximo gesto de máscara cria uma máscara nova.
    pub criando: bool,
    pub subtrair: bool,
    pub poligonal: bool,
    /// O Content-Aware cercando (laço) em vez de pintando.
    pub preencher_por_laco: bool,
    /// Alfinetes, contornos e setas (`H`).
    pub marcacoes: bool,
    gesto: Option<Gesto>,
    /// O laço poligonal em curso, vértice a vértice.
    poligono: Vec<[f32; 2]>,
    /// A origem escolhida com ⌥ + clique (carimbo e band-aid).
    origem: Option<[f32; 2]>,
    /// O retoque selecionado — mostra as alças e é o que os sliders mudam.
    pub selecionado: Option<usize>,
    /// Onde o ponteiro está, em pontos da área.
    cursor: Option<Ponto>,
    /// Os valores da ferramenta (os sliders escrevem aqui).
    /// O raio da ferramenta **em pontos da tela** — ver [`TAMANHO_INICIAL`].
    pub tamanho: f32,
    pub feather: f32,
    pub opacidade: f32,
    pub sliders: SlidersLocais,
}

pub(in crate::revelacao) struct SlidersLocais {
    pub raio: Entity<SliderState>,
    pub feather: Entity<SliderState>,
    pub opacidade: Entity<SliderState>,
    pub exposicao: Entity<SliderState>,
}

/// O raio da ferramenta antes de haver foto para medir a tela.
const RAIO_INICIAL: f32 = 0.03;
/// 🔎 **O Tamanho da ferramenta é na tela** (dono, 27/set/2026: *"quando eu
/// der um zoom ao ponto de querer remover uma sarda, o círculo vai ser
/// inteligente o suficiente para acompanhar?"*). É o do Lightroom: o raio do
/// cursor, em pontos, fica o mesmo em qualquer zoom — então aproximar já afina
/// a ferramenta, e o círculo que se vê antes do clique é o que se recebe.
/// Antes o Tamanho era fração da foto: a 400% o carimbo de 3% cobria meio
/// rosto, e era preciso descer o slider a cada zoom.
///
/// ⚠️ Só a **ferramenta** é na tela. O retoque feito guarda o raio em fração
/// da foto (`locais.rs`), como sempre: um carimbo dado a 400% continua do
/// tamanho da sarda quando a foto volta a caber.
const TAMANHO_INICIAL: f32 = 30.0;
const TAMANHO_MINIMO: f32 = 2.0;
const TAMANHO_MAXIMO: f32 = 400.0;
/// O que o raio vira na foto, depois da conta com o zoom: o piso (~2 px numa
/// foto de 6000 px) e o teto de sempre.
const RAIO_MINIMO_DA_FERRAMENTA: f32 = 0.0003;
const RAIO_MAXIMO_DA_FERRAMENTA: f32 = 0.15;
/// O teto do retoque já feito, que as alças esticam além do slider.
const RAIO_MAXIMO_DO_RETOQUE: f32 = 0.25;
const FEATHER_INICIAL: f32 = 0.5;
/// O feather do laço é fração do maior lado (até 4%); o do pincel, do raio.
const FEATHER_DO_LACO: f32 = 0.04;
/// Até onde o clique ainda pega uma alça, em pontos.
const ALCANCE_DA_ALCA: f32 = 8.0;
/// Abaixo deste raio na tela, em pontos, o retoque circular fica sem alças.
const RAIO_MINIMO_COM_ALCAS: f32 = 2.0 * ALCANCE_DA_ALCA;
/// A chave de lembrança da sanfona da Revelação local (`revelacao:<título>`).
const CHAVE_DO_PAINEL_LOCAL: &str = "revelacao:Revelação local";

/// O slider do Tamanho anda em escala logarítmica (`0..1`): cada passo
/// multiplica o tamanho pela mesma razão, e o começo do trilho vira precisão
/// para retoque pequeno em vez de sumir num canto.
fn tamanho_da_posicao(t: f32) -> f32 {
    TAMANHO_MINIMO * (TAMANHO_MAXIMO / TAMANHO_MINIMO).powf(t.clamp(0.0, 1.0))
}

fn posicao_do_tamanho(tamanho: f32) -> f32 {
    let t = tamanho.clamp(TAMANHO_MINIMO, TAMANHO_MAXIMO);
    (t / TAMANHO_MINIMO).ln() / (TAMANHO_MAXIMO / TAMANHO_MINIMO).ln()
}

/// O Tamanho no painel: quantos pixels **da foto** o círculo cobre agora (o
/// diâmetro). É o número que muda com o zoom — e é ele que diz se o carimbo
/// cabe na sarda.
fn rotulo_do_tamanho(diametro_em_pixels: f32) -> String {
    if diametro_em_pixels < 10.0 {
        format!("{diametro_em_pixels:.1} px")
    } else {
        format!("{diametro_em_pixels:.0} px")
    }
}

impl Local {
    /// Cria os sliders e liga cada um à tela.
    pub(in crate::revelacao) fn novo(
        window: &mut Window,
        cx: &mut Context<Revelacao>,
        assinaturas: &mut Vec<Subscription>,
    ) -> Self {
        let slider = |min: f32, max: f32, passo: f32, valor: f32, cx: &mut Context<Revelacao>| {
            cx.new(|_| {
                SliderState::new()
                    .min(min)
                    .max(max)
                    .step(passo)
                    .default_value(valor)
            })
        };
        let sliders = SlidersLocais {
            raio: slider(0.0, 1.0, 0.001, posicao_do_tamanho(TAMANHO_INICIAL), cx),
            feather: slider(0.0, 1.0, 0.01, FEATHER_INICIAL, cx),
            opacidade: slider(0.05, 1.0, 0.01, 1.0, cx),
            exposicao: slider(-3.0, 3.0, 0.05, 0.0, cx),
        };
        for (estado, qual) in [
            (&sliders.raio, 0u8),
            (&sliders.feather, 1),
            (&sliders.opacidade, 2),
        ] {
            assinaturas.push(cx.subscribe_in(
                estado,
                window,
                move |tela: &mut Revelacao, _e, evento: &SliderEvent, _w, cx| {
                    let (valor, soltou) = match evento {
                        SliderEvent::Change(v) => (v.start(), false),
                        SliderEvent::Release(v) => (v.start(), true),
                    };
                    tela.slider_local(qual, valor, soltou, cx);
                },
            ));
        }
        assinaturas.push(cx.subscribe_in(
            &sliders.exposicao,
            window,
            move |tela: &mut Revelacao, _e, evento: &SliderEvent, _w, cx| {
                let (valor, soltou) = match evento {
                    SliderEvent::Change(v) => (v.start(), false),
                    SliderEvent::Release(v) => (v.start(), true),
                };
                tela.exposicao_da_mascara(valor, soltou, cx);
            },
        ));
        Self {
            ferramenta: None,
            mascara_sel: None,
            criando: false,
            subtrair: false,
            poligonal: false,
            preencher_por_laco: false,
            marcacoes: true,
            gesto: None,
            poligono: Vec::new(),
            origem: None,
            selecionado: None,
            cursor: None,
            tamanho: TAMANHO_INICIAL,
            feather: FEATHER_INICIAL,
            opacidade: 1.0,
            sliders,
        }
    }

    /// Solta tudo o que é da foto que sai.
    pub(in crate::revelacao) fn esquecer_a_foto(&mut self) {
        self.gesto = None;
        self.poligono.clear();
        self.origem = None;
        self.selecionado = None;
        self.mascara_sel = None;
        self.criando = self.ferramenta.is_some_and(Ferramenta::de_mascara);
    }

    pub(in crate::revelacao) fn arrastando(&self) -> bool {
        self.gesto.is_some()
    }
}

/// Um traço das marcações, em pontos da área.
enum Marca {
    Circulo {
        centro: Ponto,
        raio: f32,
        cor: Hsla,
        tracejado: bool,
    },
    Caminho {
        pontos: Vec<Ponto>,
        fechado: bool,
        cor: Hsla,
        tracejado: bool,
        largura: f32,
    },
    /// A borda de uma área com raio: trechos abertos, riscados como o
    /// círculo (linha fina, sombra leve que segue o tracejado).
    Contorno {
        trechos: Vec<Vec<Ponto>>,
        cor: Hsla,
        tracejado: bool,
    },
    Alca {
        centro: Ponto,
        ativa: bool,
    },
    Alfinete {
        centro: Ponto,
        ativo: bool,
    },
}

fn branco() -> Hsla {
    gpui_kit::white()
}

fn ambar() -> Hsla {
    gpui_kit::rgb(0xe0a24a).into()
}

/// O pedaço do segmento `a`–`b` (0–1 da foto) que fica dentro da foto
/// (Liang–Barsky). `None` se passa todo por fora.
fn cortar_na_foto(a: [f32; 2], b: [f32; 2]) -> Option<([f32; 2], [f32; 2])> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d[0], a[0]),
        (d[0], 1.0 - a[0]),
        (-d[1], a[1]),
        (d[1], 1.0 - a[1]),
    ] {
        if p.abs() < 1e-9 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
    }
    (t0 <= t1).then(|| {
        (
            [a[0] + d[0] * t0, a[1] + d[1] * t0],
            [a[0] + d[0] * t1, a[1] + d[1] * t1],
        )
    })
}

/// "Máscara N" com o menor N livre na foto — o contador corrido dava
/// "Máscara 2" à primeira máscara depois de um ⌘Z, e seguia contando de uma
/// foto para a outra.
fn nome_livre(parametros: &ParametrosLocais) -> String {
    (1..)
        .map(|n| format!("Máscara {n}"))
        .find(|nome| parametros.camadas.iter().all(|c| &c.nome != nome))
        .unwrap_or_default()
}

/// 🧪 A revelação numa linha, para o roteiro: camadas e retoques com o que
/// importa conferir.
fn resumo_dos_parametros(r: &ParametrosLocais) -> String {
    let camadas = r.camadas.iter().map(|c| {
        let formas: Vec<String> = c
            .componentes
            .iter()
            .map(|k| match (&k.forma, k.modo) {
                (Forma::Pincel(_), Modo::Somar) => "pincel+".to_string(),
                (Forma::Pincel(_), Modo::Subtrair) => "pincel-".to_string(),
                (Forma::Linear(g), _) => format!(
                    "linear ({:.3},{:.3})→({:.3},{:.3})",
                    g.inicio[0], g.inicio[1], g.fim[0], g.fim[1]
                ),
                (Forma::Radial(g), _) => format!(
                    "radial c=({:.3},{:.3}) r=({:.3},{:.3})",
                    g.centro[0], g.centro[1], g.raio_x, g.raio_y
                ),
                (Forma::Laco(l), _) => format!("laco {}", l.pontos.len()),
            })
            .collect();
        format!(
            "[{} ev={:+.2} vis={} inv={} {:?}]",
            c.nome, c.ajustes.exposicao_ev, c.visivel, c.invertida, formas
        )
    });
    let retoques = r.retoques.iter().map(|t| match t {
        Retoque::Clone(c) | Retoque::Heal(c) => format!(
            "[{} r={:.4} origem=({:.3},{:.3}) destino=({:.3},{:.3}) op={:.2}]",
            if matches!(t, Retoque::Clone(_)) {
                "clone"
            } else {
                "heal"
            },
            c.raio,
            c.origem[0],
            c.origem[1],
            c.destino_inicial[0],
            c.destino_inicial[1],
            c.opacidade
        ),
        Retoque::Preencher(p) => format!(
            "[preencher r={:.4} pontos={} laco={}]",
            p.raio,
            p.caminho.len(),
            p.laco.len()
        ),
    });
    camadas.chain(retoques).collect::<Vec<_>>().join(" ")
}

fn sombra() -> Hsla {
    gpui_kit::rgba(0x0000008c).into()
}

fn sombra_leve() -> Hsla {
    gpui_kit::rgba(0x00000066).into()
}

impl Revelacao {
    // ------------------------------------------------------------ a revelação

    /// A revelação que a GPU revela agora: a da foto, mais o gesto em curso.
    pub(super) fn locais_com_o_gesto(&self) -> Arc<ParametrosLocais> {
        let Some(gesto) = &self.local.gesto else {
            return self.locais.clone();
        };
        let mut parametros = (*self.locais).clone();
        match gesto {
            Gesto::Componente { nova, componente } => {
                if *nova {
                    parametros.camadas.push(Camada {
                        componentes: vec![componente.clone()],
                        ajustes: AjustesLocais {
                            exposicao_ev: self.exposicao_da_nova(),
                        },
                        ..Default::default()
                    });
                } else if let Some(camada) = self
                    .local
                    .mascara_sel
                    .and_then(|i| parametros.camadas.get_mut(i))
                {
                    camada.componentes.push(componente.clone());
                }
            }
            // O Content-Aware sintetiza no fim do gesto, e não a cada ponto.
            Gesto::Retoque(Retoque::Preencher(_)) => {}
            Gesto::Retoque(r) => parametros.retoques.push(r.clone()),
            Gesto::Editando { indice, valor, .. } => {
                if let Some(r) = parametros.retoques.get_mut(*indice) {
                    *r = valor.clone();
                }
            }
            Gesto::Mascara {
                camada,
                componente,
                valor,
                ..
            } => {
                if let Some(k) = parametros
                    .camadas
                    .get_mut(*camada)
                    .and_then(|c| c.componentes.get_mut(*componente))
                {
                    *k = valor.clone();
                }
            }
        }
        Arc::new(parametros)
    }

    /// A exposição com que nasce uma máscara nova: a do slider.
    fn exposicao_da_nova(&self) -> f32 {
        self.local
            .mascara_sel
            .and_then(|i| self.locais.camadas.get(i))
            .map(|c| c.ajustes.exposicao_ev)
            .unwrap_or(0.8)
    }

    /// A revelação nova vira um passo do histórico, e vai ao banco e à GPU.
    fn comprometer(&mut self, parametros: ParametrosLocais, cx: &mut Context<Self>) {
        self.gravar_o_que_estiver_pendente();
        self.locais = Arc::new(parametros);
        self.historico.registrar(self.estado());
        self.gravar();
        self.pedir_revelacao(cx);
        cx.notify();
    }

    // --------------------------------------------------------------- a conta

    /// O ponto da área (pontos) na foto inteira (0–1).
    fn foto_do_ponto(&self, p: Ponto) -> Option<[f32; 2]> {
        let (cena, vista) = self.vista()?;
        let (w, h) = self.tamanho_da_copia()?;
        let s = (p.x - vista.x) / (cena.janela.largura * vista.escala);
        let t = (p.y - vista.y) / (cena.janela.altura * vista.escala);
        let corte = infrastructure::transformacao::corte(&self.corte_atual());
        Some(transformacao::do_quadro_para_a_foto(
            w as u32, h as u32, &corte, s, t,
        ))
    }

    /// O ponto da foto (0–1) na área (pontos).
    pub(super) fn ponto_da_foto(&self, q: [f32; 2]) -> Option<Ponto> {
        let (cena, vista) = self.vista()?;
        let (w, h) = self.tamanho_da_copia()?;
        let corte = infrastructure::transformacao::corte(&self.corte_atual());
        let [s, t] = transformacao::da_foto_para_o_quadro(w as u32, h as u32, &corte, q[0], q[1]);
        Some(Ponto {
            x: vista.x + s * cena.janela.largura * vista.escala,
            y: vista.y + t * cena.janela.altura * vista.escala,
        })
    }

    /// Quantos pontos da tela tem um pixel da foto.
    fn pontos_por_pixel(&self) -> Option<f32> {
        let (cena, vista) = self.vista()?;
        let (w, h) = self.tamanho_da_copia()?;
        let corte = infrastructure::transformacao::corte(&self.corte_atual());
        let largura =
            transformacao::pixels_da_foto_na_largura_do_quadro(w as u32, h as u32, &corte);
        Some(cena.janela.largura * vista.escala / largura.max(1.0))
    }

    /// Quantos pontos da tela tem o raio 1 — a fração inteira do maior lado.
    fn escala_do_raio(&self) -> Option<f32> {
        let lado = self.tamanho_da_copia().map(|(w, h)| w.max(h))?;
        let escala = lado * self.pontos_por_pixel()?;
        (escala.is_finite() && escala > 0.0).then_some(escala)
    }

    /// O raio (fração do maior lado) que um tamanho na tela tem neste zoom.
    fn raio_do_tamanho(&self, tamanho: f32, teto: f32) -> f32 {
        match self.escala_do_raio() {
            Some(escala) => (tamanho / escala).clamp(RAIO_MINIMO_DA_FERRAMENTA, teto),
            None => RAIO_INICIAL,
        }
    }

    /// O raio da ferramenta na foto, agora: o tamanho na tela dividido pelo
    /// zoom. É ele que o gesto grava na revelação.
    fn raio_da_ferramenta(&self) -> f32 {
        self.raio_do_tamanho(self.local.tamanho, RAIO_MAXIMO_DA_FERRAMENTA)
    }

    /// O diâmetro que o círculo da ferramenta cobre, em pixels da foto
    /// **original**. A cópia na tela troca de resolução com o zoom (a
    /// encaixada pode ter 2048 px e a ampliada, 4608): contar pelos dela faria
    /// o mesmo carimbo mudar de número só por aproximar.
    fn diametro_da_ferramenta(&self) -> f32 {
        let lado = self.lado_da_foto() * self.fator_do_bruto();
        2.0 * self.raio_da_ferramenta() * lado
    }

    /// Um raio da revelação (fração do maior lado) em pontos da tela.
    fn raio_na_tela(&self, raio: f32) -> f32 {
        let lado = self
            .tamanho_da_copia()
            .map(|(w, h)| w.max(h))
            .unwrap_or(1.0);
        raio * lado * self.pontos_por_pixel().unwrap_or(0.0)
    }

    /// A distância, em pixels da foto, de `q` ao caminho.
    fn distancia_ao_caminho(&self, q: [f32; 2], caminho: &[[f32; 2]]) -> f32 {
        let Some((w, h)) = self.tamanho_da_copia() else {
            return f32::MAX;
        };
        let px = |a: [f32; 2]| [a[0] * w, a[1] * h];
        let p = px(q);
        let n = caminho.len().max(2) - 1;
        (0..n)
            .map(|i| {
                let a = px(caminho[i.min(caminho.len() - 1)]);
                let b = px(caminho[(i + 1).min(caminho.len() - 1)]);
                let ab = [b[0] - a[0], b[1] - a[1]];
                let ap = [p[0] - a[0], p[1] - a[1]];
                let l2 = ab[0] * ab[0] + ab[1] * ab[1];
                let t = if l2 > 1e-9 {
                    ((ap[0] * ab[0] + ap[1] * ab[1]) / l2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                ((ap[0] - ab[0] * t).powi(2) + (ap[1] - ab[1] * t).powi(2)).sqrt()
            })
            .fold(f32::MAX, f32::min)
    }

    fn lado_da_foto(&self) -> f32 {
        self.tamanho_da_copia()
            .map(|(w, h)| w.max(h))
            .unwrap_or(1.0)
    }

    // -------------------------------------------------------------- escolher

    /// Escolhe a ferramenta (ou solta, se é a mesma). Máscara sem máscara
    /// selecionada cria uma nova no primeiro gesto.
    pub fn usar_ferramenta(&mut self, ferramenta: Ferramenta, cx: &mut Context<Self>) {
        if self.edicao.is_some() {
            return;
        }
        if self.local.ferramenta == Some(ferramenta) {
            self.local.ferramenta = None;
        } else {
            self.local.ferramenta = Some(ferramenta);
            // Pelo atalho, com a sanfona recolhida, os controles da
            // ferramenta ficariam escondidos: escolher abre o painel.
            self.estado_do_painel.abrir(CHAVE_DO_PAINEL_LOCAL);
        }
        self.local.poligono.clear();
        self.local.gesto = None;
        if ferramenta.de_mascara() {
            self.local.criando = self.local.mascara_sel.is_none();
            self.local.selecionado = None;
        } else {
            self.local.mascara_sel = None;
            self.local.criando = false;
            if !matches!(ferramenta, Ferramenta::Carimbo | Ferramenta::BandAid) {
                self.local.selecionado = None;
            }
        }
        cx.notify();
    }

    /// Cria uma máscara nova com esta ferramenta (o "+" do painel).
    pub fn nova_mascara_com(&mut self, ferramenta: Ferramenta, cx: &mut Context<Self>) {
        self.local.ferramenta = Some(ferramenta);
        self.local.mascara_sel = None;
        self.local.criando = true;
        self.local.subtrair = false;
        self.local.selecionado = None;
        self.local.poligono.clear();
        cx.notify();
    }

    /// Está com alguma ferramenta da Revelação local na mão?
    pub fn com_ferramenta_local(&self) -> bool {
        self.local.ferramenta.is_some()
    }

    /// Onde a foto está desenhada, em pixels da janela.
    pub fn palco_da_foto(&self) -> gpui_kit::Bounds<gpui_kit::Pixels> {
        self.palco
    }

    /// 🧪 O estado da Revelação local numa linha, para o roteiro conferir.
    pub fn descrever_local(&self, window: &Window) -> String {
        format!(
            "ferramenta={:?} raio={:.5} tamanho={:.1} feather={:.2} selecionado={:?} mascara_sel={:?} \
             camadas={} componentes={} retoques={} foco_no_palco={} marcacoes={} gesto={} \
             origem={:?} bruta={} revelada={} aguardando={:?} gpu={:?} cursor={:?} \
             pontos_do_ultimo={} desfaz={} refaz={}\n        revelação: {}",
            self.local.ferramenta,
            self.raio_da_ferramenta(),
            self.local.tamanho,
            self.local.feather,
            self.local.selecionado,
            self.local.mascara_sel,
            self.locais.camadas.len(),
            self.locais
                .camadas
                .iter()
                .map(|c| c.componentes.len())
                .sum::<usize>(),
            self.locais.retoques.len(),
            self.foco_do_palco.is_focused(window),
            self.local.marcacoes,
            self.local.gesto.is_some(),
            self.aberta
                .as_ref()
                .and_then(|a| a.origem.as_ref())
                .map(|o| (o.largura, o.altura)),
            self.aberta.as_ref().is_some_and(|a| a.bruta.is_some()),
            self.aberta.as_ref().is_some_and(|a| a.revelada.is_some()),
            self.aguardando,
            self.processador.disponivel(),
            self.local.cursor.map(|c| (c.x.round(), c.y.round())),
            self.locais
                .camadas
                .last()
                .and_then(|c| c.componentes.last())
                .map_or(0, |c| match &c.forma {
                    Forma::Pincel(t) => t.pontos.len(),
                    Forma::Laco(l) => l.pontos.len(),
                    _ => 0,
                }),
            self.historico.pode_desfazer(),
            self.historico.pode_refazer(),
            resumo_dos_parametros(&self.locais),
        )
    }

    /// O tamanho da ferramenta, em fração do maior lado da foto.
    #[cfg(test)]
    pub fn raio_local(&self) -> f32 {
        self.raio_da_ferramenta()
    }

    /// Quantos pontos da tela tem o raio 1, no zoom de agora.
    #[cfg(test)]
    pub fn escala_local(&self) -> Option<f32> {
        self.escala_do_raio()
    }

    /// O meio do palco, em pixels da janela — onde o teste clica na foto.
    #[cfg(test)]
    pub fn meio_do_palco(&self) -> gpui_kit::Point<gpui_kit::Pixels> {
        self.palco.center()
    }

    /// Quantos retoques a revelação tem.
    #[cfg(test)]
    pub fn retoques_locais(&self) -> usize {
        self.locais.retoques.len()
    }

    /// Se há um gesto da Revelação local em curso.
    #[cfg(test)]
    pub fn gesto_local_em_curso(&self) -> bool {
        self.local.gesto.is_some()
    }

    /// Põe o cursor na busca de predefinições, como o clique do operador.
    #[cfg(test)]
    pub fn focar_a_busca(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.busca_de_presets
            .update(cx, |campo, cx| campo.focus(window, cx));
    }

    /// `Esc`: larga o laço em curso, depois a seleção, depois a ferramenta.
    /// Devolve se fez alguma coisa (senão o `Esc` segue para a raiz).
    pub fn esc_local(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.local.poligono.is_empty() {
            self.local.poligono.clear();
        } else if self.local.selecionado.is_some() {
            self.local.selecionado = None;
        } else if self.local.ferramenta.is_some() {
            self.local.ferramenta = None;
            self.local.criando = false;
        } else {
            return false;
        }
        cx.notify();
        true
    }

    /// Está na Revelação local: com ferramenta, máscara ou retoque escolhido,
    /// ou com um laço pela metade.
    pub fn dentro_da_revelacao_local(&self) -> bool {
        self.local.ferramenta.is_some()
            || self.local.mascara_sel.is_some()
            || self.local.selecionado.is_some()
            || !self.local.poligono.is_empty()
    }

    /// O botão de sair: tudo o que o `Esc` larga em vários toques, de uma vez
    /// — a foto volta ao que a Revelação global mostra.
    pub fn sair_da_revelacao_local(&mut self, cx: &mut Context<Self>) {
        self.local.poligono.clear();
        self.local.origem = None;
        self.local.selecionado = None;
        self.local.mascara_sel = None;
        self.local.ferramenta = None;
        self.local.criando = false;
        cx.notify();
    }

    pub fn alternar_marcacoes(&mut self, cx: &mut Context<Self>) {
        self.local.marcacoes = !self.local.marcacoes;
        cx.notify();
    }

    /// `[` e `]` (com Shift, a suavização): na ferramenta e, se houver, no
    /// retoque selecionado.
    pub fn mudar_tamanho_local(
        &mut self,
        mais: bool,
        suavizacao: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if suavizacao {
            let novo = (self.local.feather + if mais { 0.05 } else { -0.05 }).clamp(0.0, 1.0);
            self.local
                .sliders
                .feather
                .update(cx, |s, cx| s.set_value(novo, window, cx));
            self.slider_local(1, novo, true, cx);
            return;
        }
        let fator = if mais { 1.1 } else { 1.0 / 1.1 };
        // O retoque selecionado cresce junto — mas a ferramenta também: o
        // círculo do cursor é o retorno que o dedo espera ver.
        if let Some(i) = self.local.selecionado {
            if let Some(r) = self.locais.retoques.get(i).cloned() {
                let novo = com_raio(
                    &r,
                    (raio_do(&r) * fator).clamp(RAIO_MINIMO_DA_FERRAMENTA, RAIO_MAXIMO_DO_RETOQUE),
                );
                let mut parametros = (*self.locais).clone();
                parametros.retoques[i] = novo;
                self.comprometer(parametros, cx);
            }
        }
        let novo = (self.local.tamanho * fator).clamp(TAMANHO_MINIMO, TAMANHO_MAXIMO);
        self.local.sliders.raio.update(cx, |s, cx| {
            s.set_value(posicao_do_tamanho(novo), window, cx)
        });
        self.local.tamanho = novo;
        cx.notify();
    }

    /// `Delete` com um retoque selecionado. Devolve se apagou.
    pub fn apagar_retoque_selecionado(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(i) = self.local.selecionado.take() else {
            return false;
        };
        let mut parametros = (*self.locais).clone();
        if i >= parametros.retoques.len() {
            return false;
        }
        parametros.retoques.remove(i);
        self.comprometer(parametros, cx);
        true
    }

    fn slider_local(&mut self, qual: u8, valor: f32, soltou: bool, cx: &mut Context<Self>) {
        // O Tamanho chega em posição do trilho e vira pontos da tela; o
        // retoque selecionado recebe o raio que esse tamanho tem neste zoom.
        let valor = if qual == 0 {
            let tamanho = tamanho_da_posicao(valor);
            self.local.tamanho = tamanho;
            self.raio_do_tamanho(tamanho, RAIO_MAXIMO_DO_RETOQUE)
        } else {
            valor
        };
        match qual {
            0 => {}
            1 => self.local.feather = valor,
            _ => self.local.opacidade = valor,
        }
        // Com um retoque selecionado, o slider é dele.
        if let Some(i) = self.local.selecionado {
            if let Some(original) = self.locais.retoques.get(i).cloned() {
                let valor_do_retoque = match (qual, &original) {
                    (1, Retoque::Preencher(p)) if !p.laco.is_empty() => valor * FEATHER_DO_LACO,
                    _ => valor,
                };
                let novo = match qual {
                    0 => com_raio(&original, valor_do_retoque),
                    1 => com_feather(&original, valor_do_retoque),
                    _ => com_opacidade(&original, valor_do_retoque),
                };
                if soltou {
                    self.local.gesto = None;
                    let mut parametros = (*self.locais).clone();
                    parametros.retoques[i] = novo;
                    self.comprometer(parametros, cx);
                } else {
                    self.local.gesto = Some(Gesto::Editando {
                        indice: i,
                        parte: Parte::Raio,
                        de: [0.0, 0.0],
                        original,
                        valor: novo,
                    });
                    self.pedir_revelacao(cx);
                }
            }
        }
        cx.notify();
    }

    /// O slider de exposição da máscara selecionada: ao vivo, e um passo ao
    /// soltar.
    fn exposicao_da_mascara(&mut self, valor: f32, soltou: bool, cx: &mut Context<Self>) {
        let Some(i) = self.local.mascara_sel else {
            return;
        };
        if i >= self.locais.camadas.len() {
            return;
        }
        let mut parametros = (*self.locais).clone();
        parametros.camadas[i].ajustes.exposicao_ev = valor;
        if soltou {
            self.comprometer(parametros, cx);
        } else {
            // Ao vivo, sem passo: a revelação da tela anda, o histórico não.
            self.locais = Arc::new(parametros);
            self.pedir_revelacao(cx);
        }
    }

    /// Seleciona a máscara `i` (ou solta, se já era ela).
    pub fn selecionar_mascara(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.local.mascara_sel == Some(i) {
            self.local.mascara_sel = None;
        } else {
            self.local.mascara_sel = Some(i);
            self.local.criando = false;
            self.local.selecionado = None;
            if let Some(c) = self.locais.camadas.get(i) {
                let ev = c.ajustes.exposicao_ev;
                self.local
                    .sliders
                    .exposicao
                    .update(cx, |s, cx| s.set_value(ev, window, cx));
            }
            if !self.local.ferramenta.is_some_and(Ferramenta::de_mascara) {
                self.local.ferramenta = None;
            }
        }
        cx.notify();
    }

    fn mudar_mascara(
        &mut self,
        i: usize,
        cx: &mut Context<Self>,
        mudar: impl FnOnce(&mut Camada) -> bool,
    ) {
        let mut parametros = (*self.locais).clone();
        let Some(camada) = parametros.camadas.get_mut(i) else {
            return;
        };
        if mudar(camada) {
            parametros.camadas.remove(i);
            self.local.mascara_sel = None;
        }
        self.comprometer(parametros, cx);
    }

    // ---------------------------------------------------------------- gestos

    /// O que o ponteiro pega num retoque existente, na ordem: as alças do
    /// selecionado, a origem dele, e o destino de qualquer um.
    fn acertar(&self, p: Ponto, q: [f32; 2]) -> Option<(usize, Parte)> {
        if let Some(i) = self.local.selecionado {
            if let Some(r) = self.locais.retoques.get(i) {
                for (alca, parte) in self.alcas(r) {
                    if (alca.x - p.x).hypot(alca.y - p.y) <= ALCANCE_DA_ALCA {
                        return Some((i, parte));
                    }
                }
            }
        }
        let mut ordem: Vec<usize> = (0..self.locais.retoques.len()).rev().collect();
        if let Some(i) = self.local.selecionado {
            ordem.retain(|&k| k != i);
            ordem.insert(0, i);
        }
        let lado = self.lado_da_foto();
        for k in ordem {
            let r = &self.locais.retoques[k];
            if let (Some(c), true) = (r.carimbo(), Some(k) == self.local.selecionado) {
                let [dx, dy] = c.deslocamento();
                let fonte: Vec<[f32; 2]> =
                    c.caminho.iter().map(|a| [a[0] + dx, a[1] + dy]).collect();
                if self.distancia_ao_caminho(q, &fonte) <= c.raio * lado {
                    return Some((k, Parte::Origem));
                }
            }
            let toca = match r {
                Retoque::Preencher(p) if !p.laco.is_empty() => {
                    let (w, h) = self.tamanho_da_copia().unwrap_or((1.0, 1.0));
                    let poligono: Vec<[f32; 2]> =
                        p.laco.iter().map(|a| [a[0] * w, a[1] * h]).collect();
                    locais::distancia_ao_laco(&poligono, [q[0] * w, q[1] * h]) > 0.0
                }
                _ => {
                    let t = r.traco();
                    let caminho: Vec<[f32; 2]> = t.pontos.iter().map(|a| [a[0], a[1]]).collect();
                    self.distancia_ao_caminho(q, &caminho) <= t.raio * lado
                }
            };
            if toca {
                return Some((k, Parte::Destino));
            }
        }
        None
    }

    /// As alças de um retoque, em pontos da área.
    fn alcas(&self, r: &Retoque) -> Vec<(Ponto, Parte)> {
        match r {
            Retoque::Preencher(p) if !p.laco.is_empty() => {
                let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for a in &p.laco {
                    x0 = x0.min(a[0]);
                    y0 = y0.min(a[1]);
                    x1 = x1.max(a[0]);
                    y1 = y1.max(a[1]);
                }
                [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
                    .into_iter()
                    .filter_map(|q| Some((self.ponto_da_foto(q)?, Parte::Escala)))
                    .collect()
            }
            _ => {
                let t = r.traco();
                let Some(fim) = t.pontos.last() else {
                    return Vec::new();
                };
                let Some(c) = self.ponto_da_foto([fim[0], fim[1]]) else {
                    return Vec::new();
                };
                let raio = self.raio_na_tela(t.raio);
                // Retoque do tamanho de uma sarda: as quatro alças se
                // empilhariam sobre ele e esconderiam o que se corrige. O
                // tamanho segue no slider e no `[` `]`.
                if raio < RAIO_MINIMO_COM_ALCAS {
                    return Vec::new();
                }
                [(raio, 0.0), (0.0, raio), (-raio, 0.0), (0.0, -raio)]
                    .into_iter()
                    .map(|(dx, dy)| {
                        (
                            Ponto {
                                x: c.x + dx,
                                y: c.y + dy,
                            },
                            Parte::Raio,
                        )
                    })
                    .collect()
            }
        }
    }

    /// Um componente da máscara como está na tela — o do gesto, se é ele que
    /// se arrasta.
    fn componente_na_tela(&self, camada: usize, k: usize) -> Option<Componente> {
        if let Some(Gesto::Mascara {
            camada: c,
            componente,
            valor,
            ..
        }) = &self.local.gesto
        {
            if (*c, *componente) == (camada, k) {
                return Some(valor.clone());
            }
        }
        self.locais.camadas.get(camada)?.componentes.get(k).cloned()
    }

    /// Os pontos (0–1) das alças de um componente, com o que cada uma faz.
    /// O pincel não tem alça: ele se refaz pintando.
    fn alcas_do_componente(&self, k: &Componente) -> Vec<([f32; 2], AlcaDeMascara)> {
        let (w, h) = self.tamanho_da_copia().unwrap_or((1.0, 1.0));
        let lado = self.lado_da_foto();
        match &k.forma {
            Forma::Linear(g) => vec![
                (g.inicio, AlcaDeMascara::Inicio),
                (g.fim, AlcaDeMascara::Fim),
                (
                    [
                        (g.inicio[0] + g.fim[0]) / 2.0,
                        (g.inicio[1] + g.fim[1]) / 2.0,
                    ],
                    AlcaDeMascara::Mover,
                ),
            ],
            Forma::Radial(g) => {
                let (s, c) = g.angulo.to_radians().sin_cos();
                let rx = g.raio_x * lado;
                let ry = g.raio_y * lado;
                vec![
                    (g.centro, AlcaDeMascara::Mover),
                    (
                        [g.centro[0] + rx * c / w, g.centro[1] + rx * s / h],
                        AlcaDeMascara::RaioX,
                    ),
                    (
                        [g.centro[0] - ry * s / w, g.centro[1] + ry * c / h],
                        AlcaDeMascara::RaioY,
                    ),
                ]
            }
            Forma::Laco(l) if !l.pontos.is_empty() => {
                let n = l.pontos.len() as f32;
                let (sx, sy) = l
                    .pontos
                    .iter()
                    .fold((0.0, 0.0), |(x, y), p| (x + p[0], y + p[1]));
                vec![([sx / n, sy / n], AlcaDeMascara::Mover)]
            }
            _ => Vec::new(),
        }
    }

    /// A alça da máscara selecionada sob o ponteiro.
    fn acertar_alca_da_mascara(&self, p: Ponto) -> Option<(usize, usize, AlcaDeMascara)> {
        let camada = self.local.mascara_sel?;
        let n = self.locais.camadas.get(camada)?.componentes.len();
        (0..n).rev().find_map(|k| {
            let comp = self.componente_na_tela(camada, k)?;
            self.alcas_do_componente(&comp)
                .into_iter()
                .find(|(q, _)| {
                    self.ponto_da_foto(*q)
                        .is_some_and(|a| (a.x - p.x).hypot(a.y - p.y) <= ALCANCE_DA_ALCA)
                })
                .map(|(_, alca)| (camada, k, alca))
        })
    }

    /// A máscara (visível) que tem um componente sob o ponteiro.
    fn mascara_em(&self, p: Ponto, q: [f32; 2]) -> Option<(usize, Ferramenta)> {
        let (w, h) = self.tamanho_da_copia()?;
        let lado = self.lado_da_foto();
        // Pixels da cópia → pontos da tela.
        let por_pixel = self.raio_na_tela(1.0 / lado);
        for (i, camada) in self.locais.camadas.iter().enumerate().rev() {
            if !camada.visivel {
                continue;
            }
            for k in camada.componentes.iter().rev() {
                let acerta = match &k.forma {
                    Forma::Pincel(t) => {
                        let c: Vec<[f32; 2]> = t.pontos.iter().map(|a| [a[0], a[1]]).collect();
                        self.distancia_ao_caminho(q, &c) <= t.raio * lado
                    }
                    Forma::Linear(g) => {
                        self.distancia_ao_caminho(q, &[g.inicio, g.fim]) * por_pixel
                            <= ALCANCE_DA_ALCA * 1.5
                    }
                    Forma::Radial(g) => {
                        let (s, c) = g.angulo.to_radians().sin_cos();
                        let dx = (q[0] - g.centro[0]) * w;
                        let dy = (q[1] - g.centro[1]) * h;
                        let u = (dx * c + dy * s) / (g.raio_x * lado).max(1e-6);
                        let v = (-dx * s + dy * c) / (g.raio_y * lado).max(1e-6);
                        u * u + v * v <= 1.0
                    }
                    Forma::Laco(l) => {
                        let poligono: Vec<[f32; 2]> =
                            l.pontos.iter().map(|a| [a[0] * w, a[1] * h]).collect();
                        locais::distancia_ao_laco(&poligono, [q[0] * w, q[1] * h]) > 0.0
                    }
                };
                if acerta {
                    let f = match &k.forma {
                        Forma::Pincel(_) => Ferramenta::Pincel,
                        Forma::Linear(_) => Ferramenta::Linear,
                        Forma::Radial(_) => Ferramenta::Radial,
                        Forma::Laco(_) => Ferramenta::Laco,
                    };
                    let _ = p;
                    return Some((i, f));
                }
            }
        }
        None
    }

    /// 🔑 **Duplo clique na foto, sem ferramenta: entra na edição do que está
    /// ali** — o retoque, com a ferramenta dele e as alças; ou a máscara, com
    /// a ferramenta do componente e as alças dela (pedido do dono). Devolve se
    /// achou alguma coisa.
    pub(super) fn editar_o_que_esta_em(
        &mut self,
        p: Ponto,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.revelacao_travada() {
            return false;
        }
        let Some(q) = self.foto_do_ponto(p) else {
            return false;
        };
        self.local.selecionado = None;
        if let Some((k, _)) = self.acertar(p, q) {
            let f = match &self.locais.retoques[k] {
                Retoque::Clone(_) => Ferramenta::Carimbo,
                Retoque::Heal(_) => Ferramenta::BandAid,
                Retoque::Preencher(_) => Ferramenta::Preencher,
            };
            self.local.ferramenta = Some(f);
            self.local.selecionado = Some(k);
            self.local.mascara_sel = None;
            self.local.criando = false;
            self.estado_do_painel.abrir(CHAVE_DO_PAINEL_LOCAL);
            self.sincronizar_sliders_com_o_selecionado(window, cx);
            cx.notify();
            return true;
        }
        if let Some((i, f)) = self.mascara_em(p, q) {
            self.local.ferramenta = Some(f);
            self.local.mascara_sel = Some(i);
            self.local.criando = false;
            self.estado_do_painel.abrir(CHAVE_DO_PAINEL_LOCAL);
            cx.notify();
            return true;
        }
        false
    }

    /// A origem automática do band-aid: do lado com mais espaço, a 3 raios.
    fn origem_automatica(&self, q: [f32; 2]) -> [f32; 2] {
        let (w, h) = self.tamanho_da_copia().unwrap_or((1.0, 1.0));
        let r = self.raio_da_ferramenta();
        let d = 3.0 * r * w.max(h);
        let candidatos = [
            [q[0] + d / w, q[1]],
            [q[0] - d / w, q[1]],
            [q[0], q[1] + d / h],
            [q[0], q[1] - d / h],
        ];
        candidatos
            .into_iter()
            .find(|c| c[0] > r && c[0] < 1.0 - r && c[1] > r && c[1] < 1.0 - r)
            .unwrap_or(candidatos[0])
    }

    pub(super) fn local_apertar(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ferramenta) = self.local.ferramenta else {
            return;
        };
        if self.revelacao_travada() {
            return;
        }
        crate::desempenho::operacao(ferramenta.operacao_de_desempenho());
        let p = self.ponto_na_area(e.position);
        let Some(q) = self.foto_do_ponto(p) else {
            return;
        };
        let alt = e.modifiers.alt;
        let laco = ferramenta.usa_laco()
            || (ferramenta == Ferramenta::Preencher && self.local.preencher_por_laco);

        // O laço poligonal: vértice a vértice; o primeiro ponto, ou o clique
        // duplo, fecha.
        if laco && self.local.poligonal {
            let fecha = self.local.poligono.len() >= 3
                && (e.click_count >= 2
                    || self
                        .ponto_da_foto(self.local.poligono[0])
                        .is_some_and(|a| (a.x - p.x).hypot(a.y - p.y) < 9.0));
            if fecha {
                let pontos = std::mem::take(&mut self.local.poligono);
                self.laco_pronto(ferramenta, pontos, cx);
            } else {
                self.local.poligono.push(q);
            }
            cx.notify();
            return;
        }

        // As alças da máscara selecionada vêm antes de um componente novo.
        if ferramenta.de_mascara() && !alt {
            if let Some((camada, k, alca)) = self.acertar_alca_da_mascara(p) {
                if let Some(original) = self.componente_na_tela(camada, k) {
                    self.local.gesto = Some(Gesto::Mascara {
                        camada,
                        componente: k,
                        alca,
                        de: q,
                        valor: original.clone(),
                        original,
                    });
                    cx.notify();
                    return;
                }
            }
        }

        if matches!(ferramenta, Ferramenta::Carimbo | Ferramenta::BandAid) && alt {
            self.local.origem = Some(q);
            self.local.selecionado = None;
            cx.notify();
            return;
        }

        // Um retoque existente sob o ponteiro: seleciona e, arrastando, muda.
        if !ferramenta.de_mascara() {
            if let Some((indice, parte)) = self.acertar(p, q) {
                let original = self.locais.retoques[indice].clone();
                self.local.selecionado = Some(indice);
                self.sincronizar_sliders_com_o_selecionado(window, cx);
                self.local.gesto = Some(Gesto::Editando {
                    indice,
                    parte,
                    de: q,
                    valor: original.clone(),
                    original,
                });
                cx.notify();
                return;
            }
        }
        self.local.selecionado = None;

        let modo = if self.local.subtrair != alt {
            Modo::Subtrair
        } else {
            Modo::Somar
        };
        let nova = self.local.criando || self.local.mascara_sel.is_none();
        let raio = self.raio_da_ferramenta();
        let feather = self.local.feather;
        let opacidade = self.local.opacidade;
        self.local.gesto = Some(match ferramenta {
            Ferramenta::Pincel => Gesto::Componente {
                nova,
                componente: Componente {
                    modo,
                    forma: Forma::Pincel(BrushStroke {
                        raio,
                        feather,
                        opacidade,
                        pontos: vec![[q[0], q[1], 1.0]],
                    }),
                },
            },
            Ferramenta::Linear => Gesto::Componente {
                nova,
                componente: Componente {
                    modo,
                    forma: Forma::Linear(GradienteLinear {
                        inicio: q,
                        fim: [q[0], q[1] + 0.001],
                    }),
                },
            },
            Ferramenta::Radial => Gesto::Componente {
                nova,
                componente: Componente {
                    modo,
                    forma: Forma::Radial(GradienteRadial {
                        centro: q,
                        raio_x: 0.005,
                        raio_y: 0.005,
                        angulo: 0.0,
                        feather,
                        fora: false,
                    }),
                },
            },
            Ferramenta::Laco => Gesto::Componente {
                nova,
                componente: Componente {
                    modo,
                    forma: Forma::Laco(Laco {
                        pontos: vec![q],
                        feather: feather * FEATHER_DO_LACO,
                    }),
                },
            },
            Ferramenta::Carimbo | Ferramenta::BandAid => {
                // 🔑 **Os dois acham a origem sozinhos, como no Lightroom** — e
                // ela se arrasta depois. O carimbo esperava um ⌥-clique antes,
                // à moda do Photoshop, e o clique sem ele não fazia nada
                // (achado no app real, 2026-09-26). O ⌥-clique continua valendo.
                let origem = self
                    .local
                    .origem
                    .unwrap_or_else(|| self.origem_automatica(q));
                let carimbo = Carimbo {
                    origem,
                    destino_inicial: q,
                    caminho: vec![q],
                    raio,
                    feather,
                    opacidade,
                };
                Gesto::Retoque(if ferramenta == Ferramenta::Carimbo {
                    Retoque::Clone(carimbo)
                } else {
                    Retoque::Heal(carimbo)
                })
            }
            Ferramenta::Preencher => Gesto::Retoque(Retoque::Preencher(Preenchimento {
                caminho: if self.local.preencher_por_laco {
                    Vec::new()
                } else {
                    vec![q]
                },
                raio,
                feather,
                opacidade,
                laco: if self.local.preencher_por_laco {
                    vec![q]
                } else {
                    Vec::new()
                },
            })),
        });
        self.pedir_revelacao(cx);
        cx.notify();
    }

    /// O ponteiro andou — com gesto, ele cresce; sem, só o cursor muda.
    pub(super) fn local_mover(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let p = self.ponto_na_area(e.position);
        self.local.cursor = Some(p);
        let Some(q) = self.foto_do_ponto(p) else {
            cx.notify();
            return;
        };
        let lado = self.lado_da_foto();
        let (w, h) = self.tamanho_da_copia().unwrap_or((1.0, 1.0));
        let Some(gesto) = self.local.gesto.as_mut() else {
            cx.notify();
            return;
        };
        if let Some(ferramenta) = self.local.ferramenta {
            crate::desempenho::operacao(ferramenta.operacao_de_desempenho());
        }
        // 🚨 **Ponto repetido não entra** (achado no app real, 2026-09-26): o
        // arrasto é ouvido pela janela e pelo palco, e a mesma posição chegava
        // várias vezes — um traço de quatro movimentos saía com trinta pontos,
        // que a revelação grava e a GPU desenha. A tolerância só pega o idêntico
        // (0,06 px numa foto de 6000 px).
        let passo = 1e-5;
        let andou = |ultimo: Option<[f32; 2]>| {
            ultimo.is_none_or(|u| (u[0] - q[0]).abs() > passo || (u[1] - q[1]).abs() > passo)
        };
        let mut revelar = true;
        match gesto {
            Gesto::Componente { componente, .. } => match &mut componente.forma {
                Forma::Pincel(t) => {
                    if !andou(t.pontos.last().map(|p| [p[0], p[1]])) {
                        cx.notify();
                        return;
                    }
                    t.pontos.push([q[0], q[1], 1.0])
                }
                Forma::Linear(g) => g.fim = q,
                Forma::Radial(g) => {
                    g.raio_x = ((q[0] - g.centro[0]).abs() * w / lado).max(0.003);
                    g.raio_y = ((q[1] - g.centro[1]).abs() * h / lado).max(0.003);
                }
                Forma::Laco(l) => {
                    if !andou(l.pontos.last().copied()) {
                        cx.notify();
                        return;
                    }
                    l.pontos.push(q)
                }
            },
            Gesto::Retoque(Retoque::Clone(c) | Retoque::Heal(c)) => {
                if !andou(c.caminho.last().copied()) {
                    cx.notify();
                    return;
                }
                c.caminho.push(q)
            }
            Gesto::Retoque(Retoque::Preencher(p)) => {
                let alvo = if p.laco.is_empty() {
                    &mut p.caminho
                } else {
                    &mut p.laco
                };
                if !andou(alvo.last().copied()) {
                    cx.notify();
                    return;
                }
                alvo.push(q);
                revelar = false;
            }
            Gesto::Editando {
                parte,
                de,
                original,
                valor,
                ..
            } => {
                let (dx, dy) = (q[0] - de[0], q[1] - de[1]);
                *valor = match (*parte, original.clone()) {
                    (Parte::Origem, Retoque::Clone(mut c)) => {
                        c.origem = [c.origem[0] + dx, c.origem[1] + dy];
                        Retoque::Clone(c)
                    }
                    (Parte::Origem, Retoque::Heal(mut c)) => {
                        c.origem = [c.origem[0] + dx, c.origem[1] + dy];
                        Retoque::Heal(c)
                    }
                    (Parte::Destino, r) => mover(r, dx, dy),
                    (Parte::Raio, r) => {
                        let t = r.traco();
                        let caminho: Vec<[f32; 2]> =
                            t.pontos.iter().map(|a| [a[0], a[1]]).collect();
                        let px = |a: [f32; 2]| [a[0] * w, a[1] * h];
                        let d = caminho
                            .iter()
                            .map(|a| {
                                let (pa, pq) = (px(*a), px(q));
                                (pa[0] - pq[0]).hypot(pa[1] - pq[1])
                            })
                            .fold(f32::MAX, f32::min);
                        com_raio(
                            &r,
                            (d / lado).clamp(RAIO_MINIMO_DA_FERRAMENTA, RAIO_MAXIMO_DO_RETOQUE),
                        )
                    }
                    (Parte::Escala, Retoque::Preencher(mut p)) => {
                        let n = p.laco.len() as f32;
                        let c = [
                            p.laco.iter().map(|a| a[0]).sum::<f32>() / n,
                            p.laco.iter().map(|a| a[1]).sum::<f32>() / n,
                        ];
                        let dist = |a: [f32; 2]| ((a[0] - c[0]) * w).hypot((a[1] - c[1]) * h);
                        let f = (dist(q) / dist(*de).max(1.0)).max(0.05);
                        p.laco = p
                            .laco
                            .iter()
                            .map(|a| [c[0] + (a[0] - c[0]) * f, c[1] + (a[1] - c[1]) * f])
                            .collect();
                        Retoque::Preencher(p)
                    }
                    (_, r) => r,
                };
            }
            Gesto::Mascara {
                alca,
                de,
                original,
                valor,
                ..
            } => {
                let (dx, dy) = (q[0] - de[0], q[1] - de[1]);
                let mut novo = original.clone();
                match (&mut novo.forma, *alca) {
                    (Forma::Linear(g), AlcaDeMascara::Inicio) => g.inicio = q,
                    (Forma::Linear(g), AlcaDeMascara::Fim) => g.fim = q,
                    (Forma::Linear(g), _) => {
                        g.inicio = [g.inicio[0] + dx, g.inicio[1] + dy];
                        g.fim = [g.fim[0] + dx, g.fim[1] + dy];
                    }
                    (Forma::Radial(g), AlcaDeMascara::RaioX | AlcaDeMascara::RaioY) => {
                        // A distância ao centro, em fração do maior lado.
                        let r = ((q[0] - g.centro[0]) * w).hypot((q[1] - g.centro[1]) * h) / lado;
                        if *alca == AlcaDeMascara::RaioX {
                            g.raio_x = r.max(0.003);
                        } else {
                            g.raio_y = r.max(0.003);
                        }
                    }
                    (Forma::Radial(g), _) => {
                        g.centro = [g.centro[0] + dx, g.centro[1] + dy];
                    }
                    (Forma::Laco(l), _) => {
                        for p in &mut l.pontos {
                            *p = [p[0] + dx, p[1] + dy];
                        }
                    }
                    (Forma::Pincel(_), _) => {}
                }
                *valor = novo;
            }
        }
        if revelar {
            self.pedir_revelacao(cx);
        }
        cx.notify();
    }

    /// Fim do gesto: vira **um** passo do histórico.
    /// Larga a seleção que aponta para fora da revelação — depois de um ⌘Z ou
    /// ⌘⇧Z, a camada ou o retoque escolhido pode não existir mais (achado no
    /// app real, 2026-09-27: o traço seguinte se perdia).
    pub(super) fn conferir_a_selecao_local(&mut self) {
        if self
            .local
            .mascara_sel
            .is_some_and(|i| i >= self.locais.camadas.len())
        {
            self.local.mascara_sel = None;
        }
        if self
            .local
            .selecionado
            .is_some_and(|i| i >= self.locais.retoques.len())
        {
            self.local.selecionado = None;
        }
        if matches!(&self.local.gesto, Some(Gesto::Editando { indice, .. }) if *indice >= self.locais.retoques.len())
        {
            self.local.gesto = None;
        }
        if matches!(&self.local.gesto, Some(Gesto::Mascara { camada, .. }) if *camada >= self.locais.camadas.len())
        {
            self.local.gesto = None;
        }
    }

    pub(super) fn local_soltar(&mut self, cx: &mut Context<Self>) {
        let Some(gesto) = self.local.gesto.take() else {
            return;
        };
        let mut parametros = (*self.locais).clone();
        match gesto {
            Gesto::Mascara {
                camada,
                componente,
                original,
                valor,
                ..
            } => {
                // Clique na alça sem arrastar não vira passo.
                if valor == original {
                    self.pedir_revelacao(cx);
                    cx.notify();
                    return;
                }
                if let Some(k) = parametros
                    .camadas
                    .get_mut(camada)
                    .and_then(|c| c.componentes.get_mut(componente))
                {
                    *k = valor;
                }
            }
            Gesto::Componente { nova, componente } => {
                if let Forma::Laco(l) = &componente.forma {
                    if l.pontos.len() < 3 {
                        cx.notify();
                        return;
                    }
                }
                // 🚨 **A camada escolhida pode ter sumido** (um ⌘Z que a
                // desfez): o traço vira máscara nova em vez de se perder.
                let existe = self
                    .local
                    .mascara_sel
                    .is_some_and(|i| i < parametros.camadas.len());
                if nova || !existe {
                    let nome = nome_livre(&parametros);
                    parametros.camadas.push(Camada {
                        nome,
                        componentes: vec![componente],
                        ajustes: AjustesLocais {
                            exposicao_ev: self.exposicao_da_nova(),
                        },
                        ..Default::default()
                    });
                    self.local.mascara_sel = Some(parametros.camadas.len() - 1);
                    self.local.criando = false;
                } else if let Some(c) = self
                    .local
                    .mascara_sel
                    .and_then(|i| parametros.camadas.get_mut(i))
                {
                    c.componentes.push(componente);
                }
            }
            Gesto::Retoque(r) => {
                if let Retoque::Preencher(p) = &r {
                    if !p.laco.is_empty() && p.laco.len() < 3 {
                        cx.notify();
                        return;
                    }
                }
                let com_origem = r.carimbo().is_some();
                parametros.retoques.push(r);
                if com_origem {
                    self.local.selecionado = Some(parametros.retoques.len() - 1);
                }
            }
            Gesto::Editando {
                indice,
                original,
                valor,
                ..
            } => {
                if valor == original {
                    cx.notify();
                    return;
                }
                parametros.retoques[indice] = valor;
            }
        }
        self.comprometer(parametros, cx);
    }

    fn laco_pronto(
        &mut self,
        ferramenta: Ferramenta,
        pontos: Vec<[f32; 2]>,
        cx: &mut Context<Self>,
    ) {
        if pontos.len() < 3 {
            return;
        }
        let feather = self.local.feather * FEATHER_DO_LACO;
        let mut parametros = (*self.locais).clone();
        if ferramenta == Ferramenta::Preencher {
            parametros.retoques.push(Retoque::Preencher(Preenchimento {
                caminho: Vec::new(),
                raio: 0.01,
                feather,
                opacidade: self.local.opacidade,
                laco: pontos,
            }));
        } else {
            let modo = if self.local.subtrair {
                Modo::Subtrair
            } else {
                Modo::Somar
            };
            let componente = Componente {
                modo,
                forma: Forma::Laco(Laco { pontos, feather }),
            };
            match self.local.mascara_sel.filter(|_| !self.local.criando) {
                Some(i) if i < parametros.camadas.len() => {
                    parametros.camadas[i].componentes.push(componente)
                }
                _ => {
                    let nome = nome_livre(&parametros);
                    parametros.camadas.push(Camada {
                        nome,
                        componentes: vec![componente],
                        ajustes: AjustesLocais {
                            exposicao_ev: self.exposicao_da_nova(),
                        },
                        ..Default::default()
                    });
                    self.local.mascara_sel = Some(parametros.camadas.len() - 1);
                    self.local.criando = false;
                }
            }
        }
        self.comprometer(parametros, cx);
    }

    fn sincronizar_sliders_com_o_selecionado(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(r) = self
            .local
            .selecionado
            .and_then(|i| self.locais.retoques.get(i))
        else {
            return;
        };
        let t = r.traco();
        let feather = match r {
            Retoque::Preencher(p) if !p.laco.is_empty() => p.feather / FEATHER_DO_LACO,
            _ => t.feather,
        };
        let (raio, opacidade) = (t.raio, r.opacidade());
        // A ferramenta passa a ter o tamanho do retoque como ele aparece agora.
        let tamanho = self
            .raio_na_tela(raio)
            .clamp(TAMANHO_MINIMO, TAMANHO_MAXIMO);
        self.local.tamanho = tamanho;
        self.local.feather = feather;
        self.local.opacidade = opacidade;
        for (s, v) in [
            (self.local.sliders.raio.clone(), posicao_do_tamanho(tamanho)),
            (self.local.sliders.feather.clone(), feather),
            (self.local.sliders.opacidade.clone(), opacidade),
        ] {
            s.update(cx, |s, cx| s.set_value(v, window, cx));
        }
    }

    fn revelacao_travada(&self) -> bool {
        self.aberta
            .as_ref()
            .is_some_and(|a| a.foto.revelacao_travada)
            || self.locais_ilegiveis.is_some()
    }

    // ------------------------------------------------------------- marcações

    fn marcas(&self) -> Vec<Marca> {
        let mut marcas = Vec::new();
        // O laço em curso — o poligonal com o elástico até o cursor.
        let em_curso: Option<(Vec<[f32; 2]>, bool)> = if !self.local.poligono.is_empty() {
            Some((self.local.poligono.clone(), false))
        } else {
            match &self.local.gesto {
                Some(Gesto::Componente {
                    componente:
                        Componente {
                            forma: Forma::Laco(l),
                            ..
                        },
                    ..
                }) => Some((l.pontos.clone(), true)),
                Some(Gesto::Retoque(Retoque::Preencher(p))) if !p.laco.is_empty() => {
                    Some((p.laco.clone(), true))
                }
                Some(Gesto::Retoque(Retoque::Preencher(p))) => {
                    let pontos: Option<Vec<Ponto>> =
                        p.caminho.iter().map(|q| self.ponto_da_foto(*q)).collect();
                    if let Some(pontos) = pontos {
                        marcas.push(Marca::Caminho {
                            pontos,
                            fechado: false,
                            cor: gpui_kit::rgba(0xe0a24a88).into(),
                            tracejado: false,
                            largura: (2.0 * self.raio_na_tela(p.raio)).max(1.0),
                        });
                    }
                    None
                }
                _ => None,
            }
        };
        if let Some((pontos, fechado)) = em_curso {
            let mut tela: Vec<Ponto> = pontos
                .iter()
                .filter_map(|q| self.ponto_da_foto(*q))
                .collect();
            if !fechado {
                if let Some(c) = self.local.cursor {
                    tela.push(c);
                }
                if let Some(primeiro) = tela.first().copied() {
                    marcas.push(Marca::Alca {
                        centro: primeiro,
                        ativa: self.local.poligono.len() >= 3,
                    });
                }
            }
            marcas.push(Marca::Caminho {
                pontos: tela,
                fechado,
                cor: branco(),
                tracejado: true,
                largura: 1.0,
            });
        }

        let editando = match &self.local.gesto {
            Some(Gesto::Editando { indice, valor, .. }) => Some((*indice, valor.clone())),
            _ => None,
        };
        let retoque = |k: usize| -> Retoque {
            match &editando {
                Some((i, v)) if *i == k => v.clone(),
                _ => self.locais.retoques[k].clone(),
            }
        };
        let mostrar = self.local.marcacoes || editando.is_some();
        if mostrar && self.local.ferramenta.is_some_and(|f| !f.de_mascara()) {
            for k in 0..self.locais.retoques.len() {
                if !self.local.marcacoes && editando.as_ref().map(|e| e.0) != Some(k) {
                    continue;
                }
                let r = retoque(k);
                let t = r.traco();
                let ancora = match &r {
                    Retoque::Preencher(p) if !p.laco.is_empty() => p.laco[0],
                    _ => [t.pontos[0][0], t.pontos[0][1]],
                };
                if let Some(c) = self.ponto_da_foto(ancora) {
                    marcas.push(Marca::Alfinete {
                        centro: c,
                        ativo: Some(k) == self.local.selecionado,
                    });
                }
            }
        }
        // O selecionado: destino (branco), origem (âmbar, tracejada), seta e
        // alças.
        let selecionado = self
            .local
            .selecionado
            .filter(|&k| k < self.locais.retoques.len());
        let provisorio = match &self.local.gesto {
            Some(Gesto::Retoque(r)) if r.carimbo().is_some() => Some(r.clone()),
            _ => None,
        };
        let alvo = selecionado.map(retoque).or(provisorio);
        if let (Some(r), true) = (&alvo, mostrar) {
            match r {
                Retoque::Preencher(p) if !p.laco.is_empty() => {
                    let pontos: Vec<Ponto> = p
                        .laco
                        .iter()
                        .filter_map(|q| self.ponto_da_foto(*q))
                        .collect();
                    marcas.push(Marca::Caminho {
                        pontos,
                        fechado: true,
                        cor: branco(),
                        tracejado: false,
                        largura: 1.5,
                    });
                }
                _ => {
                    let t = r.traco();
                    self.contorno(&mut marcas, &t.pontos, t.raio, branco(), false);
                    if let Some(c) = r.carimbo() {
                        let [dx, dy] = c.deslocamento();
                        let fonte: Vec<[f32; 3]> = t
                            .pontos
                            .iter()
                            .map(|a| [a[0] + dx, a[1] + dy, 1.0])
                            .collect();
                        self.contorno(&mut marcas, &fonte, t.raio, ambar(), true);
                        if let (Some(a), Some(b)) = (
                            self.ponto_da_foto([fonte[0][0], fonte[0][1]]),
                            self.ponto_da_foto([t.pontos[0][0], t.pontos[0][1]]),
                        ) {
                            let raio = self.raio_na_tela(t.raio);
                            let ang = (b.y - a.y).atan2(b.x - a.x);
                            let (s, e) = (
                                Ponto {
                                    x: a.x + ang.cos() * raio,
                                    y: a.y + ang.sin() * raio,
                                },
                                Ponto {
                                    x: b.x - ang.cos() * raio,
                                    y: b.y - ang.sin() * raio,
                                },
                            );
                            if (e.x - s.x).hypot(e.y - s.y) > 8.0 {
                                marcas.push(Marca::Caminho {
                                    pontos: vec![s, e],
                                    fechado: false,
                                    cor: branco(),
                                    tracejado: false,
                                    largura: 1.5,
                                });
                                for d in [-0.4f32, 0.4] {
                                    marcas.push(Marca::Caminho {
                                        pontos: vec![
                                            e,
                                            Ponto {
                                                x: e.x - 8.0 * (ang + d).cos(),
                                                y: e.y - 8.0 * (ang + d).sin(),
                                            },
                                        ],
                                        fechado: false,
                                        cor: branco(),
                                        tracejado: false,
                                        largura: 1.5,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            if selecionado.is_some() && provisorio_ausente(&self.local.gesto) {
                for (centro, _) in self.alcas(r) {
                    marcas.push(Marca::Alca {
                        centro,
                        ativa: false,
                    });
                }
            }
        } else if let (Some(o), Some(f)) = (self.local.origem, self.local.ferramenta) {
            if matches!(f, Ferramenta::Carimbo | Ferramenta::BandAid) && self.local.marcacoes {
                if let Some(c) = self.ponto_da_foto(o) {
                    marcas.push(Marca::Circulo {
                        centro: c,
                        raio: self.raio_na_tela(self.raio_da_ferramenta()),
                        cor: ambar(),
                        tracejado: true,
                    });
                }
            }
        }

        // A máscara selecionada: o contorno de cada componente e as alças,
        // com uma ferramenta de máscara na mão (o `H` esconde; arrastando,
        // o que se arrasta continua).
        let arrastando_mascara = matches!(self.local.gesto, Some(Gesto::Mascara { .. }));
        if let (Some(camada), true) = (
            self.local.mascara_sel,
            (self.local.marcacoes || arrastando_mascara)
                && self.local.ferramenta.is_some_and(Ferramenta::de_mascara),
        ) {
            let n = self
                .locais
                .camadas
                .get(camada)
                .map_or(0, |c| c.componentes.len());
            for k in 0..n {
                let Some(comp) = self.componente_na_tela(camada, k) else {
                    continue;
                };
                // O linear são três retas perpendiculares à direção, como no
                // Lightroom: onde vale 100%, o meio e onde chega a 0%. A reta
                // na direção não dizia onde a transição acontece.
                if let Forma::Linear(g) = &comp.forma {
                    let (w, h) = self.tamanho_da_copia().unwrap_or((1.0, 1.0));
                    let d = [(g.fim[0] - g.inicio[0]) * w, (g.fim[1] - g.inicio[1]) * h];
                    let n = d[0].hypot(d[1]).max(1e-6);
                    // A perpendicular, do tamanho de duas diagonais da foto.
                    let alcance = 2.0 * w.hypot(h);
                    let perp = [-d[1] / n * alcance / w, d[0] / n * alcance / h];
                    let meio = [
                        (g.inicio[0] + g.fim[0]) / 2.0,
                        (g.inicio[1] + g.fim[1]) / 2.0,
                    ];
                    for (c, tracejada) in [(g.inicio, false), (meio, true), (g.fim, false)] {
                        let Some((a, b)) = cortar_na_foto(
                            [c[0] - perp[0], c[1] - perp[1]],
                            [c[0] + perp[0], c[1] + perp[1]],
                        ) else {
                            continue;
                        };
                        let pontos: Vec<Ponto> = [a, b]
                            .iter()
                            .filter_map(|q| self.ponto_da_foto(*q))
                            .collect();
                        if pontos.len() == 2 {
                            marcas.push(Marca::Caminho {
                                pontos,
                                fechado: false,
                                cor: branco(),
                                tracejado: tracejada,
                                largura: 1.0,
                            });
                        }
                    }
                }
                let contorno: Vec<[f32; 2]> = match &comp.forma {
                    Forma::Linear(_) => Vec::new(),
                    Forma::Radial(g) => {
                        let (w, h) = self.tamanho_da_copia().unwrap_or((1.0, 1.0));
                        let lado = self.lado_da_foto();
                        let (s, c) = g.angulo.to_radians().sin_cos();
                        (0..=48)
                            .map(|i| {
                                let t = i as f32 / 48.0 * std::f32::consts::TAU;
                                let (u, v) = (g.raio_x * lado * t.cos(), g.raio_y * lado * t.sin());
                                [
                                    g.centro[0] + (u * c - v * s) / w,
                                    g.centro[1] + (u * s + v * c) / h,
                                ]
                            })
                            .collect()
                    }
                    Forma::Laco(l) => l.pontos.clone(),
                    Forma::Pincel(_) => Vec::new(),
                };
                let pontos: Vec<Ponto> = contorno
                    .iter()
                    .filter_map(|q| self.ponto_da_foto(*q))
                    .collect();
                if pontos.len() >= 2 {
                    marcas.push(Marca::Caminho {
                        pontos,
                        fechado: matches!(comp.forma, Forma::Laco(_)),
                        cor: branco(),
                        tracejado: true,
                        largura: 1.0,
                    });
                }
                let ativa = |alca: AlcaDeMascara| {
                    matches!(&self.local.gesto,
                        Some(Gesto::Mascara { camada: c, componente, alca: a, .. })
                            if (*c, *componente, *a) == (camada, k, alca))
                };
                for (q, alca) in self.alcas_do_componente(&comp) {
                    if let Some(centro) = self.ponto_da_foto(q) {
                        marcas.push(Marca::Alca {
                            centro,
                            ativa: ativa(alca),
                        });
                    }
                }
            }
        }

        // O cursor: o tamanho e, dentro, onde a máscara ainda vale 100% — a
        // suavização entre os dois, como no Lightroom.
        if let (Some(c), Some(f)) = (self.local.cursor, self.local.ferramenta) {
            if f.circular()
                && !(f == Ferramenta::Preencher && self.local.preencher_por_laco)
                && editando.is_none()
            {
                let raio = self.raio_na_tela(self.raio_da_ferramenta());
                marcas.push(Marca::Circulo {
                    centro: c,
                    raio,
                    cor: branco(),
                    tracejado: false,
                });
                let interno = raio * (1.0 - self.local.feather);
                if self.local.feather > 0.02 && interno > 1.0 {
                    marcas.push(Marca::Circulo {
                        centro: c,
                        raio: interno,
                        cor: gpui_kit::rgba(0xffffffbf).into(),
                        tracejado: true,
                    });
                }
            }
        }
        marcas
    }

    /// O contorno de um caminho com raio: os círculos das pontas e a faixa.
    fn contorno(
        &self,
        marcas: &mut Vec<Marca>,
        pontos: &[[f32; 3]],
        raio: f32,
        cor: Hsla,
        tracejado: bool,
    ) {
        let tela: Vec<Ponto> = pontos
            .iter()
            .filter_map(|a| self.ponto_da_foto([a[0], a[1]]))
            .collect();
        marcas.extend(marcas_do_contorno(
            &tela,
            self.raio_na_tela(raio),
            cor,
            tracejado,
        ));
    }

    /// As marcações por cima da foto, e o arrasto que continua fora dela.
    pub(super) fn marcacoes_locais(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.local.ferramenta?;
        let marcas = self.marcas();
        let arrastando = self.local.arrastando();
        let ouvinte = cx.entity();
        Some(
            canvas(
                move |_bounds, _window, _cx| {},
                move |bounds, _, window, _cx| {
                    let origem = bounds.origin;
                    let em = |p: Ponto| -> Point<Pixels> {
                        point(origem.x + px(p.x), origem.y + px(p.y))
                    };
                    for marca in &marcas {
                        pintar(marca, &em, window);
                    }
                    if arrastando {
                        window.on_mouse_event({
                            let esta = ouvinte.clone();
                            move |evento: &MouseMoveEvent, fase, _window, cx| {
                                if fase.bubble() {
                                    esta.update(cx, |tela, cx| tela.local_mover(evento, cx));
                                }
                            }
                        });
                        window.on_mouse_event({
                            let esta = ouvinte.clone();
                            move |_evento: &MouseUpEvent, fase, _window, cx| {
                                if fase.bubble() {
                                    esta.update(cx, |tela, cx| tela.local_soltar(cx));
                                }
                            }
                        });
                    }
                },
            )
            .absolute()
            .size_full()
            .into_any_element(),
        )
    }

    // ----------------------------------------------------------------- painel

    /// A seção "Revelação local" do painel da direita.
    pub(super) fn painel_local(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let travada = self.revelacao_travada();
        let ferramenta = self.local.ferramenta;

        let botao_da_ferramenta = |f: Ferramenta, cx: &mut Context<Self>| {
            Button::new(SharedString::from(format!("local-{}", f.nome())))
                .icon(Icon::new(f.icone()).size(px(16.)))
                .small()
                .ghost()
                .selected(ferramenta == Some(f))
                .disabled(travada)
                .tooltip(SharedString::from(format!("{} ({})", f.nome(), f.atalho())))
                .on_click(cx.listener(move |tela, _e, _w, cx| tela.usar_ferramenta(f, cx)))
        };

        let mut barra = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(3.))
            .rounded(crate::tema::canto(8.))
            .bg(tema.muted);
        for f in Ferramenta::MASCARA {
            barra = barra.child(botao_da_ferramenta(f, cx));
        }
        barra = barra.child(div().w(px(1.)).h(px(18.)).mx(px(3.)).bg(tema.border));
        for f in Ferramenta::RETOQUE {
            barra = barra.child(botao_da_ferramenta(f, cx));
        }
        let ampliada = self.navegacao.zoom.nivel != crate::revelacao::zoom::Nivel::Encaixar;
        barra = barra
            .child(div().flex_1())
            // O zoom da prévia: laço, carimbo e band-aid pedem a foto de perto.
            // A barra completa (níveis, slider, navegador) fica no palco.
            .child(
                Button::new("local-zoom")
                    .icon(Icon::new(Icone::ZoomIn).size(px(15.)))
                    .small()
                    .ghost()
                    .selected(ampliada)
                    .tooltip(
                        "Alternar zoom: encaixar ↔ último nível (Z · Espaço; pinça no trackpad)",
                    )
                    .on_click(cx.listener(|tela, _e, _w, cx| tela.alternar_zoom(None, cx))),
            )
            .child(
                Button::new("local-marcacoes")
                    .icon(Icon::new(Icone::MapPin).size(px(15.)))
                    .small()
                    .ghost()
                    .selected(self.local.marcacoes)
                    .debug_selector(|| "local-marcacoes".into())
                    .tooltip(if self.local.marcacoes {
                        "Esconder marcações (H)"
                    } else {
                        "Mostrar marcações (H)"
                    })
                    .on_click(cx.listener(|tela, _e, _w, cx| tela.alternar_marcacoes(cx))),
            )
            .child(
                Button::new("local-sair")
                    .icon(Icon::new(Icone::LogOut).size(px(15.)))
                    .small()
                    .ghost()
                    .disabled(!self.dentro_da_revelacao_local())
                    .debug_selector(|| "local-sair".into())
                    .tooltip("Sair da Revelação local (Esc)")
                    .on_click(cx.listener(|tela, _e, _w, cx| tela.sair_da_revelacao_local(cx))),
            );

        // 🔑 **Sanfona, como os outros painéis da coluna**: a mesma chave de
        // lembrança (`revelacao:<título>`) e o ponto dos outros: âmbar se a
        // Revelação local mudou e não foi salva, cinza se a foto já tem máscara
        // ou retoque salvos — fechado, o painel não pode esconder que há.
        let chave = CHAVE_DO_PAINEL_LOCAL.to_string();
        let aberto = self.estado_do_painel.aberto(&chave, false);
        let cabecalho = self.cabecalho_da_sanfona(
            "Revelação local",
            aberto,
            self.marca_da_revelacao_local(),
            chave,
            false,
            cx,
        );
        let caixa = div()
            .flex()
            .flex_col()
            .flex_none()
            .rounded(crate::tema::canto(6.))
            .border_1()
            .border_color(tema.border)
            .child(cabecalho);
        if !aberto {
            return caixa.into_any_element();
        }

        let mut secao = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(12.))
            .border_t_1()
            .border_color(tema.border)
            .child(barra);
        if let Some(erro) = &self.locais_ilegiveis {
            secao = secao.child(
                div()
                    .text_xs()
                    .text_color(tema.warning)
                    .child(SharedString::from(format!(
                        "A Revelação local desta foto foi feita numa versão mais nova do app e não pode ser alterada aqui ({erro})."
                    ))),
            );
        }
        if let Some(f) = ferramenta {
            secao = secao.child(self.opcoes_da_ferramenta(f, cx));
        }
        secao = secao.child(self.lista_de_mascaras(cx));
        secao = secao.child(self.lista_de_retoques(cx));
        caixa.child(secao).into_any_element()
    }

    fn dica_da_ferramenta(&self, f: Ferramenta) -> String {
        let onde = if f.de_mascara() {
            if self.local.criando || self.local.mascara_sel.is_none() {
                "Nova máscara. ".to_string()
            } else {
                let nome = self
                    .local
                    .mascara_sel
                    .and_then(|i| self.locais.camadas.get(i))
                    .map(|c| c.nome.clone())
                    .unwrap_or_default();
                format!(
                    "{nome}, {}. ",
                    if self.local.subtrair {
                        "subtraindo"
                    } else {
                        "adicionando"
                    }
                )
            }
        } else {
            String::new()
        };
        let texto = match f {
            Ferramenta::Pincel => "Arraste para pintar. ⌥ inverte o modo.",
            Ferramenta::Linear => "Arraste do início (100%) ao fim (0%).",
            Ferramenta::Radial => "Arraste do centro para fora.",
            Ferramenta::Laco if self.local.poligonal => {
                "Clique os vértices; o primeiro ponto ou o clique duplo fecha. Esc cancela."
            }
            Ferramenta::Laco => "Arraste em volta da área; soltar fecha.",
            Ferramenta::Carimbo if self.local.selecionado.is_some() => {
                "Arraste o tracejado para mudar a amostragem, o branco para mover, as alças para o tamanho. Delete apaga."
            }
            Ferramenta::Carimbo if self.local.origem.is_some() => "Pinte o destino. ⌥ + clique troca a origem.",
            Ferramenta::Carimbo => {
                "Pinte o destino; a origem é escolhida ao lado e dá para arrastar depois. ⌥ + clique fixa uma."
            }
            Ferramenta::BandAid if self.local.selecionado.is_some() => {
                "Arraste o tracejado para mudar a amostragem, o branco para mover, as alças para o tamanho. Delete apaga."
            }
            Ferramenta::BandAid => "Pinte o defeito; a origem é escolhida ao lado e dá para arrastar depois.",
            Ferramenta::Preencher if self.local.preencher_por_laco => "Cerque o que quer tirar.",
            Ferramenta::Preencher => "Pinte o que quer tirar; é preenchido com o que está em volta.",
        };
        format!("{onde}{texto}")
    }

    fn opcoes_da_ferramenta(&self, f: Ferramenta, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let segmento = |id: &'static str,
                        rotulos: [(&'static str, bool); 2],
                        cx: &mut Context<Self>,
                        acao: fn(&mut Revelacao, bool)| {
            let mut linha = div()
                .flex()
                .gap(px(2.))
                .p(px(2.))
                .rounded(crate::tema::canto(7.))
                .bg(tema.muted);
            // 🚨 **O escolhido tem de aparecer** (visto no app real,
            // 2026-09-27): o `Button` fantasma marcado ganha o mesmo fundo
            // `muted` da barra, e nenhum dos dois parecia escolhido. Desenho
            // das abas sRGB/RGB: fundo e texto claros no escolhido.
            for (k, (rotulo, ativo)) in rotulos.into_iter().enumerate() {
                linha = linha.child(
                    div()
                        .id(SharedString::from(format!("{id}-{k}")))
                        .flex()
                        .flex_1()
                        .justify_center()
                        .py(px(3.))
                        .rounded(crate::tema::canto(5.))
                        .text_xs()
                        .cursor_pointer()
                        .when(ativo, |d| {
                            d.bg(tema.background)
                                .border_1()
                                .border_color(tema.border)
                                .text_color(tema.foreground)
                                .font_weight(gpui_kit::FontWeight::MEDIUM)
                        })
                        .when(!ativo, |d| {
                            d.text_color(tema.muted_foreground)
                                .hover(|d| d.text_color(tema.foreground))
                        })
                        .child(rotulo)
                        .on_click(cx.listener(move |tela, _e, _w, cx| {
                            acao(tela, k == 1);
                            cx.notify();
                        })),
                );
            }
            linha
        };
        let mut caixa = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(10.))
            .rounded(crate::tema::canto(8.))
            .border_1()
            .border_color(tema.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_sm()
                    .child(Icon::new(f.icone()).size(px(14.)))
                    .child(f.nome()),
            );
        if f.de_mascara() && !self.local.criando && self.local.mascara_sel.is_some() {
            caixa = caixa.child(segmento(
                "local-modo",
                [
                    ("Adicionar", !self.local.subtrair),
                    ("Subtrair", self.local.subtrair),
                ],
                cx,
                |tela, sub| tela.local.subtrair = sub,
            ));
        }
        if f == Ferramenta::Preencher {
            caixa = caixa.child(segmento(
                "local-area",
                [
                    ("Pintar", !self.local.preencher_por_laco),
                    ("Cercar", self.local.preencher_por_laco),
                ],
                cx,
                |tela, laco| tela.local.preencher_por_laco = laco,
            ));
        }
        if f.usa_laco() || (f == Ferramenta::Preencher && self.local.preencher_por_laco) {
            caixa = caixa.child(segmento(
                "local-laco",
                [
                    ("Livre", !self.local.poligonal),
                    ("Poligonal", self.local.poligonal),
                ],
                cx,
                |tela, poligonal| {
                    tela.local.poligonal = poligonal;
                    tela.local.poligono.clear();
                },
            ));
        }
        let linha = |rotulo: &'static str, valor: String, estado: &Entity<SliderState>| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_xs()
                .child(
                    div()
                        .w(px(76.))
                        .text_color(tema.muted_foreground)
                        .child(rotulo),
                )
                .child(div().flex_1().child(Slider::new(estado).horizontal()))
                .child(
                    div()
                        .w(px(42.))
                        .flex()
                        .justify_end()
                        .child(SharedString::from(valor)),
                )
        };
        if f.circular() && !(f == Ferramenta::Preencher && self.local.preencher_por_laco) {
            caixa = caixa.child(linha(
                "Tamanho",
                rotulo_do_tamanho(self.diametro_da_ferramenta()),
                &self.local.sliders.raio,
            ));
        }
        if f != Ferramenta::Linear {
            caixa = caixa.child(linha(
                "Suavização",
                format!("{}%", (self.local.feather * 100.0).round()),
                &self.local.sliders.feather,
            ));
        }
        if !matches!(
            f,
            Ferramenta::Linear | Ferramenta::Radial | Ferramenta::Laco
        ) {
            caixa = caixa.child(linha(
                if f == Ferramenta::Pincel {
                    "Fluxo"
                } else {
                    "Opacidade"
                },
                format!("{}%", (self.local.opacidade * 100.0).round()),
                &self.local.sliders.opacidade,
            ));
        }
        caixa
            .child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(SharedString::from(self.dica_da_ferramenta(f))),
            )
            .into_any_element()
    }

    fn lista_de_mascaras(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let mut novas = div().flex().gap(px(2.));
        for f in Ferramenta::MASCARA {
            novas = novas.child(
                Button::new(SharedString::from(format!("nova-mascara-{}", f.nome())))
                    .icon(Icon::new(f.icone()).size(px(14.)))
                    .xsmall()
                    .ghost()
                    .tooltip(SharedString::from(format!(
                        "Nova máscara com {}",
                        f.nome().to_lowercase()
                    )))
                    .on_click(cx.listener(move |tela, _e, _w, cx| tela.nova_mascara_com(f, cx))),
            );
        }
        let cabecalho = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child(Icon::new(Icone::Layers).size(px(13.)))
                    .child("Máscaras"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(Icon::new(Icone::Plus).size(px(12.)))
                    .child(novas),
            );
        let mut lista = div().flex().flex_col().gap(px(3.)).child(cabecalho);
        if self.locais.camadas.is_empty() {
            lista = lista.child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .child("Nenhuma máscara. Use + ou uma ferramenta de máscara."),
            );
        }
        for (i, camada) in self.locais.camadas.iter().enumerate() {
            let sel = self.local.mascara_sel == Some(i);
            let nome = if camada.nome.is_empty() {
                format!("Máscara {}", i + 1)
            } else {
                camada.nome.clone()
            };
            let ev = camada.ajustes.exposicao_ev;
            let linha =
                div()
                    .id(SharedString::from(format!("mascara-{i}")))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(6.))
                    .py(px(4.))
                    .rounded(crate::tema::canto(6.))
                    .border_1()
                    .border_color(if sel {
                        gpui_kit::rgb(0xe0a24a).into()
                    } else {
                        tema.border
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |tela, _e, window, cx| {
                        tela.selecionar_mascara(i, window, cx)
                    }))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .text_xs()
                            .child(
                                div()
                                    .text_color(if camada.visivel {
                                        tema.foreground
                                    } else {
                                        tema.muted_foreground
                                    })
                                    .child(SharedString::from(nome)),
                            )
                            .child(div().text_color(tema.muted_foreground).child(
                                SharedString::from(format!(
                                    "{}{:.2} EV{}",
                                    if ev > 0.0 { "+" } else { "" },
                                    ev,
                                    if camada.invertida {
                                        " · invertida"
                                    } else {
                                        ""
                                    }
                                )),
                            )),
                    )
                    .child(
                        Button::new(SharedString::from(format!("ver-{i}")))
                            .icon(
                                Icon::new(if camada.visivel {
                                    Icone::Eye
                                } else {
                                    Icone::EyeOff
                                })
                                .size(px(13.)),
                            )
                            .xsmall()
                            .ghost()
                            .tooltip(if camada.visivel { "Ocultar" } else { "Mostrar" })
                            .on_click(cx.listener(move |tela, _e, _w, cx| {
                                // Dentro da linha: o clique não pode chegar a ela,
                                // que alternaria a seleção da máscara.
                                cx.stop_propagation();
                                tela.mudar_mascara(i, cx, |c| {
                                    c.visivel = !c.visivel;
                                    false
                                })
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("inverter-{i}")))
                            .icon(Icon::new(Icone::Contrast).size(px(13.)))
                            .xsmall()
                            .ghost()
                            .selected(camada.invertida)
                            .tooltip("Inverter")
                            .on_click(cx.listener(move |tela, _e, _w, cx| {
                                // Dentro da linha: o clique não pode chegar a ela,
                                // que alternaria a seleção da máscara.
                                cx.stop_propagation();
                                tela.mudar_mascara(i, cx, |c| {
                                    c.invertida = !c.invertida;
                                    false
                                })
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("excluir-{i}")))
                            .icon(Icon::new(Icone::Trash2).size(px(13.)))
                            .xsmall()
                            .ghost()
                            .tooltip("Excluir máscara")
                            .on_click(cx.listener(move |tela, _e, _w, cx| {
                                // Dentro da linha: o clique não pode chegar a ela,
                                // que alternaria a seleção da máscara.
                                cx.stop_propagation();
                                tela.mudar_mascara(i, cx, |_| true)
                            })),
                    );
            lista = lista.child(linha);
            if sel {
                let mut comps = div().flex().flex_col().gap(px(2.)).pl(px(12.));
                let mut contagem: std::collections::HashMap<&str, usize> = Default::default();
                for (k, c) in camada.componentes.iter().enumerate() {
                    let (tipo, icone) = match c.forma {
                        Forma::Pincel(_) => ("Pincel", Icone::Paintbrush),
                        Forma::Linear(_) => ("Linear", Icone::RectangleHorizontal),
                        Forma::Radial(_) => ("Radial", Icone::CircleDashed),
                        Forma::Laco(_) => ("Laço", Icone::Lasso),
                    };
                    let n = contagem.entry(tipo).or_default();
                    *n += 1;
                    let menos = c.modo == Modo::Subtrair;
                    comps = comps.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_xs()
                            .text_color(if menos {
                                tema.danger
                            } else {
                                tema.muted_foreground
                            })
                            .child(Icon::new(icone).size(px(12.)))
                            .child(div().flex_1().child(SharedString::from(format!(
                                "{tipo} {n}{}",
                                if menos { " · subtrair" } else { "" }
                            ))))
                            .child(
                                Button::new(SharedString::from(format!("tirar-{i}-{k}")))
                                    .icon(Icon::new(Icone::X).size(px(11.)))
                                    .xsmall()
                                    .ghost()
                                    .tooltip("Tirar este componente")
                                    .on_click(cx.listener(move |tela, _e, _w, cx| {
                                        tela.mudar_mascara(i, cx, |c| {
                                            if k < c.componentes.len() {
                                                c.componentes.remove(k);
                                            }
                                            c.componentes.is_empty()
                                        })
                                    })),
                            ),
                    );
                }
                // A exposição logo abaixo da linha, antes dos componentes: é o
                // controle da máscara, e no pé de uma lista longa ele ficava
                // fora da coluna (visto no app real, 2026-09-27).
                lista = lista
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .pl(px(12.))
                            .text_xs()
                            .child(
                                div()
                                    .w(px(64.))
                                    .text_color(tema.muted_foreground)
                                    .child("Exposição"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .child(Slider::new(&self.local.sliders.exposicao).horizontal()),
                            )
                            .child(div().w(px(42.)).flex().justify_end().child(
                                SharedString::from(format!(
                                    "{}{ev:.2}",
                                    if ev > 0.0 { "+" } else { "" }
                                )),
                            )),
                    )
                    .child(comps);
            }
        }
        lista.into_any_element()
    }

    fn lista_de_retoques(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let mut lista = div().flex().flex_col().gap(px(3.)).child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_xs()
                .text_color(tema.muted_foreground)
                .child(Icon::new(Icone::Stamp).size(px(13.)))
                .child("Retoque"),
        );
        if self.locais.retoques.is_empty() {
            return lista
                .child(
                    div()
                        .text_xs()
                        .text_color(tema.muted_foreground)
                        .child("Nenhum retoque."),
                )
                .into_any_element();
        }
        let mut contagem: std::collections::HashMap<&str, usize> = Default::default();
        for (k, r) in self.locais.retoques.iter().enumerate() {
            let (nome, icone, ferramenta) = match r {
                Retoque::Clone(_) => ("Carimbo", Icone::Stamp, Ferramenta::Carimbo),
                Retoque::Heal(_) => ("Band-aid", Icone::Bandage, Ferramenta::BandAid),
                Retoque::Preencher(_) => ("Content-Aware", Icone::Sparkles, Ferramenta::Preencher),
            };
            let n = contagem.entry(nome).or_default();
            *n += 1;
            let sel = self.local.selecionado == Some(k);
            let detalhe = match r {
                Retoque::Preencher(p) if !p.laco.is_empty() => "laço",
                Retoque::Preencher(_) => "pincel",
                _ => "com origem",
            };
            lista = lista.child(
                div()
                    .id(SharedString::from(format!("retoque-{k}")))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(6.))
                    .py(px(4.))
                    .rounded(crate::tema::canto(6.))
                    .border_1()
                    .border_color(if sel {
                        gpui_kit::rgb(0xe0a24a).into()
                    } else {
                        tema.border
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |tela, _e, window, cx| {
                        tela.local.selecionado = if tela.local.selecionado == Some(k) {
                            None
                        } else {
                            Some(k)
                        };
                        tela.local.ferramenta = Some(ferramenta);
                        tela.local.mascara_sel = None;
                        tela.local.criando = false;
                        tela.sincronizar_sliders_com_o_selecionado(window, cx);
                        cx.notify();
                    }))
                    .child(Icon::new(icone).size(px(13.)))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .text_xs()
                            .child(SharedString::from(format!("{nome} {n}")))
                            .child(div().text_color(tema.muted_foreground).child(detalhe)),
                    )
                    .child(
                        Button::new(SharedString::from(format!("apagar-retoque-{k}")))
                            .icon(Icon::new(Icone::Trash2).size(px(13.)))
                            .xsmall()
                            .ghost()
                            .tooltip("Excluir retoque")
                            .on_click(cx.listener(move |tela, _e, _w, cx| {
                                let mut parametros = (*tela.locais).clone();
                                if k < parametros.retoques.len() {
                                    parametros.retoques.remove(k);
                                    tela.local.selecionado = None;
                                    tela.comprometer(parametros, cx);
                                }
                            })),
                    ),
            );
        }
        lista.into_any_element()
    }
}

fn provisorio_ausente(gesto: &Option<Gesto>) -> bool {
    !matches!(gesto, Some(Gesto::Retoque(_)))
}

fn raio_do(r: &Retoque) -> f32 {
    r.traco().raio
}

fn com_raio(r: &Retoque, raio: f32) -> Retoque {
    match r.clone() {
        Retoque::Clone(mut c) => {
            c.raio = raio;
            Retoque::Clone(c)
        }
        Retoque::Heal(mut c) => {
            c.raio = raio;
            Retoque::Heal(c)
        }
        Retoque::Preencher(mut p) => {
            p.raio = raio;
            Retoque::Preencher(p)
        }
    }
}

fn com_feather(r: &Retoque, feather: f32) -> Retoque {
    match r.clone() {
        Retoque::Clone(mut c) => {
            c.feather = feather;
            Retoque::Clone(c)
        }
        Retoque::Heal(mut c) => {
            c.feather = feather;
            Retoque::Heal(c)
        }
        Retoque::Preencher(mut p) => {
            p.feather = feather;
            Retoque::Preencher(p)
        }
    }
}

fn com_opacidade(r: &Retoque, opacidade: f32) -> Retoque {
    match r.clone() {
        Retoque::Clone(mut c) => {
            c.opacidade = opacidade;
            Retoque::Clone(c)
        }
        Retoque::Heal(mut c) => {
            c.opacidade = opacidade;
            Retoque::Heal(c)
        }
        Retoque::Preencher(mut p) => {
            p.opacidade = opacidade;
            Retoque::Preencher(p)
        }
    }
}

/// O retoque inteiro andando `dx, dy` (normalizado): a origem fica.
fn mover(r: Retoque, dx: f32, dy: f32) -> Retoque {
    let andar = |a: &[f32; 2]| [a[0] + dx, a[1] + dy];
    match r {
        Retoque::Clone(mut c) => {
            c.caminho = c.caminho.iter().map(andar).collect();
            c.destino_inicial = andar(&c.destino_inicial);
            Retoque::Clone(c)
        }
        Retoque::Heal(mut c) => {
            c.caminho = c.caminho.iter().map(andar).collect();
            c.destino_inicial = andar(&c.destino_inicial);
            Retoque::Heal(c)
        }
        Retoque::Preencher(mut p) => {
            p.caminho = p.caminho.iter().map(andar).collect();
            p.laco = p.laco.iter().map(andar).collect();
            Retoque::Preencher(p)
        }
    }
}

/// Uma curva candidata do contorno: o ponto em `s` (0 a 1), as amostras, e se
/// ela fecha.
type CurvaDoContorno = (Box<dyn Fn(f32) -> Ponto>, usize, bool);

/// O contorno de um caminho com raio, em pontos da tela: **a borda** da área
/// que o retoque cobre — a união dos discos de raio `r` ao longo do caminho.
///
/// 🚨 **Só linha, nunca véu.** Até 2026-09-27 o corpo do caminho era um traço
/// de 2·`r` com 18% da cor, e só entre a primeira e a última ponta: numa
/// pincelada curta, com zoom, era um retângulo estreito e mais alto que o
/// palco — uma faixa clara atravessando a foto, que parecia defeito da
/// revelação. E os círculos das pontas eram inteiros, riscando por dentro.
///
/// Cada ponto do caminho dá um círculo, e cada trecho os dois lados; fica o
/// que está na borda (a `r` do caminho, e não mais perto), e os cortes são
/// achados por bissecção — as pontas dos pedaços se encontram sem degrau.
fn marcas_do_contorno(tela: &[Ponto], r: f32, cor: Hsla, tracejado: bool) -> Vec<Marca> {
    if r < 1.0 || tela.is_empty() {
        return Vec::new();
    }
    // Um ponto a cada 0,2·r (e sempre a última ponta): a pincelada longa tem
    // milhares de pontos, e a conta abaixo é por quadro. A borda que muda com
    // isso fica dentro do traço; o ponto repetido (mouse parado) sai também.
    let espaco = (r * 0.2).max(0.25);
    let mut caminho: Vec<Ponto> = Vec::with_capacity(tela.len());
    for (i, p) in tela.iter().enumerate() {
        let longe = |minimo: f32| {
            caminho
                .last()
                .is_none_or(|u: &Ponto| (p.x - u.x).hypot(p.y - u.y) > minimo)
        };
        if longe(espaco) || (i + 1 == tela.len() && longe(0.25)) {
            caminho.push(*p);
        }
    }
    let distancia = |q: Ponto| -> f32 {
        let segmento = |a: Ponto, b: Ponto| {
            let (abx, aby) = (b.x - a.x, b.y - a.y);
            let l2 = abx * abx + aby * aby;
            let t = if l2 > 0.0 {
                (((q.x - a.x) * abx + (q.y - a.y) * aby) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (q.x - a.x - abx * t).hypot(q.y - a.y - aby * t)
        };
        if caminho.len() == 1 {
            return segmento(caminho[0], caminho[0]);
        }
        caminho
            .windows(2)
            .map(|p| segmento(p[0], p[1]))
            .fold(f32::MAX, f32::min)
    };
    // Na borda: a folga é da conta em f32, não da tela.
    let na_borda = |q: Ponto| distancia(q) >= r - (r * 1e-4).max(0.01);

    // As curvas candidatas, em parâmetro `s` de 0 a 1: `(ponto em s, amostras,
    // fechada)`.
    let mut curvas: Vec<CurvaDoContorno> = Vec::new();
    let voltas = ((r * 0.8) as usize).clamp(24, 96);
    for &c in &caminho {
        curvas.push((
            Box::new(move |s: f32| {
                let a = s * std::f32::consts::TAU;
                Ponto {
                    x: c.x + r * a.cos(),
                    y: c.y + r * a.sin(),
                }
            }),
            voltas,
            true,
        ));
    }
    for par in caminho.windows(2) {
        let (a, b) = (par[0], par[1]);
        let l = (b.x - a.x).hypot(b.y - a.y);
        let (nx, ny) = (-(b.y - a.y) / l * r, (b.x - a.x) / l * r);
        let amostras = ((l / 4.0) as usize).clamp(1, 256);
        for lado in [1.0f32, -1.0] {
            curvas.push((
                Box::new(move |s: f32| Ponto {
                    x: a.x + (b.x - a.x) * s + lado * nx,
                    y: a.y + (b.y - a.y) * s + lado * ny,
                }),
                amostras,
                false,
            ));
        }
    }

    let mut trechos: Vec<Vec<Ponto>> = Vec::new();
    for (curva, n, fechada) in &curvas {
        let n = *n;
        let s_de = |k: usize| k as f32 / n as f32;
        let dentro: Vec<bool> = (0..=n).map(|k| na_borda(curva(s_de(k)))).collect();
        // O corte entre uma amostra que fica e uma que sai.
        let corte = |mut fica: f32, mut sai: f32| {
            for _ in 0..24 {
                let meio = (fica + sai) * 0.5;
                if na_borda(curva(meio)) {
                    fica = meio;
                } else {
                    sai = meio;
                }
            }
            curva(fica)
        };
        if *fechada && dentro.iter().all(|&d| d) {
            trechos.push((0..=n).map(|k| curva(s_de(k))).collect());
            continue;
        }
        // Fechada: começa numa amostra que sai, para nenhum pedaço ser
        // partido na costura do `s = 0`.
        let inicio = if *fechada {
            dentro.iter().position(|&d| !d).unwrap_or(0)
        } else {
            0
        };
        let passos = if *fechada { n } else { n + 1 };
        let mut atual: Vec<Ponto> = Vec::new();
        let mut anterior: Option<(usize, bool)> = None;
        for passo in 0..passos {
            let k = if *fechada {
                (inicio + passo) % n
            } else {
                passo
            };
            let d = dentro[k];
            match (anterior, d) {
                (Some((j, false)), true) => {
                    let (sj, sk) = (
                        s_de(j),
                        if *fechada && k < j {
                            s_de(k) + 1.0
                        } else {
                            s_de(k)
                        },
                    );
                    atual.push(corte(sk, sj));
                }
                (Some((j, true)), false) => {
                    let (sj, sk) = (
                        s_de(j),
                        if *fechada && k < j {
                            s_de(k) + 1.0
                        } else {
                            s_de(k)
                        },
                    );
                    atual.push(corte(sj, sk));
                    if atual.len() >= 2 {
                        trechos.push(std::mem::take(&mut atual));
                    }
                    atual.clear();
                }
                _ => {}
            }
            if d {
                atual.push(curva(s_de(k)));
            }
            anterior = Some((k, d));
        }
        if *fechada {
            // Volta ao começo, que sai: fecha o último pedaço.
            if let Some((j, true)) = anterior {
                atual.push(corte(
                    s_de(j),
                    s_de(inicio) + if inicio <= j { 1.0 } else { 0.0 },
                ));
            }
        }
        if atual.len() >= 2 {
            trechos.push(atual);
        }
    }
    vec![Marca::Contorno {
        trechos,
        cor,
        tracejado,
    }]
}

/// Uma marca na janela: o traço escuro por baixo (para aparecer sobre foto
/// clara) e o da cor por cima.
fn pintar(marca: &Marca, em: &dyn Fn(Ponto) -> Point<Pixels>, window: &mut Window) {
    let traco = |pontos: &[Point<Pixels>],
                 fechado: bool,
                 largura: f32,
                 cor: Hsla,
                 tracejado: bool,
                 window: &mut Window| {
        if pontos.len() < 2 {
            return;
        }
        let mut b = PathBuilder::stroke(px(largura));
        if tracejado {
            b = b.dash_array(&[px(5.), px(4.)]);
        }
        b.move_to(pontos[0]);
        for p in &pontos[1..] {
            b.line_to(*p);
        }
        if fechado {
            b.close();
        }
        if let Ok(caminho) = b.build() {
            window.paint_path(caminho, cor);
        }
    };
    let circulo = |centro: Ponto, raio: f32| -> Vec<Point<Pixels>> {
        let n = ((raio * 0.8) as usize).clamp(24, 96);
        (0..=n)
            .map(|k| {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                em(Ponto {
                    x: centro.x + raio * a.cos(),
                    y: centro.y + raio * a.sin(),
                })
            })
            .collect()
    };
    match marca {
        Marca::Circulo {
            centro,
            raio,
            cor,
            tracejado,
        } => {
            if *raio < 1.0 {
                return;
            }
            let pontos = circulo(*centro, *raio);
            // A sombra só separa o traço da foto clara. Grossa e contínua sob
            // o tracejado, ela virava uma faixa preta dentro do pincel (visto
            // no app, 2026-09-26): no tracejado ela segue os traços.
            traco(&pontos, false, 2.2, sombra_leve(), *tracejado, window);
            traco(&pontos, false, 1.2, *cor, *tracejado, window);
        }
        Marca::Caminho {
            pontos,
            fechado,
            cor,
            tracejado,
            largura,
        } => {
            let na_janela: Vec<Point<Pixels>> = pontos.iter().map(|p| em(*p)).collect();
            if *largura <= 2.0 {
                traco(&na_janela, *fechado, largura + 2.0, sombra(), false, window);
            }
            traco(&na_janela, *fechado, *largura, *cor, *tracejado, window);
        }
        Marca::Contorno {
            trechos,
            cor,
            tracejado,
        } => {
            for trecho in trechos {
                let pontos: Vec<Point<Pixels>> = trecho.iter().map(|p| em(*p)).collect();
                traco(&pontos, false, 2.2, sombra_leve(), *tracejado, window);
                traco(&pontos, false, 1.2, *cor, *tracejado, window);
            }
        }
        Marca::Alca { centro, ativa } => {
            let c = em(*centro);
            let l = px(4.5);
            window.paint_quad(gpui_kit::quad(
                gpui_kit::Bounds::new(point(c.x - l, c.y - l), gpui_kit::size(l * 2., l * 2.)),
                px(1.),
                if *ativa { ambar() } else { branco() },
                px(1.),
                gpui_kit::rgba(0x000000cc),
                gpui_kit::BorderStyle::Solid,
            ));
        }
        Marca::Alfinete { centro, ativo } => {
            let c = em(*centro);
            let r = px(if *ativo { 5. } else { 4. });
            window.paint_quad(gpui_kit::quad(
                gpui_kit::Bounds::new(point(c.x - r, c.y - r), gpui_kit::size(r * 2., r * 2.)),
                r,
                if *ativo {
                    ambar()
                } else {
                    gpui_kit::rgba(0xffffffd9).into()
                },
                px(1.),
                gpui_kit::rgba(0x000000b3),
                gpui_kit::BorderStyle::Solid,
            ));
        }
    }
}

#[cfg(test)]
mod testes {
    use crate::revelacao::zoom::Ponto;

    /// O que cada marca risca na tela: `(polilinha, largura do traço)`.
    fn riscos(marcas: &[super::Marca]) -> Vec<(Vec<Ponto>, f32)> {
        marcas
            .iter()
            .filter_map(|m| match m {
                super::Marca::Circulo { centro, raio, .. } => Some(vec![(
                    (0..=720)
                        .map(|k| {
                            let a = k as f32 / 720.0 * std::f32::consts::TAU;
                            Ponto {
                                x: centro.x + raio * a.cos(),
                                y: centro.y + raio * a.sin(),
                            }
                        })
                        .collect(),
                    1.2,
                )]),
                super::Marca::Caminho {
                    pontos, largura, ..
                } => Some(vec![(pontos.clone(), *largura)]),
                super::Marca::Contorno { trechos, .. } => {
                    Some(trechos.iter().map(|t| (t.clone(), 1.2)).collect())
                }
                _ => None,
            })
            .flatten()
            .collect()
    }

    fn distancia_ao_segmento(q: Ponto, a: Ponto, b: Ponto) -> f32 {
        let (abx, aby) = (b.x - a.x, b.y - a.y);
        let l2 = abx * abx + aby * aby;
        let t = if l2 > 0.0 {
            (((q.x - a.x) * abx + (q.y - a.y) * aby) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (q.x - a.x - abx * t).hypot(q.y - a.y - aby * t)
    }

    fn distancia_ao_caminho(q: Ponto, caminho: &[Ponto]) -> f32 {
        if caminho.len() == 1 {
            return distancia_ao_segmento(q, caminho[0], caminho[0]);
        }
        caminho
            .windows(2)
            .map(|p| distancia_ao_segmento(q, p[0], p[1]))
            .fold(f32::MAX, f32::min)
    }

    /// 🚨 A faixa clara do band-aid (visto no app, 2026-09-27): o contorno
    /// enchia o corpo do caminho com um traço de 2·raio e 18% de branco, só
    /// entre a primeira e a última ponta. Numa pincelada curta, com zoom, isso
    /// é um retângulo estreito e alto — uma faixa vertical que atravessa a
    /// foto e parece defeito da revelação. O contorno é **linha na borda** da
    /// área retocada: não vela a foto, não risca por dentro, e fecha a volta.
    #[test]
    fn o_contorno_do_retoque_e_so_a_borda_da_area() {
        let p = |x: f32, y: f32| Ponto { x, y };
        for (caminho, r) in [
            (vec![p(100.0, 100.0), p(108.0, 100.0)], 320.0),
            (vec![p(0.0, 0.0), p(200.0, 0.0), p(200.0, 150.0)], 40.0),
            (vec![p(50.0, 60.0)], 30.0),
        ] {
            let riscos = riscos(&super::marcas_do_contorno(
                &caminho,
                r,
                gpui_kit::white(),
                false,
            ));
            assert!(!riscos.is_empty());
            for (_, largura) in &riscos {
                assert!(*largura <= 2.0, "traço de {largura} pt vela a foto");
            }
            for (linha, _) in &riscos {
                for par in linha.windows(2) {
                    for k in 0..=4 {
                        let t = k as f32 / 4.0;
                        let q = p(
                            par[0].x + (par[1].x - par[0].x) * t,
                            par[0].y + (par[1].y - par[0].y) * t,
                        );
                        let d = distancia_ao_caminho(q, &caminho);
                        assert!(
                            (d - r).abs() <= 1.0,
                            "risco em ({:.1}, {:.1}) a {d:.1} do caminho, e não na borda ({r})",
                            q.x,
                            q.y
                        );
                    }
                }
            }
            // A borda inteira está riscada: os lados de cada trecho e as pontas.
            let mut sondas = Vec::new();
            for par in caminho.windows(2) {
                let (dx, dy) = (par[1].x - par[0].x, par[1].y - par[0].y);
                let l = dx.hypot(dy);
                let (nx, ny) = (-dy / l, dx / l);
                let meio = p((par[0].x + par[1].x) / 2.0, (par[0].y + par[1].y) / 2.0);
                sondas.push(p(meio.x + nx * r, meio.y + ny * r));
                sondas.push(p(meio.x - nx * r, meio.y - ny * r));
            }
            let n = caminho.len();
            let fim = |a: Ponto, b: Ponto| {
                let (dx, dy) = (a.x - b.x, a.y - b.y);
                let l = dx.hypot(dy).max(1e-6);
                p(a.x + dx / l * r, a.y + dy / l * r)
            };
            if n > 1 {
                sondas.push(fim(caminho[0], caminho[1]));
                sondas.push(fim(caminho[n - 1], caminho[n - 2]));
            } else {
                sondas.push(p(caminho[0].x + r, caminho[0].y));
            }
            for q in sondas {
                let perto = riscos.iter().any(|(linha, _)| {
                    linha
                        .windows(2)
                        .any(|par| distancia_ao_segmento(q, par[0], par[1]) <= 1.0)
                });
                assert!(perto, "a borda em ({:.1}, {:.1}) ficou sem risco", q.x, q.y);
            }
        }
    }

    /// 🔎 O Tamanho é na tela, num trilho logarítmico: fino embaixo, o
    /// mesmo teto de sempre em cima, e o rótulo em pixels da foto.
    #[test]
    fn o_tamanho_na_tela_anda_em_trilho_logaritmico() {
        use super::{
            posicao_do_tamanho, rotulo_do_tamanho, tamanho_da_posicao, TAMANHO_MAXIMO,
            TAMANHO_MINIMO,
        };
        assert!((tamanho_da_posicao(0.0) - TAMANHO_MINIMO).abs() < 1e-5);
        assert!((tamanho_da_posicao(1.0) - TAMANHO_MAXIMO).abs() < 1e-3);
        for t in [2.0, 5.0, 30.0, 120.0, 400.0] {
            assert!((tamanho_da_posicao(posicao_do_tamanho(t)) - t).abs() < t * 1e-4);
        }
        // Um passo do trilho (0,001) no piso é cerca de um centésimo de ponto.
        assert!(tamanho_da_posicao(0.001) - TAMANHO_MINIMO < 0.02);
        assert_eq!(rotulo_do_tamanho(2.8), "2.8 px");
        assert_eq!(rotulo_do_tamanho(180.4), "180 px");
    }

    #[test]
    fn a_reta_do_linear_e_cortada_na_borda_da_foto() {
        let (a, b) = super::cortar_na_foto([-1.0, 0.5], [2.0, 0.5]).expect("atravessa");
        assert!(a[0].abs() < 1e-6 && (b[0] - 1.0).abs() < 1e-6);
        assert!(
            super::cortar_na_foto([-1.0, 2.0], [2.0, 2.0]).is_none(),
            "passa por fora"
        );
    }

    #[test]
    fn o_nome_da_mascara_nova_e_o_menor_livre() {
        use super::{Camada, ParametrosLocais};
        let com = |nomes: &[&str]| ParametrosLocais {
            camadas: nomes
                .iter()
                .map(|n| Camada {
                    nome: n.to_string(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        assert_eq!(super::nome_livre(&com(&[])), "Máscara 1");
        assert_eq!(
            super::nome_livre(&com(&["Máscara 1", "Máscara 3"])),
            "Máscara 2"
        );
    }
}
