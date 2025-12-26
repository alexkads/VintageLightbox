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

            DockTab::HSLColor => {
                self.render_hsl_color(ui);
            }

            DockTab::HSLHue => {
                self.render_hsl_hue(ui);
            }

            DockTab::HSLLuminance => {
                self.render_hsl_luminance(ui);
            }

            DockTab::LensCorrections => {
                self.render_lens_corrections(ui);
            }

            DockTab::Detail => {
                self.render_detail(ui);
            }

            DockTab::AllAdjustments => {
                self.render_all_adjustments(ui);
            }

            DockTab::Presets => {
                self.render_presets_panel(ui);
            }

            DockTab::Filmstrip => {
                use crate::state::CurrentView;
                use crate::components::filmstrip::FilmstripAction;
                let current_view = self.context.state.current_view;
                
                // For Develop view, we need the filmstrip to update develop_selected_photo_id
                // The show() method updates library_selected_photo_id, so we need to sync
                let prev_library_id = self.context.state.library_selected_photo_id.clone();
                
                let action = self.context.filmstrip.show(
                    ui,
                    self.context.ctx,
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
                                let id = metadata.id.clone();
                                
                                let mut dialog = egui_file::FileDialog::save_file(None)
                                    .title("Export Photo")
                                    .default_filename("exported.jpg");
                                dialog.open();
                                
                                self.context.state.export_target_id = Some(id);
                                self.context.state.import_dialog = Some(dialog);
                                self.context.state.import_dialog_mode = crate::state::ImportDialogMode::Export;

                            } else if let Some(photo_id) = &self.context.state.library_selected_photo_id {
                                let id = photo_id.clone();
                                
                                let mut dialog = egui_file::FileDialog::save_file(None)
                                    .title("Export Photo")
                                    .default_filename("exported.jpg");
                                dialog.open();
                                
                                self.context.state.export_target_id = Some(id);
                                self.context.state.import_dialog = Some(dialog);
                                self.context.state.import_dialog_mode = crate::state::ImportDialogMode::Export;
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
                self.context.state.filmstrip_filter.folder_path.as_deref(),
                &mut self.context.state.expanded_folders,
            );

            if let Some(clicked_path) = folder_tree.show(ui, &self.context.state.folder_tree_roots) {
                self.context.state.filmstrip_filter.folder_path = Some(clicked_path);
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
        ui.label(egui::RichText::new("Columns").size(Theme::FONT_SM).color(ui.visuals().weak_text_color()));
        ui.horizontal(|ui| {
            for cols in [1, 2, 3, 4, 5] {
                let label = if cols == 1 { "⬛" } else { &format!("{}", cols) };
                if ui.selectable_label(self.context.state.grid_columns == cols, label).clicked() {
                    self.context.state.grid_columns = cols;
                }
            }
        });
    }


    fn render_quick_develop(&mut self, ui: &mut Ui) {
        if let Some(photo) = self.context.state.get_current_photo() {
            ui.label(egui::RichText::new("Rating").size(Theme::FONT_SM).color(ui.visuals().weak_text_color()));
            RatingWidget::show_readonly(ui, photo.rating, 16.0);

            ui.add_space(Theme::SPACE_MD);

            ui.label(egui::RichText::new("Color Label").size(Theme::FONT_SM).color(ui.visuals().weak_text_color()));
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

            // Reset button with icon
            let reset_label = format!("{} Reset All", crate::design_system::icons::ACTION_RESET);
            if ui.button(&reset_label).clicked() {
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

            // Export button with icon
            let export_label = format!("{} Export JPEG", crate::design_system::icons::ACTION_EXPORT);
            if ui.button(&export_label).clicked() {
                if let Some(metadata) = &self.context.state.detail_metadata {
                    let id = metadata.id.clone();
                    
                    let mut dialog = egui_file::FileDialog::save_file(None)
                        .title("Export Photo")
                        .default_filename("exported.jpg");
                    dialog.open();
                    
                    self.context.state.export_target_id = Some(id);
                    self.context.state.import_dialog = Some(dialog);
                    self.context.state.import_dialog_mode = crate::state::ImportDialogMode::Export;
                } else {
                    self.context.state.toasts.warning("No photo selected");
                }
            }
        });
    }

    fn render_presets_panel(&mut self, ui: &mut Ui) {
        // Clone presets to avoid borrow issues
        let system_presets: Vec<_> = self.context.state.presets.iter()
            .filter(|p| p.is_system)
            .cloned()
            .collect();
        let user_presets: Vec<_> = self.context.state.presets.iter()
            .filter(|p| !p.is_system)
            .cloned()
            .collect();
        
        let mut preset_to_apply: Option<domain::entities::Preset> = None;
        
        egui::ScrollArea::vertical().show(ui, |ui| {
            // System Presets
            ui.collapsing("System Presets", |ui| {
                for preset in &system_presets {
                    if ui.selectable_label(false, &preset.name).clicked() {
                        preset_to_apply = Some(preset.clone());
                    }
                }
            });
            
            ui.add_space(Theme::SPACE_SM);
            
            // User Presets
            ui.collapsing("User Presets", |ui| {
                if user_presets.is_empty() {
                    ui.label("No user presets yet");
                } else {
                    for preset in &user_presets {
                        if ui.selectable_label(false, &preset.name).clicked() {
                            preset_to_apply = Some(preset.clone());
                        }
                    }
                }
            });
            
            ui.add_space(Theme::SPACE_MD);
            
            // Save as Preset button
            if widgets::secondary_button(ui, "+ Save as Preset").clicked() {
                self.context.state.show_save_preset_dialog = true;
            }
        });
        
        // Apply preset outside the closure
        if let Some(preset) = preset_to_apply {
            self.apply_preset(&preset);
        }
    }
    
    fn apply_preset(&mut self, preset: &domain::entities::Preset) {
        if let Some(v) = preset.adjustments.exposure { self.context.state.active_exposure = v; }
        if let Some(v) = preset.adjustments.contrast { self.context.state.active_contrast = v; }
        if let Some(v) = preset.adjustments.temperature { self.context.state.active_temperature = v; }
        if let Some(v) = preset.adjustments.tint { self.context.state.active_tint = v; }
        if let Some(v) = preset.adjustments.highlights { self.context.state.active_highlights = v; }
        if let Some(v) = preset.adjustments.shadows { self.context.state.active_shadows = v; }
        if let Some(v) = preset.adjustments.whites { self.context.state.active_whites = v; }
        if let Some(v) = preset.adjustments.blacks { self.context.state.active_blacks = v; }
        if let Some(v) = preset.adjustments.clarity { self.context.state.active_clarity = v; }
        if let Some(v) = preset.adjustments.vibrance { self.context.state.active_vibrance = v; }
        if let Some(v) = preset.adjustments.saturation { self.context.state.active_saturation = v; }
        if let Some(v) = preset.adjustments.tone_curve_shadows { self.context.state.active_tone_curve_shadows = v; }
        if let Some(v) = preset.adjustments.tone_curve_darks { self.context.state.active_tone_curve_darks = v; }
        if let Some(v) = preset.adjustments.tone_curve_lights { self.context.state.active_tone_curve_lights = v; }
        if let Some(v) = preset.adjustments.tone_curve_highlights { self.context.state.active_tone_curve_highlights = v; }
        
        self.context.state.pending_auto_save = true;
        self.context.state.last_slider_change_time = Some(std::time::Instant::now());
        self.context.state.toasts.success(format!("Applied preset: {}", preset.name));
    }

    fn render_hsl_color(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);
            
            let colors = [
                ("Red", &mut self.context.state.active_hsl_red_sat),
                ("Orange", &mut self.context.state.active_hsl_orange_sat),
                ("Yellow", &mut self.context.state.active_hsl_yellow_sat),
                ("Green", &mut self.context.state.active_hsl_green_sat),
                ("Aqua", &mut self.context.state.active_hsl_aqua_sat),
                ("Blue", &mut self.context.state.active_hsl_blue_sat),
                ("Purple", &mut self.context.state.active_hsl_purple_sat),
                ("Magenta", &mut self.context.state.active_hsl_magenta_sat),
            ];
            
            for (name, value) in colors {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(name).size(Theme::FONT_SM));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("{:+.0}", *value));
                    });
                });
                if ui.add(egui::Slider::new(value, -100.0..=100.0).show_value(false)).changed() {
                    self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                    self.context.state.pending_auto_save = true;
                }
                ui.add_space(Theme::SPACE_SM);
            }
        });
    }

    fn render_hsl_hue(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);
            
            let colors = [
                ("Red", &mut self.context.state.active_hsl_red_hue),
                ("Orange", &mut self.context.state.active_hsl_orange_hue),
                ("Yellow", &mut self.context.state.active_hsl_yellow_hue),
                ("Green", &mut self.context.state.active_hsl_green_hue),
                ("Aqua", &mut self.context.state.active_hsl_aqua_hue),
                ("Blue", &mut self.context.state.active_hsl_blue_hue),
                ("Purple", &mut self.context.state.active_hsl_purple_hue),
                ("Magenta", &mut self.context.state.active_hsl_magenta_hue),
            ];
            
            for (name, value) in colors {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(name).size(Theme::FONT_SM));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("{:+.0}°", *value));
                    });
                });
                if ui.add(egui::Slider::new(value, -180.0..=180.0).show_value(false)).changed() {
                    self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                    self.context.state.pending_auto_save = true;
                }
                ui.add_space(Theme::SPACE_SM);
            }
        });
    }

    fn render_hsl_luminance(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);
            
            let colors = [
                ("Red", &mut self.context.state.active_hsl_red_lum),
                ("Orange", &mut self.context.state.active_hsl_orange_lum),
                ("Yellow", &mut self.context.state.active_hsl_yellow_lum),
                ("Green", &mut self.context.state.active_hsl_green_lum),
                ("Aqua", &mut self.context.state.active_hsl_aqua_lum),
                ("Blue", &mut self.context.state.active_hsl_blue_lum),
                ("Purple", &mut self.context.state.active_hsl_purple_lum),
                ("Magenta", &mut self.context.state.active_hsl_magenta_lum),
            ];
            
            for (name, value) in colors {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(name).size(Theme::FONT_SM));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("{:+.0}", *value));
                    });
                });
                if ui.add(egui::Slider::new(value, -100.0..=100.0).show_value(false)).changed() {
                    self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                    self.context.state.pending_auto_save = true;
                }
                ui.add_space(Theme::SPACE_SM);
            }
        });
    }

    fn render_lens_corrections(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);
            
            // Distortion
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Distortion").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", self.context.state.active_lens_distortion));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_lens_distortion, -100.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            
            ui.add_space(Theme::SPACE_MD);
            
            // Vignette Amount
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Vignette Amount").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", self.context.state.active_lens_vignette_amount));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_lens_vignette_amount, -100.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            
            ui.add_space(Theme::SPACE_SM);
            
            // Vignette Midpoint
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Vignette Midpoint").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", self.context.state.active_lens_vignette_midpoint));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_lens_vignette_midpoint, 0.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
        });
    }

    fn render_detail(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);
            
            widgets::section_title(ui, "Noise Reduction");
            ui.add_space(Theme::SPACE_SM);
            
            // Luminance NR
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Luminance").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", self.context.state.active_nr_luminance));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_nr_luminance, 0.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            
            ui.add_space(Theme::SPACE_SM);
            
            // Color NR
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Color").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", self.context.state.active_nr_color));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_nr_color, 0.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            
            ui.add_space(Theme::SPACE_LG);
            
            widgets::section_title(ui, "Sharpening");
            ui.add_space(Theme::SPACE_SM);
            
            // Sharpen Amount
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Amount").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", self.context.state.active_sharpen_amount));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_sharpen_amount, 0.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            
            ui.add_space(Theme::SPACE_SM);
            
            // Sharpen Radius
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Radius").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.1}", self.context.state.active_sharpen_radius));
                });
            });
            if ui.add(egui::Slider::new(&mut self.context.state.active_sharpen_radius, 0.5..=3.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
        });
    }

    /// Render all adjustments in a single scrollable panel with collapsible sections
    fn render_all_adjustments(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(Theme::SPACE_SM);
                
                // Basic Section (collapsible)
                egui::CollapsingHeader::new(egui::RichText::new("Basic").strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        self.render_basic_sliders(ui);
                    });
                
                ui.add_space(Theme::SPACE_SM);
                
                // Tone Curve Section
                egui::CollapsingHeader::new(egui::RichText::new("Tone Curve").strong())
                    .default_open(false)
                    .show(ui, |ui| {
                        ToneCurveEditor::show(ui, self.context.state);
                    });
                
                ui.add_space(Theme::SPACE_SM);
                
                // Detail Section
                egui::CollapsingHeader::new(egui::RichText::new("Detail").strong())
                    .default_open(false)
                    .show(ui, |ui| {
                        self.render_detail_sliders(ui);
                    });
                
                ui.add_space(Theme::SPACE_SM);
                
                // HSL / Color Section
                egui::CollapsingHeader::new(egui::RichText::new("HSL / Color").strong())
                    .default_open(false)
                    .show(ui, |ui| {
                        self.render_hsl_color_sliders(ui);
                    });
                
                ui.add_space(Theme::SPACE_SM);
                
                // HSL / Luminance Section
                egui::CollapsingHeader::new(egui::RichText::new("HSL / Luminance").strong())
                    .default_open(false)
                    .show(ui, |ui| {
                        self.render_hsl_luminance_sliders(ui);
                    });
                
                ui.add_space(Theme::SPACE_SM);
                
                // HSL / Hue Section
                egui::CollapsingHeader::new(egui::RichText::new("HSL / Hue").strong())
                    .default_open(false)
                    .show(ui, |ui| {
                        self.render_hsl_hue_sliders(ui);
                    });
                
                ui.add_space(Theme::SPACE_SM);
                
                // Lens Corrections Section
                egui::CollapsingHeader::new(egui::RichText::new("Lens Corrections").strong())
                    .default_open(false)
                    .show(ui, |ui| {
                        self.render_lens_corrections_sliders(ui);
                    });
                
                ui.add_space(Theme::SPACE_MD);
                
                // Reset and Export buttons
                ui.horizontal(|ui| {
                    let reset_label = format!("{} Reset All", crate::design_system::icons::ACTION_RESET);
                    if ui.button(&reset_label).clicked() {
                        self.context.state.reset_edits();
                        self.context.state.pending_auto_save = true;
                        self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                    }
                });
                
                ui.add_space(Theme::SPACE_SM);
                
                let export_label = format!("{} Export JPEG", crate::design_system::icons::ACTION_EXPORT);
                if ui.button(&export_label).clicked() {
                    if let Some(metadata) = &self.context.state.detail_metadata {
                        let id = metadata.id.clone();
                        
                        let mut dialog = egui_file::FileDialog::save_file(None)
                            .title("Export Photo")
                            .default_filename("exported.jpg");
                        dialog.open();
                        
                        self.context.state.export_target_id = Some(id);
                        self.context.state.import_dialog = Some(dialog);
                        self.context.state.import_dialog_mode = crate::state::ImportDialogMode::Export;
                    } else {
                        self.context.state.toasts.warning("No photo selected");
                    }
                }
            });
    }

    // Helper methods for collapsible sections (no outer ScrollArea)
    fn render_basic_sliders(&mut self, ui: &mut Ui) {
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

        ui.add_space(Theme::SPACE_SM);

        // Highlights, Shadows, Whites, Blacks
        for (name, value, range) in [
            ("Highlights", &mut self.context.state.active_highlights, -100.0..=100.0),
            ("Shadows", &mut self.context.state.active_shadows, -100.0..=100.0),
            ("Whites", &mut self.context.state.active_whites, -100.0..=100.0),
            ("Blacks", &mut self.context.state.active_blacks, -100.0..=100.0),
        ] {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(name).size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", *value));
                });
            });
            if ui.add(egui::Slider::new(value, range).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
        }

        ui.add_space(Theme::SPACE_SM);

        // Clarity, Vibrance, Saturation
        for (name, value) in [
            ("Clarity", &mut self.context.state.active_clarity),
            ("Vibrance", &mut self.context.state.active_vibrance),
            ("Saturation", &mut self.context.state.active_saturation),
        ] {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(name).size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.2}", *value));
                });
            });
            if ui.add(egui::Slider::new(value, -1.0..=1.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
        }
    }

    fn render_detail_sliders(&mut self, ui: &mut Ui) {
        ui.label(egui::RichText::new("Noise Reduction").size(Theme::FONT_SM).color(ui.visuals().weak_text_color()));
        ui.add_space(Theme::SPACE_XS);
        
        for (name, value) in [
            ("Luminance", &mut self.context.state.active_nr_luminance),
            ("Color", &mut self.context.state.active_nr_color),
        ] {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(name).size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", *value));
                });
            });
            if ui.add(egui::Slider::new(value, 0.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
        }

        ui.add_space(Theme::SPACE_SM);
        ui.label(egui::RichText::new("Sharpening").size(Theme::FONT_SM).color(ui.visuals().weak_text_color()));
        ui.add_space(Theme::SPACE_XS);

        // Sharpen Amount
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Amount").size(Theme::FONT_SM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{:.0}", self.context.state.active_sharpen_amount));
            });
        });
        if ui.add(egui::Slider::new(&mut self.context.state.active_sharpen_amount, 0.0..=100.0).show_value(false)).changed() {
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Sharpen Radius
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Radius").size(Theme::FONT_SM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{:.1}", self.context.state.active_sharpen_radius));
            });
        });
        if ui.add(egui::Slider::new(&mut self.context.state.active_sharpen_radius, 0.5..=3.0).show_value(false)).changed() {
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
    }

    fn render_hsl_color_sliders(&mut self, ui: &mut Ui) {
        let colors = ["Red", "Orange", "Yellow", "Green", "Aqua", "Blue", "Purple", "Magenta"];
        let values = [
            &mut self.context.state.active_hsl_red_sat,
            &mut self.context.state.active_hsl_orange_sat,
            &mut self.context.state.active_hsl_yellow_sat,
            &mut self.context.state.active_hsl_green_sat,
            &mut self.context.state.active_hsl_aqua_sat,
            &mut self.context.state.active_hsl_blue_sat,
            &mut self.context.state.active_hsl_purple_sat,
            &mut self.context.state.active_hsl_magenta_sat,
        ];
        
        for (name, value) in colors.iter().zip(values.into_iter()) {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(*name).size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", *value));
                });
            });
            if ui.add(egui::Slider::new(value, -100.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
        }
    }

    fn render_hsl_luminance_sliders(&mut self, ui: &mut Ui) {
        let colors = ["Red", "Orange", "Yellow", "Green", "Aqua", "Blue", "Purple", "Magenta"];
        let values = [
            &mut self.context.state.active_hsl_red_lum,
            &mut self.context.state.active_hsl_orange_lum,
            &mut self.context.state.active_hsl_yellow_lum,
            &mut self.context.state.active_hsl_green_lum,
            &mut self.context.state.active_hsl_aqua_lum,
            &mut self.context.state.active_hsl_blue_lum,
            &mut self.context.state.active_hsl_purple_lum,
            &mut self.context.state.active_hsl_magenta_lum,
        ];
        
        for (name, value) in colors.iter().zip(values.into_iter()) {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(*name).size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", *value));
                });
            });
            if ui.add(egui::Slider::new(value, -100.0..=100.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
        }
    }

    fn render_hsl_hue_sliders(&mut self, ui: &mut Ui) {
        let colors = ["Red", "Orange", "Yellow", "Green", "Aqua", "Blue", "Purple", "Magenta"];
        let values = [
            &mut self.context.state.active_hsl_red_hue,
            &mut self.context.state.active_hsl_orange_hue,
            &mut self.context.state.active_hsl_yellow_hue,
            &mut self.context.state.active_hsl_green_hue,
            &mut self.context.state.active_hsl_aqua_hue,
            &mut self.context.state.active_hsl_blue_hue,
            &mut self.context.state.active_hsl_purple_hue,
            &mut self.context.state.active_hsl_magenta_hue,
        ];
        
        for (name, value) in colors.iter().zip(values.into_iter()) {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(*name).size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}°", *value));
                });
            });
            if ui.add(egui::Slider::new(value, -180.0..=180.0).show_value(false)).changed() {
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
        }
    }

    fn render_lens_corrections_sliders(&mut self, ui: &mut Ui) {
        // Distortion
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Distortion").size(Theme::FONT_SM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{:+.0}", self.context.state.active_lens_distortion));
            });
        });
        if ui.add(egui::Slider::new(&mut self.context.state.active_lens_distortion, -100.0..=100.0).show_value(false)).changed() {
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Vignette Amount
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Vignette Amount").size(Theme::FONT_SM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{:+.0}", self.context.state.active_lens_vignette_amount));
            });
        });
        if ui.add(egui::Slider::new(&mut self.context.state.active_lens_vignette_amount, -100.0..=100.0).show_value(false)).changed() {
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Vignette Midpoint
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Vignette Midpoint").size(Theme::FONT_SM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{:.0}", self.context.state.active_lens_vignette_midpoint));
            });
        });
        if ui.add(egui::Slider::new(&mut self.context.state.active_lens_vignette_midpoint, 0.0..=100.0).show_value(false)).changed() {
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
    }
}

