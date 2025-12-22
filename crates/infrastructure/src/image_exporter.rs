use async_trait::async_trait;
use domain::services::ImageExporter;
use domain::entities::Photo;
use domain::value_objects::FilePath;
use domain::{DomainError, DomainResult};
use std::path::Path;
use image::{DynamicImage, Rgba, Pixel};

pub struct ImageExporterImpl;

impl ImageExporterImpl {
    pub fn new() -> Self {
        Self
    }

    /// Apply all 11 adjustments to an image
    /// This mirrors the logic in crates/ui/src/image_processing.rs
    fn process_image(
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
    ) -> DynamicImage {
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
                    r += temperature * 10.0;
                    b -= temperature * 10.0;
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

                // 11. Clarity (local contrast - simplified)
                if clarity != 0.0 {
                    let factor = 1.0 + (clarity * 0.5);
                    let luminance = (r + g + b) / 3.0;
                    r = luminance + (r - luminance) * factor;
                    g = luminance + (g - luminance) * factor;
                    b = luminance + (b - luminance) * factor;
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
}

#[async_trait]
impl ImageExporter for ImageExporterImpl {
    async fn export(&self, photo: &Photo, output_path: &FilePath) -> DomainResult<()> {
        let input_path = photo.file_path().as_str()?;
        
        // Load original image
        let img = image::open(Path::new(&input_path))
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to open source image: {}", e)))?;

        // Get all adjustments from photo
        let exposure = photo.edit_exposure().unwrap_or(0.0);
        let contrast = photo.edit_contrast().unwrap_or(1.0);
        let temperature = photo.edit_temperature().unwrap_or(0.0);
        let tint = photo.edit_tint().unwrap_or(0.0);
        let highlights = photo.edit_highlights().unwrap_or(0.0);
        let shadows = photo.edit_shadows().unwrap_or(0.0);
        let whites = photo.edit_whites().unwrap_or(0.0);
        let blacks = photo.edit_blacks().unwrap_or(0.0);
        let clarity = photo.edit_clarity().unwrap_or(0.0);
        let vibrance = photo.edit_vibrance().unwrap_or(0.0);
        let saturation = photo.edit_saturation().unwrap_or(0.0);

        // Apply all 11 adjustments
        let processed = Self::process_image(
            &img, exposure, contrast, temperature, tint,
            highlights, shadows, whites, blacks,
            clarity, vibrance, saturation
        );

        // Save as JPEG with quality 90
        let output_path_str = output_path.as_str()?;
        let rgb_img = processed.to_rgb8();
        
        let file = std::fs::File::create(Path::new(output_path_str))
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to create output file: {}", e)))?;
        
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, 90);
        encoder.encode(&rgb_img, rgb_img.width(), rgb_img.height(), image::ColorType::Rgb8)
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to encode JPEG: {}", e)))?;

        Ok(())
    }
}

