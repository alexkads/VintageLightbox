use ui::state::AppState;
use ui::components::filmstrip_filter::FilmstripFilter;
use adapters::view_models::PhotoViewModel;
use domain::value_objects::ColorLabel;

fn create_view_model(id: &str, color: Option<ColorLabel>) -> PhotoViewModel {
    let mut vm = PhotoViewModel::default();
    vm.id = id.to_string();
    vm.color_label = color.map(|c| c.to_string());
    vm
}

#[test]
fn test_sanitize_selection_auto_advances_when_filtered() {
    let mut state = AppState::new();

    // Setup 3 photos: 1 Red, 2 Blue
    let photo1 = create_view_model("photo1", Some(ColorLabel::Red));
    let photo2 = create_view_model("photo2", Some(ColorLabel::Blue));
    let photo3 = create_view_model("photo3", Some(ColorLabel::Blue));

    state.photos = vec![photo1, photo2, photo3];

    // Select Photo 1 (Red)
    state.develop_selected_photo_id = Some("photo1".to_string());
    state.loaded_photo_id = Some("photo1".to_string());

    // Apply Filter: Blue only
    state.filmstrip_filter.color_labels.insert(ColorLabel::Blue);

    // Filter excludes currently selected photo. Sanitize should switch to next available (photo2)
    state.sanitize_develop_selection();

    assert_eq!(state.develop_selected_photo_id, Some("photo2".to_string()), "Should advance to first available photo (photo2)");
    assert_eq!(state.loaded_photo_id, None, "Should force reload");

    // Case 2: Current photo IS allowed by filter
    state.develop_selected_photo_id = Some("photo3".to_string());
    state.loaded_photo_id = Some("photo3".to_string());

    state.sanitize_develop_selection();

    assert_eq!(state.develop_selected_photo_id, Some("photo3".to_string()), "Should remain on photo3 as it matches filter");
    assert_eq!(state.loaded_photo_id, Some("photo3".to_string()), "Should NOT force reload");
}

#[test]
fn test_sanitize_selection_clears_if_none_available() {
    let mut state = AppState::new();
    let photo1 = create_view_model("photo1", Some(ColorLabel::Red));
    state.photos = vec![photo1];

    state.develop_selected_photo_id = Some("photo1".to_string());

    // Filter: Blue only (none matches)
    state.filmstrip_filter.color_labels.insert(ColorLabel::Blue);

    state.sanitize_develop_selection();

    assert_eq!(state.develop_selected_photo_id, None, "Should clear selection if no photos match");
}
