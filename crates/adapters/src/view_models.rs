use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PhotoViewModel {
    pub id: String,
    pub name: String,
    pub path: String,
    pub thumbnail_path: Option<String>,
    pub date: String,
    pub camera: String,
    pub exposure: String,
    pub rating: i32,
    pub color_label: Option<String>,
    pub flag: Option<i32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub edit_exposure: Option<f32>,
    pub edit_contrast: Option<f32>,
    pub edit_temperature: Option<f32>,
    pub edit_tint: Option<f32>,
    pub edit_highlights: Option<f32>,
    pub edit_shadows: Option<f32>,
    pub edit_whites: Option<f32>,
    pub edit_blacks: Option<f32>,
    pub edit_clarity: Option<f32>,
    pub edit_vibrance: Option<f32>,
    pub edit_saturation: Option<f32>,
    pub edit_tone_curve_shadows: Option<f32>,
    pub edit_tone_curve_darks: Option<f32>,
    pub edit_tone_curve_lights: Option<f32>,
    pub edit_tone_curve_highlights: Option<f32>,
    // HSL Saturation
    pub edit_hsl_red_sat: Option<f32>,
    pub edit_hsl_orange_sat: Option<f32>,
    pub edit_hsl_yellow_sat: Option<f32>,
    pub edit_hsl_green_sat: Option<f32>,
    pub edit_hsl_aqua_sat: Option<f32>,
    pub edit_hsl_blue_sat: Option<f32>,
    pub edit_hsl_purple_sat: Option<f32>,
    pub edit_hsl_magenta_sat: Option<f32>,
    // HSL Hue
    pub edit_hsl_red_hue: Option<f32>,
    pub edit_hsl_orange_hue: Option<f32>,
    pub edit_hsl_yellow_hue: Option<f32>,
    pub edit_hsl_green_hue: Option<f32>,
    pub edit_hsl_aqua_hue: Option<f32>,
    pub edit_hsl_blue_hue: Option<f32>,
    pub edit_hsl_purple_hue: Option<f32>,
    pub edit_hsl_magenta_hue: Option<f32>,
    // HSL Lum
    pub edit_hsl_red_lum: Option<f32>,
    pub edit_hsl_orange_lum: Option<f32>,
    pub edit_hsl_yellow_lum: Option<f32>,
    pub edit_hsl_green_lum: Option<f32>,
    pub edit_hsl_aqua_lum: Option<f32>,
    pub edit_hsl_blue_lum: Option<f32>,
    pub edit_hsl_purple_lum: Option<f32>,
    pub edit_hsl_magenta_lum: Option<f32>,
    // Lens
    pub edit_lens_distortion: Option<f32>,
    pub edit_lens_vignette_amount: Option<f32>,
    pub edit_lens_vignette_midpoint: Option<f32>,
    // NR
    pub edit_nr_luminance: Option<f32>,
    pub edit_nr_color: Option<f32>,
    // Sharpening
    pub edit_sharpen_amount: Option<f32>,
    pub edit_sharpen_radius: Option<f32>,
    // Crop & Rotation
    pub edit_crop_x: Option<f32>,
    pub edit_crop_y: Option<f32>,
    pub edit_crop_width: Option<f32>,
    pub edit_crop_height: Option<f32>,
    pub edit_crop_rotation: Option<i32>,
    pub edit_crop_angle: Option<f32>,
    pub edit_crop_flip_h: Option<bool>,
    pub edit_crop_flip_v: Option<bool>,
    pub edit_crop_fill_mode: Option<u8>,
}

/// ViewModel for import preview items
#[derive(Debug, Clone)]
pub struct ImportPreviewItemViewModel {
    pub file_path: String,
    pub thumbnail_data: Vec<u8>,
    pub file_size: u64,
    pub is_raw: bool,
    pub camera: String,
    pub date_time: String,
    pub dimensions: Option<String>,
}

/// ViewModel for duplicate check results
#[derive(Debug, Clone)]
pub struct DuplicateCheckViewModel {
    pub file_path: String,
    pub content_hash: String,
    pub is_duplicate: bool,
    pub existing_photo_path: Option<String>,
}

/// ViewModel for import progress events
#[derive(Debug, Clone)]
pub enum ImportProgressViewModel {
    Starting { total: usize },
    Processing { index: usize, path: String },
    Completed { photo_id: String, path: String },
    Failed { path: String, error: String },
    DuplicateSkipped { path: String, existing_path: String },
    Paused { completed: usize, remaining: usize },
    Finished { successful: usize, failed: usize, skipped: usize },
}
