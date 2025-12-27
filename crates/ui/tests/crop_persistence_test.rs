use std::sync::Arc;
use ui::state::{AppState, CurrentView};
use adapters::controllers::{LibraryController, EditorController};
use use_cases::SavePhotoEditsUseCase; // Reduced unused
use domain::repositories::PhotoRepository;
use domain::entities::Photo;
use domain::value_objects::{FilePath, CropSettings}; // Correct import
// Removed unused imports

// Re-using setup harness pattern from other tests
async fn setup_harness() -> (
    AppState,
    Arc<infrastructure::database::PhotoRepositoryImpl>,
    Arc<LibraryController>,
    Arc<EditorController>,
) {
    let db_url = "sqlite::memory:";
    let pool = sqlx::SqlitePool::connect(db_url).await.expect("Failed to create in-memory db");
    sqlx::migrate!("../infrastructure/migrations").run(&pool).await.expect("Failed to run migrations");

    let photo_repo = Arc::new(infrastructure::database::PhotoRepositoryImpl::new(pool.clone()));
    let save_uc = SavePhotoEditsUseCase::new(photo_repo.clone());
    
    let lib_controller = Arc::new(LibraryController::new(photo_repo.clone()));
    let editor_controller = Arc::new(EditorController::new(Arc::new(save_uc)));
    
    let mut state = AppState::new();
    state.current_view = CurrentView::Develop;

    (state, photo_repo, lib_controller, editor_controller)
}

#[tokio::test]
async fn test_crop_persistence_flow() {
    let (mut state, photo_repo, lib_controller, editor_controller) = setup_harness().await;

    // 1. Create and Save a Photo
    let photo_path = FilePath::new("/tmp/test_crop_persist.jpg").unwrap();
    let photo = Photo::new(photo_path.clone());
    let photo_id = photo.id().clone();
    photo_repo.save(&photo).await.unwrap();

    // 2. Load into State
    let photos = lib_controller.get_all_photos().await.unwrap();
    state.photos = photos;
    state.develop_selected_photo_id = Some(photo_id.to_string());
    
    // Simulate selection to populate view model
    let _ = state.get_current_photo().unwrap();
    
    // 3. Apply Crop Settings via ViewModel/Controller
    // In the app, this happens when "Apply" is clicked or auto-save triggers.
    // We simulate the "EditSnapshot" creation and save.
    
    let crop_settings = CropSettings::new(
        0.1, 0.1, 0.5, 0.5, 
        1, 15.0, // Rotated 90 + 15 deg
        true, false
    );
    
    // Manually trigger save (like EditorController does)
    // We need to pass all edit parameters. 
    // For simplicity, we use the controller's save_edits which takes many args.
    // Or simpler: use the repository directly to verify persistence of what the controller WOULD send.
    // But testing the Controller -> UseCase -> Repo chain is better.
    
    let vm = state.get_current_photo().unwrap();
    
    editor_controller.save_edits(
         vm.id.clone(),
         vm.edit_exposure.unwrap_or(0.0), vm.edit_contrast.unwrap_or(0.0), vm.edit_temperature.unwrap_or(0.0), vm.edit_tint.unwrap_or(0.0),
         vm.edit_highlights.unwrap_or(0.0), vm.edit_shadows.unwrap_or(0.0), vm.edit_whites.unwrap_or(0.0), vm.edit_blacks.unwrap_or(0.0),
         vm.edit_clarity.unwrap_or(0.0), vm.edit_vibrance.unwrap_or(0.0), vm.edit_saturation.unwrap_or(0.0),
         vm.edit_tone_curve_shadows.unwrap_or(0.0), vm.edit_tone_curve_darks.unwrap_or(0.0), vm.edit_tone_curve_lights.unwrap_or(0.0), vm.edit_tone_curve_highlights.unwrap_or(0.0),
         vm.edit_hsl_red_sat.unwrap_or(0.0), vm.edit_hsl_orange_sat.unwrap_or(0.0), vm.edit_hsl_yellow_sat.unwrap_or(0.0), vm.edit_hsl_green_sat.unwrap_or(0.0), vm.edit_hsl_aqua_sat.unwrap_or(0.0), vm.edit_hsl_blue_sat.unwrap_or(0.0), vm.edit_hsl_purple_sat.unwrap_or(0.0), vm.edit_hsl_magenta_sat.unwrap_or(0.0),
         vm.edit_hsl_red_hue.unwrap_or(0.0), vm.edit_hsl_orange_hue.unwrap_or(0.0), vm.edit_hsl_yellow_hue.unwrap_or(0.0), vm.edit_hsl_green_hue.unwrap_or(0.0), vm.edit_hsl_aqua_hue.unwrap_or(0.0), vm.edit_hsl_blue_hue.unwrap_or(0.0), vm.edit_hsl_purple_hue.unwrap_or(0.0), vm.edit_hsl_magenta_hue.unwrap_or(0.0),
         vm.edit_hsl_red_lum.unwrap_or(0.0), vm.edit_hsl_orange_lum.unwrap_or(0.0), vm.edit_hsl_yellow_lum.unwrap_or(0.0), vm.edit_hsl_green_lum.unwrap_or(0.0), vm.edit_hsl_aqua_lum.unwrap_or(0.0), vm.edit_hsl_blue_lum.unwrap_or(0.0), vm.edit_hsl_purple_lum.unwrap_or(0.0), vm.edit_hsl_magenta_lum.unwrap_or(0.0),
         vm.edit_lens_distortion.unwrap_or(0.0), vm.edit_lens_vignette_amount.unwrap_or(0.0), vm.edit_lens_vignette_midpoint.unwrap_or(0.0),
         vm.edit_nr_luminance.unwrap_or(0.0), vm.edit_nr_color.unwrap_or(0.0),
         vm.edit_sharpen_amount.unwrap_or(0.0), vm.edit_sharpen_radius.unwrap_or(0.0),
         Some(crop_settings.crop_x()),
         Some(crop_settings.crop_y()),
         Some(crop_settings.crop_width()),
         Some(crop_settings.crop_height()),
         Some(crop_settings.rotation_90()),
         Some(crop_settings.angle()),
         Some(crop_settings.flip_horizontal()),
         Some(crop_settings.flip_vertical()),
     ).await.expect("Failed to save edits");

    // 4. Reload from DB (simulate app restart)
    // Explicit trait call to avoid ambiguity/Arc issues
    let saved_photo = photo_repo.find_by_id(&photo_id).await.unwrap().expect("Photo not found");
    
    // 5. Verify Settings in Domain Entity
    // Photo entity doesn't have crop_settings() helper, checking fields directly
    assert_eq!(saved_photo.edit_crop_x().expect("crop_x"), 0.1);
    assert_eq!(saved_photo.edit_crop_width().expect("crop_width"), 0.5);
    assert_eq!(saved_photo.edit_crop_flip_v().expect("flip_v"), false); // Input was false for V?
    // Wait, input was: true, false (flip_h, flip_v). So flip_h=true, flip_v=false.
    assert!(saved_photo.edit_crop_flip_h().expect("flip_h"));
    assert!(!saved_photo.edit_crop_flip_v().expect("flip_v"));
    
    // Rotation was 1 (90 deg)
    assert_eq!(saved_photo.edit_crop_rotation().expect("rotation"), 1);

    // 6. Verify LibraryController loads it back to ViewModel
    let newly_loaded_photos = lib_controller.get_all_photos().await.unwrap();
    let loaded_vm = newly_loaded_photos.iter().find(|p| p.id == photo_id.to_string()).unwrap();
    
    // Check individual VM fields instead of looking for crop_settings struct
    assert_eq!(loaded_vm.edit_crop_x.unwrap(), 0.1);
    // Calculate total rotation to verify logic is implicit ??
    // Actually VM doesn't have total_rotation method usually if it's just data struct.
    // We can check edit_crop_rotation and edit_crop_angle
    assert_eq!(loaded_vm.edit_crop_rotation.unwrap(), 1);
    assert_eq!(loaded_vm.edit_crop_angle.unwrap(), 15.0);
}

#[tokio::test]
async fn test_crop_persistence_on_photo_switch() {
    let (mut state, photo_repo, lib_controller, editor_controller) = setup_harness().await;

    // 1. Create two photos
    let photo1 = Photo::new(FilePath::new("/tmp/p1.jpg").unwrap());
    let photo2 = Photo::new(FilePath::new("/tmp/p2.jpg").unwrap());
    let id1 = photo1.id().clone();
    let id2 = photo2.id().clone();
    
    photo_repo.save(&photo1).await.unwrap();
    photo_repo.save(&photo2).await.unwrap();
    
    state.photos = lib_controller.get_all_photos().await.unwrap();
    state.develop_selected_photo_id = Some(id1.to_string());
    
    // 2. Simulate User Editing Photo 1
    // In UI, this updates `state.active_*` values and `state.crop_settings`
    state.active_exposure = 1.5;
    let crop_settings = CropSettings::new(
        0.2, 0.2, 0.6, 0.6,
        0, 10.0,
        false, true
    );
    state.crop_settings = Some(crop_settings.clone());
    
    // Mark as pending save
    state.pending_auto_save = true;
    
    // 3. User switches to Photo 2 in Filmstrip
    // This logic mimics what we added to DevelopView filmstrip callback
    // We manually execute the save logic here to verify it works as expected "in isolation"
    // (We aren't spawning a full UI loop here, but verifying the logic block)
    
    if state.pending_auto_save {
        if let Some(vm) = state.get_current_photo() {
             let controller = editor_controller.clone();
             let id = vm.id.clone();
             let exposure = state.active_exposure;
             // ... other params ...
             let active_crop = state.crop_settings.clone();
             
             // Await directly here instead of spawn
             controller.save_edits(
                 id,
                 exposure, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
                 , 0.0, // Added one more float
                 active_crop.as_ref().map(|c| c.crop_x()),
                 active_crop.as_ref().map(|c| c.crop_y()),
                 active_crop.as_ref().map(|c| c.crop_width()),
                 active_crop.as_ref().map(|c| c.crop_height()),
                 active_crop.as_ref().map(|c| c.rotation_90()),
                 active_crop.as_ref().map(|c| c.angle()),
                 active_crop.as_ref().map(|c| c.flip_horizontal()),
                 active_crop.as_ref().map(|c| c.flip_vertical()),
             ).await.expect("Save failed");
        }
    }
    
    // Switch ID
    state.develop_selected_photo_id = Some(id2.to_string());
    state.active_exposure = 0.0; // Reset active state (simulating load)
    state.crop_settings = None;
    
    // 4. Verify Photo 1 persisted
    let saved_p1 = photo_repo.find_by_id(&id1).await.unwrap().unwrap();
    assert_eq!(saved_p1.edit_exposure().unwrap(), 1.5);
    
    // Check crop fields
    assert_eq!(saved_p1.edit_crop_x().unwrap(), 0.2);
    // Input p1: new(0.2, 0.2, 0.6, 0.6, 0, 10.0, false, true); 
    // flip_h=false, flip_v=true.
    assert!(saved_p1.edit_crop_flip_v().unwrap());
    assert!(!saved_p1.edit_crop_flip_h().unwrap());
}
