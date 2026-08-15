use adapters::view_models::PhotoViewModel;
use ui::state::AppState;

/// Helper to create a dummy photo with a specific flag
fn create_photo(id: &str, flag: Option<i32>) -> PhotoViewModel {
    PhotoViewModel {
        id: id.to_string(),
        name: format!("Photo {}", id),
        path: format!("/path/to/{}", id),
        thumbnail_path: None,
        date: "2023-01-01".to_string(),
        camera: "Test Camera".to_string(),
        exposure: "1/100".to_string(),
        rating: 0,
        color_label: None,
        flag, // 1=Pick, -1=Reject, None/0=Unflagged
        edit_exposure: None,
        edit_contrast: None,
        edit_temperature: None,
        edit_tint: None,
        edit_highlights: None,
        edit_shadows: None,
        edit_whites: None,
        edit_blacks: None,
        edit_clarity: None,
        edit_vibrance: None,
        edit_saturation: None,
        edit_tone_curve_shadows: None,
        edit_tone_curve_darks: None,
        edit_tone_curve_lights: None,
        edit_tone_curve_highlights: None,
        ..Default::default()
    }
}

#[test]
fn test_filter_by_flags() {
    // Setup state
    let mut state = AppState::new();

    // Add photos with different flags
    state.photos = vec![
        create_photo("1", Some(1)),  // Picked
        create_photo("2", Some(-1)), // Rejected
        create_photo("3", Some(0)),  // Unflagged (Explicit 0)
        create_photo("4", None),     // Unflagged (None)
        create_photo("5", Some(1)),  // Picked
    ];

    // 1. Test Filter: Picked (1)
    state.filmstrip_filter.reset();
    state.filmstrip_filter.show_flagged = true;
    let filtered_refs = state.filmstrip_filter.apply(&state.photos);
    let filtered: Vec<PhotoViewModel> = filtered_refs.into_iter().cloned().collect();
    assert_eq!(filtered.len(), 2, "Should have 2 picked photos");
    assert!(filtered.iter().any(|p| p.id == "1"));
    assert!(filtered.iter().any(|p| p.id == "5"));

    // 2. Test Filter: Rejected (-1)
    state.filmstrip_filter.reset();
    state.filmstrip_filter.show_rejected = true;
    let filtered_refs = state.filmstrip_filter.apply(&state.photos);
    let filtered: Vec<PhotoViewModel> = filtered_refs.into_iter().cloned().collect();
    assert_eq!(filtered.len(), 1, "Should have 1 rejected photo");
    assert_eq!(filtered[0].id, "2");

    // 3. Test Filter: Unflagged (0)
    state.filmstrip_filter.reset();
    state.filmstrip_filter.show_unflagged = true;
    let filtered_refs = state.filmstrip_filter.apply(&state.photos);
    let filtered: Vec<PhotoViewModel> = filtered_refs.into_iter().cloned().collect();
    assert_eq!(
        filtered.len(),
        2,
        "Should have 2 unflagged photos (Explicit 0 and None)"
    );
    assert!(filtered.iter().any(|p| p.id == "3"));
    assert!(filtered.iter().any(|p| p.id == "4"));

    // 4. Test Filter: All (None)
    state.filmstrip_filter.reset();
    // Default is all off = show all
    let filtered_refs = state.filmstrip_filter.apply(&state.photos);
    let filtered: Vec<PhotoViewModel> = filtered_refs.into_iter().cloned().collect();
    assert_eq!(filtered.len(), 5, "Should return all photos");
}

#[test]
fn test_filter_combined_with_rating() {
    let mut state = AppState::new();

    // Add photos with flags AND ratings
    let mut p1 = create_photo("1", Some(1));
    p1.rating = 5; // Picked, 5 stars
    let mut p2 = create_photo("2", Some(1));
    p2.rating = 3; // Picked, 3 stars
    let mut p3 = create_photo("3", Some(-1));
    p3.rating = 5; // Rejected, 5 stars

    state.photos = vec![p1, p2, p3];

    // Filter: Picked AND Rating >= 4
    state.filmstrip_filter.show_flagged = true;
    state.filmstrip_filter.min_rating = 4;

    let filtered_refs = state.filmstrip_filter.apply(&state.photos);
    let filtered: Vec<PhotoViewModel> = filtered_refs.into_iter().cloned().collect();
    assert_eq!(filtered.len(), 1, "Should filter by both Flag and Rating");
    assert_eq!(filtered[0].id, "1");
}
