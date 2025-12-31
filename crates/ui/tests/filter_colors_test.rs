use std::sync::Arc;

use ui::state::{AppState, CurrentView};
use ui::keyboard::KeyboardHandler;
use adapters::controllers::{PhotoController, LibraryController};
use adapters::view_models::PhotoViewModel;
use domain::repositories::PhotoRepository;
use domain::entities::Photo;
use domain::value_objects::FilePath;

async fn setup_harness() -> (
    AppState,
    Arc<KeyboardHandler>,
    Arc<PhotoController>,
    Arc<LibraryController>,
    tokio::sync::mpsc::Sender<Result<Vec<PhotoViewModel>, String>>,
    tokio::sync::mpsc::Receiver<Result<Vec<PhotoViewModel>, String>>,
    Arc<infrastructure::database::PhotoRepositoryImpl>
) {
    let db_url = "sqlite::memory:";
    let pool = sqlx::SqlitePool::connect(db_url).await.expect("Failed to create in-memory db");
    sqlx::migrate!("../infrastructure/migrations").run(&pool).await.expect("Failed to run migrations");

    let photo_repo = Arc::new(infrastructure::database::PhotoRepositoryImpl::new(pool.clone()));
    let rate_uc = use_cases::RatePhotoUseCase::new(photo_repo.clone());
    let color_uc = use_cases::SetColorLabelUseCase::new(photo_repo.clone());
    let flag_uc = use_cases::SetFlagUseCase::new(photo_repo.clone());
    let delete_uc = use_cases::DeletePhotoUseCase::new(photo_repo.clone());

    let photo_controller = Arc::new(PhotoController::new(
        Arc::new(rate_uc),
        Arc::new(color_uc),
        Arc::new(flag_uc),
        Arc::new(delete_uc)
    ));

    let lib_controller = Arc::new(LibraryController::new(photo_repo.clone()));
    let kb_handler = Arc::new(KeyboardHandler::new());
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    let mut state = AppState::new(false);
    state.internal_state.current_view = CurrentView::Library;

    // Seed Data: 1 Red Photo, 1 Blue Photo
    let photo1 = Photo::new(FilePath::new("/tmp/red_photo.jpg").unwrap());
    let mut photo1_mod = photo1.clone();
    photo1_mod.set_color_label(domain::value_objects::ColorLabel::Red);
    photo_repo.save(&photo1_mod).await.unwrap();

    let photo2 = Photo::new(FilePath::new("/tmp/blue_photo.jpg").unwrap());
    let mut photo2_mod = photo2.clone();
    photo2_mod.set_color_label(domain::value_objects::ColorLabel::Blue);
    photo_repo.save(&photo2_mod).await.unwrap();

    let view_models = lib_controller.get_all_photos().await.unwrap();
    state.photos = view_models;

    (state, kb_handler, photo_controller, lib_controller, tx, rx, photo_repo)
}

#[tokio::test]
async fn test_filtering_by_color_label() {
    let (mut state, _, _, _, _, _, _) = setup_harness().await;

    // 1. Filter by "Red" (Capitalized, as set by UI)
    // 1. Filter by "Red" (Capitalized, as set by UI)
    state.internal_state.photo_filters.color_labels.insert(domain::value_objects::ColorLabel::Red);
    
    // NOTE: Both PhotoGrid and Filmstrip rely on `state.get_filtered_photos()` or equivalent logic.
    // By verifying this method returns the correct subset, we ensure both components receive the correct data.
    
    // 2. Apply logic that PhotoGrid uses (simulated here, we need to find where it is)
    // Assuming logic is: photo.color_label == state.filter_color_label
    
    // 2. Apply filtering logic
    let filtered_refs = state.internal_state.photo_filters.apply(&state.photos);
    let filtered_photos: Vec<PhotoViewModel> = filtered_refs.into_iter().cloned().collect();

    // 3. Asset we found the red photo
    assert_eq!(filtered_photos.len(), 1, "Should find exactly one Red photo");
    assert_eq!(filtered_photos[0].name, "red_photo.jpg");
}
