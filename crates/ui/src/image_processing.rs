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

    /// Resize image for fast preview (max 1920x1080)
    /// This dramatically speeds up loading by reducing texture size
    pub fn resize_for_preview(img: &DynamicImage) -> DynamicImage {
        let (width, height) = (img.width(), img.height());
        
        // Only resize if image is larger than preview size
        const MAX_WIDTH: u32 = 1920;
        const MAX_HEIGHT: u32 = 1080;
        
        if width <= MAX_WIDTH && height <= MAX_HEIGHT {
            return img.clone();
        }
        
        // Calculate scale to fit within max dimensions
        let width_scale = MAX_WIDTH as f32 / width as f32;
        let height_scale = MAX_HEIGHT as f32 / height as f32;
        let scale = width_scale.min(height_scale);
        
        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;
        
        img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
    }

    /// Process image with exposure and contrast adjustments
    pub fn process_image(img: &DynamicImage, exposure: f32, contrast: f32) -> DynamicImage {
        let mut rgba = img.to_rgba8();
        
        // Apply exposure (brightness adjustment)
        if exposure != 0.0 {
            let exposure_factor = 2.0_f32.powf(exposure);
            for pixel in rgba.pixels_mut() {
                pixel[0] = (pixel[0] as f32 * exposure_factor).min(255.0) as u8;
                pixel[1] = (pixel[1] as f32 * exposure_factor).min(255.0) as u8;
                pixel[2] = (pixel[2] as f32 * exposure_factor).min(255.0) as u8;
            }
        }
        
        // Apply contrast
        if contrast != 1.0 {
            let factor = contrast;
            for pixel in rgba.pixels_mut() {
                pixel[0] = ((pixel[0] as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8;
                pixel[1] = ((pixel[1] as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8;
                pixel[2] = ((pixel[2] as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8;
            }
        }
        
        DynamicImage::ImageRgba8(rgba)
    }

    /// Check if enough time has passed since last edit for debouncing
    pub fn should_process(&self) -> bool {
        if let Some(last_time) = self.last_edit_time {
            last_time.elapsed() >= self.debounce_duration
        } else {
            true
        }
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
    use image::{DynamicImage, RgbaImage, GenericImageView};

    #[test]
    fn test_process_image_no_changes() {
        let img = DynamicImage::ImageRgba8(RgbaImage::new(100, 100));
        let processed = ImageProcessor::process_image(&img, 0.0, 1.0);
        assert_eq!(img.dimensions(), processed.dimensions());
    }

    #[test]
    fn test_resize_for_preview() {
        // Test with large image
        let large_img = DynamicImage::ImageRgba8(RgbaImage::new(4000, 3000));
        let preview = ImageProcessor::resize_for_preview(&large_img);
        
        // Should be resized to fit within 1920x1080
        assert!(preview.width() <= 1920);
        assert!(preview.height() <= 1080);
        
        // Test with small image (should not resize)
        let small_img = DynamicImage::ImageRgba8(RgbaImage::new(800, 600));
        let preview = ImageProcessor::resize_for_preview(&small_img);
        assert_eq!(preview.dimensions(), (800, 600));
    }
}
