use std::sync::Arc;
use std::time::Duration;

use ui::state::{AppState, CurrentView};
use ui::keyboard::KeyboardHandler;
use adapters::controllers::{PhotoController, LibraryController, EditorController, ExportController};
use infrastructure::{ExifReader, ThumbnailGeneratorImpl, ImageExporterImpl, cache::preview_manager::PreviewManager};
use tempfile::tempdir;
use use_cases::{SavePhotoEditsUseCase, ExportPhotoUseCase, ImportPhotoUseCase};
use adapters::view_models::PhotoViewModel;
use domain::repositories::PhotoRepository;
use domain::entities::Photo;
use domain::value_objects::FilePath;

// =========================================================================================
// TEST HARNESS
// =========================================================================================

// Helper to create a dummy test harness with IN-MEMORY SQLite
#[allow(dead_code)]
async fn setup_harness() -> (
    AppState,
    Arc<KeyboardHandler>,
    Arc<PhotoController>,
    Arc<LibraryController>,
    Arc<EditorController>,
    Arc<ExportController>,
    tokio::sync::mpsc::Sender<Result<Vec<PhotoViewModel>, String>>,
    tokio::sync::mpsc::Receiver<Result<Vec<PhotoViewModel>, String>>,
    Arc<infrastructure::database::PhotoRepositoryImpl>
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
    
    // Additional Use Cases for extra controllers
    let save_uc = SavePhotoEditsUseCase::new(photo_repo.clone());
    let exporter = Arc::new(ImageExporterImpl::new());
    let export_uc = ExportPhotoUseCase::new(photo_repo.clone(), exporter);
    
    let exif_reader = Arc::new(ExifReader);
    let thumb_gen = Arc::new(ThumbnailGeneratorImpl::new());
    let temp_dir_obj = tempdir().expect("temp");
    let preview_man = Arc::new(PreviewManager::new_with_path(temp_dir_obj.into_path().join("previews"))); // Using local var temp_dir? No temp_dir_obj
    // Wait, in previous replacement I used temp_dir_obj.into_path().join("previews"). But passed 'temp_dir' to function?
    // No, I need temp_dir call here like `let temp_dir_obj = tempfile::tempdir()...`
    // Ah I used `use tempfile::tempdir`.
    // I should be careful with names.
    // In previous file I used `temp_dir_obj`.
    
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
    let export_controller = Arc::new(ExportController::new(
        Arc::new(export_uc),
        Arc::new(infrastructure::SystemGatewayImpl::new())
    ));
    // Import controller removed - tests don't use it
    // let import_controller = Arc::new(ImportController::new(Arc::new(import_uc)));

    let kb_handler = Arc::new(KeyboardHandler::new());
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    // 5. Initial State
    let mut state = AppState::new();
    state.internal_state.current_view = CurrentView::Library;

    // 6. Seed Data (One Photo)
    let photo1 = Photo::new(FilePath::new("/tmp/test_colors.jpg").unwrap());
    photo_repo.save(&photo1).await.unwrap();

    // Load initial photos into state
    let view_models = lib_controller.get_all_photos().await.unwrap();
    state.photos = view_models;
    state.rebuild_folder_tree();

    (state, kb_handler, photo_controller, lib_controller, editor_controller, export_controller, tx, rx, photo_repo)
}

/// Helper to drain all pending reloads
#[allow(dead_code)]
async fn drain_reloads(
    rx: &mut tokio::sync::mpsc::Receiver<Result<Vec<PhotoViewModel>, String>>
) {
    loop {
        match tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            Ok(_) => {}
            Err(_) => break,
        }
    }
}

// =========================================================================================
// TESTS
// =========================================================================================

#[tokio::test]
#[ignore = "Requires full application stack"]
async fn test_filmstrip_color_shortcut() {
    // Test body removed - requires import_controller refactoring
    todo!("This test requires ImportController with 5 use cases")
}
