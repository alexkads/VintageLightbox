use domain::value_objects::PhotoId;
use std::sync::Arc;
use use_cases::pos_venda::RevelacoesLocaisUseCase;
use use_cases::SavePhotoEditsUseCase;

pub struct EditorController {
    save_photo_edits_use_case: Arc<SavePhotoEditsUseCase>,
    /// O depósito das fotos que **só existem no site** — ver
    /// [`Self::guardar_revelacao_do_site`].
    revelacoes_do_site: Arc<RevelacoesLocaisUseCase>,
}

impl EditorController {
    pub fn new(
        save_photo_edits_use_case: Arc<SavePhotoEditsUseCase>,
        revelacoes_do_site: Arc<RevelacoesLocaisUseCase>,
    ) -> Self {
        Self {
            save_photo_edits_use_case,
            revelacoes_do_site,
        }
    }

    /// Guarda a receita de uma foto que **não é do catálogo desta máquina**.
    ///
    /// 🚨 **É a outra metade de [`Self::save_edits`], e existe porque aquela não
    /// tem onde escrever.** A foto aberta de uma sessão do pós-venda tem o id
    /// `site:<uuid>` e nenhuma linha em `photos` — `save_edits` respondia
    /// `PhotoNotFound` a cada gesto, calado, e a receita morria com a tela. Foi
    /// o *"os parâmetros de edição não estão sendo gravados"* de 8/set/2026.
    ///
    /// `ajustes` é o JSON que sobe para a API e volta dela: os 53 por nome mais
    /// o enquadramento com prefixo `corte_`. Um formato só nos dois sentidos.
    pub async fn guardar_revelacao_do_site(
        &self,
        foto_no_site: &str,
        ajustes: &str,
    ) -> Result<(), String> {
        self.revelacoes_do_site
            .guardar(foto_no_site, ajustes)
            .await
            .map_err(|e| e.to_string())
    }

    /// Tudo o que ficou por subir — lido de uma vez na abertura do app.
    pub async fn revelacoes_do_site(&self) -> Result<Vec<(String, String)>, String> {
        self.revelacoes_do_site
            .todas()
            .await
            .map_err(|e| e.to_string())
    }

    /// A revelação subiu: o servidor passa a ser a verdade desta foto.
    pub async fn esquecer_revelacao_do_site(&self, foto_no_site: &str) -> Result<(), String> {
        self.revelacoes_do_site
            .esquecer(foto_no_site)
            .await
            .map_err(|e| e.to_string())
    }

    // ⚠️ Dívida reconhecida, não descuido — docs/10-MIGRACAO-GPUI.md §2.1.
    //
    // A cadeia de ajustes de revelação (exposição, contraste, HSL nos 8 canais,
    // detalhe, lente…) viaja como parâmetro solto do controller até o caso de
    // uso. Agrupá-la num tipo é o conserto certo e é **outro commit**: mexe em
    // quatro camadas de uma vez, e a migração para GPUI não depende disso — o
    // plano registra explicitamente que virou dívida, e não pré-requisito.
    //
    // O `allow` fica na função, e não no crate, para que uma assinatura nova
    // longa continue sendo cobrada pelo lint.
    #[allow(clippy::too_many_arguments)]
    pub async fn save_edits(
        &self,
        id: String,
        exposure: f32,
        contrast: f32,
        temperature: f32,
        tint: f32,
        highlights: f32,
        shadows: f32,
        whites: f32,
        blacks: f32,
        clarity: f32,
        vibrance: f32,
        saturation: f32,
        tone_curve_shadows: f32,
        tone_curve_darks: f32,
        tone_curve_lights: f32,
        tone_curve_highlights: f32,
        hsl_red_sat: f32,
        hsl_orange_sat: f32,
        hsl_yellow_sat: f32,
        hsl_green_sat: f32,
        hsl_aqua_sat: f32,
        hsl_blue_sat: f32,
        hsl_purple_sat: f32,
        hsl_magenta_sat: f32,
        hsl_red_hue: f32,
        hsl_orange_hue: f32,
        hsl_yellow_hue: f32,
        hsl_green_hue: f32,
        hsl_aqua_hue: f32,
        hsl_blue_hue: f32,
        hsl_purple_hue: f32,
        hsl_magenta_hue: f32,
        hsl_red_lum: f32,
        hsl_orange_lum: f32,
        hsl_yellow_lum: f32,
        hsl_green_lum: f32,
        hsl_aqua_lum: f32,
        hsl_blue_lum: f32,
        hsl_purple_lum: f32,
        hsl_magenta_lum: f32,
        lens_distortion: f32,
        lens_vignette_amount: f32,
        lens_vignette_midpoint: f32,
        nr_luminance: f32,
        nr_color: f32,
        sharpen_amount: f32,
        sharpen_radius: f32,
        split_shadow_hue: f32,
        split_shadow_sat: f32,
        split_highlight_hue: f32,
        split_highlight_sat: f32,
        split_balance: f32,
        grain_amount: f32,
        grain_size: f32,
        crop_x: Option<f32>,
        crop_y: Option<f32>,
        crop_width: Option<f32>,
        crop_height: Option<f32>,
        crop_rotation: Option<i32>,
        crop_angle: Option<f32>,
        crop_flip_h: Option<bool>,
        crop_flip_v: Option<bool>,
    ) -> Result<(), String> {
        let photo_id = PhotoId::from_string(&id).map_err(|e| e.to_string())?;

        self.save_photo_edits_use_case
            .execute(
                photo_id,
                exposure,
                contrast,
                temperature,
                tint,
                highlights,
                shadows,
                whites,
                blacks,
                clarity,
                vibrance,
                saturation,
                tone_curve_shadows,
                tone_curve_darks,
                tone_curve_lights,
                tone_curve_highlights,
                hsl_red_sat,
                hsl_orange_sat,
                hsl_yellow_sat,
                hsl_green_sat,
                hsl_aqua_sat,
                hsl_blue_sat,
                hsl_purple_sat,
                hsl_magenta_sat,
                hsl_red_hue,
                hsl_orange_hue,
                hsl_yellow_hue,
                hsl_green_hue,
                hsl_aqua_hue,
                hsl_blue_hue,
                hsl_purple_hue,
                hsl_magenta_hue,
                hsl_red_lum,
                hsl_orange_lum,
                hsl_yellow_lum,
                hsl_green_lum,
                hsl_aqua_lum,
                hsl_blue_lum,
                hsl_purple_lum,
                hsl_magenta_lum,
                lens_distortion,
                lens_vignette_amount,
                lens_vignette_midpoint,
                nr_luminance,
                nr_color,
                sharpen_amount,
                sharpen_radius,
                split_shadow_hue,
                split_shadow_sat,
                split_highlight_hue,
                split_highlight_sat,
                split_balance,
                grain_amount,
                grain_size,
                crop_x,
                crop_y,
                crop_width,
                crop_height,
                crop_rotation,
                crop_angle,
                crop_flip_h,
                crop_flip_v,
            )
            .await
            .map_err(|e| e.to_string())
    }
}
