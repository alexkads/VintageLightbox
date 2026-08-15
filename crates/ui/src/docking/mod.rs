//! Docking Module
//!
//! Provides egui_dock integration for VintageLightbox,
//! enabling flexible, rearrangeable panel layouts.

pub mod dock_tab;
pub mod dock_viewer;

pub use dock_tab::DockTab;
pub use dock_viewer::{DockViewer, DockViewerContext};

use egui_dock::{DockState, NodeIndex};

/// Create default Library layout (photo grid focused)
///
/// Layout:
/// ```text
/// ┌─────────────┬─────────────────────────┬──────────────┐
/// │  Folders    │                         │  Histogram   │
/// │  Filters    │      PhotoGrid          │  QuickDev    │
/// │  Grid       │                         │  Metadata    │
/// ├─────────────┴─────────────────────────┴──────────────┤
/// │                    Filmstrip                         │
/// └──────────────────────────────────────────────────────┘
/// ```
pub fn create_library_layout() -> DockState<DockTab> {
    // Start with the main content panel
    let mut dock_state = DockState::new(vec![DockTab::PhotoGrid]);
    let tree = dock_state.main_surface_mut();

    // Split off left sidebar (20% width)
    let [left_sidebar, _center_and_right] =
        tree.split_left(NodeIndex::root(), 0.18, vec![DockTab::Folders]);

    // Add more tabs to left sidebar
    tree.set_focused_node(left_sidebar);
    tree.push_to_focused_leaf(DockTab::GridSettings);

    // Split off right sidebar from center (80% center, 20% right)
    let [center, right_sidebar] =
        tree.split_right(NodeIndex::root(), 0.75, vec![DockTab::Histogram]);

    // Add more tabs to right sidebar
    tree.set_focused_node(right_sidebar);
    tree.push_to_focused_leaf(DockTab::QuickDevelop);
    tree.push_to_focused_leaf(DockTab::Metadata);

    // Split off filmstrip at bottom (85% main area, 15% filmstrip)
    let [_main, _filmstrip] = tree.split_below(center, 0.85, vec![DockTab::Filmstrip]);

    dock_state
}

/// Create default Develop layout (image editing focused)
///
/// Layout:
/// ```text
/// ┌─────────────┬─────────────────────────┬──────────────┐
/// │  Presets    │                         │  Histogram   │
/// │             │      ImageViewer        │  Adjustments │
/// │             │                         │  (all in 1)  │
/// ├─────────────┴─────────────────────────┴──────────────┤
/// │                    Filmstrip                         │
/// └──────────────────────────────────────────────────────┘
/// ```
pub fn create_develop_layout() -> DockState<DockTab> {
    // Start with the main image viewer
    let mut dock_state = DockState::new(vec![DockTab::ImageViewer]);
    let tree = dock_state.main_surface_mut();

    // Split off left sidebar with presets (18% width)
    let [_left_sidebar, _center_and_right] =
        tree.split_left(NodeIndex::root(), 0.18, vec![DockTab::Presets]);

    // Split off right sidebar with histogram at top and adjustments below
    let [center, right_sidebar] =
        tree.split_right(NodeIndex::root(), 0.70, vec![DockTab::Histogram]);

    // Split the right sidebar to have Histogram at top and AllAdjustments below
    let [_histogram, _adjustments] =
        tree.split_below(right_sidebar, 0.20, vec![DockTab::AllAdjustments]);

    // Split off filmstrip at bottom
    let [_main, _filmstrip] = tree.split_below(center, 0.85, vec![DockTab::Filmstrip]);

    dock_state
}
