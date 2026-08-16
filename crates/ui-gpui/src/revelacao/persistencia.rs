//! O que a foto já traz do banco, virando [`Ajustes`].
//!
//! Sem isto a Revelação abre toda foto no neutro — inclusive as que já foram
//! reveladas. O sintoma não é "faltou uma tela": é o trabalho do fotógrafo
//! sumindo da vista, com a foto na frente dele mostrando o arquivo cru.
//!
//! ## 🔑 O neutro vem de `Ajustes::default`, não de números escritos aqui
//!
//! O legado faz `photo.edit_exposure.unwrap_or(0.0)` campo a campo — 46
//! `unwrap_or` com o padrão digitado em cada um. Dois deles não são zero
//! (`contrast` e `sharpen_radius`), e num terceiro o legado se contradiz: o meio
//! da vinheta tem **quatro** declarações de neutro espalhadas, contando a coluna
//! do banco. Aqui o padrão é sempre o campo correspondente de
//! [`Ajustes::default`]: um lugar só dizendo qual é o neutro, que é a mesma regra
//! que [`super::controles::Definicao::neutro`] segue para os sliders.

use adapters::view_models::PhotoViewModel;

use super::processador::Ajustes;

/// Lê os ajustes gravados na foto. Campo ausente fica no neutro.
///
/// ⚠️ **Ausente não é zero, e não é "nunca revelada".** O legado grava os 46 de
/// uma vez, então uma foto ou tem todos ou não tem nenhum — mas ler campo a campo
/// é o que sobrevive a um `NULL` solto no banco, que nenhum dos dois apps sabe
/// produzir hoje e o SQLite aceita sem reclamar.
pub fn da_foto(foto: &PhotoViewModel) -> Ajustes {
    let mut ajustes = Ajustes::default();

    macro_rules! ler {
        ($($campo:ident <- $salvo:ident),* $(,)?) => {
            $(if let Some(valor) = foto.$salvo {
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
    }

    ajustes
}

#[cfg(test)]
mod testes {
    use super::*;

    fn foto() -> PhotoViewModel {
        PhotoViewModel {
            id: "id-retrato.jpg".into(),
            name: "retrato.jpg".into(),
            ..Default::default()
        }
    }

    #[test]
    fn foto_sem_edicao_nenhuma_da_o_neutro() {
        assert_eq!(da_foto(&foto()), Ajustes::default());
    }

    #[test]
    fn o_que_esta_gravado_chega_aos_ajustes() {
        let salva = PhotoViewModel {
            edit_exposure: Some(1.5),
            edit_contrast: Some(1.2),
            edit_hsl_blue_lum: Some(-40.0),
            ..foto()
        };

        let ajustes = da_foto(&salva);
        assert_eq!(ajustes.exposure, 1.5);
        assert_eq!(ajustes.contrast, 1.2);
        assert_eq!(ajustes.hsl_blue_lum, -40.0);
        assert_eq!(
            ajustes.saturation, 0.0,
            "o que não foi gravado fica no neutro"
        );
    }

    /// 🚨 Campo ausente cai no **neutro do campo**, e não em zero.
    ///
    /// O contraste é o caso que pega: `None` virando `0.0` é contraste zero, que
    /// achata a foto inteira em cinza. Numa foto que tenha exposição gravada e
    /// contraste não — o que acontece se um dia algum caminho gravar parcialmente
    /// — a foto abriria cinza, e a suspeita cairia no motor de cor.
    #[test]
    fn campo_ausente_cai_no_neutro_dele_e_nao_em_zero() {
        let ajustes = da_foto(&PhotoViewModel {
            edit_exposure: Some(0.5),
            ..foto()
        });

        assert_eq!(ajustes.contrast, 1.0);
        assert_eq!(ajustes.sharpen_radius, 1.0);
    }

    /// 🔑 Um valor gravado que **é** o zero não pode ser confundido com ausência.
    ///
    /// `Some(0.0)` num campo cujo neutro é 1.0 tem de virar 0.0 — é o fotógrafo
    /// tendo arrastado o contraste até o fim, e não o banco sem resposta. Um
    /// `unwrap_or_default` fora de lugar, ou um `if valor != 0.0`, apagaria isso.
    #[test]
    fn zero_gravado_e_diferente_de_campo_ausente() {
        let ajustes = da_foto(&PhotoViewModel {
            edit_contrast: Some(0.0),
            ..foto()
        });

        assert_eq!(ajustes.contrast, 0.0);
    }

    /// Os 46 campos estão na macro — nenhum ficou de fora na cópia.
    ///
    /// 🚨 Um campo esquecido não falha: ele simplesmente nunca volta do banco, e
    /// a foto abre com aquele ajuste no neutro. Com 46 nomes parecidos
    /// (`hsl_blue_lum` e `hsl_blue_sat` diferem em três letras), esquecer um é o
    /// erro provável — e o sintoma seria "o app novo perdeu meu HSL", meses
    /// depois.
    #[test]
    fn todos_os_46_campos_voltam_do_banco() {
        // Uma foto com **tudo** gravado num valor que não é o neutro de nenhum
        // campo, e a conferência de que nenhum sobrou no neutro.
        const MARCA: f32 = 7.25;
        let salva = PhotoViewModel {
            edit_exposure: Some(MARCA),
            edit_contrast: Some(MARCA),
            edit_temperature: Some(MARCA),
            edit_tint: Some(MARCA),
            edit_highlights: Some(MARCA),
            edit_shadows: Some(MARCA),
            edit_whites: Some(MARCA),
            edit_blacks: Some(MARCA),
            edit_clarity: Some(MARCA),
            edit_vibrance: Some(MARCA),
            edit_saturation: Some(MARCA),
            edit_tone_curve_shadows: Some(MARCA),
            edit_tone_curve_darks: Some(MARCA),
            edit_tone_curve_lights: Some(MARCA),
            edit_tone_curve_highlights: Some(MARCA),
            edit_hsl_red_sat: Some(MARCA),
            edit_hsl_orange_sat: Some(MARCA),
            edit_hsl_yellow_sat: Some(MARCA),
            edit_hsl_green_sat: Some(MARCA),
            edit_hsl_aqua_sat: Some(MARCA),
            edit_hsl_blue_sat: Some(MARCA),
            edit_hsl_purple_sat: Some(MARCA),
            edit_hsl_magenta_sat: Some(MARCA),
            edit_hsl_red_hue: Some(MARCA),
            edit_hsl_orange_hue: Some(MARCA),
            edit_hsl_yellow_hue: Some(MARCA),
            edit_hsl_green_hue: Some(MARCA),
            edit_hsl_aqua_hue: Some(MARCA),
            edit_hsl_blue_hue: Some(MARCA),
            edit_hsl_purple_hue: Some(MARCA),
            edit_hsl_magenta_hue: Some(MARCA),
            edit_hsl_red_lum: Some(MARCA),
            edit_hsl_orange_lum: Some(MARCA),
            edit_hsl_yellow_lum: Some(MARCA),
            edit_hsl_green_lum: Some(MARCA),
            edit_hsl_aqua_lum: Some(MARCA),
            edit_hsl_blue_lum: Some(MARCA),
            edit_hsl_purple_lum: Some(MARCA),
            edit_hsl_magenta_lum: Some(MARCA),
            edit_lens_distortion: Some(MARCA),
            edit_lens_vignette_amount: Some(MARCA),
            edit_lens_vignette_midpoint: Some(MARCA),
            edit_nr_luminance: Some(MARCA),
            edit_nr_color: Some(MARCA),
            edit_sharpen_amount: Some(MARCA),
            edit_sharpen_radius: Some(MARCA),
            ..foto()
        };

        let ajustes = da_foto(&salva);
        let campos: &[f32] = bytemuck::cast_slice(bytemuck::bytes_of(&ajustes));

        assert_eq!(campos.len(), 46);
        for (i, valor) in campos.iter().enumerate() {
            assert_eq!(
                *valor, MARCA,
                "o campo {i} não foi lido do banco — falta uma linha na macro `ler!`"
            );
        }
    }
}
