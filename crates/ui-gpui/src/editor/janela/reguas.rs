//! 📏 As réguas do Photoshop (⌘R, Visualizar › Réguas): em cima e à
//! esquerda do palco, em pixels da foto, com o zero no canto de cima à
//! esquerda dela. Acompanham o zoom e a mão; a marca do ponteiro corre nelas.
//!
//! - O passo das marcas é o "número redondo" (1, 2 ou 5 × 10ⁿ px) que dá
//!   ~64 pt na tela, com as subdivisões que couberem — o arranjo de lá;
//! - os números da régua da esquerda vão empilhados, um algarismo por linha
//!   (o GPUI não gira texto, e é o que o Photoshop faz no Windows);
//! - com a vista girada (R), a régua segue a foto sem o giro;
//! - ligada ou não fica na arrumação da área de trabalho, como lá.
//!
//! 📐 **Guias**: arrastar da régua de cima cria uma guia horizontal; da
//! esquerda, uma vertical. A ferramenta Mover (ou ⌘/Ctrl com outra) pega uma
//! guia na foto e a leva; solta fora da foto, ela sai. Cada gesto é um passo
//! ("Nova guia", "Mover guia", "Excluir guia"). Visualizar › Guias (⌘;)
//! mostra ou esconde, Travar guias (⌥⌘;) e Limpar guias. Ciano, como o
//! padrão de lá. Com a vista girada, as guias somem da tela (ficam no
//! documento) e não se criam.

use gpui_kit::{
    canvas, div, fill, point, prelude::*, px, size, AnyElement, App, Bounds, Context, Hsla, Pixels,
    SharedString, TextAlign, TextRun, Window,
};

use super::aparencia;
use super::EditorDeFoto;
use editor_core::documento::Guia;

/// A distância (em pontos) em que o ponteiro pega uma guia.
const PEGA: f32 = 4.0;

/// A distância (em pontos da tela) em que o "Ajustar" gruda.
const AJUSTE: f32 = 8.0;

/// O ciano das guias do Photoshop.
fn ciano() -> Hsla {
    gpui_kit::hsla(186.0 / 360.0, 1.0, 0.5, 1.0)
}

/// Uma guia a caminho: nova (da régua) ou levada (a de `indice`), e as
/// guias de antes do gesto (o passo sai delas).
#[derive(Clone, Debug)]
pub struct ArrastoDeGuia {
    vertical: bool,
    indice: Option<usize>,
    antes: Vec<Guia>,
}

/// A grossura das réguas, em pontos.
pub const LADO: f32 = 16.0;

/// Quanto o passo maior ocupa na tela, no mínimo, em pontos.
const PASSO_NA_TELA: f32 = 64.0;

/// A letra dos números.
const LETRA: f32 = 9.0;

/// O passo "redondo" ≥ `minimo`: 1, 2 ou 5 × 10ⁿ (nunca abaixo de 1 px).
pub fn passo_redondo(minimo: f32) -> f32 {
    let minimo = minimo.max(1.0);
    let mut base = 10f32.powf(minimo.log10().floor());
    loop {
        for m in [1.0, 2.0, 5.0] {
            if base * m >= minimo - 1e-3 {
                return base * m;
            }
        }
        base *= 10.0;
    }
}

/// Em quantas partes o passo maior se divide: a maior que deixe ≥ 4 pt
/// entre as marcas (10, 5, 2 ou nenhuma), sem passar de 1 px da foto.
pub fn subdivisoes(passo: f32, escala: f32) -> u32 {
    [10u32, 5, 2]
        .into_iter()
        .find(|&n| passo / n as f32 >= 1.0 && passo * escala / n as f32 >= 4.0)
        .unwrap_or(1)
}

impl EditorDeFoto {
    pub fn reguas_ligadas(&self) -> bool {
        self.arranjo.reguas
    }

    /// ⌘R.
    pub fn alternar_reguas(&mut self, cx: &mut Context<Self>) {
        self.arranjo.reguas = !self.arranjo.reguas;
        self.area_de_trabalho_mudou(cx);
        cx.notify();
    }

    pub fn guias_visiveis(&self) -> bool {
        !self.arranjo.guias_ocultas
    }

    pub fn guias_travadas(&self) -> bool {
        self.arranjo.guias_travadas
    }

    /// ⌘; — Visualizar › Guias.
    pub fn alternar_guias(&mut self, cx: &mut Context<Self>) {
        self.arranjo.guias_ocultas = !self.arranjo.guias_ocultas;
        self.area_de_trabalho_mudou(cx);
        cx.notify();
    }

    /// ⌥⌘; — Travar guias.
    pub fn alternar_trava_das_guias(&mut self, cx: &mut Context<Self>) {
        self.arranjo.guias_travadas = !self.arranjo.guias_travadas;
        self.area_de_trabalho_mudou(cx);
        cx.notify();
    }

    pub fn limpar_guias(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.limpar_guias();
        });
    }

    pub fn ajustar_ligado(&self) -> bool {
        !self.arranjo.sem_ajuste
    }

    /// ⇧⌘; — Visualizar › Ajustar.
    pub fn alternar_ajuste(&mut self, cx: &mut Context<Self>) {
        self.arranjo.sem_ajuste = !self.arranjo.sem_ajuste;
        self.area_de_trabalho_mudou(cx);
        cx.notify();
    }

    /// "Ajustar a" do Photoshop: o ponto (pixels da foto) gruda na guia ou
    /// na borda da foto que estiver a até 8 pontos da tela, eixo por eixo.
    /// As seleções retangular, elíptica e poligonal passam por aqui.
    pub fn ajustado_as_guias(&self, p: (f32, f32)) -> (f32, f32) {
        if self.arranjo.sem_ajuste || self.giro != 0.0 {
            return p;
        }
        let (Some((_, v)), Some(s)) = (self.vista_do_zoom(), self.sessao()) else {
            return p;
        };
        let alcance = AJUSTE / v.escala.max(1e-3);
        let (l, a) = (
            s.documento().largura() as f32,
            s.documento().altura() as f32,
        );
        let visiveis = !self.arranjo.guias_ocultas;
        let perto = |valor: f32, vertical: bool, borda: f32| {
            let mut alvos: Vec<f32> = vec![0.0, borda];
            if visiveis {
                alvos.extend(
                    s.guias()
                        .iter()
                        .filter(|g| g.vertical == vertical)
                        .map(|g| g.posicao),
                );
            }
            alvos
                .into_iter()
                .map(|alvo| (alvo, (alvo - valor).abs()))
                .filter(|(_, d)| *d <= alcance)
                .min_by(|x, y| x.1.total_cmp(&y.1))
                .map_or(valor, |(alvo, _)| alvo)
        };
        (perto(p.0, true, l), perto(p.1, false, a))
    }

    pub fn arrastando_guia(&self) -> bool {
        self.arrasto_de_guia.is_some()
    }

    /// O ponto da janela na foto, só no eixo da guia (`vertical` = coluna).
    fn posicao_da_guia(&self, ponto: gpui_kit::Point<Pixels>, vertical: bool) -> Option<f32> {
        if self.giro != 0.0 {
            return None;
        }
        let (_, v) = self.vista_do_zoom()?;
        Some(if vertical {
            (f(ponto.x - self.palco.origin.x) - v.x) / v.escala
        } else {
            (f(ponto.y - self.palco.origin.y) - v.y) / v.escala
        })
    }

    /// O apertar numa régua: uma guia nova (`vertical` = da régua da
    /// esquerda) segue o ponteiro até soltar.
    pub fn comecar_guia_da_regua(&mut self, vertical: bool, cx: &mut Context<Self>) {
        if self.giro != 0.0 || self.arranjo.guias_travadas || self.filtro_aberto().is_some() {
            return;
        }
        let Some(antes) = self.sessao().map(|s| s.guias().to_vec()) else {
            return;
        };
        // Criar mostra as guias, como lá.
        if self.arranjo.guias_ocultas {
            self.arranjo.guias_ocultas = false;
            self.area_de_trabalho_mudou(cx);
        }
        self.arrasto_de_guia = Some(ArrastoDeGuia {
            vertical,
            indice: None,
            antes,
        });
        cx.notify();
    }

    /// O apertar na foto com o Mover (ou ⌘/Ctrl): pega a guia debaixo do
    /// ponteiro. Falso quando não há guia ali.
    pub fn pegar_guia(
        &mut self,
        ponto: gpui_kit::Point<Pixels>,
        modificadores: gpui_kit::Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let com_o_mover =
            self.auxiliar == Some(super::Auxiliar::Mover) || modificadores.secondary();
        if !com_o_mover
            || self.arranjo.guias_ocultas
            || self.arranjo.guias_travadas
            || self.giro != 0.0
        {
            return false;
        }
        let Some((_, v)) = self.vista_do_zoom() else {
            return false;
        };
        let Some(guias) = self.sessao().map(|s| s.guias().to_vec()) else {
            return false;
        };
        let (px_, py_) = (
            f(ponto.x - self.palco.origin.x),
            f(ponto.y - self.palco.origin.y),
        );
        let perto = guias.iter().rposition(|g| {
            let tela = if g.vertical {
                v.x + g.posicao * v.escala - px_
            } else {
                v.y + g.posicao * v.escala - py_
            };
            tela.abs() <= PEGA
        });
        let Some(indice) = perto else {
            return false;
        };
        self.arrasto_de_guia = Some(ArrastoDeGuia {
            vertical: guias[indice].vertical,
            indice: Some(indice),
            antes: guias,
        });
        cx.notify();
        true
    }

    /// O ponteiro andou com uma guia: dentro da foto ela vai junto (no pixel
    /// inteiro); fora, some até voltar.
    pub fn arrastar_guia(&mut self, ponto: gpui_kit::Point<Pixels>, cx: &mut Context<Self>) {
        let Some(a) = self.arrasto_de_guia.clone() else {
            return;
        };
        let mut guias = a.antes.clone();
        if let Some(i) = a.indice {
            guias.remove(i);
        }
        let limite = self.sessao().map_or(0.0, |s| {
            if a.vertical {
                s.documento().largura() as f32
            } else {
                s.documento().altura() as f32
            }
        });
        if self.palco.contains(&ponto) {
            if let Some(p) = self.posicao_da_guia(ponto, a.vertical) {
                if (0.0..=limite).contains(&p) {
                    guias.push(Guia {
                        vertical: a.vertical,
                        posicao: p.round(),
                    });
                }
            }
        }
        if let Some(s) = self.sessao_mut() {
            s.guias_ao_vivo(guias);
        }
        cx.notify();
    }

    /// Soltou: um passo com o nome do gesto (ou nenhum, se nada mudou).
    pub fn soltar_guia(&mut self, cx: &mut Context<Self>) {
        let Some(a) = self.arrasto_de_guia.take() else {
            return;
        };
        if let Some(s) = self.sessao_mut() {
            let agora = s.guias().len();
            let nome = match a.indice {
                None => "Nova guia",
                Some(_) if agora < a.antes.len() => "Excluir guia",
                Some(_) => "Mover guia",
            };
            s.confirmar_guias(nome, a.antes);
        }
        cx.notify();
    }

    /// O palco com as guias por cima e as réguas em volta (desligadas, só o
    /// palco e as guias).
    pub(super) fn com_reguas(&self, palco: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let vista = self.vista_do_zoom().map(|(_, v)| v);
        let guias: Vec<Guia> = if self.arranjo.guias_ocultas || self.giro != 0.0 {
            Vec::new()
        } else {
            self.sessao()
                .map(|s| s.guias().to_vec())
                .unwrap_or_default()
        };
        let arrastando = self.arrasto_de_guia.is_some();
        let ouvinte = cx.entity();
        let por_cima = canvas(
            |_, _, _| {},
            move |bounds, _, window, _cx| {
                if let Some(v) = vista {
                    for g in &guias {
                        let r = if g.vertical {
                            let x = (v.x + g.posicao * v.escala).round();
                            Bounds::new(
                                point(bounds.left() + px(x), bounds.top()),
                                size(px(1.), bounds.size.height),
                            )
                        } else {
                            let y = (v.y + g.posicao * v.escala).round();
                            Bounds::new(
                                point(bounds.left(), bounds.top() + px(y)),
                                size(bounds.size.width, px(1.)),
                            )
                        };
                        if r.intersects(&bounds) {
                            window.paint_quad(fill(r, ciano()));
                        }
                    }
                }
                // O arrasto de uma guia é ouvido na janela: o ponteiro sai da
                // régua para a foto (e pode sair dela para excluir).
                if !arrastando {
                    return;
                }
                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |e: &gpui_kit::MouseMoveEvent, fase, _w, cx| {
                        if fase.bubble() {
                            esta.update(cx, |ed, cx| ed.arrastar_guia(e.position, cx));
                        }
                    }
                });
                window.on_mouse_event({
                    let esta = ouvinte.clone();
                    move |_: &gpui_kit::MouseUpEvent, fase, _w, cx| {
                        if fase.bubble() {
                            esta.update(cx, |ed, cx| ed.soltar_guia(cx));
                        }
                    }
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let palco = div()
            .relative()
            .flex()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .child(palco)
            .child(por_cima)
            .into_any_element();
        if !self.arranjo.reguas {
            return palco;
        }
        let Some(v) = vista else {
            return palco;
        };
        let c = aparencia::cores(cx);
        let ponteiro = self
            .ponteiro
            .filter(|p| self.palco.contains(p))
            .map(|p| (f(p.x - self.palco.origin.x), f(p.y - self.palco.origin.y)));
        let regua = |vertical: bool, cx: &mut Context<Self>| {
            let (origem, marca) = if vertical {
                (v.y, ponteiro.map(|p| p.1))
            } else {
                (v.x, ponteiro.map(|p| p.0))
            };
            let escala = v.escala;
            div()
                .id(if vertical {
                    "editor-regua-vertical"
                } else {
                    "editor-regua-horizontal"
                })
                .debug_selector(move || {
                    if vertical {
                        "editor-regua-vertical".into()
                    } else {
                        "editor-regua-horizontal".into()
                    }
                })
                .cursor(if vertical {
                    gpui_kit::CursorStyle::ResizeLeftRight
                } else {
                    gpui_kit::CursorStyle::ResizeUpDown
                })
                .on_mouse_down(
                    gpui_kit::MouseButton::Left,
                    // A régua da esquerda dá a guia vertical; a de cima, a
                    // horizontal.
                    cx.listener(move |ed, _: &gpui_kit::MouseDownEvent, window, cx| {
                        window.focus(&ed.foco, cx);
                        ed.comecar_guia_da_regua(vertical, cx);
                    }),
                )
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, cx| {
                            desenhar(bounds, vertical, origem, escala, marca, c, window, cx)
                        },
                    )
                    .size_full(),
                )
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .h(px(LADO))
                    .child(
                        div()
                            .w(px(LADO))
                            .h_full()
                            .bg(c.cromo)
                            .border_r_1()
                            .border_b_1()
                            .border_color(c.borda),
                    )
                    .child(regua(false, cx).flex_1().h_full()),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(regua(true, cx).w(px(LADO)).h_full().flex_shrink_0())
                    .child(palco),
            )
            .into_any_element()
    }
}

fn f(p: Pixels) -> f32 {
    f32::from(p)
}

#[allow(clippy::too_many_arguments)]
fn desenhar(
    bounds: Bounds<Pixels>,
    vertical: bool,
    origem: f32,
    escala: f32,
    marca: Option<f32>,
    c: aparencia::Cores,
    window: &mut Window,
    cx: &mut App,
) {
    window.paint_quad(fill(bounds, c.cromo));
    let (comprimento, grossura) = if vertical {
        (f(bounds.size.height), f(bounds.size.width))
    } else {
        (f(bounds.size.width), f(bounds.size.height))
    };
    // A borda com o palco.
    let borda = if vertical {
        Bounds::new(
            point(bounds.right() - px(1.), bounds.top()),
            size(px(1.), bounds.size.height),
        )
    } else {
        Bounds::new(
            point(bounds.left(), bounds.bottom() - px(1.)),
            size(bounds.size.width, px(1.)),
        )
    };
    window.paint_quad(fill(borda, c.borda));
    if escala <= 0.0 || comprimento <= 0.0 {
        return;
    }
    let passo = passo_redondo(PASSO_NA_TELA / escala);
    let partes = subdivisoes(passo, escala);
    let menor = passo / partes as f32;
    let primeiro = ((-origem / escala) / menor).floor() as i64;
    let ultimo = (((comprimento - origem) / escala) / menor).ceil() as i64;
    let fonte = window.text_style().font();
    let linha = |window: &mut Window, texto: SharedString, cor: Hsla| {
        let run = TextRun {
            len: texto.len(),
            font: fonte.clone(),
            color: cor,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        window
            .text_system()
            .shape_line(texto, px(LETRA), &[run], None)
    };
    for k in primeiro..=ultimo {
        let valor = k as f32 * menor;
        let pos = origem + valor * escala;
        if !(0.0..comprimento).contains(&pos) {
            continue;
        }
        let i = k.rem_euclid(partes as i64);
        let altura = if i == 0 {
            grossura
        } else if partes.is_multiple_of(2) && i == partes as i64 / 2 {
            grossura * 0.5
        } else {
            grossura * 0.25
        };
        let traco = if vertical {
            Bounds::new(
                point(bounds.right() - px(altura), bounds.top() + px(pos)),
                size(px(altura), px(1.)),
            )
        } else {
            Bounds::new(
                point(bounds.left() + px(pos), bounds.bottom() - px(altura)),
                size(px(1.), px(altura)),
            )
        };
        window.paint_quad(fill(traco, c.apagado));
        if i != 0 {
            continue;
        }
        let numero = format!("{}", valor.round() as i64);
        if vertical {
            // Um algarismo por linha, descendo da marca.
            for (j, ch) in numero.chars().enumerate() {
                let l = linha(window, ch.to_string().into(), c.texto);
                let _ = l.paint(
                    point(
                        bounds.left() + px(2.),
                        bounds.top() + px(pos + 1.0 + j as f32 * LETRA),
                    ),
                    px(LETRA),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        } else {
            let l = linha(window, numero.into(), c.texto);
            let _ = l.paint(
                point(bounds.left() + px(pos + 2.0), bounds.top() + px(1.)),
                px(LETRA),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }
    // A marca do ponteiro, de ponta a ponta.
    if let Some(m) = marca.filter(|m| (0.0..comprimento).contains(m)) {
        let r = if vertical {
            Bounds::new(
                point(bounds.left(), bounds.top() + px(m)),
                size(bounds.size.width, px(1.)),
            )
        } else {
            Bounds::new(
                point(bounds.left() + px(m), bounds.top()),
                size(px(1.), bounds.size.height),
            )
        };
        window.paint_quad(fill(r, c.alvo));
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_passo_e_redondo() {
        assert_eq!(passo_redondo(0.3), 1.0);
        assert_eq!(passo_redondo(1.0), 1.0);
        assert_eq!(passo_redondo(1.5), 2.0);
        assert_eq!(passo_redondo(27.0), 50.0);
        assert_eq!(passo_redondo(64.0), 100.0);
        assert_eq!(passo_redondo(120.0), 200.0);
        assert_eq!(passo_redondo(600.0), 1000.0);
    }

    #[test]
    fn as_subdivisoes_cabem() {
        // 100 px a 0,64 pt/px = 64 pt: 10 partes de 6,4 pt.
        assert_eq!(subdivisoes(100.0, 0.64), 10);
        // 1 px a 64 pt/px: nada abaixo de 1 px.
        assert_eq!(subdivisoes(1.0, 64.0), 1);
        // 2 px a 32 pt: duas partes de 1 px.
        assert_eq!(subdivisoes(2.0, 32.0), 2);
        // 5 px a 13 pt: 5 partes de 13 pt.
        assert_eq!(subdivisoes(5.0, 13.0), 5);
    }
}
