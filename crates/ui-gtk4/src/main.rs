//! VintageLightbox GTK4 - Main Entry Point
//!
//! Uses relm4 for the Elm Architecture-based UI.

use std::sync::Arc;

use relm4::prelude::*;

use infrastructure::{
    create_pool, run_migrations,
    PhotoRepositoryImpl, ExifReader,
    ThumbnailGeneratorImpl, ImageExporterImpl,
    FileOrganizerImpl,
    SqlitePresetRepository,
    cache::preview_manager::PreviewManager,
    paths::AppPaths,
};
use use_cases::{
    ImportPhotoUseCase, SavePhotoEditsUseCase, ExportPhotoUseCase,
    RatePhotoUseCase, SetColorLabelUseCase, SetFlagUseCase, DeletePhotoUseCase,
    PreviewBeforeImportUseCase, CheckDuplicatesUseCase, ImportWithOptionsUseCase,
    presets::{ListPresetsUseCase, SavePresetUseCase, DeletePresetUseCase},
};
use adapters::controllers::{
    ImportController, LibraryController, EditorController,
    ExportController, PhotoController, PresetController,
};

use ui_gtk4::app::VintageLightboxApp;
use ui_gtk4::model::AppInit;

const APP_ID: &str = "com.vintagelightbox.gtk";

fn main() {
    // Initialize environment
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();
    
    // Create tokio runtime for async operations
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    );
    
    // Initialize infrastructure in the runtime
    let (controllers, preview_manager) = runtime.block_on(setup_infrastructure());
    
    // Initialize GTK4
    gtk4::init().expect("Failed to initialize GTK4");
    
    // Create and run the relm4 application
    let app = RelmApp::new(APP_ID);
    app.run::<VintageLightboxApp>(AppInit {
        import_controller: controllers.0,
        library_controller: controllers.1,
        editor_controller: controllers.2,
        export_controller: controllers.3,
        photo_controller: controllers.4,
        preset_controller: controllers.5,
        preview_manager,
        runtime,
    });
}

/// Setup all infrastructure, use cases, and controllers
async fn setup_infrastructure() -> (
    (
        Arc<ImportController>,
        Arc<LibraryController>,
        Arc<EditorController>,
        Arc<ExportController>,
        Arc<PhotoController>,
        Arc<PresetController>,
    ),
    Arc<PreviewManager>,
) {
    // Create catalog directory structure
    let catalog_path = AppPaths::catalog_root();
    
    if !catalog_path.exists() {
        std::fs::create_dir_all(&catalog_path).expect("Failed to create catalog directory");
    }
    
    // Database setup
    let db_path = catalog_path.join("vintage_lightbox.db");
    let database_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
    
    let pool = create_pool(&database_url).await
        .expect("Failed to create database pool");
    
    run_migrations(&pool).await
        .expect("Failed to run database migrations");
    
    // Infrastructure layer
    let photo_repository = Arc::new(PhotoRepositoryImpl::new(pool.clone()));
    let preset_repository = Arc::new(SqlitePresetRepository::new(pool));
    let metadata_extractor = Arc::new(ExifReader);
    let thumbnail_generator = Arc::new(ThumbnailGeneratorImpl::new());
    let image_exporter = Arc::new(ImageExporterImpl::new());
    let file_organizer = Arc::new(FileOrganizerImpl::new(catalog_path.clone()));
    
    // Preview cache
    let preview_path = catalog_path.join("Previews.lrdata");
    let preview_manager = Arc::new(PreviewManager::new_with_path(preview_path));
    
    // Use cases
    let import_photo_use_case = Arc::new(ImportPhotoUseCase::new(
        photo_repository.clone(),
        metadata_extractor.clone(),
        thumbnail_generator.clone(),
        preview_manager.clone(),
    ));
    
    let preview_before_import_use_case = Arc::new(PreviewBeforeImportUseCase::new(
        metadata_extractor.clone(),
        thumbnail_generator.clone(),
    ));
    let check_duplicates_use_case = Arc::new(CheckDuplicatesUseCase::new(
        photo_repository.clone()
    ));
    let import_with_options_use_case = Arc::new(ImportWithOptionsUseCase::new(
        photo_repository.clone(),
        metadata_extractor.clone(),
        thumbnail_generator.clone(),
        preview_manager.clone(),
        file_organizer,
    ));
    
    let save_photo_edits_use_case = Arc::new(SavePhotoEditsUseCase::new(
        photo_repository.clone()
    ));
    let export_photo_use_case = Arc::new(ExportPhotoUseCase::new(
        photo_repository.clone(),
        image_exporter,
    ));
    let rate_photo_use_case = Arc::new(RatePhotoUseCase::new(
        photo_repository.clone()
    ));
    let set_color_label_use_case = Arc::new(SetColorLabelUseCase::new(
        photo_repository.clone()
    ));
    let set_flag_use_case = Arc::new(SetFlagUseCase::new(
        photo_repository.clone()
    ));
    let delete_photo_use_case = Arc::new(DeletePhotoUseCase::new(
        photo_repository.clone()
    ));
    
    // Preset use cases
    let list_presets_use_case = Arc::new(ListPresetsUseCase::new(
        preset_repository.clone()
    ));
    let save_preset_use_case = Arc::new(SavePresetUseCase::new(
        preset_repository.clone()
    ));
    let delete_preset_use_case = Arc::new(DeletePresetUseCase::new(
        preset_repository
    ));
    
    // Controllers
    let import_controller = Arc::new(ImportController::new(
        import_photo_use_case,
        preview_before_import_use_case,
        check_duplicates_use_case,
        import_with_options_use_case,
    ));
    let library_controller = Arc::new(LibraryController::new(photo_repository));
    let editor_controller = Arc::new(EditorController::new(save_photo_edits_use_case));
    let export_controller = Arc::new(ExportController::new(export_photo_use_case));
    let photo_controller = Arc::new(PhotoController::new(
        rate_photo_use_case,
        set_color_label_use_case,
        set_flag_use_case,
        delete_photo_use_case,
    ));
    let preset_controller = Arc::new(PresetController::new(
        list_presets_use_case,
        save_preset_use_case,
        delete_preset_use_case,
    ));
    
    (
        (
            import_controller,
            library_controller,
            editor_controller,
            export_controller,
            photo_controller,
            preset_controller,
        ),
        preview_manager,
    )
}
