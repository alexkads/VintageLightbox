#![allow(dead_code)]

// Application State Management
// Replaces Slint's declarative properties with Rust state struct

use adapters::view_models::PhotoViewModel;
use image::DynamicImage;
use std::sync::{Arc, Mutex};

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

/// Current view in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentView {
    Library,
    Develop,
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

/// Main application state
pub struct AppState {
    // ============================================
    // Navigation
    // ============================================
    pub current_view: CurrentView,

    // ============================================
    // Photo Library
    // ============================================
    pub photos: Vec<PhotoViewModel>,
    pub selected_photo_id: Option<String>,
    /// ID of the photo currently loaded in detail_image (for change detection)
    pub loaded_photo_id: Option<String>,

    // ============================================
    // Detail View
    // ============================================
    /// Texture handle for the currently displayed image
    pub detail_image: Option<egui::TextureHandle>,
    /// Metadata of the currently selected photo
    pub detail_metadata: Option<DetailMetadata>,
    /// Histogram data for the current image
    pub histogram_data: Option<crate::components::histogram::HistogramData>,

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
    /// Original unprocessed preview image
    pub original_preview: Option<DynamicImage>,

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

    // ============================================
    // Filters
    // ============================================
    pub filter_min_rating: i32,
    pub filter_color_label: Option<String>,

    // ============================================
    // Async Operations
    // ============================================
    pub pending_import: Option<poll_promise::Promise<Result<(), String>>>,
    pub pending_export: Option<poll_promise::Promise<Result<(), String>>>,

    // ============================================
    // Undo/Redo History
    // ============================================
    pub edit_history: Vec<EditSnapshot>,
    pub history_index: Option<usize>, // Current position in history (None = no history)
}

impl AppState {
    pub fn new() -> Self {
        Self {
            current_view: CurrentView::Library,
            photos: Vec::new(),
            selected_photo_id: None,
            loaded_photo_id: None,
            detail_image: None,
            detail_metadata: None,
            histogram_data: None,
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
            original_preview: None,
            zoom_level: 1.0,
            pan_offset: egui::Vec2::ZERO,
            show_before: false,
            prev_show_before: false,
            is_busy: false,
            busy_message: String::new(),
            filter_min_rating: 0,
            filter_color_label: None,
            pending_import: None,
            pending_export: None,
            edit_history: Vec::new(),
            history_index: None,
        }
    }

    /// Get the currently selected photo view model
    pub fn get_current_photo(&self) -> Option<&PhotoViewModel> {
        self.selected_photo_id.as_ref()
            .and_then(|id| self.photos.iter().find(|p| &p.id == id))
    }

    /// Navigate to the next or previous photo
    /// direction: 1 for next, -1 for previous
    /// Returns the new photo ID if navigation was successful
    pub fn navigate(&mut self, direction: i32) -> Option<String> {
        if let Some(current_id) = &self.selected_photo_id {
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

    /// Reset zoom and pan to defaults
    pub fn reset_viewer(&mut self) {
        self.zoom_level = 1.0;
        self.pan_offset = egui::Vec2::ZERO;
    }

    /// Check if we have any photos loaded
    pub fn has_photos(&self) -> bool {
        !self.photos.is_empty()
    }

    /// Get the index of the currently selected photo
    pub fn current_photo_index(&self) -> Option<usize> {
        self.selected_photo_id.as_ref()
            .and_then(|id| self.photos.iter().position(|p| &p.id == id))
    }

    /// Get filtered photos based on current filter settings
    pub fn get_filtered_photos(&self) -> Vec<PhotoViewModel> {
        self.photos.iter()
            .filter(|photo| {
                // Filter by minimum rating
                if photo.rating < self.filter_min_rating {
                    return false;
                }

                // Filter by color label
                if let Some(ref filter_label) = self.filter_color_label {
                    if photo.color_label.as_ref() != Some(filter_label) {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect()
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
                self.history_index = Some(new_index);
                return true;
            }
        }
        false
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
