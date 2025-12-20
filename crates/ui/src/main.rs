// VintageLightbox - Photo Management Application
// Main entry point using eframe (egui)

mod app;
mod state;
mod design_system;
mod image_processing;
mod keyboard;
mod components;
mod views;

use std::sync::Arc;

use infrastructure::{
    create_pool, run_migrations,
    PhotoRepositoryImpl, ExifReader,
    ThumbnailGeneratorImpl, ImageExporterImpl,
};
use use_cases::{
    ImportPhotoUseCase, SavePhotoEditsUseCase, ExportPhotoUseCase,
    RatePhotoUseCase, SetColorLabelUseCase,
};
use adapters::controllers::{
    ImportController, LibraryController, EditorController,
    ExportController, PhotoController,
};

use app::VintageLightboxApp;

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    // ============================================
    // 1. Setup Infrastructure Layer
    // ============================================
    let database_url = "sqlite:vintage_lightbox.db?mode=rwc";
    let pool = create_pool(database_url).await
        .expect("Failed to create database pool");

    // Run migrations
    run_migrations(&pool).await
        .expect("Failed to run database migrations");

    let photo_repository = Arc::new(PhotoRepositoryImpl::new(pool));
    let metadata_extractor = Arc::new(ExifReader);
    let thumbnail_generator = Arc::new(ThumbnailGeneratorImpl::new());
    let image_exporter = Arc::new(ImageExporterImpl::new());

    // ============================================
    // 2. Setup Use Cases Layer
    // ============================================
    let import_photo_use_case = Arc::new(ImportPhotoUseCase::new(
        photo_repository.clone(),
        metadata_extractor,
        thumbnail_generator,
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

    // ============================================
    // 3. Setup Controllers (Adapters Layer)
    // ============================================
    let import_controller = Arc::new(ImportController::new(import_photo_use_case));
    let library_controller = Arc::new(LibraryController::new(photo_repository));
    let editor_controller = Arc::new(EditorController::new(save_photo_edits_use_case));
    let export_controller = Arc::new(ExportController::new(export_photo_use_case));
    let photo_controller = Arc::new(PhotoController::new(
        rate_photo_use_case,
        set_color_label_use_case,
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
            );

            // Load photos on startup
            app.load_photos(&cc.egui_ctx);

            Ok(Box::new(app))
        }),
    )
}
