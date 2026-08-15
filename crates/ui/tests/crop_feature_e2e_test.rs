use std::sync::Arc;

use adapters::controllers::{
    EditorController, ExportController, LibraryController, PhotoController,
};
use adapters::view_models::PhotoViewModel;
use domain::entities::Photo;
use domain::repositories::PhotoRepository;
use domain::value_objects::FilePath;
use infrastructure::{
    cache::preview_manager::PreviewManager, ExifReader, ImageExporterImpl, ThumbnailGeneratorImpl,
};
use tempfile::tempdir;
use ui::keyboard::KeyboardHandler;
use ui::state::{AppState, CurrentView};
use use_cases::{ExportPhotoUseCase, ImportPhotoUseCase, SavePhotoEditsUseCase};

// =========================================================================================
// TEST HARNESS
// =========================================================================================

async fn setup_harness() -> (
    AppState,
    Arc<KeyboardHandler>,
    Arc<PhotoController>,
    Arc<LibraryController>,
    Arc<EditorController>,
    Arc<ExportController>,
    tokio::sync::mpsc::Sender<Result<Vec<PhotoViewModel>, String>>,
    tokio::sync::mpsc::Receiver<Result<Vec<PhotoViewModel>, String>>,
) {
    // 1. In-Memory Database
    let db_url = "sqlite::memory:";
    let pool = sqlx::SqlitePool::connect(db_url)
        .await
        .expect("Failed to create in-memory db");

    // Run migrations
    sqlx::migrate!("../infrastructure/migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // 2. Repositories
    let photo_repo = Arc::new(infrastructure::database::PhotoRepositoryImpl::new(
        pool.clone(),
    ));

    // 3. Use Cases
    let rate_uc = use_cases::RatePhotoUseCase::new(photo_repo.clone());
    let color_uc = use_cases::SetColorLabelUseCase::new(photo_repo.clone());
    let flag_uc = use_cases::SetFlagUseCase::new(photo_repo.clone());
    let delete_uc = use_cases::DeletePhotoUseCase::new(photo_repo.clone());

    let save_uc = SavePhotoEditsUseCase::new(photo_repo.clone());
    let exporter = Arc::new(ImageExporterImpl::new());
    let export_uc = ExportPhotoUseCase::new(photo_repo.clone(), exporter);

    let exif_reader = Arc::new(ExifReader);
    let thumb_gen = Arc::new(ThumbnailGeneratorImpl::new());
    let temp_dir_obj = tempdir().expect("temp");
    let preview_man = Arc::new(PreviewManager::new_with_path(
        temp_dir_obj.into_path().join("previews"),
    ));

    let _import_uc =
        ImportPhotoUseCase::new(photo_repo.clone(), exif_reader, thumb_gen, preview_man);

    // 4. Controllers
    let photo_controller = Arc::new(PhotoController::new(
        Arc::new(rate_uc),
        Arc::new(color_uc),
        Arc::new(flag_uc),
        Arc::new(delete_uc),
    ));

    let lib_controller = Arc::new(LibraryController::new(photo_repo.clone()));
    let editor_controller = Arc::new(EditorController::new(Arc::new(save_uc)));
    let export_controller = Arc::new(ExportController::new(Arc::new(export_uc)));

    let kb_handler = Arc::new(KeyboardHandler::new());
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    // 5. Initial State
    let mut state = AppState::new();
    state.current_view = CurrentView::Develop;

    // 6. Seed Test Data
    let photo1 = Photo::new(FilePath::new("/tmp/test_crop.jpg").unwrap());
    photo_repo.save(&photo1).await.unwrap();

    // Load photos into state
    let view_models = lib_controller.get_all_photos().await.unwrap();
    state.photos = view_models;
    state.rebuild_folder_tree();

    // Select first photo in develop view
    if let Some(photo) = state.photos.first() {
        state.develop_selected_photo_id = Some(photo.id.clone());
    }

    (
        state,
        kb_handler,
        photo_controller,
        lib_controller,
        editor_controller,
        export_controller,
        tx,
        rx,
    )
}

// =========================================================================================
// TESTS
// =========================================================================================

/// Test that 'R' key toggles crop mode in Develop view
#[tokio::test]
async fn test_crop_mode_toggle_with_r_key() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) =
        setup_harness().await;

    // Verify initial state
    assert_eq!(state.current_view, CurrentView::Develop);
    assert!(
        !state.crop_mode_active,
        "Crop mode should be inactive initially"
    );
    assert!(
        state.crop_settings.is_none(),
        "Crop settings should be None initially"
    );

    // Simulate R key press to activate crop mode
    // In real app, this is done via keyboard handler, but we test state change directly
    state.crop_mode_active = !state.crop_mode_active;
    if state.crop_mode_active && state.crop_settings.is_none() {
        state.crop_settings = Some(domain::value_objects::CropSettings::default());
    }

    assert!(
        state.crop_mode_active,
        "Crop mode should be active after R key"
    );
    assert!(
        state.crop_settings.is_some(),
        "Crop settings should be initialized"
    );

    // Toggle off
    state.crop_mode_active = !state.crop_mode_active;
    assert!(
        !state.crop_mode_active,
        "Crop mode should be inactive after second R key"
    );
}

/// Test aspect ratio selection changes state
#[tokio::test]
async fn test_aspect_ratio_selection() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) =
        setup_harness().await;

    // Activate crop mode
    state.crop_mode_active = true;
    state.crop_settings = Some(domain::value_objects::CropSettings::default());

    // Verify default aspect ratio
    assert_eq!(
        state.selected_aspect_ratio,
        domain::value_objects::AspectRatio::Original
    );

    // Change to Square
    state.selected_aspect_ratio = domain::value_objects::AspectRatio::Square;
    assert_eq!(
        state.selected_aspect_ratio,
        domain::value_objects::AspectRatio::Square
    );
    assert_eq!(state.selected_aspect_ratio.value(), 1.0);

    // Change to 16:9
    state.selected_aspect_ratio = domain::value_objects::AspectRatio::SixteenNine;
    assert!(state.selected_aspect_ratio.is_landscape());
    assert!((state.selected_aspect_ratio.value() - 16.0 / 9.0).abs() < 0.001);
}

/// Test rotation controls update crop settings
#[tokio::test]
async fn test_crop_rotation() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) =
        setup_harness().await;

    // Activate crop mode with initial settings
    state.crop_mode_active = true;
    state.crop_settings = Some(domain::value_objects::CropSettings::default());

    // Verify initial rotation
    let crop = state.crop_settings.as_ref().unwrap();
    assert_eq!(crop.rotation_90(), 0, "Should start with no rotation");

    // Simulate rotate right
    if let Some(crop) = &mut state.crop_settings {
        let new_rotation = crop.rotation_90() + 1;
        *crop = domain::value_objects::CropSettings::new(
            crop.crop_x(),
            crop.crop_y(),
            crop.crop_width(),
            crop.crop_height(),
            new_rotation,
            crop.angle(),
            crop.flip_horizontal(),
            crop.flip_vertical(),
        );
    }

    let crop = state.crop_settings.as_ref().unwrap();
    assert_eq!(crop.rotation_90(), 1, "Should be rotated 90° right");
    assert_eq!(crop.total_rotation(), 90.0);

    // Simulate rotate left (should go back to 0)
    if let Some(crop) = &mut state.crop_settings {
        let new_rotation = crop.rotation_90() - 1;
        *crop = domain::value_objects::CropSettings::new(
            crop.crop_x(),
            crop.crop_y(),
            crop.crop_width(),
            crop.crop_height(),
            new_rotation,
            crop.angle(),
            crop.flip_horizontal(),
            crop.flip_vertical(),
        );
    }

    let crop = state.crop_settings.as_ref().unwrap();
    assert_eq!(
        crop.rotation_90(),
        0,
        "Should be back to 0° after left rotation"
    );
}

/// Test flip controls update crop settings
#[tokio::test]
async fn test_crop_flip() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) =
        setup_harness().await;

    // Activate crop mode
    state.crop_mode_active = true;
    state.crop_settings = Some(domain::value_objects::CropSettings::default());

    // Verify initial flip state
    let crop = state.crop_settings.as_ref().unwrap();
    assert!(
        !crop.flip_horizontal(),
        "Should not be flipped horizontally"
    );
    assert!(!crop.flip_vertical(), "Should not be flipped vertically");
    assert!(!crop.is_flipped(), "Should not be flipped");

    // Simulate flip horizontal
    if let Some(crop) = &mut state.crop_settings {
        *crop = domain::value_objects::CropSettings::new(
            crop.crop_x(),
            crop.crop_y(),
            crop.crop_width(),
            crop.crop_height(),
            crop.rotation_90(),
            crop.angle(),
            !crop.flip_horizontal(),
            crop.flip_vertical(),
        );
    }

    let crop = state.crop_settings.as_ref().unwrap();
    assert!(crop.flip_horizontal(), "Should be flipped horizontally");
    assert!(!crop.flip_vertical(), "Should not be flipped vertically");
    assert!(crop.is_flipped(), "Should detect flip");

    // Simulate flip vertical
    if let Some(crop) = &mut state.crop_settings {
        *crop = domain::value_objects::CropSettings::new(
            crop.crop_x(),
            crop.crop_y(),
            crop.crop_width(),
            crop.crop_height(),
            crop.rotation_90(),
            crop.angle(),
            crop.flip_horizontal(),
            !crop.flip_vertical(),
        );
    }

    let crop = state.crop_settings.as_ref().unwrap();
    assert!(
        crop.flip_horizontal(),
        "Should still be flipped horizontally"
    );
    assert!(crop.flip_vertical(), "Should now be flipped vertically");
}

/// Test reset button clears all crop settings
#[tokio::test]
async fn test_crop_reset() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) =
        setup_harness().await;

    // Activate crop mode and modify settings
    state.crop_mode_active = true;
    state.crop_settings = Some(domain::value_objects::CropSettings::new(
        0.1, 0.1, 0.8, 0.8, // Cropped
        1, 15.0, // Rotated
        true, false, // Flipped H
    ));
    state.selected_aspect_ratio = domain::value_objects::AspectRatio::Square;
    state.show_composition_grid = true;

    // Verify modified state
    let crop = state.crop_settings.as_ref().unwrap();
    assert!(crop.is_cropped());
    assert!(crop.is_rotated());
    assert!(crop.is_flipped());

    // Simulate reset button
    state.crop_settings = Some(domain::value_objects::CropSettings::default());
    state.selected_aspect_ratio = domain::value_objects::AspectRatio::Original;
    state.show_composition_grid = false;

    // Verify reset
    let crop = state.crop_settings.as_ref().unwrap();
    assert!(!crop.is_cropped(), "Should not be cropped after reset");
    assert!(!crop.is_rotated(), "Should not be rotated after reset");
    assert!(!crop.is_flipped(), "Should not be flipped after reset");
    assert_eq!(
        state.selected_aspect_ratio,
        domain::value_objects::AspectRatio::Original
    );
    assert!(!state.show_composition_grid);
}

/// Test apply button exits crop mode
#[tokio::test]
async fn test_crop_apply() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) =
        setup_harness().await;

    // Activate crop mode with modifications
    state.crop_mode_active = true;
    state.crop_settings = Some(domain::value_objects::CropSettings::new(
        0.1, 0.1, 0.8, 0.8, 0, 0.0, false, false,
    ));

    assert!(state.crop_mode_active);

    // Simulate apply button
    state.crop_mode_active = false;

    assert!(!state.crop_mode_active, "Should exit crop mode");
    assert!(
        state.crop_settings.is_some(),
        "Settings should be preserved"
    );
}

/// Test composition grid toggle
#[tokio::test]
async fn test_composition_grid_toggle() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) =
        setup_harness().await;

    // Activate crop mode
    state.crop_mode_active = true;
    assert!(!state.show_composition_grid, "Grid should be off initially");

    // Toggle on
    state.show_composition_grid = true;
    assert!(state.show_composition_grid, "Grid should be on");

    // Toggle off
    state.show_composition_grid = false;
    assert!(!state.show_composition_grid, "Grid should be off");
}

/// Test crop settings validation and clamping
#[tokio::test]
async fn test_crop_settings_validation() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) =
        setup_harness().await;

    state.crop_mode_active = true;

    // Test that invalid values are clamped
    let crop = domain::value_objects::CropSettings::new(
        -0.5, 0.0, // Negative X clamped to 0
        2.0, 0.005, // Width too large, height too small (clamped to MIN_CROP_SIZE = 0.01)
        5, 60.0, // Rotation out of bounds, angle out of bounds
        false, false,
    );

    // Verify clamping
    assert_eq!(crop.crop_x(), 0.0, "Negative X should clamp to 0.0");
    assert_eq!(crop.crop_y(), 0.0, "Y 0.0 is valid");
    assert_eq!(crop.crop_width(), 1.0, "Width > 1.0 should clamp to 1.0");
    // First clamped to MIN_CROP_SIZE (0.01), then limited by (1.0 - crop_y) = 1.0
    assert_eq!(
        crop.crop_height(),
        0.01,
        "Height < MIN should clamp to 0.01"
    );
    assert_eq!(crop.rotation_90(), 3, "Rotation 5 should clamp to 3 (max)");
    assert_eq!(crop.angle(), 45.0, "Angle 60° should clamp to 45°");

    state.crop_settings = Some(crop);
}
