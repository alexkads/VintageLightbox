use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ui::components::settings_dialog::{SettingsDialog, SettingsAction};
use ui::state::AppState;
use infrastructure::cache::CacheStats;
use std::cell::RefCell;
use std::rc::Rc;
use std::path::PathBuf;

#[test]
fn test_settings_dialog_clear_thumbnails() {
    // Setup State
    let state = Rc::new(RefCell::new(AppState::new()));
    {
        let mut s = state.borrow_mut();
        s.show_settings_dialog = true; // Force dialog open
        s.cache_stats = Some(CacheStats {
            thumbnail_count: 100,
            large_preview_count: 50,
            total_size_bytes: 1024 * 1024 * 10,
            db_path: PathBuf::from("test_cache.db"),
        });
    }

    let state_clone = state.clone();
    let captured_action = Rc::new(RefCell::new(None));
    let captured_action_clone = captured_action.clone();

    // Harness render loop
    let mut harness = Harness::new_ui(move |ui| {
        let mut s = state_clone.borrow_mut();
        // Render the dialog
        let action = SettingsDialog::show(ui.ctx(), &mut s);
        
        if let Some(act) = action {
            *captured_action_clone.borrow_mut() = Some(act);
        }
    });

    // 1. Run initial frame to show dialog
    harness.run();

    // 2. Find "Clear Thumbnails" button and click
    // Note: Button::new("Clear Thumbnails") creates a button with that label
    let button = harness.get_by_label("Clear Thumbnails");
    button.click();

    // 3. Run frame to process click
    harness.run();

    // 4. Verify action was captured
    let action = captured_action.borrow();
    match action.as_ref() {
        Some(SettingsAction::ClearThumbnails) => {}, // Pass
        Some(other) => panic!("Expected ClearThumbnails, got {:?}", other),
        None => panic!("No action captured after clicking button"),
    }
}

#[test]
fn test_settings_dialog_stats_display() {
    // Test verifying stats are shown
    let state = Rc::new(RefCell::new(AppState::new()));
    {
        let mut s = state.borrow_mut();
        s.show_settings_dialog = true; // Force dialog open
        s.cache_stats = Some(CacheStats {
            thumbnail_count: 12345,
            large_preview_count: 678,
            total_size_bytes: 1024,
            db_path: PathBuf::from("custom_path.db"),
        });
    }

    let state_clone = state.clone();
    let mut harness = Harness::new_ui(move |ui| {
        let mut s = state_clone.borrow_mut();
        SettingsDialog::show(ui.ctx(), &mut s);
    });

    harness.run();

    // Verify stats text exists
    // "12345 items"
    let _ = harness.get_by_label("12345 items"); 
    // "678 items"
    let _ = harness.get_by_label("678 items");
}
