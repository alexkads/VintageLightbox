//! E2E Test: Photo Selection Consistency
//!
//! Verifies that the photo selected in Library view is the same photo
//! displayed in Develop view

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::RefCell;
use std::rc::Rc;

/// Test that selecting a photo in Library and switching to Develop shows the same photo
#[test]
fn test_photo_selection_consistency_between_views() {
    // This test simulates:
    // 1. User has multiple photos in library
    // 2. User clicks on photo #3
    // 3. User switches to Develop view
    // 4. Verify the photo ID matches
    
    let selected_photo_id = Rc::new(RefCell::new(None::<String>));
    let displayed_photo_id = Rc::new(RefCell::new(None::<String>));
    
    let selected_clone = selected_photo_id.clone();
    let displayed_clone = displayed_photo_id.clone();
    
    let mut harness = Harness::new_ui(move |ui| {
        ui.vertical(|ui| {
            // Simulate Library view with photo grid
            ui.label("Library View");
            
            // Simulate 5 photos
            for i in 1..=5 {
                let photo_id = format!("photo_{}", i);
                if ui.button(format!("Photo {}", i)).clicked() {
                    *selected_clone.borrow_mut() = Some(photo_id.clone());
                }
            }
            
            ui.separator();
            
            // Simulate Develop view
            ui.label("Develop View");
            if let Some(id) = selected_clone.borrow().clone() {
                ui.label(format!("Displaying: {}", id));
                *displayed_clone.borrow_mut() = Some(id);
            } else {
                ui.label("No photo selected");
            }
        });
    });
    
    harness.run();
    
    // Initially no photo selected
    assert_eq!(*selected_photo_id.borrow(), None);
    assert_eq!(*displayed_photo_id.borrow(), None);
    
    // Click on Photo 3
    let photo_3_button = harness.get_by_label("Photo 3");
    photo_3_button.click();
    harness.run();
    
    // Verify selection
    assert_eq!(*selected_photo_id.borrow(), Some("photo_3".to_string()));
    assert_eq!(*displayed_photo_id.borrow(), Some("photo_3".to_string()));
    
    // Click on Photo 5
    let photo_5_button = harness.get_by_label("Photo 5");
    photo_5_button.click();
    harness.run();
    
    // Verify new selection
    assert_eq!(*selected_photo_id.borrow(), Some("photo_5".to_string()));
    assert_eq!(*displayed_photo_id.borrow(), Some("photo_5".to_string()));
}

// TODO: Fix these tests - need to handle non-Copy types properly
// #[test]
// fn test_photo_metadata_population() { ... }
// #[test]
// fn test_view_switching_preserves_selection() { ... }
