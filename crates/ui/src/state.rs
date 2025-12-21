#![allow(dead_code)]

// Application State Management
// Replaces Slint's declarative properties with Rust state struct

use adapters::view_models::PhotoViewModel;
use image::DynamicImage;
use std::sync::{Arc, Mutex};

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
    /// Previous exposure value (for change detection)
    pub prev_exposure: f32,
    /// Previous contrast value (for change detection)
    pub prev_contrast: f32,
    /// Original unprocessed preview image
    pub original_preview: Option<DynamicImage>,

    // ============================================
    // Image Viewer State
    // ============================================
    pub zoom_level: f32,
    pub pan_offset: egui::Vec2,

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
            prev_exposure: 0.0,
            prev_contrast: 1.0,
            original_preview: None,
            zoom_level: 1.0,
            pan_offset: egui::Vec2::ZERO,
            is_busy: false,
            busy_message: String::new(),
            filter_min_rating: 0,
            filter_color_label: None,
            pending_import: None,
            pending_export: None,
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
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
