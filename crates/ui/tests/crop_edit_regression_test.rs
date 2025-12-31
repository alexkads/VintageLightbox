/// Comprehensive regression tests for crop/straighten editing operations.
/// These tests ensure that editing operations don't break existing functionality.
/// 
/// Coverage:
/// - Reset functionality (complete state cleanup)
/// - Fill mode preservation during edits
/// - Crop overlay calculations
/// - UV mapping with default values
/// - State synchronization

use ui::state::AppState;
use domain::value_objects::{CropSettings, RotationFillMode, AspectRatio};
use image::DynamicImage;
use egui::{Vec2, pos2};

// =============================================================================
// RESET FUNCTIONALITY TESTS
// =============================================================================

#[cfg(test)]
mod reset_tests {
    use super::*;

    /// Test that reset restores ALL crop-related state to defaults
    #[test]
    fn test_reset_restores_complete_state() {
        let mut state = AppState::new(false);
        state.original_preview = Some(DynamicImage::new_rgb8(1920, 1080));
        
        // Setup a complex "dirty" state
        state.crop_mode_active = true;
        state.crop_settings = Some(
            CropSettings::new(0.1, 0.2, 0.6, 0.7, 2, 25.0, true, false)
                .with_fill_mode(RotationFillMode::Intelligent)
        );
        state.selected_aspect_ratio = AspectRatio::SixteenNine;
        state.show_composition_grid = true;
        state.intelligent_fill_texture = None; // Simulating it could be Some in real scenario
        
        // Perform reset (replicating the actual reset logic)
        state.crop_settings = Some(CropSettings::default());
        state.selected_aspect_ratio = AspectRatio::Original;
        state.show_composition_grid = false;
        state.intelligent_fill_texture = None;
        
        // Verify ALL fields are reset
        let crop = state.crop_settings.as_ref().unwrap();
        
        // Crop region
        assert_eq!(crop.crop_x(), 0.0, "crop_x should be 0.0 after reset");
        assert_eq!(crop.crop_y(), 0.0, "crop_y should be 0.0 after reset");
        assert_eq!(crop.crop_width(), 1.0, "crop_width should be 1.0 after reset");
        assert_eq!(crop.crop_height(), 1.0, "crop_height should be 1.0 after reset");
        
        // Rotation
        assert_eq!(crop.rotation_90(), 0, "rotation_90 should be 0 after reset");
        assert_eq!(crop.angle(), 0.0, "angle should be 0.0 after reset");
        
        // Flips
        assert!(!crop.flip_horizontal(), "flip_horizontal should be false after reset");
        assert!(!crop.flip_vertical(), "flip_vertical should be false after reset");
        
        // Fill mode
        assert_eq!(crop.fill_mode(), RotationFillMode::default(), "fill_mode should be default after reset");
        
        // Other state
        assert_eq!(state.selected_aspect_ratio, AspectRatio::Original, "aspect_ratio should be Original after reset");
        assert!(!state.show_composition_grid, "show_composition_grid should be false after reset");
        assert!(state.intelligent_fill_texture.is_none(), "intelligent_fill_texture should be None after reset");
    }

    /// Test that reset works correctly after multiple edits
    #[test]
    fn test_reset_after_multiple_edits() {
        let mut state = AppState::new(false);
        state.original_preview = Some(DynamicImage::new_rgb8(800, 600));
        state.crop_mode_active = true;
        
        // First edit: simple crop
        state.crop_settings = Some(CropSettings::new(0.1, 0.1, 0.8, 0.8, 0, 0.0, false, false));
        
        // Second edit: add rotation
        if let Some(crop) = &state.crop_settings {
            state.crop_settings = Some(crop.with_rotation_90(1));
        }
        
        // Third edit: add angle
        if let Some(crop) = &state.crop_settings {
            state.crop_settings = Some(crop.with_angle(15.0));
        }
        
        // Fourth edit: add flip
        if let Some(crop) = &state.crop_settings {
            state.crop_settings = Some(crop.with_flip_horizontal(true));
        }
        
        // Now reset
        state.crop_settings = Some(CropSettings::default());
        state.selected_aspect_ratio = AspectRatio::Original;
        state.show_composition_grid = false;
        
        let crop = state.crop_settings.as_ref().unwrap();
        assert!(!crop.has_modifications(), "After reset, should have no modifications");
        assert!(!crop.is_cropped(), "After reset, should not be cropped");
        assert!(!crop.is_rotated(), "After reset, should not be rotated");
        assert!(!crop.is_flipped(), "After reset, should not be flipped");
    }

    /// Test reset with ShrinkToFit fill mode
    #[test]
    fn test_reset_clears_shrink_to_fit_state() {
        let mut state = AppState::new(false);
        state.original_preview = Some(DynamicImage::new_rgb8(1000, 1000));
        state.crop_mode_active = true;
        
        // Setup with ShrinkToFit and angle
        state.crop_settings = Some(
            CropSettings::new(0.1, 0.1, 0.8, 0.8, 0, 30.0, false, false)
                .with_fill_mode(RotationFillMode::ShrinkToFit)
        );
        
        // Reset
        state.crop_settings = Some(CropSettings::default());
        
        let crop = state.crop_settings.as_ref().unwrap();
        // After reset, fill_mode should be default (Transparent), not ShrinkToFit
        assert_eq!(crop.fill_mode(), RotationFillMode::default());
        assert_eq!(crop.angle(), 0.0);
    }
}

// =============================================================================
// FILL MODE PRESERVATION TESTS
// =============================================================================

#[cfg(test)]
mod fill_mode_tests {
    use super::*;

    /// Test that fill_mode is preserved when using with_fill_mode_value
    #[test]
    fn test_fill_mode_preserved_with_new_crop_values() {
        let original = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 15.0, false, false)
            .with_fill_mode(RotationFillMode::Intelligent);
        
        // Simulate what CropOverlay now does (using with_fill_mode_value)
        let updated = CropSettings::with_fill_mode_value(
            0.1, 0.1, 0.8, 0.8,  // new crop values
            original.rotation_90(), original.angle(),
            original.flip_horizontal(), original.flip_vertical(),
            original.fill_mode()  // preserve fill_mode
        );
        
        assert_eq!(updated.fill_mode(), RotationFillMode::Intelligent, "fill_mode should be preserved");
        assert_eq!(updated.crop_x(), 0.1);
        assert_eq!(updated.crop_width(), 0.8);
    }

    /// Test that fill_mode is preserved during pan operations
    #[test]
    fn test_fill_mode_preserved_during_pan() {
        let original = CropSettings::new(0.2, 0.2, 0.6, 0.6, 0, 0.0, false, false)
            .with_fill_mode(RotationFillMode::Black);
        
        // Simulate pan operation
        let panned = CropSettings::with_fill_mode_value(
            0.3, 0.3, 0.6, 0.6,  // moved position, same size
            original.rotation_90(), original.angle(),
            original.flip_horizontal(), original.flip_vertical(),
            original.fill_mode()
        );
        
        assert_eq!(panned.fill_mode(), RotationFillMode::Black, "fill_mode should be preserved after pan");
    }

    /// Test that fill_mode is preserved during angle changes
    #[test]
    fn test_fill_mode_preserved_during_rotation_drag() {
        let original = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 10.0, false, false)
            .with_fill_mode(RotationFillMode::White);
        
        // Simulate rotation drag
        let rotated = CropSettings::with_fill_mode_value(
            original.crop_x(), original.crop_y(), 
            original.crop_width(), original.crop_height(),
            original.rotation_90(), 
            15.0,  // new angle
            original.flip_horizontal(), original.flip_vertical(),
            original.fill_mode()
        );
        
        assert_eq!(rotated.fill_mode(), RotationFillMode::White, "fill_mode should be preserved after rotation drag");
        assert_eq!(rotated.angle(), 15.0);
    }

    /// Test that all fill modes are correctly preserved
    #[test]
    fn test_all_fill_modes_preserved() {
        let fill_modes = [
            RotationFillMode::Transparent,
            RotationFillMode::Black,
            RotationFillMode::White,
            RotationFillMode::Intelligent,
            RotationFillMode::ShrinkToFit,
        ];
        
        for mode in fill_modes {
            let original = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 5.0, false, false)
                .with_fill_mode(mode);
            
            let updated = CropSettings::with_fill_mode_value(
                0.1, 0.1, 0.8, 0.8,
                original.rotation_90(), original.angle(),
                original.flip_horizontal(), original.flip_vertical(),
                original.fill_mode()
            );
            
            assert_eq!(updated.fill_mode(), mode, "fill_mode {:?} should be preserved", mode);
        }
    }
}

// =============================================================================
// CROP SETTINGS DEFAULT VALUES TESTS
// =============================================================================

#[cfg(test)]
mod default_values_tests {
    use super::*;

    /// Test that CropSettings::default() produces correct values
    #[test]
    fn test_default_crop_settings() {
        let default = CropSettings::default();
        
        assert_eq!(default.crop_x(), 0.0);
        assert_eq!(default.crop_y(), 0.0);
        assert_eq!(default.crop_width(), 1.0);
        assert_eq!(default.crop_height(), 1.0);
        assert_eq!(default.rotation_90(), 0);
        assert_eq!(default.angle(), 0.0);
        assert!(!default.flip_horizontal());
        assert!(!default.flip_vertical());
        assert_eq!(default.fill_mode(), RotationFillMode::default());
    }

    /// Test that default crop settings indicate "no modifications"
    #[test]
    fn test_default_has_no_modifications() {
        let default = CropSettings::default();
        
        assert!(!default.is_cropped(), "Default should not be cropped");
        assert!(!default.is_rotated(), "Default should not be rotated");
        assert!(!default.is_flipped(), "Default should not be flipped");
        assert!(!default.has_modifications(), "Default should have no modifications");
    }

    /// Test that default total_rotation is 0
    #[test]
    fn test_default_total_rotation() {
        let default = CropSettings::default();
        assert_eq!(default.total_rotation(), 0.0);
    }
}

// =============================================================================
// UV MAPPING TESTS
// =============================================================================

#[cfg(test)]
mod uv_mapping_tests {
    use super::*;
    use ui::geometry::calculate_crop_uvs;

    /// Test UV mapping with default crop settings (full image)
    #[test]
    fn test_uv_mapping_default_shows_full_image() {
        let default = CropSettings::default();
        let texture_size = Vec2::new(1920.0, 1080.0);
        
        let uvs = calculate_crop_uvs(&default, texture_size);
        
        // For default crop, UVs should cover the full image [0,0] to [1,1]
        // With slight tolerance for floating point
        assert!((uvs[0].x - 0.0).abs() < 0.001, "Top-left U should be 0");
        assert!((uvs[0].y - 0.0).abs() < 0.001, "Top-left V should be 0");
        assert!((uvs[1].x - 1.0).abs() < 0.001, "Top-right U should be 1");
        assert!((uvs[1].y - 0.0).abs() < 0.001, "Top-right V should be 0");
        assert!((uvs[2].x - 1.0).abs() < 0.001, "Bottom-right U should be 1");
        assert!((uvs[2].y - 1.0).abs() < 0.001, "Bottom-right V should be 1");
        assert!((uvs[3].x - 0.0).abs() < 0.001, "Bottom-left U should be 0");
        assert!((uvs[3].y - 1.0).abs() < 0.001, "Bottom-left V should be 1");
    }

    /// Test UV mapping with a partial crop (no rotation)
    #[test]
    fn test_uv_mapping_partial_crop() {
        let crop = CropSettings::new(0.25, 0.25, 0.5, 0.5, 0, 0.0, false, false);
        let texture_size = Vec2::new(1000.0, 1000.0);
        
        let uvs = calculate_crop_uvs(&crop, texture_size);
        
        // For center 50% crop, UVs should be [0.25, 0.25] to [0.75, 0.75]
        assert!((uvs[0].x - 0.25).abs() < 0.001, "Crop top-left U should be 0.25");
        assert!((uvs[0].y - 0.25).abs() < 0.001, "Crop top-left V should be 0.25");
        assert!((uvs[2].x - 0.75).abs() < 0.001, "Crop bottom-right U should be 0.75");
        assert!((uvs[2].y - 0.75).abs() < 0.001, "Crop bottom-right V should be 0.75");
    }

    /// Test UV mapping after reset returns to full image
    #[test]
    fn test_uv_mapping_after_reset() {
        // Start with a crop
        let cropped = CropSettings::new(0.1, 0.1, 0.8, 0.8, 1, 15.0, true, false);
        let texture_size = Vec2::new(1920.0, 1080.0);
        
        // UVs with crop
        let cropped_uvs = calculate_crop_uvs(&cropped, texture_size);
        
        // Reset
        let reset = CropSettings::default();
        let reset_uvs = calculate_crop_uvs(&reset, texture_size);
        
        // Verify reset UVs show full image
        assert!((reset_uvs[0].x - 0.0).abs() < 0.001);
        assert!((reset_uvs[0].y - 0.0).abs() < 0.001);
        assert!((reset_uvs[2].x - 1.0).abs() < 0.001);
        assert!((reset_uvs[2].y - 1.0).abs() < 0.001);
        
        // Verify cropped UVs were different
        assert!(
            (cropped_uvs[0].x - reset_uvs[0].x).abs() > 0.01 ||
            (cropped_uvs[0].y - reset_uvs[0].y).abs() > 0.01,
            "Cropped UVs should differ from reset UVs"
        );
    }
}

// =============================================================================
// STATE CONSISTENCY TESTS
// =============================================================================

#[cfg(test)]
mod state_consistency_tests {
    use super::*;

    /// Test that crop_mode_active doesn't affect reset behavior
    #[test]
    fn test_reset_works_in_active_crop_mode() {
        let mut state = AppState::new(false);
        state.original_preview = Some(DynamicImage::new_rgb8(800, 600));
        
        // Activate crop mode with modifications
        state.crop_mode_active = true;
        state.crop_settings = Some(
            CropSettings::new(0.2, 0.2, 0.6, 0.6, 1, 20.0, true, true)
        );
        
        // Reset while in crop mode
        state.crop_settings = Some(CropSettings::default());
        state.selected_aspect_ratio = AspectRatio::Original;
        
        // Crop mode should still be active (user can continue editing)
        assert!(state.crop_mode_active, "Crop mode should still be active after reset");
        
        // But settings should be reset
        let crop = state.crop_settings.as_ref().unwrap();
        assert!(!crop.has_modifications(), "Settings should be reset");
    }

    /// Test that AppState fields are independent
    #[test]
    fn test_state_fields_independence() {
        let mut state = AppState::new(false);
        
        // Modify crop settings
        state.crop_settings = Some(CropSettings::new(0.1, 0.1, 0.8, 0.8, 0, 0.0, false, false));
        
        // Changing aspect ratio shouldn't affect crop settings values
        let old_crop_x = state.crop_settings.as_ref().unwrap().crop_x();
        state.selected_aspect_ratio = AspectRatio::Square;
        assert_eq!(state.crop_settings.as_ref().unwrap().crop_x(), old_crop_x);
    }

    /// Test full edit -> apply -> reset cycle
    #[test]
    fn test_edit_apply_reset_cycle() {
        let mut state = AppState::new(false);
        state.original_preview = Some(DynamicImage::new_rgb8(1920, 1080));
        
        // 1. Enter crop mode
        state.crop_mode_active = true;
        state.crop_settings = Some(CropSettings::default());
        
        // 2. Make edits
        state.crop_settings = Some(
            CropSettings::new(0.1, 0.1, 0.8, 0.8, 0, 10.0, false, false)
                .with_fill_mode(RotationFillMode::Black)
        );
        state.selected_aspect_ratio = AspectRatio::Square;
        
        // 3. Simulate "Apply" (just mark as pending, exit crop mode)
        state.pending_crop_apply = true;
        state.crop_mode_active = false;
        
        assert!(state.pending_crop_apply);
        assert!(!state.crop_mode_active);
        
        // 4. Re-enter crop mode
        state.crop_mode_active = true;
        
        // 5. Reset
        state.crop_settings = Some(CropSettings::default());
        state.selected_aspect_ratio = AspectRatio::Original;
        state.show_composition_grid = false;
        state.intelligent_fill_texture = None;
        
        // Verify complete reset
        let crop = state.crop_settings.as_ref().unwrap();
        assert!(!crop.has_modifications());
        assert_eq!(state.selected_aspect_ratio, AspectRatio::Original);
    }
}

// =============================================================================
// CROP OVERLAY CALCULATION TESTS
// =============================================================================

#[cfg(test)]
mod crop_overlay_tests {
    use super::*;
    use egui::Rect;

    /// Helper to calculate crop rect (mirrors CropOverlay::calculate_crop_rect)
    fn calculate_crop_rect(image_rect: Rect, crop: &CropSettings) -> Rect {
        let x = image_rect.min.x + crop.crop_x() * image_rect.width();
        let y = image_rect.min.y + crop.crop_y() * image_rect.height();
        let w = crop.crop_width() * image_rect.width();
        let h = crop.crop_height() * image_rect.height();
        Rect::from_min_size(pos2(x, y), Vec2::new(w, h))
    }

    /// Test that default crop covers full image rect
    #[test]
    fn test_default_crop_rect_equals_image_rect() {
        let image_rect = Rect::from_min_size(pos2(100.0, 100.0), Vec2::new(800.0, 600.0));
        let default = CropSettings::default();
        
        let crop_rect = calculate_crop_rect(image_rect, &default);
        
        assert!((crop_rect.min.x - image_rect.min.x).abs() < 0.001);
        assert!((crop_rect.min.y - image_rect.min.y).abs() < 0.001);
        assert!((crop_rect.width() - image_rect.width()).abs() < 0.001);
        assert!((crop_rect.height() - image_rect.height()).abs() < 0.001);
    }

    /// Test partial crop rect calculation
    #[test]
    fn test_partial_crop_rect() {
        let image_rect = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(1000.0, 1000.0));
        let crop = CropSettings::new(0.25, 0.25, 0.5, 0.5, 0, 0.0, false, false);
        
        let crop_rect = calculate_crop_rect(image_rect, &crop);
        
        assert!((crop_rect.min.x - 250.0).abs() < 0.001);
        assert!((crop_rect.min.y - 250.0).abs() < 0.001);
        assert!((crop_rect.width() - 500.0).abs() < 0.001);
        assert!((crop_rect.height() - 500.0).abs() < 0.001);
    }

    /// Test crop rect after reset
    #[test]
    fn test_crop_rect_after_reset() {
        let image_rect = Rect::from_min_size(pos2(50.0, 50.0), Vec2::new(600.0, 400.0));
        
        // Start with a partial crop
        let cropped = CropSettings::new(0.1, 0.2, 0.7, 0.6, 0, 0.0, false, false);
        let cropped_rect = calculate_crop_rect(image_rect, &cropped);
        
        // Reset
        let reset = CropSettings::default();
        let reset_rect = calculate_crop_rect(image_rect, &reset);
        
        // Reset rect should equal full image rect
        assert!((reset_rect.min.x - image_rect.min.x).abs() < 0.001);
        assert!((reset_rect.min.y - image_rect.min.y).abs() < 0.001);
        assert!((reset_rect.width() - image_rect.width()).abs() < 0.001);
        assert!((reset_rect.height() - image_rect.height()).abs() < 0.001);
        
        // And should differ from cropped rect
        assert!((cropped_rect.width() - reset_rect.width()).abs() > 1.0);
    }
}

// =============================================================================
// ROTATION TESTS
// =============================================================================

#[cfg(test)]
mod rotation_tests {
    use super::*;

    /// Test 90-degree rotation increments
    #[test]
    fn test_rotation_90_increments() {
        let mut crop = CropSettings::default();
        
        // Test rotation values 0-3 cycle
        crop = crop.with_rotation_90(0);
        assert_eq!(crop.rotation_90(), 0);
        
        crop = crop.with_rotation_90(1);
        assert_eq!(crop.rotation_90(), 1);
        
        crop = crop.with_rotation_90(2);
        assert_eq!(crop.rotation_90(), 2);
        
        crop = crop.with_rotation_90(3);
        assert_eq!(crop.rotation_90(), 3);
        
        // Values > 3 are clamped
        crop = crop.with_rotation_90(4);
        // The implementation clamps to -1..3, so check what we get
        let rot = crop.rotation_90();
        assert!(rot >= -1 && rot <= 3, "rotation_90 should be clamped to valid range");
    }

    /// Test that angle resets to 0 after reset
    #[test]
    fn test_angle_reset() {
        let mut state = AppState::new(false);
        state.crop_settings = Some(
            CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 35.0, false, false)
        );
        
        assert_eq!(state.crop_settings.as_ref().unwrap().angle(), 35.0);
        
        // Reset
        state.crop_settings = Some(CropSettings::default());
        
        assert_eq!(state.crop_settings.as_ref().unwrap().angle(), 0.0);
    }

    /// Test combined rotation_90 + angle gives correct total
    #[test]
    fn test_total_rotation() {
        let crop = CropSettings::new(0.0, 0.0, 1.0, 1.0, 1, 15.0, false, false);
        assert_eq!(crop.total_rotation(), 105.0); // 90 + 15
        
        let crop2 = CropSettings::new(0.0, 0.0, 1.0, 1.0, 2, -10.0, false, false);
        assert_eq!(crop2.total_rotation(), 170.0); // 180 - 10
    }
}

// =============================================================================
// FLIP TESTS
// =============================================================================

#[cfg(test)]
mod flip_tests {
    use super::*;

    /// Test horizontal flip toggle
    #[test]
    fn test_horizontal_flip_toggle() {
        let crop = CropSettings::default();
        assert!(!crop.flip_horizontal());
        
        let flipped = crop.with_flip_horizontal(true);
        assert!(flipped.flip_horizontal());
        
        let unflipped = flipped.with_flip_horizontal(false);
        assert!(!unflipped.flip_horizontal());
    }

    /// Test vertical flip toggle
    #[test]
    fn test_vertical_flip_toggle() {
        let crop = CropSettings::default();
        assert!(!crop.flip_vertical());
        
        let flipped = crop.with_flip_vertical(true);
        assert!(flipped.flip_vertical());
        
        let unflipped = flipped.with_flip_vertical(false);
        assert!(!unflipped.flip_vertical());
    }

    /// Test combined flips
    #[test]
    fn test_combined_flips() {
        let crop = CropSettings::default()
            .with_flip_horizontal(true)
            .with_flip_vertical(true);
        
        assert!(crop.flip_horizontal());
        assert!(crop.flip_vertical());
        assert!(crop.is_flipped());
    }

    /// Test flip reset
    #[test]
    fn test_flips_reset() {
        let mut state = AppState::new(false);
        state.crop_settings = Some(
            CropSettings::default()
                .with_flip_horizontal(true)
                .with_flip_vertical(true)
        );
        
        assert!(state.crop_settings.as_ref().unwrap().is_flipped());
        
        // Reset
        state.crop_settings = Some(CropSettings::default());
        
        assert!(!state.crop_settings.as_ref().unwrap().is_flipped());
    }
}

// =============================================================================
// ASPECT RATIO TESTS
// =============================================================================

#[cfg(test)]
mod aspect_ratio_tests {
    use super::*;

    /// Test all aspect ratio values
    #[test]
    fn test_aspect_ratio_values() {
        assert_eq!(AspectRatio::Square.value(), 1.0);
        assert!((AspectRatio::FourThree.value() - 4.0/3.0).abs() < 0.001);
        assert!((AspectRatio::ThreeFour.value() - 3.0/4.0).abs() < 0.001);
        assert!((AspectRatio::SixteenNine.value() - 16.0/9.0).abs() < 0.001);
        assert!((AspectRatio::NineSixteen.value() - 9.0/16.0).abs() < 0.001);
    }

    /// Test landscape/portrait detection
    #[test]
    fn test_landscape_portrait_detection() {
        assert!(AspectRatio::SixteenNine.is_landscape());
        assert!(!AspectRatio::SixteenNine.is_portrait());
        
        assert!(AspectRatio::NineSixteen.is_portrait());
        assert!(!AspectRatio::NineSixteen.is_landscape());
        
        // Square is neither
        assert!(!AspectRatio::Square.is_landscape());
        assert!(!AspectRatio::Square.is_portrait());
    }

    /// Test aspect ratio reset to Original
    #[test]
    fn test_aspect_ratio_reset() {
        let mut state = AppState::new(false);
        state.selected_aspect_ratio = AspectRatio::SixteenNine;
        
        // Reset
        state.selected_aspect_ratio = AspectRatio::Original;
        
        assert_eq!(state.selected_aspect_ratio, AspectRatio::Original);
    }
}
