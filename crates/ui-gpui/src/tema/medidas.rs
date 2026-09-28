//! As medidas de cada estilo do `/create`: alturas, respiros, cantos e letra.
//!
//! # De onde vêm
//!
//! De `apps/v4/registry/styles/style-<nome>.css` do shadcn (commit `98a1fe6`),
//! lidas classe por classe: `cn-button-size-default` é [`Medidas::botao`],
//! `cn-button-size-sm` é [`Medidas::botao_pequeno`], `cn-input` é
//! [`Medidas::campo`], e assim por diante. Uma unidade do Tailwind são 4 px.
//!
//! # 🔑 O canto é uma proporção, e não um número
//!
//! O shadcn escreve `rounded-lg`, `rounded-xl`, `rounded-4xl` — e cada um é uma
//! conta sobre o `--radius` que o eixo "raio" escolheu (`update-css-vars.ts`):
//! `sm = r·0,6`, `md = r·0,8`, `lg = r`, `xl = r·1,4`, até `4xl = r·2,6`. Por
//! isso [`Canto`] guarda a proporção, e o raio do preset decide os pixels: com
//! `raio = "none"`, até a pílula da `maia` vira quadrado — como lá.

use super::preset::Preset;

/// Um canto do shadcn: proporção do `--radius`, com teto opcional
/// (`rounded-[min(var(--radius-md),10px)]`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Canto {
    fator: f32,
    teto: Option<f32>,
}

impl Canto {
    const fn de(fator: f32) -> Self {
        Self { fator, teto: None }
    }
    const fn ate(self, teto: f32) -> Self {
        Self {
            fator: self.fator,
            teto: Some(teto),
        }
    }
    /// `rounded-none` — os estilos quadrados.
    pub const NENHUM: Self = Self::de(0.);
    pub const SM: Self = Self::de(0.6);
    pub const MD: Self = Self::de(0.8);
    pub const LG: Self = Self::de(1.);
    pub const XL: Self = Self::de(1.4);
    pub const X2: Self = Self::de(1.8);
    pub const X3: Self = Self::de(2.2);
    pub const X4: Self = Self::de(2.6);
    /// `rounded-full`: a pílula que não depende do raio.
    pub const PILULA: Self = Self::de(1000.);

    /// Em pixels, com o `--radius` do preset.
    pub fn px(self, raio: f32) -> f32 {
        let px = if self.fator >= 1000. {
            9999.
        } else {
            raio * self.fator
        };
        self.teto.map_or(px, |teto| px.min(teto))
    }
}

/// Um controle com altura fixa: botão, campo, item de menu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Controle {
    pub altura: f32,
    /// Respiro dos lados.
    pub lados: f32,
    /// Vão entre ícone e rótulo.
    pub vao: f32,
    pub canto: Canto,
    pub letra: f32,
}

/// Uma caixa: cartão, diálogo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caixa {
    /// Respiro por dentro (`p-*` ou `--card-spacing`).
    pub respiro: f32,
    /// Vão entre as partes (`gap-*`).
    pub vao: f32,
    pub canto: Canto,
    pub letra: f32,
}

/// As medidas de um estilo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Medidas {
    /// `Button` no tamanho padrão.
    pub botao: Controle,
    /// `Button size="sm"`.
    pub botao_pequeno: Controle,
    /// O lado do `Button size="icon-sm"` (o `SidebarTrigger`).
    pub botao_icone: f32,
    /// `Input` e o gatilho do `Select`.
    pub campo: Controle,
    /// `Badge`.
    pub selo: Controle,
    pub cartao: Caixa,
    pub dialogo: Caixa,
    /// O item de um menu suspenso.
    pub item_de_menu: Controle,
    /// O item do menu lateral.
    pub item_lateral: Controle,
    /// A letra de quase todo texto do painel (`text-sm`, ou `text-xs` nos
    /// estilos densos). É o `font.size` do `gpui-component`.
    pub letra: f32,
    /// O `--radius` do preset, em pixels.
    pub raio: f32,
}

const fn controle(altura: f32, lados: f32, vao: f32, canto: Canto, letra: f32) -> Controle {
    Controle {
        altura,
        lados,
        vao,
        canto,
        letra,
    }
}

const fn caixa(respiro: f32, vao: f32, canto: Canto, letra: f32) -> Caixa {
    Caixa {
        respiro,
        vao,
        canto,
        letra,
    }
}

/// O `--radius` de cada opção do eixo "raio" (`RADII`, `config.ts`).
pub fn raio_do_preset(raio: &str) -> f32 {
    match raio {
        "none" => 0.,
        "small" => 0.45 * 16.,
        "large" => 0.875 * 16.,
        // `default` é o `--radius` da cor base, que é 0,625rem em todas.
        _ => 0.625 * 16.,
    }
}

impl Medidas {
    pub fn do_preset(preset: &Preset) -> Self {
        let mut medidas = Self::do_estilo(preset.estilo);
        medidas.raio = raio_do_preset(preset.raio);
        medidas
    }

    fn do_estilo(estilo: &str) -> Self {
        use Canto as C;
        let selo = controle(20., 8., 4., C::X4, 12.);
        match estilo {
            "vega" => Self {
                botao: controle(36., 10., 6., C::MD, 14.),
                botao_pequeno: controle(32., 10., 4., C::MD.ate(10.), 14.),
                botao_icone: 32.,
                campo: controle(36., 10., 0., C::MD, 14.),
                selo,
                cartao: caixa(24., 24., C::XL, 14.),
                dialogo: caixa(24., 24., C::XL, 14.),
                item_de_menu: controle(32., 8., 8., C::SM, 14.),
                item_lateral: controle(32., 8., 8., C::MD, 14.),
                letra: 14.,
                raio: 10.,
            },
            "maia" | "luma" => {
                let luma = estilo == "luma";
                Self {
                    botao: controle(36., 12., 6., C::X4, 14.),
                    botao_pequeno: controle(32., 12., 4., C::X4, 14.),
                    botao_icone: 32.,
                    campo: controle(36., 12., 0., if luma { C::X3 } else { C::X4 }, 14.),
                    selo: Controle {
                        canto: if luma { C::X3 } else { C::X4 },
                        ..selo
                    },
                    cartao: caixa(24., 24., if luma { C::X4 } else { C::X2 }, 14.),
                    dialogo: caixa(24., 24., C::X4, 14.),
                    item_de_menu: controle(36., 12., 10., if luma { C::X2 } else { C::XL }, 14.),
                    item_lateral: controle(36., 12., 8., if luma { C::XL } else { C::LG }, 14.),
                    letra: 14.,
                    raio: 10.,
                }
            }
            "lyra" => Self {
                botao: controle(32., 10., 6., C::NENHUM, 12.),
                botao_pequeno: controle(28., 10., 4., C::NENHUM, 12.),
                botao_icone: 28.,
                campo: controle(32., 10., 0., C::NENHUM, 12.),
                selo: Controle {
                    canto: C::NENHUM,
                    ..selo
                },
                cartao: caixa(16., 16., C::NENHUM, 12.),
                dialogo: caixa(16., 16., C::NENHUM, 12.),
                item_de_menu: controle(32., 8., 8., C::NENHUM, 12.),
                item_lateral: controle(32., 8., 8., C::NENHUM, 12.),
                letra: 12.,
                raio: 0.,
            },
            "mira" => Self {
                botao: controle(28., 8., 4., C::MD, 12.),
                botao_pequeno: controle(24., 8., 4., C::MD, 12.),
                botao_icone: 24.,
                campo: controle(28., 8., 0., C::MD, 12.),
                selo: Controle {
                    canto: C::PILULA,
                    letra: 10.,
                    ..selo
                },
                cartao: caixa(16., 16., C::LG, 12.),
                dialogo: caixa(16., 16., C::XL, 12.),
                item_de_menu: controle(28., 8., 8., C::MD, 12.),
                item_lateral: controle(32., 8., 8., C::MD, 12.),
                letra: 12.,
                raio: 10.,
            },
            "rhea" => Self {
                botao: controle(32., 12., 6., C::X2, 14.),
                botao_pequeno: controle(28., 12., 4., C::X2, 14.),
                botao_icone: 28.,
                campo: controle(32., 10., 0., C::X2, 14.),
                selo: Controle {
                    canto: C::X2,
                    ..selo
                },
                cartao: caixa(20., 20., C::X4.ate(24.), 14.),
                dialogo: caixa(24., 24., C::X4.ate(24.), 14.),
                item_de_menu: controle(32., 8., 8., C::XL, 14.),
                item_lateral: controle(32., 12., 8., C::XL, 14.),
                letra: 14.,
                raio: 10.,
            },
            // ⚠️ O `sera` do shadcn escreve botão, selo e item de menu em
            // maiúsculas espaçadas (`uppercase tracking-widest`). O GPUI não
            // tem `text-transform` nem espaçamento de letra: aqui ficam as
            // medidas, e o texto como está escrito.
            "sera" => Self {
                botao: controle(40., 24., 6., C::NENHUM, 12.),
                botao_pequeno: controle(36., 16., 4., C::NENHUM, 12.),
                botao_icone: 36.,
                campo: controle(40., 0., 0., C::NENHUM, 14.),
                selo: Controle {
                    canto: C::NENHUM,
                    letra: 10.,
                    ..selo
                },
                cartao: caixa(32., 32., C::NENHUM, 14.),
                dialogo: caixa(24., 24., C::NENHUM, 14.),
                item_de_menu: controle(32., 12., 10., C::NENHUM, 12.),
                item_lateral: controle(36., 12., 8., C::NENHUM, 14.),
                letra: 14.,
                raio: 0.,
            },
            // `nova`, o estilo do site — e o que vale para nome desconhecido.
            _ => Self {
                botao: controle(32., 10., 6., C::LG, 14.),
                botao_pequeno: controle(28., 10., 4., C::MD.ate(12.), 12.8),
                botao_icone: 28.,
                campo: controle(32., 10., 0., C::LG, 14.),
                selo,
                cartao: caixa(16., 16., C::XL, 14.),
                dialogo: caixa(16., 16., C::XL, 14.),
                item_de_menu: controle(28., 6., 6., C::MD, 14.),
                item_lateral: controle(32., 8., 8., C::MD, 14.),
                letra: 14.,
                raio: 10.,
            },
        }
    }

    /// Um canto em pixels, com o raio deste preset.
    pub fn canto(&self, canto: Canto) -> f32 {
        canto.px(self.raio)
    }

    /// 🔑 **Um canto escrito em pixels numa tela**, levado ao raio do
    /// template. As telas foram desenhadas sobre o `--radius` de 10 px do
    /// site: `rounded(px(6.))` ali é o `rounded-sm`. Com o raio grande ele
    /// cresce na mesma proporção, e com `none` some — sem ninguém reescrever a
    /// tela.
    pub fn canto_da_tela(&self, px_com_raio_10: f32) -> gpui_kit::Pixels {
        gpui_kit::px(px_com_raio_10 * self.raio / 10.)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::tema::preset::{Preset, ESTILOS};

    #[test]
    fn a_escala_do_raio_e_a_do_shadcn() {
        let r = raio_do_preset("default");
        assert_eq!(r, 10.);
        let escala: Vec<f32> = [C::SM, C::MD, C::LG, C::XL, C::X2, C::X3, C::X4]
            .iter()
            .map(|c| (c.px(r) * 10.).round() / 10.)
            .collect();
        assert_eq!(escala, [6., 8., 10., 14., 18., 22., 26.]);
        assert_eq!(C::MD.ate(10.).px(raio_do_preset("large")), 10.);
        assert_eq!(C::X4.px(raio_do_preset("none")), 0., "sem raio, sem pílula");
    }
    use Canto as C;

    /// O visual da casa é o `nova`, como o site: os 32 px do botão que o app
    /// sempre teve.
    #[test]
    fn a_casa_tem_as_medidas_do_nova() {
        let casa = Medidas::do_preset(&Preset::da_casa());
        assert_eq!(casa.botao.altura, 32.);
        assert_eq!(casa.letra, 14.);
        assert_eq!(casa.canto(casa.botao.canto), 10.);
    }

    #[test]
    fn todo_estilo_tem_medidas_proprias() {
        let nova = Medidas::do_estilo("nova");
        for estilo in ESTILOS.iter().filter(|e| **e != "nova") {
            assert_ne!(
                Medidas::do_estilo(estilo),
                nova,
                "`{estilo}` caiu no padrão"
            );
        }
    }

    #[test]
    fn o_canto_da_tela_segue_o_raio() {
        let mut m = Medidas::do_preset(&Preset::da_casa());
        assert_eq!(
            m.canto_da_tela(6.),
            gpui_kit::px(6.),
            "a casa não muda nada"
        );
        m.raio = raio_do_preset("none");
        assert_eq!(m.canto_da_tela(6.), gpui_kit::px(0.));
        m.raio = raio_do_preset("large");
        assert_eq!(m.canto_da_tela(10.), gpui_kit::px(14.));
    }

    #[test]
    fn os_densos_tem_letra_menor() {
        for estilo in ["lyra", "mira"] {
            assert_eq!(Medidas::do_estilo(estilo).letra, 12., "{estilo}");
        }
    }
}
