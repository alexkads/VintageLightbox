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

        // Create channel for async photo loading (capacity 10 to avoid blocking)
        let (photo_sender, photo_receiver) = mpsc::channel(10);

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
                    println!("Received {} photos from channel", photos.len());
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
            // Check if we need to load a new image
            let needs_reload = self.state.loaded_photo_id.as_ref() != Some(photo_id);

            if needs_reload {
                // Clear previous image first
                self.state.detail_image = None;
                self.state.original_preview = None;

                // Find the photo in our list
                if let Some(photo) = self.state.photos.iter().find(|p| &p.id == photo_id) {
                    // Load image
                    if let Ok(img) = image::open(&photo.path) {
                        // OPTIMIZATION: Resize to preview resolution (1920x1080) for fast loading
                        let preview_img = crate::image_processing::ImageProcessor::resize_for_preview(&img, 1920);

                        // Calculate histogram from preview
                        self.state.histogram_data = Some(
                            crate::components::histogram::HistogramData::from_image(&preview_img)
                        );

                        // Store original preview for real-time processing
                        self.state.original_preview = Some(preview_img.clone());

                        // Load saved edits
                        let exposure = photo.edit_exposure.unwrap_or(0.0);
                        let contrast = photo.edit_contrast.unwrap_or(1.0);
                        let temperature = photo.edit_temperature.unwrap_or(0.0);
                        let tint = photo.edit_tint.unwrap_or(0.0);
                        let highlights = photo.edit_highlights.unwrap_or(0.0);
                        let shadows = photo.edit_shadows.unwrap_or(0.0);
                        let whites = photo.edit_whites.unwrap_or(0.0);
                        let blacks = photo.edit_blacks.unwrap_or(0.0);
                        let clarity = photo.edit_clarity.unwrap_or(0.0);
                        let vibrance = photo.edit_vibrance.unwrap_or(0.0);
                        let saturation = photo.edit_saturation.unwrap_or(0.0);

                        // Initialize active values
                        self.state.active_exposure = exposure;
                        self.state.active_contrast = contrast;
                        self.state.active_temperature = temperature;
                        self.state.active_tint = tint;
                        self.state.active_highlights = highlights;
                        self.state.active_shadows = shadows;
                        self.state.active_whites = whites;
                        self.state.active_blacks = blacks;
                        self.state.active_clarity = clarity;
                        self.state.active_vibrance = vibrance;
                        self.state.active_saturation = saturation;
                        self.state.prev_exposure = exposure;
                        self.state.prev_contrast = contrast;
                        self.state.prev_temperature = temperature;
                        self.state.prev_tint = tint;
                        self.state.prev_highlights = highlights;
                        self.state.prev_shadows = shadows;
                        self.state.prev_whites = whites;
                        self.state.prev_blacks = blacks;
                        self.state.prev_clarity = clarity;
                        self.state.prev_vibrance = vibrance;
                        self.state.prev_saturation = saturation;

                        // Apply edits if they exist
                        let has_edits = exposure != 0.0 || contrast != 1.0 || temperature != 0.0 ||
                                       tint != 0.0 || highlights != 0.0 || shadows != 0.0 ||
                                       whites != 0.0 || blacks != 0.0 || clarity != 0.0 ||
                                       vibrance != 0.0 || saturation != 0.0;
                        let processed = if has_edits {
                            crate::image_processing::ImageProcessor::process_image(
                                &preview_img, exposure, contrast, temperature, tint,
                                highlights, shadows, whites, blacks, clarity, vibrance, saturation
                            )
                        } else {
                            preview_img
                        };

                        // Create texture with unique name per photo
                        let texture = crate::image_processing::ImageProcessor::load_texture(
                            ctx,
                            format!("photo_{}", photo_id),
                            &processed
                        );

                        self.state.detail_image = Some(texture);
                        self.state.loaded_photo_id = Some(photo_id.clone());
                    }
                }
            } else {
                // Photo is already loaded, check if edits changed OR show_before toggled
                let edits_changed =
                    self.state.active_exposure != self.state.prev_exposure ||
                    self.state.active_contrast != self.state.prev_contrast ||
                    self.state.active_temperature != self.state.prev_temperature ||
                    self.state.active_tint != self.state.prev_tint ||
                    self.state.active_highlights != self.state.prev_highlights ||
                    self.state.active_shadows != self.state.prev_shadows ||
                    self.state.active_whites != self.state.prev_whites ||
                    self.state.active_blacks != self.state.prev_blacks ||
                    self.state.active_clarity != self.state.prev_clarity ||
                    self.state.active_vibrance != self.state.prev_vibrance ||
                    self.state.active_saturation != self.state.prev_saturation;
                let before_toggled = self.state.show_before != self.state.prev_show_before;

                if edits_changed || before_toggled {
                    // If edits changed (not just before/after toggle), save to history
                    if edits_changed && !self.state.show_before {
                        // Check if this is actually a new state (not just reprocessing)
                        let should_save = if let Some(index) = self.state.history_index {
                            if let Some(last_snapshot) = self.state.edit_history.get(index) {
                                last_snapshot.exposure != self.state.active_exposure ||
                                last_snapshot.contrast != self.state.active_contrast ||
                                last_snapshot.temperature != self.state.active_temperature ||
                                last_snapshot.tint != self.state.active_tint ||
                                last_snapshot.highlights != self.state.active_highlights ||
                                last_snapshot.shadows != self.state.active_shadows ||
                                last_snapshot.whites != self.state.active_whites ||
                                last_snapshot.blacks != self.state.active_blacks ||
                                last_snapshot.clarity != self.state.active_clarity ||
                                last_snapshot.vibrance != self.state.active_vibrance ||
                                last_snapshot.saturation != self.state.active_saturation
                            } else {
                                true
                            }
                        } else {
                            true // No history yet
                        };

                        if should_save {
                            self.state.push_edit_snapshot();
                        }
                    }

                    // Reprocess image
                    if let Some(original) = &self.state.original_preview {
                        // If showing "before", use original without edits
                        // Otherwise, apply current edits
                        let processed = if self.state.show_before {
                            original.clone()
                        } else {
                            crate::image_processing::ImageProcessor::process_image(
                                original,
                                self.state.active_exposure,
                                self.state.active_contrast,
                                self.state.active_temperature,
                                self.state.active_tint,
                                self.state.active_highlights,
                                self.state.active_shadows,
                                self.state.active_whites,
                                self.state.active_blacks,
                                self.state.active_clarity,
                                self.state.active_vibrance,
                                self.state.active_saturation,
                            )
                        };

                        // Update texture
                        let texture = crate::image_processing::ImageProcessor::load_texture(
                            ctx,
                            format!("photo_{}", photo_id),
                            &processed
                        );

                        self.state.detail_image = Some(texture);

                        // Update previous values only if not in before mode
                        if !self.state.show_before {
                            self.state.prev_exposure = self.state.active_exposure;
                            self.state.prev_contrast = self.state.active_contrast;
                            self.state.prev_temperature = self.state.active_temperature;
                            self.state.prev_tint = self.state.active_tint;
                            self.state.prev_highlights = self.state.active_highlights;
                            self.state.prev_shadows = self.state.active_shadows;
                            self.state.prev_whites = self.state.active_whites;
                            self.state.prev_blacks = self.state.active_blacks;
                            self.state.prev_clarity = self.state.active_clarity;
                            self.state.prev_vibrance = self.state.active_vibrance;
                            self.state.prev_saturation = self.state.active_saturation;
                        }

                        // Always update prev_show_before
                        self.state.prev_show_before = self.state.show_before;
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
                        &self.library_controller,
                        &self.photo_sender,
                        ctx,
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
        let sender = self.photo_sender.clone();

        self.state.is_busy = true;
        self.state.busy_message = "Importing photos...".to_string();

        // Spawn file dialog
        tokio::spawn(async move {
            let file_dialog = rfd::AsyncFileDialog::new()
                .add_filter("Images", &["jpg", "jpeg", "png", "raw", "cr2", "nef", "arw"])
                .set_title("Import Photos");

            let files_opt = file_dialog.pick_files().await;

            // Always reload photos at the end, even if canceled
            let result = if let Some(files) = files_opt {
                if !files.is_empty() {
                    // Collect all file paths
                    let paths: Vec<String> = files
                        .iter()
                        .filter_map(|f| f.path().to_str().map(|s| s.to_string()))
                        .collect();

                    if !paths.is_empty() {
                        // Import all photos at once
                        match import_controller.import_files(paths).await {
                            Ok(_) => {
                                println!("Photos imported successfully");
                                // Reload photos after successful import
                                library_controller.get_all_photos().await
                            }
                            Err(e) => {
                                eprintln!("Failed to import photos: {}", e);
                                // Still reload to show any partial imports
                                library_controller.get_all_photos().await
                            }
                        }
                    } else {
                        library_controller.get_all_photos().await
                    }
                } else {
                    library_controller.get_all_photos().await
                }
            } else {
                // User canceled, still reload to ensure consistency
                library_controller.get_all_photos().await
            };

            // Send result to UI
            let _ = sender.send(result).await;
            ctx.request_repaint();
        });
    }
}
