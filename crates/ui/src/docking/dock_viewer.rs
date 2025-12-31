//! DockViewer - Implementation of egui_dock::TabViewer
//!
//! Renders the content of each dockable tab based on its type.

use egui::Ui;
use egui_dock::TabViewer;
use std::sync::Arc;
use tokio::sync::mpsc;

use adapters::controllers::*;
use adapters::services::editor_service::EditorService;
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
    advanced_slider::AdvancedSlider,
    crop_panel::CropPanel,
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
    pub editor_service: &'a mut EditorService,
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
                let current_edits = self.context.editor_service.current_edits();
                ToneCurveEditor::show(ui, &current_edits);
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

            DockTab::CropTool => {
                CropPanel::show(ui, self.context.state);
            }

            DockTab::Filmstrip => {
                use crate::state::CurrentView;
                use crate::components::filmstrip::FilmstripAction;
                let current_view = self.context.state.internal_state.current_view;
                
                // For Develop view, we need the filmstrip to update develop_selected_photo_id
                // The show() method updates library_selected_photo_id, so we need to sync
                let prev_library_id = self.context.state.internal_state.library_selected_id.clone();
                
                let action = self.context.filmstrip.show(
                    ui,
                    self.context.ctx,
                    self.context.state,
                );

                // Handle context menu actions
                if let Some(action) = action {
                    match action {
                        FilmstripAction::OpenInDevelop => {
                            if let Some(photo_id) = self.context.state.internal_state.library_selected_id.clone() {
                                self.context.state.internal_state.develop_selected_id = Some(photo_id);
                                self.context.state.internal_state.current_view = CurrentView::Develop;
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

                            } else if let Some(photo_id) = &self.context.state.internal_state.library_selected_id {
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
                        FilmstripAction::ToggleSecondaryWindow => {
                            self.context.state.request_toggle_secondary_window = true;
                        }
                    }
                }
                
                // If we're in Develop mode and library_selected_photo_id changed,
                // sync it to develop_selected_photo_id and trigger image reload
                if current_view == CurrentView::Develop && self.context.state.internal_state.library_selected_id != prev_library_id {
                    if let Some(new_id) = self.context.state.internal_state.library_selected_id.clone() {
                        self.context.state.internal_state.develop_selected_id = Some(new_id);
                        self.context.state.loaded_photo_id = None; // Force reload
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
                self.context.state.internal_state.photo_filters.folder_path.as_deref(),
                &mut self.context.state.expanded_folders,
            );

            if let Some(clicked_path) = folder_tree.show(ui, &self.context.state.folder_tree_roots) {
                self.context.state.internal_state.photo_filters.folder_path = Some(clicked_path);
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
            let current_edits = self.context.editor_service.current_edits();
            let mut exposure = current_edits.exposure;
            
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Exposure").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.2}", exposure));
                });
            });
            if ui.add(egui::Slider::new(&mut exposure, -5.0..=5.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Exposure", |e| e.exposure = exposure);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Contrast
            let mut contrast = current_edits.contrast;
            
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Contrast").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.2}", contrast));
                });
            });
            if ui.add(egui::Slider::new(&mut contrast, 0.0..=2.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Contrast", |e| e.contrast = contrast);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Temperature
            let mut temperature = current_edits.temperature;
            
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Temperature").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.1}", temperature));
                });
            });
            if ui.add(egui::Slider::new(&mut temperature, -10.0..=10.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Temperature", |e| e.temperature = temperature);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Tint
            let mut tint = current_edits.tint;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tint").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.1}", tint));
                });
            });
            if ui.add(egui::Slider::new(&mut tint, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Tint", |e| e.tint = tint);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_MD);
            ui.separator();
            ui.add_space(Theme::SPACE_SM);

            // Highlights
            let mut highlights = current_edits.highlights;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Highlights").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", highlights));
                });
            });
            if ui.add(egui::Slider::new(&mut highlights, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Highlights", |e| e.highlights = highlights);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Shadows
            let mut shadows = current_edits.shadows;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Shadows").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", shadows));
                });
            });
            if ui.add(egui::Slider::new(&mut shadows, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Shadows", |e| e.shadows = shadows);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Whites
            let mut whites = current_edits.whites;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Whites").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", whites));
                });
            });
            if ui.add(egui::Slider::new(&mut whites, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Whites", |e| e.whites = whites);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Blacks
            let mut blacks = current_edits.blacks;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Blacks").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", blacks));
                });
            });
            if ui.add(egui::Slider::new(&mut blacks, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Blacks", |e| e.blacks = blacks);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_MD);
            ui.separator();
            ui.add_space(Theme::SPACE_SM);

            // Clarity
            let mut clarity = current_edits.clarity;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Clarity").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.2}", clarity));
                });
            });
            if ui.add(egui::Slider::new(&mut clarity, -1.0..=1.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Clarity", |e| e.clarity = clarity);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Vibrance
            let mut vibrance = current_edits.vibrance;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Vibrance").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.2}", vibrance));
                });
            });
            if ui.add(egui::Slider::new(&mut vibrance, -1.0..=1.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Vibrance", |e| e.vibrance = vibrance);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_SM);

            // Saturation
            let mut saturation = current_edits.saturation;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Saturation").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.2}", saturation));
                });
            });
            if ui.add(egui::Slider::new(&mut saturation, -1.0..=1.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Saturation", |e| e.saturation = saturation);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }

            ui.add_space(Theme::SPACE_MD);

            // Reset button with icon
            let reset_label = format!("{} Reset All", crate::design_system::icons::ACTION_RESET);
            if ui.button(&reset_label).clicked() {
                // Use EditorService to reset all fields at once
                let _ = self.context.editor_service.update_field("Reset All", |e| {
                    e.exposure = 0.0;
                    e.contrast = 1.0;
                    e.temperature = 0.0;
                    e.tint = 0.0;
                    e.highlights = 0.0;
                    e.shadows = 0.0;
                    e.whites = 0.0;
                    e.blacks = 0.0;
                    e.clarity = 0.0;
                    e.vibrance = 0.0;
                    e.saturation = 0.0;
                });
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
        // Use EditorService to apply all preset adjustments
        let preset_name = preset.name.clone();
        let _ = self.context.editor_service.update_field(&format!("Preset: {}", preset_name), |e| {
            if let Some(v) = preset.adjustments.exposure { e.exposure = v; }
            if let Some(v) = preset.adjustments.contrast { e.contrast = v; }
            if let Some(v) = preset.adjustments.temperature { e.temperature = v; }
            if let Some(v) = preset.adjustments.tint { e.tint = v; }
            if let Some(v) = preset.adjustments.highlights { e.highlights = v; }
            if let Some(v) = preset.adjustments.shadows { e.shadows = v; }
            if let Some(v) = preset.adjustments.whites { e.whites = v; }
            if let Some(v) = preset.adjustments.blacks { e.blacks = v; }
            if let Some(v) = preset.adjustments.clarity { e.clarity = v; }
            if let Some(v) = preset.adjustments.vibrance { e.vibrance = v; }
            if let Some(v) = preset.adjustments.saturation { e.saturation = v; }
            if let Some(v) = preset.adjustments.tone_curve_shadows { e.tone_curve_shadows = v; }
            if let Some(v) = preset.adjustments.tone_curve_darks { e.tone_curve_darks = v; }
            if let Some(v) = preset.adjustments.tone_curve_lights { e.tone_curve_lights = v; }
            if let Some(v) = preset.adjustments.tone_curve_highlights { e.tone_curve_highlights = v; }
        });
        
        self.context.state.pending_auto_save = true;
        self.context.state.last_slider_change_time = Some(std::time::Instant::now());
        self.context.state.toasts.success(format!("Applied preset: {}", preset_name));
    }

    fn render_hsl_color(&mut self, ui: &mut Ui) {
        let current_edits = self.context.editor_service.current_edits();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);

            // Red
            let mut red = current_edits.hsl_red_sat;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Red").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", red));
                });
            });
            if ui.add(egui::Slider::new(&mut red, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Red Sat", |e| e.hsl_red_sat = red);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Orange
            let mut orange = current_edits.hsl_orange_sat;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Orange").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", orange));
                });
            });
            if ui.add(egui::Slider::new(&mut orange, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Orange Sat", |e| e.hsl_orange_sat = orange);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Yellow
            let mut yellow = current_edits.hsl_yellow_sat;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Yellow").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", yellow));
                });
            });
            if ui.add(egui::Slider::new(&mut yellow, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Yellow Sat", |e| e.hsl_yellow_sat = yellow);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Green
            let mut green = current_edits.hsl_green_sat;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Green").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", green));
                });
            });
            if ui.add(egui::Slider::new(&mut green, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Green Sat", |e| e.hsl_green_sat = green);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Aqua
            let mut aqua = current_edits.hsl_aqua_sat;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Aqua").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", aqua));
                });
            });
            if ui.add(egui::Slider::new(&mut aqua, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Aqua Sat", |e| e.hsl_aqua_sat = aqua);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Blue
            let mut blue = current_edits.hsl_blue_sat;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Blue").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", blue));
                });
            });
            if ui.add(egui::Slider::new(&mut blue, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Blue Sat", |e| e.hsl_blue_sat = blue);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Purple
            let mut purple = current_edits.hsl_purple_sat;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Purple").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", purple));
                });
            });
            if ui.add(egui::Slider::new(&mut purple, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Purple Sat", |e| e.hsl_purple_sat = purple);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Magenta
            let mut magenta = current_edits.hsl_magenta_sat;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Magenta").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", magenta));
                });
            });
            if ui.add(egui::Slider::new(&mut magenta, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Magenta Sat", |e| e.hsl_magenta_sat = magenta);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);
        });
    }

    fn render_hsl_hue(&mut self, ui: &mut Ui) {
        let current_edits = self.context.editor_service.current_edits();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);

            // Red
            let mut red = current_edits.hsl_red_hue;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Red").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}°", red));
                });
            });
            if ui.add(egui::Slider::new(&mut red, -180.0..=180.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Red Hue", |e| e.hsl_red_hue = red);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Orange
            let mut orange = current_edits.hsl_orange_hue;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Orange").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}°", orange));
                });
            });
            if ui.add(egui::Slider::new(&mut orange, -180.0..=180.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Orange Hue", |e| e.hsl_orange_hue = orange);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Yellow
            let mut yellow = current_edits.hsl_yellow_hue;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Yellow").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}°", yellow));
                });
            });
            if ui.add(egui::Slider::new(&mut yellow, -180.0..=180.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Yellow Hue", |e| e.hsl_yellow_hue = yellow);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Green
            let mut green = current_edits.hsl_green_hue;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Green").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}°", green));
                });
            });
            if ui.add(egui::Slider::new(&mut green, -180.0..=180.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Green Hue", |e| e.hsl_green_hue = green);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Aqua
            let mut aqua = current_edits.hsl_aqua_hue;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Aqua").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}°", aqua));
                });
            });
            if ui.add(egui::Slider::new(&mut aqua, -180.0..=180.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Aqua Hue", |e| e.hsl_aqua_hue = aqua);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Blue
            let mut blue = current_edits.hsl_blue_hue;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Blue").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}°", blue));
                });
            });
            if ui.add(egui::Slider::new(&mut blue, -180.0..=180.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Blue Hue", |e| e.hsl_blue_hue = blue);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Purple
            let mut purple = current_edits.hsl_purple_hue;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Purple").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}°", purple));
                });
            });
            if ui.add(egui::Slider::new(&mut purple, -180.0..=180.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Purple Hue", |e| e.hsl_purple_hue = purple);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Magenta
            let mut magenta = current_edits.hsl_magenta_hue;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Magenta").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}°", magenta));
                });
            });
            if ui.add(egui::Slider::new(&mut magenta, -180.0..=180.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Magenta Hue", |e| e.hsl_magenta_hue = magenta);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);
        });
    }

    fn render_hsl_luminance(&mut self, ui: &mut Ui) {
        let current_edits = self.context.editor_service.current_edits();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);

            // Red
            let mut red = current_edits.hsl_red_lum;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Red").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", red));
                });
            });
            if ui.add(egui::Slider::new(&mut red, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Red Lum", |e| e.hsl_red_lum = red);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Orange
            let mut orange = current_edits.hsl_orange_lum;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Orange").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", orange));
                });
            });
            if ui.add(egui::Slider::new(&mut orange, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Orange Lum", |e| e.hsl_orange_lum = orange);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Yellow
            let mut yellow = current_edits.hsl_yellow_lum;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Yellow").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", yellow));
                });
            });
            if ui.add(egui::Slider::new(&mut yellow, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Yellow Lum", |e| e.hsl_yellow_lum = yellow);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Green
            let mut green = current_edits.hsl_green_lum;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Green").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", green));
                });
            });
            if ui.add(egui::Slider::new(&mut green, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Green Lum", |e| e.hsl_green_lum = green);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Aqua
            let mut aqua = current_edits.hsl_aqua_lum;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Aqua").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", aqua));
                });
            });
            if ui.add(egui::Slider::new(&mut aqua, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Aqua Lum", |e| e.hsl_aqua_lum = aqua);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Blue
            let mut blue = current_edits.hsl_blue_lum;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Blue").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", blue));
                });
            });
            if ui.add(egui::Slider::new(&mut blue, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Blue Lum", |e| e.hsl_blue_lum = blue);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Purple
            let mut purple = current_edits.hsl_purple_lum;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Purple").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", purple));
                });
            });
            if ui.add(egui::Slider::new(&mut purple, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Purple Lum", |e| e.hsl_purple_lum = purple);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);

            // Magenta
            let mut magenta = current_edits.hsl_magenta_lum;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Magenta").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", magenta));
                });
            });
            if ui.add(egui::Slider::new(&mut magenta, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("HSL Magenta Lum", |e| e.hsl_magenta_lum = magenta);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            ui.add_space(Theme::SPACE_SM);
        });
    }

    fn render_lens_corrections(&mut self, ui: &mut Ui) {
        let current_edits = self.context.editor_service.current_edits();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);
            
            // Distortion
            let mut distortion = current_edits.lens_distortion;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Distortion").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", distortion));
                });
            });
            if ui.add(egui::Slider::new(&mut distortion, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Distortion", |e| e.lens_distortion = distortion);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            
            ui.add_space(Theme::SPACE_MD);
            
            // Vignette Amount
            let mut vignette = current_edits.lens_vignette_amount;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Vignette Amount").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:+.0}", vignette));
                });
            });
            if ui.add(egui::Slider::new(&mut vignette, -100.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Vignette Amount", |e| e.lens_vignette_amount = vignette);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            
            ui.add_space(Theme::SPACE_SM);
            
            // Vignette Midpoint
            let mut midpoint = current_edits.lens_vignette_midpoint;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Vignette Midpoint").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", midpoint));
                });
            });
            if ui.add(egui::Slider::new(&mut midpoint, 0.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Vignette Midpoint", |e| e.lens_vignette_midpoint = midpoint);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
        });
    }

    fn render_detail(&mut self, ui: &mut Ui) {
        let current_edits = self.context.editor_service.current_edits();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);
            
            widgets::section_title(ui, "Noise Reduction");
            ui.add_space(Theme::SPACE_SM);
            
            // Luminance NR
            let mut nr_lum = current_edits.nr_luminance;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Luminance").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", nr_lum));
                });
            });
            if ui.add(egui::Slider::new(&mut nr_lum, 0.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Luminance NR", |e| e.nr_luminance = nr_lum);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            
            ui.add_space(Theme::SPACE_SM);
            
            // Color NR
            let mut nr_col = current_edits.nr_color;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Color").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", nr_col));
                });
            });
            if ui.add(egui::Slider::new(&mut nr_col, 0.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Color NR", |e| e.nr_color = nr_col);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            
            ui.add_space(Theme::SPACE_LG);
            
            widgets::section_title(ui, "Sharpening");
            ui.add_space(Theme::SPACE_SM);
            
            // Sharpen Amount
            let mut sharpen = current_edits.sharpen_amount;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Amount").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.0}", sharpen));
                });
            });
            if ui.add(egui::Slider::new(&mut sharpen, 0.0..=100.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Sharpen Amount", |e| e.sharpen_amount = sharpen);
                self.context.state.last_slider_change_time = Some(std::time::Instant::now());
                self.context.state.pending_auto_save = true;
            }
            
            ui.add_space(Theme::SPACE_SM);
            
            // Sharpen Radius
            let mut radius = current_edits.sharpen_radius;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Radius").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.1}", radius));
                });
            });
            if ui.add(egui::Slider::new(&mut radius, 0.5..=3.0).show_value(false)).changed() {
                let _ = self.context.editor_service.update_field("Sharpen Radius", |e| e.sharpen_radius = radius);
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
                
                // Crop & Straighten Section (collapsible)
                // Crop & Straighten Section (collapsible)
                let is_open = self.context.state.crop_mode_active;
                let response = egui::CollapsingHeader::new(egui::RichText::new("Crop & Straighten").strong())
                    .open(Some(is_open))
                    .show(ui, |ui| {
                        CropPanel::show(ui, self.context.state);
                    });

                if response.header_response.clicked() {
                    self.context.state.crop_mode_active = !is_open;
                }
                
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
                        let current_edits = self.context.editor_service.current_edits();
                        ToneCurveEditor::show(ui, &current_edits);
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
                        let _ = self.context.editor_service.update_field("Reset All", |e| {
                            *e = Default::default();
                        });
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
        let current_edits = self.context.editor_service.current_edits();
        
        // Exposure (default: 0.0)
        let mut exposure = current_edits.exposure;
        if AdvancedSlider::show(ui, "exposure", "Exposure", &mut exposure, -5.0..=5.0, 0.0, 2) {
            let _ = self.context.editor_service.update_field("Exposure", |e| e.exposure = exposure);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Contrast (default: 1.0)
        let mut contrast = current_edits.contrast;
        if AdvancedSlider::show(ui, "contrast", "Contrast", &mut contrast, 0.0..=2.0, 1.0, 2) {
            let _ = self.context.editor_service.update_field("Contrast", |e| e.contrast = contrast);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Temperature (default: 0.0)
        let mut temperature = current_edits.temperature;
        if AdvancedSlider::show(ui, "temperature", "Temperature", &mut temperature, -10.0..=10.0, 0.0, 1) {
            let _ = self.context.editor_service.update_field("Temperature", |e| e.temperature = temperature);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Tint (default: 0.0)
        let mut tint = current_edits.tint;
        if AdvancedSlider::show(ui, "tint", "Tint", &mut tint, -10.0..=10.0, 0.0, 1) {
            let _ = self.context.editor_service.update_field("Tint", |e| e.tint = tint);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Highlights (default: 0.0)
        let mut highlights = current_edits.highlights;
        if AdvancedSlider::show(ui, "highlights", "Highlights", &mut highlights, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("Highlights", |e| e.highlights = highlights);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Shadows (default: 0.0)
        let mut shadows = current_edits.shadows;
        if AdvancedSlider::show(ui, "shadows", "Shadows", &mut shadows, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("Shadows", |e| e.shadows = shadows);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Whites (default: 0.0)
        let mut whites = current_edits.whites;
        if AdvancedSlider::show(ui, "whites", "Whites", &mut whites, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("Whites", |e| e.whites = whites);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Blacks (default: 0.0)
        let mut blacks = current_edits.blacks;
        if AdvancedSlider::show(ui, "blacks", "Blacks", &mut blacks, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("Blacks", |e| e.blacks = blacks);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Clarity (default: 0.0)
        let mut clarity = current_edits.clarity;
        if AdvancedSlider::show(ui, "clarity", "Clarity", &mut clarity, -1.0..=1.0, 0.0, 2) {
            let _ = self.context.editor_service.update_field("Clarity", |e| e.clarity = clarity);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Vibrance (default: 0.0)
        let mut vibrance = current_edits.vibrance;
        if AdvancedSlider::show(ui, "vibrance", "Vibrance", &mut vibrance, -1.0..=1.0, 0.0, 2) {
            let _ = self.context.editor_service.update_field("Vibrance", |e| e.vibrance = vibrance);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Saturation (default: 0.0)
        let mut saturation = current_edits.saturation;
        if AdvancedSlider::show(ui, "saturation", "Saturation", &mut saturation, -1.0..=1.0, 0.0, 2) {
            let _ = self.context.editor_service.update_field("Saturation", |e| e.saturation = saturation);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
    }

    fn render_detail_sliders(&mut self, ui: &mut Ui) {
        let current_edits = self.context.editor_service.current_edits();
        ui.label(egui::RichText::new("Noise Reduction").size(Theme::FONT_SM).color(ui.visuals().weak_text_color()));
        ui.add_space(Theme::SPACE_XS);
        
        // NR Luminance (default: 0.0)
        let mut nr_luminance = current_edits.nr_luminance;
        if AdvancedSlider::show(ui, "nr_luminance", "Luminance", &mut nr_luminance, 0.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("Luminance NR", |e| e.nr_luminance = nr_luminance);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // NR Color (default: 0.0)
        let mut nr_color = current_edits.nr_color;
        if AdvancedSlider::show(ui, "nr_color", "Color", &mut nr_color, 0.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("Color NR", |e| e.nr_color = nr_color);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        ui.add_space(Theme::SPACE_SM);
        ui.label(egui::RichText::new("Sharpening").size(Theme::FONT_SM).color(ui.visuals().weak_text_color()));
        ui.add_space(Theme::SPACE_XS);

        // Sharpen Amount (default: 0.0)
        let mut sharpen_amount = current_edits.sharpen_amount;
        if AdvancedSlider::show(ui, "sharpen_amount", "Amount", &mut sharpen_amount, 0.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("Sharpen Amount", |e| e.sharpen_amount = sharpen_amount);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Sharpen Radius (default: 1.0)
        let mut sharpen_radius = current_edits.sharpen_radius;
        if AdvancedSlider::show(ui, "sharpen_radius", "Radius", &mut sharpen_radius, 0.5..=3.0, 1.0, 1) {
            let _ = self.context.editor_service.update_field("Sharpen Radius", |e| e.sharpen_radius = sharpen_radius);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
    }

    fn render_hsl_color_sliders(&mut self, ui: &mut Ui) {
        let current_edits = self.context.editor_service.current_edits();
        
        let mut red = current_edits.hsl_red_sat;
        if AdvancedSlider::show(ui, "hsl_red_sat", "Red", &mut red, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Red Sat", |e| e.hsl_red_sat = red);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut orange = current_edits.hsl_orange_sat;
        if AdvancedSlider::show(ui, "hsl_orange_sat", "Orange", &mut orange, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Orange Sat", |e| e.hsl_orange_sat = orange);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut yellow = current_edits.hsl_yellow_sat;
        if AdvancedSlider::show(ui, "hsl_yellow_sat", "Yellow", &mut yellow, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Yellow Sat", |e| e.hsl_yellow_sat = yellow);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut green = current_edits.hsl_green_sat;
        if AdvancedSlider::show(ui, "hsl_green_sat", "Green", &mut green, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Green Sat", |e| e.hsl_green_sat = green);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut aqua = current_edits.hsl_aqua_sat;
        if AdvancedSlider::show(ui, "hsl_aqua_sat", "Aqua", &mut aqua, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Aqua Sat", |e| e.hsl_aqua_sat = aqua);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
        
        let mut blue = current_edits.hsl_blue_sat;
        if AdvancedSlider::show(ui, "hsl_blue_sat", "Blue", &mut blue, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Blue Sat", |e| e.hsl_blue_sat = blue);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
        
        let mut purple = current_edits.hsl_purple_sat;
        if AdvancedSlider::show(ui, "hsl_purple_sat", "Purple", &mut purple, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Purple Sat", |e| e.hsl_purple_sat = purple);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
        
        let mut magenta = current_edits.hsl_magenta_sat;
        if AdvancedSlider::show(ui, "hsl_magenta_sat", "Magenta", &mut magenta, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Magenta Sat", |e| e.hsl_magenta_sat = magenta);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
    }

    fn render_hsl_luminance_sliders(&mut self, ui: &mut Ui) {
        let current_edits = self.context.editor_service.current_edits();

        let mut red = current_edits.hsl_red_lum;
        if AdvancedSlider::show(ui, "hsl_red_lum", "Red", &mut red, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Red Lum", |e| e.hsl_red_lum = red);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut orange = current_edits.hsl_orange_lum;
        if AdvancedSlider::show(ui, "hsl_orange_lum", "Orange", &mut orange, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Orange Lum", |e| e.hsl_orange_lum = orange);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut yellow = current_edits.hsl_yellow_lum;
        if AdvancedSlider::show(ui, "hsl_yellow_lum", "Yellow", &mut yellow, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Yellow Lum", |e| e.hsl_yellow_lum = yellow);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut green = current_edits.hsl_green_lum;
        if AdvancedSlider::show(ui, "hsl_green_lum", "Green", &mut green, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Green Lum", |e| e.hsl_green_lum = green);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut aqua = current_edits.hsl_aqua_lum;
        if AdvancedSlider::show(ui, "hsl_aqua_lum", "Aqua", &mut aqua, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Aqua Lum", |e| e.hsl_aqua_lum = aqua);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
        
        let mut blue = current_edits.hsl_blue_lum;
        if AdvancedSlider::show(ui, "hsl_blue_lum", "Blue", &mut blue, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Blue Lum", |e| e.hsl_blue_lum = blue);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
        
        let mut purple = current_edits.hsl_purple_lum;
        if AdvancedSlider::show(ui, "hsl_purple_lum", "Purple", &mut purple, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Purple Lum", |e| e.hsl_purple_lum = purple);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
        
        let mut magenta = current_edits.hsl_magenta_lum;
        if AdvancedSlider::show(ui, "hsl_magenta_lum", "Magenta", &mut magenta, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Magenta Lum", |e| e.hsl_magenta_lum = magenta);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
    }

    fn render_hsl_hue_sliders(&mut self, ui: &mut Ui) {
        let current_edits = self.context.editor_service.current_edits();

        let mut red = current_edits.hsl_red_hue;
        if AdvancedSlider::show(ui, "hsl_red_hue", "Red", &mut red, -180.0..=180.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Red Hue", |e| e.hsl_red_hue = red);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut orange = current_edits.hsl_orange_hue;
        if AdvancedSlider::show(ui, "hsl_orange_hue", "Orange", &mut orange, -180.0..=180.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Orange Hue", |e| e.hsl_orange_hue = orange);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut yellow = current_edits.hsl_yellow_hue;
        if AdvancedSlider::show(ui, "hsl_yellow_hue", "Yellow", &mut yellow, -180.0..=180.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Yellow Hue", |e| e.hsl_yellow_hue = yellow);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut green = current_edits.hsl_green_hue;
        if AdvancedSlider::show(ui, "hsl_green_hue", "Green", &mut green, -180.0..=180.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Green Hue", |e| e.hsl_green_hue = green);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        let mut aqua = current_edits.hsl_aqua_hue;
        if AdvancedSlider::show(ui, "hsl_aqua_hue", "Aqua", &mut aqua, -180.0..=180.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Aqua Hue", |e| e.hsl_aqua_hue = aqua);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
        
        let mut blue = current_edits.hsl_blue_hue;
        if AdvancedSlider::show(ui, "hsl_blue_hue", "Blue", &mut blue, -180.0..=180.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Blue Hue", |e| e.hsl_blue_hue = blue);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
        
        let mut purple = current_edits.hsl_purple_hue;
        if AdvancedSlider::show(ui, "hsl_purple_hue", "Purple", &mut purple, -180.0..=180.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Purple Hue", |e| e.hsl_purple_hue = purple);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
        
        let mut magenta = current_edits.hsl_magenta_hue;
        if AdvancedSlider::show(ui, "hsl_magenta_hue", "Magenta", &mut magenta, -180.0..=180.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("HSL Magenta Hue", |e| e.hsl_magenta_hue = magenta);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
    }

    fn render_lens_corrections_sliders(&mut self, ui: &mut Ui) {
        let current_edits = self.context.editor_service.current_edits();
        
        // Distortion (default: 0.0)
        let mut lens_distortion = current_edits.lens_distortion;
        if AdvancedSlider::show(ui, "lens_distortion", "Distortion", &mut lens_distortion, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("Distortion", |e| e.lens_distortion = lens_distortion);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Vignette Amount (default: 0.0)
        let mut lens_vignette_amount = current_edits.lens_vignette_amount;
        if AdvancedSlider::show(ui, "lens_vignette_amount", "Vignette Amount", &mut lens_vignette_amount, -100.0..=100.0, 0.0, 0) {
            let _ = self.context.editor_service.update_field("Vignette Amount", |e| e.lens_vignette_amount = lens_vignette_amount);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }

        // Vignette Midpoint (default: 50.0)
        let mut lens_vignette_midpoint = current_edits.lens_vignette_midpoint;
        if AdvancedSlider::show(ui, "lens_vignette_midpoint", "Vignette Midpoint", &mut lens_vignette_midpoint, 0.0..=100.0, 50.0, 0) {
            let _ = self.context.editor_service.update_field("Vignette Midpoint", |e| e.lens_vignette_midpoint = lens_vignette_midpoint);
            self.context.state.last_slider_change_time = Some(std::time::Instant::now());
            self.context.state.pending_auto_save = true;
        }
    }
}

