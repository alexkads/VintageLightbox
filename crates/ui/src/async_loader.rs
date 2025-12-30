// Async Thumbnail Loader and Image Processor Wrapper
// Delegates to infrastructure logic and handles UI-specific conversions

use std::sync::Arc;
use image::DynamicImage;
use eframe::egui::ColorImage;
use infrastructure::image_processing::async_loader::{
    AsyncImageProcessor as InfraAsyncImageProcessor,
    AsyncEditProcessor as InfraAsyncEditProcessor,
    ImageProcessRequest as InfraImageProcessRequest,
    EditRequest as InfraEditRequest,
};
use infrastructure::image_processing::histogram::HistogramData;
pub use infrastructure::image_processing::async_loader::{
    AsyncThumbnailLoader, ThumbnailRequest, ThumbnailResult,
};
use adapters::view_models::PhotoEdits;

// Re-export or redefine UI structs
// We keep flattened structs for API compatibility with UI code

/// Request to process an image (UI Version - Flattened)
pub struct ImageProcessRequest {
    pub photo_id: String,
    pub path: String,
    pub max_preview_size: u32,
    
    // Flattened PhotoEdits fields
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
    pub tone_curve_shadows: f32,
    pub tone_curve_darks: f32,
    pub tone_curve_lights: f32,
    pub tone_curve_highlights: f32,
    pub hsl_red_sat: f32,
    pub hsl_orange_sat: f32,
    pub hsl_yellow_sat: f32,
    pub hsl_green_sat: f32,
    pub hsl_aqua_sat: f32,
    pub hsl_blue_sat: f32,
    pub hsl_purple_sat: f32,
    pub hsl_magenta_sat: f32,
    pub hsl_red_hue: f32,
    pub hsl_orange_hue: f32,
    pub hsl_yellow_hue: f32,
    pub hsl_green_hue: f32,
    pub hsl_aqua_hue: f32,
    pub hsl_blue_hue: f32,
    pub hsl_purple_hue: f32,
    pub hsl_magenta_hue: f32,
    pub hsl_red_lum: f32,
    pub hsl_orange_lum: f32,
    pub hsl_yellow_lum: f32,
    pub hsl_green_lum: f32,
    pub hsl_aqua_lum: f32,
    pub hsl_blue_lum: f32,
    pub hsl_purple_lum: f32,
    pub hsl_magenta_lum: f32,
    pub lens_distortion: f32,
    pub lens_vignette_amount: f32,
    pub lens_vignette_midpoint: f32,
    pub nr_luminance: f32,
    pub nr_color: f32,
    pub sharpen_amount: f32,
    pub sharpen_radius: f32,
}

impl ImageProcessRequest {
    pub fn to_infra(self) -> InfraImageProcessRequest {
        InfraImageProcessRequest {
            photo_id: self.photo_id,
            path: self.path,
            max_preview_size: self.max_preview_size,
            edits: PhotoEdits {
                exposure: self.exposure,
                contrast: self.contrast,
                temperature: self.temperature,
                tint: self.tint,
                highlights: self.highlights,
                shadows: self.shadows,
                whites: self.whites,
                blacks: self.blacks,
                clarity: self.clarity,
                vibrance: self.vibrance,
                saturation: self.saturation,
                tone_curve_shadows: self.tone_curve_shadows,
                tone_curve_darks: self.tone_curve_darks,
                tone_curve_lights: self.tone_curve_lights,
                tone_curve_highlights: self.tone_curve_highlights,
                hsl_red_sat: self.hsl_red_sat,
                hsl_orange_sat: self.hsl_orange_sat,
                hsl_yellow_sat: self.hsl_yellow_sat,
                hsl_green_sat: self.hsl_green_sat,
                hsl_aqua_sat: self.hsl_aqua_sat,
                hsl_blue_sat: self.hsl_blue_sat,
                hsl_purple_sat: self.hsl_purple_sat,
                hsl_magenta_sat: self.hsl_magenta_sat,
                hsl_red_hue: self.hsl_red_hue,
                hsl_orange_hue: self.hsl_orange_hue,
                hsl_yellow_hue: self.hsl_yellow_hue,
                hsl_green_hue: self.hsl_green_hue,
                hsl_aqua_hue: self.hsl_aqua_hue,
                hsl_blue_hue: self.hsl_blue_hue,
                hsl_purple_hue: self.hsl_purple_hue,
                hsl_magenta_hue: self.hsl_magenta_hue,
                hsl_red_lum: self.hsl_red_lum,
                hsl_orange_lum: self.hsl_orange_lum,
                hsl_yellow_lum: self.hsl_yellow_lum,
                hsl_green_lum: self.hsl_green_lum,
                hsl_aqua_lum: self.hsl_aqua_lum,
                hsl_blue_lum: self.hsl_blue_lum,
                hsl_purple_lum: self.hsl_purple_lum,
                hsl_magenta_lum: self.hsl_magenta_lum,
                lens_distortion: self.lens_distortion,
                lens_vignette_amount: self.lens_vignette_amount,
                lens_vignette_midpoint: self.lens_vignette_midpoint,
                nr_luminance: self.nr_luminance,
                nr_color: self.nr_color,
                sharpen_amount: self.sharpen_amount,
                sharpen_radius: self.sharpen_radius,
                crop_settings: None, // Crop not handled here for now
            },
        }
    }
}

/// Result of image processing (UI Version - with ColorImage)
pub struct ImageProcessResult {
    pub photo_id: String,
    pub preview: ColorImage,
    pub original_preview: DynamicImage,
    pub processed_image: DynamicImage,
    pub histogram: HistogramData,
    pub load_time_ms: f32,
}

/// Async image processor wrapper
pub struct AsyncImageProcessor {
    infra: InfraAsyncImageProcessor,
}

impl AsyncImageProcessor {
    pub fn new(preview_manager: Arc<infrastructure::cache::preview_manager::PreviewManager>) -> Self {
        Self {
            infra: InfraAsyncImageProcessor::new(preview_manager),
        }
    }

    pub fn request_process(&self, request: ImageProcessRequest) {
        self.infra.request_process(request.to_infra());
    }

    pub fn poll_result(&self) -> Option<ImageProcessResult> {
        if let Some(res) = self.infra.poll_result() {
            // Convert to ColorImage
            let preview = crate::image_processing::ImageProcessor::dynamic_to_color_image(&res.processed_image);
            
            Some(ImageProcessResult {
                photo_id: res.photo_id,
                preview,
                original_preview: res.original_preview,
                processed_image: res.processed_image,
                histogram: res.histogram,
                load_time_ms: res.load_time_ms,
            })
        } else {
            None
        }
    }

    pub fn is_processing(&self) -> bool {
        self.infra.is_processing()
    }

    pub fn processing_photo_id(&self) -> Option<String> {
        // Need to expose this in infra or remove if unused in UI
        // Assuming UI uses it for loading indicators
        // For now, return None or update infra
        // Infra has `processing` but it's private.
        // I should stick to public API.
        None 
    }
    
    // Legacy cache methods (if needed by UI components)
    // For now, returning false as cache is internal to infra
    pub fn is_in_cache(&self, _photo_id: &str) -> bool {
        false 
    }
    
    pub fn prefetch(&self, _photo_id: String, _path: String, _max_preview_size: u32) {
         // Prefetch not exposed in infra yet
    }
}

/// Request to apply edits (UI Wrapper)
pub struct EditRequest {
    pub request_id: u64,
    pub original: DynamicImage,
    // Flattened fields again
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
    pub tone_curve_shadows: f32,
    pub tone_curve_darks: f32,
    pub tone_curve_lights: f32,
    pub tone_curve_highlights: f32,
    pub hsl_red_sat: f32,
    pub hsl_orange_sat: f32,
    pub hsl_yellow_sat: f32,
    pub hsl_green_sat: f32,
    pub hsl_aqua_sat: f32,
    pub hsl_blue_sat: f32,
    pub hsl_purple_sat: f32,
    pub hsl_magenta_sat: f32,
    pub hsl_red_hue: f32,
    pub hsl_orange_hue: f32,
    pub hsl_yellow_hue: f32,
    pub hsl_green_hue: f32,
    pub hsl_aqua_hue: f32,
    pub hsl_blue_hue: f32,
    pub hsl_purple_hue: f32,
    pub hsl_magenta_hue: f32,
    pub hsl_red_lum: f32,
    pub hsl_orange_lum: f32,
    pub hsl_yellow_lum: f32,
    pub hsl_green_lum: f32,
    pub hsl_aqua_lum: f32,
    pub hsl_blue_lum: f32,
    pub hsl_purple_lum: f32,
    pub hsl_magenta_lum: f32,
    pub lens_distortion: f32,
    pub lens_vignette_amount: f32,
    pub lens_vignette_midpoint: f32,
    pub nr_luminance: f32,
    pub nr_color: f32,
    pub sharpen_amount: f32,
    pub sharpen_radius: f32,
}

impl EditRequest {
    pub fn to_infra(self) -> InfraEditRequest {
        InfraEditRequest {
            request_id: self.request_id,
            original: self.original,
            edits: PhotoEdits {
                exposure: self.exposure,
                contrast: self.contrast,
                temperature: self.temperature,
                tint: self.tint,
                highlights: self.highlights,
                shadows: self.shadows,
                whites: self.whites,
                blacks: self.blacks,
                clarity: self.clarity,
                vibrance: self.vibrance,
                saturation: self.saturation,
                tone_curve_shadows: self.tone_curve_shadows,
                tone_curve_darks: self.tone_curve_darks,
                tone_curve_lights: self.tone_curve_lights,
                tone_curve_highlights: self.tone_curve_highlights,
                hsl_red_sat: self.hsl_red_sat,
                hsl_orange_sat: self.hsl_orange_sat,
                hsl_yellow_sat: self.hsl_yellow_sat,
                hsl_green_sat: self.hsl_green_sat,
                hsl_aqua_sat: self.hsl_aqua_sat,
                hsl_blue_sat: self.hsl_blue_sat,
                hsl_purple_sat: self.hsl_purple_sat,
                hsl_magenta_sat: self.hsl_magenta_sat,
                hsl_red_hue: self.hsl_red_hue,
                hsl_orange_hue: self.hsl_orange_hue,
                hsl_yellow_hue: self.hsl_yellow_hue,
                hsl_green_hue: self.hsl_green_hue,
                hsl_aqua_hue: self.hsl_aqua_hue,
                hsl_blue_hue: self.hsl_blue_hue,
                hsl_purple_hue: self.hsl_purple_hue,
                hsl_magenta_hue: self.hsl_magenta_hue,
                hsl_red_lum: self.hsl_red_lum,
                hsl_orange_lum: self.hsl_orange_lum,
                hsl_yellow_lum: self.hsl_yellow_lum,
                hsl_green_lum: self.hsl_green_lum,
                hsl_aqua_lum: self.hsl_aqua_lum,
                hsl_blue_lum: self.hsl_blue_lum,
                hsl_purple_lum: self.hsl_purple_lum,
                hsl_magenta_lum: self.hsl_magenta_lum,
                lens_distortion: self.lens_distortion,
                lens_vignette_amount: self.lens_vignette_amount,
                lens_vignette_midpoint: self.lens_vignette_midpoint,
                nr_luminance: self.nr_luminance,
                nr_color: self.nr_color,
                sharpen_amount: self.sharpen_amount,
                sharpen_radius: self.sharpen_radius,
                crop_settings: None,
            },
        }
    }
}

/// Result of edit processing
pub struct EditResult {
    pub request_id: u64,
    pub processed: DynamicImage,
}

pub struct AsyncEditProcessor {
    infra: InfraAsyncEditProcessor,
}

impl AsyncEditProcessor {
    pub fn new() -> Self {
        Self {
            infra: InfraAsyncEditProcessor::new(),
        }
    }

    pub fn request_edit(&self, request: EditRequest) -> u64 {
        self.infra.request_edit(request.to_infra())
    }

    pub fn next_request_id(&self) -> u64 {
        self.infra.next_request_id()
    }

    pub fn poll_result(&self) -> Option<EditResult> {
        self.infra.poll_result().map(|res| EditResult {
            request_id: res.request_id,
            processed: res.processed,
        })
    }
}

impl Default for AsyncEditProcessor {
    fn default() -> Self {
        Self::new()
    }
}
