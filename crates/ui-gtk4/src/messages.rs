//! Application Messages (Elm Architecture)
//!
//! This module defines all possible messages/actions in the application.
//! Following the Elm Architecture, state changes only happen through messages.

use adapters::view_models::PhotoViewModel;
use domain::value_objects::ColorLabel;

/// All possible messages in the application
#[derive(Debug, Clone)]
pub enum AppMsg {
    // ============================================
    // Navigation
    // ============================================
    /// Switch to Library view
    SwitchToLibrary,
    /// Switch to Develop view
    SwitchToDevelop,
    
    // ============================================
    // Photo Loading
    // ============================================
    /// Load all photos from database
    LoadPhotos,
    /// Photos loaded successfully
    PhotosLoaded(Vec<PhotoViewModel>),
    /// Load presets from database
    LoadPresets,
    /// Presets loaded successfully
    PresetsLoaded(Vec<String>),
    
    // ============================================
    // Import Operations
    // ============================================
    /// Open import dialog
    OpenImportDialog,
    /// Import files selected by user
    ImportFiles(Vec<std::path::PathBuf>),
    /// Import completed
    ImportCompleted(Vec<PhotoViewModel>),
    
    // ============================================
    // Photo Selection
    // ============================================
    /// Select a single photo
    SelectPhoto(String),
    /// Toggle photo selection (Cmd+click)
    TogglePhotoSelection(String),
    /// Select range of photos (Shift+click)
    SelectPhotoRange(usize),
    /// Select all photos
    SelectAll,
    /// Clear selection
    ClearSelection,
    /// Double-click on photo (open in Develop)
    OpenPhotoInDevelop(String),
    
    // ============================================
    // Photo Navigation
    // ============================================
    /// Navigate to next photo
    NavigateNext,
    /// Navigate to previous photo
    NavigatePrevious,
    
    // ============================================
    // Editing Operations
    // ============================================
    /// Set exposure value
    SetExposure(f32),
    /// Set contrast value
    SetContrast(f32),
    /// Set temperature value
    SetTemperature(f32),
    /// Set tint value
    SetTint(f32),
    /// Set highlights value
    SetHighlights(f32),
    /// Set shadows value
    SetShadows(f32),
    /// Set whites value
    SetWhites(f32),
    /// Set blacks value
    SetBlacks(f32),
    /// Set clarity value
    SetClarity(f32),
    /// Set vibrance value
    SetVibrance(f32),
    /// Set saturation value
    SetSaturation(f32),
    /// Reset all adjustments
    ResetAdjustments,
    /// Undo last edit
    Undo,
    /// Redo last undone edit
    Redo,
    /// Toggle before/after comparison
    ToggleBeforeAfter,
    /// Save current edits
    SaveEdits,
    /// Edits saved successfully
    EditsSaved,
    
    // ============================================
    // Rating & Labels
    // ============================================
    /// Set photo rating (0-5)
    SetRating(i32),
    /// Set color label
    SetColorLabel(Option<ColorLabel>),
    /// Set flag (Pick/Reject/None)
    SetFlag(Option<i32>),
    
    // ============================================
    // Filtering
    // ============================================
    /// Filter by minimum rating
    FilterByRating(i32),
    /// Filter by color label
    FilterByColorLabel(Option<ColorLabel>),
    /// Filter by flag
    FilterByFlag(Option<i32>),
    /// Filter by folder path
    FilterByFolder(Option<String>),
    /// Clear all filters
    ClearFilters,
    
    // ============================================
    // Export Operations
    // ============================================
    /// Open export dialog
    OpenExportDialog,
    /// Export photo(s) to path
    ExportPhotos(std::path::PathBuf),
    /// Export completed
    ExportCompleted,
    
    // ============================================
    // Preset Operations
    // ============================================
    /// Apply preset to current photo
    ApplyPreset(String),
    /// Save current settings as preset
    SaveAsPreset(String),
    /// Delete preset
    DeletePreset(String),
    
    // ============================================
    // Dialog Operations
    // ============================================
    /// Open settings dialog
    OpenSettings,
    /// Close current dialog
    CloseDialog,
    
    // ============================================
    // Cache Operations
    // ============================================
    /// Clear thumbnail cache
    ClearThumbnailCache,
    /// Clear preview cache
    ClearPreviewCache,
    /// Clear all cache
    ClearAllCache,
    /// Cache cleared
    CacheCleared,
    
    // ============================================
    // Photo Operations
    // ============================================
    /// Delete selected photo(s)
    DeletePhotos,
    /// Photo(s) deleted
    PhotosDeleted(Vec<String>),
    
    // ============================================
    // Async Results
    // ============================================
    /// Thumbnail loaded for photo
    ThumbnailLoaded { photo_id: String, data: Vec<u8> },
    /// Preview loaded for photo
    PreviewLoaded { photo_id: String, data: Vec<u8> },
    /// Error occurred
    Error(String),
    /// Show toast notification
    ShowToast { message: String, is_error: bool },
    
    // ============================================
    // Window Operations
    // ============================================
    /// Window resized
    WindowResized { width: i32, height: i32 },
    /// Request repaint
    Repaint,
}

/// Messages for async command results
#[derive(Debug)]
pub enum CommandOutput {
    PhotosLoaded(Result<Vec<PhotoViewModel>, String>),
    PresetsLoaded(Result<Vec<String>, String>),
    ImportComplete(Result<Vec<PhotoViewModel>, String>),
    EditsSaved(Result<(), String>),
    ExportComplete(Result<(), String>),
    ThumbnailReady { photo_id: String, data: Vec<u8> },
    PreviewReady { photo_id: String, data: Vec<u8> },
}
