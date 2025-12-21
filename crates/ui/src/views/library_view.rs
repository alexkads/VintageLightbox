// Library View
// Three-panel layout with photo grid in the center

use egui::Ui;
use crate::state::AppState;
use crate::design_system::theme::Theme;
use crate::components::photo_grid::PhotoGrid;
use crate::components::filmstrip::Filmstrip;

pub struct LibraryView {
    photo_grid: PhotoGrid,
    filmstrip: Filmstrip,
}

impl LibraryView {
    pub fn new() -> Self {
        Self {
            photo_grid: PhotoGrid::new(),
            filmstrip: Filmstrip::new(),
        }
    }

    pub fn show(&mut self, ui: &mut Ui, state: &mut AppState, ctx: &egui::Context) {
        // Bottom filmstrip (must be first to reserve space)
        egui::TopBottomPanel::bottom("filmstrip")
            .exact_height(120.0)  // 80px thumbnails + 40px padding
            .show_inside(ui, |ui| {
                let photos = state.photos.clone();
                let selected_id = state.selected_photo_id.clone();
                
                self.filmstrip.show(
                    ui,
                    ctx,
                    &photos,
                    &selected_id,
                    |photo_id| {
                        // Select photo but STAY in Library view
                        state.selected_photo_id = Some(photo_id);
                    },
                );
            });

        // Left sidebar
        egui::SidePanel::left("left_sidebar")
            .resizable(false)
            .exact_width(Theme::SIDEBAR_WIDTH)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_left_sidebar(ui, state);
                });
            });

        // Right sidebar
        egui::SidePanel::right("right_sidebar")
            .resizable(false)
            .exact_width(Theme::PANEL_WIDTH)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_right_sidebar(ui, state);
                });
            });

        // Center - Photo grid
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.photo_grid.show(ui, state, ctx);
        });
    }

    fn show_left_sidebar(&mut self, ui: &mut Ui, state: &mut AppState) {
        use crate::design_system::widgets;

        // Grid View Panel
        widgets::section_title(ui, "Grid View");
        ui.add_space(Theme::SPACE_SM);
        
        ui.horizontal(|ui| {
            for cols in [1, 2, 3, 4, 5] {
                let label = if cols == 1 { "⬛" } else { &format!("{}", cols) };
                if ui.selectable_label(state.grid_columns == cols, label).clicked() {
                    state.grid_columns = cols;
                }
            }
        });
        
        ui.add_space(Theme::SPACE_LG);

        // Filters Panel
        widgets::section_title(ui, "Filters");
        ui.add_space(Theme::SPACE_SM);

        // Rating filter
        ui.label(
            egui::RichText::new("Min Rating")
                .size(Theme::FONT_SM)
                .color(Theme::TEXT_MUTED)
        );
        ui.horizontal(|ui| {
            for rating in 0..=5 {
                let text = if rating == 0 { "All" } else { &"★".repeat(rating) };
                if ui.selectable_label(state.filter_min_rating == rating as i32, text).clicked() {
                    state.filter_min_rating = rating as i32;
                }
            }
        });

        ui.add_space(Theme::SPACE_SM);

        // Color label filter
        ui.label(
            egui::RichText::new("Color Label")
                .size(Theme::FONT_SM)
                .color(Theme::TEXT_MUTED)
        );

        let labels = vec![
            ("All", None),
            ("Red", Some("Red".to_string())),
            ("Yellow", Some("Yellow".to_string())),
            ("Green", Some("Green".to_string())),
            ("Blue", Some("Blue".to_string())),
            ("Purple", Some("Purple".to_string())),
        ];

        for (name, label) in labels {
            if widgets::menu_item(ui, name, state.filter_color_label == label).clicked() {
                state.filter_color_label = label;
            }
        }

        ui.add_space(Theme::SPACE_LG);

        // Catalog Panel
        widgets::section_title(ui, "Catalog");
        ui.add_space(Theme::SPACE_SM);

        if widgets::menu_item(ui, "All Photographs", true).clicked() {
            // Reset filters
            state.filter_min_rating = 0;
            state.filter_color_label = None;
        }

        ui.add_space(Theme::SPACE_LG);

        // Collections Panel
        widgets::section_title(ui, "Collections");
        ui.add_space(Theme::SPACE_SM);

        if widgets::secondary_button(ui, "+ Create Collection").clicked() {
            // TODO: Create new collection
        }
    }

    fn show_right_sidebar(&self, ui: &mut Ui, state: &AppState) {
        use crate::design_system::widgets;
        use crate::components::histogram::Histogram;

        // Histogram
        Histogram::show(ui, None);
        ui.add_space(Theme::SPACE_MD);

        // Quick Develop Panel
        widgets::section_title(ui, "Quick Develop");
        ui.add_space(Theme::SPACE_SM);

        if let Some(photo) = state.get_current_photo() {
            // Rating
            ui.label(
                egui::RichText::new("Rating")
                    .size(Theme::FONT_SM)
                    .color(Theme::TEXT_MUTED)
            );
            crate::components::rating_widget::RatingWidget::show_readonly(
                ui,
                photo.rating,
                16.0
            );

            ui.add_space(Theme::SPACE_MD);

            // Color Labels
            ui.label(
                egui::RichText::new("Color Label")
                    .size(Theme::FONT_SM)
                    .color(Theme::TEXT_MUTED)
            );
            crate::components::color_labels::ColorLabels::show(ui, &None, false);
        }

        ui.add_space(Theme::SPACE_XL);

        // Metadata Panel
        widgets::section_title(ui, "Metadata");
        ui.add_space(Theme::SPACE_SM);

        if let Some(photo) = state.get_current_photo() {
            widgets::label_text(ui, &format!("File: {}", photo.name));
            widgets::label_text(ui, &format!("Date: {}", photo.date));
            widgets::label_text(ui, &format!("Camera: {}", photo.camera));
            widgets::label_text(ui, &format!("Exposure: {}", photo.exposure));
        }
    }
}

impl Default for LibraryView {
    fn default() -> Self {
        Self::new()
    }
}
