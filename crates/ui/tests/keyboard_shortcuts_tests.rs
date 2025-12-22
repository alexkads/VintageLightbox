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
    let _coll_repo = Arc::new(infrastructure::database::CollectionRepositoryImpl::new(pool.clone()));

    // 3. Use Cases (Full Real Stack)
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

    // 6. Seed Data
    let photo1 = Photo::new(FilePath::new("/tmp/p1.jpg").unwrap());
    let _photo_id_1 = photo1.id().to_string();
    photo_repo.save(&photo1).await.unwrap();

    // Load initial photos into state
    let view_models = lib_controller.get_all_photos().await.unwrap();
    state.photos = view_models;
    state.rebuild_folder_tree();

    (state, kb_handler, photo_controller, lib_controller, tx, rx, photo_repo)
}

/// Helper to drain all pending reloads and return the last one
/// This prevents race conditions from out-of-order async updates  
async fn drain_reloads(
    rx: &mut tokio::sync::mpsc::Receiver<Result<Vec<PhotoViewModel>, String>>
) -> Option<Vec<PhotoViewModel>> {
    let mut last_photos = None;
    loop {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Some(Ok(photos))) => {
                last_photos = Some(photos);
            }
            Ok(Some(Err(_))) | Ok(None) | Err(_) => {
                break;
            }
        }
    }
    last_photos
}

/// Helper to wait for database to reflect expected flag value
async fn wait_for_db_flag(
    repo: &Arc<infrastructure::database::PhotoRepositoryImpl>,
    photo_id: &str,
    expected_flag: Option<domain::value_objects::Flag>,
    timeout_ms: u64,
) -> bool {
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    
    while start.elapsed() < timeout {
        let p = repo.find_by_id(&domain::value_objects::PhotoId::from_string(photo_id).unwrap())
            .await.unwrap().unwrap();
        if p.flag() == expected_flag {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

// =========================================================================================
// TESTS
// =========================================================================================

/// Test that P (Pick) and X (Reject) keyboard shortcuts work correctly with toggle behavior
/// This test is designed to be robust against race conditions by:
/// 1. Draining all pending reloads and using only the last one
/// 2. Polling the database to verify persistence before making assertions
/// 3. Reloading state from database before each step to ensure consistency
#[tokio::test]
async fn test_flag_toggle_shortcut() {
    let (mut state, kb_handler, photo_controller, lib_controller, tx, mut rx, repo) = setup_harness().await;
    
    // Select the first photo
    let photo_id = state.photos[0].id.clone();
    state.single_select(&photo_id, 0);

    // Initial state check
    assert_eq!(state.photos[0].flag, None, "Initial flag should be None (Unflagged)");

    // -------------------------------------------------------------
    // 1. Press 'P' (Pick) -> Should set to 1
    // -------------------------------------------------------------
    {
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
    }
    
    // Verify Optimistic Update
    assert_eq!(state.photos[0].flag, Some(1), "Optimistic update should set flag to 1");
    
    // Wait for persistence and drain reloads
    assert!(wait_for_db_flag(&repo, &photo_id, Some(domain::value_objects::Flag::Pick), 500).await,
        "DB should be updated to Pick");
    let _ = drain_reloads(&mut rx).await;
    
    // Reload state from DB to ensure consistency
    state.photos = lib_controller.get_all_photos().await.unwrap();
    assert_eq!(state.photos[0].flag, Some(1), "State should reflect Pick after reload");

    // -------------------------------------------------------------
    // 2. Press 'P' (Pick) AGAIN -> Should toggle to 0
    // -------------------------------------------------------------
    {
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
    }
    
    // Verify Optimistic Update
    assert_eq!(state.photos[0].flag, Some(0), "Optimistic update should toggle flag to 0");
    
    // Wait for persistence and drain reloads
    assert!(wait_for_db_flag(&repo, &photo_id, None, 500).await,
        "DB should be updated to None (Unflagged)");
    let _ = drain_reloads(&mut rx).await;
    // Reload state from DB
    state.photos = lib_controller.get_all_photos().await.unwrap();

    // -------------------------------------------------------------
    // 3. Press 'X' (Reject) -> Should set to -1
    // -------------------------------------------------------------
    {
        let ctx = egui::Context::default();
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
    }
    assert_eq!(state.photos[0].flag, Some(-1), "Optimistic update should set flag to -1");
    
    // Wait for persistence and drain reloads
    assert!(wait_for_db_flag(&repo, &photo_id, Some(domain::value_objects::Flag::Reject), 500).await,
        "DB should be updated to Reject");
    let _ = drain_reloads(&mut rx).await;
    
    // Reload state from DB
    state.photos = lib_controller.get_all_photos().await.unwrap();
    assert_eq!(state.photos[0].flag, Some(-1), "State should reflect Reject after reload");

    // -------------------------------------------------------------
    // 4. Press 'X' (Reject) AGAIN -> Should toggle to 0
    // -------------------------------------------------------------
    state.single_select(&photo_id, 0); // Ensure selection is active
    
    {
        let ctx = egui::Context::default();
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
    }
    assert_eq!(state.photos[0].flag, Some(0), "Optimistic update should toggle flag to 0");
    
    // Wait for persistence and drain reloads
    assert!(wait_for_db_flag(&repo, &photo_id, None, 500).await,
        "DB should be updated to None (Unflagged)");
    let _ = drain_reloads(&mut rx).await;
    
    // Final verification
    state.photos = lib_controller.get_all_photos().await.unwrap();
    assert_eq!(state.photos[0].flag, None, "Final state should be None (Unflagged)");
}
