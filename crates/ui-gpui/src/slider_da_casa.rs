//! 🎚️ **O slider do app, montado com as peças do `gpui_kit::base`.**
//!
//! O `Slider` do gpui-kit pinta **uma cor só** (`slider.background`) e
//! preenche **sempre da ponta esquerda**. O tema Lightroom (dono, 2026-09-28,
//! depois de aprovar a POC em WASM) pede duas coisas que ele não faz:
//!
//! - **O trilho colorido** ([`Trilho`] de `revelacao::controles`): a
//!   Temperatura vai do azul ao amarelo, cada faixa do HSL aparece na própria
//!   cor. Ali o trilho é a informação, e não há preenchimento.
//! - **O preenchimento a partir do neutro** ([`SliderDaCasa::neutro`]): a
//!   Exposição em 0 no meio da barra não pinta nada; em +1 pinta do meio até
//!   o punho.
//!
//! 🔑 **O estado é o `SliderState` do kit**, o mesmo que o `Slider` do kit
//! usa (`gpui_kit::component::slider` reexporta o do `gpui_kit::base`). Arrasto,
//! passo, clique na barra e os `SliderEvent` continuam os do kit; aqui muda só
//! o desenho. Por isso trocar `Slider::new(&e).horizontal()` por
//! [`crate::estilo::slider`] não mexe em mais nada da tela.
//!
//! As cores vêm do tema ([`tema::cores::slider`]): nos temas de antes o
//! âmbar do kit, com o trilho a 20% dele e as medidas do kit; no Lightroom o
//! cinza, com a barra fina.

use gpui_kit::base::slider::SliderState;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    div, linear_color_stop, linear_gradient, prelude::*, px, relative, App, Entity, Hsla,
    IntoElement, RenderOnce, Window,
};

use crate::revelacao::controles::Trilho;
use crate::tema;

/// Um slider horizontal com o desenho do tema.
#[derive(IntoElement)]
pub struct SliderDaCasa {
    estado: Entity<SliderState>,
    trilho: Trilho,
    neutro: Option<f32>,
    desligado: bool,
}

impl SliderDaCasa {
    pub fn new(estado: &Entity<SliderState>) -> Self {
        Self {
            estado: estado.clone(),
            trilho: Trilho::Liso,
            neutro: None,
            desligado: false,
        }
    }

    /// O desenho da barra. Com um trilho colorido não há preenchimento.
    pub fn trilho(mut self, trilho: Trilho) -> Self {
        self.trilho = trilho;
        self
    }

    /// O valor de onde o preenchimento parte. Sem ele, parte do mínimo.
    pub fn neutro(mut self, neutro: f32) -> Self {
        self.neutro = Some(neutro);
        self
    }

    pub fn disabled(mut self, desligado: bool) -> Self {
        self.desligado = desligado;
        self
    }
}

/// Onde `valor` cai na barra (0–1), numa faixa linear.
pub fn fracao(valor: f32, minimo: f32, maximo: f32) -> f32 {
    if maximo <= minimo {
        return 0.;
    }
    ((valor - minimo) / (maximo - minimo)).clamp(0., 1.)
}

/// O trecho preenchido, de `origem` ao punho, em frações da barra.
pub fn trecho_preenchido(origem: f32, punho: f32) -> (f32, f32) {
    if punho < origem {
        (punho, origem)
    } else {
        (origem, punho)
    }
}

fn hsl(graus: f32, s: f32, l: f32) -> Hsla {
    Hsla {
        h: graus.rem_euclid(360.) / 360.,
        s,
        l,
        a: 1.,
    }
}

/// As paradas de cor de um trilho, da esquerda para a direita. Vazio é
/// [`Trilho::Liso`].
pub fn paradas(trilho: Trilho) -> Vec<Hsla> {
    match trilho {
        Trilho::Liso => Vec::new(),
        Trilho::Temperatura => vec![hsl(212., 0.75, 0.55), hsl(50., 0.85, 0.55)],
        Trilho::VerdeMagenta => vec![hsl(120., 0.6, 0.45), hsl(300., 0.6, 0.55)],
        Trilho::Saturacao => vec![
            hsl(0., 0., 0.45),
            hsl(0., 0.75, 0.55),
            hsl(60., 0.75, 0.5),
            hsl(120., 0.7, 0.45),
            hsl(200., 0.75, 0.5),
            hsl(280., 0.7, 0.55),
        ],
        Trilho::Roda => (0..=6).map(|i| hsl(i as f32 * 60., 0.8, 0.52)).collect(),
        Trilho::RodaDoLightroom => (0..=12)
            .map(|i| {
                let [r, g, b] = revelacao_core::ajustes::cor_da_roda_do_lightroom(i as f32 * 30.);
                Hsla::from(gpui_kit::Rgba { r, g, b, a: 1. })
            })
            .collect(),
        Trilho::Luminancia => vec![hsl(0., 0., 0.12), hsl(0., 0., 0.88)],
        Trilho::HslSaturacao(h) => vec![hsl(h, 0., 0.45), hsl(h, 0.85, 0.52)],
        Trilho::HslLuminancia(h) => {
            vec![hsl(h, 0.6, 0.15), hsl(h, 0.8, 0.5), hsl(h, 0.8, 0.85)]
        }
        Trilho::HslMatiz(h) => vec![
            hsl(h - 40., 0.75, 0.52),
            hsl(h, 0.8, 0.52),
            hsl(h + 40., 0.75, 0.52),
        ],
    }
}

impl RenderOnce for SliderDaCasa {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (cor_do_trilho, preenchimento, punho, fino) = tema::cores::slider();
        // As medidas do `Slider` do kit (`h_1p5`, `size_4`), ou as do
        // Lightroom: barra de 4 px e punho de 11.
        let (barra, lado) = if fino { (4., 11.) } else { (6., 16.) };
        let estado = self.estado.read(cx);
        let punho_em = estado.percentage().end.clamp(0., 1.);
        let origem = self
            .neutro
            .map(|n| fracao(n, estado.min_value(), estado.max_value()))
            .unwrap_or(0.);
        let paradas = paradas(self.trilho);
        let esmaecer = |c: Hsla| {
            if self.desligado {
                c.opacity(c.a * 0.5)
            } else {
                c
            }
        };

        let mut indicador = SliderIndicator::new(&self.estado)
            .relative()
            .w_full()
            .h(px(barra))
            .rounded_full();

        if paradas.is_empty() {
            let (de, ate) = trecho_preenchido(origem, punho_em);
            indicador = indicador.bg(esmaecer(cor_do_trilho)).child(
                div()
                    .absolute()
                    .h_full()
                    .left(relative(de))
                    .right(relative(1. - ate))
                    .rounded_full()
                    .bg(esmaecer(preenchimento)),
            );
        } else {
            // 🌈 O GPUI só tem degradê de duas paradas: N paradas são N−1
            // faixas lado a lado.
            let faixas = paradas.len() - 1;
            indicador = indicador.child(div().absolute().size_full().flex().children(
                (0..faixas).map(|i| {
                    div()
                        .flex_1()
                        .h_full()
                        .when(i == 0, |d| d.rounded_l_full())
                        .when(i + 1 == faixas, |d| d.rounded_r_full())
                        .bg(linear_gradient(
                            90.,
                            linear_color_stop(esmaecer(paradas[i]), 0.),
                            linear_color_stop(esmaecer(paradas[i + 1]), 1.),
                        ))
                }),
            ));
        }

        let borda_do_punho = if fino {
            gpui_kit::rgb(0x111111).into()
        } else {
            preenchimento.opacity(0.5)
        };
        let anel = cx.theme().ring;
        let punho = SliderThumb::new(&self.estado)
            .disabled(self.desligado)
            .absolute()
            .top(px(-(lado - barra) / 2.))
            .left(relative(punho_em))
            .ml(px(-lado / 2.))
            .size(px(lado))
            .rounded_full()
            .bg(esmaecer(punho))
            .border_1()
            .border_color(esmaecer(borda_do_punho))
            .shadow_sm()
            .when(!self.desligado, |p| p.hover(move |p| p.border_color(anel)));

        BaseSlider::new(&self.estado)
            .horizontal()
            .disabled(self.desligado)
            .flex()
            .flex_1()
            .items_center()
            .w_full()
            .child(
                SliderTrack::new(&self.estado)
                    .disabled(self.desligado)
                    .flex()
                    .items_center()
                    .h(px(24.))
                    .w_full()
                    .child(indicador.child(punho)),
            )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_preenchimento_vai_do_neutro_ao_punho() {
        // Exposição −5..5: o neutro no meio.
        let origem = fracao(0., -5., 5.);
        assert_eq!(origem, 0.5);
        assert_eq!(trecho_preenchido(origem, fracao(2.5, -5., 5.)), (0.5, 0.75));
        assert_eq!(trecho_preenchido(origem, fracao(-5., -5., 5.)), (0., 0.5));
        // No neutro, nada pintado.
        let (de, ate) = trecho_preenchido(origem, origem);
        assert_eq!(de, ate);
    }

    #[test]
    fn a_fracao_nao_sai_da_barra() {
        assert_eq!(fracao(9., 0., 1.), 1.);
        assert_eq!(fracao(-9., 0., 1.), 0.);
        assert_eq!(fracao(1., 1., 1.), 0., "faixa vazia não divide por zero");
    }

    #[test]
    fn todo_trilho_colorido_tem_ao_menos_duas_paradas() {
        use crate::revelacao::controles::CONTROLES;
        for def in CONTROLES {
            let n = paradas(def.trilho).len();
            assert!(
                def.trilho == Trilho::Liso && n == 0 || n >= 2,
                "`{}`: {n} paradas",
                def.rotulo
            );
        }
    }

    /// A Temperatura começa no azul e termina no amarelo; a faixa do HSL é da
    /// própria cor no meio (ou no fim, na saturação).
    #[test]
    fn as_cores_dizem_para_onde_o_controle_leva() {
        let t = paradas(Trilho::Temperatura);
        let graus = |c: Hsla| c.h * 360.;
        assert!((200. ..240.).contains(&graus(t[0])), "começa no azul");
        assert!((40. ..65.).contains(&graus(t[1])), "termina no amarelo");
        let verde = paradas(Trilho::HslMatiz(120.));
        assert!((graus(verde[1]) - 120.).abs() < 0.5);
        let sat = paradas(Trilho::HslSaturacao(220.));
        assert_eq!(sat[0].s, 0., "começa no cinza");
        assert!((graus(sat[1]) - 220.).abs() < 0.5);
        let roda = paradas(Trilho::Roda);
        assert_eq!(
            roda.first().map(|c| c.h),
            roda.last().map(|c| c.h),
            "dá a volta"
        );
    }
}
