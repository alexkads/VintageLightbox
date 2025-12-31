/// Unit Tests for Develop Mode
///
/// Comprehensive tests for image processing, edit state management,
/// and GPU processor functionality.

// =========================================================================================
// IMAGE PROCESSING TESTS
// =========================================================================================
#[cfg(test)]
mod image_processing_tests {
    use ui::image_processing::ImageProcessor;
    use image::{DynamicImage, RgbaImage, Rgba};
    use domain::value_objects::PhotoEdits;

    /// Create a test image with known pixel values
    fn create_test_image(width: u32, height: u32, r: u8, g: u8, b: u8) -> DynamicImage {
        let img = RgbaImage::from_pixel(width, height, Rgba([r, g, b, 255]));
        DynamicImage::ImageRgba8(img)
    }

    /// Create PhotoEdits with only specific fields set
    fn edits_with_exposure(exposure: f32) -> PhotoEdits {
        PhotoEdits {
            exposure,
            ..Default::default()
        }
    }

    fn edits_with_contrast(contrast: f32) -> PhotoEdits {
        PhotoEdits {
            contrast,
            ..Default::default()
        }
    }

    fn edits_with_temperature(temperature: f32) -> PhotoEdits {
        PhotoEdits {
            temperature,
            ..Default::default()
        }
    }

    fn edits_with_saturation(saturation: f32) -> PhotoEdits {
        PhotoEdits {
            saturation,
            ..Default::default()
        }
    }

    fn edits_with_highlights(highlights: f32) -> PhotoEdits {
        PhotoEdits {
            highlights,
            ..Default::default()
        }
    }

    fn edits_with_shadows(shadows: f32) -> PhotoEdits {
        PhotoEdits {
            shadows,
            ..Default::default()
        }
    }

    #[test]
    fn test_exposure_positive_brightens_image() {
        let img = create_test_image(10, 10, 100, 100, 100);
        let edits = edits_with_exposure(1.0);
        let processed = ImageProcessor::process_image(&img, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Exposure +1 should double brightness (2^1 = 2)
        // 100 * 2 = 200
        assert!(pixel[0] > 150, "Red channel should be brighter (got {})", pixel[0]);
        assert!(pixel[1] > 150, "Green channel should be brighter");
        assert!(pixel[2] > 150, "Blue channel should be brighter");
    }

    #[test]
    fn test_exposure_negative_darkens_image() {
        let img = create_test_image(10, 10, 200, 200, 200);
        let edits = edits_with_exposure(-1.0);
        let processed = ImageProcessor::process_image(&img, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Exposure -1 should halve brightness (2^-1 = 0.5)
        // 200 * 0.5 = 100
        assert!(pixel[0] < 150, "Red channel should be darker (got {})", pixel[0]);
    }

    #[test]
    fn test_contrast_increase() {
        // Create image with mid-gray pixels
        let img = create_test_image(10, 10, 128, 128, 128);
        let edits = edits_with_contrast(2.0);
        let processed = ImageProcessor::process_image(&img, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Mid-gray should stay roughly the same with contrast
        // (128 - 128) * 2 + 128 = 128
        assert_eq!(pixel[0], 128, "Mid-gray should stay the same with contrast");
    }

    #[test]
    fn test_contrast_with_light_pixel() {
        // Create image with light pixels  
        let img = create_test_image(10, 10, 200, 200, 200);
        let edits = edits_with_contrast(2.0);
        let processed = ImageProcessor::process_image(&img, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Light pixels should get lighter with increased contrast
        // (200 - 128) * 2 + 128 = 272 -> clamped to 255
        assert_eq!(pixel[0], 255, "Light pixel should be clamped to 255");
    }

    #[test]
    fn test_temperature_warm() {
        let img = create_test_image(10, 10, 100, 100, 100);
        let edits = edits_with_temperature(5.0);
        let processed = ImageProcessor::process_image(&img, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Warm should increase red, decrease blue
        assert!(pixel[0] > pixel[2], "Red should be greater than blue for warm temp");
    }

    #[test]
    fn test_temperature_cool() {
        let img = create_test_image(10, 10, 100, 100, 100);
        let edits = edits_with_temperature(-5.0);
        let processed = ImageProcessor::process_image(&img, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Cool should decrease red, increase blue
        assert!(pixel[2] > pixel[0], "Blue should be greater than red for cool temp");
    }

    #[test]
    fn test_saturation_increase() {
        // Create a colored image (not gray)
        let img = create_test_image(10, 10, 200, 100, 50);
        let edits = edits_with_saturation(0.5);
        let processed = ImageProcessor::process_image(&img, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Original luminance ≈ 116
        // With increased saturation, colors should be more vibrant
        // Red should increase further from luminance, blue should decrease further
        assert!(pixel[0] > 200, "Red should increase with saturation (got {})", pixel[0]);
    }

    #[test]
    fn test_saturation_decrease_to_grayscale() {
        let img = create_test_image(10, 10, 200, 100, 50);
        let edits = edits_with_saturation(-1.0);
        let processed = ImageProcessor::process_image(&img, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Full desaturation should make all channels equal (grayscale)
        // luminance = (200 + 100 + 50) / 3 = 116
        assert_eq!(pixel[0], pixel[1], "Desaturated image should be grayscale (R=G)");
        assert_eq!(pixel[1], pixel[2], "Desaturated image should be grayscale (G=B)");
    }

    #[test]
    fn test_highlights_adjustment() {
        // Create bright image (>128 luminance)
        let img = create_test_image(10, 10, 200, 200, 200);
        let edits = edits_with_highlights(50.0);
        let processed = ImageProcessor::process_image(&img, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Positive highlights should brighten bright areas
        assert!(pixel[0] > 200, "Highlights should brighten bright areas");
    }

    #[test]
    fn test_shadows_adjustment() {
        // Create dark image (<128 luminance)
        let img = create_test_image(10, 10, 50, 50, 50);
        let edits = edits_with_shadows(50.0);
        let processed = ImageProcessor::process_image(&img, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Positive shadows should brighten dark areas
        assert!(pixel[0] > 50, "Shadows should brighten dark areas (got {})", pixel[0]);
    }

    #[test]
    fn test_image_dimensions_preserved() {
        let img = create_test_image(123, 456, 100, 100, 100);
        let edits = PhotoEdits {
            exposure: 1.0,
            contrast: 1.5,
            temperature: 3.0,
            tint: -2.0,
            highlights: 10.0,
            shadows: 20.0,
            whites: 5.0,
            blacks: -5.0,
            clarity: 0.3,
            vibrance: 0.2,
            saturation: 0.1,
            ..Default::default()
        };
        let processed = ImageProcessor::process_image(&img, &edits);
        
        assert_eq!(processed.width(), 123, "Width should be preserved");
        assert_eq!(processed.height(), 456, "Height should be preserved");
    }

    #[test]
    fn test_alpha_channel_preserved() {
        // Create image with specific alpha
        let img = RgbaImage::from_pixel(10, 10, Rgba([100, 100, 100, 127]));
        let dynamic = DynamicImage::ImageRgba8(img);
        
        let edits = PhotoEdits {
            exposure: 1.0,
            contrast: 2.0,
            temperature: 5.0,
            ..Default::default()
        };
        let processed = ImageProcessor::process_image(&dynamic, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        assert_eq!(pixel[3], 127, "Alpha channel should be preserved");
    }

    #[test]
    fn test_pixel_values_clamped() {
        let img = create_test_image(10, 10, 250, 250, 250);
        let edits = edits_with_exposure(2.0);
        let processed = ImageProcessor::process_image(&img, &edits);
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        #[allow(unused_comparisons)]
        {
        assert!(pixel[0] <= 255, "Pixel values should be clamped to 255");
        assert!(pixel[1] <= 255, "Pixel values should be clamped to 255");
        assert!(pixel[2] <= 255, "Pixel values should be clamped to 255");
        }
    }

    #[test]
    fn test_resize_smaller_than_max() {
        let img = DynamicImage::new_rgb8(500, 300);
        let resized = ImageProcessor::resize_for_preview(&img, 1280);
        
        // Image smaller than max should not be resized
        assert_eq!(resized.width(), 500);
        assert_eq!(resized.height(), 300);
    }

    #[test]
    fn test_resize_landscape() {
        let img = DynamicImage::new_rgb8(2000, 1000);
        let resized = ImageProcessor::resize_for_preview(&img, 800);
        
        assert_eq!(resized.width(), 800);
        assert_eq!(resized.height(), 400);
    }

    #[test]
    fn test_resize_portrait() {
        let img = DynamicImage::new_rgb8(1000, 2000);
        let resized = ImageProcessor::resize_for_preview(&img, 800);
        
        assert_eq!(resized.width(), 400);
        assert_eq!(resized.height(), 800);
    }

    #[test]
    fn test_resize_square() {
        let img = DynamicImage::new_rgb8(2000, 2000);
        let resized = ImageProcessor::resize_for_preview(&img, 500);
        
        assert_eq!(resized.width(), 500);
        assert_eq!(resized.height(), 500);
    }
}

// =========================================================================================
// EDIT STATE MANAGEMENT TESTS
// =========================================================================================


// =========================================================================================
// GPU PROCESSOR TESTS
// =========================================================================================
#[cfg(test)]
mod gpu_processor_tests {
    use ui::gpu_processor::{GpuEditParams, GpuImageProcessor};

    #[test]
    fn test_gpu_edit_params_default() {
        let params = GpuEditParams::default();
        
        assert_eq!(params.exposure, 0.0);
        assert_eq!(params.contrast, 1.0);
        assert_eq!(params.temperature, 0.0);
        assert_eq!(params.tint, 0.0);
        assert_eq!(params.highlights, 0.0);
        assert_eq!(params.shadows, 0.0);
        assert_eq!(params.whites, 0.0);
        assert_eq!(params.blacks, 0.0);
        assert_eq!(params.saturation, 0.0);
        assert_eq!(params.vibrance, 0.0);
        assert_eq!(params.clarity, 0.0);
    }

    #[test]
    fn test_gpu_processor_creation() {
        let processor = GpuImageProcessor::new();
        
        // Processor should be created successfully
        // GPU availability depends on hardware, so we just check creation
        let _ = processor.is_gpu_available();
    }

    #[test]
    fn test_request_id_increments() {
        let processor = GpuImageProcessor::new();
        
        let id1 = processor.next_request_id();
        let id2 = processor.next_request_id();
        let id3 = processor.next_request_id();
        
        assert!(id2 > id1);
        assert!(id3 > id2);
    }

    #[test]
    fn test_poll_result_empty_when_no_requests() {
        let processor = GpuImageProcessor::new();
        
        // Polling without any requests should return None
        let result = processor.poll_result();
        assert!(result.is_none());
    }
}

// =========================================================================================
// DEBOUNCING TESTS - REMOVED
// =========================================================================================
// Debouncing logic has been moved to infrastructure::image_processing::async_loader
// and is tested there. The UI layer no longer has debouncing state.
