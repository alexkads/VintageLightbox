//! Print Dialog Component
//!
//! Dialog for configuring and executing print jobs.
//! Provides layout selection, page configuration, and print preview.

use eframe::egui;
use crate::design_system::{theme::Theme, icons, widgets};

/// Print layout options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintLayoutOption {
    Single,
    Grid2x2,
    Grid3x3,
    ContactSheet,
}

impl PrintLayoutOption {
    pub fn display_name(&self) -> &'static str {
        match self {
            PrintLayoutOption::Single => "Single Photo",
            PrintLayoutOption::Grid2x2 => "2×2 Grid",
            PrintLayoutOption::Grid3x3 => "3×3 Grid",
            PrintLayoutOption::ContactSheet => "Contact Sheet",
        }
    }
    
    pub fn all() -> &'static [PrintLayoutOption] {
        &[
            PrintLayoutOption::Single,
            PrintLayoutOption::Grid2x2,
            PrintLayoutOption::Grid3x3,
            PrintLayoutOption::ContactSheet,
        ]
    }
}

/// Paper size options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperSizeOption {
    A4,
    Letter,
    A3,
}

impl PaperSizeOption {
    pub fn display_name(&self) -> &'static str {
        match self {
            PaperSizeOption::A4 => "A4 (210×297mm)",
            PaperSizeOption::Letter => "Letter (8.5×11\")",
            PaperSizeOption::A3 => "A3 (297×420mm)",
        }
    }
    
    pub fn all() -> &'static [PaperSizeOption] {
        &[
            PaperSizeOption::A4,
            PaperSizeOption::Letter,
            PaperSizeOption::A3,
        ]
    }
}

/// Orientation options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrientationOption {
    Portrait,
    Landscape,
}

impl OrientationOption {
    pub fn display_name(&self) -> &'static str {
        match self {
            OrientationOption::Portrait => "Portrait",
            OrientationOption::Landscape => "Landscape",
        }
    }
}

/// State for the print dialog
#[derive(Debug)]
pub struct PrintDialogState {
    /// IDs of photos to print
    pub photo_ids: Vec<String>,
    /// Selected layout
    pub layout: PrintLayoutOption,
    /// Paper size
    pub paper_size: PaperSizeOption,
    /// Orientation
    pub orientation: OrientationOption,
    /// Include metadata below photos
    pub include_metadata: bool,
    /// Number of copies
    pub copies: u8,
    /// Margins in mm
    pub margin_mm: u16,
}

impl Default for PrintDialogState {
    fn default() -> Self {
        Self {
            photo_ids: Vec::new(),
            layout: PrintLayoutOption::Single,
            paper_size: PaperSizeOption::A4,
            orientation: OrientationOption::Portrait,
            include_metadata: false,
            copies: 1,
            margin_mm: 10,
        }
    }
}

impl PrintDialogState {
    /// Create a new print dialog state with the given photo IDs
    pub fn new(photo_ids: Vec<String>) -> Self {
        Self {
            photo_ids,
            ..Default::default()
        }
    }
}

/// Actions that can be returned from the print dialog
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintDialogAction {
    /// User clicked Print
    Print,
    /// User clicked Cancel
    Cancel,
    /// No action yet
    None,
}

/// Print Dialog component
pub struct PrintDialog;

impl PrintDialog {
    /// Show the print dialog
    /// Returns the action taken by the user
    pub fn show(
        ctx: &egui::Context,
        state: &mut PrintDialogState,
    ) -> PrintDialogAction {
        let mut action = PrintDialogAction::None;
        
        egui::Window::new(format!("{} Print", icons::NAV_PRINT))
            .collapsible(false)
            .resizable(true)
            .default_width(500.0)
            .default_height(400.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.add_space(Theme::SPACE_MD);
                
                // Photo count info
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} {} photo(s) selected", icons::FILE_IMAGE, state.photo_ids.len()))
                            .size(Theme::FONT_MD)
                            .color(Theme::TEXT_PRIMARY)
                    );
                });
                
                ui.add_space(Theme::SPACE_LG);
                ui.separator();
                ui.add_space(Theme::SPACE_MD);
                
                // Layout section
                ui.heading(egui::RichText::new("Layout").size(Theme::FONT_LG));
                ui.add_space(Theme::SPACE_SM);
                
                egui::ComboBox::from_label("Print Layout")
                    .selected_text(state.layout.display_name())
                    .show_ui(ui, |ui| {
                        for layout in PrintLayoutOption::all() {
                            ui.selectable_value(&mut state.layout, *layout, layout.display_name());
                        }
                    });
                
                ui.add_space(Theme::SPACE_MD);
                
                // Page Setup section
                ui.heading(egui::RichText::new("Page Setup").size(Theme::FONT_LG));
                ui.add_space(Theme::SPACE_SM);
                
                egui::ComboBox::from_label("Paper Size")
                    .selected_text(state.paper_size.display_name())
                    .show_ui(ui, |ui| {
                        for size in PaperSizeOption::all() {
                            ui.selectable_value(&mut state.paper_size, *size, size.display_name());
                        }
                    });
                
                ui.add_space(Theme::SPACE_SM);
                
                ui.horizontal(|ui| {
                    ui.label("Orientation:");
                    ui.selectable_value(&mut state.orientation, OrientationOption::Portrait, "Portrait");
                    ui.selectable_value(&mut state.orientation, OrientationOption::Landscape, "Landscape");
                });
                
                ui.add_space(Theme::SPACE_SM);
                
                ui.horizontal(|ui| {
                    ui.label("Margins (mm):");
                    ui.add(egui::DragValue::new(&mut state.margin_mm)
                        .range(0..=50)
                        .speed(1));
                });
                
                ui.add_space(Theme::SPACE_MD);
                
                // Options section
                ui.heading(egui::RichText::new("Options").size(Theme::FONT_LG));
                ui.add_space(Theme::SPACE_SM);
                
                ui.checkbox(&mut state.include_metadata, "Include photo metadata");
                
                ui.horizontal(|ui| {
                    ui.label("Copies:");
                    ui.add(egui::DragValue::new(&mut state.copies)
                        .range(1..=99)
                        .speed(1));
                });
                
                ui.add_space(Theme::SPACE_LG);
                ui.separator();
                ui.add_space(Theme::SPACE_MD);
                
                // Preview section (simplified)
                ui.heading(egui::RichText::new("Preview").size(Theme::FONT_LG));
                ui.add_space(Theme::SPACE_SM);
                
                // Calculate page count
                let photos_per_page = match state.layout {
                    PrintLayoutOption::Single => 1,
                    PrintLayoutOption::Grid2x2 => 4,
                    PrintLayoutOption::Grid3x3 => 9,
                    PrintLayoutOption::ContactSheet => 12,
                };
                let page_count = (state.photo_ids.len() + photos_per_page - 1) / photos_per_page.max(1);
                
                ui.label(format!(
                    "{} page(s) will be printed ({} photos per page)",
                    page_count, photos_per_page
                ));
                
                ui.add_space(Theme::SPACE_LG);
                ui.separator();
                ui.add_space(Theme::SPACE_MD);
                
                // Action buttons
                ui.horizontal(|ui| {
                    // Spacer to push buttons to the right
                    ui.allocate_space(egui::vec2(ui.available_width() - 180.0, 0.0));
                    
                    if widgets::secondary_button(ui, "Cancel").clicked() {
                        action = PrintDialogAction::Cancel;
                    }
                    
                    ui.add_space(Theme::SPACE_SM);
                    
                    let print_label = format!("{} Print", icons::NAV_PRINT);
                    if widgets::primary_button(ui, &print_label).clicked() {
                        action = PrintDialogAction::Print;
                    }
                });
                
                ui.add_space(Theme::SPACE_MD);
            });
        
        action
    }
}
