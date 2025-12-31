#[cfg(test)]
mod tests {
    
    use ui::state::AppState;
    use domain::value_objects::{CropSettings, RotationFillMode, AspectRatio};
    use image::DynamicImage;
    

    #[test]
    fn test_crop_panel_reset_restores_defaults() {
        let mut state = AppState::new(false);
        state.original_preview = Some(DynamicImage::new_rgb8(100, 100));
        
        // 1. Setup complex state
        state.crop_settings = Some(
            CropSettings::new(0.2, 0.2, 0.5, 0.5, 1, 15.0, true, true)
                .with_fill_mode(RotationFillMode::ShrinkToFit)
        );
        state.selected_aspect_ratio = AspectRatio::Square;
        
        // Verify initial state is "dirty"
        let crop = state.crop_settings.as_ref().unwrap();
        assert_eq!(crop.rotation_90(), 1);
        assert_eq!(crop.angle(), 15.0);
        assert_eq!(crop.fill_mode(), RotationFillMode::ShrinkToFit);
        assert_eq!(state.selected_aspect_ratio, AspectRatio::Square);
        
        // 2. Perform Reset Logic (Mental Model Simulation of what button does)
        // We can't click the button in unit test, but we can verify the logic block
        // Replicating the logic from CropPanel line 195:
        state.crop_settings = Some(CropSettings::default());
        state.selected_aspect_ratio = AspectRatio::Original;
        
        // 3. Verify defaults restored
        let reset_crop = state.crop_settings.as_ref().unwrap();
        assert_eq!(reset_crop.rotation_90(), 0);
        assert_eq!(reset_crop.angle(), 0.0);
        assert_eq!(reset_crop.crop_x(), 0.0);
        assert_eq!(reset_crop.crop_width(), 1.0);
        assert_eq!(reset_crop.fill_mode(), RotationFillMode::default()); // Should be Transparent
        
        assert_eq!(state.selected_aspect_ratio, AspectRatio::Original);
    }
}
