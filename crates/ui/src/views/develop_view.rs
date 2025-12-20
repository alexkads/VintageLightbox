// Develop View
// Image editing view with adjustments panel

use egui::Ui;
use crate::state::AppState;
use crate::design_system::theme::Theme;
use crate::components::image_viewer::ImageViewer;

pub struct DevelopView;

impl DevelopView {
    pub fn new() -> Self {
        Self
    }

    pub fn show(
        &self,
        ui: &mut Ui,
        state: &mut AppState,
        editor_controller: &std::sync::Arc<adapters::controllers::EditorController>,
        export_controller: &std::sync::Arc<adapters::controllers::ExportController>,
        photo_controller: &std::sync::Arc<adapters::controllers::PhotoController>,
    ) {
        // Left sidebar - Presets & History
        egui::SidePanel::left("develop_left")
            .resizable(false)
            .exact_width(Theme::SIDEBAR_WIDTH)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_left_sidebar(ui);
                });
            });

        // Right sidebar - Adjustments
        egui::SidePanel::right("develop_right")
            .resizable(false)
            .exact_width(Theme::PANEL_WIDTH + 20.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_right_sidebar(ui, state, editor_controller, export_controller, photo_controller);
                });
            });

        // Center - Image viewer
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ImageViewer::show(ui, state);
        });
    }

    fn show_left_sidebar(&self, ui: &mut Ui) {
        use crate::design_system::widgets;

        // Presets Panel
        widgets::section_title(ui, "Presets");
        ui.add_space(Theme::SPACE_SM);

        if widgets::menu_item(ui, "Default", false).clicked() {
            // TODO: Apply default preset
        }
        if widgets::menu_item(ui, "Auto", false).clicked() {
            // TODO: Apply auto preset
        }
        if widgets::menu_item(ui, "B&W", false).clicked() {
            // TODO: Apply B&W preset
        }

        ui.add_space(Theme::SPACE_LG);

        // History Panel
        widgets::section_title(ui, "History");
        ui.add_space(Theme::SPACE_SM);

        widgets::menu_item(ui, "Current State", true);
    }

    fn show_right_sidebar(
        &self,
        ui: &mut Ui,
        state: &mut AppState,
        editor_controller: &std::sync::Arc<adapters::controllers::EditorController>,
        export_controller: &std::sync::Arc<adapters::controllers::ExportController>,
        photo_controller: &std::sync::Arc<adapters::controllers::PhotoController>,
    ) {
        use crate::design_system::widgets;
        use crate::components::{histogram::Histogram, slider_control::SliderControl, rating_widget::RatingWidget};

        // Histogram
        Histogram::show(ui, state.histogram_data.as_ref());
        ui.add_space(Theme::SPACE_MD);

        // Basic Adjustments
        widgets::section_title(ui, "Basic");
        ui.add_space(Theme::SPACE_SM);

        // Exposure slider
        SliderControl::show(
            ui,
            "Exposure",
            &mut state.active_exposure,
            -2.0..=2.0,
            0.1,
        );

        ui.add_space(Theme::SPACE_SM);

        // Contrast slider
        SliderControl::show(
            ui,
            "Contrast",
            &mut state.active_contrast,
            0.5..=1.5,
            0.05,
        );

        ui.add_space(Theme::SPACE_LG);

        // Collapsed sections (placeholders)
        ui.label(
            egui::RichText::new("▶ Tone Curve")
                .size(Theme::FONT_SM)
                .color(Theme::TEXT_SECONDARY)
        );
        ui.label(
            egui::RichText::new("▶ HSL / Color")
                .size(Theme::FONT_SM)
                .color(Theme::TEXT_SECONDARY)
        );

        ui.add_space(Theme::SPACE_XXL);

        // Action buttons
        if widgets::primary_button(ui, "Save").clicked() {
            if let Some(metadata) = &state.detail_metadata {
                let controller = editor_controller.clone();
                let id = metadata.id.clone();
                let exposure = state.active_exposure;
                let contrast = state.active_contrast;
                
                state.is_busy = true;
                state.busy_message = "Saving edits...".to_string();
                
                tokio::spawn(async move {
                    if let Err(e) = controller.save_edits(id, exposure, contrast).await {
                        eprintln!("Failed to save edits: {}", e);
                    }
                });
            }
        }

        ui.add_space(Theme::SPACE_SM);

        if widgets::secondary_button(ui, "Export").clicked() {
            if let Some(metadata) = &state.detail_metadata {
                let controller = export_controller.clone();
                let id = metadata.id.clone();
                
                tokio::spawn(async move {
                    let file_dialog = rfd::AsyncFileDialog::new()
                        .set_title("Export Photo")
                        .set_file_name("exported.jpg")
                        .add_filter("JPEG", &["jpg", "jpeg"])
                        .save_file()
                        .await;
                    
                    if let Some(file) = file_dialog {
                        if let Some(path) = file.path().to_str() {
                            if let Err(e) = controller.export_photo(id, path.to_string()).await {
                                eprintln!("Failed to export: {}", e);
                            }
                        }
                    }
                });
            }
        }

        ui.add_space(Theme::SPACE_XL);

        // Rating
        widgets::section_title(ui, "Rating");
        ui.add_space(Theme::SPACE_SM);

        if let Some(metadata) = &mut state.detail_metadata {
            if let Some(new_rating) = RatingWidget::show(ui, &mut metadata.rating, 20.0) {
                // Persist rating change to database
                if let Some(photo_id) = &state.selected_photo_id {
                    let controller = photo_controller.clone();
                    let photo_id = photo_id.clone();
                    tokio::spawn(async move {
                        if let Err(e) = controller.rate_photo(&photo_id, new_rating).await {
                            eprintln!("Failed to save rating: {}", e);
                        }
                    });
                }
            }
        }

        ui.add_space(Theme::SPACE_XL);

        // Metadata
        if let Some(metadata) = &state.detail_metadata {
            widgets::section_title(ui, "Metadata");
            ui.add_space(Theme::SPACE_XS);

            widgets::label_text(ui, &metadata.name);
            widgets::label_text(ui, &metadata.date);
            widgets::label_text(ui, &metadata.camera);
            widgets::label_text(ui, &metadata.exposure);
        }
    }
}

impl Default for DevelopView {
    fn default() -> Self {
        Self::new()
    }
}
