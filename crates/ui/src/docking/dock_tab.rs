//! DockTab - Enum representing all dockable tabs
//! 
//! Each variant represents a tab that can be docked, moved, or closed.

use std::fmt;
use serde::{Serialize, Deserialize};

/// All possible dockable tabs in VintageLightbox
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DockTab {
    // ============================================
    // Main Content Areas
    // ============================================
    /// Photo grid view (Library mode)
    PhotoGrid,
    /// Full image viewer (Develop mode)
    ImageViewer,
    
    // ============================================
    // Left Sidebar Panels
    // ============================================
    /// Folder tree navigation
    Folders,
    /// Collections panel
    Collections,
    /// Grid view settings
    GridSettings,
    
    // ============================================
    // Right Sidebar Panels
    // ============================================
    /// Histogram display
    Histogram,
    /// Quick develop shortcuts
    QuickDevelop,
    /// Photo metadata display
    Metadata,
    /// Basic adjustments sliders
    BasicAdjustments,
    /// Tone curve controls
    ToneCurve,
    /// HSL / Color (Saturation) controls
    HSLColor,
    /// HSL / Hue controls
    HSLHue,
    /// HSL / Luminance controls
    HSLLuminance,
    /// Lens Corrections controls
    LensCorrections,
    /// Detail (Noise Reduction & Sharpening) controls
    Detail,
    /// All adjustments combined in collapsible sections
    AllAdjustments,
    /// Presets panel
    Presets,
    
    // ============================================
    // Bottom Panels
    // ============================================
    /// Filmstrip thumbnail bar
    Filmstrip,
}

impl fmt::Display for DockTab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DockTab::PhotoGrid => write!(f, "Photo Grid"),
            DockTab::ImageViewer => write!(f, "Image Viewer"),
            DockTab::Folders => write!(f, "Folders"),
            DockTab::Collections => write!(f, "Collections"),
            DockTab::GridSettings => write!(f, "Grid Settings"),
            DockTab::Histogram => write!(f, "Histogram"),
            DockTab::QuickDevelop => write!(f, "Quick Develop"),
            DockTab::Metadata => write!(f, "Metadata"),
            DockTab::BasicAdjustments => write!(f, "Basic"),
            DockTab::ToneCurve => write!(f, "Tone Curve"),
            DockTab::HSLColor => write!(f, "HSL / Color"),
            DockTab::HSLHue => write!(f, "HSL / Hue"),
            DockTab::HSLLuminance => write!(f, "HSL / Luminance"),
            DockTab::LensCorrections => write!(f, "Lens Corrections"),
            DockTab::Detail => write!(f, "Detail"),
            DockTab::AllAdjustments => write!(f, "Adjustments"),
            DockTab::Presets => write!(f, "Presets"),
            DockTab::Filmstrip => write!(f, "Filmstrip"),
        }
    }
}

impl DockTab {
    /// Check if this tab is a main content tab (PhotoGrid or ImageViewer)
    pub fn is_main_content(&self) -> bool {
        matches!(self, DockTab::PhotoGrid | DockTab::ImageViewer)
    }
    
    /// Check if this tab should be closeable
    pub fn is_closeable(&self) -> bool {
        // Main content tabs cannot be closed
        !self.is_main_content()
    }
}
