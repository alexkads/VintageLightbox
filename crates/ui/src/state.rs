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
// Replaced by adapters::state::PhotoFilters within internal_state
// use crate::components::filmstrip_filter::FilmstripFilter;
use adapters::state::ApplicationState; 
pub use adapters::state::CurrentView; // View enum now from adapters, re-exported


// EditSnapshot removed - using domain::value_objects::PhotoEdits directly
// History management is now handled by adapters::services::EditorService

// CurrentView enum removed - using adapters::state::CurrentView

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
    // Core State (Adapters)
    // ============================================
    pub internal_state: ApplicationState,

    // ============================================
    // Navigation (Managed by internal_state/ApplicationState)
    // ============================================
    // pub current_view: CurrentView, // Removed

    pub import_view_state: ImportViewState,

    // ============================================
    // Photo Library
    // ============================================
    pub photos: Vec<PhotoViewModel>,
    /// Selected photo in Library view (Managed by internal_state)
    // pub library_selected_photo_id: Option<String>,
    /// Selected photo in Develop view (Managed by internal_state)
    // pub develop_selected_photo_id: Option<String>,

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
    // active_* fields REMOVED - Using EditorService
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
    // Crop Tool State (Develop mode only)
    // ============================================
    pub crop_mode_active: bool,
    pub crop_settings: Option<domain::value_objects::CropSettings>,
    pub selected_aspect_ratio: domain::value_objects::AspectRatio,
    pub show_composition_grid: bool,

    // ============================================
    // Intelligent Fill State
    // ============================================
    /// Cached intelligent fill result texture
    pub intelligent_fill_texture: Option<egui::TextureHandle>,
    /// Current intelligent fill request ID (for debouncing)
    pub intelligent_fill_request_id: u64,
    /// Whether intelligent fill is processing
    pub intelligent_fill_pending: bool,
    /// Previous angle used for intelligent fill (to detect changes)
    pub prev_intelligent_fill_angle: f32,

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
    // Filmstrip specific filter (Managed by internal_state/ApplicationState)
    // pub filmstrip_filter: FilmstripFilter, // Removed

    // ============================================
    // Async Operations
    // ============================================
    pub pending_import: Option<poll_promise::Promise<Result<(), String>>>,
    pub pending_export: Option<poll_promise::Promise<Result<(), String>>>,
    /// Receiver for export results (path of exported file or error)
    pub pending_export_receiver: Option<tokio::sync::mpsc::Receiver<Result<String, String>>>,


    // ============================================
    // Undo/Redo History - REMOVED
    // ============================================
    // History management is now handled by adapters::services::EditorService
    // pub edit_history: Vec<EditSnapshot>,
    // pub history_index: Option<usize>,

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
    /// Flag to apply crop settings (set by CropPanel Apply button)
    pub pending_crop_apply: bool,
    /// Flag to request save and switch to Library mode (set by Escape key, handled by app.rs)
    pub deferred_exit_develop_mode: bool,
    /// Timestamp of the last slider change (for debounce)
    pub last_slider_change_time: Option<std::time::Instant>,
    /// Last saved values (to avoid unnecessary saves)
    // saved_* fields REMOVED - Using EditorService

    // ============================================
    // Folder Navigation
    // ============================================
    /// Root nodes of the folder tree
    pub folder_tree_roots: Vec<adapters::view_models::FolderNode>,
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
    /// Print dialog state (legacy - kept for compatibility)
    pub print_dialog_state: Option<crate::components::print_dialog::PrintDialogState>,
    
    // Invalidation Queue for Thumbnails
    pub invalidation_queue: HashSet<String>,
    
    // ============================================
    // Change Detection (EditorService Integration)
    // ============================================
    /// Last processed edits (for efficient change detection)
    /// Replaces individual prev_* field comparisons
    pub last_processed_edits: domain::value_objects::PhotoEdits,
    /// Last saved edits (for auto-save comparison)
    /// Replaces individual saved_* field comparisons
    pub last_saved_edits: domain::value_objects::PhotoEdits,
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
        let mut internal_state = ApplicationState::new();
        // Ensure default view is Library
        internal_state.current_view = CurrentView::Library;

        Self {
            internal_state,
            // current_view: CurrentView::Library,
            import_view_state: ImportViewState::default(),
            photos: Vec::new(),
            // library_selected_photo_id: None,
            // develop_selected_photo_id: None,
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
            // active_* initialization REMOVED - Using EditorService

            original_preview: None,
            original_image_data: None,
            zoom_level: 1.0,

            pan_offset: egui::Vec2::ZERO,
            request_toggle_secondary_window: false,
            show_before: false,
            prev_show_before: false,
            // Crop state
            crop_mode_active: false,
            crop_settings: None,
            selected_aspect_ratio: domain::value_objects::AspectRatio::Original,
            show_composition_grid: false,
            // Intelligent fill state
            intelligent_fill_texture: None,
            intelligent_fill_request_id: 0,
            intelligent_fill_pending: false,
            prev_intelligent_fill_angle: 0.0,
            is_busy: false,
            busy_message: String::new(),
            grid_columns: 4,  // Default 4 columns
            // filmstrip_filter: FilmstripFilter::new(),
            pending_import: None,
            pending_export: None,
            // edit_history: Vec::new(), // REMOVED - using EditorService
            // history_index: None,       // REMOVED - using EditorService
            performance_metrics: PerformanceMetrics::default(),
            show_performance_stats: std::env::var("SHOW_PERFORMANCE_STATS").is_ok_and(|v| v == "true"),
            start_load_time: None,
            pending_auto_save: false,
            pending_crop_apply: false,
            deferred_exit_develop_mode: false,
            last_slider_change_time: None,
            // saved_* initialization REMOVED - Using EditorService


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
            invalidation_queue: HashSet::new(),
            last_processed_edits: domain::value_objects::PhotoEdits::default(),
            last_saved_edits: domain::value_objects::PhotoEdits::default(),
        }
    }


    /// Get the selected photo ID for the current view
    pub fn selected_photo_id(&self) -> Option<&String> {
        match self.internal_state.current_view {
            CurrentView::Library => self.internal_state.library_selected_id.as_ref(),
            CurrentView::Develop => self.internal_state.develop_selected_id.as_ref(),
            CurrentView::Print => self.internal_state.library_selected_id.as_ref(),
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
        self.internal_state.library_selected_id.as_ref()
            .and_then(|id| self.photos.iter().find(|p| &p.id == id))
    }

    /// Get the develop selected photo view model
    pub fn get_develop_photo(&self) -> Option<&PhotoViewModel> {
        self.internal_state.develop_selected_id.as_ref()
            .and_then(|id| self.photos.iter().find(|p| &p.id == id))
    }

    /// Navigate to the next or previous photo in develop view
    /// direction: 1 for next, -1 for previous
    /// Returns the new photo ID if navigation was successful
    pub fn navigate_develop(&mut self, direction: i32) -> Option<String> {
        if let Some(current_id) = &self.internal_state.develop_selected_id {
            if let Some(pos) = self.photos.iter().position(|p| &p.id == current_id) {
                let new_pos = if direction > 0 {
                    (pos + 1).min(self.photos.len().saturating_sub(1))
                } else {
                    pos.saturating_sub(1)
                };

                if new_pos != pos {
                    let new_id = self.photos.get(new_pos).map(|p| p.id.clone());
                    if let Some(ref id) = new_id {
                        self.internal_state.develop_selected_id = Some(id.clone());
                    }
                    return new_id;
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
        let filtered: Vec<_> = self.internal_state.photo_filters.apply(&self.photos);
        
        if filtered.is_empty() {
            return None;
        }
        
        // Find current position in filtered list
        let current_pos = if let Some(current_id) = &self.internal_state.library_selected_id {
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
            let new_id = filtered.get(new_pos).map(|p| p.id.clone());
            if let Some(ref id) = new_id {
                self.internal_state.library_selected_id = Some(id.clone());
            }
            return new_id;
        }
        
        None
    }

    /// Ensure the selected photo in develop view is valid according to current filters
    /// If the current photo is filtered out, select the next available one.
    pub fn sanitize_develop_selection(&mut self) {
        // Apply filters
        let filtered = self.internal_state.photo_filters.apply(&self.photos);
        
        let should_change = if let Some(current_id) = &self.internal_state.develop_selected_id {
            // Check if current ID is in filtered list
            !filtered.iter().any(|p| &p.id == current_id)
        } else {
            // No selection, should select first if available
            !filtered.is_empty()
        };

        if should_change {
             self.internal_state.develop_selected_id = filtered.first().map(|p| p.id.clone());
             // Force reload
             self.loaded_photo_id = None;
        }
    }

    /// Reset zoom and pan to defaults
    pub fn reset_viewer(&mut self) {
        self.zoom_level = 1.0;
        self.pan_offset = egui::Vec2::ZERO;
    }

    // ============================================
    // Business Logic Methods - REMOVED
    // ============================================
    // The following methods have been removed as they duplicate EditorService functionality:
    // - reset_edits() -> use EditorService::reset()
    // - push_edit_snapshot() -> EditorService handles history automatically  
    // - undo() -> use EditorService::undo()
    // - redo() -> use EditorService::redo()

    /// Check if we have any photos loaded
    pub fn has_photos(&self) -> bool {
        !self.photos.is_empty()
    }

    /// Get the index of the currently selected photo in develop view
    pub fn develop_photo_index(&self) -> Option<usize> {
        self.internal_state.develop_selected_id.as_ref()
            .and_then(|id| self.photos.iter().position(|p| &p.id == id))
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
        self.internal_state.library_selected_id = Some(photo_id.to_string());
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
        use adapters::view_models::FolderNode;
        self.folder_tree_roots = FolderNode::build_tree(&self.photos);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adapters::view_models::PhotoViewModel;
    use adapters::state::CurrentView;

    fn create_test_photo(id: &str) -> PhotoViewModel {
        PhotoViewModel {
            id: id.to_string(),
            path: format!("/tmp/{}.jpg", id),
            name: format!("{}.jpg", id),
            thumbnail_path: None,
            date: "2024-01-01".to_string(),
            camera: "Test Camera".to_string(),
            exposure: "1/100".to_string(),
            rating: 0,
            color_label: None,
            flag: None,
            width: Some(100),
            height: Some(100),
            file_missing: false,
            // Initialize optional edit fields to None using default
            ..Default::default()
        }
    }

    #[test]
    fn test_single_select() {
        let mut state = AppState::new();
        state.photos = vec![
            create_test_photo("1"),
            create_test_photo("2"),
        ];

        state.single_select("1", 0);

        assert!(state.selected_photo_ids.contains("1"));
        assert!(!state.selected_photo_ids.contains("2"));
        assert_eq!(state.internal_state.library_selected_id, Some("1".to_string()));
        assert_eq!(state.last_clicked_index, Some(0));
    }

    #[test]
    fn test_toggle_selection() {
        let mut state = AppState::new();
        state.photos = vec![
            create_test_photo("1"),
            create_test_photo("2"),
        ];

        state.toggle_selection("1");
        assert!(state.selected_photo_ids.contains("1"));

        state.toggle_selection("2");
        assert!(state.selected_photo_ids.contains("1"));
        assert!(state.selected_photo_ids.contains("2"));

        state.toggle_selection("1");
        assert!(!state.selected_photo_ids.contains("1"));
        assert!(state.selected_photo_ids.contains("2"));
    }

    #[test]
    fn test_navigate_develop() {
        let mut state = AppState::new();
        state.photos = vec![
            create_test_photo("1"),
            create_test_photo("2"),
            create_test_photo("3"),
        ];
        state.internal_state.current_view = CurrentView::Develop;
        state.internal_state.develop_selected_id = Some("2".to_string());

        // Next
        let next = state.navigate_develop(1);
        assert_eq!(next, Some("3".to_string()));
        assert_eq!(state.internal_state.develop_selected_id, Some("3".to_string()));

        // Previous
        let prev = state.navigate_develop(-1);
        assert_eq!(prev, Some("2".to_string()));
        assert_eq!(state.internal_state.develop_selected_id, Some("2".to_string()));

        // Bounds check (Next at end)
        state.navigate_develop(1); // to 3
        let none = state.navigate_develop(1); // to 4? no
        assert_eq!(none, None);
        assert_eq!(state.internal_state.develop_selected_id, Some("3".to_string()));
    }
}
