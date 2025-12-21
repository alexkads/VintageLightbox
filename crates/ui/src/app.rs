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
use crate::async_loader::{AsyncImageProcessor, ImageProcessRequest};

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

    // ============================================
    // Async Image Processing (Rayon-powered + GPU)
    // ============================================
    /// Processes full image loading in background
    image_processor: AsyncImageProcessor,
    /// GPU-accelerated edit processor (primary for sliders)
    gpu_edit_processor: crate::gpu_processor::GpuImageProcessor,
    /// Current edit request ID for tracking latest edit
    current_edit_request_id: u64,
    /// The ID of the photo currently requested for loading (to avoid race conditions)
    requested_photo_id: Option<String>,
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
            image_processor: AsyncImageProcessor::new(),
            gpu_edit_processor: crate::gpu_processor::GpuImageProcessor::new(),
            current_edit_request_id: 0,
            requested_photo_id: None,
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

        // ============================================
        // ASYNC IMAGE LOADING (Non-blocking)
        // ============================================
        
        // Poll for completed image processing results
        if let Some(result) = self.image_processor.poll_result() {
            // Check if this is still the photo we want
            if self.state.develop_selected_photo_id.as_ref() == Some(&result.photo_id) {
                // Store original for before/after
                // Create cached Arc<Vec<u8>> for GPU processing to avoid repeated allocations
                let rgba = result.original_preview.to_rgba8();
                self.state.original_image_data = Some(Arc::new(rgba.into_raw()));
                
                self.state.original_preview = Some(result.original_preview);
                self.state.histogram_data = Some(result.histogram);
                self.state.performance_metrics.image_load_time_ms = Some(result.load_time_ms);
                
                // Calculate TTI (Time To Interactive)
                if let Some(start_time) = self.state.start_load_time {
                    let tti = start_time.elapsed().as_secs_f32() * 1000.0;
                    println!("Controls released for editing effects in {:.2}ms", tti);
                    self.state.start_load_time = None;
                }
                
                // Create texture (measure upload time)
                let upload_start = std::time::Instant::now();
                let texture = ctx.load_texture(
                    format!("display_{}", result.photo_id),
                    result.preview,
                    egui::TextureOptions::default()
                );
                self.state.performance_metrics.texture_upload_time_ms = Some(upload_start.elapsed().as_secs_f32() * 1000.0);
                
                self.state.detail_image = Some(texture);
                // LIGHTROOM-STYLE: Start transition from thumbnail to full-res
                self.state.detail_image_loaded_at = Some(std::time::Instant::now());
                // self.state.thumbnail_preview = None;  // Keep thumbnail for cross-fade transition
                self.state.loaded_photo_id = Some(result.photo_id);
                
                // Force repaint to show the loaded image immediately
                ctx.request_repaint();
            }
        }

        // Poll for completed GPU edit processing results
        if let Some(result) = self.gpu_edit_processor.poll_result() {
            // Only apply if this is the latest request
            if result.request_id >= self.current_edit_request_id.saturating_sub(5) {
                if let Some(photo_id) = &self.state.develop_selected_photo_id.clone() {
                    self.state.performance_metrics.gpu_process_time_ms = Some(result.process_time_ms);

                    let upload_start = std::time::Instant::now();
                    let texture = ctx.load_texture(
                        format!("display_{}", photo_id),
                        result.preview,
                        egui::TextureOptions::default()
                    );
                    self.state.performance_metrics.texture_upload_time_ms = Some(upload_start.elapsed().as_secs_f32() * 1000.0);
                    self.state.detail_image = Some(texture);
                    
                    // Force repaint to show the edited image immediately
                    ctx.request_repaint();
                }
            }
        }

        // Request image loading if needed (non-blocking)
        if let Some(photo_id) = &self.state.develop_selected_photo_id.clone() {
            let needs_reload = self.state.loaded_photo_id.as_ref() != Some(photo_id);
            // Check if we already requested this specific photo to avoid loops
            let already_requested = self.requested_photo_id.as_ref() == Some(photo_id);

            if needs_reload && !already_requested {
                // Update requested ID immediately prevents loop
                self.requested_photo_id = Some(photo_id.clone());

                // Clear previous full-res image (but keep thumbnail for instant preview)
                self.state.detail_image = None;
                self.state.original_preview = None;
                self.state.original_image_data = None;
                self.state.detail_image_loaded_at = None;

                // Find the photo and request async processing
                if let Some(photo) = self.state.photos.iter().find(|p| &p.id == photo_id) {
                    // LIGHTROOM-STYLE: Load thumbnail as instant preview
                    // This gives immediate visual feedback while high-res loads
                    if let Some(thumb_path) = &photo.thumbnail_path {
                        if let Ok(thumb_img) = image::open(thumb_path) {
                            let thumb_texture = crate::image_processing::ImageProcessor::load_texture(
                                ctx,
                                format!("thumb_{}", photo_id),
                                &thumb_img
                            );
                            self.state.thumbnail_preview = Some(thumb_texture);
                        }
                    }

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

                    // Update detail metadata immediately
                    self.state.detail_metadata = Some(crate::state::DetailMetadata {
                        id: photo.id.clone(),
                        name: photo.name.clone(),
                        date: photo.date.clone(),
                        camera: photo.camera.clone(),
                        exposure: photo.exposure.clone(),
                        rating: photo.rating,
                        color_label: photo.color_label.clone(),
                    });

                    // Initialize active values immediately (UI responds instantly)
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
                    // Initialize saved values for auto-save comparison
                    self.state.saved_exposure = exposure;
                    self.state.saved_contrast = contrast;
                    self.state.saved_temperature = temperature;
                    self.state.saved_tint = tint;
                    self.state.saved_highlights = highlights;
                    self.state.saved_shadows = shadows;
                    self.state.saved_whites = whites;
                    self.state.saved_blacks = blacks;
                    self.state.saved_clarity = clarity;
                    self.state.saved_vibrance = vibrance;
                    self.state.saved_saturation = saturation;

                    // Request async image loading (non-blocking!)
                    self.image_processor.request_process(ImageProcessRequest {
                        photo_id: photo_id.clone(),
                        path: photo.path.clone(),
                        exposure,
                        contrast,
                        temperature,
                        tint,
                        highlights,
                        shadows,
                        whites,
                        blacks,
                        clarity,
                        vibrance,
                        saturation,
                        max_preview_size: 2560,
                    });

                    // Start timing TTI
                    self.state.start_load_time = Some(std::time::Instant::now());

                    // Request repaint to poll for results
                    ctx.request_repaint();
                }
            } else if !needs_reload {
                // Photo is loaded, check for edit changes
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

                if (edits_changed || before_toggled) && self.state.original_preview.is_some() {
                    // Save to history if needed
                    if edits_changed && !self.state.show_before {
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
                            true
                        };

                        if should_save {
                            self.state.push_edit_snapshot();
                        }
                    }

                    // Request async edit processing (GPU-accelerated!)
                    if let Some(original) = &self.state.original_preview {
                        if self.state.show_before {
                            // Show original immediately (no processing needed)
                            let texture = crate::image_processing::ImageProcessor::load_texture(
                                ctx,
                                format!("display_{}", photo_id),
                                original
                            );
                            self.state.detail_image = Some(texture);
                        } else {
                            // Request GPU-accelerated edit processing
                            // Use cached data to avoid expensive cloning
                            let (image_data, width, height) = if let Some(data) = &self.state.original_image_data {
                                let width = original.width();
                                let height = original.height();
                                (data.clone(), width, height)
                            } else {
                                // Fallback if cache is missing (shouldn't happen)
                                let rgba = original.to_rgba8();
                                (Arc::new(rgba.clone().into_raw()), rgba.width(), rgba.height())
                            };
                            
                            self.current_edit_request_id = self.gpu_edit_processor.next_request_id();
                            self.gpu_edit_processor.request_process(crate::gpu_processor::GpuProcessRequest {
                                request_id: self.current_edit_request_id,
                                image_data,
                                width,
                                height,
                                params: crate::gpu_processor::GpuEditParams {
                                    exposure: self.state.active_exposure,
                                    contrast: self.state.active_contrast,
                                    temperature: self.state.active_temperature,
                                    tint: self.state.active_tint,
                                    highlights: self.state.active_highlights,
                                    shadows: self.state.active_shadows,
                                    whites: self.state.active_whites,
                                    blacks: self.state.active_blacks,
                                    clarity: self.state.active_clarity,
                                    vibrance: self.state.active_vibrance,
                                    saturation: self.state.active_saturation,
                                    _padding: 0.0,
                                },
                            });

                            // Request repaint to poll for results
                            ctx.request_repaint();
                        }
                    }

                    // Update previous values
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
                    self.state.prev_show_before = self.state.show_before;
                }
            }
        }

        // ============================================
        // AUTO-SAVE DEBOUNCE (500ms delay)
        // ============================================
        const AUTO_SAVE_DEBOUNCE_MS: u128 = 500;
        
        if self.state.pending_auto_save {
            if let Some(last_change) = self.state.last_slider_change_time {
                let elapsed = last_change.elapsed().as_millis();
                if elapsed >= AUTO_SAVE_DEBOUNCE_MS {
                    // Check if values actually changed from last saved state
                    let values_changed =
                        self.state.active_exposure != self.state.saved_exposure ||
                        self.state.active_contrast != self.state.saved_contrast ||
                        self.state.active_temperature != self.state.saved_temperature ||
                        self.state.active_tint != self.state.saved_tint ||
                        self.state.active_highlights != self.state.saved_highlights ||
                        self.state.active_shadows != self.state.saved_shadows ||
                        self.state.active_whites != self.state.saved_whites ||
                        self.state.active_blacks != self.state.saved_blacks ||
                        self.state.active_clarity != self.state.saved_clarity ||
                        self.state.active_vibrance != self.state.saved_vibrance ||
                        self.state.active_saturation != self.state.saved_saturation;

                    if values_changed {
                        if let Some(metadata) = &self.state.detail_metadata {
                            let controller = self.editor_controller.clone();
                            let id = metadata.id.clone();
                            let exposure = self.state.active_exposure;
                            let contrast = self.state.active_contrast;
                            let temperature = self.state.active_temperature;
                            let tint = self.state.active_tint;
                            let highlights = self.state.active_highlights;
                            let shadows = self.state.active_shadows;
                            let whites = self.state.active_whites;
                            let blacks = self.state.active_blacks;
                            let clarity = self.state.active_clarity;
                            let vibrance = self.state.active_vibrance;
                            let saturation = self.state.active_saturation;

                            // Update saved values BEFORE spawning
                            self.state.saved_exposure = exposure;
                            self.state.saved_contrast = contrast;
                            self.state.saved_temperature = temperature;
                            self.state.saved_tint = tint;
                            self.state.saved_highlights = highlights;
                            self.state.saved_shadows = shadows;
                            self.state.saved_whites = whites;
                            self.state.saved_blacks = blacks;
                            self.state.saved_clarity = clarity;
                            self.state.saved_vibrance = vibrance;
                            self.state.saved_saturation = saturation;

                            // Update the PhotoViewModel in the local list to reflect saved edits
                            // This ensures the photo loads with correct values when switching photos
                            if let Some(photo) = self.state.photos.iter_mut().find(|p| p.id == id) {
                                photo.edit_exposure = Some(exposure);
                                photo.edit_contrast = Some(contrast);
                                photo.edit_temperature = Some(temperature);
                                photo.edit_tint = Some(tint);
                                photo.edit_highlights = Some(highlights);
                                photo.edit_shadows = Some(shadows);
                                photo.edit_whites = Some(whites);
                                photo.edit_blacks = Some(blacks);
                                photo.edit_clarity = Some(clarity);
                                photo.edit_vibrance = Some(vibrance);
                                photo.edit_saturation = Some(saturation);
                            }

                            // Clear pending flag
                            self.state.pending_auto_save = false;
                            self.state.last_slider_change_time = None;

                            let ctx_clone = ctx.clone();

                            tokio::spawn(async move {
                                if let Err(e) = controller.save_edits(
                                    id, exposure, contrast, temperature, tint, highlights, shadows,
                                    whites, blacks, clarity, vibrance, saturation,
                                    0.0, 0.0, 0.0, 0.0  // Tone curve (to be implemented in UI)
                                ).await {
                                    eprintln!("Auto-save failed: {}", e);
                                } else {
                                    println!("Auto-saved edits");
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                    }
                    
                    // Clear pending flag even if values didn't change
                    self.state.pending_auto_save = false;
                    self.state.last_slider_change_time = None;
                } else {
                    // Not yet time to save, request repaint to check again
                    ctx.request_repaint();
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

        // Performance Debug Overlay
        self.show_debug_overlay(ctx);

        // Keep UI active if we are waiting for background tasks
        // This prevents the UI from sleeping (gray screen) while image loads
        if self.image_processor.is_processing() || self.requested_photo_id.is_some() {
            ctx.request_repaint();
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

            // Develop button enabled if Library has a selection
            let develop_enabled = self.state.library_selected_photo_id.is_some();
            if develop_enabled {
                if widgets::nav_button(ui, "Develop", self.state.current_view == CurrentView::Develop).clicked() {
                    // Copy Library selection to Develop when entering Develop mode
                    if self.state.develop_selected_photo_id.is_none() {
                        self.state.develop_selected_photo_id = self.state.library_selected_photo_id.clone();
                        self.state.loaded_photo_id = None; // Force image load
                    }
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

    /// Show debug overlay with performance metrics
    fn show_debug_overlay(&self, ctx: &egui::Context) {
        if !self.state.show_performance_stats {
            return;
        }

        let metrics = &self.state.performance_metrics;
        
        egui::Window::new("Performance Stats")
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -10.0))
            .resizable(false)
            .collapsible(true)
            .title_bar(true)
            .default_open(true)
            .show(ctx, |ui| {
                ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                
                egui::Grid::new("perf_grid").num_columns(2).striped(true).show(ui, |ui| {
                    if let Some(t) = metrics.image_load_time_ms {
                        ui.label("Image Load:");
                        ui.label(format!("{:.1} ms", t));
                        ui.end_row();
                    }
                    if let Some(t) = metrics.gpu_process_time_ms {
                        ui.label("GPU Process:");
                        ui.colored_label(
                            if t > 16.0 { egui::Color32::RED } else { egui::Color32::GREEN },
                            format!("{:.1} ms", t)
                        );
                        ui.end_row();
                    }
                    if let Some(t) = metrics.texture_upload_time_ms {
                        ui.label("Texture Upload:");
                        ui.label(format!("{:.1} ms", t));
                        ui.end_row();
                    }
                    
                    // FPS
                    ui.label("FPS:");
                    ui.label(format!("{:.0}", 1.0 / ctx.input(|i| i.stable_dt)));
                    ui.end_row();
                });
            });
    }
}
