#![allow(dead_code)]

// Image Processing - Simplified
// 
// This module provides backward-compatible static helper methods.
// Debouncing logic has been removed (handled by infrastructure::image_processing::async_loader).
// Image processing algorithms delegate to infrastructure::image_processing::ImageAlgorithms.

use image::DynamicImage;
use infrastructure::image_processing::ImageAlgorithms;
use adapters::view_models::PhotoEdits;

// Re-export pure conversion functions from adapters
pub use crate::adapters::egui_texture::{dynamic_to_color_image, load_texture};

/// Image processor - now just a collection of static helper methods
/// 
/// The debouncing functionality has been removed as it's handled by
/// infrastructure::image_processing::async_loader.
pub struct ImageProcessor;

impl ImageProcessor {
    /// Core image processing logic - applies all adjustments
    /// 
    /// Accepts a PhotoEdits struct directly for cleaner API.
    /// Delegates to infrastructure layer.
    pub fn process_image(img: &DynamicImage, edits: &PhotoEdits) -> DynamicImage {
        ImageAlgorithms::process_image(img, edits)
    }

    /// Resize image for preview - delegates to infrastructure
    pub fn resize_for_preview(img: &DynamicImage, max_size: u32) -> DynamicImage {
        ImageAlgorithms::resize_for_preview(img, max_size)
    }

    /// Apply crop settings - delegates to infrastructure
    pub fn apply_crop(img: &DynamicImage, crop_settings: &domain::value_objects::CropSettings) -> DynamicImage {
        ImageAlgorithms::apply_crop(img, crop_settings)
    }
    
    /// Convert a DynamicImage to egui ColorImage
    /// 
    /// Delegates to adapters::egui_texture for the actual conversion.
    pub fn dynamic_to_color_image(img: &DynamicImage) -> egui::ColorImage {
        dynamic_to_color_image(img)
    }
    
    /// Create or update an egui texture from a DynamicImage
    /// 
    /// Delegates to adapters::egui_texture for the actual conversion.
    pub fn load_texture(
        ctx: &egui::Context,
        name: impl Into<String>,
        img: &DynamicImage,
    ) -> egui::TextureHandle {
        load_texture(ctx, name, img)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resize_delegation() {
        let img = DynamicImage::new_rgb8(100, 100);
        let resized = ImageProcessor::resize_for_preview(&img, 50);
        assert_eq!(resized.width(), 50);
    }
    
    #[test]
    fn test_process_image_with_photo_edits() {
        let img = DynamicImage::new_rgb8(10, 10);
        let edits = PhotoEdits::default();
        let result = ImageProcessor::process_image(&img, &edits);
        assert_eq!(result.width(), 10);
    }
}
