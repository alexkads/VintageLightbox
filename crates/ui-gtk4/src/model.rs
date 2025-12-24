//! Application Model (Elm Architecture)
//!
//! This module defines the application state following the Elm Architecture.
//! The model is the single source of truth for the entire UI state.

use std::collections::HashSet;
use std::sync::Arc;
use adapters::controllers::{
    ImportController, LibraryController, EditorController,
    ExportController, PhotoController, PresetController,
};
use adapters::view_models::PhotoViewModel;
use domain::value_objects::ColorLabel;
use infrastructure::cache::preview_manager::PreviewManager;

/// Initialization data passed to the app
pub struct AppInit {
    pub import_controller: Arc<ImportController>,
    pub library_controller: Arc<LibraryController>,
    pub editor_controller: Arc<EditorController>,
    pub export_controller: Arc<ExportController>,
    pub photo_controller: Arc<PhotoController>,
    pub preset_controller: Arc<PresetController>,
    pub preview_manager: Arc<PreviewManager>,
    pub runtime: Arc<tokio::runtime::Runtime>,
}

/// Current view in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurrentView {
    #[default]
    Library,
    Develop,
}

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
}

impl Default for EditSnapshot {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 0.0,
            temperature: 0.0,
            tint: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            clarity: 0.0,
            vibrance: 0.0,
            saturation: 0.0,
        }
    }
}

/// Filter state for the photo library
#[derive(Debug, Clone, Default)]
pub struct FilterState {
    pub min_rating: i32,
    pub color_label: Option<ColorLabel>,
    pub flag: Option<i32>,
    pub folder_path: Option<String>,
}

/// Main application model (state)
pub struct AppModel {
    // ============================================
    // Controllers (Clean Architecture adapters)
    // ============================================
    pub import_controller: Arc<ImportController>,
    pub library_controller: Arc<LibraryController>,
    pub editor_controller: Arc<EditorController>,
    pub export_controller: Arc<ExportController>,
    pub photo_controller: Arc<PhotoController>,
    pub preset_controller: Arc<PresetController>,
    
    // ============================================
    // Infrastructure
    // ============================================
    pub preview_manager: Arc<PreviewManager>,
    pub runtime: Arc<tokio::runtime::Runtime>,
    
    // ============================================
    // Navigation State
    // ============================================
    pub current_view: CurrentView,
    
    // ============================================
    // Photo Data
    // ============================================
    pub photos: Vec<PhotoViewModel>,
    pub presets: Vec<String>,
    
    // ============================================
    // Selection State
    // ============================================
    /// Currently selected photo ID (for single selection context)
    pub selected_photo_id: Option<String>,
    /// All selected photo IDs (for multi-selection)
    pub selected_photo_ids: HashSet<String>,
    /// Last selected index (for shift-click range selection)
    pub last_selected_index: Option<usize>,
    /// Photo ID selected in Develop view (independent from Library)
    pub develop_photo_id: Option<String>,
    
    // ============================================
    // Editing State
    // ============================================
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
    
    // ============================================
    // Undo/Redo
    // ============================================
    pub edit_history: Vec<EditSnapshot>,
    pub history_index: usize,
    pub max_history_size: usize,
    
    // ============================================
    // Before/After Comparison
    // ============================================
    pub show_before: bool,
    pub before_snapshot: Option<EditSnapshot>,
    
    // ============================================
    // Filtering
    // ============================================
    pub filter: FilterState,
    
    // ============================================
    // UI State
    // ============================================
    pub is_loading: bool,
    pub loading_message: Option<String>,
    pub toast_message: Option<String>,
    pub toast_is_error: bool,
    
    // ============================================
    // Dialogs
    // ============================================
    pub show_import_dialog: bool,
    pub show_export_dialog: bool,
    pub show_settings_dialog: bool,
}

impl AppModel {
    /// Create a new AppModel from initialization data
    pub fn new(init: AppInit) -> Self {
        Self {
            // Controllers
            import_controller: init.import_controller,
            library_controller: init.library_controller,
            editor_controller: init.editor_controller,
            export_controller: init.export_controller,
            photo_controller: init.photo_controller,
            preset_controller: init.preset_controller,
            
            // Infrastructure
            preview_manager: init.preview_manager,
            runtime: init.runtime,
            
            // Navigation
            current_view: CurrentView::Library,
            
            // Data
            photos: Vec::new(),
            presets: Vec::new(),
            
            // Selection
            selected_photo_id: None,
            selected_photo_ids: HashSet::new(),
            last_selected_index: None,
            develop_photo_id: None,
            
            // Editing
            exposure: 0.0,
            contrast: 0.0,
            temperature: 0.0,
            tint: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            clarity: 0.0,
            vibrance: 0.0,
            saturation: 0.0,
            
            // History
            edit_history: Vec::new(),
            history_index: 0,
            max_history_size: 20,
            
            // Before/After
            show_before: false,
            before_snapshot: None,
            
            // Filtering
            filter: FilterState::default(),
            
            // UI State
            is_loading: false,
            loading_message: None,
            toast_message: None,
            toast_is_error: false,
            
            // Dialogs
            show_import_dialog: false,
            show_export_dialog: false,
            show_settings_dialog: false,
        }
    }
    
    /// Get the currently selected photo for the current view
    pub fn get_current_photo(&self) -> Option<&PhotoViewModel> {
        let photo_id = match self.current_view {
            CurrentView::Library => self.selected_photo_id.as_ref(),
            CurrentView::Develop => self.develop_photo_id.as_ref(),
        };
        
        photo_id.and_then(|id| self.photos.iter().find(|p| &p.id == id))
    }
    
    /// Get filtered photos based on current filter settings
    pub fn get_filtered_photos(&self) -> Vec<&PhotoViewModel> {
        self.photos.iter().filter(|photo| {
            // Rating filter
            if photo.rating < self.filter.min_rating {
                return false;
            }
            
            // Color label filter
            if let Some(ref label) = self.filter.color_label {
                if photo.color_label.as_ref() != Some(&label.to_string()) {
                    return false;
                }
            }
            
            // Flag filter
            if let Some(flag) = self.filter.flag {
                if photo.flag != Some(flag) {
                    return false;
                }
            }
            
            // Folder filter
            if let Some(ref folder) = self.filter.folder_path {
                if !photo.path.starts_with(folder) {
                    return false;
                }
            }
            
            true
        }).collect()
    }
    
    /// Create a snapshot of current edit state
    pub fn create_edit_snapshot(&self) -> EditSnapshot {
        EditSnapshot {
            exposure: self.exposure,
            contrast: self.contrast,
            temperature: self.temperature,
            tint: self.tint,
            highlights: self.highlights,
            shadows: self.shadows,
            whites: self.whites,
            blacks: self.blacks,
            clarity: self.clarity,
            vibrance: self.vibrance,
            saturation: self.saturation,
        }
    }
    
    /// Apply an edit snapshot to current state
    pub fn apply_edit_snapshot(&mut self, snapshot: &EditSnapshot) {
        self.exposure = snapshot.exposure;
        self.contrast = snapshot.contrast;
        self.temperature = snapshot.temperature;
        self.tint = snapshot.tint;
        self.highlights = snapshot.highlights;
        self.shadows = snapshot.shadows;
        self.whites = snapshot.whites;
        self.blacks = snapshot.blacks;
        self.clarity = snapshot.clarity;
        self.vibrance = snapshot.vibrance;
        self.saturation = snapshot.saturation;
    }
    
    /// Push current state to undo history
    pub fn push_to_history(&mut self) {
        let snapshot = self.create_edit_snapshot();
        
        // Remove any redo states
        self.edit_history.truncate(self.history_index);
        
        // Add new snapshot
        self.edit_history.push(snapshot);
        self.history_index = self.edit_history.len();
        
        // Limit history size
        if self.edit_history.len() > self.max_history_size {
            self.edit_history.remove(0);
            self.history_index = self.edit_history.len();
        }
    }
    
    /// Undo to previous state
    pub fn undo(&mut self) -> bool {
        if self.history_index > 0 {
            self.history_index -= 1;
            let snapshot = self.edit_history[self.history_index].clone();
            self.apply_edit_snapshot(&snapshot);
            true
        } else {
            false
        }
    }
    
    /// Redo to next state
    pub fn redo(&mut self) -> bool {
        if self.history_index < self.edit_history.len() {
            let snapshot = self.edit_history[self.history_index].clone();
            self.apply_edit_snapshot(&snapshot);
            self.history_index += 1;
            true
        } else {
            false
        }
    }
    
    /// Reset all adjustments to defaults
    pub fn reset_adjustments(&mut self) {
        self.push_to_history();
        self.exposure = 0.0;
        self.contrast = 0.0;
        self.temperature = 0.0;
        self.tint = 0.0;
        self.highlights = 0.0;
        self.shadows = 0.0;
        self.whites = 0.0;
        self.blacks = 0.0;
        self.clarity = 0.0;
        self.vibrance = 0.0;
        self.saturation = 0.0;
    }
    
    /// Navigate to next photo in develop view
    pub fn navigate_next(&mut self) -> bool {
        if let Some(ref current_id) = self.develop_photo_id {
            let filtered = self.get_filtered_photos();
            if let Some(idx) = filtered.iter().position(|p| &p.id == current_id) {
                if idx + 1 < filtered.len() {
                    self.develop_photo_id = Some(filtered[idx + 1].id.clone());
                    return true;
                }
            }
        }
        false
    }
    
    /// Navigate to previous photo in develop view
    pub fn navigate_previous(&mut self) -> bool {
        if let Some(ref current_id) = self.develop_photo_id {
            let filtered = self.get_filtered_photos();
            if let Some(idx) = filtered.iter().position(|p| &p.id == current_id) {
                if idx > 0 {
                    self.develop_photo_id = Some(filtered[idx - 1].id.clone());
                    return true;
                }
            }
        }
        false
    }
    
    /// Select all photos
    pub fn select_all(&mut self) {
        self.selected_photo_ids = self.photos.iter().map(|p| p.id.clone()).collect();
    }
    
    /// Clear all selections
    pub fn clear_selection(&mut self) {
        self.selected_photo_ids.clear();
        self.selected_photo_id = None;
        self.last_selected_index = None;
    }
    
    /// Check if a photo is selected
    pub fn is_photo_selected(&self, photo_id: &str) -> bool {
        self.selected_photo_ids.contains(photo_id)
    }
    
    /// Get selection count
    pub fn selection_count(&self) -> usize {
        self.selected_photo_ids.len()
    }
}
