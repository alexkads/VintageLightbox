// Async Image Loading and Processing - Simplified
//
// This module re-exports infrastructure types and provides only the essential
// egui-specific conversions. Wrapper structs have been removed to reduce duplication.

use eframe::egui::ColorImage;

// Re-export infrastructure types directly
pub use infrastructure::image_processing::async_loader::{
    AsyncImageProcessor,
    AsyncEditProcessor,
    AsyncThumbnailLoader,
    ThumbnailRequest,
    ThumbnailResult,
    ImageProcessRequest,
    EditRequest,
};

// Re-export histogram data
pub use infrastructure::image_processing::histogram::HistogramData;

// Re-export the conversion function for egui
pub use crate::adapters::egui_texture::dynamic_to_color_image;

/// Result of image processing with egui ColorImage
/// 
/// This is the only UI-specific type we need, as it includes the ColorImage
/// for egui rendering.
pub struct ImageProcessResult {
    pub photo_id: String,
    pub preview: ColorImage,
    pub original_preview: image::DynamicImage,
    pub processed_image: image::DynamicImage,
    pub histogram: HistogramData,
    pub load_time_ms: f32,
}

/// Result of edit processing
pub struct EditResult {
    pub request_id: u64,
    pub processed: image::DynamicImage,
}

/// Helper to convert infrastructure result to UI result with ColorImage
pub fn convert_image_result(
    result: infrastructure::image_processing::async_loader::ImageProcessResult
) -> ImageProcessResult {
    let preview = dynamic_to_color_image(&result.processed_image);
    ImageProcessResult {
        photo_id: result.photo_id,
        preview,
        original_preview: result.original_preview,
        processed_image: result.processed_image,
        histogram: result.histogram,
        load_time_ms: result.load_time_ms,
    }
}
