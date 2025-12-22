use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
