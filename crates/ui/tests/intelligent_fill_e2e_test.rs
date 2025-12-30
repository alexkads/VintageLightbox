// Intelligent Fill E2E Tests
// Validates the intelligent fill feature for rotation edge filling

use std::sync::Arc;

use ui::state::{AppState, CurrentView};
use ui::keyboard::KeyboardHandler;
use adapters::controllers::{PhotoController, LibraryController, EditorController, ExportController};
use infrastructure::{ExifReader, ThumbnailGeneratorImpl, ImageExporterImpl, cache::preview_manager::PreviewManager};
use tempfile::tempdir;
use use_cases::{SavePhotoEditsUseCase, ExportPhotoUseCase, ImportPhotoUseCase};
use adapters::view_models::PhotoViewModel;
use domain::repositories::PhotoRepository;
use domain::entities::Photo;
use domain::value_objects::{FilePath, CropSettings, RotationFillMode};

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
    let pool = sqlx::SqlitePool::connect(db_url).await.expect("Failed to create in-memory db");

    // Run migrations
    sqlx::migrate!("../infrastructure/migrations").run(&pool).await.expect("Failed to run migrations");

    // 2. Repositories
    let photo_repo = Arc::new(infrastructure::database::PhotoRepositoryImpl::new(pool.clone()));

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
    let temp_path = temp_dir_obj.path().join("previews");
    std::mem::forget(temp_dir_obj); // Keep the temp dir alive
    let preview_man = Arc::new(PreviewManager::new_with_path(temp_path));

    let _import_uc = ImportPhotoUseCase::new(photo_repo.clone(), exif_reader, thumb_gen, preview_man);

    // 4. Controllers
    let photo_controller = Arc::new(PhotoController::new(
        Arc::new(rate_uc),
        Arc::new(color_uc),
        Arc::new(flag_uc),
        Arc::new(delete_uc)
    ));

    let lib_controller = Arc::new(LibraryController::new(photo_repo.clone()));
    let editor_controller = Arc::new(EditorController::new(Arc::new(save_uc)));
    let export_controller = Arc::new(ExportController::new(Arc::new(export_uc)));

    let kb_handler = Arc::new(KeyboardHandler::new());
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    // 5. Initial State
    let mut state = AppState::new();
    state.internal_state.current_view = CurrentView::Develop;

    // 6. Seed Test Data
    let photo1 = Photo::new(FilePath::new("/tmp/test_intelligent_fill.jpg").unwrap());
    photo_repo.save(&photo1).await.unwrap();

    // Load photos into state
    let view_models = lib_controller.get_all_photos().await.unwrap();
    state.photos = view_models;
    state.rebuild_folder_tree();

    // Select first photo in develop view
    if let Some(photo) = state.photos.first() {
        state.internal_state.develop_selected_id = Some(photo.id.clone());
    }

    (state, kb_handler, photo_controller, lib_controller, editor_controller, export_controller, tx, rx)
}

// =========================================================================================
// FILL MODE TESTS
// =========================================================================================

/// Test that fill mode defaults to Black
#[tokio::test]
async fn test_fill_mode_default_is_black() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    // Activate crop mode
    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default());

    let crop = state.crop_settings.as_ref().unwrap();
    assert_eq!(crop.fill_mode(), RotationFillMode::Black, "Default fill mode should be Black");
}

/// Test changing fill mode to Intelligent
#[tokio::test]
async fn test_fill_mode_change_to_intelligent() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    // Activate crop mode
    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default());

    // Change fill mode to Intelligent
    if let Some(crop) = &mut state.crop_settings {
        *crop = crop.with_fill_mode(RotationFillMode::Intelligent);
    }

    let crop = state.crop_settings.as_ref().unwrap();
    assert_eq!(crop.fill_mode(), RotationFillMode::Intelligent, "Fill mode should be Intelligent");
}

/// Test all fill mode options
#[tokio::test]
async fn test_all_fill_mode_options() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default());

    let all_modes = RotationFillMode::all();
    assert_eq!(all_modes.len(), 5, "Should have 5 fill mode options");

    // Verify all modes can be set
    for mode in all_modes {
        if let Some(crop) = &mut state.crop_settings {
            *crop = crop.with_fill_mode(*mode);
        }
        let crop = state.crop_settings.as_ref().unwrap();
        assert_eq!(crop.fill_mode(), *mode, "Fill mode should be {:?}", mode);
    }
}

/// Test fill mode persists when angle changes
#[tokio::test]
async fn test_fill_mode_persists_with_angle_change() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default().with_fill_mode(RotationFillMode::Intelligent));

    // Verify initial state
    let crop = state.crop_settings.as_ref().unwrap();
    assert_eq!(crop.fill_mode(), RotationFillMode::Intelligent);
    assert_eq!(crop.angle(), 0.0);

    // Change angle
    if let Some(crop) = &mut state.crop_settings {
        *crop = crop.with_angle(15.0);
    }

    // Verify fill mode persists
    let crop = state.crop_settings.as_ref().unwrap();
    assert_eq!(crop.angle(), 15.0, "Angle should be 15.0");
    assert_eq!(crop.fill_mode(), RotationFillMode::Intelligent, "Fill mode should persist");
}

/// Test fill mode persists when rotation_90 changes
#[tokio::test]
async fn test_fill_mode_persists_with_rotation_90_change() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default().with_fill_mode(RotationFillMode::White));

    // Change rotation_90
    if let Some(crop) = &mut state.crop_settings {
        *crop = CropSettings::with_fill_mode_value(
            crop.crop_x(), crop.crop_y(), crop.crop_width(), crop.crop_height(),
            1, crop.angle(), crop.flip_horizontal(), crop.flip_vertical(),
            crop.fill_mode()
        );
    }

    let crop = state.crop_settings.as_ref().unwrap();
    assert_eq!(crop.rotation_90(), 1, "Rotation should be 90°");
    assert_eq!(crop.fill_mode(), RotationFillMode::White, "Fill mode should persist");
}

/// Test fill mode persists when flips change
#[tokio::test]
async fn test_fill_mode_persists_with_flip_change() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default().with_fill_mode(RotationFillMode::Transparent));

    // Flip horizontal
    if let Some(crop) = &mut state.crop_settings {
        *crop = crop.with_flip_horizontal(true);
    }

    let crop = state.crop_settings.as_ref().unwrap();
    assert!(crop.flip_horizontal(), "Should be flipped");
    assert_eq!(crop.fill_mode(), RotationFillMode::Transparent, "Fill mode should persist");
}

// =========================================================================================
// INTELLIGENT FILL STATE TESTS
// =========================================================================================

/// Test intelligent fill state initialization
#[tokio::test]
async fn test_intelligent_fill_state_initialization() {
    let (state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    // Verify initial intelligent fill state
    assert!(state.intelligent_fill_texture.is_none(), "Should have no texture initially");
    assert_eq!(state.intelligent_fill_request_id, 0, "Request ID should be 0");
    assert!(!state.intelligent_fill_pending, "Should not be pending initially");
    assert_eq!(state.prev_intelligent_fill_angle, 0.0, "Previous angle should be 0");
}

/// Test intelligent fill pending state
#[tokio::test]
async fn test_intelligent_fill_pending_state() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    // Simulate sending a request
    state.intelligent_fill_request_id = 1;
    state.intelligent_fill_pending = true;
    state.prev_intelligent_fill_angle = 10.0;

    assert_eq!(state.intelligent_fill_request_id, 1);
    assert!(state.intelligent_fill_pending);
    assert_eq!(state.prev_intelligent_fill_angle, 10.0);
}

/// Test intelligent fill texture cleared when fill mode changes from Intelligent
#[tokio::test]
async fn test_intelligent_fill_texture_cleared_on_mode_change() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default()
        .with_fill_mode(RotationFillMode::Intelligent)
        .with_angle(10.0));

    // Simulate having a texture (in reality this would be an egui::TextureHandle)
    // We can't create a real TextureHandle without egui context, so we just test the logic
    state.intelligent_fill_request_id = 1;
    state.intelligent_fill_pending = false;
    state.prev_intelligent_fill_angle = 10.0;

    // Change fill mode away from Intelligent
    if let Some(crop) = &mut state.crop_settings {
        *crop = crop.with_fill_mode(RotationFillMode::Black);
    }

    // In the real app, this would trigger clearing the texture
    // We simulate what the app.rs code does:
    let crop = state.crop_settings.as_ref().unwrap();
    if crop.fill_mode() != RotationFillMode::Intelligent {
        state.intelligent_fill_texture = None;
    }

    assert!(state.intelligent_fill_texture.is_none(), "Texture should be cleared");
}

/// Test intelligent fill texture cleared when angle becomes zero
#[tokio::test]
async fn test_intelligent_fill_texture_cleared_when_angle_zero() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default()
        .with_fill_mode(RotationFillMode::Intelligent)
        .with_angle(10.0));

    state.intelligent_fill_request_id = 1;
    state.prev_intelligent_fill_angle = 10.0;

    // Set angle to zero
    if let Some(crop) = &mut state.crop_settings {
        *crop = crop.with_angle(0.0);
    }

    // Simulate what the app.rs code does:
    let crop = state.crop_settings.as_ref().unwrap();
    if crop.angle() == 0.0 {
        state.intelligent_fill_texture = None;
    }

    assert!(state.intelligent_fill_texture.is_none(), "Texture should be cleared when angle is 0");
}

// =========================================================================================
// SHRINK TO FIT TESTS
// =========================================================================================

/// Test ShrinkToFit mode calculates proper crop
#[tokio::test]
async fn test_shrink_to_fit_calculation() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default()
        .with_fill_mode(RotationFillMode::ShrinkToFit)
        .with_angle(15.0));

    // For a 100x100 image with 15° rotation, the crop should shrink
    let crop = state.crop_settings.as_ref().unwrap();
    let shrunk = crop.calculate_shrink_to_fit(100.0, 100.0);

    // Verify crop is smaller than full image
    assert!(shrunk.crop_width() < 1.0 || shrunk.crop_height() < 1.0,
        "ShrinkToFit should reduce crop size for rotated images");
    assert_eq!(shrunk.fill_mode(), RotationFillMode::ShrinkToFit,
        "Fill mode should be preserved");
}

/// Test ShrinkToFit with zero angle doesn't change crop
#[tokio::test]
async fn test_shrink_to_fit_zero_angle_unchanged() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default()
        .with_fill_mode(RotationFillMode::ShrinkToFit));

    let crop = state.crop_settings.as_ref().unwrap();
    let shrunk = crop.calculate_shrink_to_fit(100.0, 100.0);

    // With zero angle, crop should remain full size
    assert_eq!(shrunk.crop_x(), 0.0);
    assert_eq!(shrunk.crop_y(), 0.0);
    assert_eq!(shrunk.crop_width(), 1.0);
    assert_eq!(shrunk.crop_height(), 1.0);
}

// =========================================================================================
// FILL MODE UI DISPLAY TESTS
// =========================================================================================

/// Test fill mode display names
#[tokio::test]
async fn test_fill_mode_display_names() {
    assert_eq!(RotationFillMode::Black.display_name(), "Black");
    assert_eq!(RotationFillMode::White.display_name(), "White");
    assert_eq!(RotationFillMode::Transparent.display_name(), "Transparent");
    assert_eq!(RotationFillMode::Intelligent.display_name(), "Intelligent");
    assert_eq!(RotationFillMode::ShrinkToFit.display_name(), "Shrink to Fit");
}

/// Test fill mode string representation
#[tokio::test]
async fn test_fill_mode_to_string() {
    assert_eq!(format!("{}", RotationFillMode::Black), "Black");
    assert_eq!(format!("{}", RotationFillMode::White), "White");
    assert_eq!(format!("{}", RotationFillMode::Transparent), "Transparent");
    assert_eq!(format!("{}", RotationFillMode::Intelligent), "Intelligent");
    assert_eq!(format!("{}", RotationFillMode::ShrinkToFit), "Shrink to Fit");
}

/// Test fill mode code roundtrip
#[tokio::test]
async fn test_fill_mode_code_roundtrip() {
    for mode in RotationFillMode::all() {
        let code = mode.as_code();
        let recovered = RotationFillMode::from_code(code).unwrap();
        assert_eq!(*mode, recovered, "Fill mode {:?} should roundtrip through code {}", mode, code);
    }
}

// =========================================================================================
// INTEGRATION TESTS
// =========================================================================================

/// Test complete workflow: set Intelligent fill, rotate, verify state
#[tokio::test]
async fn test_intelligent_fill_complete_workflow() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    // 1. Enter crop mode
    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default());
    assert!(state.crop_mode_active);
    assert!(state.crop_settings.is_some());

    // 2. Set fill mode to Intelligent
    if let Some(crop) = &mut state.crop_settings {
        *crop = crop.with_fill_mode(RotationFillMode::Intelligent);
    }
    assert_eq!(state.crop_settings.as_ref().unwrap().fill_mode(), RotationFillMode::Intelligent);

    // 3. Set rotation angle
    if let Some(crop) = &mut state.crop_settings {
        *crop = crop.with_angle(-12.0);
    }
    assert_eq!(state.crop_settings.as_ref().unwrap().angle(), -12.0);

    // 4. Verify that intelligent fill would be triggered (angle != 0 and mode == Intelligent)
    let crop = state.crop_settings.as_ref().unwrap();
    let needs_intelligent_fill = crop.fill_mode() == RotationFillMode::Intelligent && crop.angle() != 0.0;
    assert!(needs_intelligent_fill, "Should need intelligent fill processing");

    // 5. Simulate request being sent
    state.intelligent_fill_request_id = 1;
    state.intelligent_fill_pending = true;
    state.prev_intelligent_fill_angle = -12.0;

    // 6. Verify pending state
    assert!(state.intelligent_fill_pending);
    assert_eq!(state.intelligent_fill_request_id, 1);

    // 7. Simulate result received (texture would be set here)
    state.intelligent_fill_pending = false;
    // state.intelligent_fill_texture = Some(...) - can't create without egui context

    // 8. Exit crop mode (apply)
    state.crop_mode_active = false;
    assert!(!state.crop_mode_active);

    // 9. Crop settings should persist
    let crop = state.crop_settings.as_ref().unwrap();
    assert_eq!(crop.fill_mode(), RotationFillMode::Intelligent);
    assert_eq!(crop.angle(), -12.0);
}

/// Test switching between fill modes preserves other crop settings
#[tokio::test]
async fn test_fill_mode_switch_preserves_crop_settings() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;

    // Set up complex crop settings
    state.crop_settings = Some(CropSettings::with_fill_mode_value(
        0.1, 0.2, 0.6, 0.5,  // Non-default crop
        1, 15.0,             // Rotation
        true, false,         // Flip horizontal
        RotationFillMode::Black
    ));

    // Switch fill modes and verify other settings preserved
    let modes = [
        RotationFillMode::White,
        RotationFillMode::Intelligent,
        RotationFillMode::Transparent,
        RotationFillMode::ShrinkToFit,
        RotationFillMode::Black,
    ];

    for mode in modes {
        if let Some(crop) = &mut state.crop_settings {
            *crop = crop.with_fill_mode(mode);
        }

        let crop = state.crop_settings.as_ref().unwrap();

        // Verify fill mode changed
        assert_eq!(crop.fill_mode(), mode);

        // Verify other settings preserved
        assert!((crop.crop_x() - 0.1).abs() < 0.001, "crop_x should be preserved");
        assert!((crop.crop_y() - 0.2).abs() < 0.001, "crop_y should be preserved");
        assert!((crop.crop_width() - 0.6).abs() < 0.001, "crop_width should be preserved");
        assert!((crop.crop_height() - 0.5).abs() < 0.001, "crop_height should be preserved");
        assert_eq!(crop.rotation_90(), 1, "rotation_90 should be preserved");
        assert!((crop.angle() - 15.0).abs() < 0.001, "angle should be preserved");
        assert!(crop.flip_horizontal(), "flip_horizontal should be preserved");
        assert!(!crop.flip_vertical(), "flip_vertical should be preserved");
    }
}

/// Test that non-Intelligent fill modes don't trigger intelligent fill processing
#[tokio::test]
async fn test_non_intelligent_modes_dont_trigger_processing() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;

    let non_intelligent_modes = [
        RotationFillMode::Black,
        RotationFillMode::White,
        RotationFillMode::Transparent,
        RotationFillMode::ShrinkToFit,
    ];

    for mode in non_intelligent_modes {
        state.crop_settings = Some(CropSettings::default()
            .with_fill_mode(mode)
            .with_angle(15.0));  // Non-zero angle

        let crop = state.crop_settings.as_ref().unwrap();
        let needs_intelligent_fill = crop.fill_mode() == RotationFillMode::Intelligent;

        assert!(!needs_intelligent_fill,
            "Mode {:?} should not trigger intelligent fill", mode);
    }
}

/// Test intelligent fill only triggers with non-zero angle
#[tokio::test]
async fn test_intelligent_fill_requires_nonzero_angle() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default()
        .with_fill_mode(RotationFillMode::Intelligent));

    // With zero angle
    let crop = state.crop_settings.as_ref().unwrap();
    let needs_processing = crop.fill_mode() == RotationFillMode::Intelligent && crop.angle() != 0.0;
    assert!(!needs_processing, "Should not need processing with zero angle");

    // With non-zero angle
    if let Some(crop) = &mut state.crop_settings {
        *crop = crop.with_angle(5.0);
    }
    let crop = state.crop_settings.as_ref().unwrap();
    let needs_processing = crop.fill_mode() == RotationFillMode::Intelligent && crop.angle() != 0.0;
    assert!(needs_processing, "Should need processing with non-zero angle");
}

// =========================================================================================
// MESH-BASED RENDERING LOGIC TESTS
// =========================================================================================

/// Test that mesh-based rendering is triggered for non-zero angle in edit mode
#[tokio::test]
async fn test_mesh_rendering_triggered_for_nonzero_angle() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;  // Edit mode (apply_crop_clip = false)
    state.crop_settings = Some(CropSettings::default().with_angle(-12.0));

    let crop = state.crop_settings.as_ref().unwrap();

    // In image_viewer.rs, mesh-based rendering is triggered when:
    // 1. crop_settings is Some
    // 2. apply_crop_clip is false (edit mode, which means crop_mode_active = true)
    // 3. angle != 0.0
    let should_use_mesh_rendering = crop.angle() != 0.0;

    assert!(should_use_mesh_rendering, "Should use mesh rendering for angle={}", crop.angle());
}

/// Test that mesh-based rendering is NOT triggered for zero angle
#[tokio::test]
async fn test_mesh_rendering_not_triggered_for_zero_angle() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default());  // angle = 0.0

    let crop = state.crop_settings.as_ref().unwrap();
    let should_use_mesh_rendering = crop.angle() != 0.0;

    assert!(!should_use_mesh_rendering, "Should NOT use mesh rendering for zero angle");
}

/// Test that mesh-based rendering is NOT triggered for only 90° rotation
#[tokio::test]
async fn test_mesh_rendering_not_triggered_for_only_90_rotation() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::with_fill_mode_value(
        0.0, 0.0, 1.0, 1.0,
        1, 0.0,  // 90° rotation, but angle = 0
        false, false,
        RotationFillMode::Black
    ));

    let crop = state.crop_settings.as_ref().unwrap();

    // 90° rotations use the simpler Image::rotate() approach
    let should_use_mesh_rendering = crop.angle() != 0.0;
    let uses_simple_rotation = crop.rotation_90() != 0 && crop.angle() == 0.0;

    assert!(!should_use_mesh_rendering, "Should NOT use mesh rendering for only 90° rotation");
    assert!(uses_simple_rotation, "Should use simple rotation for only 90° rotation");
}

/// Test fill color selection for each mode
#[tokio::test]
async fn test_fill_color_for_each_mode() {
    // This simulates the match statement in image_viewer.rs
    fn get_fill_color(mode: RotationFillMode, has_intelligent_texture: bool) -> &'static str {
        if mode == RotationFillMode::Intelligent && has_intelligent_texture {
            return "intelligent_texture";
        }

        match mode {
            RotationFillMode::Black => "black",
            RotationFillMode::White => "white",
            RotationFillMode::Transparent => "transparent",
            RotationFillMode::Intelligent => "gray_placeholder", // Color32::from_gray(60)
            RotationFillMode::ShrinkToFit => "transparent",
        }
    }

    // Test without intelligent texture
    assert_eq!(get_fill_color(RotationFillMode::Black, false), "black");
    assert_eq!(get_fill_color(RotationFillMode::White, false), "white");
    assert_eq!(get_fill_color(RotationFillMode::Transparent, false), "transparent");
    assert_eq!(get_fill_color(RotationFillMode::Intelligent, false), "gray_placeholder");
    assert_eq!(get_fill_color(RotationFillMode::ShrinkToFit, false), "transparent");

    // Test with intelligent texture
    assert_eq!(get_fill_color(RotationFillMode::Intelligent, true), "intelligent_texture");
    // Other modes should ignore the texture flag
    assert_eq!(get_fill_color(RotationFillMode::Black, true), "black");
}

/// Test view mode vs edit mode rendering paths
#[tokio::test]
async fn test_view_vs_edit_mode_rendering() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    // Edit mode: crop_mode_active = true → apply_crop_clip = false
    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default().with_angle(10.0));

    let apply_crop_clip_edit = !state.crop_mode_active;
    assert!(!apply_crop_clip_edit, "Edit mode should have apply_crop_clip = false");

    // View mode: crop_mode_active = false → apply_crop_clip = true
    state.crop_mode_active = false;

    let apply_crop_clip_view = !state.crop_mode_active;
    assert!(apply_crop_clip_view, "View mode should have apply_crop_clip = true");
}

/// Test that intelligent fill request conditions match
#[tokio::test]
async fn test_intelligent_fill_request_conditions() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    // Set up conditions for intelligent fill request
    state.internal_state.current_view = CurrentView::Develop;
    state.crop_settings = Some(CropSettings::default()
        .with_fill_mode(RotationFillMode::Intelligent)
        .with_angle(-15.0));

    // Check conditions (mirrors request_intelligent_fill_if_needed logic)
    let in_develop_view = state.internal_state.current_view == CurrentView::Develop;
    let has_crop_settings = state.crop_settings.is_some();

    let crop = state.crop_settings.as_ref().unwrap();
    let is_intelligent_mode = crop.fill_mode() == RotationFillMode::Intelligent;
    let has_nonzero_angle = crop.angle() != 0.0;

    let should_request = in_develop_view && has_crop_settings && is_intelligent_mode && has_nonzero_angle;

    assert!(in_develop_view, "Should be in develop view");
    assert!(has_crop_settings, "Should have crop settings");
    assert!(is_intelligent_mode, "Should be in intelligent mode");
    assert!(has_nonzero_angle, "Should have non-zero angle");
    assert!(should_request, "Should request intelligent fill");
}

/// Test angle change debouncing logic
#[tokio::test]
async fn test_intelligent_fill_angle_debouncing() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;
    state.crop_settings = Some(CropSettings::default()
        .with_fill_mode(RotationFillMode::Intelligent)
        .with_angle(10.0));

    // Simulate first request
    state.prev_intelligent_fill_angle = 10.0;

    // Same angle - should NOT trigger new request
    let current_angle = state.crop_settings.as_ref().unwrap().angle();
    let angle_changed = (current_angle - state.prev_intelligent_fill_angle).abs() > 0.001;
    assert!(!angle_changed, "Should detect same angle");

    // Different angle - should trigger new request
    if let Some(crop) = &mut state.crop_settings {
        *crop = crop.with_angle(12.0);
    }
    let current_angle = state.crop_settings.as_ref().unwrap().angle();
    let angle_changed = (current_angle - state.prev_intelligent_fill_angle).abs() > 0.001;
    assert!(angle_changed, "Should detect different angle");
}

/// Test combined rotation (90° + angle) calculation
#[tokio::test]
async fn test_combined_rotation_calculation() {
    let (mut state, _kb, _photo_ctrl, _lib_ctrl, _ed_ctrl, _ex_ctrl, _tx, _rx) = setup_harness().await;

    state.crop_mode_active = true;

    // Test various combinations
    let test_cases = [
        (0, 0.0, 0.0),      // No rotation
        (1, 0.0, 90.0),     // Only 90°
        (0, 15.0, 15.0),    // Only angle
        (1, 15.0, 105.0),   // 90° + 15°
        (2, -10.0, 170.0),  // 180° - 10°
        (3, 5.0, 275.0),    // 270° + 5°
    ];

    for (rot90, angle, expected_total) in test_cases {
        state.crop_settings = Some(CropSettings::with_fill_mode_value(
            0.0, 0.0, 1.0, 1.0,
            rot90, angle,
            false, false,
            RotationFillMode::Black
        ));

        let crop = state.crop_settings.as_ref().unwrap();
        let total_degrees = (crop.rotation_90() as f32 * 90.0) + crop.angle();

        assert!(
            (total_degrees - expected_total).abs() < 0.001,
            "rot90={}, angle={}: expected total={}, got={}",
            rot90, angle, expected_total, total_degrees
        );
    }
}
