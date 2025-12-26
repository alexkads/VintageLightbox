// Main Application Structure
// VintageLightboxApp implements eframe::App and manages the UI loop

use eframe::egui;
use std::sync::Arc;
use tokio::sync::mpsc;
use infrastructure::cache::preview_manager::PreviewManager;
use egui_dock::DockArea;

use adapters::controllers::*;
use adapters::view_models::PhotoViewModel;
use crate::state::{AppState, CurrentView};
use crate::design_system::{theme::Theme, widgets};
use crate::keyboard::KeyboardHandler;
use crate::async_loader::{AsyncImageProcessor, ImageProcessRequest};
use crate::docking::{DockViewer, DockViewerContext};
use crate::components::{photo_grid::PhotoGrid, filmstrip::Filmstrip};
use crate::components::settings_dialog::{SettingsDialog, SettingsAction};

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
    pub preset_controller: Arc<PresetController>,

    // ============================================
    // Input Handlers
    // ============================================
    keyboard_handler: KeyboardHandler,

    // ============================================
    // Async Communication
    // ============================================
    photo_receiver: mpsc::Receiver<Result<Vec<PhotoViewModel>, String>>,
    photo_sender: mpsc::Sender<Result<Vec<PhotoViewModel>, String>>,
    preset_receiver: mpsc::Receiver<Result<Vec<domain::entities::Preset>, String>>,
    preset_sender: mpsc::Sender<Result<Vec<domain::entities::Preset>, String>>,
    
    import_source_sender: mpsc::Sender<(Vec<domain::import_source::ImportSource>, Vec<domain::import_source::ImportSource>)>,
    #[allow(dead_code)]
    import_source_receiver: mpsc::Receiver<(Vec<domain::import_source::ImportSource>, Vec<domain::import_source::ImportSource>)>,

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
    
    /// Preview Manager for instant sync lookups
    preview_manager: Arc<PreviewManager>,
    
    // ============================================
    // Docking UI Components
    // ============================================
    photo_grid: PhotoGrid,
    filmstrip: Filmstrip,
    library_dock_state: egui_dock::DockState<crate::docking::DockTab>,
    develop_dock_state: egui_dock::DockState<crate::docking::DockTab>,
    
    /// Flag to trigger initial photo load on first frame
    needs_initial_load: bool,
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
        preset_controller: Arc<PresetController>,
        preview_manager: Arc<PreviewManager>,
    ) -> Self {
        // Apply default theme
        use crate::design_system::theme_selector::ThemeVariant;
        ThemeVariant::default().apply_to_context(&cc.egui_ctx);

        // Initialize Phosphor icon fonts
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        // Create channel for async photo loading (capacity 10 to avoid blocking)
        let (photo_sender, photo_receiver) = mpsc::channel(10);
        // Create channel for async preset loading
        let (preset_sender, preset_receiver) = mpsc::channel(10);
        // Create channel for async import source loading
        let (import_source_sender, import_source_receiver) = mpsc::channel(5);

        Self {
            state: AppState::new(),
            import_controller,
            library_controller,
            editor_controller,
            export_controller,
            photo_controller,
            preset_controller,
            keyboard_handler: KeyboardHandler::new(),
            photo_receiver,
            photo_sender,
            preset_receiver,
            preset_sender,
            import_source_sender,
            import_source_receiver,
            image_processor: AsyncImageProcessor::new(preview_manager.clone()),
            gpu_edit_processor: crate::gpu_processor::GpuImageProcessor::new(),
            current_edit_request_id: 0,
            requested_photo_id: None,
            photo_grid: PhotoGrid::new(preview_manager.clone()),
            filmstrip: Filmstrip::new(preview_manager.clone()),
            // Load dock states from storage, or create defaults
            library_dock_state: cc.storage
                .and_then(|s| eframe::get_value(s, "library_dock_state"))
                .unwrap_or_else(crate::docking::create_library_layout),
            develop_dock_state: cc.storage
                .and_then(|s| eframe::get_value(s, "develop_dock_state"))
                .unwrap_or_else(crate::docking::create_develop_layout),
            preview_manager,
            needs_initial_load: true,
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

    /// Load all presets from the database
    pub fn load_presets(&mut self, ctx: &egui::Context) {
        let preset_controller = self.preset_controller.clone();
        let ctx = ctx.clone();
        let sender = self.preset_sender.clone();

        // Spawn task to load presets
        tokio::spawn(async move {
            let result = preset_controller.list_presets().await;
            let _ = sender.send(result).await;
            ctx.request_repaint();
        });
    }
}


impl eframe::App for VintageLightboxApp {
    /// Save dock layouts to storage on exit
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "library_dock_state", &self.library_dock_state);
        eframe::set_value(storage, "develop_dock_state", &self.develop_dock_state);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Trigger initial photo and preset loading on first frame
        if self.needs_initial_load {
            self.needs_initial_load = false;
            self.load_photos(ctx);
            self.load_presets(ctx);
        }

        // Poll for async photo loading results
        if let Ok(result) = self.photo_receiver.try_recv() {
            match result {
                Ok(photos) => {
                    self.state.photos = photos;
                    self.state.rebuild_folder_tree();
                }
                Err(e) => {
                    self.state.toasts.error(format!("Failed to load photos: {}", e));
                }
            }
            self.state.is_busy = false;
            self.state.busy_message.clear();
        }

        // Poll for async preset loading results
        if let Ok(result) = self.preset_receiver.try_recv() {
            match result {
                Ok(presets) => {
                    self.state.presets = presets;
                }
                Err(e) => {
                    self.state.toasts.error(format!("Failed to load presets: {}", e));
                }
            }
        }

        // Poll for import preview dialog
        if let Some(receiver) = &mut self.state.pending_import_preview_receiver {
            if let Ok(dialog_opt) = receiver.try_recv() {
                if let Some(dialog) = dialog_opt {
                    self.state.import_preview_dialog = Some(dialog);
                }
                self.state.is_busy = false;
                self.state.busy_message.clear();
                self.state.pending_import_preview_receiver = None;
            }
        }

        // Poll for export results
        if let Some(receiver) = &mut self.state.pending_export_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(path) => {
                        self.state.toasts.success("Exported successfully!");
                        // Open the folder containing the exported file
                        if let Some(parent) = std::path::Path::new(&path).parent() {
                            if let Err(e) = opener::open(parent) {
                                eprintln!("Failed to open folder: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        // Don't show error toast for user cancellation
                        if e != "Cancelled" {
                            self.state.toasts.error(format!("Export failed: {}", e));
                        }
                    }
                }
                self.state.is_busy = false;
                self.state.busy_message.clear();
                self.state.pending_export_receiver = None;
            }
        }

        // Handle keyboard input
        // Handle keyboard input
        self.keyboard_handler.handle_input(
            ctx,
            &mut self.state,
            &self.photo_controller,
            &self.library_controller,
            &self.editor_controller,
            &self.export_controller,
            &self.import_controller,
            &self.photo_sender
        );

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
                    let _tti = start_time.elapsed().as_secs_f32() * 1000.0;
                    // TTI measurement complete (could be logged to metrics)
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
                    // Load saved edits FIRST (needed for thumbnail processing)
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
                    
                    // Tone Curve
                    let tone_curve_shadows = photo.edit_tone_curve_shadows.unwrap_or(0.0);
                    let tone_curve_darks = photo.edit_tone_curve_darks.unwrap_or(0.0);
                    let tone_curve_lights = photo.edit_tone_curve_lights.unwrap_or(0.0);
                    let tone_curve_highlights = photo.edit_tone_curve_highlights.unwrap_or(0.0);

                    // HSL Saturation
                    let hsl_red_sat = photo.edit_hsl_red_sat.unwrap_or(0.0);
                    let hsl_orange_sat = photo.edit_hsl_orange_sat.unwrap_or(0.0);
                    let hsl_yellow_sat = photo.edit_hsl_yellow_sat.unwrap_or(0.0);
                    let hsl_green_sat = photo.edit_hsl_green_sat.unwrap_or(0.0);
                    let hsl_aqua_sat = photo.edit_hsl_aqua_sat.unwrap_or(0.0);
                    let hsl_blue_sat = photo.edit_hsl_blue_sat.unwrap_or(0.0);
                    let hsl_purple_sat = photo.edit_hsl_purple_sat.unwrap_or(0.0);
                    let hsl_magenta_sat = photo.edit_hsl_magenta_sat.unwrap_or(0.0);
                    // HSL Hue
                    let hsl_red_hue = photo.edit_hsl_red_hue.unwrap_or(0.0);
                    let hsl_orange_hue = photo.edit_hsl_orange_hue.unwrap_or(0.0);
                    let hsl_yellow_hue = photo.edit_hsl_yellow_hue.unwrap_or(0.0);
                    let hsl_green_hue = photo.edit_hsl_green_hue.unwrap_or(0.0);
                    let hsl_aqua_hue = photo.edit_hsl_aqua_hue.unwrap_or(0.0);
                    let hsl_blue_hue = photo.edit_hsl_blue_hue.unwrap_or(0.0);
                    let hsl_purple_hue = photo.edit_hsl_purple_hue.unwrap_or(0.0);
                    let hsl_magenta_hue = photo.edit_hsl_magenta_hue.unwrap_or(0.0);
                    // HSL Lum
                    let hsl_red_lum = photo.edit_hsl_red_lum.unwrap_or(0.0);
                    let hsl_orange_lum = photo.edit_hsl_orange_lum.unwrap_or(0.0);
                    let hsl_yellow_lum = photo.edit_hsl_yellow_lum.unwrap_or(0.0);
                    let hsl_green_lum = photo.edit_hsl_green_lum.unwrap_or(0.0);
                    let hsl_aqua_lum = photo.edit_hsl_aqua_lum.unwrap_or(0.0);
                    let hsl_blue_lum = photo.edit_hsl_blue_lum.unwrap_or(0.0);
                    let hsl_purple_lum = photo.edit_hsl_purple_lum.unwrap_or(0.0);
                    let hsl_magenta_lum = photo.edit_hsl_magenta_lum.unwrap_or(0.0);
                    // Lens
                    let lens_distortion = photo.edit_lens_distortion.unwrap_or(0.0);
                    let lens_vignette_amount = photo.edit_lens_vignette_amount.unwrap_or(0.0);
                    let lens_vignette_midpoint = photo.edit_lens_vignette_midpoint.unwrap_or(0.0);
                    // NR
                    let nr_luminance = photo.edit_nr_luminance.unwrap_or(0.0);
                    let nr_color = photo.edit_nr_color.unwrap_or(0.0);
                    // Sharpening
                    let sharpen_amount = photo.edit_sharpen_amount.unwrap_or(0.0);
                    let sharpen_radius = photo.edit_sharpen_radius.unwrap_or(1.0);
                    
                    // LIGHTROOM-STYLE: Load thumbnail as instant preview WITH EFFECTS APPLIED
                    // This gives immediate visual feedback that matches the final look
                    // Try to load from PreviewManager (BLOB cache) FIRST
                    if let Some(thumb_img) = self.preview_manager.get_thumbnail(photo_id) {
                         // Apply the same effects to thumbnail for consistent appearance
                         let processed_thumb = crate::image_processing::ImageProcessor::process_image(
                             &thumb_img,
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
                             tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
                             hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
                             hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
                             // HSL Hue
                             hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue,
                             hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
                             // HSL Lum
                             hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum,
                             hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
                             // Lens
                             lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
                             // NR
                             nr_luminance, nr_color,
                             // Sharpening
                             sharpen_amount, sharpen_radius,
                         );
                         
                         let thumb_texture = crate::image_processing::ImageProcessor::load_texture(
                             ctx,
                             format!("thumb_{}", photo_id),
                             &processed_thumb
                         );
                         self.state.thumbnail_preview = Some(thumb_texture);
                    } else if let Some(thumb_path) = &photo.thumbnail_path {
                        // Fallback to legacy file path (migration support)
                        if let Ok(thumb_img) = image::open(thumb_path) {
                            let processed_thumb = crate::image_processing::ImageProcessor::process_image(
                                &thumb_img,
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
                                tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
                                hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
                                hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
                                // HSL Hue
                                hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue,
                                hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
                                // HSL Lum
                                hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum,
                                hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
                                // Lens
                                lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
                                // NR
                                nr_luminance, nr_color,
                                // Sharpening
                                sharpen_amount, sharpen_radius,
                            );
                            
                            let thumb_texture = crate::image_processing::ImageProcessor::load_texture(
                                ctx,
                                format!("thumb_{}", photo_id),
                                &processed_thumb
                            );
                            self.state.thumbnail_preview = Some(thumb_texture);
                        }
                    }

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
                    // Tone Curve
                    self.state.saved_tone_curve_shadows = tone_curve_shadows;
                    self.state.saved_tone_curve_darks = tone_curve_darks;
                    self.state.saved_tone_curve_lights = tone_curve_lights;
                    self.state.saved_tone_curve_highlights = tone_curve_highlights;
                    // HSL Saturation state
                    self.state.active_hsl_red_sat = hsl_red_sat;
                    self.state.active_hsl_orange_sat = hsl_orange_sat;
                    self.state.active_hsl_yellow_sat = hsl_yellow_sat;
                    self.state.active_hsl_green_sat = hsl_green_sat;
                    self.state.active_hsl_aqua_sat = hsl_aqua_sat;
                    self.state.active_hsl_blue_sat = hsl_blue_sat;
                    self.state.active_hsl_purple_sat = hsl_purple_sat;
                    self.state.active_hsl_magenta_sat = hsl_magenta_sat;
                    
                    self.state.prev_hsl_red_sat = hsl_red_sat;
                    self.state.prev_hsl_orange_sat = hsl_orange_sat;
                    self.state.prev_hsl_yellow_sat = hsl_yellow_sat;
                    self.state.prev_hsl_green_sat = hsl_green_sat;
                    self.state.prev_hsl_aqua_sat = hsl_aqua_sat;
                    self.state.prev_hsl_blue_sat = hsl_blue_sat;
                    self.state.prev_hsl_purple_sat = hsl_purple_sat;
                    self.state.prev_hsl_magenta_sat = hsl_magenta_sat;
                    
                    self.state.saved_hsl_red_sat = hsl_red_sat;
                    self.state.saved_hsl_orange_sat = hsl_orange_sat;
                    self.state.saved_hsl_yellow_sat = hsl_yellow_sat;
                    self.state.saved_hsl_green_sat = hsl_green_sat;
                    self.state.saved_hsl_aqua_sat = hsl_aqua_sat;
                    self.state.saved_hsl_blue_sat = hsl_blue_sat;
                    self.state.saved_hsl_purple_sat = hsl_purple_sat;
                    self.state.saved_hsl_magenta_sat = hsl_magenta_sat;

                    // HSL Hue state
                    self.state.active_hsl_red_hue = hsl_red_hue;
                    self.state.active_hsl_orange_hue = hsl_orange_hue;
                    self.state.active_hsl_yellow_hue = hsl_yellow_hue;
                    self.state.active_hsl_green_hue = hsl_green_hue;
                    self.state.active_hsl_aqua_hue = hsl_aqua_hue;
                    self.state.active_hsl_blue_hue = hsl_blue_hue;
                    self.state.active_hsl_purple_hue = hsl_purple_hue;
                    self.state.active_hsl_magenta_hue = hsl_magenta_hue;
                    
                    self.state.prev_hsl_red_hue = hsl_red_hue;
                    self.state.prev_hsl_orange_hue = hsl_orange_hue;
                    self.state.prev_hsl_yellow_hue = hsl_yellow_hue;
                    self.state.prev_hsl_green_hue = hsl_green_hue;
                    self.state.prev_hsl_aqua_hue = hsl_aqua_hue;
                    self.state.prev_hsl_blue_hue = hsl_blue_hue;
                    self.state.prev_hsl_purple_hue = hsl_purple_hue;
                    self.state.prev_hsl_magenta_hue = hsl_magenta_hue;
                    
                    self.state.saved_hsl_red_hue = hsl_red_hue;
                    self.state.saved_hsl_orange_hue = hsl_orange_hue;
                    self.state.saved_hsl_yellow_hue = hsl_yellow_hue;
                    self.state.saved_hsl_green_hue = hsl_green_hue;
                    self.state.saved_hsl_aqua_hue = hsl_aqua_hue;
                    self.state.saved_hsl_blue_hue = hsl_blue_hue;
                    self.state.saved_hsl_purple_hue = hsl_purple_hue;
                    self.state.saved_hsl_magenta_hue = hsl_magenta_hue;

                    // HSL Lum state
                    self.state.active_hsl_red_lum = hsl_red_lum;
                    self.state.active_hsl_orange_lum = hsl_orange_lum;
                    self.state.active_hsl_yellow_lum = hsl_yellow_lum;
                    self.state.active_hsl_green_lum = hsl_green_lum;
                    self.state.active_hsl_aqua_lum = hsl_aqua_lum;
                    self.state.active_hsl_blue_lum = hsl_blue_lum;
                    self.state.active_hsl_purple_lum = hsl_purple_lum;
                    self.state.active_hsl_magenta_lum = hsl_magenta_lum;
                    
                    self.state.prev_hsl_red_lum = hsl_red_lum;
                    self.state.prev_hsl_orange_lum = hsl_orange_lum;
                    self.state.prev_hsl_yellow_lum = hsl_yellow_lum;
                    self.state.prev_hsl_green_lum = hsl_green_lum;
                    self.state.prev_hsl_aqua_lum = hsl_aqua_lum;
                    self.state.prev_hsl_blue_lum = hsl_blue_lum;
                    self.state.prev_hsl_purple_lum = hsl_purple_lum;
                    self.state.prev_hsl_magenta_lum = hsl_magenta_lum;
                    
                    self.state.saved_hsl_red_lum = hsl_red_lum;
                    self.state.saved_hsl_orange_lum = hsl_orange_lum;
                    self.state.saved_hsl_yellow_lum = hsl_yellow_lum;
                    self.state.saved_hsl_green_lum = hsl_green_lum;
                    self.state.saved_hsl_aqua_lum = hsl_aqua_lum;
                    self.state.saved_hsl_blue_lum = hsl_blue_lum;
                    self.state.saved_hsl_purple_lum = hsl_purple_lum;
                    self.state.saved_hsl_magenta_lum = hsl_magenta_lum;

                    // Lens
                    self.state.active_lens_distortion = lens_distortion;
                    self.state.active_lens_vignette_amount = lens_vignette_amount;
                    self.state.active_lens_vignette_midpoint = lens_vignette_midpoint;
                    self.state.prev_lens_distortion = lens_distortion;
                    self.state.prev_lens_vignette_amount = lens_vignette_amount;
                    self.state.prev_lens_vignette_midpoint = lens_vignette_midpoint;
                    self.state.saved_lens_distortion = lens_distortion;
                    self.state.saved_lens_vignette_amount = lens_vignette_amount;
                    self.state.saved_lens_vignette_midpoint = lens_vignette_midpoint;

                    // NR
                    self.state.active_nr_luminance = nr_luminance;
                    self.state.active_nr_color = nr_color;
                    self.state.prev_nr_luminance = nr_luminance;
                    self.state.prev_nr_color = nr_color;
                    self.state.saved_nr_luminance = nr_luminance;
                    self.state.saved_nr_color = nr_color;

                    // Sharpening
                    self.state.active_sharpen_amount = sharpen_amount;
                    self.state.active_sharpen_radius = sharpen_radius;
                    self.state.prev_sharpen_amount = sharpen_amount;
                    self.state.prev_sharpen_radius = sharpen_radius;
                    self.state.saved_sharpen_amount = sharpen_amount;
                    self.state.saved_sharpen_radius = sharpen_radius;

                    // Initialize Tone Curve active/prev values (saved already done above)
                    self.state.active_tone_curve_shadows = tone_curve_shadows;
                    self.state.active_tone_curve_darks = tone_curve_darks;
                    self.state.active_tone_curve_lights = tone_curve_lights;
                    self.state.active_tone_curve_highlights = tone_curve_highlights;
                    self.state.prev_tone_curve_shadows = tone_curve_shadows;
                    self.state.prev_tone_curve_darks = tone_curve_darks;
                    self.state.prev_tone_curve_lights = tone_curve_lights;
                    self.state.prev_tone_curve_highlights = tone_curve_highlights;

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
                        tone_curve_shadows,
                        tone_curve_darks,
                        tone_curve_lights,
                        tone_curve_highlights,
                        // HSL Saturation
                        hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
                        hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
                        // HSL Hue
                        hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue,
                        hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
                        // HSL Lum
                        hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum,
                        hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
                        // Lens
                        lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
                        // NR
                        nr_luminance, nr_color,
                        // Sharpening
                        sharpen_amount, sharpen_radius,
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
                    self.state.active_saturation != self.state.prev_saturation ||
                    self.state.active_tone_curve_shadows != self.state.prev_tone_curve_shadows ||
                    self.state.active_tone_curve_darks != self.state.prev_tone_curve_darks ||
                    self.state.active_tone_curve_lights != self.state.prev_tone_curve_lights ||
                    self.state.active_tone_curve_highlights != self.state.prev_tone_curve_highlights ||
                    // HSL Saturation
                    self.state.active_hsl_red_sat != self.state.prev_hsl_red_sat ||
                    self.state.active_hsl_orange_sat != self.state.prev_hsl_orange_sat ||
                    self.state.active_hsl_yellow_sat != self.state.prev_hsl_yellow_sat ||
                    self.state.active_hsl_green_sat != self.state.prev_hsl_green_sat ||
                    self.state.active_hsl_aqua_sat != self.state.prev_hsl_aqua_sat ||
                    self.state.active_hsl_blue_sat != self.state.prev_hsl_blue_sat ||
                    self.state.active_hsl_purple_sat != self.state.prev_hsl_purple_sat ||
                    self.state.active_hsl_magenta_sat != self.state.prev_hsl_magenta_sat ||
                    // HSL Hue
                    self.state.active_hsl_red_hue != self.state.prev_hsl_red_hue ||
                    self.state.active_hsl_orange_hue != self.state.prev_hsl_orange_hue ||
                    self.state.active_hsl_yellow_hue != self.state.prev_hsl_yellow_hue ||
                    self.state.active_hsl_green_hue != self.state.prev_hsl_green_hue ||
                    self.state.active_hsl_aqua_hue != self.state.prev_hsl_aqua_hue ||
                    self.state.active_hsl_blue_hue != self.state.prev_hsl_blue_hue ||
                    self.state.active_hsl_purple_hue != self.state.prev_hsl_purple_hue ||
                    self.state.active_hsl_magenta_hue != self.state.prev_hsl_magenta_hue ||
                    // HSL Lum
                    self.state.active_hsl_red_lum != self.state.prev_hsl_red_lum ||
                    self.state.active_hsl_orange_lum != self.state.prev_hsl_orange_lum ||
                    self.state.active_hsl_yellow_lum != self.state.prev_hsl_yellow_lum ||
                    self.state.active_hsl_green_lum != self.state.prev_hsl_green_lum ||
                    self.state.active_hsl_aqua_lum != self.state.prev_hsl_aqua_lum ||
                    self.state.active_hsl_blue_lum != self.state.prev_hsl_blue_lum ||
                    self.state.active_hsl_purple_lum != self.state.prev_hsl_purple_lum ||
                    self.state.active_hsl_magenta_lum != self.state.prev_hsl_magenta_lum ||
                    // Lens
                    self.state.active_lens_distortion != self.state.prev_lens_distortion ||
                    self.state.active_lens_vignette_amount != self.state.prev_lens_vignette_amount ||
                    self.state.active_lens_vignette_midpoint != self.state.prev_lens_vignette_midpoint ||
                    // NR
                    self.state.active_nr_luminance != self.state.prev_nr_luminance ||
                    self.state.active_nr_color != self.state.prev_nr_color ||
                    // Sharpen
                    self.state.active_sharpen_amount != self.state.prev_sharpen_amount ||
                    self.state.active_sharpen_radius != self.state.prev_sharpen_radius;
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
                                last_snapshot.saturation != self.state.active_saturation ||
                                last_snapshot.tone_curve_shadows != self.state.active_tone_curve_shadows ||
                                last_snapshot.tone_curve_darks != self.state.active_tone_curve_darks ||
                                last_snapshot.tone_curve_lights != self.state.active_tone_curve_lights ||
                                last_snapshot.tone_curve_highlights != self.state.active_tone_curve_highlights ||
                                // HSL Sat
                                last_snapshot.hsl_red_sat != self.state.active_hsl_red_sat ||
                                last_snapshot.hsl_orange_sat != self.state.active_hsl_orange_sat ||
                                last_snapshot.hsl_yellow_sat != self.state.active_hsl_yellow_sat ||
                                last_snapshot.hsl_green_sat != self.state.active_hsl_green_sat ||
                                last_snapshot.hsl_aqua_sat != self.state.active_hsl_aqua_sat ||
                                last_snapshot.hsl_blue_sat != self.state.active_hsl_blue_sat ||
                                last_snapshot.hsl_purple_sat != self.state.active_hsl_purple_sat ||
                                last_snapshot.hsl_magenta_sat != self.state.active_hsl_magenta_sat ||
                                // HSL Hue
                                last_snapshot.hsl_red_hue != self.state.active_hsl_red_hue ||
                                last_snapshot.hsl_orange_hue != self.state.active_hsl_orange_hue ||
                                last_snapshot.hsl_yellow_hue != self.state.active_hsl_yellow_hue ||
                                last_snapshot.hsl_green_hue != self.state.active_hsl_green_hue ||
                                last_snapshot.hsl_aqua_hue != self.state.active_hsl_aqua_hue ||
                                last_snapshot.hsl_blue_hue != self.state.active_hsl_blue_hue ||
                                last_snapshot.hsl_purple_hue != self.state.active_hsl_purple_hue ||
                                last_snapshot.hsl_magenta_hue != self.state.active_hsl_magenta_hue ||
                                // HSL Lum
                                last_snapshot.hsl_red_lum != self.state.active_hsl_red_lum ||
                                last_snapshot.hsl_orange_lum != self.state.active_hsl_orange_lum ||
                                last_snapshot.hsl_yellow_lum != self.state.active_hsl_yellow_lum ||
                                last_snapshot.hsl_green_lum != self.state.active_hsl_green_lum ||
                                last_snapshot.hsl_aqua_lum != self.state.active_hsl_aqua_lum ||
                                last_snapshot.hsl_blue_lum != self.state.active_hsl_blue_lum ||
                                last_snapshot.hsl_purple_lum != self.state.active_hsl_purple_lum ||
                                last_snapshot.hsl_magenta_lum != self.state.active_hsl_magenta_lum ||
                                // Lens
                                last_snapshot.lens_distortion != self.state.active_lens_distortion ||
                                last_snapshot.lens_vignette_amount != self.state.active_lens_vignette_amount ||
                                last_snapshot.lens_vignette_midpoint != self.state.active_lens_vignette_midpoint ||
                                // NR
                                last_snapshot.nr_luminance != self.state.active_nr_luminance ||
                                last_snapshot.nr_color != self.state.active_nr_color ||
                                // Sharpen
                                last_snapshot.sharpen_amount != self.state.active_sharpen_amount ||
                                last_snapshot.sharpen_radius != self.state.active_sharpen_radius
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
                                    tone_curve_shadows: self.state.active_tone_curve_shadows,
                                    tone_curve_darks: self.state.active_tone_curve_darks,
                                    tone_curve_lights: self.state.active_tone_curve_lights,
                                    tone_curve_highlights: self.state.active_tone_curve_highlights,
                                    // HSL from AppState
                                    hsl_red_sat: self.state.active_hsl_red_sat,
                                    hsl_orange_sat: self.state.active_hsl_orange_sat,
                                    hsl_yellow_sat: self.state.active_hsl_yellow_sat,
                                    hsl_green_sat: self.state.active_hsl_green_sat,
                                    hsl_aqua_sat: self.state.active_hsl_aqua_sat,
                                    hsl_blue_sat: self.state.active_hsl_blue_sat,
                                    hsl_purple_sat: self.state.active_hsl_purple_sat,
                                    hsl_magenta_sat: self.state.active_hsl_magenta_sat,
                                    // HSL Hue
                                    hsl_red_hue: self.state.active_hsl_red_hue,
                                    hsl_orange_hue: self.state.active_hsl_orange_hue,
                                    hsl_yellow_hue: self.state.active_hsl_yellow_hue,
                                    hsl_green_hue: self.state.active_hsl_green_hue,
                                    hsl_aqua_hue: self.state.active_hsl_aqua_hue,
                                    hsl_blue_hue: self.state.active_hsl_blue_hue,
                                    hsl_purple_hue: self.state.active_hsl_purple_hue,
                                    hsl_magenta_hue: self.state.active_hsl_magenta_hue,
                                    // HSL Lum
                                    hsl_red_lum: self.state.active_hsl_red_lum,
                                    hsl_orange_lum: self.state.active_hsl_orange_lum,
                                    hsl_yellow_lum: self.state.active_hsl_yellow_lum,
                                    hsl_green_lum: self.state.active_hsl_green_lum,
                                    hsl_aqua_lum: self.state.active_hsl_aqua_lum,
                                    hsl_blue_lum: self.state.active_hsl_blue_lum,
                                    hsl_purple_lum: self.state.active_hsl_purple_lum,
                                    hsl_magenta_lum: self.state.active_hsl_magenta_lum,
                                    // Lens
                                    lens_distortion: self.state.active_lens_distortion,
                                    lens_vignette_amount: self.state.active_lens_vignette_amount,
                                    lens_vignette_midpoint: self.state.active_lens_vignette_midpoint,
                                    // NR
                                    nr_luminance: self.state.active_nr_luminance,
                                    nr_color: self.state.active_nr_color,
                                    // Sharpening
                                    sharpen_amount: self.state.active_sharpen_amount,
                                    sharpen_radius: self.state.active_sharpen_radius,
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
                        self.state.prev_tone_curve_shadows = self.state.active_tone_curve_shadows;
                        self.state.prev_tone_curve_darks = self.state.active_tone_curve_darks;
                        self.state.prev_tone_curve_lights = self.state.active_tone_curve_lights;
                        self.state.prev_tone_curve_highlights = self.state.active_tone_curve_highlights;
                        // HSL Sat
                        self.state.prev_hsl_red_sat = self.state.active_hsl_red_sat;
                        self.state.prev_hsl_orange_sat = self.state.active_hsl_orange_sat;
                        self.state.prev_hsl_yellow_sat = self.state.active_hsl_yellow_sat;
                        self.state.prev_hsl_green_sat = self.state.active_hsl_green_sat;
                        self.state.prev_hsl_aqua_sat = self.state.active_hsl_aqua_sat;
                        self.state.prev_hsl_blue_sat = self.state.active_hsl_blue_sat;
                        self.state.prev_hsl_purple_sat = self.state.active_hsl_purple_sat;
                        self.state.prev_hsl_magenta_sat = self.state.active_hsl_magenta_sat;
                        // HSL Hue
                        self.state.prev_hsl_red_hue = self.state.active_hsl_red_hue;
                        self.state.prev_hsl_orange_hue = self.state.active_hsl_orange_hue;
                        self.state.prev_hsl_yellow_hue = self.state.active_hsl_yellow_hue;
                        self.state.prev_hsl_green_hue = self.state.active_hsl_green_hue;
                        self.state.prev_hsl_aqua_hue = self.state.active_hsl_aqua_hue;
                        self.state.prev_hsl_blue_hue = self.state.active_hsl_blue_hue;
                        self.state.prev_hsl_purple_hue = self.state.active_hsl_purple_hue;
                        self.state.prev_hsl_magenta_hue = self.state.active_hsl_magenta_hue;
                        // HSL Lum
                        self.state.prev_hsl_red_lum = self.state.active_hsl_red_lum;
                        self.state.prev_hsl_orange_lum = self.state.active_hsl_orange_lum;
                        self.state.prev_hsl_yellow_lum = self.state.active_hsl_yellow_lum;
                        self.state.prev_hsl_green_lum = self.state.active_hsl_green_lum;
                        self.state.prev_hsl_aqua_lum = self.state.active_hsl_aqua_lum;
                        self.state.prev_hsl_blue_lum = self.state.active_hsl_blue_lum;
                        self.state.prev_hsl_purple_lum = self.state.active_hsl_purple_lum;
                        self.state.prev_hsl_magenta_lum = self.state.active_hsl_magenta_lum;
                        // Lens
                        self.state.prev_lens_distortion = self.state.active_lens_distortion;
                        self.state.prev_lens_vignette_amount = self.state.active_lens_vignette_amount;
                        self.state.prev_lens_vignette_midpoint = self.state.active_lens_vignette_midpoint;
                        // NR
                        self.state.prev_nr_luminance = self.state.active_nr_luminance;
                        self.state.prev_nr_color = self.state.active_nr_color;
                        // Sharpen
                        self.state.prev_sharpen_amount = self.state.active_sharpen_amount;
                        self.state.prev_sharpen_radius = self.state.active_sharpen_radius;
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
                        self.state.active_saturation != self.state.saved_saturation ||
                        // Tone Curve
                        self.state.active_tone_curve_shadows != self.state.saved_tone_curve_shadows ||
                        self.state.active_tone_curve_darks != self.state.saved_tone_curve_darks ||
                        self.state.active_tone_curve_lights != self.state.saved_tone_curve_lights ||
                        self.state.active_tone_curve_highlights != self.state.saved_tone_curve_highlights ||
                        // HSL Sat
                        self.state.active_hsl_red_sat != self.state.saved_hsl_red_sat ||
                        self.state.active_hsl_orange_sat != self.state.saved_hsl_orange_sat ||
                        self.state.active_hsl_yellow_sat != self.state.saved_hsl_yellow_sat ||
                        self.state.active_hsl_green_sat != self.state.saved_hsl_green_sat ||
                        self.state.active_hsl_aqua_sat != self.state.saved_hsl_aqua_sat ||
                        self.state.active_hsl_blue_sat != self.state.saved_hsl_blue_sat ||
                        self.state.active_hsl_purple_sat != self.state.saved_hsl_purple_sat ||
                        self.state.active_hsl_magenta_sat != self.state.saved_hsl_magenta_sat ||
                        // HSL Hue
                        self.state.active_hsl_red_hue != self.state.saved_hsl_red_hue ||
                        self.state.active_hsl_orange_hue != self.state.saved_hsl_orange_hue ||
                        self.state.active_hsl_yellow_hue != self.state.saved_hsl_yellow_hue ||
                        self.state.active_hsl_green_hue != self.state.saved_hsl_green_hue ||
                        self.state.active_hsl_aqua_hue != self.state.saved_hsl_aqua_hue ||
                        self.state.active_hsl_blue_hue != self.state.saved_hsl_blue_hue ||
                        self.state.active_hsl_purple_hue != self.state.saved_hsl_purple_hue ||
                        self.state.active_hsl_magenta_hue != self.state.saved_hsl_magenta_hue ||
                        // HSL Lum
                        self.state.active_hsl_red_lum != self.state.saved_hsl_red_lum ||
                        self.state.active_hsl_orange_lum != self.state.saved_hsl_orange_lum ||
                        self.state.active_hsl_yellow_lum != self.state.saved_hsl_yellow_lum ||
                        self.state.active_hsl_green_lum != self.state.saved_hsl_green_lum ||
                        self.state.active_hsl_aqua_lum != self.state.saved_hsl_aqua_lum ||
                        self.state.active_hsl_blue_lum != self.state.saved_hsl_blue_lum ||
                        self.state.active_hsl_purple_lum != self.state.saved_hsl_purple_lum ||
                        self.state.active_hsl_magenta_lum != self.state.saved_hsl_magenta_lum ||
                        // Lens
                        self.state.active_lens_distortion != self.state.saved_lens_distortion ||
                        self.state.active_lens_vignette_amount != self.state.saved_lens_vignette_amount ||
                        self.state.active_lens_vignette_midpoint != self.state.saved_lens_vignette_midpoint ||
                        // NR
                        self.state.active_nr_luminance != self.state.saved_nr_luminance ||
                        self.state.active_nr_color != self.state.saved_nr_color ||
                        // Sharpen
                        self.state.active_sharpen_amount != self.state.saved_sharpen_amount ||
                        self.state.active_sharpen_radius != self.state.saved_sharpen_radius;

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
                            let tone_curve_shadows = self.state.active_tone_curve_shadows;
                            let tone_curve_darks = self.state.active_tone_curve_darks;
                            let tone_curve_lights = self.state.active_tone_curve_lights;
                            let tone_curve_highlights = self.state.active_tone_curve_highlights;
                            // HSL saturation values from state
                            let hsl_red_sat = self.state.active_hsl_red_sat;
                            let hsl_orange_sat = self.state.active_hsl_orange_sat;
                            let hsl_yellow_sat = self.state.active_hsl_yellow_sat;
                            let hsl_green_sat = self.state.active_hsl_green_sat;
                            let hsl_aqua_sat = self.state.active_hsl_aqua_sat;
                            let hsl_blue_sat = self.state.active_hsl_blue_sat;
                            let hsl_purple_sat = self.state.active_hsl_purple_sat;
                            let hsl_magenta_sat = self.state.active_hsl_magenta_sat;
                            // HSL Hue
                            let hsl_red_hue = self.state.active_hsl_red_hue;
                            let hsl_orange_hue = self.state.active_hsl_orange_hue;
                            let hsl_yellow_hue = self.state.active_hsl_yellow_hue;
                            let hsl_green_hue = self.state.active_hsl_green_hue;
                            let hsl_aqua_hue = self.state.active_hsl_aqua_hue;
                            let hsl_blue_hue = self.state.active_hsl_blue_hue;
                            let hsl_purple_hue = self.state.active_hsl_purple_hue;
                            let hsl_magenta_hue = self.state.active_hsl_magenta_hue;
                            // HSL Lum
                            let hsl_red_lum = self.state.active_hsl_red_lum;
                            let hsl_orange_lum = self.state.active_hsl_orange_lum;
                            let hsl_yellow_lum = self.state.active_hsl_yellow_lum;
                            let hsl_green_lum = self.state.active_hsl_green_lum;
                            let hsl_aqua_lum = self.state.active_hsl_aqua_lum;
                            let hsl_blue_lum = self.state.active_hsl_blue_lum;
                            let hsl_purple_lum = self.state.active_hsl_purple_lum;
                            let hsl_magenta_lum = self.state.active_hsl_magenta_lum;
                            // Lens
                            let lens_distortion = self.state.active_lens_distortion;
                            let lens_vignette_amount = self.state.active_lens_vignette_amount;
                            let lens_vignette_midpoint = self.state.active_lens_vignette_midpoint;

                            // Noise Reduction from state
                            let nr_luminance = self.state.active_nr_luminance;
                            let nr_color = self.state.active_nr_color;
                            // Sharpening from state
                            let sharpen_amount = self.state.active_sharpen_amount;
                            let sharpen_radius = self.state.active_sharpen_radius;

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
                            // Update saved HSL values
                            self.state.saved_hsl_red_sat = hsl_red_sat;
                            self.state.saved_hsl_orange_sat = hsl_orange_sat;
                            self.state.saved_hsl_yellow_sat = hsl_yellow_sat;
                            self.state.saved_hsl_green_sat = hsl_green_sat;
                            self.state.saved_hsl_aqua_sat = hsl_aqua_sat;
                            self.state.saved_hsl_blue_sat = hsl_blue_sat;
                            self.state.saved_hsl_purple_sat = hsl_purple_sat;
                            self.state.saved_hsl_magenta_sat = hsl_magenta_sat;

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
                                photo.edit_tone_curve_shadows = Some(tone_curve_shadows);
                                photo.edit_tone_curve_darks = Some(tone_curve_darks);
                                photo.edit_tone_curve_lights = Some(tone_curve_lights);
                                photo.edit_tone_curve_highlights = Some(tone_curve_highlights);
                                // HSL Sat
                                photo.edit_hsl_red_sat = Some(hsl_red_sat);
                                photo.edit_hsl_orange_sat = Some(hsl_orange_sat);
                                photo.edit_hsl_yellow_sat = Some(hsl_yellow_sat);
                                photo.edit_hsl_green_sat = Some(hsl_green_sat);
                                photo.edit_hsl_aqua_sat = Some(hsl_aqua_sat);
                                photo.edit_hsl_blue_sat = Some(hsl_blue_sat);
                                photo.edit_hsl_purple_sat = Some(hsl_purple_sat);
                                photo.edit_hsl_magenta_sat = Some(hsl_magenta_sat);
                                // HSL Hue
                                photo.edit_hsl_red_hue = Some(hsl_red_hue);
                                photo.edit_hsl_orange_hue = Some(hsl_orange_hue);
                                photo.edit_hsl_yellow_hue = Some(hsl_yellow_hue);
                                photo.edit_hsl_green_hue = Some(hsl_green_hue);
                                photo.edit_hsl_aqua_hue = Some(hsl_aqua_hue);
                                photo.edit_hsl_blue_hue = Some(hsl_blue_hue);
                                photo.edit_hsl_purple_hue = Some(hsl_purple_hue);
                                photo.edit_hsl_magenta_hue = Some(hsl_magenta_hue);
                                // HSL Lum
                                photo.edit_hsl_red_lum = Some(hsl_red_lum);
                                photo.edit_hsl_orange_lum = Some(hsl_orange_lum);
                                photo.edit_hsl_yellow_lum = Some(hsl_yellow_lum);
                                photo.edit_hsl_green_lum = Some(hsl_green_lum);
                                photo.edit_hsl_aqua_lum = Some(hsl_aqua_lum);
                                photo.edit_hsl_blue_lum = Some(hsl_blue_lum);
                                photo.edit_hsl_purple_lum = Some(hsl_purple_lum);
                                photo.edit_hsl_magenta_lum = Some(hsl_magenta_lum);
                                // Lens
                                photo.edit_lens_distortion = Some(lens_distortion);
                                photo.edit_lens_vignette_amount = Some(lens_vignette_amount);
                                photo.edit_lens_vignette_midpoint = Some(lens_vignette_midpoint);
                                // NR
                                photo.edit_nr_luminance = Some(nr_luminance);
                                photo.edit_nr_color = Some(nr_color);
                                // Sharpen
                                photo.edit_sharpen_amount = Some(sharpen_amount);
                                photo.edit_sharpen_radius = Some(sharpen_radius);
                            }

                            // Invalidate cached thumbnails to force regeneration with updated effects
                            self.photo_grid.invalidate_thumbnail(&id);
                            self.filmstrip.invalidate_thumbnail(&id);

                            // Clear pending flag
                            self.state.pending_auto_save = false;
                            self.state.last_slider_change_time = None;

                            let ctx_clone = ctx.clone();

                            tokio::spawn(async move {
                                if let Err(e) = controller.save_edits(
                                    id, exposure, contrast, temperature, tint, highlights, shadows,
                                    whites, blacks, clarity, vibrance, saturation,
                                    tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
                                    hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
                                    hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
                                    // HSL Hue
                                    hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue,
                                    hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
                                    // HSL Lum
                                    hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum,
                                    hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
                                    // Lens
                                    lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
                                    // NR
                                    nr_luminance, nr_color,
                                    // Sharpen
                                    sharpen_amount, sharpen_radius
                                ).await {
                                    // Note: Toast will be shown in the next frame via state
                                    eprintln!("Auto-save failed: {}", e);
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

        // ============================================
        // KEYBOARD SHORTCUTS (Library View)
        // ============================================
        // Keyboard shortcuts are now handled globally by keyboard_handler (line 225)


        // ============================================
        // ADVANCED IMPORT DIALOGS
        // ============================================

        // Import Preview Dialog
        if let Some(dialog) = &mut self.state.import_preview_dialog {
            if let Some(action) = dialog.show(ctx) {
                use crate::components::import_dialogs::ImportDialogAction;
                match action {
                    ImportDialogAction::Import => {
                        // Get selected files and options
                        let files = dialog.get_selected_files();
                        let options = dialog.get_options();

                        // Start import with progress dialog
                        let (progress_sender, progress_receiver) = tokio::sync::mpsc::unbounded_channel();
                        let pause_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

                        use crate::components::import_dialogs::ImportProgressDialog;
                        self.state.import_progress_dialog = Some(ImportProgressDialog::new(
                            files.len(),
                            pause_flag.clone(),
                            cancel_flag.clone(),
                            progress_receiver,
                        ));

                        // Spawn import task
                        let import_controller = self.import_controller.clone();
                        let library_controller = self.library_controller.clone();
                        let photo_sender = self.photo_sender.clone();
                        let ctx_clone = ctx.clone();

                        tokio::spawn(async move {
                            // Run import
                            let _ = import_controller.import_with_options(
                                files,
                                options,
                                progress_sender,
                                pause_flag,
                                cancel_flag,
                            ).await;

                            // Reload library
                            match library_controller.get_all_photos().await {
                                Ok(photos) => {
                                    let _ = photo_sender.send(Ok(photos)).await;
                                }
                                Err(e) => {
                                    eprintln!("Failed to reload photos: {}", e);
                                }
                            }
                            ctx_clone.request_repaint();
                        });
                    }
                    ImportDialogAction::Cancel => {
                        // User canceled, close dialog
                        self.state.import_preview_dialog = None;
                    }
                }
            }
        }

        // Import Progress Dialog
        if let Some(dialog) = &mut self.state.import_progress_dialog {
            let is_open = dialog.show(ctx);
            if !is_open {
                // Dialog closed, cleanup
                self.state.import_progress_dialog = None;
                self.state.import_preview_dialog = None;
            }
        }

        // ============================================
        // DELETE CONFIRMATION DIALOG
        // ============================================
        if self.state.show_delete_confirmation {
            let count = self.state.selection_count();
            egui::Window::new("Delete Photos")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new("⚠️").size(32.0));
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "Are you sure you want to delete {} photo{}?",
                                count,
                                if count == 1 { "" } else { "s" }
                            ))
                            .size(16.0)
                        );
                        ui.add_space(15.0);
                        
                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                self.state.show_delete_confirmation = false;
                            }
                            ui.add_space(20.0);
                            if ui.button(egui::RichText::new("Delete").color(egui::Color32::from_rgb(255, 100, 100))).clicked() {
                                // Perform deletion
                                let ids_to_delete: Vec<String> = self.state.selected_photo_ids.iter().cloned().collect();
                                let photo_controller = self.photo_controller.clone();
                                let library_controller = self.library_controller.clone();
                                let photo_sender = self.photo_sender.clone();
                                let ctx_clone = ctx.clone();
                                
                                let deleted_count = ids_to_delete.len();
                                tokio::spawn(async move {
                                    for id in &ids_to_delete {
                                        let _ = photo_controller.delete_photo(id).await;
                                    }
                                    // Reload photos list
                                    let _ = match library_controller.get_all_photos().await {
                                        Ok(photos) => photo_sender.send(Ok(photos)).await,
                                        Err(e) => photo_sender.send(Err(e)).await,
                                    };
                                    ctx_clone.request_repaint();
                                });

                                self.state.toasts.info(format!("Deleting {} photo{}...", deleted_count, if deleted_count == 1 { "" } else { "s" }));
                                self.state.clear_selection();
                                self.state.library_selected_photo_id = None;
                                self.state.show_delete_confirmation = false;
                            }
                        });
                        ui.add_space(10.0);
                    });
                });
        }

        // Show import dialog if active
        self.render_import_dialog(ctx);

        // Show toast notifications
        self.state.toasts.show(ctx);

        // Show Settings Dialog
        if let Some(action) = SettingsDialog::show(ctx, &mut self.state) {
            match action {
                SettingsAction::ClearThumbnails => {
                    if let Ok(count) = self.preview_manager.clear_thumbnails() {
                        self.state.toasts.success(format!("Cleared {} thumbnails", count));
                        // Refresh stats
                        self.state.cache_stats = Some(self.preview_manager.get_stats());
                    } else {
                        self.state.toasts.error("Failed to clear thumbnails");
                    }
                }
                SettingsAction::ClearPreviews => {
                    if let Ok(count) = self.preview_manager.clear_previews() {
                        self.state.toasts.success(format!("Cleared {} previews", count));
                        self.state.cache_stats = Some(self.preview_manager.get_stats());
                    } else {
                        self.state.toasts.error("Failed to clear previews");
                    }
                }
                SettingsAction::ClearAllCache => {
                    if let Ok(count) = self.preview_manager.clear_all() {
                        self.state.toasts.success(format!("Cache cleared completely ({} items)", count));
                        self.state.cache_stats = Some(self.preview_manager.get_stats());
                    } else {
                        self.state.toasts.error("Failed to clear cache");
                    }
                }
                SettingsAction::ResetDockingLayout => {
                    self.library_dock_state = crate::docking::create_library_layout();
                    self.develop_dock_state = crate::docking::create_develop_layout();
                    self.state.toasts.success("Layout reset to default");
                }
            }
        }

        // Save Preset Dialog
        if self.state.show_save_preset_dialog {
            egui::Window::new("Save Preset")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.label("Enter a name for your preset:");
                        ui.add_space(5.0);
                        
                        let response = ui.text_edit_singleline(&mut self.state.save_preset_name);
                        
                        // Auto-focus the text field
                        if self.state.show_save_preset_dialog {
                            response.request_focus();
                        }
                        
                        ui.add_space(15.0);
                        
                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                self.state.show_save_preset_dialog = false;
                                self.state.save_preset_name.clear();
                            }
                            ui.add_space(20.0);
                            
                            let can_save = !self.state.save_preset_name.trim().is_empty();
                            if ui.add_enabled(can_save, egui::Button::new("Save")).clicked() {
                                // Create adjustments from current state
                                let adjustments = domain::entities::preset::PresetAdjustments {
                                    exposure: Some(self.state.active_exposure),
                                    contrast: Some(self.state.active_contrast),
                                    temperature: Some(self.state.active_temperature),
                                    tint: Some(self.state.active_tint),
                                    highlights: Some(self.state.active_highlights),
                                    shadows: Some(self.state.active_shadows),
                                    whites: Some(self.state.active_whites),
                                    blacks: Some(self.state.active_blacks),
                                    clarity: Some(self.state.active_clarity),
                                    vibrance: Some(self.state.active_vibrance),
                                    saturation: Some(self.state.active_saturation),
                                    tone_curve_shadows: Some(self.state.active_tone_curve_shadows),
                                    tone_curve_darks: Some(self.state.active_tone_curve_darks),
                                    tone_curve_lights: Some(self.state.active_tone_curve_lights),
                                    tone_curve_highlights: Some(self.state.active_tone_curve_highlights),
                                };
                                
                                let name = self.state.save_preset_name.trim().to_string();
                                let controller = self.preset_controller.clone();
                                let preset_sender = self.preset_sender.clone();
                                let ctx_clone = ctx.clone();
                                
                                tokio::spawn(async move {
                                    match controller.save_preset(name.clone(), adjustments).await {
                                        Ok(_) => {
                                            // Reload presets
                                            if let Ok(presets) = controller.list_presets().await {
                                                let _ = preset_sender.send(Ok(presets)).await;
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to save preset: {}", e);
                                        }
                                    }
                                    ctx_clone.request_repaint();
                                });
                                
                                self.state.toasts.success(format!("Preset '{}' saved!", self.state.save_preset_name.trim()));
                                self.state.show_save_preset_dialog = false;
                                self.state.save_preset_name.clear();
                            }
                        });
                        ui.add_space(10.0);
                    });
                });
        }

        // Top toolbar
        egui::TopBottomPanel::top("toolbar")

            .exact_height(Theme::TOOLBAR_HEIGHT)
            .show(ctx, |ui| {
                self.show_toolbar(ui);
            });

        // Main content area - Docking UI
        if self.state.current_view == CurrentView::Import {
            crate::views::import_view::ImportView::show(
                ctx, 
                &mut self.state, 
                &self.import_controller, 
                &self.import_source_sender
            );
        } else {
            egui::CentralPanel::default().show(ctx, |_ui| {
                // Get the appropriate dock state for the current view
                let dock_state = match self.state.current_view {
                    CurrentView::Library => &mut self.library_dock_state,
                    CurrentView::Develop => &mut self.develop_dock_state,
                    CurrentView::Import => &mut self.library_dock_state,
                };
            
            // Create dock viewer context with all required references
            let context = DockViewerContext {
                state: &mut self.state,
                library_controller: &self.library_controller,
                editor_controller: &self.editor_controller,
                export_controller: &self.export_controller,
                photo_controller: &self.photo_controller,
                preview_manager: &self.preview_manager,
                photo_sender: &self.photo_sender,
                ctx,
                photo_grid: &mut self.photo_grid,
                filmstrip: &mut self.filmstrip,
            };
            
            // Render the dock area with all tabs
            let mut dock_viewer = DockViewer::new(context);
            DockArea::new(dock_state)
                .show(ctx, &mut dock_viewer);
            });
        }

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
        use crate::design_system::icons;
        
        ui.horizontal(|ui| {
            ui.add_space(Theme::SPACE_LG);

            // App title
            ui.label(
                egui::RichText::new("VintageLightbox")
                    .size(Theme::FONT_LG)
                    .color(Theme::TEXT_PRIMARY)
            );

            ui.add_space(Theme::SPACE_XXL);

            // Theme selector
            use crate::design_system::theme_selector::ThemeVariant;
            let ctx_clone = ui.ctx().clone();
            egui::ComboBox::from_id_salt("theme_selector")
                .selected_text(self.state.selected_theme.display_name())
                .show_ui(ui, |ui| {
                    for variant in ThemeVariant::all() {
                        if ui.selectable_value(
                            &mut self.state.selected_theme,
                            variant,
                            variant.display_name()
                        ).clicked() {
                            variant.apply_to_context(&ctx_clone);
                            self.state.toasts.info(format!("Theme changed to {}", variant.display_name()));
                        }
                    }
                });

            ui.add_space(Theme::SPACE_LG);

            // View tabs with icons
            let library_label = format!("{} Library", icons::NAV_LIBRARY);
            if widgets::nav_button(ui, &library_label, self.state.current_view == CurrentView::Library).clicked() {
                self.state.current_view = CurrentView::Library;
                self.state.reset_viewer();
            }

            ui.add_space(Theme::SPACE_SM);

            // Develop button enabled if Library has a selection
            let develop_label = format!("{} Develop", icons::NAV_DEVELOP);
            let develop_enabled = self.state.library_selected_photo_id.is_some();
            if develop_enabled {
                if widgets::nav_button(ui, &develop_label, self.state.current_view == CurrentView::Develop).clicked() {
                    // Copy Library selection to Develop when entering Develop mode
                    if self.state.develop_selected_photo_id.is_none() {
                        self.state.develop_selected_photo_id = self.state.library_selected_photo_id.clone();
                        self.state.loaded_photo_id = None; // Force image load
                    }
                    self.state.current_view = CurrentView::Develop;
                }
            } else {
                ui.add_enabled_ui(false, |ui| {
                    widgets::nav_button(ui, &develop_label, false);
                });
            }

            // Spacer to push photo count and import button to the right
            ui.allocate_space(egui::vec2(ui.available_width() - 280.0, 0.0));

            // Photo count with icon
            let photo_count = self.state.photos.len();
            ui.label(
                egui::RichText::new(format!("{} {} photos", icons::FILE_IMAGE, photo_count))
                    .size(Theme::FONT_MD)
                    .color(Theme::TEXT_SECONDARY)
            );

            ui.add_space(Theme::SPACE_LG);

            // Import button with icon
            let import_label = format!("{} Import", icons::ACTION_IMPORT);
            if widgets::primary_button(ui, &import_label).clicked() {
                self.handle_import(ui.ctx());
            }

            ui.add_space(Theme::SPACE_SM);

            // Advanced Import icon button with tooltip
            if widgets::icon_button_tooltip(ui, icons::ACTION_SETTINGS, "Advanced Import").clicked() {
                self.handle_advanced_import(ui.ctx());
            }

            ui.add_space(Theme::SPACE_SM);

            // Cache building progress indicator
            crate::components::settings_dialog::show_cache_progress(ui, &self.state);

            // Settings button
            if widgets::icon_button_tooltip(ui, icons::ACTION_SETTINGS, "Settings").clicked() {
                // Refresh cache stats when opening
                self.state.cache_stats = Some(self.preview_manager.get_stats());
                self.state.show_settings_dialog = true;
            }

            ui.add_space(Theme::SPACE_LG);
        });
    }

    /// Handle import button click
    fn handle_import(&mut self, _ctx: &egui::Context) {
        self.state.current_view = crate::state::CurrentView::Import;
        
        // Trigger loading devices
        let _controller = self.import_controller.clone();
        // Since we don't have a direct way to mutate state from here async easily without Arc<Mutex<AppState>> which we don't have,
        // we might need a channel or just rely on ImportView to load on mount/poll.
        // For now, let's assume ImportView handles loading logic or we implement a "LoadSources" action.
    }

    /// Handle advanced import button click
    fn handle_advanced_import(&mut self, _ctx: &egui::Context) {
        let mut dialog = egui_file::FileDialog::open_file(None)
            .title("Select Photos for Advanced Import");

        dialog.open();
        // Reuse import_dialog state - this means regular import and advanced import share the same dialog slot,
        // but render_import_dialog needs to know which action to take.
        // For simplicity in this migration, I'll modify render_import_dialog to handle a flag or check context.
        // Actually, simpler: I'll add a 'dialog_mode' to AppState.
        self.state.import_dialog = Some(dialog);
        self.state.import_dialog_mode = crate::state::ImportDialogMode::Advanced;
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

    /// Render import dialog and handle file selection
    fn render_import_dialog(&mut self, ctx: &egui::Context) {
        let mut open = false;
        
        if let Some(dialog) = &mut self.state.import_dialog {
            if dialog.show(ctx).selected() {
                if let Some(path) = dialog.path() {
                    let path = path.to_path_buf();
                    let path_str = path.to_string_lossy().to_string();
                    
                    match self.state.import_dialog_mode {
                        crate::state::ImportDialogMode::Simple => {
                             // Simple Import
                            let import_controller = self.import_controller.clone();
                            let library_controller = self.library_controller.clone();
                            let sender = self.photo_sender.clone();
                            let ctx_clone = ctx.clone();
                            
                            self.state.is_busy = true;
                            self.state.busy_message = "Importing photo...".to_string();
                            
                            tokio::spawn(async move {
                                 let result = import_controller.import_files(vec![path_str]).await;
                                 let _ = library_controller.get_all_photos().await;
                                 let _ = sender.send(match result {
                                     Ok(_) => Ok(vec![]),
                                     Err(e) => Err(e),
                                 }).await;
                                 ctx_clone.request_repaint();
                            });
                        },
                        crate::state::ImportDialogMode::Advanced => {
                            // Advanced Import
                            let import_controller = self.import_controller.clone();
                            let ctx_clone = ctx.clone();
                            
                            // Create a channel
                            let (dialog_tx, dialog_rx) = tokio::sync::mpsc::channel::<Option<crate::components::import_dialogs::ImportPreviewDialog>>(1);
                            self.state.pending_import_preview_receiver = Some(dialog_rx); // We need this polling in update(), make sure it's there
                            // Wait, render_import_dialog is CALLED from update. poll_import_preview_dialog is likely missing?
                            // I should verify update() loop has checking for pending_import_preview_receiver.
                            
                            self.state.is_busy = true;
                            self.state.busy_message = "Loading preview...".to_string();
                            
                            tokio::spawn(async move {
                                 // We only get one file from egui_file unless we handle dirs? egui_file supports dirs but we picked file.
                                 // Assuming user selected a file.
                                 let paths = vec![path_str];
                                 
                                 let preview_future = import_controller.preview_import(paths.clone());
                                 let duplicates_future = import_controller.check_duplicates(paths.clone());

                                match tokio::try_join!(preview_future, duplicates_future) {
                                    Ok((previews, duplicates)) => {
                                        let duplicate_flags: Vec<bool> = previews.iter()
                                            .map(|preview| {
                                                duplicates.iter().any(|d| {
                                                    d.file_path == preview.file_path && d.is_duplicate
                                                })
                                            })
                                            .collect();

                                        use crate::components::import_dialogs::ImportPreviewDialog;
                                        let mut dialog = ImportPreviewDialog::new();
                                        dialog.set_items(previews, duplicate_flags);
                                        let _ = dialog_tx.send(Some(dialog)).await;
                                    }
                                    Err(_) => { let _ = dialog_tx.send(None).await; }
                                }
                                ctx_clone.request_repaint();
                            });
                        },
                         crate::state::ImportDialogMode::Export => {
                            if let Some(id) = self.state.export_target_id.clone() {
                                let controller = self.export_controller.clone();
                                let ctx_clone = ctx.clone();
                                // We need to wire up the existing pending_export_receiver handling logic if we want to show toasts/status
                                // or just handle it here. App.rs already has logic to check pending_export_receiver?
                                // Let's check app.rs... no, wait, DockViewer was creating the receiver specifically.
                                // App.rs has `self.state.pending_export_receiver` and `pending_export`.
                                // So we should use that mechanism.
                                
                                let (export_tx, export_rx) = tokio::sync::mpsc::channel::<Result<String, String>>(1);
                                self.state.pending_export_receiver = Some(export_rx);
                                self.state.is_busy = true;
                                self.state.busy_message = "Exporting...".to_string();
                                self.state.toasts.info("Exporting photo...");
                                
                                tokio::spawn(async move {
                                     match controller.export_photo(id, path_str.clone()).await {
                                         Ok(_) => { let _ = export_tx.send(Ok(path_str)).await; }
                                         Err(e) => { let _ = export_tx.send(Err(e)).await; }
                                     }
                                     ctx_clone.request_repaint();
                                });
                            }
                         }
                    }
                }
            }
            open = dialog.state() == egui_file::State::Open;
        }
        
        if !open && self.state.import_dialog.is_some() {
             self.state.import_dialog = None;
        }
    }
}
