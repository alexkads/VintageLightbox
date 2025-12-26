#![allow(dead_code)]

// Application State Management
// Replaces Slint's declarative properties with Rust state struct

use adapters::view_models::PhotoViewModel;
use image::DynamicImage;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use crate::components::import_dialogs::{ImportPreviewDialog, ImportProgressDialog};
use crate::design_system::theme_selector::ThemeVariant;
use crate::docking::DockTab;
use egui_dock::DockState;
use egui_notify::Toasts;
use infrastructure::cache::CacheStats;
use crate::components::filmstrip_filter::FilmstripFilter;

/// Snapshot of editing state for undo/redo
#[derive(Debug, Clone)]
pub struct EditSnapshot {
    pub exposure: f32,
    pub contrast: f32,
    pub temperature: f32,
    pub tint: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub clarity: f32,
    pub vibrance: f32,
    pub saturation: f32,
    // Tone Curve (parametric zones)
    pub tone_curve_shadows: f32,
    pub tone_curve_darks: f32,
    pub tone_curve_lights: f32,
    pub tone_curve_highlights: f32,
    // HSL color channel saturations
    pub hsl_red_sat: f32,
    pub hsl_orange_sat: f32,
    pub hsl_yellow_sat: f32,
    pub hsl_green_sat: f32,
    pub hsl_aqua_sat: f32,
    pub hsl_blue_sat: f32,
    pub hsl_purple_sat: f32,
    pub hsl_magenta_sat: f32,
    // HSL Hue
    pub hsl_red_hue: f32,
    pub hsl_orange_hue: f32,
    pub hsl_yellow_hue: f32,
    pub hsl_green_hue: f32,
    pub hsl_aqua_hue: f32,
    pub hsl_blue_hue: f32,
    pub hsl_purple_hue: f32,
    pub hsl_magenta_hue: f32,
    // HSL Lum
    pub hsl_red_lum: f32,
    pub hsl_orange_lum: f32,
    pub hsl_yellow_lum: f32,
    pub hsl_green_lum: f32,
    pub hsl_aqua_lum: f32,
    pub hsl_blue_lum: f32,
    pub hsl_purple_lum: f32,
    pub hsl_magenta_lum: f32,
    // Lens
    pub lens_distortion: f32,
    pub lens_vignette_amount: f32,
    pub lens_vignette_midpoint: f32,
    // Noise Reduction
    pub nr_luminance: f32,
    pub nr_color: f32,
    // Sharpening
    pub sharpen_amount: f32,
    pub sharpen_radius: f32,
}

/// Current view in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentView {
    Library,
    Develop,
    Print,
    Import,
}

/// Metadata for the currently selected photo in detail view
#[derive(Debug, Clone)]
pub struct DetailMetadata {
    pub id: String,
    pub name: String,
    pub date: String,
    pub camera: String,
    pub exposure: String,
    pub rating: i32,
    pub color_label: Option<String>,
}

/// State for the Import View
#[derive(Default)]
pub struct ImportViewState {
    pub devices: Vec<domain::import_source::ImportSource>,
    pub selected_source_id: Option<String>,
    pub found_files: Vec<String>, // Paths
    pub selected_files: std::collections::HashSet<String>,
    pub options: domain::value_objects::ImportOptions,
}

/// Main application state
pub struct AppState {
    // ============================================
    // Navigation
    // ============================================
    pub current_view: CurrentView,
    pub import_view_state: ImportViewState,

    // ============================================
    // Photo Library
    // ============================================
    pub photos: Vec<PhotoViewModel>,
    /// Selected photo in Library view (independent from Develop)
    pub library_selected_photo_id: Option<String>,
    /// Selected photo in Develop view (independent from Library)
    pub develop_selected_photo_id: Option<String>,
    /// ID of the photo currently loaded in detail_image (for change detection)
    pub loaded_photo_id: Option<String>,
    /// Multi-selection: set of selected photo IDs
    pub selected_photo_ids: HashSet<String>,
    /// Last clicked photo index for Shift+click range selection
    pub last_clicked_index: Option<usize>,
    /// Whether to show delete confirmation dialog
    pub show_delete_confirmation: bool,

    // ============================================
    // Detail View
    // ============================================
    /// Texture handle for the currently displayed image (full resolution)
    pub detail_image: Option<egui::TextureHandle>,
    /// Thumbnail preview shown instantly while full image loads (Lightroom-style)
    pub thumbnail_preview: Option<egui::TextureHandle>,
    /// Metadata of the currently selected photo
    pub detail_metadata: Option<DetailMetadata>,
    /// Histogram data for the current image
    pub histogram_data: Option<crate::components::histogram::HistogramData>,
    /// Timestamp when the detail image was fully loaded (for transition animation)
    pub detail_image_loaded_at: Option<std::time::Instant>,

    // ============================================
    // Image Processing
    // ============================================
    /// Active image being edited (shared with background processing)
    pub active_image: Arc<Mutex<Option<DynamicImage>>>,
    /// Current exposure adjustment value
    pub active_exposure: f32,
    /// Current contrast adjustment value
    pub active_contrast: f32,
    /// Current temperature adjustment value
    pub active_temperature: f32,
    /// Current tint adjustment value
    pub active_tint: f32,
    /// Current highlights adjustment value
    pub active_highlights: f32,
    /// Current shadows adjustment value
    pub active_shadows: f32,
    /// Current whites adjustment value
    pub active_whites: f32,
    /// Current blacks adjustment value
    pub active_blacks: f32,
    /// Current clarity adjustment value
    pub active_clarity: f32,
    /// Current vibrance adjustment value
    pub active_vibrance: f32,
    /// Current saturation adjustment value
    pub active_saturation: f32,
    pub active_tone_curve_shadows: f32,
    pub active_tone_curve_darks: f32,
    pub active_tone_curve_lights: f32,
    pub active_tone_curve_highlights: f32,
    // HSL color channel saturations (-100 to +100)
    pub active_hsl_red_sat: f32,
    pub active_hsl_orange_sat: f32,
    pub active_hsl_yellow_sat: f32,
    pub active_hsl_green_sat: f32,
    pub active_hsl_aqua_sat: f32,
    pub active_hsl_blue_sat: f32,
    pub active_hsl_purple_sat: f32,
    pub active_hsl_magenta_sat: f32,
    // HSL Hue
    pub active_hsl_red_hue: f32,
    pub active_hsl_orange_hue: f32,
    pub active_hsl_yellow_hue: f32,
    pub active_hsl_green_hue: f32,
    pub active_hsl_aqua_hue: f32,
    pub active_hsl_blue_hue: f32,
    pub active_hsl_purple_hue: f32,
    pub active_hsl_magenta_hue: f32,
    // HSL Lum
    pub active_hsl_red_lum: f32,
    pub active_hsl_orange_lum: f32,
    pub active_hsl_yellow_lum: f32,
    pub active_hsl_green_lum: f32,
    pub active_hsl_aqua_lum: f32,
    pub active_hsl_blue_lum: f32,
    pub active_hsl_purple_lum: f32,
    pub active_hsl_magenta_lum: f32,
    // Lens
    pub active_lens_distortion: f32,
    pub active_lens_vignette_amount: f32,
    pub active_lens_vignette_midpoint: f32,
    // Noise Reduction (Amount 0-100)
    pub active_nr_luminance: f32,
    pub active_nr_color: f32,
    // Sharpening
    pub active_sharpen_amount: f32,
    pub active_sharpen_radius: f32,
    /// Previous exposure value (for change detection)
    pub prev_exposure: f32,
    /// Previous contrast value (for change detection)
    pub prev_contrast: f32,
    /// Previous temperature value (for change detection)
    pub prev_temperature: f32,
    /// Previous tint value (for change detection)
    pub prev_tint: f32,
    /// Previous highlights value (for change detection)
    pub prev_highlights: f32,
    /// Previous shadows value (for change detection)
    pub prev_shadows: f32,
    /// Previous whites value (for change detection)
    pub prev_whites: f32,
    /// Previous blacks value (for change detection)
    pub prev_blacks: f32,
    /// Previous clarity value (for change detection)
    pub prev_clarity: f32,
    /// Previous vibrance value (for change detection)
    pub prev_vibrance: f32,
    /// Previous saturation value (for change detection)
    pub prev_saturation: f32,
    /// Previous tone curve shadows (for change detection)
    pub prev_tone_curve_shadows: f32,
    /// Previous tone curve darks (for change detection)
    pub prev_tone_curve_darks: f32,
    /// Previous tone curve lights (for change detection)
    pub prev_tone_curve_lights: f32,
    /// Previous tone curve highlights (for change detection)
    pub prev_tone_curve_highlights: f32,
    // Previous HSL values (for change detection)
    pub prev_hsl_red_sat: f32,
    pub prev_hsl_orange_sat: f32,
    pub prev_hsl_yellow_sat: f32,
    pub prev_hsl_green_sat: f32,
    pub prev_hsl_aqua_sat: f32,
    pub prev_hsl_blue_sat: f32,
    pub prev_hsl_purple_sat: f32,
    pub prev_hsl_magenta_sat: f32,
    // HSL Hue
    pub prev_hsl_red_hue: f32,
    pub prev_hsl_orange_hue: f32,
    pub prev_hsl_yellow_hue: f32,
    pub prev_hsl_green_hue: f32,
    pub prev_hsl_aqua_hue: f32,
    pub prev_hsl_blue_hue: f32,
    pub prev_hsl_purple_hue: f32,
    pub prev_hsl_magenta_hue: f32,
    // HSL Lum
    pub prev_hsl_red_lum: f32,
    pub prev_hsl_orange_lum: f32,
    pub prev_hsl_yellow_lum: f32,
    pub prev_hsl_green_lum: f32,
    pub prev_hsl_aqua_lum: f32,
    pub prev_hsl_blue_lum: f32,
    pub prev_hsl_purple_lum: f32,
    pub prev_hsl_magenta_lum: f32,
    // Lens
    pub prev_lens_distortion: f32,
    pub prev_lens_vignette_amount: f32,
    pub prev_lens_vignette_midpoint: f32,
    // Previous NR (for change detection)
    pub prev_nr_luminance: f32,
    pub prev_nr_color: f32,
    // Previous Sharpening
    pub prev_sharpen_amount: f32,
    pub prev_sharpen_radius: f32,
    /// Original unprocessed preview image
    pub original_preview: Option<DynamicImage>,
    /// Cached raw image data for GPU processing (Arc to avoid cloning)
    pub original_image_data: Option<Arc<Vec<u8>>>,

    // ============================================
    // Image Viewer State
    // ============================================
    pub zoom_level: f32,
    pub pan_offset: egui::Vec2,
    pub show_before: bool, // Before/After toggle state
    pub prev_show_before: bool, // Previous before/after state for change detection

    // ============================================
    // UI State
    // ============================================
    pub is_busy: bool,
    pub busy_message: String,
    /// Number of columns in photo grid (1-5)
    pub grid_columns: usize,
    /// Toast notifications
    pub toasts: Toasts,
    /// Selected theme variant
    pub selected_theme: ThemeVariant,

    // ============================================
    // Filters
    // ============================================
    // Filmstrip specific filter
    pub filmstrip_filter: FilmstripFilter,

    // ============================================
    // Async Operations
    // ============================================
    pub pending_import: Option<poll_promise::Promise<Result<(), String>>>,
    pub pending_export: Option<poll_promise::Promise<Result<(), String>>>,
    /// Receiver for export results (path of exported file or error)
    pub pending_export_receiver: Option<tokio::sync::mpsc::Receiver<Result<String, String>>>,

    // ============================================
    // Undo/Redo History
    // ============================================
    /// Undo/Redo history
    pub edit_history: Vec<EditSnapshot>,
    pub history_index: Option<usize>, // Current position in history (None = no history)

    /// Performance Metrics
    pub performance_metrics: PerformanceMetrics,
    pub show_performance_stats: bool,
    /// Timestamp when the current photo load started (to measure TTI)
    pub start_load_time: Option<std::time::Instant>,

    // ============================================
    // Auto-Save Debouncing
    // ============================================
    /// Whether edits are pending to be auto-saved
    pub pending_auto_save: bool,
    /// Timestamp of the last slider change (for debounce)
    pub last_slider_change_time: Option<std::time::Instant>,
    /// Last saved values (to avoid unnecessary saves)
    pub saved_exposure: f32,
    pub saved_contrast: f32,
    pub saved_temperature: f32,
    pub saved_tint: f32,
    pub saved_highlights: f32,
    pub saved_shadows: f32,
    pub saved_whites: f32,
    pub saved_blacks: f32,
    pub saved_clarity: f32,
    pub saved_vibrance: f32,
    pub saved_saturation: f32,
    // Saved Tone Curve
    pub saved_tone_curve_shadows: f32,
    pub saved_tone_curve_darks: f32,
    pub saved_tone_curve_lights: f32,
    pub saved_tone_curve_highlights: f32,
    // Saved HSL values
    pub saved_hsl_red_sat: f32,
    pub saved_hsl_orange_sat: f32,
    pub saved_hsl_yellow_sat: f32,
    pub saved_hsl_green_sat: f32,
    pub saved_hsl_aqua_sat: f32,
    pub saved_hsl_blue_sat: f32,
    pub saved_hsl_purple_sat: f32,
    pub saved_hsl_magenta_sat: f32,
    // HSL Hue
    pub saved_hsl_red_hue: f32,
    pub saved_hsl_orange_hue: f32,
    pub saved_hsl_yellow_hue: f32,
    pub saved_hsl_green_hue: f32,
    pub saved_hsl_aqua_hue: f32,
    pub saved_hsl_blue_hue: f32,
    pub saved_hsl_purple_hue: f32,
    pub saved_hsl_magenta_hue: f32,
    // HSL Lum
    pub saved_hsl_red_lum: f32,
    pub saved_hsl_orange_lum: f32,
    pub saved_hsl_yellow_lum: f32,
    pub saved_hsl_green_lum: f32,
    pub saved_hsl_aqua_lum: f32,
    pub saved_hsl_blue_lum: f32,
    pub saved_hsl_purple_lum: f32,
    pub saved_hsl_magenta_lum: f32,
    // Lens
    pub saved_lens_distortion: f32,
    pub saved_lens_vignette_amount: f32,
    pub saved_lens_vignette_midpoint: f32,
    // Saved NR
    pub saved_nr_luminance: f32,
    pub saved_nr_color: f32,
    // Sharpening
    pub saved_sharpen_amount: f32,
    pub saved_sharpen_radius: f32,

    // ============================================
    // Folder Navigation
    // ============================================
    /// Root nodes of the folder tree
    pub folder_tree_roots: Vec<crate::components::folder_tree::FolderNode>,
    /// Set of expanded folder paths in the tree
    pub expanded_folders: HashSet<String>,

    // ============================================
    // Advanced Import Dialogs
    // ============================================
    /// Import preview dialog state
    pub import_preview_dialog: Option<ImportPreviewDialog>,
    /// Import progress dialog state
    pub import_progress_dialog: Option<ImportProgressDialog>,
    /// Receiver for import preview dialog (async communication)
    pub pending_import_preview_receiver: Option<tokio::sync::mpsc::Receiver<Option<ImportPreviewDialog>>>,

    // ============================================
    // Docking System (egui_dock)
    // ============================================
    /// Dock state for Library view layout
    pub library_dock_state: DockState<DockTab>,
    /// Dock state for Develop view layout
    pub develop_dock_state: DockState<DockTab>,

    // ============================================
    // Cache & Settings
    // ============================================
    /// Whether to show the settings dialog
    pub show_settings_dialog: bool,
    /// Current cache statistics (refreshed when settings dialog opens)
    pub cache_stats: Option<CacheStats>,
    /// Progress of cache building (shown in toolbar)
    pub cache_building_progress: Option<CacheBuildingProgress>,

    // ============================================
    // Presets
    // ============================================
    /// Loaded presets (system + user)
    pub presets: Vec<domain::entities::Preset>,
    /// Whether to show the save preset dialog
    pub show_save_preset_dialog: bool,
    /// Name input for new preset
    pub save_preset_name: String,

    // ============================================
    // File Dialogs
    // ============================================
    pub import_dialog: Option<egui_file::FileDialog>,
    pub import_dialog_mode: ImportDialogMode,
    pub export_target_id: Option<String>,
    
    // Command request from UI to App
    pub request_toggle_secondary_window: bool,

    // ============================================
    // Print View State
    // ============================================
    /// State for the Print view (Lightroom-style print module)
    pub print_view_state: Option<crate::views::print_view::PrintViewState>,
    /// Whether to show the print dialog (legacy - kept for compatibility)
    pub show_print_dialog: bool,
    /// Print dialog state (legacy - kept for compatibility)
    pub print_dialog_state: Option<crate::components::print_dialog::PrintDialogState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportDialogMode {
    Simple,
    Advanced,
    Export, // Reusing for export too?
}

#[derive(Clone, Debug, Default)]
pub struct PerformanceMetrics {
    pub image_load_time_ms: Option<f32>,
    pub gpu_process_time_ms: Option<f32>,
    pub texture_upload_time_ms: Option<f32>,
}

/// Progress of cache/preview building during import or regeneration
#[derive(Debug, Clone)]
pub struct CacheBuildingProgress {
    /// Total number of items to process
    pub total: usize,
    /// Number of items completed
    pub completed: usize,
    /// Current file being processed (display name)
    pub current_file: String,
    /// Type of preview being built ("Thumbnail" or "Preview")
    pub preview_type: String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            current_view: CurrentView::Library,
            import_view_state: ImportViewState::default(),
            photos: Vec::new(),
            library_selected_photo_id: None,
            develop_selected_photo_id: None,
            loaded_photo_id: None,
            selected_photo_ids: HashSet::new(),
            last_clicked_index: None,
            show_delete_confirmation: false,
            detail_image: None,
            thumbnail_preview: None,
            detail_metadata: None,
            histogram_data: None,
            detail_image_loaded_at: None,
            active_image: Arc::new(Mutex::new(None)),
            active_exposure: 0.0,
            active_contrast: 1.0,
            active_temperature: 0.0,
            active_tint: 0.0,
            active_highlights: 0.0,
            active_shadows: 0.0,
            active_whites: 0.0,
            active_blacks: 0.0,
            active_clarity: 0.0,
            active_vibrance: 0.0,
            active_saturation: 0.0,
            active_tone_curve_shadows: 0.0,
            active_tone_curve_darks: 0.0,
            active_tone_curve_lights: 0.0,
            active_tone_curve_highlights: 0.0,
            // HSL active values
            active_hsl_red_sat: 0.0,
            active_hsl_orange_sat: 0.0,
            active_hsl_yellow_sat: 0.0,
            active_hsl_green_sat: 0.0,
            active_hsl_aqua_sat: 0.0,
            active_hsl_blue_sat: 0.0,
            active_hsl_purple_sat: 0.0,
            active_hsl_magenta_sat: 0.0,
            // HSL Hue
            active_hsl_red_hue: 0.0,
            active_hsl_orange_hue: 0.0,
            active_hsl_yellow_hue: 0.0,
            active_hsl_green_hue: 0.0,
            active_hsl_aqua_hue: 0.0,
            active_hsl_blue_hue: 0.0,
            active_hsl_purple_hue: 0.0,
            active_hsl_magenta_hue: 0.0,
            // HSL Lum
            active_hsl_red_lum: 0.0,
            active_hsl_orange_lum: 0.0,
            active_hsl_yellow_lum: 0.0,
            active_hsl_green_lum: 0.0,
            active_hsl_aqua_lum: 0.0,
            active_hsl_blue_lum: 0.0,
            active_hsl_purple_lum: 0.0,
            active_hsl_magenta_lum: 0.0,
            // Lens
            active_lens_distortion: 0.0,
            active_lens_vignette_amount: 0.0,
            active_lens_vignette_midpoint: 0.0,
            // NR
            active_nr_luminance: 0.0,
            active_nr_color: 0.0,
            // Sharpening
            active_sharpen_amount: 0.0,
            active_sharpen_radius: 1.0,
            prev_exposure: 0.0,
            prev_contrast: 1.0,
            prev_temperature: 0.0,
            prev_tint: 0.0,
            prev_highlights: 0.0,
            prev_shadows: 0.0,
            prev_whites: 0.0,
            prev_blacks: 0.0,
            prev_clarity: 0.0,
            prev_vibrance: 0.0,
            prev_saturation: 0.0,
            prev_tone_curve_shadows: 0.0,
            prev_tone_curve_darks: 0.0,
            prev_tone_curve_lights: 0.0,
            prev_tone_curve_highlights: 0.0,
            // HSL prev values
            prev_hsl_red_sat: 0.0,
            prev_hsl_orange_sat: 0.0,
            prev_hsl_yellow_sat: 0.0,
            prev_hsl_green_sat: 0.0,
            prev_hsl_aqua_sat: 0.0,
            prev_hsl_blue_sat: 0.0,
            prev_hsl_purple_sat: 0.0,
            prev_hsl_magenta_sat: 0.0,
            // HSL Hue
            prev_hsl_red_hue: 0.0,
            prev_hsl_orange_hue: 0.0,
            prev_hsl_yellow_hue: 0.0,
            prev_hsl_green_hue: 0.0,
            prev_hsl_aqua_hue: 0.0,
            prev_hsl_blue_hue: 0.0,
            prev_hsl_purple_hue: 0.0,
            prev_hsl_magenta_hue: 0.0,
            // HSL Lum
            prev_hsl_red_lum: 0.0,
            prev_hsl_orange_lum: 0.0,
            prev_hsl_yellow_lum: 0.0,
            prev_hsl_green_lum: 0.0,
            prev_hsl_aqua_lum: 0.0,
            prev_hsl_blue_lum: 0.0,
            prev_hsl_purple_lum: 0.0,
            prev_hsl_magenta_lum: 0.0,
            // Lens
            prev_lens_distortion: 0.0,
            prev_lens_vignette_amount: 0.0,
            prev_lens_vignette_midpoint: 0.0,
            // NR prev
            prev_nr_luminance: 0.0,
            prev_nr_color: 0.0,
            // Sharpening prev
            prev_sharpen_amount: 0.0,
            prev_sharpen_radius: 1.0,
            original_preview: None,
            original_image_data: None,
            zoom_level: 1.0,

            pan_offset: egui::Vec2::ZERO,
            request_toggle_secondary_window: false,
            show_before: false,
            prev_show_before: false,
            is_busy: false,
            busy_message: String::new(),
            grid_columns: 4,  // Default 4 columns
            filmstrip_filter: FilmstripFilter::new(),
            pending_import: None,
            pending_export: None,
            edit_history: Vec::new(),
            history_index: None,
            performance_metrics: PerformanceMetrics::default(),
            show_performance_stats: std::env::var("SHOW_PERFORMANCE_STATS").map_or(false, |v| v == "true"),
            start_load_time: None,
            pending_auto_save: false,
            last_slider_change_time: None,
            saved_exposure: 0.0,
            saved_contrast: 1.0,
            saved_temperature: 0.0,
            saved_tint: 0.0,
            saved_highlights: 0.0,
            saved_shadows: 0.0,
            saved_whites: 0.0,
            saved_blacks: 0.0,
            saved_clarity: 0.0,
            saved_vibrance: 0.0,
            saved_saturation: 0.0,
            // Saved Tone Curve
            saved_tone_curve_shadows: 0.0,
            saved_tone_curve_darks: 0.0,
            saved_tone_curve_lights: 0.0,
            saved_tone_curve_highlights: 0.0,
            // HSL saved values
            saved_hsl_red_sat: 0.0,
            saved_hsl_orange_sat: 0.0,
            saved_hsl_yellow_sat: 0.0,
            saved_hsl_green_sat: 0.0,
            saved_hsl_aqua_sat: 0.0,
            saved_hsl_blue_sat: 0.0,
            saved_hsl_purple_sat: 0.0,
            saved_hsl_magenta_sat: 0.0,
            // HSL saved values
            saved_hsl_red_hue: 0.0,
            saved_hsl_orange_hue: 0.0,
            saved_hsl_yellow_hue: 0.0,
            saved_hsl_green_hue: 0.0,
            saved_hsl_aqua_hue: 0.0,
            saved_hsl_blue_hue: 0.0,
            saved_hsl_purple_hue: 0.0,
            saved_hsl_magenta_hue: 0.0,
            saved_hsl_red_lum: 0.0,
            saved_hsl_orange_lum: 0.0,
            saved_hsl_yellow_lum: 0.0,
            saved_hsl_green_lum: 0.0,
            saved_hsl_aqua_lum: 0.0,
            saved_hsl_blue_lum: 0.0,
            saved_hsl_purple_lum: 0.0,
            saved_hsl_magenta_lum: 0.0,
            saved_lens_distortion: 0.0,
            saved_lens_vignette_amount: 0.0,
            saved_lens_vignette_midpoint: 0.0,
            saved_nr_luminance: 0.0,
            saved_nr_color: 0.0,
            // Sharpening saved values
            saved_sharpen_amount: 0.0,
            saved_sharpen_radius: 1.0,
            folder_tree_roots: Vec::new(),
            expanded_folders: HashSet::new(),
            import_preview_dialog: None,
            import_progress_dialog: None,
            pending_import_preview_receiver: None,
            pending_export_receiver: None,
            toasts: Toasts::default(),
            selected_theme: ThemeVariant::default(),
            library_dock_state: crate::docking::create_library_layout(),
            develop_dock_state: crate::docking::create_develop_layout(),
            show_settings_dialog: false,
            cache_stats: None,
            cache_building_progress: None,
            presets: Vec::new(),
            show_save_preset_dialog: false,
            save_preset_name: String::new(),
            import_dialog: None,
            import_dialog_mode: ImportDialogMode::Simple,
            export_target_id: None,
            // Print view state
            print_view_state: None,
            show_print_dialog: false,
            print_dialog_state: None,
        }
    }


    /// Get the selected photo ID for the current view
    pub fn selected_photo_id(&self) -> Option<&String> {
        match self.current_view {
            CurrentView::Library => self.library_selected_photo_id.as_ref(),
            CurrentView::Develop => self.develop_selected_photo_id.as_ref(),
            CurrentView::Print => self.library_selected_photo_id.as_ref(),
            CurrentView::Import => None,
        }
    }

    /// Get the currently selected photo view model (based on current view)
    pub fn get_current_photo(&self) -> Option<&PhotoViewModel> {
        self.selected_photo_id()
            .and_then(|id| self.photos.iter().find(|p| &p.id == id))
    }

    /// Get the library selected photo view model
    pub fn get_library_photo(&self) -> Option<&PhotoViewModel> {
        self.library_selected_photo_id.as_ref()
            .and_then(|id| self.photos.iter().find(|p| &p.id == id))
    }

    /// Get the develop selected photo view model
    pub fn get_develop_photo(&self) -> Option<&PhotoViewModel> {
        self.develop_selected_photo_id.as_ref()
            .and_then(|id| self.photos.iter().find(|p| &p.id == id))
    }

    /// Navigate to the next or previous photo in develop view
    /// direction: 1 for next, -1 for previous
    /// Returns the new photo ID if navigation was successful
    pub fn navigate_develop(&mut self, direction: i32) -> Option<String> {
        if let Some(current_id) = &self.develop_selected_photo_id {
            if let Some(pos) = self.photos.iter().position(|p| &p.id == current_id) {
                let new_pos = if direction > 0 {
                    (pos + 1).min(self.photos.len().saturating_sub(1))
                } else {
                    pos.saturating_sub(1)
                };

                if new_pos != pos {
                    return self.photos.get(new_pos).map(|p| p.id.clone());
                }
            }
        }
        None
    }

    /// Navigate to the next or previous photo in library view (filmstrip)
    /// direction: 1 for next, -1 for previous
    /// Returns the new photo ID if navigation was successful
    pub fn navigate_library(&mut self, direction: i32) -> Option<String> {
        // Get currently filtered photos
        let filtered: Vec<_> = self.filmstrip_filter.apply(&self.photos);
        
        if filtered.is_empty() {
            return None;
        }
        
        // Find current position in filtered list
        let current_pos = if let Some(current_id) = &self.library_selected_photo_id {
            filtered.iter().position(|p| &p.id == current_id)
        } else {
            None
        };
        
        let new_pos = match current_pos {
            Some(pos) => {
                if direction > 0 {
                    (pos + 1).min(filtered.len().saturating_sub(1))
                } else {
                    pos.saturating_sub(1)
                }
            }
            None => {
                // No current selection, select first or last
                if direction > 0 { 0 } else { filtered.len().saturating_sub(1) }
            }
        };
        
        // Return the new photo ID if it's different
        if current_pos != Some(new_pos) {
            return filtered.get(new_pos).map(|p| p.id.clone());
        }
        
        None
    }

    /// Reset zoom and pan to defaults
    pub fn reset_viewer(&mut self) {
        self.zoom_level = 1.0;
        self.pan_offset = egui::Vec2::ZERO;
    }

    /// Reset all edits to default values
    pub fn reset_edits(&mut self) {
        self.active_exposure = 0.0;
        self.active_contrast = 1.0;
        self.active_temperature = 0.0;
        self.active_tint = 0.0;
        self.active_highlights = 0.0;
        self.active_shadows = 0.0;
        self.active_whites = 0.0;
        self.active_blacks = 0.0;
        self.active_clarity = 0.0;
        self.active_vibrance = 0.0;
        self.active_saturation = 0.0;
        self.active_tone_curve_shadows = 0.0;
        self.active_tone_curve_darks = 0.0;
        self.active_tone_curve_lights = 0.0;
        self.active_tone_curve_highlights = 0.0;
        self.active_hsl_red_sat = 0.0;
        self.active_hsl_orange_sat = 0.0;
        self.active_hsl_yellow_sat = 0.0;
        self.active_hsl_green_sat = 0.0;
        self.active_hsl_aqua_sat = 0.0;
        self.active_hsl_blue_sat = 0.0;
        self.active_hsl_purple_sat = 0.0;
        self.active_hsl_magenta_sat = 0.0;
        // HSL Hue
        self.active_hsl_red_hue = 0.0;
        self.active_hsl_orange_hue = 0.0;
        self.active_hsl_yellow_hue = 0.0;
        self.active_hsl_green_hue = 0.0;
        self.active_hsl_aqua_hue = 0.0;
        self.active_hsl_blue_hue = 0.0;
        self.active_hsl_purple_hue = 0.0;
        self.active_hsl_magenta_hue = 0.0;
        // HSL Lum
        self.active_hsl_red_lum = 0.0;
        self.active_hsl_orange_lum = 0.0;
        self.active_hsl_yellow_lum = 0.0;
        self.active_hsl_green_lum = 0.0;
        self.active_hsl_aqua_lum = 0.0;
        self.active_hsl_blue_lum = 0.0;
        self.active_hsl_purple_lum = 0.0;
        self.active_hsl_magenta_lum = 0.0;
        // Lens
        self.active_lens_distortion = 0.0;
        self.active_lens_vignette_amount = 0.0;
        self.active_lens_vignette_midpoint = 0.0;
        self.active_nr_luminance = 0.0;
        self.active_nr_color = 0.0;
        self.active_sharpen_amount = 0.0;
        self.active_sharpen_radius = 1.0;
    }

    /// Check if we have any photos loaded
    pub fn has_photos(&self) -> bool {
        !self.photos.is_empty()
    }

    /// Get the index of the currently selected photo in develop view
    pub fn develop_photo_index(&self) -> Option<usize> {
        self.develop_selected_photo_id.as_ref()
            .and_then(|id| self.photos.iter().position(|p| &p.id == id))
    }


    /// Push current edit state to history (for undo/redo)
    pub fn push_edit_snapshot(&mut self) {
        let snapshot = EditSnapshot {
            exposure: self.active_exposure,
            contrast: self.active_contrast,
            temperature: self.active_temperature,
            tint: self.active_tint,
            highlights: self.active_highlights,
            shadows: self.active_shadows,
            whites: self.active_whites,
            blacks: self.active_blacks,
            clarity: self.active_clarity,
            vibrance: self.active_vibrance,
            saturation: self.active_saturation,
            tone_curve_shadows: self.active_tone_curve_shadows,
            tone_curve_darks: self.active_tone_curve_darks,
            tone_curve_lights: self.active_tone_curve_lights,
            tone_curve_highlights: self.active_tone_curve_highlights,
            hsl_red_sat: self.active_hsl_red_sat,
            hsl_orange_sat: self.active_hsl_orange_sat,
            hsl_yellow_sat: self.active_hsl_yellow_sat,
            hsl_green_sat: self.active_hsl_green_sat,
            hsl_aqua_sat: self.active_hsl_aqua_sat,
            hsl_blue_sat: self.active_hsl_blue_sat,
            hsl_purple_sat: self.active_hsl_purple_sat,
            hsl_magenta_sat: self.active_hsl_magenta_sat,
            // HSL Hue
            hsl_red_hue: self.active_hsl_red_hue,
            hsl_orange_hue: self.active_hsl_orange_hue,
            hsl_yellow_hue: self.active_hsl_yellow_hue,
            hsl_green_hue: self.active_hsl_green_hue,
            hsl_aqua_hue: self.active_hsl_aqua_hue,
            hsl_blue_hue: self.active_hsl_blue_hue,
            hsl_purple_hue: self.active_hsl_purple_hue,
            hsl_magenta_hue: self.active_hsl_magenta_hue,
            // HSL Lum
            hsl_red_lum: self.active_hsl_red_lum,
            hsl_orange_lum: self.active_hsl_orange_lum,
            hsl_yellow_lum: self.active_hsl_yellow_lum,
            hsl_green_lum: self.active_hsl_green_lum,
            hsl_aqua_lum: self.active_hsl_aqua_lum,
            hsl_blue_lum: self.active_hsl_blue_lum,
            hsl_purple_lum: self.active_hsl_purple_lum,
            hsl_magenta_lum: self.active_hsl_magenta_lum,
            // Lens
            lens_distortion: self.active_lens_distortion,
            lens_vignette_amount: self.active_lens_vignette_amount,
            lens_vignette_midpoint: self.active_lens_vignette_midpoint,
            nr_luminance: self.active_nr_luminance,
            nr_color: self.active_nr_color,
            sharpen_amount: self.active_sharpen_amount,
            sharpen_radius: self.active_sharpen_radius,
        };

        // If we're not at the end of history, truncate everything after current position
        if let Some(index) = self.history_index {
            self.edit_history.truncate(index + 1);
        }

        // Add new snapshot
        self.edit_history.push(snapshot);

        // Limit history to 20 states
        if self.edit_history.len() > 20 {
            self.edit_history.remove(0);
        }

        // Update index to point to the new snapshot
        self.history_index = Some(self.edit_history.len() - 1);
    }

    /// Undo to previous edit state
    pub fn undo(&mut self) -> bool {
        if let Some(index) = self.history_index {
            if index > 0 {
                let new_index = index - 1;
                let snapshot = &self.edit_history[new_index];
                self.active_exposure = snapshot.exposure;
                self.active_contrast = snapshot.contrast;
                self.active_temperature = snapshot.temperature;
                self.active_tint = snapshot.tint;
                self.active_highlights = snapshot.highlights;
                self.active_shadows = snapshot.shadows;
                self.active_whites = snapshot.whites;
                self.active_blacks = snapshot.blacks;
                self.active_clarity = snapshot.clarity;
                self.active_vibrance = snapshot.vibrance;
                self.active_saturation = snapshot.saturation;
                self.active_tone_curve_shadows = snapshot.tone_curve_shadows;
                self.active_tone_curve_darks = snapshot.tone_curve_darks;
                self.active_tone_curve_lights = snapshot.tone_curve_lights;
                self.active_tone_curve_highlights = snapshot.tone_curve_highlights;
                self.active_hsl_red_sat = snapshot.hsl_red_sat;
                self.active_hsl_orange_sat = snapshot.hsl_orange_sat;
                self.active_hsl_yellow_sat = snapshot.hsl_yellow_sat;
                self.active_hsl_green_sat = snapshot.hsl_green_sat;
                self.active_hsl_blue_sat = snapshot.hsl_blue_sat;
                self.active_hsl_purple_sat = snapshot.hsl_purple_sat;
                self.active_hsl_magenta_sat = snapshot.hsl_magenta_sat;
                // HSL Hue
                self.active_hsl_red_hue = snapshot.hsl_red_hue;
                self.active_hsl_orange_hue = snapshot.hsl_orange_hue;
                self.active_hsl_yellow_hue = snapshot.hsl_yellow_hue;
                self.active_hsl_green_hue = snapshot.hsl_green_hue;
                self.active_hsl_aqua_hue = snapshot.hsl_aqua_hue;
                self.active_hsl_blue_hue = snapshot.hsl_blue_hue;
                self.active_hsl_purple_hue = snapshot.hsl_purple_hue;
                self.active_hsl_magenta_hue = snapshot.hsl_magenta_hue;
                // HSL Lum
                self.active_hsl_red_lum = snapshot.hsl_red_lum;
                self.active_hsl_orange_lum = snapshot.hsl_orange_lum;
                self.active_hsl_yellow_lum = snapshot.hsl_yellow_lum;
                self.active_hsl_green_lum = snapshot.hsl_green_lum;
                self.active_hsl_aqua_lum = snapshot.hsl_aqua_lum;
                self.active_hsl_blue_lum = snapshot.hsl_blue_lum;
                self.active_hsl_purple_lum = snapshot.hsl_purple_lum;
                self.active_hsl_magenta_lum = snapshot.hsl_magenta_lum;
                // Lens
                self.active_lens_distortion = snapshot.lens_distortion;
                self.active_lens_vignette_amount = snapshot.lens_vignette_amount;
                self.active_lens_vignette_midpoint = snapshot.lens_vignette_midpoint;
                self.active_hsl_purple_sat = snapshot.hsl_purple_sat;
                self.active_hsl_magenta_sat = snapshot.hsl_magenta_sat;
                self.active_nr_luminance = snapshot.nr_luminance;
                self.active_nr_color = snapshot.nr_color;
                self.history_index = Some(new_index);
                return true;
            }
        }
        false
    }

    /// Redo to next edit state
    pub fn redo(&mut self) -> bool {
        if let Some(index) = self.history_index {
            if index < self.edit_history.len() - 1 {
                let new_index = index + 1;
                let snapshot = &self.edit_history[new_index];
                self.active_exposure = snapshot.exposure;
                self.active_contrast = snapshot.contrast;
                self.active_temperature = snapshot.temperature;
                self.active_tint = snapshot.tint;
                self.active_highlights = snapshot.highlights;
                self.active_shadows = snapshot.shadows;
                self.active_whites = snapshot.whites;
                self.active_blacks = snapshot.blacks;
                self.active_clarity = snapshot.clarity;
                self.active_vibrance = snapshot.vibrance;
                self.active_saturation = snapshot.saturation;
                self.active_tone_curve_shadows = snapshot.tone_curve_shadows;
                self.active_tone_curve_darks = snapshot.tone_curve_darks;
                self.active_tone_curve_lights = snapshot.tone_curve_lights;
                self.active_tone_curve_highlights = snapshot.tone_curve_highlights;
                self.active_hsl_red_sat = snapshot.hsl_red_sat;
                self.active_hsl_orange_sat = snapshot.hsl_orange_sat;
                self.active_hsl_yellow_sat = snapshot.hsl_yellow_sat;
                self.active_hsl_green_sat = snapshot.hsl_green_sat;
                self.active_hsl_aqua_sat = snapshot.hsl_aqua_sat;
                self.active_hsl_blue_sat = snapshot.hsl_blue_sat;
                self.active_hsl_purple_sat = snapshot.hsl_purple_sat;
                self.active_hsl_magenta_sat = snapshot.hsl_magenta_sat;
                self.active_nr_luminance = snapshot.nr_luminance;
                self.active_nr_color = snapshot.nr_color;
                self.history_index = Some(new_index);
                return true;
            }
        }
        false
    }

    // ============================================
    // Multi-Selection Methods
    // ============================================
    
    /// Select all photos (Cmd+A)
    pub fn select_all(&mut self) {
        self.selected_photo_ids = self.photos.iter().map(|p| p.id.clone()).collect();
    }
    
    /// Clear all selections (Escape)
    pub fn clear_selection(&mut self) {
        self.selected_photo_ids.clear();
        self.last_clicked_index = None;
    }
    
    /// Toggle selection of a single photo (Cmd+click)
    pub fn toggle_selection(&mut self, photo_id: &str) {
        if self.selected_photo_ids.contains(photo_id) {
            self.selected_photo_ids.remove(photo_id);
        } else {
            self.selected_photo_ids.insert(photo_id.to_string());
        }
    }
    
    /// Select range from last clicked to current (Shift+click)
    pub fn select_range(&mut self, current_index: usize) {
        if let Some(last_index) = self.last_clicked_index {
            let start = last_index.min(current_index);
            let end = last_index.max(current_index);
            
            for i in start..=end {
                if let Some(photo) = self.photos.get(i) {
                    self.selected_photo_ids.insert(photo.id.clone());
                }
            }
        } else {
            // No previous click, just select this one
            if let Some(photo) = self.photos.get(current_index) {
                self.selected_photo_ids.insert(photo.id.clone());
            }
        }
    }
    
    /// Single select (regular click)
    pub fn single_select(&mut self, photo_id: &str, index: usize) {
        self.selected_photo_ids.clear();
        self.selected_photo_ids.insert(photo_id.to_string());
        self.last_clicked_index = Some(index);
        self.library_selected_photo_id = Some(photo_id.to_string());
    }
    
    /// Check if a photo is selected
    pub fn is_photo_selected(&self, photo_id: &str) -> bool {
        self.selected_photo_ids.contains(photo_id)
    }
    
    /// Get count of selected photos
    pub fn selection_count(&self) -> usize {
        self.selected_photo_ids.len()
    }

    /// Rebuild the folder tree from current photos
    pub fn rebuild_folder_tree(&mut self) {
        use crate::components::folder_tree::FolderNode;
        self.folder_tree_roots = FolderNode::build_tree(&self.photos);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
