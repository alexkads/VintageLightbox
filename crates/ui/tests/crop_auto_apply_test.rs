use ui::state::{AppState, CurrentView};
use ui::keyboard::KeyboardHandler;
use adapters::controllers::{EditorController, PhotoController, LibraryController, ExportController, ImportController};
use adapters::view_models::PhotoViewModel;
use domain::value_objects::{CropSettings, FilePath, PhotoId, PhotoMetadata, OrganizationStrategy, RenamePattern};
use domain::services::{ImageExporter, MetadataExtractor, ThumbnailGenerator, PreviewStorage, PreviewType, FileOrganizer};
use domain::import_source::{DeviceRepository, ImportSource};
use domain::DomainResult;
use std::sync::Arc;
use egui::Context;

struct DummyExporter;
#[async_trait::async_trait]
impl ImageExporter for DummyExporter {
    async fn export(&self, _photo: &domain::entities::Photo, _path: &FilePath) -> DomainResult<()> { Ok(()) }
}

struct DummyMetadataExtractor;
impl MetadataExtractor for DummyMetadataExtractor {
    fn extract(&self, _path: &FilePath) -> DomainResult<PhotoMetadata> { Ok(PhotoMetadata::default()) }
}

struct DummyThumbnailGenerator;
#[async_trait::async_trait]
impl ThumbnailGenerator for DummyThumbnailGenerator {
    async fn generate(&self, _path: &FilePath, _max: u32) -> DomainResult<Vec<u8>> { Ok(vec![]) }
    async fn generate_set(&self, _path: &FilePath, _sizes: &[u32]) -> DomainResult<Vec<Vec<u8>>> { Ok(vec![vec![]; _sizes.len()]) }
}

struct DummyPreviewStorage;
impl PreviewStorage for DummyPreviewStorage {
    fn save(&self, _id: &PhotoId, _type: PreviewType, _data: &[u8]) -> DomainResult<()> { Ok(()) }
    fn get(&self, _id: &PhotoId, _type: PreviewType) -> DomainResult<Option<Vec<u8>>> { Ok(None) }
    fn has(&self, _id: &PhotoId, _type: PreviewType) -> DomainResult<bool> { Ok(false) }
    fn delete(&self, _id: &PhotoId) -> DomainResult<()> { Ok(()) }
}

struct DummyDeviceRepository;
#[async_trait::async_trait]
impl DeviceRepository for DummyDeviceRepository {
    async fn get_mounted_devices(&self) -> Vec<ImportSource> { vec![] }
    async fn get_history(&self) -> Vec<ImportSource> { vec![] }
    async fn add_to_history(&self, _path: std::path::PathBuf) {}
}

struct DummyFileOrganizer;
#[async_trait::async_trait]
impl FileOrganizer for DummyFileOrganizer {
    async fn organize_file(
        &self,
        source: &FilePath,
        _metadata: Option<&PhotoMetadata>,
        _strategy: OrganizationStrategy,
        _rename_pattern: RenamePattern,
    ) -> DomainResult<FilePath> {
        Ok(source.clone())
    }
}

async fn setup_harness() -> (
    AppState,
    Arc<KeyboardHandler>,
    Arc<PhotoController>,
    Arc<LibraryController>,
    Arc<EditorController>,
    Arc<ExportController>,
    Arc<ImportController>,
    tokio::sync::mpsc::Sender<Result<Vec<PhotoViewModel>, String>>,
    Context,
    adapters::services::EditorService,
) {
     let db_url = "sqlite::memory:";
     let pool = sqlx::SqlitePool::connect(db_url).await.expect("Failed to create in-memory db");
     sqlx::migrate!("../infrastructure/migrations").run(&pool).await.expect("Failed to run migrations");

     let photo_repo = Arc::new(infrastructure::database::PhotoRepositoryImpl::new(pool.clone()));
     
     let save_edits_uc = use_cases::SavePhotoEditsUseCase::new(photo_repo.clone());
     let editor_controller = Arc::new(EditorController::new(Arc::new(save_edits_uc)));
     
     // Dummies for others
     let rate_uc = use_cases::RatePhotoUseCase::new(photo_repo.clone());
     let color_uc = use_cases::SetColorLabelUseCase::new(photo_repo.clone());
     let flag_uc = use_cases::SetFlagUseCase::new(photo_repo.clone());
     let delete_uc = use_cases::DeletePhotoUseCase::new(photo_repo.clone());
     let photo_controller = Arc::new(PhotoController::new(
        Arc::new(rate_uc), Arc::new(color_uc), Arc::new(flag_uc), Arc::new(delete_uc)
     ));
     
     let lib_controller = Arc::new(LibraryController::new(photo_repo.clone()));
     
     // Export with Dummy
     let export_uc = use_cases::ExportPhotoUseCase::new(
         photo_repo.clone(),
         Arc::new(DummyExporter)
     );
     let export_controller = Arc::new(ExportController::new(
        Arc::new(export_uc),
        Arc::new(infrastructure::SystemGatewayImpl::new())
     ));
     
     // Import with dependencies
     let meta_extractor = Arc::new(DummyMetadataExtractor);
     let thumb_gen = Arc::new(DummyThumbnailGenerator);
     let preview_storage = Arc::new(DummyPreviewStorage);
     let file_organizer = Arc::new(DummyFileOrganizer);
     
     let import_photo_uc = Arc::new(use_cases::ImportPhotoUseCase::new(
         photo_repo.clone(),
         meta_extractor.clone(),
         thumb_gen.clone(),
         preview_storage.clone()
     ));
     
     let preview_uc = Arc::new(use_cases::PreviewBeforeImportUseCase::new(
         meta_extractor.clone(),
         thumb_gen.clone()
     ));
     
     let check_dupes_uc = Arc::new(use_cases::CheckDuplicatesUseCase::new(photo_repo.clone()));
     
     let import_with_opts_uc = Arc::new(use_cases::ImportWithOptionsUseCase::new(
         photo_repo.clone(),
         meta_extractor.clone(),
         thumb_gen.clone(),
         preview_storage.clone(),
         file_organizer.clone()
     ));
     
     let get_sources_uc = Arc::new(use_cases::GetImportSourcesUseCase::new(Arc::new(DummyDeviceRepository)));

     let import_controller = Arc::new(ImportController::new(
         import_photo_uc,
         preview_uc,
         check_dupes_uc,
         import_with_opts_uc,
         get_sources_uc
     ));
     
     let kb_handler = Arc::new(KeyboardHandler::new());
     let mut state = AppState::new();
     state.internal_state.current_view = CurrentView::Develop;
     
     let (tx, _rx) = tokio::sync::mpsc::channel(100);
     
     let editor_service = adapters::services::EditorService::new();
     
     (state, kb_handler, photo_controller, lib_controller, editor_controller, export_controller, import_controller, tx, Context::default(), editor_service)
}

#[tokio::test]
async fn test_keyboard_nav_saves_crop() {
    let (mut state, kb_handler, photo_controller, lib_controller, editor_controller, export_controller, import_controller, tx, ctx, mut editor_service) = setup_harness().await;
    
    // Mock Photo 1
    let mut photo1 = PhotoViewModel::default();
    photo1.id = "p1".to_string();
    
    let mut photo2 = PhotoViewModel::default();
    photo2.id = "p2".to_string();
    
    state.photos = vec![photo1, photo2];
    state.internal_state.develop_selected_id = Some("p1".to_string());
    
    // Enter Crop Mode
    state.crop_mode_active = true;
    // Create normalized crop settings (x, y, w, h, rotation, angle, flip_h, flip_v)
    let crop = CropSettings::new(0.5, 0.5, 0.2, 0.2, 0, 0.0, false, false);
    state.crop_settings = Some(crop.clone());
    state.saved_crop_settings = None;
    
    // Simulate Right Arrow Key
    ctx.input_mut(|i| i.events.push(egui::Event::Key { 
        key: egui::Key::ArrowRight, 
        pressed: true, 
        repeat: false, 
        modifiers: egui::Modifiers::NONE,
        physical_key: None,
    }));
    
    kb_handler.handle_input(
        &ctx, 
        &mut state, 
        &photo_controller, 
        &lib_controller, 
        &editor_controller,
        &export_controller,
        &import_controller,
        &tx,
        &mut editor_service
    );
    
    // Wait for async spawn to complete (checking synchronous flag update)
    
    // Verify State Change:
    // 1. Photo ID should be "p2"
    assert_eq!(state.internal_state.develop_selected_id, Some("p2".to_string()), "Should navigate to p2");
    
    // 2. saved_crop_settings should be set (This is our flag that the logic ran)
    assert!(state.saved_crop_settings.is_some(), "saved_crop_settings should be updated");
    // Verify using getter
    assert_eq!(state.saved_crop_settings.as_ref().unwrap().crop_x(), 0.5);
}
