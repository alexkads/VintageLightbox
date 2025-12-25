// Develop View
// Image editing view with adjustments panel

use egui::Ui;
use crate::state::AppState;
use crate::design_system::theme::Theme;
use crate::components::image_viewer::ImageViewer;
use crate::components::filmstrip::Filmstrip;
use std::sync::Arc;
use infrastructure::cache::preview_manager::PreviewManager;

use crate::panels::PresetsPanel;

#[allow(dead_code)]
pub struct DevelopView {
    filmstrip: Filmstrip,
    presets_panel: PresetsPanel,
}

#[allow(dead_code)]
impl DevelopView {
    pub fn new(preview_manager: Arc<PreviewManager>) -> Self {
        let presets_panel = PresetsPanel::new();
        
        Self {
            filmstrip: Filmstrip::new(preview_manager),
            presets_panel,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        state: &mut AppState,
        editor_controller: &std::sync::Arc<adapters::controllers::EditorController>,
        export_controller: &std::sync::Arc<adapters::controllers::ExportController>,
        photo_controller: &std::sync::Arc<adapters::controllers::PhotoController>,
        library_controller: &std::sync::Arc<adapters::controllers::LibraryController>,
        photo_sender: &tokio::sync::mpsc::Sender<Result<Vec<adapters::view_models::PhotoViewModel>, String>>,
        ctx: &egui::Context,
    ) {
        // Bottom filmstrip (must be first to reserve space)
        egui::TopBottomPanel::bottom("filmstrip_develop")
            .exact_height(120.0)  // 80px thumbnails + 40px padding
            .show_inside(ui, |ui| {
                let selected_id = state.develop_selected_photo_id.clone();
                
                self.filmstrip.show_develop(
                    ui,
                    ctx,
                    &state.photos,
                    &selected_id,
                    &mut state.filmstrip_filter,
                    |photo_id| {
                        // Select photo and trigger load in Develop view (independent from Library)
                        state.develop_selected_photo_id = Some(photo_id.clone());
                        // Clear current image to trigger reload
                        state.loaded_photo_id = None;
                        ctx.request_repaint();
                    },
                    |photo_id, flag_code| {
                        // Handle flag
                        let controller = photo_controller.clone();
                        let library_controller = library_controller.clone();
                        let photo_sender = photo_sender.clone();
                        let ctx_clone = ctx.clone();
                        
                        tokio::spawn(async move {
                            let _ = controller.set_flag(&photo_id, flag_code).await;
                            // Reload
                            if let Ok(photos) = library_controller.get_all_photos().await {
                                     let _ = photo_sender.send(Ok(photos)).await;
                            }
                            ctx_clone.request_repaint();
                        });
                    }
                );
            });

        // Left sidebar - Presets & History
        egui::SidePanel::left("develop_left")
            .resizable(false)
            .exact_width(Theme::SIDEBAR_WIDTH)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_left_sidebar(ui, state);
                });
            });

        // Right sidebar - Adjustments
        egui::SidePanel::right("develop_right")
            .resizable(false)
            .exact_width(Theme::PANEL_WIDTH + 20.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_right_sidebar(ui, state, editor_controller, export_controller, photo_controller, library_controller, photo_sender, ctx);
                });
            });

        // Center - Image viewer
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ImageViewer::show(ui, state);
        });
    }

    fn show_left_sidebar(&mut self, ui: &mut Ui, state: &mut AppState) {
        use crate::design_system::widgets;

        // Presets Panel - update from state.presets
        let system_presets: Vec<_> = state.presets.iter()
            .filter(|p| p.is_system)
            .cloned()
            .collect();
        let user_presets: Vec<_> = state.presets.iter()
            .filter(|p| !p.is_system)
            .cloned()
            .collect();
        self.presets_panel.set_presets(system_presets, user_presets);

        widgets::section_title(ui, "Presets");
        ui.add_space(Theme::SPACE_SM);

        if let Some(preset) = self.presets_panel.ui(ui, &state.selected_theme) {
             // Apply preset logic
             if let Some(v) = preset.adjustments.exposure { state.active_exposure = v; }
             if let Some(v) = preset.adjustments.contrast { state.active_contrast = v; }
             if let Some(v) = preset.adjustments.temperature { state.active_temperature = v; }
             if let Some(v) = preset.adjustments.tint { state.active_tint = v; }
             if let Some(v) = preset.adjustments.highlights { state.active_highlights = v; }
             if let Some(v) = preset.adjustments.shadows { state.active_shadows = v; }
             if let Some(v) = preset.adjustments.whites { state.active_whites = v; }
             if let Some(v) = preset.adjustments.blacks { state.active_blacks = v; }
             if let Some(v) = preset.adjustments.clarity { state.active_clarity = v; }
             if let Some(v) = preset.adjustments.vibrance { state.active_vibrance = v; }
             if let Some(v) = preset.adjustments.saturation { state.active_saturation = v; }
             
             if let Some(v) = preset.adjustments.tone_curve_shadows { state.active_tone_curve_shadows = v; }
             if let Some(v) = preset.adjustments.tone_curve_darks { state.active_tone_curve_darks = v; }
             if let Some(v) = preset.adjustments.tone_curve_lights { state.active_tone_curve_lights = v; }
             if let Some(v) = preset.adjustments.tone_curve_highlights { state.active_tone_curve_highlights = v; }

             state.pending_auto_save = true;
             state.last_slider_change_time = Some(std::time::Instant::now());
             
             // Trigger repaint
             ui.ctx().request_repaint();
        }

        ui.add_space(Theme::SPACE_MD);

        // Save as Preset button
        if widgets::secondary_button(ui, "+ Save as Preset").clicked() {
            state.show_save_preset_dialog = true;
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
        _editor_controller: &std::sync::Arc<adapters::controllers::EditorController>,
        export_controller: &std::sync::Arc<adapters::controllers::ExportController>,
        photo_controller: &std::sync::Arc<adapters::controllers::PhotoController>,
        library_controller: &std::sync::Arc<adapters::controllers::LibraryController>,
        photo_sender: &tokio::sync::mpsc::Sender<Result<Vec<adapters::view_models::PhotoViewModel>, String>>,
        ctx: &egui::Context,
    ) {
        use crate::design_system::widgets;
        use crate::components::{
            histogram_plot::HistogramPlot,
            tone_curve::ToneCurveEditor,
            slider_control::SliderControl,
            rating_widget::RatingWidget
        };

        // Interactive Histogram
        HistogramPlot::show(ui, state.histogram_data.as_ref());
        ui.add_space(Theme::SPACE_MD);

        // Tone Curve Visualization
        ToneCurveEditor::show(ui, state);
        ui.add_space(Theme::SPACE_MD);

        // Basic Adjustments
        widgets::section_title(ui, "Basic");
        ui.add_space(Theme::SPACE_SM);

        // Track if any slider changed for auto-save
        let mut any_slider_changed = false;

        // Exposure slider
        if SliderControl::show(
            ui,
            "Exposure",
            &mut state.active_exposure,
            -2.0..=2.0,
            0.1,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Contrast slider
        if SliderControl::show(
            ui,
            "Contrast",
            &mut state.active_contrast,
            0.5..=1.5,
            0.05,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Temperature slider
        if SliderControl::show(
            ui,
            "Temperature",
            &mut state.active_temperature,
            -10.0..=10.0,
            0.5,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Tint slider
        if SliderControl::show(
            ui,
            "Tint",
            &mut state.active_tint,
            -10.0..=10.0,
            0.5,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Highlights slider
        if SliderControl::show(
            ui,
            "Highlights",
            &mut state.active_highlights,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Shadows slider
        if SliderControl::show(
            ui,
            "Shadows",
            &mut state.active_shadows,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Whites slider
        if SliderControl::show(
            ui,
            "Whites",
            &mut state.active_whites,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Blacks slider
        if SliderControl::show(
            ui,
            "Blacks",
            &mut state.active_blacks,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Clarity slider
        if SliderControl::show(
            ui,
            "Clarity",
            &mut state.active_clarity,
            -1.0..=1.0,
            0.05,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Vibrance slider
        if SliderControl::show(
            ui,
            "Vibrance",
            &mut state.active_vibrance,
            -1.0..=1.0,
            0.05,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Saturation slider
        if SliderControl::show(
            ui,
            "Saturation",
            &mut state.active_saturation,
            -1.0..=1.0,
            0.05,
        ) {
            any_slider_changed = true;
        }

        // Mark for auto-save if any slider changed
        if any_slider_changed {
            state.pending_auto_save = true;
            state.last_slider_change_time = Some(std::time::Instant::now());
        }

        ui.add_space(Theme::SPACE_LG);

        // Tone Curve Section
        widgets::section_title(ui, "Tone Curve");
        ui.add_space(Theme::SPACE_SM);

        // Shadows (darkest tones)
        if SliderControl::show(
            ui,
            "Shadows",
            &mut state.active_tone_curve_shadows,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Darks (dark midtones)
        if SliderControl::show(
            ui,
            "Darks",
            &mut state.active_tone_curve_darks,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Lights (light midtones)
        if SliderControl::show(
            ui,
            "Lights",
            &mut state.active_tone_curve_lights,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Highlights (brightest tones)
        if SliderControl::show(
            ui,
            "Highlights (Curve)",
            &mut state.active_tone_curve_highlights,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_LG);

        // HSL / Color Section
        widgets::section_title(ui, "HSL / Color");
        ui.add_space(Theme::SPACE_SM);

        // Red saturation
        if SliderControl::show(
            ui,
            "Red",
            &mut state.active_hsl_red_sat,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Orange saturation
        if SliderControl::show(
            ui,
            "Orange",
            &mut state.active_hsl_orange_sat,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Yellow saturation
        if SliderControl::show(
            ui,
            "Yellow",
            &mut state.active_hsl_yellow_sat,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Green saturation
        if SliderControl::show(
            ui,
            "Green",
            &mut state.active_hsl_green_sat,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Aqua saturation
        if SliderControl::show(
            ui,
            "Aqua",
            &mut state.active_hsl_aqua_sat,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Blue saturation
        if SliderControl::show(
            ui,
            "Blue",
            &mut state.active_hsl_blue_sat,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Purple saturation
        if SliderControl::show(
            ui,
            "Purple",
            &mut state.active_hsl_purple_sat,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Magenta saturation
        if SliderControl::show(
            ui,
            "Magenta",
            &mut state.active_hsl_magenta_sat,
            -100.0..=100.0,
            5.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_XXL);

        // Detail (Noise Reduction)
        widgets::section_title(ui, "Detail");
        ui.add_space(Theme::SPACE_SM);

        if SliderControl::show(
            ui,
            "Luminance NR",
            &mut state.active_nr_luminance,
            0.0..=100.0,
            1.0,
        ) {
            any_slider_changed = true;
        }

        if SliderControl::show(
            ui,
            "Color NR",
            &mut state.active_nr_color,
            0.0..=100.0,
            1.0,
        ) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_XXL);

        // Reset button
        if widgets::secondary_button(ui, "Reset").clicked() {
            // Reset adjustments to default values
            state.reset_edits();

            // Auto-save happens after reset too
            state.pending_auto_save = true;
            state.last_slider_change_time = Some(std::time::Instant::now());
        }

        ui.add_space(Theme::SPACE_SM);

        if widgets::secondary_button(ui, "Export").clicked() {
            if let Some(photo) = &state.detail_metadata {
                 let mut dialog = egui_file::FileDialog::save_file(None)
                    .title("Export Photo")
                    .default_filename("exported.jpg");
                dialog.open();
                
                state.export_target_id = Some(photo.id.clone());
                state.import_dialog = Some(dialog);
                state.import_dialog_mode = crate::state::ImportDialogMode::Export;
            } else {
                 // Warning if no photo
            }
        }

        ui.add_space(Theme::SPACE_XL);

        // Delete button
        ui.label(
            egui::RichText::new("Danger Zone")
                .size(Theme::FONT_SM)
                .color(ui.visuals().weak_text_color())
        );
        ui.add_space(Theme::SPACE_SM);

        if ui.button(
            egui::RichText::new("Delete Photo")
                .color(egui::Color32::from_rgb(200, 60, 60))
        ).clicked() {
            if let Some(photo_id) = &state.develop_selected_photo_id {
                let photo_ctrl = photo_controller.clone();
                let lib_ctrl = library_controller.clone();
                let sender = photo_sender.clone();
                let ctx_clone = ctx.clone();
                let id = photo_id.clone();

                state.is_busy = true;
                state.busy_message = "Deleting photo...".to_string();

                tokio::spawn(async move {
                    // Delete the photo
                    if let Err(e) = photo_ctrl.delete_photo(&id).await {
                        eprintln!("Failed to delete photo: {}", e);
                    } else {
                        // Reload photos after delete
                        let result = lib_ctrl.get_all_photos().await;
                        let _ = sender.send(result).await;
                        ctx_clone.request_repaint();
                    }
                });

                // Clear develop selection and return to library
                state.develop_selected_photo_id = None;
                state.loaded_photo_id = None;
                state.detail_image = None;
                state.detail_metadata = None;
                state.current_view = crate::state::CurrentView::Library;
            }
        }

        ui.add_space(Theme::SPACE_XL);

        // Rating
        widgets::section_title(ui, "Rating");
        ui.add_space(Theme::SPACE_SM);

        if let Some(metadata) = &mut state.detail_metadata {
            if let Some(new_rating) = RatingWidget::show(ui, &mut metadata.rating, 20.0) {
                // Persist rating change to database
                if let Some(photo_id) = &state.develop_selected_photo_id {
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

// Default implementation removed because PreviewManager is required
// impl Default for DevelopView { ... }
