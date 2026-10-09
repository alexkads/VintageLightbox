//! 🌫️ Filtro › Desfoque gaussiano… e Máscara de nitidez… — o diálogo do
//! Photoshop: uma caixa pequena por cima do palco, **sem escurecer a foto**,
//! porque a prévia é a própria foto ("Visualizar" ligado).
//!
//! - A conta ([`editor_core::filtros::filtrada`]) corre em segundo plano a
//!   cada mudança de controle (com um respiro de 80 ms para o arrasto não
//!   empilhar contas); a de uma geração velha é jogada fora;
//! - OK grava **um** passo com o nome do filtro (se a prévia ainda está
//!   correndo, a conta termina ali mesmo); Cancelar e Esc voltam a camada;
//! - os valores ficam para a próxima vez, como no Photoshop;
//! - o raio anda numa escala exponencial (0,1 a 250 px): perto do zero o
//!   controle é fino, e o fim ainda alcança o desfoque forte.

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, Context, Entity, MouseButton,
    Pixels, Subscription, Task, Window,
};

use super::aparencia;
use super::EditorDeFoto;
use editor_core::filtros::{filtrada_com, Filtro, RAIO_MAXIMO};
use editor_core::selecao::Selecao;
use editor_core::tiles::CamadaDePixels;

/// O menor raio, em px.
const RAIO_MINIMO: f32 = 0.1;
/// A largura da caixa, em pontos.
const LARGURA: f32 = 300.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tipo {
    Desfoque,
    Nitidez,
    Superficie,
    AltaFrequencia,
    Mediana,
    Ruido,
    /// O desfoque pesado pela seleção, para a baixa frequência (Tratamento
    /// de pele).
    SuavizarTons,
}

impl Tipo {
    pub const TODOS: [Tipo; 7] = [
        Tipo::Desfoque,
        Tipo::Nitidez,
        Tipo::Superficie,
        Tipo::AltaFrequencia,
        Tipo::Mediana,
        Tipo::Ruido,
        Tipo::SuavizarTons,
    ];

    pub fn titulo(self) -> &'static str {
        match self {
            Tipo::Desfoque => "Desfoque gaussiano",
            Tipo::Nitidez => "Máscara de nitidez",
            Tipo::Superficie => "Desfoque de superfície",
            Tipo::AltaFrequencia => "Alta frequência",
            Tipo::Mediana => "Mediana",
            Tipo::Ruido => "Adicionar ruído",
            Tipo::SuavizarTons => "Suavizar tons",
        }
    }

    fn indice(self) -> usize {
        Self::TODOS.iter().position(|t| *t == self).unwrap_or(0)
    }

    /// (raio, quantidade, limiar) com que abre na primeira vez — os do
    /// Photoshop, menos o desfoque (1 px mal se vê em 24 MP; aqui 4).
    fn padrao(self) -> (f32, f32, f32) {
        match self {
            Tipo::Desfoque => (4.0, 100.0, 0.0),
            Tipo::Nitidez => (1.0, 100.0, 0.0),
            Tipo::Superficie => (5.0, 100.0, 15.0),
            Tipo::AltaFrequencia => (10.0, 100.0, 0.0),
            Tipo::Mediana => (1.0, 100.0, 0.0),
            Tipo::Ruido => (1.0, 10.0, 0.0),
            // Manchas de pele num retrato de 24 MP: dezenas de px.
            Tipo::SuavizarTons => (20.0, 100.0, 0.0),
        }
    }

    fn tem_raio(self) -> bool {
        self != Tipo::Ruido
    }

    fn tem_quantidade(self) -> bool {
        matches!(self, Tipo::Nitidez | Tipo::Ruido)
    }

    fn tem_limiar(self) -> bool {
        matches!(self, Tipo::Nitidez | Tipo::Superficie)
    }

    /// O raio do slider no passo do filtro: inteiro na superfície e na
    /// mediana, e dentro do limite de cada um.
    fn raio(self, r: f32) -> f32 {
        match self {
            Tipo::Superficie => r
                .round()
                .clamp(1.0, editor_core::filtros::SUPERFICIE_MAXIMA),
            Tipo::Mediana => r
                .round()
                .clamp(1.0, editor_core::filtros::MEDIANA_MAXIMA as f32),
            _ => r,
        }
    }
}

/// O que os controles valiam no último OK de cada filtro (a próxima abertura
/// começa deles), como no Photoshop.
#[derive(Clone, Copy, Debug)]
struct Lembrado {
    valores: [(f32, f32, f32); 7],
    /// A intensidade (%) de cada filtro.
    intensidades: [f32; 7],
    gaussiano: bool,
    monocromatico: bool,
}

impl Default for Lembrado {
    fn default() -> Self {
        Self {
            valores: Tipo::TODOS.map(Tipo::padrao),
            intensidades: [100.0; 7],
            gaussiano: true,
            monocromatico: false,
        }
    }
}

pub struct Aberto {
    pub tipo: Tipo,
    original: CamadaDePixels,
    selecao: Option<Arc<Selecao>>,
}

/// O estado do diálogo dentro do editor.
pub struct Estado {
    pub aberto: Option<Aberto>,
    raio: Entity<SliderState>,
    quantidade: Entity<SliderState>,
    limiar: Entity<SliderState>,
    /// A intensidade (o "Atenuar" do Photoshop), em %: o resultado sobre a
    /// original, sempre a partir da original.
    intensidade: Entity<SliderState>,
    pub visualizar: bool,
    /// As opções do Adicionar ruído.
    pub gaussiano: bool,
    pub monocromatico: bool,
    lembrado: Lembrado,
    /// A caixa arrastada pelo título: o deslocamento do canto padrão, e o
    /// arrasto em curso (onde o ponteiro desceu, o deslocamento de então).
    deslocamento: gpui_kit::Point<Pixels>,
    arrasto: Option<(gpui_kit::Point<Pixels>, gpui_kit::Point<Pixels>)>,
    /// Sobe a cada pedido de conta; `mostrada` é a que está na tela.
    geracao: u64,
    mostrada: u64,
    _conta: Option<Task<()>>,
    _assinaturas: Vec<Subscription>,
}

/// Raio (px) ↔ posição do slider (0–100), exponencial.
pub fn raio_da_posicao(p: f32) -> f32 {
    let p = p.clamp(0.0, 100.0) / 100.0;
    let r = RAIO_MINIMO * (RAIO_MAXIMO / RAIO_MINIMO).powf(p);
    // Um décimo de pixel.
    (r * 10.0).round() / 10.0
}

pub fn posicao_do_raio(r: f32) -> f32 {
    let r = r.clamp(RAIO_MINIMO, RAIO_MAXIMO);
    100.0 * (r / RAIO_MINIMO).ln() / (RAIO_MAXIMO / RAIO_MINIMO).ln()
}

fn numero(x: f32) -> String {
    let t = format!("{:.1}", x);
    let t = t.strip_suffix(".0").map(str::to_string).unwrap_or(t);
    t.replace('.', ",")
}

impl Estado {
    pub fn novo(window: &mut Window, cx: &mut Context<EditorDeFoto>) -> Self {
        let l = Lembrado::default();
        let novo = |min: f32, max: f32, passo: f32, v: f32, cx: &mut Context<EditorDeFoto>| {
            cx.new(|_| {
                SliderState::new()
                    .min(min)
                    .max(max)
                    .step(passo)
                    .default_value(v)
            })
        };
        let (r, q, li) = l.valores[0];
        let raio = novo(0.0, 100.0, 0.1, posicao_do_raio(r), cx);
        let quantidade = novo(1.0, 500.0, 1.0, q, cx);
        let limiar = novo(0.0, 255.0, 1.0, li, cx);
        let intensidade = novo(0.0, 100.0, 1.0, 100.0, cx);
        let assinaturas = [&raio, &quantidade, &limiar, &intensidade]
            .into_iter()
            .map(|s| {
                cx.subscribe_in(
                    s,
                    window,
                    |ed: &mut EditorDeFoto, _s, _e: &SliderEvent, _w, cx| {
                        ed.controle_do_filtro_mudou(cx)
                    },
                )
            })
            .collect();
        Self {
            aberto: None,
            raio,
            quantidade,
            limiar,
            intensidade,
            visualizar: true,
            gaussiano: l.gaussiano,
            monocromatico: l.monocromatico,
            lembrado: l,
            deslocamento: gpui_kit::point(px(0.), px(0.)),
            arrasto: None,
            geracao: 0,
            mostrada: 0,
            _conta: None,
            _assinaturas: assinaturas,
        }
    }
}

impl EditorDeFoto {
    pub fn filtro_aberto(&self) -> Option<Tipo> {
        self.filtro.aberto.as_ref().map(|a| a.tipo)
    }

    /// Filtro › … : abre o diálogo na camada escolhida (ou na máscara dela).
    pub fn abrir_filtro(&mut self, tipo: Tipo, window: &mut Window, cx: &mut Context<Self>) {
        if self.filtro.aberto.is_some() || self.area_do_preenchimento.is_some() {
            return;
        }
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let Some((original, selecao)) = s.comecar_filtro() else {
            self.aviso = Some((
                "O filtro precisa de uma camada de pixels visível e com conteúdo — para a foto, dê duplo clique no Fundo"
                    .into(),
                true,
            ));
            cx.notify();
            return;
        };
        let l = self.filtro.lembrado;
        let (raio, quantidade, limiar) = l.valores[tipo.indice()];
        self.filtro.gaussiano = l.gaussiano;
        self.filtro.monocromatico = l.monocromatico;
        let f = &self.filtro;
        f.raio
            .clone()
            .update(cx, |s, cx| s.set_value(posicao_do_raio(raio), window, cx));
        f.quantidade
            .clone()
            .update(cx, |s, cx| s.set_value(quantidade, window, cx));
        f.limiar
            .clone()
            .update(cx, |s, cx| s.set_value(limiar, window, cx));
        let intensidade = l.intensidades[tipo.indice()];
        f.intensidade
            .clone()
            .update(cx, |s, cx| s.set_value(intensidade, window, cx));
        self.filtro.aberto = Some(Aberto {
            tipo,
            original,
            selecao,
        });
        self.recalcular_o_filtro(Duration::ZERO, cx);
        cx.notify();
    }

    /// O filtro com os valores dos controles de agora.
    pub fn filtro_dos_controles(&self, cx: &gpui_kit::App) -> Option<Filtro> {
        let aberto = self.filtro.aberto.as_ref()?;
        let tipo = aberto.tipo;
        let raio = tipo.raio(raio_da_posicao(self.filtro.raio.read(cx).value().start()));
        let quantidade = self.filtro.quantidade.read(cx).value().start() / 100.0;
        let limiar = self
            .filtro
            .limiar
            .read(cx)
            .value()
            .start()
            .round()
            .clamp(0.0, 255.0) as u8;
        Some(match tipo {
            Tipo::Desfoque => Filtro::DesfoqueGaussiano { raio },
            Tipo::Nitidez => Filtro::MascaraDeNitidez {
                quantidade,
                raio,
                limiar,
            },
            Tipo::Superficie => Filtro::DesfoqueDeSuperficie {
                raio,
                limiar: limiar.max(2),
            },
            Tipo::AltaFrequencia => Filtro::AltaFrequencia { raio },
            Tipo::Mediana => Filtro::Mediana { raio: raio as u32 },
            Tipo::Ruido => Filtro::AdicionarRuido {
                quantidade,
                gaussiano: self.filtro.gaussiano,
                monocromatico: self.filtro.monocromatico,
            },
            Tipo::SuavizarTons => Filtro::SuavizarTons { raio },
        })
    }

    /// A intensidade dos controles, 0..=1.
    fn intensidade_do_filtro(&self, cx: &gpui_kit::App) -> f32 {
        (self.filtro.intensidade.read(cx).value().start() / 100.0).clamp(0.0, 1.0)
    }

    /// O roteiro e os testes: um controle pelo valor ("raio" em px,
    /// "quantidade" em %, "limiar" em níveis), como se o slider andasse.
    pub fn definir_controle_do_filtro(
        &mut self,
        qual: &str,
        valor: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (estado, v) = match qual {
            "raio" => (self.filtro.raio.clone(), posicao_do_raio(valor)),
            "quantidade" => (self.filtro.quantidade.clone(), valor),
            "limiar" => (self.filtro.limiar.clone(), valor),
            "intensidade" => (self.filtro.intensidade.clone(), valor),
            _ => return,
        };
        estado.update(cx, |s, cx| s.set_value(v, window, cx));
        self.controle_do_filtro_mudou(cx);
    }

    fn controle_do_filtro_mudou(&mut self, cx: &mut Context<Self>) {
        if self.filtro.aberto.is_some() {
            self.recalcular_o_filtro(Duration::from_millis(80), cx);
            cx.notify();
        }
    }

    /// Pede a conta da prévia (ou volta a original, com "Visualizar"
    /// desligado).
    fn recalcular_o_filtro(&mut self, respiro: Duration, cx: &mut Context<Self>) {
        let Some(filtro) = self.filtro_dos_controles(cx) else {
            return;
        };
        let Some(aberto) = self.filtro.aberto.as_ref() else {
            return;
        };
        self.filtro.geracao += 1;
        let geracao = self.filtro.geracao;
        if !self.filtro.visualizar {
            let original = aberto.original.clone();
            self.filtro._conta = None;
            self.filtro.mostrada = 0;
            if let Some(s) = self.sessao_mut() {
                s.mostrar_filtro(original);
            }
            cx.notify();
            return;
        }
        let (original, selecao) = (aberto.original.clone(), aberto.selecao.clone());
        let intensidade = self.intensidade_do_filtro(cx);
        self.filtro._conta = Some(cx.spawn(async move |ed, cx| {
            if !respiro.is_zero() {
                cx.background_executor().timer(respiro).await;
            }
            let pixels = cx
                .background_executor()
                .spawn(
                    async move { filtrada_com(&original, filtro, selecao.as_deref(), intensidade) },
                )
                .await;
            let _ = ed.update(cx, |ed, cx| {
                if ed.filtro.geracao != geracao || ed.filtro.aberto.is_none() {
                    return;
                }
                ed.filtro.mostrada = geracao;
                if let Some(s) = ed.sessao_mut() {
                    s.mostrar_filtro(pixels);
                }
                cx.notify();
            });
        }));
    }

    /// A prévia está em dia com os controles.
    pub fn previa_do_filtro_pronta(&self) -> bool {
        self.filtro.aberto.is_some()
            && self.filtro.visualizar
            && self.filtro.mostrada == self.filtro.geracao
    }

    /// As opções do Adicionar ruído (Gaussiana / Monocromático).
    pub fn alternar_opcao_do_ruido(&mut self, monocromatico: bool, cx: &mut Context<Self>) {
        if monocromatico {
            self.filtro.monocromatico = !self.filtro.monocromatico;
        } else {
            self.filtro.gaussiano = !self.filtro.gaussiano;
        }
        self.recalcular_o_filtro(Duration::ZERO, cx);
        cx.notify();
    }

    pub fn alternar_visualizar_o_filtro(&mut self, cx: &mut Context<Self>) {
        self.filtro.visualizar = !self.filtro.visualizar;
        self.recalcular_o_filtro(Duration::ZERO, cx);
    }

    /// OK (Enter).
    pub fn confirmar_filtro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(filtro) = self.filtro_dos_controles(cx) else {
            return;
        };
        let Some(aberto) = self.filtro.aberto.take() else {
            return;
        };
        self.filtro._conta = None;
        // A prévia não está em dia (correndo, ou "Visualizar" desligado): a
        // conta termina aqui.
        if !(self.filtro.visualizar && self.filtro.mostrada == self.filtro.geracao) {
            let pixels = filtrada_com(
                &aberto.original,
                filtro,
                aberto.selecao.as_deref(),
                self.intensidade_do_filtro(cx),
            );
            if let Some(s) = self.sessao_mut() {
                s.mostrar_filtro(pixels);
            }
        }
        self.filtro.geracao += 1;
        let valores = (
            aberto
                .tipo
                .raio(raio_da_posicao(self.filtro.raio.read(cx).value().start())),
            self.filtro.quantidade.read(cx).value().start(),
            self.filtro.limiar.read(cx).value().start(),
        );
        let intensidade = self.filtro.intensidade.read(cx).value().start();
        let l = &mut self.filtro.lembrado;
        l.valores[aberto.tipo.indice()] = valores;
        l.intensidades[aberto.tipo.indice()] = intensidade;
        l.gaussiano = self.filtro.gaussiano;
        l.monocromatico = self.filtro.monocromatico;
        self.na_sessao(cx, |s| {
            s.aplicar_filtro(filtro.nome());
        });
        window.focus(&self.foco, cx);
    }

    /// Cancelar (Esc).
    pub fn cancelar_o_filtro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.filtro.aberto.take().is_none() {
            return;
        }
        self.filtro._conta = None;
        self.filtro.geracao += 1;
        if let Some(s) = self.sessao_mut() {
            s.cancelar_filtro();
        }
        window.focus(&self.foco, cx);
        cx.notify();
    }

    /// O título arrastado: a caixa anda com o ponteiro.
    pub fn arrastar_a_caixa_do_filtro(
        &mut self,
        ponteiro: gpui_kit::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if let Some((de, antes)) = self.filtro.arrasto {
            self.filtro.deslocamento = antes + (ponteiro - de);
            cx.notify();
        }
    }

    pub fn deslocamento_da_caixa_do_filtro(&self) -> gpui_kit::Point<Pixels> {
        self.filtro.deslocamento
    }

    /// A caixa do diálogo, no canto de cima à direita do palco.
    pub(super) fn caixa_do_filtro(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let aberto = self.filtro.aberto.as_ref()?;
        let c = aparencia::cores(cx);
        let tema = cx.theme().clone();
        let tipo = aberto.tipo;
        let raio = tipo.raio(raio_da_posicao(self.filtro.raio.read(cx).value().start()));
        let quantidade = self.filtro.quantidade.read(cx).value().start();
        let limiar = self.filtro.limiar.read(cx).value().start();
        let intensidade = self.filtro.intensidade.read(cx).value().start();
        let linha = |id: &'static str,
                     rotulo: &'static str,
                     estado: &Entity<SliderState>,
                     texto: String| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_sm()
                        .child(rotulo)
                        .child(div().text_color(c.apagado).child(texto)),
                )
                .child(
                    div()
                        .h(px(20.))
                        .debug_selector(move || id.into())
                        .child(crate::estilo::slider(estado)),
                )
        };
        let calculando = self.filtro.visualizar && self.filtro.mostrada != self.filtro.geracao;
        let onde = point(
            self.palco.right() - px(LARGURA + 12.),
            self.palco.top() + px(12.),
        ) + self.filtro.deslocamento;
        let arrastando = self.filtro.arrasto.is_some();
        let corpo = div()
            .id("editor-filtro")
            .debug_selector(|| "editor-filtro".into())
            .occlude()
            .w(px(LARGURA))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .bg(tema.popover)
            .text_color(tema.popover_foreground)
            .border_1()
            .border_color(c.borda)
            .rounded(px(6.))
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(cx.listener(|ed, e: &gpui_kit::MouseMoveEvent, _, cx| {
                ed.arrastar_a_caixa_do_filtro(e.position, cx)
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|ed, _: &gpui_kit::MouseUpEvent, _, cx| {
                    if ed.filtro.arrasto.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .child(
                // O título: arrastar leva a caixa, como a janela do filtro lá.
                div()
                    .id("editor-filtro-titulo")
                    .debug_selector(|| "editor-filtro-titulo".into())
                    .cursor(gpui_kit::CursorStyle::OpenHand)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|ed, e: &gpui_kit::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            ed.filtro.arrasto = Some((e.position, ed.filtro.deslocamento));
                        }),
                    )
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(aberto.tipo.titulo()),
                    )
                    .child(div().text_xs().text_color(c.apagado).child(if calculando {
                        "calculando…"
                    } else {
                        ""
                    })),
            )
            .when(tipo.tem_quantidade(), |d| {
                d.child(linha(
                    "editor-filtro-quantidade",
                    "Quantidade",
                    &self.filtro.quantidade,
                    format!("{}%", quantidade.round() as i32),
                ))
            })
            .when(tipo.tem_raio(), |d| {
                d.child(linha(
                    "editor-filtro-raio",
                    "Raio",
                    &self.filtro.raio,
                    format!("{} px", numero(raio)),
                ))
            })
            .when(tipo == Tipo::Ruido, |d| {
                d.child(
                    div()
                        .flex()
                        .gap(px(16.))
                        .child(
                            Checkbox::new("editor-filtro-gaussiana")
                                .small()
                                .label("Gaussiana")
                                .checked(self.filtro.gaussiano)
                                .on_click(cx.listener(|ed, _: &bool, _, cx| {
                                    ed.alternar_opcao_do_ruido(false, cx)
                                })),
                        )
                        .child(
                            Checkbox::new("editor-filtro-monocromatico")
                                .small()
                                .label("Monocromático")
                                .checked(self.filtro.monocromatico)
                                .on_click(cx.listener(|ed, _: &bool, _, cx| {
                                    ed.alternar_opcao_do_ruido(true, cx)
                                })),
                        ),
                )
            })
            .when(tipo.tem_limiar(), |d| {
                d.child(linha(
                    "editor-filtro-limiar",
                    "Limiar",
                    &self.filtro.limiar,
                    format!("{} níveis", limiar.round() as i32),
                ))
            })
            .when(tipo == Tipo::SuavizarTons, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(c.apagado)
                        .child("Na baixa frequência: só os pixels selecionados entram na média — selecione a pele sem cabelo, olhos e lábios."),
                )
            })
            .child(linha(
                "editor-filtro-intensidade",
                "Intensidade",
                &self.filtro.intensidade,
                format!("{}%", intensidade.round() as i32),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        Checkbox::new("editor-filtro-visualizar")
                            .small()
                            .label("Visualizar")
                            .checked(self.filtro.visualizar)
                            .on_click(cx.listener(|ed, _: &bool, _, cx| {
                                ed.alternar_visualizar_o_filtro(cx)
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                crate::estilo::botao_contorno("editor-filtro-cancelar", cx)
                                    .child("Cancelar")
                                    .on_click(cx.listener(|ed, _, window, cx| {
                                        ed.cancelar_o_filtro(window, cx)
                                    })),
                            )
                            .child(
                                crate::estilo::botao_primario("editor-filtro-ok", cx)
                                    .child("OK")
                                    .on_click(cx.listener(|ed, _, window, cx| {
                                        ed.confirmar_filtro(window, cx)
                                    })),
                            ),
                    ),
            );
        // 🔒 Modal, como lá: um véu transparente toma os cliques do resto da
        // janela (as teclas, o contexto `FiltroDoEditor` já barra).
        // O véu também segue o arrasto do título (o ponteiro sai da caixa).
        let veu = deferred(
            anchored().position(point(px(0.), px(0.))).child(
                div()
                    .id("editor-filtro-veu")
                    .occlude()
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height)
                    .when(arrastando, |d| d.cursor(gpui_kit::CursorStyle::ClosedHand))
                    .on_mouse_move(cx.listener(|ed, e: &gpui_kit::MouseMoveEvent, _, cx| {
                        ed.arrastar_a_caixa_do_filtro(e.position, cx)
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|ed, _: &gpui_kit::MouseUpEvent, _, cx| {
                            ed.filtro.arrasto = None;
                            cx.notify();
                        }),
                    ),
            ),
        )
        .with_priority(1);
        Some(
            div()
                .child(veu)
                .child(
                    deferred(
                        anchored()
                            .position(onde)
                            .snap_to_window_with_margin(px(8.))
                            .child(corpo),
                    )
                    .with_priority(2),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_raio_vai_e_volta_pela_posicao() {
        assert_eq!(raio_da_posicao(0.0), 0.1);
        assert_eq!(raio_da_posicao(100.0), 250.0);
        for r in [0.5f32, 1.0, 4.0, 12.5, 100.0] {
            let p = posicao_do_raio(r);
            assert!((raio_da_posicao(p) - r).abs() <= 0.1, "{r} → {p}");
        }
        assert_eq!(numero(4.0), "4");
        assert_eq!(numero(2.5), "2,5");
    }
}
