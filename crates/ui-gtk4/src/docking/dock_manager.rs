//! Dock Manager
//!
//! Manages the panel layout using GTK4 Paned and Notebook widgets.

use gtk4::prelude::*;
use gtk4::{Box, Orientation, Paned, Notebook, Label, Widget};

/// Panel position in the dock
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPosition {
    Left,
    Center,
    Right,
    Bottom,
}

/// Dock Manager for organizing panels
#[allow(dead_code)]
pub struct DockManager {
    root: Box,
    main_paned: Paned,
    left_panel: Option<Notebook>,
    center_panel: Box,
    right_panel: Option<Notebook>,
    bottom_paned: Option<Paned>,
    bottom_panel: Option<Box>,
}

impl DockManager {
    /// Create a new Library layout
    ///
    /// Layout:
    /// ```
    /// ┌─────────┬──────────────┬──────────┐
    /// │  Left   │    Center    │  Right   │
    /// │ Folders │  PhotoGrid   │Histogram │
    /// │ Filters │              │ Metadata │
    /// ├─────────┴──────────────┴──────────┤
    /// │           Filmstrip                │
    /// └────────────────────────────────────┘
    /// ```
    pub fn new_library() -> Self {
        // Root container (vertical)
        let root = Box::new(Orientation::Vertical, 0);

        // Main horizontal paned (contains left, center, right)
        let main_paned = Paned::new(Orientation::Horizontal);
        main_paned.set_shrink_start_child(false);
        main_paned.set_shrink_end_child(false);
        main_paned.set_resize_start_child(false);
        main_paned.set_resize_end_child(true);
        main_paned.set_position(250); // Left panel width

        // Left panel (Notebook with tabs)
        let left_panel = Notebook::new();
        left_panel.set_tab_pos(gtk4::PositionType::Top);
        left_panel.set_scrollable(true);
        left_panel.set_width_request(200);

        // Center-Right paned
        let center_right_paned = Paned::new(Orientation::Horizontal);
        center_right_paned.set_shrink_start_child(false);
        center_right_paned.set_shrink_end_child(false);
        center_right_paned.set_resize_start_child(true);
        center_right_paned.set_resize_end_child(false);
        center_right_paned.set_position(800); // Will be adjusted dynamically

        // Center panel (main content)
        let center_panel = Box::new(Orientation::Vertical, 0);
        center_panel.set_hexpand(true);
        center_panel.set_vexpand(true);

        // Right panel (Notebook with tabs)
        let right_panel = Notebook::new();
        right_panel.set_tab_pos(gtk4::PositionType::Top);
        right_panel.set_scrollable(true);
        right_panel.set_width_request(250);

        // Assemble center-right
        center_right_paned.set_start_child(Some(&center_panel));
        center_right_paned.set_end_child(Some(&right_panel));

        // Assemble main paned
        main_paned.set_start_child(Some(&left_panel));
        main_paned.set_end_child(Some(&center_right_paned));

        // Bottom paned (for filmstrip)
        let bottom_paned = Paned::new(Orientation::Vertical);
        bottom_paned.set_shrink_start_child(false);
        bottom_paned.set_shrink_end_child(false);
        bottom_paned.set_resize_start_child(true);
        bottom_paned.set_resize_end_child(false);
        bottom_paned.set_position(600); // Will be adjusted dynamically

        // Bottom panel
        let bottom_panel = Box::new(Orientation::Horizontal, 0);
        bottom_panel.set_height_request(120);

        // Assemble bottom paned
        bottom_paned.set_start_child(Some(&main_paned));
        bottom_paned.set_end_child(Some(&bottom_panel));

        // Add to root
        root.append(&bottom_paned);

        Self {
            root,
            main_paned,
            left_panel: Some(left_panel),
            center_panel,
            right_panel: Some(right_panel),
            bottom_paned: Some(bottom_paned),
            bottom_panel: Some(bottom_panel),
        }
    }

    /// Create a new Develop layout
    ///
    /// Layout:
    /// ```
    /// ┌─────────┬──────────────┬──────────────┐
    /// │  Left   │    Center    │    Right     │
    /// │ Presets │ImageViewer   │ Adjustments  │
    /// │ History │              │   (Sliders)  │
    /// ├─────────┴──────────────┴──────────────┤
    /// │            Filmstrip                   │
    /// └────────────────────────────────────────┘
    /// ```
    pub fn new_develop() -> Self {
        // Root container (vertical)
        let root = Box::new(Orientation::Vertical, 0);

        // Main horizontal paned
        let main_paned = Paned::new(Orientation::Horizontal);
        main_paned.set_shrink_start_child(false);
        main_paned.set_shrink_end_child(false);
        main_paned.set_resize_start_child(false);
        main_paned.set_resize_end_child(true);
        main_paned.set_position(220); // Left panel width

        // Left panel (Notebook)
        let left_panel = Notebook::new();
        left_panel.set_tab_pos(gtk4::PositionType::Top);
        left_panel.set_scrollable(true);
        left_panel.set_width_request(200);

        // Center-Right paned
        let center_right_paned = Paned::new(Orientation::Horizontal);
        center_right_paned.set_shrink_start_child(false);
        center_right_paned.set_shrink_end_child(false);
        center_right_paned.set_resize_start_child(true);
        center_right_paned.set_resize_end_child(false);
        center_right_paned.set_position(700); // Will be adjusted

        // Center panel (ImageViewer)
        let center_panel = Box::new(Orientation::Vertical, 0);
        center_panel.set_hexpand(true);
        center_panel.set_vexpand(true);

        // Right panel (Scrolled adjustments)
        let right_panel = Notebook::new();
        right_panel.set_tab_pos(gtk4::PositionType::Top);
        right_panel.set_scrollable(true);
        right_panel.set_width_request(300);

        // Assemble center-right
        center_right_paned.set_start_child(Some(&center_panel));
        center_right_paned.set_end_child(Some(&right_panel));

        // Assemble main paned
        main_paned.set_start_child(Some(&left_panel));
        main_paned.set_end_child(Some(&center_right_paned));

        // Bottom paned (filmstrip)
        let bottom_paned = Paned::new(Orientation::Vertical);
        bottom_paned.set_shrink_start_child(false);
        bottom_paned.set_shrink_end_child(false);
        bottom_paned.set_resize_start_child(true);
        bottom_paned.set_resize_end_child(false);
        bottom_paned.set_position(600);

        // Bottom panel
        let bottom_panel = Box::new(Orientation::Horizontal, 0);
        bottom_panel.set_height_request(120);

        // Assemble bottom paned
        bottom_paned.set_start_child(Some(&main_paned));
        bottom_paned.set_end_child(Some(&bottom_panel));

        // Add to root
        root.append(&bottom_paned);

        Self {
            root,
            main_paned,
            left_panel: Some(left_panel),
            center_panel,
            right_panel: Some(right_panel),
            bottom_paned: Some(bottom_paned),
            bottom_panel: Some(bottom_panel),
        }
    }

    /// Add a widget to a panel
    pub fn add_panel(&mut self, position: PanelPosition, widget: &Widget, tab_label: Option<&str>) {
        match position {
            PanelPosition::Left => {
                if let Some(ref notebook) = self.left_panel {
                    if let Some(label) = tab_label {
                        let label_widget = Label::new(Some(label));
                        notebook.append_page(widget, Some(&label_widget));
                    } else {
                        notebook.append_page(widget, gtk4::Widget::NONE);
                    }
                }
            }
            PanelPosition::Center => {
                // Clear existing center content
                while let Some(child) = self.center_panel.first_child() {
                    self.center_panel.remove(&child);
                }
                self.center_panel.append(widget);
            }
            PanelPosition::Right => {
                if let Some(ref notebook) = self.right_panel {
                    if let Some(label) = tab_label {
                        let label_widget = Label::new(Some(label));
                        notebook.append_page(widget, Some(&label_widget));
                    } else {
                        notebook.append_page(widget, gtk4::Widget::NONE);
                    }
                }
            }
            PanelPosition::Bottom => {
                if let Some(ref panel) = self.bottom_panel {
                    // Clear existing bottom content
                    while let Some(child) = panel.first_child() {
                        panel.remove(&child);
                    }
                    panel.append(widget);
                }
            }
        }
    }

    /// Set panel visibility
    pub fn set_panel_visible(&mut self, position: PanelPosition, visible: bool) {
        match position {
            PanelPosition::Left => {
                if let Some(ref panel) = self.left_panel {
                    panel.set_visible(visible);
                }
            }
            PanelPosition::Right => {
                if let Some(ref panel) = self.right_panel {
                    panel.set_visible(visible);
                }
            }
            PanelPosition::Bottom => {
                if let Some(ref panel) = self.bottom_panel {
                    panel.set_visible(visible);
                }
            }
            PanelPosition::Center => {
                // Center is always visible
            }
        }
    }

    /// Get the root widget
    pub fn get_root(&self) -> &Box {
        &self.root
    }

    /// Get left panel notebook (if exists)
    pub fn get_left_panel(&self) -> Option<&Notebook> {
        self.left_panel.as_ref()
    }

    /// Get right panel notebook (if exists)
    pub fn get_right_panel(&self) -> Option<&Notebook> {
        self.right_panel.as_ref()
    }

    /// Get center panel
    pub fn get_center_panel(&self) -> &Box {
        &self.center_panel
    }

    /// Get bottom panel (if exists)
    pub fn get_bottom_panel(&self) -> Option<&Box> {
        self.bottom_panel.as_ref()
    }
}
