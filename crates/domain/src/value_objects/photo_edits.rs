use serde::{Deserialize, Serialize};
use super::CropSettings;

/// Representa o conjunto de edições aplicadas a uma foto.
/// Value Object imutável.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhotoEdits {
    pub exposure: f32,
    pub contrast: f32,
    pub temperature: f32,
    pub tint: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub clarity: f32,
    pub vibrance: f32,
    pub saturation: f32,
    
    // Tone Curve
    pub tone_curve_shadows: f32,
    pub tone_curve_darks: f32,
    pub tone_curve_lights: f32,
    pub tone_curve_highlights: f32,
    
    // HSL Saturation
    pub hsl_red_sat: f32,
    pub hsl_orange_sat: f32,
    pub hsl_yellow_sat: f32,
    pub hsl_green_sat: f32,
    pub hsl_aqua_sat: f32,
    pub hsl_blue_sat: f32,
    pub hsl_purple_sat: f32,
    pub hsl_magenta_sat: f32,
    
    // HSL Hue
    pub hsl_red_hue: f32,
    pub hsl_orange_hue: f32,
    pub hsl_yellow_hue: f32,
    pub hsl_green_hue: f32,
    pub hsl_aqua_hue: f32,
    pub hsl_blue_hue: f32,
    pub hsl_purple_hue: f32,
    pub hsl_magenta_hue: f32,
    
    // HSL Luminance
    pub hsl_red_lum: f32,
    pub hsl_orange_lum: f32,
    pub hsl_yellow_lum: f32,
    pub hsl_green_lum: f32,
    pub hsl_aqua_lum: f32,
    pub hsl_blue_lum: f32,
    pub hsl_purple_lum: f32,
    pub hsl_magenta_lum: f32,
    
    // Lens
    pub lens_distortion: f32,
    pub lens_vignette_amount: f32,
    pub lens_vignette_midpoint: f32,
    
    // Detail
    pub nr_luminance: f32,
    pub nr_color: f32,
    pub sharpen_amount: f32,
    pub sharpen_radius: f32,
    
    // Crop
    pub crop_settings: Option<CropSettings>,
}

impl Default for PhotoEdits {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 1.0, // Default contrast is 1.0 (neutral) or 0.0 depending on implementation. 
            // Checking UI State: `active_contrast: 1.0`
            temperature: 0.0,
            tint: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            clarity: 0.0,
            vibrance: 0.0,
            saturation: 0.0,
            
            tone_curve_shadows: 0.0,
            tone_curve_darks: 0.0,
            tone_curve_lights: 0.0,
            tone_curve_highlights: 0.0,
            
            hsl_red_sat: 0.0,
            hsl_orange_sat: 0.0,
            hsl_yellow_sat: 0.0,
            hsl_green_sat: 0.0,
            hsl_aqua_sat: 0.0,
            hsl_blue_sat: 0.0,
            hsl_purple_sat: 0.0,
            hsl_magenta_sat: 0.0,
            
            hsl_red_hue: 0.0,
            hsl_orange_hue: 0.0,
            hsl_yellow_hue: 0.0,
            hsl_green_hue: 0.0,
            hsl_aqua_hue: 0.0,
            hsl_blue_hue: 0.0,
            hsl_purple_hue: 0.0,
            hsl_magenta_hue: 0.0,
            
            hsl_red_lum: 0.0,
            hsl_orange_lum: 0.0,
            hsl_yellow_lum: 0.0,
            hsl_green_lum: 0.0,
            hsl_aqua_lum: 0.0,
            hsl_blue_lum: 0.0,
            hsl_purple_lum: 0.0,
            hsl_magenta_lum: 0.0,
            
            lens_distortion: 0.0,
            lens_vignette_amount: 0.0,
            lens_vignette_midpoint: 0.0,
            
            nr_luminance: 0.0,
            nr_color: 0.0,
            sharpen_amount: 0.0,
            sharpen_radius: 1.0,
            
            crop_settings: None,
        }
    }
}
