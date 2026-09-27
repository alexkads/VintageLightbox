//! A **Revelação local** na tela: as máscaras (pincel, gradientes, laço) com a
//! exposição delas, e o retoque (carimbo, band-aid, Content-Aware).
//!
//! O motor, a receita e a matemática moram no `revelacao-core` (`locais.rs`,
//! `mascaras.rs`, `retoque.rs`, `preenchimento.rs`) e são os mesmos do site. O
//! que mora aqui é o que só a tela tem: a ferramenta escolhida, o gesto em
//! curso, a conta do ponteiro à foto, as marcações por cima da foto e o painel.
//!
//! # A receita é a fonte de verdade
//!
//! [`Revelacao::locais`] é a receita da foto aberta, e é ela que o histórico,
//! a gravação e a GPU recebem. O gesto em curso é **provisório**: a GPU o
//! revela a cada movimento (`locais_na_tela`), mas ele só entra na receita no
//! fim do gesto — um `⌘Z` por pincelada, como os sliders.
//!
//! # Coordenadas
//!
//! O ponteiro chega em pontos da área do palco; a receita quer a foto inteira
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
    GradienteRadial, Laco, Modo, Preenchimento, ReceitaLocal, Retoque,
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
    /// O próximo número de "Máscara N".
    proximo_nome: usize,
    /// Os valores da ferramenta (os sliders escrevem aqui).
    pub raio: f32,
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

const RAIO_INICIAL: f32 = 0.03;
const FEATHER_INICIAL: f32 = 0.5;
/// O feather do laço é fração do maior lado (até 4%); o do pincel, do raio.
const FEATHER_DO_LACO: f32 = 0.04;
/// Até onde o clique ainda pega uma alça, em pontos.
const ALCANCE_DA_ALCA: f32 = 8.0;

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
            raio: slider(0.005, 0.15, 0.001, RAIO_INICIAL, cx),
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
            proximo_nome: 1,
            raio: RAIO_INICIAL,
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

fn sombra() -> Hsla {
    gpui_kit::rgba(0x0000008c).into()
}

impl Revelacao {
    // ------------------------------------------------------------ a receita

    /// A receita que a GPU revela agora: a da foto, mais o gesto em curso.
    pub(super) fn locais_com_o_gesto(&self) -> Arc<ReceitaLocal> {
        let Some(gesto) = &self.local.gesto else {
            return self.locais.clone();
        };
        let mut receita = (*self.locais).clone();
        match gesto {
            Gesto::Componente { nova, componente } => {
                if *nova {
                    receita.camadas.push(Camada {
                        componentes: vec![componente.clone()],
                        ajustes: AjustesLocais {
                            exposicao_ev: self.exposicao_da_nova(),
                        },
                        ..Default::default()
                    });
                } else if let Some(camada) = self
                    .local
                    .mascara_sel
                    .and_then(|i| receita.camadas.get_mut(i))
                {
                    camada.componentes.push(componente.clone());
                }
            }
            // O Content-Aware sintetiza no fim do gesto, e não a cada ponto.
            Gesto::Retoque(Retoque::Preencher(_)) => {}
            Gesto::Retoque(r) => receita.retoques.push(r.clone()),
            Gesto::Editando { indice, valor, .. } => {
                if let Some(r) = receita.retoques.get_mut(*indice) {
                    *r = valor.clone();
                }
            }
        }
        Arc::new(receita)
    }

    /// A exposição com que nasce uma máscara nova: a do slider.
    fn exposicao_da_nova(&self) -> f32 {
        self.local
            .mascara_sel
            .and_then(|i| self.locais.camadas.get(i))
            .map(|c| c.ajustes.exposicao_ev)
            .unwrap_or(0.8)
    }

    /// A receita nova vira um passo do histórico, e vai ao banco e à GPU.
    fn comprometer(&mut self, receita: ReceitaLocal, cx: &mut Context<Self>) {
        self.gravar_o_que_estiver_pendente();
        self.locais = Arc::new(receita);
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

    /// Um raio da receita (fração do maior lado) em pontos da tela.
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

    pub fn alternar_marcacoes(&mut self, cx: &mut Context<Self>) {
        self.local.marcacoes = !self.local.marcacoes;
        cx.notify();
    }

    /// `[` e `]` (com Shift, a suavização): no retoque selecionado, se houver;
    /// senão, na ferramenta.
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
        if let Some(i) = self.local.selecionado {
            if let Some(r) = self.locais.retoques.get(i).cloned() {
                let novo = com_raio(&r, (raio_do(&r) * fator).clamp(0.002, 0.25));
                let mut receita = (*self.locais).clone();
                receita.retoques[i] = novo;
                self.comprometer(receita, cx);
                return;
            }
        }
        let novo = (self.local.raio * fator).clamp(0.005, 0.15);
        self.local
            .sliders
            .raio
            .update(cx, |s, cx| s.set_value(novo, window, cx));
        self.local.raio = novo;
        cx.notify();
    }

    /// `Delete` com um retoque selecionado. Devolve se apagou.
    pub fn apagar_retoque_selecionado(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(i) = self.local.selecionado.take() else {
            return false;
        };
        let mut receita = (*self.locais).clone();
        if i >= receita.retoques.len() {
            return false;
        }
        receita.retoques.remove(i);
        self.comprometer(receita, cx);
        true
    }

    fn slider_local(&mut self, qual: u8, valor: f32, soltou: bool, cx: &mut Context<Self>) {
        match qual {
            0 => self.local.raio = valor,
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
                    let mut receita = (*self.locais).clone();
                    receita.retoques[i] = novo;
                    self.comprometer(receita, cx);
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
        let mut receita = (*self.locais).clone();
        receita.camadas[i].ajustes.exposicao_ev = valor;
        if soltou {
            self.comprometer(receita, cx);
        } else {
            // Ao vivo, sem passo: a receita da tela anda, o histórico não.
            self.locais = Arc::new(receita);
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
        let mut receita = (*self.locais).clone();
        let Some(camada) = receita.camadas.get_mut(i) else {
            return;
        };
        if mudar(camada) {
            receita.camadas.remove(i);
            self.local.mascara_sel = None;
        }
        self.comprometer(receita, cx);
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

    /// A origem automática do band-aid: do lado com mais espaço, a 3 raios.
    fn origem_automatica(&self, q: [f32; 2]) -> [f32; 2] {
        let (w, h) = self.tamanho_da_copia().unwrap_or((1.0, 1.0));
        let d = 3.0 * self.local.raio * w.max(h);
        let r = self.local.raio;
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
        let raio = self.local.raio;
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
                let origem = match (self.local.origem, ferramenta) {
                    (Some(o), _) => o,
                    (None, Ferramenta::BandAid) => self.origem_automatica(q),
                    (None, _) => {
                        // O carimbo precisa de origem: a dica do painel diz como.
                        self.local.gesto = None;
                        cx.notify();
                        return;
                    }
                };
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
        let mut revelar = true;
        match gesto {
            Gesto::Componente { componente, .. } => match &mut componente.forma {
                Forma::Pincel(t) => t.pontos.push([q[0], q[1], 1.0]),
                Forma::Linear(g) => g.fim = q,
                Forma::Radial(g) => {
                    g.raio_x = ((q[0] - g.centro[0]).abs() * w / lado).max(0.003);
                    g.raio_y = ((q[1] - g.centro[1]).abs() * h / lado).max(0.003);
                }
                Forma::Laco(l) => l.pontos.push(q),
            },
            Gesto::Retoque(Retoque::Clone(c) | Retoque::Heal(c)) => c.caminho.push(q),
            Gesto::Retoque(Retoque::Preencher(p)) => {
                if p.laco.is_empty() {
                    p.caminho.push(q);
                } else {
                    p.laco.push(q);
                }
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
                        com_raio(&r, (d / lado).clamp(0.002, 0.25))
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
        }
        if revelar {
            self.pedir_revelacao(cx);
        }
        cx.notify();
    }

    /// Fim do gesto: vira **um** passo do histórico.
    pub(super) fn local_soltar(&mut self, cx: &mut Context<Self>) {
        let Some(gesto) = self.local.gesto.take() else {
            return;
        };
        let mut receita = (*self.locais).clone();
        match gesto {
            Gesto::Componente { nova, componente } => {
                if let Forma::Laco(l) = &componente.forma {
                    if l.pontos.len() < 3 {
                        cx.notify();
                        return;
                    }
                }
                if nova {
                    let nome = format!("Máscara {}", self.local.proximo_nome);
                    self.local.proximo_nome += 1;
                    receita.camadas.push(Camada {
                        nome,
                        componentes: vec![componente],
                        ajustes: AjustesLocais {
                            exposicao_ev: self.exposicao_da_nova(),
                        },
                        ..Default::default()
                    });
                    self.local.mascara_sel = Some(receita.camadas.len() - 1);
                    self.local.criando = false;
                } else if let Some(c) = self
                    .local
                    .mascara_sel
                    .and_then(|i| receita.camadas.get_mut(i))
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
                receita.retoques.push(r);
                if com_origem {
                    self.local.selecionado = Some(receita.retoques.len() - 1);
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
                receita.retoques[indice] = valor;
            }
        }
        self.comprometer(receita, cx);
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
        let mut receita = (*self.locais).clone();
        if ferramenta == Ferramenta::Preencher {
            receita.retoques.push(Retoque::Preencher(Preenchimento {
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
                Some(i) if i < receita.camadas.len() => {
                    receita.camadas[i].componentes.push(componente)
                }
                _ => {
                    let nome = format!("Máscara {}", self.local.proximo_nome);
                    self.local.proximo_nome += 1;
                    receita.camadas.push(Camada {
                        nome,
                        componentes: vec![componente],
                        ajustes: AjustesLocais {
                            exposicao_ev: self.exposicao_da_nova(),
                        },
                        ..Default::default()
                    });
                    self.local.mascara_sel = Some(receita.camadas.len() - 1);
                    self.local.criando = false;
                }
            }
        }
        self.comprometer(receita, cx);
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
        self.local.raio = raio;
        self.local.feather = feather;
        self.local.opacidade = opacidade;
        for (s, v) in [
            (self.local.sliders.raio.clone(), raio),
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
                        raio: self.raio_na_tela(self.local.raio),
                        cor: ambar(),
                        tracejado: true,
                    });
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
                let raio = self.raio_na_tela(self.local.raio);
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
        let Some(primeiro) = tela.first().copied() else {
            return;
        };
        let r = self.raio_na_tela(raio);
        marcas.push(Marca::Circulo {
            centro: primeiro,
            raio: r,
            cor,
            tracejado,
        });
        if tela.len() > 1 {
            let ultimo = *tela.last().expect("não vazio");
            marcas.push(Marca::Circulo {
                centro: ultimo,
                raio: r,
                cor,
                tracejado,
            });
            let mut faixa = cor;
            faixa.a = 0.18;
            marcas.push(Marca::Caminho {
                pontos: tela,
                fechado: false,
                cor: faixa,
                tracejado: false,
                largura: 2.0 * r,
            });
        }
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
            .rounded(px(8.))
            .bg(tema.muted);
        for f in Ferramenta::MASCARA {
            barra = barra.child(botao_da_ferramenta(f, cx));
        }
        barra = barra.child(div().w(px(1.)).h(px(18.)).mx(px(3.)).bg(tema.border));
        for f in Ferramenta::RETOQUE {
            barra = barra.child(botao_da_ferramenta(f, cx));
        }
        barra = barra.child(div().flex_1()).child(
            Button::new("local-marcacoes")
                .icon(Icon::new(Icone::MapPin).size(px(15.)))
                .small()
                .ghost()
                .selected(self.local.marcacoes)
                .tooltip(if self.local.marcacoes {
                    "Esconder marcações (H)"
                } else {
                    "Mostrar marcações (H)"
                })
                .on_click(cx.listener(|tela, _e, _w, cx| tela.alternar_marcacoes(cx))),
        );

        let titulo = div().flex().items_center().justify_between().child(
            div()
                .text_xs()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(tema.muted_foreground)
                .child("REVELAÇÃO LOCAL"),
        );

        let mut secao = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(titulo)
            .child(barra);
        if let Some(erro) = &self.locais_ilegiveis {
            secao = secao.child(
                div()
                    .text_xs()
                    .text_color(tema.warning)
                    .child(SharedString::from(format!(
                        "A Revelação local desta foto foi feita numa versão mais nova do app e não pode ser editada aqui ({erro})."
                    ))),
            );
        }
        if let Some(f) = ferramenta {
            secao = secao.child(self.opcoes_da_ferramenta(f, cx));
        }
        secao = secao.child(self.lista_de_mascaras(cx));
        secao = secao.child(self.lista_de_retoques(cx));
        secao.into_any_element()
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
            Ferramenta::Carimbo => "⌥ + clique escolhe a origem; depois pinte o destino.",
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
                .rounded(px(7.))
                .bg(tema.muted);
            for (k, (rotulo, ativo)) in rotulos.into_iter().enumerate() {
                linha = linha.child(
                    Button::new(SharedString::from(format!("{id}-{k}")))
                        .label(rotulo)
                        .xsmall()
                        .ghost()
                        .selected(ativo)
                        .flex_1()
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
            .rounded(px(8.))
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
                format!("{:.1}%", self.local.raio * 100.0),
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
                    .rounded(px(6.))
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
                lista =
                    lista.child(comps).child(
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
                    );
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
                    .rounded(px(6.))
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
                                let mut receita = (*tela.locais).clone();
                                if k < receita.retoques.len() {
                                    receita.retoques.remove(k);
                                    tela.local.selecionado = None;
                                    tela.comprometer(receita, cx);
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
            traco(&pontos, false, 3.0, sombra(), false, window);
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
