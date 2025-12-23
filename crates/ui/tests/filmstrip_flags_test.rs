use ui::state::AppState;
use adapters::controllers::*;
use domain::entities::*;
use domain::value_objects::*;
use domain::repositories::PhotoRepository; // Required for save/find_by_id methods
use std::sync::Arc;
use domain::value_objects::Flag;

// Helper to create a dummy test harness with IN-MEMORY SQLite
async fn setup_harness() -> (
    AppState,
    Arc<infrastructure::database::PhotoRepositoryImpl>,
    Arc<PhotoController>,
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

    // 5. Initial State
    let mut state = AppState::new();
    
    // 6. Seed Data (One Photo)
    let photo1 = Photo::new(FilePath::new("/tmp/unflagged_photo.jpg").unwrap());
    photo_repo.save(&photo1).await.unwrap();

    // Load initial photos into state
    let view_models = lib_controller.get_all_photos().await.unwrap();
    state.photos = view_models;

    (state, photo_repo, photo_controller)
}

#[tokio::test]
async fn test_filmstrip_flag_toggling() {
    let (mut state, photo_repo, photo_controller) = setup_harness().await;

    let photo_id = state.photos[0].id.clone();
    
    // Test: Set Flag to Pick (1)
    photo_controller.set_flag(&photo_id, 1).await.unwrap();
    
    // Verify
    let updated = photo_repo.find_by_id(&PhotoId::from_string(&photo_id).unwrap()).await.unwrap().unwrap();
    assert_eq!(updated.flag(), Some(Flag::Pick));
    
    // Test: Toggle Off (set to 0/None implicitly via toggle logic, but controller set_flag(1) sets it to 1)
    // The Filmstrip UI logic implements the toggle: `if current == 1 { 0 } else { 1 }`.
    // We verify here that the backend accepts 0 to clear.
    photo_controller.set_flag(&photo_id, 0).await.unwrap();
    let cleared = photo_repo.find_by_id(&PhotoId::from_string(&photo_id).unwrap()).await.unwrap().unwrap();
    assert_eq!(cleared.flag(), None);
}
