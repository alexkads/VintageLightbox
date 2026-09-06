//! O motor de revelação, visto daqui: o que o `revelacao-core` tem, mais a
//! leitura da entidade.
//!
//! 🔑 **O motor saiu deste crate em 2026-09-04** para
//! [`revelacao_core`], porque o navegador precisava dele e este crate puxa
//! `sqlx`, `reqwest` e LibRaw — nada disso compila para wasm32. [`Ajustes`] e
//! [`Motor`] são re-exportados, então `ui-gpui` e o exportador continuam
//! importando daqui. O que ficou é [`ajustes_da_entidade`]: só este crate
//! conhece `domain::entities::Photo`, e `impl` inerente só o crate dono do tipo
//! pode escrever — por isso é função, e não mais `Ajustes::da_entidade`.
//!
//! A história do porquê o motor mora fora da interface, e do `uniform` de 28
//! campos, está na documentação de [`revelacao_core::ajustes`] e de
//! [`revelacao_core::motor`].

pub use revelacao_core::{Ajustes, Entrada, Motor};

/// A revelação que a foto tem gravada.
///
/// 🔑 **O padrão de campo ausente vem de [`Ajustes::default`], campo a
/// campo, e não de 46 números digitados aqui.** O legado escrevia
/// `photo.edit_exposure().unwrap_or(0.0)` quarenta e seis vezes, com o
/// neutro repetido em cada linha — e dois deles não são zero (`contrast` e
/// `sharpen_radius`). Um `unwrap_or` errado no contraste achataria a foto
/// inteira em cinza, e a suspeita cairia no motor de cor, não na leitura.
///
/// ⚠️ **`Some(0.0)` no contraste é o fotógrafo tendo arrastado até o fim**, e
/// não pode ser confundido com ausência — que é por que isto é `if let
/// Some`, e não `unwrap_or_default`.
pub fn ajustes_da_entidade(foto: &domain::entities::Photo) -> Ajustes {
    let mut ajustes = Ajustes::default();

    macro_rules! ler {
        ($($campo:ident <- $salvo:ident),* $(,)?) => {
            $(if let Some(valor) = foto.$salvo() {
                ajustes.$campo = valor;
            })*
        };
    }

    ler! {
        exposure <- edit_exposure,
        contrast <- edit_contrast,
        temperature <- edit_temperature,
        tint <- edit_tint,
        highlights <- edit_highlights,
        shadows <- edit_shadows,
        whites <- edit_whites,
        blacks <- edit_blacks,
        clarity <- edit_clarity,
        vibrance <- edit_vibrance,
        saturation <- edit_saturation,
        tone_curve_shadows <- edit_tone_curve_shadows,
        tone_curve_darks <- edit_tone_curve_darks,
        tone_curve_lights <- edit_tone_curve_lights,
        tone_curve_highlights <- edit_tone_curve_highlights,
        hsl_red_sat <- edit_hsl_red_sat,
        hsl_orange_sat <- edit_hsl_orange_sat,
        hsl_yellow_sat <- edit_hsl_yellow_sat,
        hsl_green_sat <- edit_hsl_green_sat,
        hsl_aqua_sat <- edit_hsl_aqua_sat,
        hsl_blue_sat <- edit_hsl_blue_sat,
        hsl_purple_sat <- edit_hsl_purple_sat,
        hsl_magenta_sat <- edit_hsl_magenta_sat,
        hsl_red_hue <- edit_hsl_red_hue,
        hsl_orange_hue <- edit_hsl_orange_hue,
        hsl_yellow_hue <- edit_hsl_yellow_hue,
        hsl_green_hue <- edit_hsl_green_hue,
        hsl_aqua_hue <- edit_hsl_aqua_hue,
        hsl_blue_hue <- edit_hsl_blue_hue,
        hsl_purple_hue <- edit_hsl_purple_hue,
        hsl_magenta_hue <- edit_hsl_magenta_hue,
        hsl_red_lum <- edit_hsl_red_lum,
        hsl_orange_lum <- edit_hsl_orange_lum,
        hsl_yellow_lum <- edit_hsl_yellow_lum,
        hsl_green_lum <- edit_hsl_green_lum,
        hsl_aqua_lum <- edit_hsl_aqua_lum,
        hsl_blue_lum <- edit_hsl_blue_lum,
        hsl_purple_lum <- edit_hsl_purple_lum,
        hsl_magenta_lum <- edit_hsl_magenta_lum,
        lens_distortion <- edit_lens_distortion,
        lens_vignette_amount <- edit_lens_vignette_amount,
        lens_vignette_midpoint <- edit_lens_vignette_midpoint,
        nr_luminance <- edit_nr_luminance,
        nr_color <- edit_nr_color,
        sharpen_amount <- edit_sharpen_amount,
        sharpen_radius <- edit_sharpen_radius,
        split_shadow_hue <- edit_split_shadow_hue,
        split_shadow_sat <- edit_split_shadow_sat,
        split_highlight_hue <- edit_split_highlight_hue,
        split_highlight_sat <- edit_split_highlight_sat,
        split_balance <- edit_split_balance,
        grain_amount <- edit_grain_amount,
        grain_size <- edit_grain_size,
    }

    ajustes
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::Arc;

    fn motor_pronto() -> Motor {
        Motor::abrir().expect("nenhum adaptador de GPU — o motor de revelação não roda aqui")
    }

    /// A mesma amostra dos testes do motor: rampa de cinza, as 8 cores do HSL
    /// saturadas e esmaecidas, em xadrez de 1px.
    fn amostra() -> Arc<Vec<u8>> {
        const CORES: [[u8; 3]; 8] = [
            [220, 40, 40],
            [230, 140, 30],
            [230, 220, 40],
            [40, 200, 60],
            [40, 210, 200],
            [50, 80, 220],
            [140, 50, 210],
            [220, 50, 180],
        ];
        let mut pixels = Vec::with_capacity(16 * 16 * 4);
        for y in 0u32..16 {
            for x in 0u32..16 {
                let cor = match y {
                    0..=3 => [(x * 17) as u8; 3],
                    4..=11 if (x + y) % 2 == 0 => CORES[(y - 4) as usize],
                    4..=11 => [128, 128, 128],
                    _ if (x + y) % 2 == 0 => {
                        let base = CORES[(y - 12) as usize];
                        [
                            (128 + (base[0] as i32 - 128) / 4) as u8,
                            (128 + (base[1] as i32 - 128) / 4) as u8,
                            (128 + (base[2] as i32 - 128) / 4) as u8,
                        ]
                    }
                    _ => [128, 128, 128],
                };
                pixels.extend_from_slice(&[cor[0], cor[1], cor[2], 255]);
            }
        }
        Arc::new(pixels)
    }

    fn revelar_e_colher(motor: &mut Motor, entrada: Arc<Vec<u8>>, ajustes: Ajustes) -> Vec<u8> {
        motor
            .revelar(&entrada, 16, 16, &ajustes)
            .expect("o motor não devolveu imagem")
            .into_rgba8()
            .into_raw()
    }

    /// ✅ O preset de sistema **"B&W" deixa a foto em preto e branco** — e este
    /// teste roda o valor que ele pede de verdade, não um número escolhido aqui.
    ///
    /// 🚨 **Até 30/ago/2026 ele não deixava.** O preset pedia
    /// `saturation: Some(-100.0)`, e a saturação do shader é um fator, não uma
    /// porcentagem:
    ///
    /// ```wgsl
    /// let factor = 1.0 + params.saturation;
    /// r = lum2 + (r - lum2) * factor;
    /// ```
    ///
    /// Cinza é `factor == 0`, ou seja **`-1.0`** — e é por isso que o slider de
    /// saturação vai de -1 a 1 (`controles.rs`, lido de `dock_viewer.rs`). Com
    /// `-100`, o fator é `-99`: cada canal era jogado 99 vezes para o **lado
    /// oposto** do cinza. Não era ausência de cor, era cor invertida e estourada.
    ///
    /// 🔑 **Fica neste crate, e não no `revelacao-core`**, porque junta o preset
    /// (`use-cases`) ao shader — e o core não conhece o `use-cases` de propósito.
    #[test]
    fn o_preset_bw_deixa_a_foto_em_preto_e_branco() {
        let bw = use_cases::presets::presets_de_sistema()
            .into_iter()
            .find(|preset| preset.name == "B&W")
            .expect("o preset de sistema \"B&W\" sumiu da lista");
        let saturacao = bw
            .adjustments
            .saturation
            .expect("o \"B&W\" é sobre saturação — sem ela ele não é nada");

        let mut motor = motor_pronto();
        let entrada = amostra();

        let como_o_preset_pede = revelar_e_colher(
            &mut motor,
            entrada.clone(),
            Ajustes {
                saturation: saturacao,
                ..Default::default()
            },
        );
        for pixel in como_o_preset_pede.as_chunks::<4>().0 {
            assert_eq!(
                (pixel[0], pixel[1]),
                (pixel[1], pixel[2]),
                "o \"B&W\" pede saturação {saturacao}, e isso tem de deixar os três canais iguais"
            );
        }

        let na_escala_errada = revelar_e_colher(
            &mut motor,
            entrada,
            Ajustes {
                saturation: -100.0,
                ..Default::default()
            },
        );
        assert!(
            na_escala_errada
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[0] != pixel[1] || pixel[1] != pixel[2]),
            "-100 é o fator -99, e fator -99 não é cinza: se virou, a conta do shader mudou"
        );
    }
}
