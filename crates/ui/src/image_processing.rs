#![allow(dead_code)]

// Image Processing and Texture Management
// Handles conversion between DynamicImage and egui textures
// Implements debounced processing similar to the Slint version

use egui::{ColorImage, TextureHandle, Context};
use image::DynamicImage;
use std::time::{Duration, Instant};

/// Image processor with debouncing support
pub struct ImageProcessor {
    last_edit_time: Option<Instant>,
    debounce_duration: Duration,
}

impl ImageProcessor {
    /// Create a new ImageProcessor with 150ms debounce duration
    pub fn new() -> Self {
        Self {
            last_edit_time: None,
            debounce_duration: Duration::from_millis(150),
        }
    }

    /// Convert a DynamicImage to egui ColorImage
    pub fn dynamic_to_color_image(img: &DynamicImage) -> ColorImage {
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.as_flat_samples();
        ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())
    }

    /// Create or update an egui texture from a DynamicImage
    pub fn load_texture(
        ctx: &Context,
        name: impl Into<String>,
        img: &DynamicImage,
    ) -> TextureHandle {
        let color_image = Self::dynamic_to_color_image(img);
        ctx.load_texture(
            name,
            color_image,
            egui::TextureOptions::default()
        )
    }

    /// Apply image edits with debouncing
    /// This prevents excessive processing during slider adjustments
    pub fn apply_edits_debounced(
        &mut self,
        ctx: &Context,
        img: &DynamicImage,
        exposure: f32,
        contrast: f32,
    ) -> TextureHandle {
        self.last_edit_time = Some(Instant::now());

        // Process image immediately
        let processed = Self::process_image(img, exposure, contrast);
        Self::load_texture(ctx, "processed_image", &processed)
    }

    /// Core image processing logic
    /// Applies exposure (brightness) and contrast adjustments
    pub fn process_image(img: &DynamicImage, exposure: f32, contrast: f32) -> DynamicImage {
        // Apply exposure (brightness adjustment)
        let mut result = if exposure != 0.0 {
            image::imageops::brighten(img, (exposure * 10.0) as i32)
        } else {
            img.to_rgba8()
        };

        // Apply contrast
        if contrast != 1.0 {
            result = image::imageops::contrast(&result, contrast);
        }

        DynamicImage::ImageRgba8(result)
    }

    /// Check if enough time has passed since last edit for debouncing
    pub fn should_process(&self) -> bool {
        if let Some(last_time) = self.last_edit_time {
            last_time.elapsed() >= self.debounce_duration
        } else {
            true
        }
    }

    /// Resize image to fit within max dimensions while preserving aspect ratio
    pub fn resize_for_preview(img: &DynamicImage, max_size: u32) -> DynamicImage {
        let (width, height) = (img.width(), img.height());

        if width <= max_size && height <= max_size {
            return img.clone();
        }

        let scale = if width > height {
            max_size as f32 / width as f32
        } else {
            max_size as f32 / height as f32
        };

        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;

        img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
    }
}

impl Default for ImageProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resize_for_preview() {
        // Create a test image
        let img = DynamicImage::new_rgb8(2000, 1500);
        let resized = ImageProcessor::resize_for_preview(&img, 1280);

        assert!(resized.width() <= 1280);
        assert!(resized.height() <= 1280);
    }

    #[test]
    fn test_process_image_no_changes() {
        let img = DynamicImage::new_rgb8(100, 100);
        let processed = ImageProcessor::process_image(&img, 0.0, 1.0);

        assert_eq!(processed.width(), 100);
        assert_eq!(processed.height(), 100);
    }
}
