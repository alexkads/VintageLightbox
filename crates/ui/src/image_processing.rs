#![allow(dead_code)]

// Image Processing and Texture Management
// Handles conversion between DynamicImage and egui textures
// Implements debounced processing similar to the Slint version

use egui::{ColorImage, TextureHandle, Context};
use image::DynamicImage;
use std::time::{Duration, Instant};
use infrastructure::image_processing::ImageAlgorithms;
use domain::value_objects::PhotoEdits;

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
    #[allow(clippy::too_many_arguments)]
    pub fn apply_edits_debounced(
        &mut self,
        ctx: &Context,
        img: &DynamicImage,
        exposure: f32,
        contrast: f32,
        temperature: f32,
        tint: f32,
        highlights: f32,
        shadows: f32,
        whites: f32,
        blacks: f32,
        clarity: f32,
        vibrance: f32,
        saturation: f32,
        tone_curve_shadows: f32,
        tone_curve_darks: f32,
        tone_curve_lights: f32,
        tone_curve_highlights: f32,
        hsl_red_sat: f32,
        hsl_orange_sat: f32,
        hsl_yellow_sat: f32,
        hsl_green_sat: f32,
        hsl_aqua_sat: f32,
        hsl_blue_sat: f32,
        hsl_purple_sat: f32,
        hsl_magenta_sat: f32,
        // HSL Hue
        hsl_red_hue: f32,
        hsl_orange_hue: f32,
        hsl_yellow_hue: f32,
        hsl_green_hue: f32,
        hsl_aqua_hue: f32,
        hsl_blue_hue: f32,
        hsl_purple_hue: f32,
        hsl_magenta_hue: f32,
        // HSL Lum
        hsl_red_lum: f32,
        hsl_orange_lum: f32,
        hsl_yellow_lum: f32,
        hsl_green_lum: f32,
        hsl_aqua_lum: f32,
        hsl_blue_lum: f32,
        hsl_purple_lum: f32,
        hsl_magenta_lum: f32,
        // Lens
        lens_distortion: f32,
        lens_vignette_amount: f32,
        lens_vignette_midpoint: f32,
        // NR
        nr_luminance: f32,
        nr_color: f32,
        // Sharpening
        sharpen_amount: f32,
        sharpen_radius: f32,
    ) -> TextureHandle {
        self.last_edit_time = Some(Instant::now());

        // Process image immediately
        // Map parameters to parameters method in infrastructure
        // Alternatively, construct PhotoEdits (cleaner)
        let edits = PhotoEdits {
            exposure, contrast, temperature, tint, highlights, shadows,
            whites, blacks, clarity, vibrance, saturation,
            tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
            hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
            hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
            hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue,
            hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
            hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum,
            hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
            lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
            nr_luminance, nr_color,
            sharpen_amount, sharpen_radius,
            crop_settings: None, // Not applied here
        };

        let processed = ImageAlgorithms::process_image(img, &edits);
        Self::load_texture(ctx, "processed_image", &processed)
    }

    /// Core image processing logic
    /// Applies all adjustments to the image
    /// Delegates to infrastructure layer
    #[allow(clippy::too_many_arguments)]
    pub fn process_image(
        img: &DynamicImage,
        exposure: f32,
        contrast: f32,
        temperature: f32,
        tint: f32,
        highlights: f32,
        shadows: f32,
        whites: f32,
        blacks: f32,
        clarity: f32,
        vibrance: f32,
        saturation: f32,
        // Tone curve parametric zones
        tone_curve_shadows: f32,
        tone_curve_darks: f32,
        tone_curve_lights: f32,
        tone_curve_highlights: f32,
        // HSL color channel saturations (-100 to +100)
        hsl_red_sat: f32,
        hsl_orange_sat: f32,
        hsl_yellow_sat: f32,
        hsl_green_sat: f32,
        hsl_aqua_sat: f32,
        hsl_blue_sat: f32,
        hsl_purple_sat: f32,
        hsl_magenta_sat: f32,
        // HSL Hue
        hsl_red_hue: f32,
        hsl_orange_hue: f32,
        hsl_yellow_hue: f32,
        hsl_green_hue: f32,
        hsl_aqua_hue: f32,
        hsl_blue_hue: f32,
        hsl_purple_hue: f32,
        hsl_magenta_hue: f32,
        // HSL Lum
        hsl_red_lum: f32,
        hsl_orange_lum: f32,
        hsl_yellow_lum: f32,
        hsl_green_lum: f32,
        hsl_aqua_lum: f32,
        hsl_blue_lum: f32,
        hsl_purple_lum: f32,
        hsl_magenta_lum: f32,
        // Lens
        lens_distortion: f32,
        lens_vignette_amount: f32,
        lens_vignette_midpoint: f32,
        // NR
        nr_luminance: f32,
        nr_color: f32,
        // Sharpening
        sharpen_amount: f32,
        sharpen_radius: f32,
    ) -> DynamicImage {
        let edits = PhotoEdits {
            exposure, contrast, temperature, tint, highlights, shadows,
            whites, blacks, clarity, vibrance, saturation,
            tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
            hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
            hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
            hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue,
            hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
            hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum,
            hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
            lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
            nr_luminance, nr_color,
            sharpen_amount, sharpen_radius,
            crop_settings: None,
        };

        ImageAlgorithms::process_image(img, &edits)
    }

    /// Check if enough time has passed since last edit for debouncing
    pub fn should_process(&self) -> bool {
        if let Some(last_time) = self.last_edit_time {
            last_time.elapsed() >= self.debounce_duration
        } else {
            true
        }
    }

    /// Resize image for preview - delegates to infrastructure
    pub fn resize_for_preview(img: &DynamicImage, max_size: u32) -> DynamicImage {
        ImageAlgorithms::resize_for_preview(img, max_size)
    }

    /// Apply crop settings - delegates to infrastructure
    pub fn apply_crop(img: &DynamicImage, crop_settings: &domain::value_objects::CropSettings) -> DynamicImage {
        ImageAlgorithms::apply_crop(img, crop_settings)
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
    fn test_resize_delegation() {
        let img = DynamicImage::new_rgb8(100, 100);
        let resized = ImageProcessor::resize_for_preview(&img, 50);
        assert_eq!(resized.width(), 50);
    }
}
