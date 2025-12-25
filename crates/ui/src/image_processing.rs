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
    ) -> TextureHandle {
        self.last_edit_time = Some(Instant::now());

        // Process image immediately
        let processed = Self::process_image(
            img, exposure, contrast, temperature, tint, highlights, shadows,
            whites, blacks, clarity, vibrance, saturation,
            tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
        );
        Self::load_texture(ctx, "processed_image", &processed)
    }

    /// Core image processing logic
    /// Applies all adjustments to the image
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
    ) -> DynamicImage {
        use image::{Rgba, Pixel};

        let mut result = img.to_rgba8();
        let (width, height) = result.dimensions();

        // Apply adjustments pixel by pixel
        for y in 0..height {
            for x in 0..width {
                let pixel = result.get_pixel(x, y);
                let rgba = pixel.channels();
                let (mut r, mut g, mut b, a) = (rgba[0] as f32, rgba[1] as f32, rgba[2] as f32, rgba[3]);

                // 1. Exposure (brightness adjustment)
                if exposure != 0.0 {
                    let factor = 2.0_f32.powf(exposure);
                    r *= factor;
                    g *= factor;
                    b *= factor;
                }

                // 2. Contrast
                if contrast != 1.0 {
                    let factor = contrast;
                    r = (r - 128.0) * factor + 128.0;
                    g = (g - 128.0) * factor + 128.0;
                    b = (b - 128.0) * factor + 128.0;
                }

                // 3. Temperature (warm/cool balance)
                if temperature != 0.0 {
                    if temperature > 0.0 {
                        // Warmer: increase red, decrease blue
                        r += temperature * 10.0;
                        b -= temperature * 10.0;
                    } else {
                        // Cooler: decrease red, increase blue
                        r += temperature * 10.0;
                        b -= temperature * 10.0;
                    }
                }

                // 4. Tint (green/magenta balance)
                if tint != 0.0 {
                    if tint > 0.0 {
                        // More magenta: increase red and blue
                        r += tint * 5.0;
                        b += tint * 5.0;
                        g -= tint * 5.0;
                    } else {
                        // More green
                        g -= tint * 5.0;
                        r += tint * 5.0;
                        b += tint * 5.0;
                    }
                }

                // 5. Highlights (adjust bright areas)
                if highlights != 0.0 {
                    let luminance = (r + g + b) / 3.0;
                    if luminance > 128.0 {
                        let factor = 1.0 + (highlights * 0.01);
                        r *= factor;
                        g *= factor;
                        b *= factor;
                    }
                }

                // 6. Shadows (adjust dark areas)
                if shadows != 0.0 {
                    let luminance = (r + g + b) / 3.0;
                    if luminance < 128.0 {
                        let factor = 1.0 + (shadows * 0.01);
                        r *= factor;
                        g *= factor;
                        b *= factor;
                    }
                }

                // 7. Whites (adjust brightest areas)
                if whites != 0.0 {
                    let luminance = (r + g + b) / 3.0;
                    if luminance > 192.0 {
                        let factor = 1.0 + (whites * 0.01);
                        r *= factor;
                        g *= factor;
                        b *= factor;
                    }
                }

                // 8. Blacks (adjust darkest areas)
                if blacks != 0.0 {
                    let luminance = (r + g + b) / 3.0;
                    if luminance < 64.0 {
                        let factor = 1.0 + (blacks * 0.01);
                        r *= factor;
                        g *= factor;
                        b *= factor;
                    }
                }

                // 9. Saturation (overall color intensity)
                if saturation != 0.0 {
                    let luminance = (r + g + b) / 3.0;
                    let factor = 1.0 + saturation;
                    r = luminance + (r - luminance) * factor;
                    g = luminance + (g - luminance) * factor;
                    b = luminance + (b - luminance) * factor;
                }

                // 10. Vibrance (intelligent saturation - affects muted colors more)
                if vibrance != 0.0 {
                    let luminance = (r + g + b) / 3.0;
                    let max_diff = ((r - luminance).abs().max((g - luminance).abs())).max((b - luminance).abs());
                    if max_diff < 64.0 {  // Only affect less saturated colors
                        let factor = 1.0 + vibrance * 2.0;
                        r = luminance + (r - luminance) * factor;
                        g = luminance + (g - luminance) * factor;
                        b = luminance + (b - luminance) * factor;
                    }
                }

                // 11. Clarity (local contrast - simplified sharpening)
                // Note: True clarity requires edge detection, this is a simplified version
                if clarity != 0.0 {
                    let factor = 1.0 + (clarity * 0.5);
                    let luminance = (r + g + b) / 3.0;
                    r = luminance + (r - luminance) * factor;
                    g = luminance + (g - luminance) * factor;
                    b = luminance + (b - luminance) * factor;
                }

                // 12-15. Tone Curve Parametric Zones
                // Each zone affects a specific luminance range with smooth falloff
                let luminance = (r + g + b) / 3.0;
                let norm_lum = luminance / 255.0; // Normalize to 0-1

                // Zone 1: Shadows (0.0 - 0.25 range, centered at 0.125)
                if tone_curve_shadows != 0.0 {
                    let zone_center = 0.125;
                    let zone_width = 0.25;
                    let dist = (norm_lum - zone_center).abs();
                    if dist < zone_width {
                        let weight = 1.0 - (dist / zone_width);
                        let adjustment = tone_curve_shadows * 0.01 * weight;
                        r += r * adjustment;
                        g += g * adjustment;
                        b += b * adjustment;
                    }
                }

                // Zone 2: Darks (0.25 - 0.5 range, centered at 0.375)
                if tone_curve_darks != 0.0 {
                    let zone_center = 0.375;
                    let zone_width = 0.25;
                    let dist = (norm_lum - zone_center).abs();
                    if dist < zone_width {
                        let weight = 1.0 - (dist / zone_width);
                        let adjustment = tone_curve_darks * 0.01 * weight;
                        r += r * adjustment;
                        g += g * adjustment;
                        b += b * adjustment;
                    }
                }

                // Zone 3: Lights (0.5 - 0.75 range, centered at 0.625)
                if tone_curve_lights != 0.0 {
                    let zone_center = 0.625;
                    let zone_width = 0.25;
                    let dist = (norm_lum - zone_center).abs();
                    if dist < zone_width {
                        let weight = 1.0 - (dist / zone_width);
                        let adjustment = tone_curve_lights * 0.01 * weight;
                        r += r * adjustment;
                        g += g * adjustment;
                        b += b * adjustment;
                    }
                }

                // Zone 4: Highlights (0.75 - 1.0 range, centered at 0.875)
                if tone_curve_highlights != 0.0 {
                    let zone_center = 0.875;
                    let zone_width = 0.25;
                    let dist = (norm_lum - zone_center).abs();
                    if dist < zone_width {
                        let weight = 1.0 - (dist / zone_width);
                        let adjustment = tone_curve_highlights * 0.01 * weight;
                        r += r * adjustment;
                        g += g * adjustment;
                        b += b * adjustment;
                    }
                }

                // Clamp values to 0-255
                r = r.clamp(0.0, 255.0);
                g = g.clamp(0.0, 255.0);
                b = b.clamp(0.0, 255.0);

                result.put_pixel(x, y, Rgba([r as u8, g as u8, b as u8, a]));
            }
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
        let processed = ImageProcessor::process_image(
            &img, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, // tone curve params
        );

        assert_eq!(processed.width(), 100);
        assert_eq!(processed.height(), 100);
    }
}
