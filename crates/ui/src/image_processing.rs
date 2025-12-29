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
        let processed = Self::process_image(
            img, exposure, contrast, temperature, tint, highlights, shadows,
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
        _hsl_red_hue: f32,
        _hsl_orange_hue: f32,
        _hsl_yellow_hue: f32,
        _hsl_green_hue: f32,
        _hsl_aqua_hue: f32,
        _hsl_blue_hue: f32,
        _hsl_purple_hue: f32,
        _hsl_magenta_hue: f32,
        // HSL Lum
        _hsl_red_lum: f32,
        _hsl_orange_lum: f32,
        _hsl_yellow_lum: f32,
        _hsl_green_lum: f32,
        _hsl_aqua_lum: f32,
        _hsl_blue_lum: f32,
        _hsl_purple_lum: f32,
        _hsl_magenta_lum: f32,
        // Lens
        _lens_distortion: f32,
        _lens_vignette_amount: f32,
        _lens_vignette_midpoint: f32,
        // NR
        _nr_luminance: f32,
        _nr_color: f32,
        // Sharpening
        _sharpen_amount: f32,
        _sharpen_radius: f32,
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

                // HSL Color Channel Saturation Adjustments
                // Only apply if any HSL slider is non-zero
                let has_hsl_adjustment = hsl_red_sat != 0.0 || hsl_orange_sat != 0.0 
                    || hsl_yellow_sat != 0.0 || hsl_green_sat != 0.0
                    || hsl_aqua_sat != 0.0 || hsl_blue_sat != 0.0
                    || hsl_purple_sat != 0.0 || hsl_magenta_sat != 0.0;
                
                if has_hsl_adjustment {
                    // Normalize RGB to 0-1 range
                    let r_norm = r / 255.0;
                    let g_norm = g / 255.0;
                    let b_norm = b / 255.0;
                    
                    let max_c = r_norm.max(g_norm).max(b_norm);
                    let min_c = r_norm.min(g_norm).min(b_norm);
                    let delta = max_c - min_c;
                    
                    // Calculate hue (0-360 degrees)
                    let hue = if delta == 0.0 {
                        0.0
                    } else if max_c == r_norm {
                        60.0 * (((g_norm - b_norm) / delta) % 6.0)
                    } else if max_c == g_norm {
                        60.0 * (((b_norm - r_norm) / delta) + 2.0)
                    } else {
                        60.0 * (((r_norm - g_norm) / delta) + 4.0)
                    };
                    let hue = if hue < 0.0 { hue + 360.0 } else { hue };
                    
                    // Calculate lightness and saturation
                    let lightness = (max_c + min_c) / 2.0;
                    let sat = if delta == 0.0 {
                        0.0
                    } else {
                        delta / (1.0 - (2.0 * lightness - 1.0).abs())
                    };
                    
                    // Determine which color channel this hue belongs to
                    // and calculate adjustment based on how close to center
                    let sat_adjustment = {
                        // Color ranges in degrees (with overlap for smooth transitions)
                        // Red: 330-30 (wraps around 0)
                        // Orange: 15-45
                        // Yellow: 45-75
                        // Green: 75-165
                        // Aqua: 165-210
                        // Blue: 210-270
                        // Purple: 270-300
                        // Magenta: 300-345
                        
                        let mut adjustment = 0.0;
                        
                        // Red (wraps around 0)
                        if !(15.0..345.0).contains(&hue) {
                            let dist = if hue >= 345.0 { hue - 360.0 } else { hue };
                            let weight = 1.0 - (dist.abs() / 15.0).min(1.0);
                            adjustment += hsl_red_sat * weight;
                        }
                        // Orange: 15-45
                        if (15.0..45.0).contains(&hue) {
                            let center = 30.0;
                            let weight = 1.0 - ((hue - center).abs() / 15.0).min(1.0);
                            adjustment += hsl_orange_sat * weight;
                        }
                        // Yellow: 45-75
                        if (45.0..75.0).contains(&hue) {
                            let center = 60.0;
                            let weight = 1.0 - ((hue - center).abs() / 15.0).min(1.0);
                            adjustment += hsl_yellow_sat * weight;
                        }
                        // Green: 75-165
                        if (75.0..165.0).contains(&hue) {
                            let center = 120.0;
                            let weight = 1.0 - ((hue - center).abs() / 45.0).min(1.0);
                            adjustment += hsl_green_sat * weight;
                        }
                        // Aqua: 165-210
                        if (165.0..210.0).contains(&hue) {
                            let center = 187.5;
                            let weight = 1.0 - ((hue - center).abs() / 22.5).min(1.0);
                            adjustment += hsl_aqua_sat * weight;
                        }
                        // Blue: 210-270
                        if (210.0..270.0).contains(&hue) {
                            let center = 240.0;
                            let weight = 1.0 - ((hue - center).abs() / 30.0).min(1.0);
                            adjustment += hsl_blue_sat * weight;
                        }
                        // Purple: 270-310
                        if (270.0..310.0).contains(&hue) {
                            let center = 290.0;
                            let weight = 1.0 - ((hue - center).abs() / 20.0).min(1.0);
                            adjustment += hsl_purple_sat * weight;
                        }
                        // Magenta: 310-345
                        if (310.0..345.0).contains(&hue) {
                            let center = 327.5;
                            let weight = 1.0 - ((hue - center).abs() / 17.5).min(1.0);
                            adjustment += hsl_magenta_sat * weight;
                        }
                        
                        adjustment * 0.01 // Convert from -100..100 to -1..1
                    };
                    
                    // Apply saturation adjustment and convert back to RGB
                    if sat_adjustment != 0.0 {
                        let new_sat = (sat + sat_adjustment * sat).clamp(0.0, 1.0);
                        
                        // HSL to RGB conversion
                        let c = (1.0 - (2.0 * lightness - 1.0).abs()) * new_sat;
                        let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
                        let m = lightness - c / 2.0;
                        
                        let (r1, g1, b1) = if hue < 60.0 {
                            (c, x, 0.0)
                        } else if hue < 120.0 {
                            (x, c, 0.0)
                        } else if hue < 180.0 {
                            (0.0, c, x)
                        } else if hue < 240.0 {
                            (0.0, x, c)
                        } else if hue < 300.0 {
                            (x, 0.0, c)
                        } else {
                            (c, 0.0, x)
                        };
                        
                        r = (r1 + m) * 255.0;
                        g = (g1 + m) * 255.0;
                        b = (b1 + m) * 255.0;
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

    /// Apply crop settings to an image
    /// Handles crop region, flips, and 90-degree rotations
    pub fn apply_crop(img: &DynamicImage, crop_settings: &domain::value_objects::CropSettings) -> DynamicImage {
        let (w, h) = (img.width() as f32, img.height() as f32);
        
        // Calculate pixel coordinates from normalized values (0.0 to 1.0)
        let crop_x = (crop_settings.crop_x() * w) as u32;
        let crop_y = (crop_settings.crop_y() * h) as u32;
        let crop_w = ((crop_settings.crop_width() * w) as u32).max(1);
        let crop_h = ((crop_settings.crop_height() * h) as u32).max(1);
        
        // Clamp to image bounds
        let crop_x = crop_x.min(img.width().saturating_sub(1));
        let crop_y = crop_y.min(img.height().saturating_sub(1));
        let crop_w = crop_w.min(img.width().saturating_sub(crop_x));
        let crop_h = crop_h.min(img.height().saturating_sub(crop_y));
        
        // Crop the image
        let mut result = img.crop_imm(crop_x, crop_y, crop_w, crop_h);
        
        // Apply horizontal flip
        if crop_settings.flip_horizontal() {
            result = result.fliph();
        }
        
        // Apply vertical flip
        if crop_settings.flip_vertical() {
            result = result.flipv();
        }
        
        // Apply 90-degree rotations
        match crop_settings.rotation_90() {
            1 => result = result.rotate90(),
            2 => result = result.rotate180(),
            3 => result = result.rotate270(),
            _ => {} // 0 or other = no rotation
        }
        
        result
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
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // HSL Sat
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // HSL Hue
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // HSL Lum
            0.0, 0.0, 0.0, // Lens
            0.0, 0.0, // NR
            0.0, 1.0, // Sharpening
        );

        assert_eq!(processed.width(), 100);
        assert_eq!(processed.height(), 100);
    }
}
