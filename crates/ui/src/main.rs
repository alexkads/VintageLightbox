// VintageLightbox - Photo Management Application
// Main entry point using eframe (egui)

// Modules are now exported via lib.rs to allow integration testing
// We use the 'ui' library crate for implementation

use ui::app::VintageLightboxApp;

use std::sync::Arc;

use infrastructure::{
    create_pool, run_migrations,
    PhotoRepositoryImpl, ExifReader,
    ThumbnailGeneratorImpl, ImageExporterImpl,
    FileOrganizerImpl,
    SqlitePresetRepository,
    cache::preview_manager::PreviewManager,
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



#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    // ============================================
    // 0. Initialize Environment & Catalog Structure
    // ============================================
    dotenv::dotenv().ok();

    // Lightroom-style Catalog Structure
    // Path: ~/Pictures/VintageLightbox/VintageLightbox Catalog
    // Logic centralized in infrastructure::paths to ensure cross-platform consistency
    let catalog_path = infrastructure::paths::AppPaths::catalog_root();

    if !catalog_path.exists() {
        std::fs::create_dir_all(&catalog_path).expect("Failed to create catalog directory");
    }

    // Database Path: ./VintageLightbox Catalog/vintage_lightbox.db
    let db_path = catalog_path.join("vintage_lightbox.db");
    // SQLite requires path to be string, prepended with sqlite:
    let database_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());

    // ============================================
    // 1. Setup Infrastructure Layer
    // ============================================
    let pool = create_pool(&database_url).await
        .expect("Failed to create database pool");

    // Run migrations
    run_migrations(&pool).await
        .expect("Failed to run database migrations");

    let photo_repository = Arc::new(PhotoRepositoryImpl::new(pool.clone()));
    let preset_repository = Arc::new(SqlitePresetRepository::new(pool));
    let metadata_extractor = Arc::new(ExifReader);
    let thumbnail_generator = Arc::new(ThumbnailGeneratorImpl::new());
    let image_exporter = Arc::new(ImageExporterImpl::new());
    let file_organizer = Arc::new(FileOrganizerImpl::new(catalog_path.clone()));

    // Preview Cache Path: ./VintageLightbox Catalog/Previews.lrdata
    let preview_path = catalog_path.join("Previews.lrdata");
    let preview_manager = Arc::new(PreviewManager::new_with_path(preview_path));

    // ============================================
    // 2. Setup Use Cases Layer
    // ============================================
    let import_photo_use_case = Arc::new(ImportPhotoUseCase::new(
        photo_repository.clone(),
        metadata_extractor.clone(),
        thumbnail_generator.clone(),
        preview_manager.clone(),
    ));

    // Advanced import use cases
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

    // ============================================
    // 3. Setup Controllers (Adapters Layer)
    // ============================================
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

    // ============================================
    // 4. Launch eframe Application
    // ============================================
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("VintageLightbox"),
        ..Default::default()
    };

    eframe::run_native(
        "VintageLightbox",
        options,
        Box::new(move |cc| {
            let mut app = VintageLightboxApp::new(
                cc,
                import_controller,
                library_controller.clone(),
                editor_controller,
                export_controller,
                photo_controller,
                preset_controller,
                preview_manager.clone(),
            );

            // Load photos on startup
            app.load_photos(&cc.egui_ctx);
            
            // Load presets on startup
            app.load_presets(&cc.egui_ctx);

            Ok(Box::new(app))
        }),
    )
}
