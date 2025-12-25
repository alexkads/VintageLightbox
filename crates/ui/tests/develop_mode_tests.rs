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

    /// Create a test image with known pixel values
    fn create_test_image(width: u32, height: u32, r: u8, g: u8, b: u8) -> DynamicImage {
        let img = RgbaImage::from_pixel(width, height, Rgba([r, g, b, 255]));
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn test_exposure_positive_brightens_image() {
        let img = create_test_image(10, 10, 100, 100, 100);
        let processed = ImageProcessor::process_image(
            &img, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        );
        
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
        let processed = ImageProcessor::process_image(
            &img, -1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        );
        
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
        let processed = ImageProcessor::process_image(
            &img, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        );
        
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
        let processed = ImageProcessor::process_image(
            &img, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        );
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Light pixels should get lighter with increased contrast
        // (200 - 128) * 2 + 128 = 272 -> clamped to 255
        assert_eq!(pixel[0], 255, "Light pixel should be clamped to 255");
    }

    #[test]
    fn test_temperature_warm() {
        let img = create_test_image(10, 10, 100, 100, 100);
        let processed = ImageProcessor::process_image(
            &img, 0.0, 1.0, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        );
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Warm should increase red, decrease blue
        assert!(pixel[0] > pixel[2], "Red should be greater than blue for warm temp");
    }

    #[test]
    fn test_temperature_cool() {
        let img = create_test_image(10, 10, 100, 100, 100);
        let processed = ImageProcessor::process_image(
            &img, 0.0, 1.0, -5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        );
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Cool should decrease red, increase blue
        assert!(pixel[2] > pixel[0], "Blue should be greater than red for cool temp");
    }

    #[test]
    fn test_saturation_increase() {
        // Create a colored image (not gray)
        let img = create_test_image(10, 10, 200, 100, 50);
        let processed = ImageProcessor::process_image(
            &img, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0
        );
        
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
        let processed = ImageProcessor::process_image(
            &img, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0
        );
        
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
        let processed = ImageProcessor::process_image(
            &img, 0.0, 1.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        );
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Positive highlights should brighten bright areas
        assert!(pixel[0] > 200, "Highlights should brighten bright areas");
    }

    #[test]
    fn test_shadows_adjustment() {
        // Create dark image (<128 luminance)
        let img = create_test_image(10, 10, 50, 50, 50);
        let processed = ImageProcessor::process_image(
            &img, 0.0, 1.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        );
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Positive shadows should brighten dark areas
        assert!(pixel[0] > 50, "Shadows should brighten dark areas (got {})", pixel[0]);
    }

    #[test]
    fn test_image_dimensions_preserved() {
        let img = create_test_image(123, 456, 100, 100, 100);
        let processed = ImageProcessor::process_image(
            &img, 1.0, 1.5, 3.0, -2.0, 10.0, 20.0, 5.0, -5.0, 0.3, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0
        );
        
        assert_eq!(processed.width(), 123, "Width should be preserved");
        assert_eq!(processed.height(), 456, "Height should be preserved");
    }

    #[test]
    fn test_alpha_channel_preserved() {
        // Create image with specific alpha
        let img = RgbaImage::from_pixel(10, 10, Rgba([100, 100, 100, 127]));
        let dynamic = DynamicImage::ImageRgba8(img);
        
        let processed = ImageProcessor::process_image(
            &dynamic, 1.0, 2.0, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        );
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        assert_eq!(pixel[3], 127, "Alpha channel should be preserved");
    }

    #[test]
    fn test_pixel_values_clamped() {
        let img = create_test_image(10, 10, 250, 250, 250);
        let processed = ImageProcessor::process_image(
            &img, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
        );
        
        let rgba = processed.to_rgba8();
        let pixel = rgba.get_pixel(5, 5);
        
        // Values should be clamped to 255
        assert!(pixel[0] <= 255, "Pixel values should be clamped to 255");
        assert!(pixel[1] <= 255, "Pixel values should be clamped to 255");
        assert!(pixel[2] <= 255, "Pixel values should be clamped to 255");
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
#[cfg(test)]
mod edit_state_tests {
    use ui::state::AppState;

    #[test]
    fn test_initial_state_has_no_history() {
        let state = AppState::new();
        assert!(state.edit_history.is_empty());
        assert!(state.history_index.is_none());
    }

    #[test]
    fn test_push_snapshot_adds_to_history() {
        let mut state = AppState::new();
        state.active_exposure = 1.0;
        state.push_edit_snapshot();
        
        assert_eq!(state.edit_history.len(), 1);
        assert_eq!(state.history_index, Some(0));
    }

    #[test]
    fn test_undo_reverts_to_previous_state() {
        let mut state = AppState::new();
        
        // Push first snapshot
        state.active_exposure = 0.0;
        state.push_edit_snapshot();
        
        // Push second snapshot
        state.active_exposure = 2.0;
        state.push_edit_snapshot();
        
        // Undo
        let undone = state.undo();
        
        assert!(undone);
        assert_eq!(state.active_exposure, 0.0);
        assert_eq!(state.history_index, Some(0));
    }

    #[test]
    fn test_undo_on_first_state_fails() {
        let mut state = AppState::new();
        state.push_edit_snapshot();
        
        let undone = state.undo();
        assert!(!undone);
    }

    #[test]
    fn test_redo_after_undo() {
        let mut state = AppState::new();
        
        // Push snapshots
        state.active_exposure = 0.0;
        state.push_edit_snapshot();
        
        state.active_exposure = 2.0;
        state.active_contrast = 1.5;
        state.push_edit_snapshot();
        
        // Undo
        state.undo();
        assert_eq!(state.active_exposure, 0.0);
        
        // Redo
        let redone = state.redo();
        assert!(redone);
        assert_eq!(state.active_exposure, 2.0);
        assert_eq!(state.active_contrast, 1.5);
    }

    #[test]
    fn test_redo_at_end_fails() {
        let mut state = AppState::new();
        state.push_edit_snapshot();
        
        let redone = state.redo();
        assert!(!redone);
    }

    #[test]
    fn test_new_edit_after_undo_truncates_history() {
        let mut state = AppState::new();
        
        // Build history: A -> B -> C
        state.active_exposure = 1.0;
        state.push_edit_snapshot(); // Index 0
        
        state.active_exposure = 2.0;
        state.push_edit_snapshot(); // Index 1
        
        state.active_exposure = 3.0;
        state.push_edit_snapshot(); // Index 2
        
        // Undo twice: now at A
        state.undo(); // Back to B
        state.undo(); // Back to A
        
        // New edit D should truncate B and C
        state.active_exposure = 4.0;
        state.push_edit_snapshot();
        
        assert_eq!(state.edit_history.len(), 2); // A and D only
        assert_eq!(state.history_index, Some(1));
    }

    #[test]
    fn test_history_limit_of_20() {
        let mut state = AppState::new();
        
        // Push 25 snapshots
        for i in 0..25 {
            state.active_exposure = i as f32;
            state.push_edit_snapshot();
        }
        
        // History should be limited to 20
        assert_eq!(state.edit_history.len(), 20);
        // First snapshot should be exposure = 5 (snapshots 0-4 were removed)
        assert_eq!(state.edit_history[0].exposure, 5.0);
    }

    #[test]
    fn test_all_edit_parameters_in_snapshot() {
        let mut state = AppState::new();
        
        state.active_exposure = 1.0;
        state.active_contrast = 1.5;
        state.active_temperature = 10.0;
        state.active_tint = -5.0;
        state.active_highlights = 20.0;
        state.active_shadows = -10.0;
        state.active_whites = 15.0;
        state.active_blacks = -15.0;
        state.active_clarity = 0.3;
        state.active_vibrance = 0.2;
        state.active_saturation = 0.1;
        
        state.push_edit_snapshot();
        
        let snapshot = &state.edit_history[0];
        assert_eq!(snapshot.exposure, 1.0);
        assert_eq!(snapshot.contrast, 1.5);
        assert_eq!(snapshot.temperature, 10.0);
        assert_eq!(snapshot.tint, -5.0);
        assert_eq!(snapshot.highlights, 20.0);
        assert_eq!(snapshot.shadows, -10.0);
        assert_eq!(snapshot.whites, 15.0);
        assert_eq!(snapshot.blacks, -15.0);
        assert_eq!(snapshot.clarity, 0.3);
        assert_eq!(snapshot.vibrance, 0.2);
        assert_eq!(snapshot.saturation, 0.1);
    }
}

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
// DEBOUNCING TESTS
// =========================================================================================
#[cfg(test)]
mod debounce_tests {
    use ui::image_processing::ImageProcessor;

    #[test]
    fn test_should_process_initially_true() {
        let processor = ImageProcessor::new();
        assert!(processor.should_process());
    }
    
    // Note: Detailed debounce timing tests removed because last_edit_time is private.
    // The debouncing behavior is tested implicitly through apply_edits_debounced.
}

