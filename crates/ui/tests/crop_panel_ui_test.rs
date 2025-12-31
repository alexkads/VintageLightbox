use egui_kittest::Harness;
use egui_kittest::kittest::Queryable; // Verify this import path
use ui::components::crop_panel::CropPanel;
use ui::state::AppState;
use domain::value_objects::CropSettings;
use std::rc::Rc;
use std::cell::RefCell;

#[test]
fn test_crop_panel_apply_button() {
    let mut val = AppState::new(false);
    // Activate crop mode
    val.crop_mode_active = true;
    val.crop_settings = Some(CropSettings::default());
    
    let state = Rc::new(RefCell::new(val));
    let state_clone = state.clone();

    let mut harness = Harness::new_ui(move |ui| {
        let mut state = state_clone.borrow_mut();
        CropPanel::show(ui, &mut state);
    });

    // 1. Initial render
    harness.run();

    // 2. Find Apply button and click
    // Note: The button text is "✓ Apply"
    harness.get_by_label("✓ Apply").click();
    harness.run();
    
    // 3. Verify state
    let state = state.borrow();
    assert!(state.pending_crop_apply, "Clicking Apply should set pending_crop_apply to true");
    assert!(!state.crop_mode_active, "Clicking Apply should deactivate crop mode");
}

#[test]
fn test_crop_panel_cancel_button() {
    let mut val = AppState::new(false);
    val.crop_mode_active = true;
    val.crop_settings = Some(CropSettings::default());

    let state = Rc::new(RefCell::new(val));
    let state_clone = state.clone();

    let mut harness = Harness::new_ui(move |ui| {
        let mut state = state_clone.borrow_mut();
        CropPanel::show(ui, &mut state);
    });
    
    harness.run();
    
    // Click Cancel
    harness.get_by_label("Cancel").click();
    harness.run();
    
    let state = state.borrow();
    assert!(!state.pending_crop_apply, "Clicking Cancel should NOT set pending_crop_apply");
    assert!(!state.crop_mode_active, "Clicking Cancel should deactivate crop mode");
}
