//! DockViewer - Implementation of egui_dock::TabViewer
//!
//! Renders the content of each dockable tab based on its type.

use egui::Ui;
use egui_dock::TabViewer;
use std::sync::Arc;
use tokio::sync::mpsc;

use adapters::controllers::*;
use adapters::view_models::PhotoViewModel;
use infrastructure::cache::preview_manager::PreviewManager;

use crate::state::AppState;
use crate::design_system::{theme::Theme, widgets};
use crate::components::{
    photo_grid::PhotoGrid,
    image_viewer::ImageViewer,
    filmstrip::Filmstrip,
    histogram::Histogram,
    folder_tree::FolderTree,
    rating_widget::RatingWidget,
    color_labels::ColorLabels,
    metadata_charts::MetadataCharts,
    tone_curve::ToneCurveEditor,
};

use super::dock_tab::DockTab;

/// Context needed to render dock tabs
pub struct DockViewerContext<'a> {
    pub state: &'a mut AppState,
    pub library_controller: &'a Arc<LibraryController>,
    pub editor_controller: &'a Arc<EditorController>,
    pub export_controller: &'a Arc<ExportController>,
    pub photo_controller: &'a Arc<PhotoController>,
    pub preview_manager: &'a Arc<PreviewManager>,
    pub photo_sender: &'a mpsc::Sender<Result<Vec<PhotoViewModel>, String>>,
    pub ctx: &'a egui::Context,
    pub photo_grid: &'a mut PhotoGrid,
    pub filmstrip: &'a mut Filmstrip,
}

/// TabViewer implementation for VintageLightbox
pub struct DockViewer<'a> {
    context: DockViewerContext<'a>,
}

impl<'a> DockViewer<'a> {
    pub fn new(context: DockViewerContext<'a>) -> Self {
        Self { context }
    }
}

impl<'a> TabViewer for DockViewer<'a> {
    type Tab = DockTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.to_string().into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            DockTab::PhotoGrid => {
                self.context.photo_grid.show(
                    ui,
                    self.context.state,
                    self.context.ctx,
                );
            }

            DockTab::ImageViewer => {
                ImageViewer::show(ui, self.context.state);
            }

            DockTab::Folders => {
                self.render_folders_panel(ui);
            }

            DockTab::Collections => {
                self.render_collections_panel(ui);
            }

            DockTab::GridSettings => {
                self.render_grid_settings(ui);
            }

            DockTab::Filters => {
                self.render_filters_panel(ui);
            }

            DockTab::Histogram => {
                Histogram::show(ui, self.context.state.histogram_data.as_ref());
            }

            DockTab::QuickDevelop => {
                self.render_quick_develop(ui);
            }

            DockTab::Metadata => {
                self.render_metadata_panel(ui);
            }

            DockTab::BasicAdjustments => {
                self.render_basic_adjustments(ui);
            }

            DockTab::ToneCurve => {
                ToneCurveEditor::show(ui, self.context.state);
            }

            DockTab::Filmstrip => {
                use crate::state::CurrentView;
                use crate::components::filmstrip::FilmstripAction;
                let photos = self.context.state.photos.clone();
                let current_view = self.context.state.current_view;
                
                // For Develop view, we need the filmstrip to update develop_selected_photo_id
                // The show() method updates library_selected_photo_id, so we need to sync
                let prev_library_id = self.context.state.library_selected_photo_id.clone();
                
                let action = self.context.filmstrip.show(
                    ui,
                    self.context.ctx,
                    &photos,
                    self.context.state,
                );

                // Handle context menu actions
                if let Some(action) = action {
                    match action {
                        FilmstripAction::OpenInDevelop => {
                            if let Some(photo_id) = self.context.state.library_selected_photo_id.clone() {
                                self.context.state.develop_selected_photo_id = Some(photo_id);
                                self.context.state.current_view = CurrentView::Develop;
                                self.context.state.loaded_photo_id = None;
                            }
                        }
                        FilmstripAction::Export => {
                            // Trigger export for selected photo
                            if let Some(metadata) = &self.context.state.detail_metadata.clone() {
                                let controller = self.context.export_controller.clone();
                                let id = metadata.id.clone();
                                let ctx_clone = self.context.ctx.clone();

                                let (export_tx, export_rx) = tokio::sync::mpsc::channel::<Result<String, String>>(1);
                                self.context.state.pending_export_receiver = Some(export_rx);
                                self.context.state.is_busy = true;
                                self.context.state.busy_message = "Exporting...".to_string();
                                self.context.state.toasts.info("Exporting photo...");

                                tokio::spawn(async move {
                                    let file_dialog = rfd::AsyncFileDialog::new()
                                        .set_title("Export Photo")
                                        .set_file_name("exported.jpg")
                                        .add_filter("JPEG", &["jpg", "jpeg"])
                                        .save_file()
                                        .await;

                                    if let Some(file) = file_dialog {
                                        if let Some(path) = file.path().to_str() {
                                            let path_string = path.to_string();
                                            match controller.export_photo(id, path_string.clone()).await {
                                                Ok(_) => {
                                                    let _ = export_tx.send(Ok(path_string)).await;
                                                }
                                                Err(e) => {
                                                    let _ = export_tx.send(Err(e)).await;
                                                }
                                            }
                                        }
                                    } else {
                                        let _ = export_tx.send(Err("Cancelled".to_string())).await;
                                    }
                                    ctx_clone.request_repaint();
                                });
                            } else if let Some(photo_id) = &self.context.state.library_selected_photo_id {
                                // No detail_metadata, use library_selected_photo_id
                                let controller = self.context.export_controller.clone();
                                let id = photo_id.clone();
                                let ctx_clone = self.context.ctx.clone();

                                let (export_tx, export_rx) = tokio::sync::mpsc::channel::<Result<String, String>>(1);
                                self.context.state.pending_export_receiver = Some(export_rx);
                                self.context.state.is_busy = true;
                                self.context.state.busy_message = "Exporting...".to_string();
                                self.context.state.toasts.info("Exporting photo...");

                                tokio::spawn(async move {
                                    let file_dialog = rfd::AsyncFileDialog::new()
                                        .set_title("Export Photo")
                                        .set_file_name("exported.jpg")
                                        .add_filter("JPEG", &["jpg", "jpeg"])
                                        .save_file()
                                        .await;

                                    if let Some(file) = file_dialog {
                                        if let Some(path) = file.path().to_str() {
                                            let path_string = path.to_string();
                                            match controller.export_photo(id, path_string.clone()).await {
                                                Ok(_) => {
                                                    let _ = export_tx.send(Ok(path_string)).await;
                                                }
                                                Err(e) => {
                                                    let _ = export_tx.send(Err(e)).await;
                                                }
                                            }
                                        }
                                    } else {
                                        let _ = export_tx.send(Err("Cancelled".to_string())).await;
                                    }
                                    ctx_clone.request_repaint();
                                });
                            }
                        }
                        FilmstripAction::SelectAll => {
                            self.context.state.select_all();
                        }
                        FilmstripAction::DeselectAll => {
                            self.context.state.clear_selection();
                        }
                        FilmstripAction::Delete => {
                            if !self.context.state.selected_photo_ids.is_empty() {
                                self.context.state.show_delete_confirmation = true;
                            }
                        }
                        FilmstripAction::SetFlag(photo_id, flag_code) => {
                            let controller = self.context.photo_controller.clone();
                            let library_controller = self.context.library_controller.clone();
                            let photo_sender = self.context.photo_sender.clone();
                            let ctx_clone = self.context.ctx.clone();
                            let id = photo_id.clone();
                            
                            tokio::spawn(async move {
                                let _ = controller.set_flag(&id, flag_code).await;
                                
                                // Reload
                                if let Ok(photos) = library_controller.get_all_photos().await {
                                     let _ = photo_sender.send(Ok(photos)).await;
                                }
                                ctx_clone.request_repaint();
                            });
                        }
                    }
                }
                
                // If we're in Develop mode and library_selected_photo_id changed,
                // sync it to develop_selected_photo_id and trigger image reload
                if current_view == CurrentView::Develop {
                    if self.context.state.library_selected_photo_id != prev_library_id {
                        if let Some(new_id) = self.context.state.library_selected_photo_id.clone() {
                            self.context.state.develop_selected_photo_id = Some(new_id);
                            self.context.state.loaded_photo_id = None; // Force reload
                        }
                    }
                }
            }
        }
    }

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        // No tabs should be closeable
        false
    }

    fn on_close(&mut self, _tab: &mut Self::Tab) -> bool {
        // Allow closing
        true
    }
}

// Private rendering methods
impl<'a> DockViewer<'a> {
    fn render_folders_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut folder_tree = FolderTree::new(
                self.context.state.filter_folder_path.as_deref(),
                &mut self.context.state.expanded_folders,
            );

            if let Some(clicked_path) = folder_tree.show(ui, &self.context.state.folder_tree_roots) {
                self.context.state.filter_folder_path = Some(clicked_path);
            }
        });
    }

    fn render_collections_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            if widgets::secondary_button(ui, "+ Create Collection").clicked() {
                // TODO: Create new collection
            }
            ui.add_space(Theme::SPACE_SM);
            ui.label("(No collections yet)");
        });
    }

    fn render_grid_settings(&mut self, ui: &mut Ui) {
        ui.add_space(Theme::SPACE_SM);
        ui.label(egui::RichText::new("Columns").size(Theme::FONT_SM).color(Theme::TEXT_MUTED));
        ui.horizontal(|ui| {
            for cols in [1, 2, 3, 4, 5] {
                let label = if cols == 1 { "⬛" } else { &format!("{}", cols) };
                if ui.selectable_label(self.context.state.grid_columns == cols, label).clicked() {
                    self.context.state.grid_columns = cols;
                }
            }
        });
    }

    fn render_filters_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            // Rating filter
            ui.label(egui::RichText::new("Min Rating").size(Theme::FONT_SM).color(Theme::TEXT_MUTED));
            ui.horizontal(|ui| {
                for rating in 0..=5 {
                    let text = if rating == 0 { "All" } else { &"★".repeat(rating) };
                    if ui.selectable_label(self.context.state.filter_min_rating == rating as i32, text).clicked() {
                        self.context.state.filter_min_rating = rating as i32;
                    }
                }
            });

            ui.add_space(Theme::SPACE_SM);

            // Color label filter
            ui.label(egui::RichText::new("Color Label").size(Theme::FONT_SM).color(Theme::TEXT_MUTED));
            let labels = vec![
                ("All", None),
                ("Red", Some("Red".to_string())),
                ("Yellow", Some("Yellow".to_string())),
                ("Green", Some("Green".to_string())),
                ("Blue", Some("Blue".to_string())),
                ("Purple", Some("Purple".to_string())),
            ];

            for (name, label) in labels {
                if widgets::menu_item(ui, name, self.context.state.filter_color_label == label).clicked() {
                    self.context.state.filter_color_label = label;
                }
            }
        });
    }

    fn render_quick_develop(&mut self, ui: &mut Ui) {
        if let Some(photo) = self.context.state.get_current_photo() {
            ui.label(egui::RichText::new("Rating").size(Theme::FONT_SM).color(Theme::TEXT_MUTED));
            RatingWidget::show_readonly(ui, photo.rating, 16.0);

            ui.add_space(Theme::SPACE_MD);

            ui.label(egui::RichText::new("Color Label").size(Theme::FONT_SM).color(Theme::TEXT_MUTED));
            ColorLabels::show(ui, &None, false);
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("No photo selected");
            });
        }
    }

    fn render_metadata_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            // Statistics
            widgets::section_title(ui, "Statistics");
            ui.add_space(Theme::SPACE_SM);
            MetadataCharts::show(ui, &self.context.state.photos);
            ui.add_space(Theme::SPACE_MD);

            // Photo info
            if let Some(photo) = self.context.state.get_current_photo() {
                widgets::section_title(ui, "Photo Info");
                ui.add_space(Theme::SPACE_SM);
                widgets::label_text(ui, &format!("File: {}", photo.name));
                widgets::label_text(ui, &format!("Date: {}", photo.date));
                widgets::label_text(ui, &format!("Camera: {}", photo.camera));
                widgets::label_text(ui, &format!("Exposure: {}", photo.exposure));
            }
        });
    }

    fn render_basic_adjustments(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);
            
            // Exposure
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Exposure").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.2}", self.context.state.active_exposure));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_exposure, -5.0..=5.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Contrast
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Contrast").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.2}", self.context.state.active_contrast));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_contrast, 0.0..=2.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Temperature
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Temperature").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.1}", self.context.state.active_temperature));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_temperature, -10.0..=10.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Tint
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tint").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.1}", self.context.state.active_tint));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_tint, -10.0..=10.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_MD);
            ui.separator();
            ui.add_space(Theme::SPACE_SM);

            // Highlights
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Highlights").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", self.context.state.active_highlights));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_highlights, -100.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Shadows
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Shadows").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", self.context.state.active_shadows));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_shadows, -100.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Whites
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Whites").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", self.context.state.active_whites));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_whites, -100.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Blacks
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Blacks").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", self.context.state.active_blacks));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_blacks, -100.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_MD);
            ui.separator();
            ui.add_space(Theme::SPACE_SM);

            // Clarity
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Clarity").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.2}", self.context.state.active_clarity));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_clarity, -1.0..=1.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Vibrance
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Vibrance").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.2}", self.context.state.active_vibrance));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_vibrance, -1.0..=1.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Saturation
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Saturation").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.2}", self.context.state.active_saturation));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_saturation, -1.0..=1.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_MD);

            // Reset button
            if ui.button("Reset All").clicked() {
                self.context.state.active_exposure = 0.0;
                self.context.state.active_contrast = 1.0;
                self.context.state.active_temperature = 0.0;
                self.context.state.active_tint = 0.0;
                self.context.state.active_highlights = 0.0;
                self.context.state.active_shadows = 0.0;
                self.context.state.active_whites = 0.0;
                self.context.state.active_blacks = 0.0;
                self.context.state.active_clarity = 0.0;
                self.context.state.active_vibrance = 0.0;
                self.context.state.active_saturation = 0.0;
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_MD);

            // Export button
            if ui.button("📤 Export JPEG").clicked() {
                if let Some(metadata) = &self.context.state.detail_metadata {
                    let controller = self.context.export_controller.clone();
                    let id = metadata.id.clone();
                    let ctx_clone = self.context.ctx.clone();

                    // Create channel for export result
                    let (export_tx, export_rx) = tokio::sync::mpsc::channel::<Result<String, String>>(1);
                    self.context.state.pending_export_receiver = Some(export_rx);
                    self.context.state.is_busy = true;
                    self.context.state.busy_message = "Exporting...".to_string();
                    self.context.state.toasts.info("Exporting photo...");

                    tokio::spawn(async move {
                        let file_dialog = rfd::AsyncFileDialog::new()
                            .set_title("Export Photo")
                            .set_file_name("exported.jpg")
                            .add_filter("JPEG", &["jpg", "jpeg"])
                            .save_file()
                            .await;

                        if let Some(file) = file_dialog {
                            if let Some(path) = file.path().to_str() {
                                let path_string = path.to_string();
                                match controller.export_photo(id, path_string.clone()).await {
                                    Ok(_) => {
                                        let _ = export_tx.send(Ok(path_string)).await;
                                    }
                                    Err(e) => {
                                        let _ = export_tx.send(Err(e)).await;
                                    }
                                }
                            }
                        } else {
                            // User cancelled
                            let _ = export_tx.send(Err("Cancelled".to_string())).await;
                        }
                        ctx_clone.request_repaint();
                    });
                } else {
                    self.context.state.toasts.warning("No photo selected");
                }
            }
        });
    }
}
