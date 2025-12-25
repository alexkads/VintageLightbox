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
    ) -> Result<(), String> {
        let photo_id = PhotoId::from_string(&id).map_err(|e| e.to_string())?;

        self.save_photo_edits_use_case.execute(
            photo_id, exposure, contrast, temperature, tint, highlights, shadows,
            whites, blacks, clarity, vibrance, saturation,
            tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
            hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
            hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat
        ).await
            .map_err(|e| e.to_string())
    }
}
