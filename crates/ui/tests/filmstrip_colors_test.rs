use std::sync::Arc;
use std::time::Duration;

use ui::state::{AppState, CurrentView};
use ui::keyboard::KeyboardHandler;
use adapters::controllers::{PhotoController, LibraryController};
use adapters::view_models::PhotoViewModel;
use domain::repositories::PhotoRepository;
use domain::entities::Photo;
use domain::value_objects::FilePath;

// =========================================================================================
// TEST HARNESS
// =========================================================================================

// Helper to create a dummy test harness with IN-MEMORY SQLite
async fn setup_harness() -> (
    AppState,
    Arc<KeyboardHandler>,
    Arc<PhotoController>,
    Arc<LibraryController>,
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

    // 4. Controllers
    let photo_controller = Arc::new(PhotoController::new(
        Arc::new(rate_uc),
        Arc::new(color_uc),
        Arc::new(flag_uc),
        Arc::new(delete_uc)
    ));

    let lib_controller = Arc::new(LibraryController::new(photo_repo.clone()));
    let kb_handler = Arc::new(KeyboardHandler::new());
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    // 5. Initial State
    let mut state = AppState::new();
    state.current_view = CurrentView::Library;

    // 6. Seed Data (One Photo)
    let photo1 = Photo::new(FilePath::new("/tmp/test_colors.jpg").unwrap());
    photo_repo.save(&photo1).await.unwrap();

    // Load initial photos into state
    let view_models = lib_controller.get_all_photos().await.unwrap();
    state.photos = view_models;
    state.rebuild_folder_tree();

    (state, kb_handler, photo_controller, lib_controller, tx, rx, photo_repo)
}

/// Helper to drain all pending reloads
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
async fn test_filmstrip_color_shortcut() {
    let (mut state, kb_handler, photo_controller, lib_controller, tx, mut rx, repo) = setup_harness().await;

    // 1. Select the photo in filmstrip (simulated by AppState selection)
    let photo_id = state.photos[0].id.clone();
    state.single_select(&photo_id, 0);
    state.detail_metadata = Some(ui::state::DetailMetadata {
        id: photo_id.clone(),
        name: "test_colors.jpg".to_string(),
        date: "2023-10-27".to_string(),
        camera: "Test Cam".to_string(),
        exposure: "1/100s".to_string(),
        rating: 0,
        color_label: None,
    });
    
    // 2. Press '6' (Red)
    {
        let ctx = egui::Context::default();
        ctx.input_mut(|i| {
            i.events.push(egui::Event::Key { 
                key: egui::Key::Num6, 
                pressed: true, 
                modifiers: egui::Modifiers::NONE, 
                repeat: false, 
                physical_key: None 
            });
        });
        kb_handler.handle_input(&ctx, &mut state, &photo_controller, &lib_controller, &tx);
    }

    // 3. Verify Optimistic UI update
    assert_eq!(state.photos[0].color_label, Some("Red".to_string()), "ViewModel should have updated color label to Red immediately");

    // 4. Wait for Async Persistence
    drain_reloads(&mut rx).await;
    let saved_photo = repo.find_by_id(&domain::value_objects::PhotoId::from_string(&photo_id).unwrap()).await.unwrap().unwrap();
    assert_eq!(saved_photo.color_label().map(|c| c.to_string()), Some("red".to_string()), "Database should have persisted Red color label");

    // 5. Press '7' (Yellow)
    {
        let ctx = egui::Context::default();
        ctx.input_mut(|i| {
            i.events.push(egui::Event::Key { 
                key: egui::Key::Num7, 
                pressed: true, 
                modifiers: egui::Modifiers::NONE, 
                repeat: false, 
                physical_key: None 
            });
        });
        kb_handler.handle_input(&ctx, &mut state, &photo_controller, &lib_controller, &tx);
    }
    
    assert_eq!(state.photos[0].color_label, Some("Yellow".to_string()));
    drain_reloads(&mut rx).await;

    // 6. Press '7' (Yellow) AGAIN -> Should toggle to None
    {
        let ctx = egui::Context::default();
        ctx.input_mut(|i| {
            i.events.push(egui::Event::Key { 
                key: egui::Key::Num7, 
                pressed: true, 
                modifiers: egui::Modifiers::NONE, 
                repeat: false, 
                physical_key: None 
            });
        });
        kb_handler.handle_input(&ctx, &mut state, &photo_controller, &lib_controller, &tx);
    }

    // Verify Optimistic Update (should be None)
    assert_eq!(state.photos[0].color_label, None, "Pressing same color shortcut should toggle label off");
    
    // Wait for Persistence
    drain_reloads(&mut rx).await;
    let saved_photo_toggle = repo.find_by_id(&domain::value_objects::PhotoId::from_string(&photo_id).unwrap()).await.unwrap().unwrap();
    assert_eq!(saved_photo_toggle.color_label(), None, "Database should persist removal of color label");
}
