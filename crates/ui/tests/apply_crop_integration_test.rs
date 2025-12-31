use ui::state::AppState;
use adapters::services::EditorService;
use domain::value_objects::{CropSettings, PhotoEdits};

#[tokio::test]
async fn test_app_pending_crop_apply_logic() {
    // 1. Setup Harness
    let mut state = AppState::new(false);
    let mut editor_service = EditorService::new();
    let initial_edits = PhotoEdits::default();
    
    // Start editing session
    let photo_id_str = "test_id".to_string();
    editor_service.start_editing(photo_id_str, initial_edits);
    
    // 2. Simulate User Action (Apply button clicked)
    // This sets the flags in AppState
    state.pending_crop_apply = true;
    let crop = CropSettings::new(
        0.5, 0.5, 0.2, 0.2, 
        0, 10.0, 
        true, false
    );
    state.crop_settings = Some(crop.clone());
    
    // 3. Execute the Logic (Simulating the block we added to app.rs)
    // This logic mimics exactly what happens in VintageLightboxApp::update
    if state.pending_crop_apply {
        state.pending_crop_apply = false;
        
        // Sync logic: this is what we added to app.rs
        if let Some(c) = &state.crop_settings {
             editor_service.update_field("Apply Crop", |e| {
                 e.crop_settings = Some(c.clone());
             }).expect("Sync failed");
        }
        
        // Mark for immediate save
        state.pending_auto_save = true;
        state.last_slider_change_time = Some(std::time::Instant::now() - std::time::Duration::from_millis(1000));
    }
    
    // 4. Verify Outcomes
    // EditorService should have updated edits with the crop settings
    let current_edits = editor_service.current_edits();
    assert!(current_edits.crop_settings.is_some(), "Crop settings should be present in EditorService");
    
    let saved_crop = current_edits.crop_settings.unwrap();
    assert_eq!(saved_crop.crop_x(), 0.5);
    assert_eq!(saved_crop.angle(), 10.0);
    assert_eq!(saved_crop.flip_horizontal(), true);
    
    // State should be ready for auto-save
    assert!(state.pending_auto_save, "pending_auto_save should be true");
    assert!(!state.pending_crop_apply, "pending_crop_apply should be reset to false");
}
