// Main Application Structure
// VintageLightboxApp implements eframe::App and manages the UI loop

use eframe::egui;
use std::sync::Arc;
use tokio::sync::mpsc;

use adapters::controllers::*;
use adapters::view_models::PhotoViewModel;
use crate::state::{AppState, CurrentView};
use crate::design_system::{theme::Theme, widgets};
use crate::views::{library_view::LibraryView, develop_view::DevelopView};
use crate::keyboard::KeyboardHandler;

/// Main application struct
pub struct VintageLightboxApp {
    /// Application state
    pub state: AppState,

    // ============================================
    // Controllers (from adapters layer)
    // Clean Architecture: UI depends on adapters
    // ============================================
    pub import_controller: Arc<ImportController>,
    pub library_controller: Arc<LibraryController>,
    pub editor_controller: Arc<EditorController>,
    pub export_controller: Arc<ExportController>,
    pub photo_controller: Arc<PhotoController>,

    // ============================================
    // Views
    // ============================================
    library_view: LibraryView,
    develop_view: DevelopView,

    // ============================================
    // Input Handlers
    // ============================================
    keyboard_handler: KeyboardHandler,

    // ============================================
    // Async Communication
    // ============================================
    photo_receiver: mpsc::Receiver<Result<Vec<PhotoViewModel>, String>>,
    photo_sender: mpsc::Sender<Result<Vec<PhotoViewModel>, String>>,
}

impl VintageLightboxApp {
    /// Create a new VintageLightboxApp instance
    /// This is called from main.rs with infrastructure setup
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        import_controller: Arc<ImportController>,
        library_controller: Arc<LibraryController>,
        editor_controller: Arc<EditorController>,
        export_controller: Arc<ExportController>,
        photo_controller: Arc<PhotoController>,
    ) -> Self {
        // Apply custom theme
        Theme::apply_to_context(&cc.egui_ctx);

        // Create channel for async photo loading
        let (photo_sender, photo_receiver) = mpsc::channel(1);

        Self {
            state: AppState::new(),
            import_controller,
            library_controller,
            editor_controller,
            export_controller,
            photo_controller,
            library_view: LibraryView::new(),
            develop_view: DevelopView::new(),
            keyboard_handler: KeyboardHandler::new(),
            photo_receiver,
            photo_sender,
        }
    }

    /// Load all photos from the library
    pub fn load_photos(&mut self, ctx: &egui::Context) {
        let library_controller = self.library_controller.clone();
        let ctx = ctx.clone();
        let sender = self.photo_sender.clone();

        self.state.is_busy = true;
        self.state.busy_message = "Loading photos...".to_string();

        // Spawn task to load photos
        tokio::spawn(async move {
            let result = library_controller.get_all_photos().await;
            let _ = sender.send(result).await;
            ctx.request_repaint();
        });
    }
}

impl eframe::App for VintageLightboxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for async photo loading results
        if let Ok(result) = self.photo_receiver.try_recv() {
            match result {
                Ok(photos) => {
                    self.state.photos = photos;
                }
                Err(e) => {
                    eprintln!("Failed to load photos: {}", e);
                }
            }
            self.state.is_busy = false;
            self.state.busy_message.clear();
        }

        // Handle keyboard input
        self.keyboard_handler.handle_input(ctx, &mut self.state, &self.photo_controller);

        // Load image for selected photo if needed
        if let Some(photo_id) = &self.state.selected_photo_id.clone() {
            if self.state.detail_image.is_none() {
                // Find the photo in our list
                if let Some(photo) = self.state.photos.iter().find(|p| &p.id == photo_id) {
                    // Load image synchronously for now (TODO: make async)
                    if let Ok(img) = image::open(&photo.path) {
                        // Calculate histogram
                        self.state.histogram_data = Some(
                            crate::components::histogram::HistogramData::from_image(&img)
                        );
                        
                        // Apply edits if they exist
                        let exposure = photo.edit_exposure.unwrap_or(0.0);
                        let contrast = photo.edit_contrast.unwrap_or(1.0);
                        
                        let processed = if exposure != 0.0 || contrast != 1.0 {
                            crate::image_processing::ImageProcessor::process_image(&img, exposure, contrast)
                        } else {
                            img
                        };
                        
                        // Create texture
                        let texture = crate::image_processing::ImageProcessor::load_texture(
                            ctx,
                            format!("detail_{}", photo_id),
                            &processed,
                        );
                        
                        self.state.detail_image = Some(texture);
                    }
                }
            }
        }

        // Top toolbar
        egui::TopBottomPanel::top("toolbar")
            .exact_height(Theme::TOOLBAR_HEIGHT)
            .show(ctx, |ui| {
                self.show_toolbar(ui);
            });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state.current_view {
                CurrentView::Library => {
                    self.library_view.show(ui, &mut self.state, ctx);
                }
                CurrentView::Develop => {
                    self.develop_view.show(
                        ui,
                        &mut self.state,
                        &self.editor_controller,
                        &self.export_controller,
                        &self.photo_controller,
                    );
                }
            }
        });

        // Busy overlay
        if self.state.is_busy {
            widgets::show_busy_overlay(ctx, &self.state.busy_message);
        }
    }
}

impl VintageLightboxApp {
    /// Show the toolbar at the top of the window
    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(Theme::SPACE_LG);

            // App title
            ui.label(
                egui::RichText::new("VintageLightbox")
                    .size(Theme::FONT_LG)
                    .color(Theme::TEXT_PRIMARY)
            );

            ui.add_space(Theme::SPACE_XXL);

            // View tabs
            if widgets::nav_button(ui, "Library", self.state.current_view == CurrentView::Library).clicked() {
                self.state.current_view = CurrentView::Library;
                self.state.reset_viewer();
            }

            ui.add_space(Theme::SPACE_SM);

            let develop_enabled = self.state.selected_photo_id.is_some();
            if develop_enabled {
                if widgets::nav_button(ui, "Develop", self.state.current_view == CurrentView::Develop).clicked() {
                    self.state.current_view = CurrentView::Develop;
                }
            } else {
                ui.add_enabled_ui(false, |ui| {
                    widgets::nav_button(ui, "Develop", false);
                });
            }

            // Spacer to push photo count and import button to the right
            ui.allocate_space(egui::vec2(ui.available_width() - 250.0, 0.0));

            // Photo count
            let photo_count = self.state.photos.len();
            ui.label(
                egui::RichText::new(format!("{} photos", photo_count))
                    .size(Theme::FONT_MD)
                    .color(Theme::TEXT_SECONDARY)
            );

            ui.add_space(Theme::SPACE_LG);

            // Import button
            if widgets::primary_button(ui, "Import").clicked() {
                self.handle_import(ui.ctx());
            }

            ui.add_space(Theme::SPACE_LG);
        });
    }

    /// Handle import button click
    fn handle_import(&mut self, ctx: &egui::Context) {
        let import_controller = self.import_controller.clone();
        let library_controller = self.library_controller.clone();
        let ctx = ctx.clone();

        // Spawn file dialog
        tokio::spawn(async move {
            let file_dialog = rfd::AsyncFileDialog::new()
                .add_filter("Images", &["jpg", "jpeg", "png", "raw", "cr2", "nef", "arw"])
                .set_title("Import Photos");

            if let Some(file) = file_dialog.pick_file().await {
                if let Some(path) = file.path().to_str() {
                    // Import the photo
                    if let Err(e) = import_controller.import_files(vec![path.to_string()]).await {
                        eprintln!("Failed to import photo: {}", e);
                    } else {
                        // Reload photos after import
                        if let Ok(_photos) = library_controller.get_all_photos().await {
                            ctx.request_repaint();
                        }
                    }
                }
            }
        });
    }
}
