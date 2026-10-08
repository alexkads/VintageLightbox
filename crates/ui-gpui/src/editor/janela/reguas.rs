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

use gpui_kit::{
    canvas, div, fill, point, prelude::*, px, size, AnyElement, App, Bounds, Context, Hsla, Pixels,
    SharedString, TextAlign, TextRun, Window,
};

use super::aparencia;
use super::EditorDeFoto;

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

    /// O palco com as réguas em volta (ou só o palco, desligadas).
    pub(super) fn com_reguas(&self, palco: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        if !self.arranjo.reguas {
            return palco;
        }
        let Some((_, v)) = self.vista_do_zoom() else {
            return palco;
        };
        let c = aparencia::cores(cx);
        let ponteiro = self
            .ponteiro
            .filter(|p| self.palco.contains(p))
            .map(|p| (f(p.x - self.palco.origin.x), f(p.y - self.palco.origin.y)));
        let regua = |vertical: bool| {
            let (origem, marca) = if vertical {
                (v.y, ponteiro.map(|p| p.1))
            } else {
                (v.x, ponteiro.map(|p| p.0))
            };
            let escala = v.escala;
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    desenhar(bounds, vertical, origem, escala, marca, c, window, cx)
                },
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
                    .child(regua(false).flex_1().h_full()),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(regua(true).w(px(LADO)).h_full().flex_shrink_0())
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
