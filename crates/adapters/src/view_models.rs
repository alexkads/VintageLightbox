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
}
