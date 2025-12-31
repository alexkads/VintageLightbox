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
use crate::components::secondary_window::SecondaryWindow;
use crate::monitors::{MonitorDetector, MonitorInfo};

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
    // Services (from adapters layer)
    // ============================================
    editor_service: adapters::services::EditorService,

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
    /// GPU-accelerated intelligent fill processor for rotation edges
    #[allow(dead_code)] // TODO: Integrar com image_viewer quando fill_mode == Intelligent
    intelligent_fill_processor: crate::intelligent_fill::IntelligentFillProcessor,
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
    /// Print view component
    print_view: crate::views::print_view::PrintView,
    
    /// Flag to trigger initial photo load on first frame
    needs_initial_load: bool,
    
    // ============================================
    // Secondary Window (Multi-Monitor)
    // ============================================
    /// Secondary window for client view on second monitor
    secondary_window: SecondaryWindow,
    /// Cached list of available monitors
    cached_monitors: Vec<MonitorInfo>,
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
            editor_service: adapters::services::EditorService::new(),
            keyboard_handler: KeyboardHandler::new(),
            photo_receiver,
            photo_sender,
            preset_receiver,
            preset_sender,
            import_source_sender,
            import_source_receiver,
            image_processor: AsyncImageProcessor::new(preview_manager.clone()),
            gpu_edit_processor: crate::gpu_processor::GpuImageProcessor::new(),
            intelligent_fill_processor: crate::intelligent_fill::IntelligentFillProcessor::new(),
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
            print_view: crate::views::print_view::PrintView::new(preview_manager.clone()),
            preview_manager,
            needs_initial_load: true,
            secondary_window: SecondaryWindow::new(),
            cached_monitors: Vec::new(),
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
        // Process thumbnail invalidation requests
        let invalidated_ids: Vec<_> = self.state.invalidation_queue.drain().collect();
        for id in invalidated_ids {
            self.filmstrip.invalidate_thumbnail(&id);
            self.photo_grid.invalidate_thumbnail(&id);
        }

        // Trigger initial photo and preset loading on first frame
        if self.needs_initial_load {
            self.needs_initial_load = false;
            self.load_photos(ctx);
            self.load_presets(ctx);
        }

        // Handle deferred exit from Develop mode (triggered by Escape key)
        // This ensures edits are saved before switching to Library
        if self.state.deferred_exit_develop_mode {
            self.state.deferred_exit_develop_mode = false;
            self.save_pending_develop_edits();
            self.state.internal_state.current_view = CurrentView::Library;
            self.state.reset_viewer();
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
                        // Open the folder containing the exported file using the controller (Adapter layer)
                        // This removes the UI layer's direct dependency on filesystem/OS operations
                        let controller = self.export_controller.clone();
                        let path_clone = path.clone();
                        tokio::spawn(async move {
                             if let Err(e) = controller.open_export_location(path_clone).await {
                                 eprintln!("Failed to open folder: {}", e);
                             }
                        });
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
        self.keyboard_handler.handle_input(
            ctx,
            &mut self.state,
            &self.photo_controller,
            &self.library_controller,
            &self.editor_controller,
            &self.export_controller,
            &self.import_controller,
            &self.photo_sender,
            &mut self.editor_service
        );

        // ============================================
        // ASYNC IMAGE LOADING (Non-blocking)
        // ============================================
        
        // Poll for completed image processing results
        if let Some(infra_result) = self.image_processor.poll_result() {
            // Convert infrastructure result to UI result with ColorImage
            let result = crate::async_loader::convert_image_result(infra_result);
            
            // Check if this is still the photo we want
            // Accept if matches develop selection OR (secondary window open AND matches library selection)
            let is_target = self.state.internal_state.develop_selected_id.as_ref() == Some(&result.photo_id) ||
                           (self.secondary_window.is_open && self.state.internal_state.library_selected_id.as_ref() == Some(&result.photo_id));

            if is_target {
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
                // Clear thumbnail to avoid micro-differences between pixel crop (thumb) and UV crop (full-res)
                self.state.thumbnail_preview = None;
                self.state.loaded_photo_id = Some(result.photo_id);
                
                // Force repaint to show the loaded image immediately
                ctx.request_repaint();
            }
        }

        // Poll for completed GPU edit processing results
        if let Some(result) = self.gpu_edit_processor.poll_result() {
            // Only apply if this is the latest request
            if result.request_id >= self.current_edit_request_id.saturating_sub(5) {
                if let Some(photo_id) = &self.state.internal_state.develop_selected_id.clone() {
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

        // Poll for completed Intelligent Fill processing results
        if let Some(result) = self.intelligent_fill_processor.poll_result() {
            eprintln!("[IntelligentFill] Got result! request_id={}, expected={}, time={}ms",
                result.request_id, self.state.intelligent_fill_request_id, result.process_time_ms);
            if result.request_id == self.state.intelligent_fill_request_id {
                // Create texture from the filled image
                let texture = ctx.load_texture(
                    "intelligent_fill_result",
                    result.preview,
                    egui::TextureOptions::default()
                );
                self.state.intelligent_fill_texture = Some(texture);
                self.state.intelligent_fill_pending = false;
                eprintln!("[IntelligentFill] Texture created successfully!");
                ctx.request_repaint();
            }
        }

        // Request Intelligent Fill if needed
        self.request_intelligent_fill_if_needed();

        // Request image loading if needed (non-blocking)
        // Check if we need to load a new photo
        // Trigger if:
        // 1. We are in Develop View AND have a selected photo
        // 2. OR we are in Library View AND Secondary Window is open (needs high-res)
        let target_photo_id = if self.state.internal_state.current_view == crate::state::CurrentView::Develop {
            self.state.internal_state.develop_selected_id.clone()
        } else if self.secondary_window.is_open {
             // Bridge: If secondary window is open, use library selection to drive image loading
             self.state.internal_state.library_selected_id.clone()
        } else {
            None
        };

        if let Some(photo_id) = &target_photo_id {
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
                // Reset intelligent fill state for new photo
                self.state.intelligent_fill_texture = None;
                self.state.prev_intelligent_fill_angle = f32::NAN; // Force reprocessing by using NaN

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

                    // Initialize crop settings
                    let crop_settings = if let (Some(x), Some(y), Some(w), Some(h)) = (photo.edit_crop_x, photo.edit_crop_y, photo.edit_crop_width, photo.edit_crop_height) {
                        let fill_mode = domain::value_objects::RotationFillMode::try_from(photo.edit_crop_fill_mode.unwrap_or(0))
                            .unwrap_or_default();
                         Some(domain::value_objects::CropSettings::with_fill_mode_value(
                             x, y, w, h,
                             photo.edit_crop_rotation.unwrap_or(0),
                             photo.edit_crop_angle.unwrap_or(0.0),
                             photo.edit_crop_flip_h.unwrap_or(false),
                             photo.edit_crop_flip_v.unwrap_or(false),
                             fill_mode
                         ))
                    } else {
                        None
                    };
                    self.state.crop_settings = crop_settings.clone();

                    let initial_edits = domain::value_objects::PhotoEdits {
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
                        
                        hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
                        hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
                        
                        hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue,
                        hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
                        
                        hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum,
                        hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
                        
                        lens_distortion,
                        lens_vignette_amount,
                        lens_vignette_midpoint,
                        
                        nr_luminance,
                        nr_color,
                        
                        sharpen_amount,
                        sharpen_radius,
                        
                        crop_settings,
                    };
                    
                    // Start editing session in EditorService (manages history, undo/redo state)
                    self.editor_service.start_editing(photo.id.clone(), initial_edits.clone());
                    
                    // Initialize change detection state
                    self.state.last_processed_edits = initial_edits.clone();
                    self.state.last_saved_edits = initial_edits;



                    // Request async image loading (non-blocking!)
                    // Use EditorService to get current edits
                    let current_edits = self.editor_service.current_edits();
                    
                    self.image_processor.request_process(ImageProcessRequest {
                        photo_id: photo_id.clone(),
                        path: photo.path.clone(),
                        max_preview_size: 2560,
                        edits: current_edits,
                    });

                    // TODO: Prefetch adjacent photos into L1 cache for faster navigation
                    // This functionality was removed during UI simplification
                    // and can be reimplemented in infrastructure if needed
                    /*
                    // Find current photo index and prefetch previous/next
                    if let Some(current_idx) = self.state.photos.iter().position(|p| &p.id == photo_id) {
                        // Prefetch previous photo
                        if current_idx > 0 {
                            if let Some(prev_photo) = self.state.photos.get(current_idx - 1) {
                                self.image_processor.prefetch(
                                    prev_photo.id.clone(),
                                    prev_photo.path.clone(),
                                    2560,
                                );
                            }
                        }
                        // Prefetch next photo
                        if let Some(next_photo) = self.state.photos.get(current_idx + 1) {
                            self.image_processor.prefetch(
                                next_photo.id.clone(),
                                next_photo.path.clone(),
                                2560,
                            );
                        }
                    }
                    */

                    // Start timing TTI
                    self.state.start_load_time = Some(std::time::Instant::now());

                    // Request repaint to poll for results
                    ctx.request_repaint();
                }
            } else if !needs_reload {
                // Photo is loaded, check for edit changes
                // Use EditorService for efficient change detection
                let current_edits = self.editor_service.current_edits();
                let edits_changed = current_edits != self.state.last_processed_edits;
                let before_toggled = self.state.show_before != self.state.prev_show_before;

                if (edits_changed || before_toggled) && self.state.original_preview.is_some() {
                    // Save to history if needed
                    // History management is handled by EditorService


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
                                params: {
                                    let mut edits = self.editor_service.current_edits();
                                    edits.crop_settings = None;
                                    edits
                                },
                            });

                            // Request repaint to poll for results
                            ctx.request_repaint();
                        }
                    }

                    // Update last processed edits for change detection
                    if !self.state.show_before {
                        self.state.last_processed_edits = current_edits;
                    }
                    self.state.prev_show_before = self.state.show_before;
                }
            }
        }

        // ============================================
        // CROP APPLY (imediato, não debounced)
        // ============================================
        if self.state.pending_crop_apply {
            self.state.pending_crop_apply = false;
            // Mark for immediate save
            self.state.pending_auto_save = true;
            self.state.last_slider_change_time = Some(std::time::Instant::now() - std::time::Duration::from_millis(1000));
        }

        // ============================================
        // AUTO-SAVE DEBOUNCE (500ms delay)
        // ============================================
        const AUTO_SAVE_DEBOUNCE_MS: u128 = 500;
        
        if self.state.pending_auto_save {
            if let Some(last_change) = self.state.last_slider_change_time {
                let elapsed = last_change.elapsed().as_millis();
                if elapsed >= AUTO_SAVE_DEBOUNCE_MS {
                    // Check if values actually changed from last saved state using PhotoEdits
                    let current_edits = self.editor_service.current_edits();
                    let values_changed = current_edits != self.state.last_saved_edits;

                    if values_changed {
                        if let Some(metadata) = &self.state.detail_metadata {
                            let controller = self.editor_controller.clone();
                            let id = metadata.id.clone();
                            // Use cached current_edits directly


                            // Update saved edits for auto-save comparison
                            self.state.last_saved_edits = current_edits.clone();


                            // Update the PhotoViewModel in the local list to reflect saved edits
                            // This ensures the photo loads with correct values when switching photos
                            if let Some(photo) = self.state.photos.iter_mut().find(|p| p.id == id) {
                                photo.edit_exposure = Some(current_edits.exposure);
                                photo.edit_contrast = Some(current_edits.contrast);
                                photo.edit_temperature = Some(current_edits.temperature);
                                photo.edit_tint = Some(current_edits.tint);
                                photo.edit_highlights = Some(current_edits.highlights);
                                photo.edit_shadows = Some(current_edits.shadows);
                                photo.edit_whites = Some(current_edits.whites);
                                photo.edit_blacks = Some(current_edits.blacks);
                                photo.edit_clarity = Some(current_edits.clarity);
                                photo.edit_vibrance = Some(current_edits.vibrance);
                                photo.edit_saturation = Some(current_edits.saturation);
                                photo.edit_tone_curve_shadows = Some(current_edits.tone_curve_shadows);
                                photo.edit_tone_curve_darks = Some(current_edits.tone_curve_darks);
                                photo.edit_tone_curve_lights = Some(current_edits.tone_curve_lights);
                                photo.edit_tone_curve_highlights = Some(current_edits.tone_curve_highlights);
                                // HSL Sat
                                photo.edit_hsl_red_sat = Some(current_edits.hsl_red_sat);
                                photo.edit_hsl_orange_sat = Some(current_edits.hsl_orange_sat);
                                photo.edit_hsl_yellow_sat = Some(current_edits.hsl_yellow_sat);
                                photo.edit_hsl_green_sat = Some(current_edits.hsl_green_sat);
                                photo.edit_hsl_aqua_sat = Some(current_edits.hsl_aqua_sat);
                                photo.edit_hsl_blue_sat = Some(current_edits.hsl_blue_sat);
                                photo.edit_hsl_purple_sat = Some(current_edits.hsl_purple_sat);
                                photo.edit_hsl_magenta_sat = Some(current_edits.hsl_magenta_sat);
                                // HSL Hue
                                photo.edit_hsl_red_hue = Some(current_edits.hsl_red_hue);
                                photo.edit_hsl_orange_hue = Some(current_edits.hsl_orange_hue);
                                photo.edit_hsl_yellow_hue = Some(current_edits.hsl_yellow_hue);
                                photo.edit_hsl_green_hue = Some(current_edits.hsl_green_hue);
                                photo.edit_hsl_aqua_hue = Some(current_edits.hsl_aqua_hue);
                                photo.edit_hsl_blue_hue = Some(current_edits.hsl_blue_hue);
                                photo.edit_hsl_purple_hue = Some(current_edits.hsl_purple_hue);
                                photo.edit_hsl_magenta_hue = Some(current_edits.hsl_magenta_hue);
                                // HSL Lum
                                photo.edit_hsl_red_lum = Some(current_edits.hsl_red_lum);
                                photo.edit_hsl_orange_lum = Some(current_edits.hsl_orange_lum);
                                photo.edit_hsl_yellow_lum = Some(current_edits.hsl_yellow_lum);
                                photo.edit_hsl_green_lum = Some(current_edits.hsl_green_lum);
                                photo.edit_hsl_aqua_lum = Some(current_edits.hsl_aqua_lum);
                                photo.edit_hsl_blue_lum = Some(current_edits.hsl_blue_lum);
                                photo.edit_hsl_purple_lum = Some(current_edits.hsl_purple_lum);
                                photo.edit_hsl_magenta_lum = Some(current_edits.hsl_magenta_lum);
                                // Lens
                                photo.edit_lens_distortion = Some(current_edits.lens_distortion);
                                photo.edit_lens_vignette_amount = Some(current_edits.lens_vignette_amount);
                                photo.edit_lens_vignette_midpoint = Some(current_edits.lens_vignette_midpoint);
                                // NR
                                photo.edit_nr_luminance = Some(current_edits.nr_luminance);
                                photo.edit_nr_color = Some(current_edits.nr_color);
                                // Sharpen
                                photo.edit_sharpen_amount = Some(current_edits.sharpen_amount);
                                photo.edit_sharpen_radius = Some(current_edits.sharpen_radius);
                                // Crop settings update
                                if let Some(crop) = &self.state.crop_settings {
                                    photo.edit_crop_x = Some(crop.crop_x());
                                    photo.edit_crop_y = Some(crop.crop_y());
                                    photo.edit_crop_width = Some(crop.crop_width());
                                    photo.edit_crop_height = Some(crop.crop_height());
                                    photo.edit_crop_rotation = Some(crop.rotation_90());
                                    photo.edit_crop_angle = Some(crop.angle());
                                    photo.edit_crop_flip_h = Some(crop.flip_horizontal());
                                    photo.edit_crop_flip_v = Some(crop.flip_vertical());
                                    photo.edit_crop_fill_mode = Some(crop.fill_mode() as u8);
                                } else {
                                    photo.edit_crop_x = None;
                                    photo.edit_crop_y = None;
                                    photo.edit_crop_width = None;
                                    photo.edit_crop_height = None;
                                    photo.edit_crop_rotation = None;
                                    photo.edit_crop_angle = None;
                                    photo.edit_crop_flip_h = None;
                                    photo.edit_crop_flip_v = None;
                                    photo.edit_crop_fill_mode = None;
                                }
                            }

                            // Invalidate cached thumbnails to force regeneration with updated effects
                            self.photo_grid.invalidate_thumbnail(&id);
                            self.filmstrip.invalidate_thumbnail(&id);

                            // Clear pending flag
                            self.state.pending_auto_save = false;
                            self.state.last_slider_change_time = None;

                            // Extract crop settings for closure
                            let (
                                crop_x, crop_y, crop_width, crop_height,
                                crop_rotation, crop_angle, crop_flip_h, crop_flip_v, crop_fill_mode
                            ) = if let Some(c) = &self.state.crop_settings {
                                (
                                    Some(c.crop_x()), Some(c.crop_y()), Some(c.crop_width()), Some(c.crop_height()),
                                    Some(c.rotation_90()), Some(c.angle()), Some(c.flip_horizontal()), Some(c.flip_vertical()), Some(c.fill_mode() as u8)
                                )
                            } else {
                                (None, None, None, None, None, None, None, None, None)
                            };

                            let ctx_clone = ctx.clone();

                            tokio::spawn(async move {
                                if let Err(e) = controller.save_edits(
                                    id, 
                                    current_edits.exposure, current_edits.contrast, current_edits.temperature, current_edits.tint, current_edits.highlights, current_edits.shadows,
                                    current_edits.whites, current_edits.blacks, current_edits.clarity, current_edits.vibrance, current_edits.saturation,
                                    current_edits.tone_curve_shadows, current_edits.tone_curve_darks, current_edits.tone_curve_lights, current_edits.tone_curve_highlights,
                                    current_edits.hsl_red_sat, current_edits.hsl_orange_sat, current_edits.hsl_yellow_sat, current_edits.hsl_green_sat,
                                    current_edits.hsl_aqua_sat, current_edits.hsl_blue_sat, current_edits.hsl_purple_sat, current_edits.hsl_magenta_sat,
                                    // HSL Hue
                                    current_edits.hsl_red_hue, current_edits.hsl_orange_hue, current_edits.hsl_yellow_hue, current_edits.hsl_green_hue,
                                    current_edits.hsl_aqua_hue, current_edits.hsl_blue_hue, current_edits.hsl_purple_hue, current_edits.hsl_magenta_hue,
                                    // HSL Lum
                                    current_edits.hsl_red_lum, current_edits.hsl_orange_lum, current_edits.hsl_yellow_lum, current_edits.hsl_green_lum,
                                    current_edits.hsl_aqua_lum, current_edits.hsl_blue_lum, current_edits.hsl_purple_lum, current_edits.hsl_magenta_lum,
                                    // Lens
                                    current_edits.lens_distortion, current_edits.lens_vignette_amount, current_edits.lens_vignette_midpoint,
                                    // NR
                                    current_edits.nr_luminance, current_edits.nr_color,
                                    // Sharpen
                                    current_edits.sharpen_amount, current_edits.sharpen_radius,
                                    // Crop
                                    crop_x, crop_y, crop_width, crop_height,
                                    crop_rotation, crop_angle, crop_flip_h, crop_flip_v, crop_fill_mode
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
                                self.state.internal_state.library_selected_id = None;
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

        // Show Print Dialog
        if self.state.show_print_dialog {
            if let Some(ref mut print_state) = self.state.print_dialog_state {
                use crate::components::print_dialog::{PrintDialog, PrintDialogAction};
                
                let action = PrintDialog::show(ctx, print_state);
                
                match action {
                    PrintDialogAction::Print => {
                        // TODO: Execute print job when infrastructure is ready
                        let photo_count = print_state.photo_ids.len();
                        let layout = print_state.layout.display_name();
                        self.state.toasts.info(format!(
                            "Print functionality coming soon! Would print {} photo(s) with {} layout", 
                            photo_count, layout
                        ));
                        self.state.show_print_dialog = false;
                        self.state.print_dialog_state = None;
                    }
                    PrintDialogAction::Cancel => {
                        self.state.show_print_dialog = false;
                        self.state.print_dialog_state = None;
                    }
                    PrintDialogAction::None => {
                        // Dialog still open, nothing to do
                    }
                }
            }
        }

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
                                let edits = self.editor_service.current_edits();
                                let adjustments = domain::entities::preset::PresetAdjustments {
                                    exposure: Some(edits.exposure),
                                    contrast: Some(edits.contrast),
                                    temperature: Some(edits.temperature),
                                    tint: Some(edits.tint),
                                    highlights: Some(edits.highlights),
                                    shadows: Some(edits.shadows),
                                    whites: Some(edits.whites),
                                    blacks: Some(edits.blacks),
                                    clarity: Some(edits.clarity),
                                    vibrance: Some(edits.vibrance),
                                    saturation: Some(edits.saturation),
                                    tone_curve_shadows: Some(edits.tone_curve_shadows),
                                    tone_curve_darks: Some(edits.tone_curve_darks),
                                    tone_curve_lights: Some(edits.tone_curve_lights),
                                    tone_curve_highlights: Some(edits.tone_curve_highlights),
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
        if self.state.internal_state.current_view == CurrentView::Import {
            crate::views::import_view::ImportView::show(
                ctx, 
                &mut self.state, 
                &self.import_controller, 
                &self.import_source_sender
            );
        } else if self.state.internal_state.current_view == CurrentView::Print {
            // Print view - Lightroom-style print module
            egui::CentralPanel::default().show(ctx, |ui| {
                self.print_view.show(
                    ui,
                    &mut self.state,
                    &self.photo_controller,
                    &self.library_controller,
                    &self.photo_sender,
                    ctx,
                );
            });
        } else {
            egui::CentralPanel::default().show(ctx, |_ui| {
                // Get the appropriate dock state for the current view
                let dock_state = match self.state.internal_state.current_view {
                    CurrentView::Library => &mut self.library_dock_state,
                    CurrentView::Develop => &mut self.develop_dock_state,
                    CurrentView::Print => &mut self.library_dock_state, // Fallback
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
                editor_service: &mut self.editor_service,
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

        // ============================================
        // SECONDARY WINDOW (Multi-Monitor Support)
        // ============================================
        // Handle F key to toggle secondary window
        // Handle F key or UI request to toggle secondary window
        let f_key = ctx.input(|i| i.key_pressed(egui::Key::F)) && !ctx.wants_keyboard_input();
        let ui_req = self.state.request_toggle_secondary_window;
        
        if f_key || ui_req {
            self.state.request_toggle_secondary_window = false; // Consume request
            // Prevent multiple toggles if update() is called multiple times per frame (common in egui)
            let toggle_id = egui::Id::new("secondary_window_toggle_frame");
            let last_time = ctx.data(|d| d.get_temp::<f64>(toggle_id).unwrap_or(-1.0));
            let current_time = ctx.input(|i| i.time);

            if last_time != current_time {
                // Record this frame as handled
                ctx.data_mut(|d| d.insert_temp(toggle_id, current_time));

                // Always refresh monitor list to catch changes
                self.cached_monitors = MonitorDetector::get_monitors();
                self.secondary_window.toggle(&self.cached_monitors);
                
                // Show toast notification
                if self.secondary_window.is_open {
                    let monitor_name = self.secondary_window.monitor
                        .as_ref()
                        .map(|m| m.name.as_str())
                        .unwrap_or("Unknown");
                    self.state.toasts.info(format!("Opening on {}", monitor_name));
                } else {
                    self.state.toasts.info("Secondary window closed");
                }
            }
        }
        
        // Handle I key to toggle info overlay in secondary window
        if ctx.input(|i| i.key_pressed(egui::Key::I)) && self.secondary_window.is_open {
            self.secondary_window.toggle_info_overlay();
        }
        
        // Sync secondary window with current selection
        if self.secondary_window.is_open {
            self.secondary_window.set_photo(self.state.internal_state.develop_selected_id.clone()
                .or_else(|| self.state.internal_state.library_selected_id.clone()));
                
            // Render secondary window viewport
            // We need to clone the info since we can't borrow self twice or pass multiple refs easily
            let photo_info = self.state.get_current_photo().map(|p| {
                let rating = if p.rating > 0 {
                    "★".repeat(p.rating as usize)
                } else {
                    String::new()
                };
                (p.name.clone(), rating)
            });
            
            let _has_selection = self.state.internal_state.develop_selected_id.is_some() || 
                               self.state.internal_state.library_selected_id.is_some();
            self.secondary_window.show(
                ctx,
                self.state.detail_image.as_ref(),
                self.state.thumbnail_preview.as_ref(),
                self.state.internal_state.develop_selected_id.is_some(),
                self.state.crop_settings.as_ref(), // Pass crop settings
                photo_info.as_ref().map(|(n, r)| (n.as_str(), r.as_str())),
            );
        }

        // Keep UI active if we are waiting for background tasks
        // This prevents the UI from sleeping (gray screen) while image loads
        if self.image_processor.is_processing() || self.requested_photo_id.is_some() {
            ctx.request_repaint();
        }
        
        // Also keep repainting if secondary window is open (to sync updates)
        if self.secondary_window.is_open {
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
            if widgets::nav_button(ui, &library_label, self.state.internal_state.current_view == CurrentView::Library).clicked() {
                // Save pending edits before leaving Develop mode
                if self.state.internal_state.current_view == CurrentView::Develop {
                    self.save_pending_develop_edits();
                }
                self.state.internal_state.current_view = CurrentView::Library;
                self.state.reset_viewer();
            }

            ui.add_space(Theme::SPACE_SM);

            // Develop button enabled if Library has a selection
            let develop_label = format!("{} Develop", icons::NAV_DEVELOP);
            let develop_enabled = self.state.internal_state.library_selected_id.is_some();
            if develop_enabled {
                if widgets::nav_button(ui, &develop_label, self.state.internal_state.current_view == CurrentView::Develop).clicked() {
                    // Copy Library selection to Develop when entering Develop mode
                    if self.state.internal_state.develop_selected_id.is_none() {
                        self.state.internal_state.develop_selected_id = self.state.internal_state.library_selected_id.clone();
                        self.state.loaded_photo_id = None; // Force image load
                    }
                    self.state.internal_state.current_view = CurrentView::Develop;
                }
            } else {
                ui.add_enabled_ui(false, |ui| {
                    widgets::nav_button(ui, &develop_label, false);
                });
            }

            ui.add_space(Theme::SPACE_SM);

            // Print button - switches to Print view (Lightroom-style print module)
            let print_label = format!("{} Print", icons::NAV_PRINT);
            let print_enabled = !self.state.selected_photo_ids.is_empty() || self.state.internal_state.library_selected_id.is_some();
            if print_enabled {
                if widgets::nav_button(ui, &print_label, self.state.internal_state.current_view == CurrentView::Print).clicked() {
                    // Initialize print view state with selected photos
                    let photo_ids: Vec<String> = if !self.state.selected_photo_ids.is_empty() {
                        self.state.selected_photo_ids.iter().cloned().collect()
                    } else if let Some(id) = &self.state.internal_state.library_selected_id {
                        vec![id.clone()]
                    } else {
                        vec![]
                    };
                    
                    // Set up print view state and switch view
                    self.state.print_view_state = Some(crate::views::print_view::PrintViewState::new(photo_ids));
                    self.state.internal_state.current_view = CurrentView::Print;
                }
            } else {
                ui.add_enabled_ui(false, |ui| {
                    widgets::nav_button(ui, &print_label, false);
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

    /// Request Intelligent Fill processing if needed
    /// Called when crop settings change and fill_mode is Intelligent
    fn request_intelligent_fill_if_needed(&mut self) {
        // Only process if we're in develop view with a selected photo
        if self.state.internal_state.current_view != crate::state::CurrentView::Develop {
            return;
        }

        // Check if we have crop settings with Intelligent fill mode
        let crop = match &self.state.crop_settings {
            Some(c) => c,
            None => {
                eprintln!("[IntelligentFill] No crop settings");
                return;
            }
        };

        // Only process if fill mode is Intelligent
        if crop.fill_mode() != domain::value_objects::RotationFillMode::Intelligent {
            // Clear any cached texture if fill mode changed
            if self.state.intelligent_fill_texture.is_some() {
                self.state.intelligent_fill_texture = None;
            }
            return;
        }

        // Only process if there's a non-zero angle
        if crop.angle() == 0.0 {
            self.state.intelligent_fill_texture = None;
            return;
        }

        eprintln!("[IntelligentFill] fill_mode=Intelligent, angle={}", crop.angle());

        // Check if angle changed (debouncing)
        let angle_changed = (crop.angle() - self.state.prev_intelligent_fill_angle).abs() > 0.001;
        if !angle_changed && self.state.intelligent_fill_texture.is_some() {
            return; // Already have a valid texture for this angle
        }

        // Need original image data to process
        let image_data = match &self.state.original_image_data {
            Some(data) => data.clone(),
            None => {
                eprintln!("[IntelligentFill] No original_image_data!");
                return;
            }
        };

        let original = match &self.state.original_preview {
            Some(img) => img,
            None => {
                eprintln!("[IntelligentFill] No original_preview!");
                return;
            }
        };

        eprintln!("[IntelligentFill] Sending request for {}x{}", original.width(), original.height());

        // Create request
        let request_id = self.intelligent_fill_processor.next_request_id();
        let request = crate::intelligent_fill::IntelligentFillRequest {
            request_id,
            image_data,
            width: original.width(),
            height: original.height(),
            crop_settings: crop.clone(),
        };

        // Send request
        self.intelligent_fill_processor.request_fill(request);
        self.state.intelligent_fill_request_id = request_id;
        self.state.intelligent_fill_pending = true;
        self.state.prev_intelligent_fill_angle = crop.angle();
    }

    /// Save any pending develop edits before switching away from Develop mode.
    /// This ensures crop and other edits are persisted even when:
    /// - Clicking Library nav button
    /// - Pressing Escape key
    /// - Switching to any other mode
    fn save_pending_develop_edits(&mut self) {
        if !self.state.pending_auto_save {
            return;
        }
        
        if let Some(vm) = self.state.get_current_photo() {
            let controller = self.editor_controller.clone();
            let id = vm.id.clone();
            let current_edits = self.editor_service.current_edits();
            // Use current edits directly

            let active_crop = self.state.crop_settings.clone();

            // Clone for in-memory update
            let id_for_update = id.clone();
            let crop_for_update = active_crop.clone();
            let edits_for_local = current_edits.clone();

            // Spawn async save
            tokio::spawn(async move {
                let _ = controller.save_edits(
                    id,
                    current_edits.exposure, current_edits.contrast, current_edits.temperature, current_edits.tint,
                    current_edits.highlights, current_edits.shadows, current_edits.whites, current_edits.blacks,
                    current_edits.clarity, current_edits.vibrance, current_edits.saturation,
                    current_edits.tone_curve_shadows, current_edits.tone_curve_darks, current_edits.tone_curve_lights, current_edits.tone_curve_highlights,
                    current_edits.hsl_red_sat, current_edits.hsl_orange_sat, current_edits.hsl_yellow_sat, current_edits.hsl_green_sat, current_edits.hsl_aqua_sat, current_edits.hsl_blue_sat, current_edits.hsl_purple_sat, current_edits.hsl_magenta_sat,
                    current_edits.hsl_red_hue, current_edits.hsl_orange_hue, current_edits.hsl_yellow_hue, current_edits.hsl_green_hue, current_edits.hsl_aqua_hue, current_edits.hsl_blue_hue, current_edits.hsl_purple_hue, current_edits.hsl_magenta_hue,
                    current_edits.hsl_red_lum, current_edits.hsl_orange_lum, current_edits.hsl_yellow_lum, current_edits.hsl_green_lum, current_edits.hsl_aqua_lum, current_edits.hsl_blue_lum, current_edits.hsl_purple_lum, current_edits.hsl_magenta_lum,
                    current_edits.lens_distortion, current_edits.lens_vignette_amount, current_edits.lens_vignette_midpoint,
                    current_edits.nr_luminance, current_edits.nr_color,
                    current_edits.sharpen_amount, current_edits.sharpen_radius,
                    active_crop.as_ref().map(|c| c.crop_x()),
                    active_crop.as_ref().map(|c| c.crop_y()),
                    active_crop.as_ref().map(|c| c.crop_width()),
                    active_crop.as_ref().map(|c| c.crop_height()),
                    active_crop.as_ref().map(|c| c.rotation_90()),
                    active_crop.as_ref().map(|c| c.angle()),
                    active_crop.as_ref().map(|c| c.flip_horizontal()),
                    active_crop.as_ref().map(|c| c.flip_vertical()),
                    active_crop.as_ref().map(|c| c.fill_mode() as u8),
                ).await;
            });
            
            // Update in-memory ViewModel immediately
            if let Some(photo_vm) = self.state.photos.iter_mut().find(|p| p.id == id_for_update) {
                photo_vm.edit_crop_x = crop_for_update.as_ref().map(|c| c.crop_x());
                photo_vm.edit_crop_y = crop_for_update.as_ref().map(|c| c.crop_y());
                photo_vm.edit_crop_width = crop_for_update.as_ref().map(|c| c.crop_width());
                photo_vm.edit_crop_height = crop_for_update.as_ref().map(|c| c.crop_height());
                photo_vm.edit_crop_rotation = crop_for_update.as_ref().map(|c| c.rotation_90());
                photo_vm.edit_crop_angle = crop_for_update.as_ref().map(|c| c.angle());
                photo_vm.edit_crop_flip_h = crop_for_update.as_ref().map(|c| c.flip_horizontal());
                photo_vm.edit_crop_flip_v = crop_for_update.as_ref().map(|c| c.flip_vertical());
                photo_vm.edit_crop_fill_mode = crop_for_update.as_ref().map(|c| c.fill_mode() as u8);
                photo_vm.edit_exposure = Some(edits_for_local.exposure);
                photo_vm.edit_contrast = Some(edits_for_local.contrast);
                photo_vm.edit_temperature = Some(edits_for_local.temperature);
                photo_vm.edit_tint = Some(edits_for_local.tint);
                photo_vm.edit_highlights = Some(edits_for_local.highlights);
                photo_vm.edit_shadows = Some(edits_for_local.shadows);
                photo_vm.edit_whites = Some(edits_for_local.whites);
                photo_vm.edit_blacks = Some(edits_for_local.blacks);
                photo_vm.edit_clarity = Some(edits_for_local.clarity);
                photo_vm.edit_vibrance = Some(edits_for_local.vibrance);
                photo_vm.edit_saturation = Some(edits_for_local.saturation);
                photo_vm.edit_tone_curve_shadows = Some(edits_for_local.tone_curve_shadows);
                photo_vm.edit_tone_curve_darks = Some(edits_for_local.tone_curve_darks);
                photo_vm.edit_tone_curve_lights = Some(edits_for_local.tone_curve_lights);
                photo_vm.edit_tone_curve_highlights = Some(edits_for_local.tone_curve_highlights);
                // HSL Sat
                photo_vm.edit_hsl_red_sat = Some(edits_for_local.hsl_red_sat);
                photo_vm.edit_hsl_orange_sat = Some(edits_for_local.hsl_orange_sat);
                photo_vm.edit_hsl_yellow_sat = Some(edits_for_local.hsl_yellow_sat);
                photo_vm.edit_hsl_green_sat = Some(edits_for_local.hsl_green_sat);
                photo_vm.edit_hsl_aqua_sat = Some(edits_for_local.hsl_aqua_sat);
                photo_vm.edit_hsl_blue_sat = Some(edits_for_local.hsl_blue_sat);
                photo_vm.edit_hsl_purple_sat = Some(edits_for_local.hsl_purple_sat);
                photo_vm.edit_hsl_magenta_sat = Some(edits_for_local.hsl_magenta_sat);
                // HSL Hue
                photo_vm.edit_hsl_red_hue = Some(edits_for_local.hsl_red_hue);
                photo_vm.edit_hsl_orange_hue = Some(edits_for_local.hsl_orange_hue);
                photo_vm.edit_hsl_yellow_hue = Some(edits_for_local.hsl_yellow_hue);
                photo_vm.edit_hsl_green_hue = Some(edits_for_local.hsl_green_hue);
                photo_vm.edit_hsl_aqua_hue = Some(edits_for_local.hsl_aqua_hue);
                photo_vm.edit_hsl_blue_hue = Some(edits_for_local.hsl_blue_hue);
                photo_vm.edit_hsl_purple_hue = Some(edits_for_local.hsl_purple_hue);
                photo_vm.edit_hsl_magenta_hue = Some(edits_for_local.hsl_magenta_hue);
                // HSL Lum
                photo_vm.edit_hsl_red_lum = Some(edits_for_local.hsl_red_lum);
                photo_vm.edit_hsl_orange_lum = Some(edits_for_local.hsl_orange_lum);
                photo_vm.edit_hsl_yellow_lum = Some(edits_for_local.hsl_yellow_lum);
                photo_vm.edit_hsl_green_lum = Some(edits_for_local.hsl_green_lum);
                photo_vm.edit_hsl_aqua_lum = Some(edits_for_local.hsl_aqua_lum);
                photo_vm.edit_hsl_blue_lum = Some(edits_for_local.hsl_blue_lum);
                photo_vm.edit_hsl_purple_lum = Some(edits_for_local.hsl_purple_lum);
                photo_vm.edit_hsl_magenta_lum = Some(edits_for_local.hsl_magenta_lum);
                // Lens
                photo_vm.edit_lens_distortion = Some(edits_for_local.lens_distortion);
                photo_vm.edit_lens_vignette_amount = Some(edits_for_local.lens_vignette_amount);
                photo_vm.edit_lens_vignette_midpoint = Some(edits_for_local.lens_vignette_midpoint);
                // NR
                photo_vm.edit_nr_luminance = Some(edits_for_local.nr_luminance);
                photo_vm.edit_nr_color = Some(edits_for_local.nr_color);
                // Sharpen
                photo_vm.edit_sharpen_amount = Some(edits_for_local.sharpen_amount);
                photo_vm.edit_sharpen_radius = Some(edits_for_local.sharpen_radius);
            }
            
            // Clear pending flag
            self.state.pending_auto_save = false;
        }
    }

    /// Handle import button click
    fn handle_import(&mut self, _ctx: &egui::Context) {
        self.state.internal_state.current_view = crate::state::CurrentView::Import;
        
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
