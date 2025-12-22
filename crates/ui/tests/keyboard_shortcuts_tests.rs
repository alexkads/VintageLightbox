
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use async_trait::async_trait;
use std::time::Duration;

use ui::state::{AppState, CurrentView};
use ui::keyboard::KeyboardHandler;
use adapters::controllers::{PhotoController, LibraryController};
use adapters::view_models::PhotoViewModel;
use domain::repositories::{PhotoRepository, CollectionRepository};
use domain::entities::Photo;
use domain::DomainResult;
use domain::value_objects::{PhotoId, CollectionId, FilePath};

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
    Arc<infrastructure::database::SqlitePhotoRepository>
) {
    // 1. In-Memory Database
    let db_url = "sqlite::memory:";
    let pool = sqlx::SqlitePool::connect(db_url).await.expect("Failed to create in-memory db");
    
    // Run migrations
    sqlx::migrate!("../../infrastructure/migrations").run(&pool).await.expect("Failed to run migrations");

    // 2. Repositories
    let photo_repo = Arc::new(infrastructure::database::SqlitePhotoRepository::new(pool.clone()));
    let coll_repo = Arc::new(infrastructure::database::SqliteCollectionRepository::new(pool.clone()));

    // 3. Use Cases (Full Real Stack)
    // We use dummy/empty implementations for filesystem/image processors as we won't trigger import/export
    let get_photos_uc = use_cases::GetPhotosUseCase::new(photo_repo.clone());
    let rate_uc = use_cases::RatePhotoUseCase::new(photo_repo.clone());
    let color_uc = use_cases::SetColorLabelUseCase::new(photo_repo.clone());
    let flag_uc = use_cases::SetFlagUseCase::new(photo_repo.clone());
    let delete_uc = use_cases::DeletePhotoUseCase::new(photo_repo.clone()); // [NEW]

    // 4. Controllers
    let photo_controller = Arc::new(PhotoController::new(
        rate_uc,
        color_uc,
        flag_uc,
        delete_uc
    ));

    let lib_controller = Arc::new(LibraryController::new(get_photos_uc));
    let kb_handler = Arc::new(KeyboardHandler::new());
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    // 5. Initial State
    let mut state = AppState::new();
    state.current_view = CurrentView::Library;

    // 6. Seed Data
    let photo1 = Photo::new(FilePath::new("/tmp/p1.jpg").unwrap());
    let photo_id_1 = photo1.id().to_string();
    photo_repo.save(&photo1).await.unwrap();

    // Load initial photos into state
    let view_models = lib_controller.get_all_photos().await.unwrap();
    state.photos = view_models;
    state.rebuild_folder_tree();

    (state, kb_handler, photo_controller, lib_controller, tx, rx, photo_repo)
}

// =========================================================================================
// TESTS
// =========================================================================================

#[tokio::test]
async fn test_flag_toggle_shortcut() {
    let (mut state, kb_handler, photo_controller, lib_controller, tx, mut rx, repo) = setup_harness().await;
    
    // Select the first photo
    let photo_id = state.photos[0].id.clone();
    state.single_select(&photo_id, 0);

    // Initial state check
    assert_eq!(state.photos[0].flag, Some(0), "Initial flag should be 0");

    // -------------------------------------------------------------
    // 1. Press 'P' (Pick) -> Should set to 1
    // -------------------------------------------------------------
    let ctx = egui::Context::default();
    ctx.input_mut(|i| {
        i.events.push(egui::Event::Key { 
            key: egui::Key::P, 
            pressed: true, 
            modifiers: egui::Modifiers::NONE,
            repeat: false,
            physical_key: None,
        });
    });

    kb_handler.handle_input(&ctx, &mut state, &photo_controller, &lib_controller, &tx);
    
    // Verify Optimistic Update
    assert_eq!(state.photos[0].flag, Some(1), "Optimistic update should set flag to 1");

    // Wait for Async Reload
    let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(result.is_ok(), "Should receive reload signal");
    let new_photos = result.unwrap().unwrap().unwrap();
    state.photos = new_photos; // Simulate app updating state
    
    // Verify Persistence
    let p = repo.find_by_id(&domain::value_objects::PhotoId::from_string(photo_id.clone()).unwrap()).await.unwrap().unwrap();
    assert_eq!(p.flag(), Some(domain::value_objects::Flag::Pick), "Repo should be updated to Pick");

    // -------------------------------------------------------------
    // 2. Press 'P' (Pick) AGAIN -> Should toggle to 0
    // -------------------------------------------------------------
    ctx.input_mut(|i| {
         i.events.push(egui::Event::Key { 
            key: egui::Key::P, 
            pressed: true, 
            modifiers: egui::Modifiers::NONE,
            repeat: false,
            physical_key: None,
        });
    });

    kb_handler.handle_input(&ctx, &mut state, &photo_controller, &lib_controller, &tx);
    
    // Verify Optimistic Update
    assert_eq!(state.photos[0].flag, Some(0), "Optimistic update should toggle flag to 0");
    
    // Wait for Async Reload
    let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(result.is_ok(), "Should receive reload signal");
    let new_photos = result.unwrap().unwrap().unwrap();
    state.photos = new_photos; 

    // Verify Persistence
    let p = repo.find_by_id(&domain::value_objects::PhotoId::from_string(photo_id.clone()).unwrap()).await.unwrap().unwrap();
    assert_eq!(p.flag(), None, "Repo should be updated to None (Unflagged)");

    // -------------------------------------------------------------
    // 3. Press 'X' (Reject) -> Should set to -1
    // -------------------------------------------------------------
     ctx.input_mut(|i| {
         i.events.push(egui::Event::Key { 
            key: egui::Key::X, 
            pressed: true, 
            modifiers: egui::Modifiers::NONE,
            repeat: false,
            physical_key: None,
        });
    });

    kb_handler.handle_input(&ctx, &mut state, &photo_controller, &lib_controller, &tx);
    assert_eq!(state.photos[0].flag, Some(-1), "Should set flag to -1");
    // Wait for reload
    let _ = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;

    // -------------------------------------------------------------
    // 4. Press 'X' (Reject) AGAIN -> Should toggle to 0
    // -------------------------------------------------------------
     ctx.input_mut(|i| {
         i.events.push(egui::Event::Key { 
            key: egui::Key::X, 
            pressed: true, 
            modifiers: egui::Modifiers::NONE,
            repeat: false,
            physical_key: None,
        });
    });

    kb_handler.handle_input(&ctx, &mut state, &photo_controller, &lib_controller, &tx);
    assert_eq!(state.photos[0].flag, Some(0), "Should set flag to 0");
}
