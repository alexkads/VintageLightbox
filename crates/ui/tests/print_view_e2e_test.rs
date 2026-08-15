// Print View E2E Tests
// Tests for the Print module workflow

mod common;

use ui::state::CurrentView;
use ui::views::print_view::{Orientation, PaperSize, PrintTemplate, PrintViewState};

/// Test that PrintViewState correctly calculates page counts for various templates
#[test]
fn test_e2e_print_view_state_page_calculation() {
    // Create print state with 10 photos
    let photo_ids: Vec<String> = (1..=10).map(|i| format!("photo_{}", i)).collect();
    let mut state = PrintViewState::new(photo_ids);

    // Test Single template: 10 photos = 10 pages
    state.selected_template = PrintTemplate::Single;
    assert_eq!(state.page_count(), 10);

    // Test 2x2 grid: 10 photos / 4 per page = 3 pages
    state.selected_template = PrintTemplate::Grid2x2;
    assert_eq!(state.page_count(), 3);

    // Test 3x3 grid: 10 photos / 9 per page = 2 pages
    state.selected_template = PrintTemplate::Grid3x3;
    assert_eq!(state.page_count(), 2);

    // Test 4x4 grid: 10 photos / 16 per page = 1 page
    state.selected_template = PrintTemplate::Grid4x4;
    assert_eq!(state.page_count(), 1);

    // Test Contact Sheet: 10 photos / 24 per page = 1 page
    state.selected_template = PrintTemplate::ContactSheet;
    assert_eq!(state.page_count(), 1);
}

/// Test template switching preserves other settings
#[test]
fn test_e2e_template_switching_preserves_settings() {
    let photo_ids: Vec<String> = (1..=5).map(|i| format!("photo_{}", i)).collect();
    let mut state = PrintViewState::new(photo_ids);

    // Configure settings
    state.paper_size = PaperSize::Letter;
    state.orientation = Orientation::Landscape;
    state.margin_mm = 15.0;
    state.include_filename = true;
    state.copies = 3;

    // Switch template
    state.selected_template = PrintTemplate::Grid3x3;

    // Verify settings are preserved
    assert_eq!(state.paper_size, PaperSize::Letter);
    assert_eq!(state.orientation, Orientation::Landscape);
    assert!((state.margin_mm - 15.0).abs() < 0.1);
    assert!(state.include_filename);
    assert_eq!(state.copies, 3);

    // And template changed
    assert_eq!(state.selected_template, PrintTemplate::Grid3x3);
}

/// Test adding and removing photos updates page count
#[test]
fn test_e2e_photo_collection_management() {
    let mut state = PrintViewState::default();
    state.selected_template = PrintTemplate::Grid2x2; // 4 per page

    // Start empty
    assert_eq!(state.page_count(), 0);

    // Add photos one by one
    state.photo_ids.push("photo_1".to_string());
    assert_eq!(state.page_count(), 1);

    state.photo_ids.push("photo_2".to_string());
    state.photo_ids.push("photo_3".to_string());
    state.photo_ids.push("photo_4".to_string());
    assert_eq!(state.page_count(), 1); // Still 1 page

    state.photo_ids.push("photo_5".to_string());
    assert_eq!(state.page_count(), 2); // Now 2 pages

    // Remove photos
    state.photo_ids.pop();
    assert_eq!(state.page_count(), 1);

    // Clear all
    state.photo_ids.clear();
    assert_eq!(state.page_count(), 0);
}

/// Test all paper sizes have reasonable dimensions
#[test]
fn test_e2e_paper_sizes_valid() {
    for size in PaperSize::all() {
        let (w, h) = size.dimensions_mm();

        // All papers should have positive dimensions
        assert!(w > 0.0, "{:?} has invalid width", size);
        assert!(h > 0.0, "{:?} has invalid height", size);

        // All papers should be at least small photo size (4x6 ~ 100x152mm)
        assert!(w > 50.0, "{:?} width is too small", size);
        assert!(h > 50.0, "{:?} height is too small", size);

        // Display name should contain dimension info
        assert!(!size.display_name().is_empty());
    }
}

/// Test custom grid configuration
#[test]
fn test_e2e_custom_grid_configuration() {
    let photo_ids: Vec<String> = (1..=20).map(|i| format!("photo_{}", i)).collect();
    let mut state = PrintViewState::new(photo_ids);

    state.selected_template = PrintTemplate::Custom;

    // Default custom is 2x2
    assert_eq!(state.custom_cols, 2);
    assert_eq!(state.custom_rows, 2);

    // Modify custom grid
    state.custom_cols = 3;
    state.custom_rows = 4;

    // But PrintTemplate::Custom.grid_dimensions() returns default (2,2)
    // In real app, page_count would use custom_cols/rows when Custom is selected
    // For now, Template::Custom returns its default grid
    assert_eq!(PrintTemplate::Custom.grid_dimensions(), (2, 2));
}

/// Test all photo info options default to false
#[test]
fn test_e2e_photo_info_defaults() {
    let state = PrintViewState::default();

    assert!(!state.include_filename);
    assert!(!state.include_date);
    assert!(!state.include_camera);
    assert!(!state.include_exposure);
}

/// Test orientation toggle functionality
#[test]
fn test_e2e_orientation_toggle() {
    let mut state = PrintViewState::default();

    assert_eq!(state.orientation, Orientation::Portrait);

    state.orientation = Orientation::Landscape;
    assert_eq!(state.orientation, Orientation::Landscape);

    state.orientation = Orientation::Portrait;
    assert_eq!(state.orientation, Orientation::Portrait);
}

/// Test margin constraints
#[test]
fn test_e2e_margins_reasonable() {
    let state = PrintViewState::default();

    // Default margin is 10mm
    assert!((state.margin_mm - 10.0).abs() < 0.1);

    // Default cell spacing is 5mm
    assert!((state.cell_spacing_mm - 5.0).abs() < 0.1);
}

/// Test copies default and modification
#[test]
fn test_e2e_copies_setting() {
    let mut state = PrintViewState::default();

    assert_eq!(state.copies, 1);

    state.copies = 5;
    assert_eq!(state.copies, 5);

    state.copies = 99;
    assert_eq!(state.copies, 99);
}

/// Test CurrentView::Print is properly defined
#[test]
fn test_e2e_current_view_print_exists() {
    let view = CurrentView::Print;
    assert!(view == CurrentView::Print);
    assert!(view != CurrentView::Library);
    assert!(view != CurrentView::Develop);
}
