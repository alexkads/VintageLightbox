// VintageLightbox - Photo Management Application
// Main entry point using eframe (egui)

// Modules are now exported via lib.rs to allow integration testing
// We use the 'ui' library crate for implementation

use ui::app::VintageLightboxApp;

use std::sync::Arc;

use infrastructure::{
    cache::preview_manager::PreviewManager, create_pool, run_migrations, ExifReader,
    FileOrganizerImpl, ImageExporterImpl, PhotoRepositoryImpl, SourceScannerImpl,
    SqlitePresetRepository, ThumbnailGeneratorImpl,
};
use use_cases::presets::{DeletePresetUseCase, ListPresetsUseCase, SavePresetUseCase};
use use_cases::{
    CheckDuplicatesUseCase, DeletePhotoUseCase, DescribeCandidatesUseCase, ExportPhotoUseCase,
    GetImportSourcesUseCase, ImportPhotoUseCase, ImportWithOptionsUseCase, RatePhotoUseCase,
    SavePhotoEditsUseCase, ScanSourceUseCase, SetColorLabelUseCase, SetFlagUseCase,
};

use adapters::controllers::{
    EditorController, ExportController, ImportController, LibraryController, PhotoController,
    PresetController,
};

#[tokio::main]
async fn main() {
    // Initialize formatting for better panic messages
    // human_panic::setup_panic!();

    // 0. Setup Paths
    // Path: ~/Pictures/VintageLightbox/VintageLightbox Catalog, ou o que
    // `VLB_CATALOG` disser. Logic centralized in infrastructure::paths to
    // ensure cross-platform consistency.
    let catalog_path = infrastructure::paths::AppPaths::catalog_root();

    if !catalog_path.exists() {
        std::fs::create_dir_all(&catalog_path).expect("Failed to create catalog directory");
    }

    // `main_db_path()`, e não `catalog_path.join("vintage_lightbox.db")`: o
    // nome do arquivo estava escrito nos dois lugares, iguais por coincidência.
    // Bastaria mudar um para o app abrir um banco e o resto do sistema procurar
    // outro — no mesmo diretório, sem erro nenhum na tela.
    let db_path = infrastructure::paths::AppPaths::main_db_path();
    // SQLite requires path to be string, prepended with sqlite:
    let database_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());

    // ============================================
    // 1. Setup Infrastructure Layer
    // ============================================
    let pool = create_pool(&database_url)
        .await
        .expect("Failed to create database pool");

    // Run migrations
    run_migrations(&pool)
        .await
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

    let device_repo =
        Arc::new(infrastructure::devices::repository::InfrastructureDeviceRepository::new());

    // ============================================
    // 2. Setup Use Cases Layer
    // ============================================
    let import_photo_use_case = Arc::new(ImportPhotoUseCase::new(
        photo_repository.clone(),
        metadata_extractor.clone(),
        thumbnail_generator.clone(),
        preview_manager.clone(),
    ));

    let save_photo_edits_use_case = Arc::new(SavePhotoEditsUseCase::new(photo_repository.clone()));
    let export_photo_use_case = Arc::new(ExportPhotoUseCase::new(
        photo_repository.clone(),
        image_exporter,
    ));
    let rate_photo_use_case = Arc::new(RatePhotoUseCase::new(photo_repository.clone()));
    let set_color_label_use_case = Arc::new(SetColorLabelUseCase::new(photo_repository.clone()));
    let set_flag_use_case = Arc::new(SetFlagUseCase::new(photo_repository.clone()));
    let delete_photo_use_case = Arc::new(DeletePhotoUseCase::new(photo_repository.clone()));

    // Preset use cases
    let list_presets_use_case = Arc::new(ListPresetsUseCase::new(preset_repository.clone()));
    let save_preset_use_case = Arc::new(SavePresetUseCase::new(preset_repository.clone()));
    let delete_preset_use_case = Arc::new(DeletePresetUseCase::new(preset_repository));

    // Advanced import use cases
    let check_duplicates_use_case = Arc::new(CheckDuplicatesUseCase::new(photo_repository.clone()));
    let import_with_options_use_case = Arc::new(ImportWithOptionsUseCase::new(
        photo_repository.clone(),
        metadata_extractor.clone(),
        thumbnail_generator.clone(),
        preview_manager.clone(),
        file_organizer,
    ));

    let get_import_sources_use_case = Arc::new(GetImportSourcesUseCase::new(device_repo.clone()));

    // Varredura e leitura de metadados da origem — o que alimenta a grade de importação
    // antes de qualquer arquivo ser copiado
    let scan_source_use_case = Arc::new(ScanSourceUseCase::new(Arc::new(SourceScannerImpl::new())));
    let describe_candidates_use_case =
        Arc::new(DescribeCandidatesUseCase::new(metadata_extractor.clone()));

    // ============================================
    // 3. Setup Controllers (Adapters Layer)
    // ============================================
    let import_controller = Arc::new(ImportController::new(
        import_photo_use_case,
        check_duplicates_use_case,
        import_with_options_use_case,
        get_import_sources_use_case,
        scan_source_use_case,
        describe_candidates_use_case,
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
    // 4. Run Application
    // ============================================
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_maximized(true)
            .with_decorations(true)
            .with_transparent(false),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "VintageLightbox",
        native_options,
        Box::new(|cc| {
            // Style/Icon configuration is handled inside VintageLightboxApp::new

            Ok(Box::new(VintageLightboxApp::new(
                cc,
                import_controller,
                library_controller,
                editor_controller,
                export_controller,
                photo_controller,
                preset_controller,
                preview_manager,
            )))
        }),
    );
}
