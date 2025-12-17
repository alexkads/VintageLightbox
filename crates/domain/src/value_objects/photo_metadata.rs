//! Photo Metadata Value Object
//!
//! Contém metadados técnicos da fotografia (EXIF).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhotoMetadata {
    /// Modelo da câmera
    pub camera_model: Option<String>,
    /// Fabricante da câmera
    pub camera_make: Option<String>,
    /// Data/hora da captura original
    pub date_time: Option<String>,
    /// ISO
    pub iso: Option<u32>,
    /// Abertura (f-number)
    pub aperture: Option<f64>,
    /// Velocidade do obturador
    pub shutter_speed: Option<String>,
    /// Distância focal
    pub focal_length: Option<f64>,
    /// Largura da imagem
    pub width: Option<u32>,
    /// Altura da imagem
    pub height: Option<u32>,
}

impl Default for PhotoMetadata {
    fn default() -> Self {
        Self {
            camera_model: None,
            camera_make: None,
            date_time: None,
            iso: None,
            aperture: None,
            shutter_speed: None,
            focal_length: None,
            width: None,
            height: None,
        }
    }
}
