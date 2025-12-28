use std::sync::Arc;
use domain::value_objects::PhotoId;
use use_cases::SavePhotoEditsUseCase;

pub struct EditorController {
    save_photo_edits_use_case: Arc<SavePhotoEditsUseCase>,
}

impl EditorController {
    pub fn new(save_photo_edits_use_case: Arc<SavePhotoEditsUseCase>) -> Self {
        Self { save_photo_edits_use_case }
    }

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
        crop_x: Option<f32>,
        crop_y: Option<f32>,
        crop_width: Option<f32>,
        crop_height: Option<f32>,
        crop_rotation: Option<i32>,
        crop_angle: Option<f32>,
        crop_flip_h: Option<bool>,
        crop_flip_v: Option<bool>,
        crop_fill_mode: Option<u8>,
    ) -> Result<(), String> {
        let photo_id = PhotoId::from_string(&id).map_err(|e| e.to_string())?;

        self.save_photo_edits_use_case.execute(
            photo_id, exposure, contrast, temperature, tint, highlights, shadows,
            whites, blacks, clarity, vibrance, saturation,
            tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
            hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
            hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
            hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue,
            hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
            hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum,
            hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
            lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
            nr_luminance, nr_color,

            sharpen_amount, sharpen_radius,
            crop_x,
            crop_y,
            crop_width,
            crop_height,
            crop_rotation,
            crop_angle,
            crop_flip_h,
            crop_flip_v,
            crop_fill_mode
        ).await
            .map_err(|e| e.to_string())
    }
}
