use super::photo::Photo;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PresetId(Uuid);

impl PresetId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PresetId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for PresetId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl std::fmt::Display for PresetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresetAdjustments {
    pub exposure: Option<f32>,
    pub contrast: Option<f32>,
    pub temperature: Option<f32>,
    pub tint: Option<f32>,
    pub highlights: Option<f32>,
    pub shadows: Option<f32>,
    pub whites: Option<f32>,
    pub blacks: Option<f32>,
    pub clarity: Option<f32>,
    pub vibrance: Option<f32>,
    pub saturation: Option<f32>,
    pub tone_curve_shadows: Option<f32>,
    pub tone_curve_darks: Option<f32>,
    pub tone_curve_lights: Option<f32>,
    pub tone_curve_highlights: Option<f32>,
}

impl From<&Photo> for PresetAdjustments {
    fn from(photo: &Photo) -> Self {
        Self {
            exposure: photo.edit_exposure(),
            contrast: photo.edit_contrast(),
            temperature: photo.edit_temperature(),
            tint: photo.edit_tint(),
            highlights: photo.edit_highlights(),
            shadows: photo.edit_shadows(),
            whites: photo.edit_whites(),
            blacks: photo.edit_blacks(),
            clarity: photo.edit_clarity(),
            vibrance: photo.edit_vibrance(),
            saturation: photo.edit_saturation(),
            tone_curve_shadows: photo.edit_tone_curve_shadows(),
            tone_curve_darks: photo.edit_tone_curve_darks(),
            tone_curve_lights: photo.edit_tone_curve_lights(),
            tone_curve_highlights: photo.edit_tone_curve_highlights(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub id: PresetId,
    pub name: String,
    pub adjustments: PresetAdjustments,
    pub is_system: bool,
}

impl Preset {
    pub fn new(name: String, adjustments: PresetAdjustments, is_system: bool) -> Self {
        Self {
            id: PresetId::new(),
            name,
            adjustments,
            is_system,
        }
    }

    pub fn system(name: &str, adjustments: PresetAdjustments) -> Self {
        Self::new(name.to_string(), adjustments, true)
    }

    pub fn user(name: String, adjustments: PresetAdjustments) -> Self {
        Self::new(name, adjustments, false)
    }
}
