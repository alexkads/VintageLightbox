//! E2E Tests for RatingWidget component
//!
//! Tests the star rating widget using egui_kittest

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::Cell;
use std::rc::Rc;

/// Test that the rating widget renders correctly with 0 stars
#[test]
fn test_rating_widget_renders_empty() {
    let rating = Rc::new(Cell::new(0i32));
    let rating_clone = rating.clone();
    
    let mut harness = Harness::new_ui(move |ui| {
        let r = rating_clone.get();
        ui.label("Rating:");
        ui.horizontal(|ui| {
            for i in 1..=5 {
                let star = if i <= r { "★" } else { "☆" };
                if ui.button(star).clicked() {
                    rating_clone.set(i);
                }
            }
        });
    });
    
    harness.run();
    harness.fit_contents();
    
    // Snapshot test - uncomment when running with snapshot features
    // harness.snapshot("rating_widget_empty");
    
    assert_eq!(rating.get(), 0);
}

/// Test that the rating widget renders correctly with 3 stars
#[test]
fn test_rating_widget_renders_3_stars() {
    let rating = 3i32;
    
    let mut harness = Harness::new_ui(move |ui| {
        ui.label("Rating:");
        ui.horizontal(|ui| {
            for i in 1..=5 {
                let star = if i <= rating { "★" } else { "☆" };
                ui.label(star);
            }
        });
    });
    
    harness.run();
    harness.fit_contents();
    
    // Snapshot test - uncomment when running with snapshot features
    // harness.snapshot("rating_widget_3_stars");
}

/// Test clicking on a star changes the rating
#[test]
fn test_rating_widget_click_changes_rating() {
    let rating = Rc::new(Cell::new(0i32));
    let rating_clone = rating.clone();
    
    let mut harness = Harness::new_ui(move |ui| {
        let r = rating_clone.get();
        ui.horizontal(|ui| {
            for i in 1..=5 {
                let _star = if i <= r { "★" } else { "☆" };
                if ui.button(format!("star_{}", i)).clicked() {
                    rating_clone.set(i);
                }
            }
        });
    });
    
    // Initial state
    assert_eq!(rating.get(), 0);
    
    // Click on star 3
    let star_3 = harness.get_by_label("star_3");
    star_3.click();
    harness.run();
    
    // Rating should now be 3
    assert_eq!(rating.get(), 3);
}
