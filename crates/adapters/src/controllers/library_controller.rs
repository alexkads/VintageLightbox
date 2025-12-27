use std::sync::Arc;
use domain::repositories::PhotoRepository;
use crate::view_models::PhotoViewModel;

pub struct LibraryController {
    photo_repository: Arc<dyn PhotoRepository>,
}

impl LibraryController {
    pub fn new(photo_repository: Arc<dyn PhotoRepository>) -> Self {
        Self { photo_repository }
    }

    pub async fn get_all_photos(&self) -> Result<Vec<PhotoViewModel>, String> {
        let photos = self.photo_repository.find_all().await
            .map_err(|e| e.to_string())?;

        let view_models = photos.into_iter().map(|photo| {
                    let metadata = photo.metadata();
                    let date = metadata.and_then(|m| m.date_time.clone()).unwrap_or_default();
                    let camera = metadata.and_then(|m| m.camera_model.clone()).unwrap_or_default();
                    let exposure = if let Some(meta) = metadata {
                        format!("ISO {} {} {}", 
                            meta.iso.unwrap_or(0), 
                            meta.aperture.map(|f| format!("f/{:.1}", f)).unwrap_or_default(), 
                            meta.shutter_speed.clone().unwrap_or_default()
                        ).trim().to_string()
                    } else {
                        String::new()
                    };
                    let rating = photo.rating().map(|r| r.value() as i32).unwrap_or(0);

                    PhotoViewModel {
                        id: photo.id().to_string(),
                        name: photo.file_name().unwrap_or_default().to_string(),
                        path: photo.file_path().to_string(),
                        thumbnail_path: photo.thumbnail_path().map(|p| p.to_string()),
                        date,
                        camera,
                        exposure,
                        rating,
                        color_label: photo.color_label().map(|c| c.to_string()),
                        flag: photo.flag().map(|f| f.as_code()),
                        width: metadata.and_then(|m| m.width),
                        height: metadata.and_then(|m| m.height),
                        edit_exposure: photo.edit_exposure(),
                        edit_contrast: photo.edit_contrast(),
                        edit_temperature: photo.edit_temperature(),
                        edit_tint: photo.edit_tint(),
                        edit_highlights: photo.edit_highlights(),
                        edit_shadows: photo.edit_shadows(),
                        edit_whites: photo.edit_whites(),
                        edit_blacks: photo.edit_blacks(),
                        edit_clarity: photo.edit_clarity(),
                        edit_vibrance: photo.edit_vibrance(),
                        edit_saturation: photo.edit_saturation(),
                        edit_tone_curve_shadows: photo.edit_tone_curve_shadows(),
                        edit_tone_curve_darks: photo.edit_tone_curve_darks(),
                        edit_tone_curve_lights: photo.edit_tone_curve_lights(),
                        edit_tone_curve_highlights: photo.edit_tone_curve_highlights(),
                        // HSL Saturation
                        edit_hsl_red_sat: photo.edit_hsl_red_sat(),
                        edit_hsl_orange_sat: photo.edit_hsl_orange_sat(),
                        edit_hsl_yellow_sat: photo.edit_hsl_yellow_sat(),
                        edit_hsl_green_sat: photo.edit_hsl_green_sat(),
                        edit_hsl_aqua_sat: photo.edit_hsl_aqua_sat(),
                        edit_hsl_blue_sat: photo.edit_hsl_blue_sat(),
                        edit_hsl_purple_sat: photo.edit_hsl_purple_sat(),
                        edit_hsl_magenta_sat: photo.edit_hsl_magenta_sat(),
                        // HSL Hue
                        edit_hsl_red_hue: photo.edit_hsl_red_hue(),
                        edit_hsl_orange_hue: photo.edit_hsl_orange_hue(),
                        edit_hsl_yellow_hue: photo.edit_hsl_yellow_hue(),
                        edit_hsl_green_hue: photo.edit_hsl_green_hue(),
                        edit_hsl_aqua_hue: photo.edit_hsl_aqua_hue(),
                        edit_hsl_blue_hue: photo.edit_hsl_blue_hue(),
                        edit_hsl_purple_hue: photo.edit_hsl_purple_hue(),
                        edit_hsl_magenta_hue: photo.edit_hsl_magenta_hue(),
                        // HSL Lum
                        edit_hsl_red_lum: photo.edit_hsl_red_lum(),
                        edit_hsl_orange_lum: photo.edit_hsl_orange_lum(),
                        edit_hsl_yellow_lum: photo.edit_hsl_yellow_lum(),
                        edit_hsl_green_lum: photo.edit_hsl_green_lum(),
                        edit_hsl_aqua_lum: photo.edit_hsl_aqua_lum(),
                        edit_hsl_blue_lum: photo.edit_hsl_blue_lum(),
                        edit_hsl_purple_lum: photo.edit_hsl_purple_lum(),
                        edit_hsl_magenta_lum: photo.edit_hsl_magenta_lum(),
                        // Lens
                        edit_lens_distortion: photo.edit_lens_distortion(),
                        edit_lens_vignette_amount: photo.edit_lens_vignette_amount(),
                        edit_lens_vignette_midpoint: photo.edit_lens_vignette_midpoint(),
                        // NR
                        edit_nr_luminance: photo.edit_nr_luminance(),
                        edit_nr_color: photo.edit_nr_color(),
                        // Sharpening
                        edit_sharpen_amount: photo.edit_sharpen_amount(),
                        edit_sharpen_radius: photo.edit_sharpen_radius(),
                    }
                })
                .collect();

        Ok(view_models)
    }
}
