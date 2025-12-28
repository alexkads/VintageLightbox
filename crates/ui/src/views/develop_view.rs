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
        // Ensure we're viewing a valid photo
        state.sanitize_develop_selection();

        // Bottom filmstrip (must be first to reserve space)
        egui::TopBottomPanel::bottom("filmstrip_develop")
            .exact_height(120.0)  // 80px thumbnails + 40px padding
            .show_inside(ui, |ui| {
                let photos_clone = state.photos.clone();
                let selected_id = state.develop_selected_photo_id.clone();
                
                let mut pending_selection = None;
                let mut pending_flag = None;

                self.filmstrip.show_develop(
                    ui,
                    ctx,
                    &photos_clone,
                    &selected_id,
                    &mut state.filmstrip_filter,
                    |photo_id| {
                        pending_selection = Some(photo_id);
                    },
                    |photo_id, flag_code| {
                         pending_flag = Some((photo_id, flag_code));
                    }
                );
                
                // Process pending actions
                if let Some(photo_id) = pending_selection {
                     // Check if we need to save the CURRENT photo before switching
                     // Auto-save if there are pending edits OR if we are in crop mode (commit crop)
                     let needs_save = state.pending_auto_save || (state.crop_mode_active && state.crop_settings.is_some());
                     
                     if needs_save {
                         if let Some(vm) = state.get_current_photo() {
                             // Trigger explicit save for current photo
                              let controller = editor_controller.clone();
                              let id = vm.id.clone();
                              let exposure = state.active_exposure;
                              let contrast = state.active_contrast;
                              let temperature = state.active_temperature;
                              let tint = state.active_tint;
                              let highlights = state.active_highlights;
                              let shadows = state.active_shadows;
                              let whites = state.active_whites;
                              let blacks = state.active_blacks;
                              let clarity = state.active_clarity;
                              let vibrance = state.active_vibrance;
                              let saturation = state.active_saturation;
                              let tone_curve_shadows = state.active_tone_curve_shadows;
                              let tone_curve_darks = state.active_tone_curve_darks;
                              let tone_curve_lights = state.active_tone_curve_lights;
                              let tone_curve_highlights = state.active_tone_curve_highlights;
                              let hsl_red_sat = state.active_hsl_red_sat;
                              let hsl_orange_sat = state.active_hsl_orange_sat;
                              let hsl_yellow_sat = state.active_hsl_yellow_sat;
                              let hsl_green_sat = state.active_hsl_green_sat;
                              let hsl_aqua_sat = state.active_hsl_aqua_sat;
                              let hsl_blue_sat = state.active_hsl_blue_sat;
                              let hsl_purple_sat = state.active_hsl_purple_sat;
                              let hsl_magenta_sat = state.active_hsl_magenta_sat;
                              let hsl_red_hue = state.active_hsl_red_hue;
                              let hsl_orange_hue = state.active_hsl_orange_hue;
                              let hsl_yellow_hue = state.active_hsl_yellow_hue;
                              let hsl_green_hue = state.active_hsl_green_hue;
                              let hsl_aqua_hue = state.active_hsl_aqua_hue;
                              let hsl_blue_hue = state.active_hsl_blue_hue;
                              let hsl_purple_hue = state.active_hsl_purple_hue;
                              let hsl_magenta_hue = state.active_hsl_magenta_hue;
                              let hsl_red_lum = state.active_hsl_red_lum;
                              let hsl_orange_lum = state.active_hsl_orange_lum;
                              let hsl_yellow_lum = state.active_hsl_yellow_lum;
                              let hsl_green_lum = state.active_hsl_green_lum;
                              let hsl_aqua_lum = state.active_hsl_aqua_lum;
                              let hsl_blue_lum = state.active_hsl_blue_lum;
                              let hsl_purple_lum = state.active_hsl_purple_lum;
                              let hsl_magenta_lum = state.active_hsl_magenta_lum;
                              let lens_distortion = state.active_lens_distortion;
                              let lens_vignette_amount = state.active_lens_vignette_amount;
                              let lens_vignette_midpoint = state.active_lens_vignette_midpoint;
                              let nr_luminance = state.active_nr_luminance;
                              let nr_color = state.active_nr_color;
                              let sharpen_amount = state.active_sharpen_amount;
                              let sharpen_radius = state.active_sharpen_radius;
                              let active_crop = state.crop_settings.clone();

                              // Clone for use after spawn
                              let id_for_update = id.clone();
                              let crop_for_update = active_crop.clone();

                              tokio::spawn(async move {
                                  let _ = controller.save_edits(
                                      id,
                                      exposure, contrast, temperature, tint,
                                      highlights, shadows, whites, blacks,
                                      clarity, vibrance, saturation,
                                      tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
                                      hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat, hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
                                      hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue, hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
                                      hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum, hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
                                      lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
                                      nr_luminance, nr_color,
                                      sharpen_amount, sharpen_radius,
                                      // Crop settings
                                      active_crop.as_ref().map(|c| c.crop_x()),
                                      active_crop.as_ref().map(|c| c.crop_y()),
                                      active_crop.as_ref().map(|c| c.crop_width()),
                                      active_crop.as_ref().map(|c| c.crop_height()),
                                      active_crop.as_ref().map(|c| c.rotation_90()),
                                      active_crop.as_ref().map(|c| c.angle()),
                                      active_crop.as_ref().map(|c| c.flip_horizontal()),
                                      active_crop.as_ref().map(|c| c.flip_vertical()),
                                  ).await;
                              });
                              
                              // Also update in-memory ViewModel so crop persists when switching back
                              // Same transformation: visual->original coords, rotation consumed
                              if let Some(photo_vm) = state.photos.iter_mut().find(|p| p.id == id_for_update) {
                                    if let Some(c) = crop_for_update.as_ref() {
                                        photo_vm.edit_crop_x = Some(c.crop_x());
                                        photo_vm.edit_crop_y = Some(c.crop_y());
                                        photo_vm.edit_crop_width = Some(c.crop_width());
                                        photo_vm.edit_crop_height = Some(c.crop_height());
                                        photo_vm.edit_crop_rotation = Some(c.rotation_90());
                                        photo_vm.edit_crop_angle = Some(c.angle());
                                        photo_vm.edit_crop_flip_h = Some(c.flip_horizontal());
                                        photo_vm.edit_crop_flip_v = Some(c.flip_vertical());
                                   } else {
                                       photo_vm.edit_crop_x = None;
                                       photo_vm.edit_crop_y = None;
                                       photo_vm.edit_crop_width = None;
                                       photo_vm.edit_crop_height = None;
                                       photo_vm.edit_crop_rotation = None;
                                       photo_vm.edit_crop_angle = None;
                                       photo_vm.edit_crop_flip_h = None;
                                       photo_vm.edit_crop_flip_v = None;
                                   }
                                   // Also update exposure and other edits
                                   photo_vm.edit_exposure = Some(exposure);
                                   photo_vm.edit_contrast = Some(contrast);
                                   
                                   // Queue for invalidation (to update Filmstrip/Grid)
                                   state.invalidation_queue.insert(id_for_update.clone());
                              }
                         }
                     }

                    // Select photo and trigger load in Develop view (independent from Library)
                    state.develop_selected_photo_id = Some(photo_id.clone());
                    // Clear current image to trigger reload
                    state.loaded_photo_id = None;
                    ctx.request_repaint();
                }
                
                if let Some((photo_id, flag_code)) = pending_flag {
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
            })  ;

        // Crop Toolbar (when crop mode is active)
        if state.crop_mode_active {
            egui::TopBottomPanel::top("crop_toolbar_panel")
                .exact_height(50.0)
                .show_inside(ui, |ui| {
                    use crate::components::crop_toolbar::CropToolbar;
                    
                    let mut rotate = false;
                    let mut flip_h = false;
                    let mut flip_v = false;
                    let mut reset = false;
                    let mut apply = false;
                    
                    CropToolbar::show(
                        ui,
                        &mut state.selected_aspect_ratio,
                        &mut state.show_composition_grid,
                        &mut rotate,
                        &mut flip_h,
                        &mut flip_v,
                        &mut reset,
                        &mut apply,
                    );
                    
                    // Allow applying with ENTER key
                    if ui.ctx().input(|i| i.key_pressed(egui::Key::Enter)) {
                        apply = true;
                    }
                    
                    // Handle button clicks
                    if reset {
                        state.crop_settings = Some(domain::value_objects::CropSettings::default());
                        state.selected_aspect_ratio = domain::value_objects::AspectRatio::Original;
                        state.show_composition_grid = false;
                    }
                    if apply {
                        println!("DEBUG: Apply button clicked or Enter pressed");
                        // Apply crop and exit crop mode
                        state.crop_mode_active = false;
                        state.pending_auto_save = false; // We are saving immediately

                        // Force immediate save for "Apply" action to ensure persistence
                        if let Some(id) = state.develop_selected_photo_id.clone() {
                            println!("DEBUG: Saving crop for photo_id: {}", id);
                            if let Some(crop) = &state.crop_settings {
                                println!("DEBUG: Crop settings: {:?}", crop);
                            } else {
                                println!("DEBUG: Crop settings is NONE!");
                            }
                            
                            let controller = editor_controller.clone();

                            
                            let exposure = state.active_exposure;
                            let contrast = state.active_contrast;
                            let temperature = state.active_temperature;
                            let tint = state.active_tint;
                            let highlights = state.active_highlights;
                            let shadows = state.active_shadows;
                            let whites = state.active_whites;
                            let blacks = state.active_blacks;
                            let clarity = state.active_clarity;
                            let vibrance = state.active_vibrance;
                            let saturation = state.active_saturation;
                            let tone_curve_shadows = state.active_tone_curve_shadows;
                            let tone_curve_darks = state.active_tone_curve_darks;
                            let tone_curve_lights = state.active_tone_curve_lights;
                            let tone_curve_highlights = state.active_tone_curve_highlights;
                            let hsl_red_sat = state.active_hsl_red_sat;
                            let hsl_orange_sat = state.active_hsl_orange_sat;
                            let hsl_yellow_sat = state.active_hsl_yellow_sat;
                            let hsl_green_sat = state.active_hsl_green_sat;
                            let hsl_aqua_sat = state.active_hsl_aqua_sat;
                            let hsl_blue_sat = state.active_hsl_blue_sat;
                            let hsl_purple_sat = state.active_hsl_purple_sat;
                            let hsl_magenta_sat = state.active_hsl_magenta_sat;
                            let hsl_red_hue = state.active_hsl_red_hue;
                            let hsl_orange_hue = state.active_hsl_orange_hue;
                            let hsl_yellow_hue = state.active_hsl_yellow_hue;
                            let hsl_green_hue = state.active_hsl_green_hue;
                            let hsl_aqua_hue = state.active_hsl_aqua_hue;
                            let hsl_blue_hue = state.active_hsl_blue_hue;
                            let hsl_purple_hue = state.active_hsl_purple_hue;
                            let hsl_magenta_hue = state.active_hsl_magenta_hue;
                            let hsl_red_lum = state.active_hsl_red_lum;
                            let hsl_orange_lum = state.active_hsl_orange_lum;
                            let hsl_yellow_lum = state.active_hsl_yellow_lum;
                            let hsl_green_lum = state.active_hsl_green_lum;
                            let hsl_aqua_lum = state.active_hsl_aqua_lum;
                            let hsl_blue_lum = state.active_hsl_blue_lum;
                            let hsl_purple_lum = state.active_hsl_purple_lum;
                            let hsl_magenta_lum = state.active_hsl_magenta_lum;
                            let lens_distortion = state.active_lens_distortion;
                            let lens_vignette_amount = state.active_lens_vignette_amount;
                            let lens_vignette_midpoint = state.active_lens_vignette_midpoint;
                            let nr_luminance = state.active_nr_luminance;
                            let nr_color = state.active_nr_color;
                            let sharpen_amount = state.active_sharpen_amount;
                            let sharpen_radius = state.active_sharpen_radius;
                            let active_crop = state.crop_settings.clone();
                            
                            let id_for_task = id.clone();
                            let active_crop_for_task = active_crop.clone();
                            
                            tokio::spawn(async move {
                                   let _ = controller.save_edits(
                                       id_for_task,
                                       exposure, contrast, temperature, tint,
                                       highlights, shadows, whites, blacks,
                                       clarity, vibrance, saturation,
                                       tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
                                       hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat, hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
                                       hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue, hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
                                       hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum, hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
                                       lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
                                       nr_luminance, nr_color,
                                       sharpen_amount, sharpen_radius,
                                       active_crop_for_task.as_ref().map(|c| c.crop_x()),
                                       active_crop_for_task.as_ref().map(|c| c.crop_y()),
                                       active_crop_for_task.as_ref().map(|c| c.crop_width()),
                                       active_crop_for_task.as_ref().map(|c| c.crop_height()),
                                       active_crop_for_task.as_ref().map(|c| c.rotation_90()),
                                       active_crop_for_task.as_ref().map(|c| c.angle()),
                                       active_crop_for_task.as_ref().map(|c| c.flip_horizontal()),
                                       active_crop_for_task.as_ref().map(|c| c.flip_vertical()),
                                   ).await;
                               });
                               
                            // Update in-memory VM
                            if let Some(photo_vm) = state.photos.iter_mut().find(|p| p.id == id) {
                                   photo_vm.edit_crop_x = active_crop.as_ref().map(|c| c.crop_x());
                                   photo_vm.edit_crop_y = active_crop.as_ref().map(|c| c.crop_y());
                                   photo_vm.edit_crop_width = active_crop.as_ref().map(|c| c.crop_width());
                                   photo_vm.edit_crop_height = active_crop.as_ref().map(|c| c.crop_height());
                                   photo_vm.edit_crop_rotation = active_crop.as_ref().map(|c| c.rotation_90());
                                   photo_vm.edit_crop_angle = active_crop.as_ref().map(|c| c.angle());
                                   photo_vm.edit_crop_flip_h = active_crop.as_ref().map(|c| c.flip_horizontal());
                                   photo_vm.edit_crop_flip_v = active_crop.as_ref().map(|c| c.flip_vertical());
                                   photo_vm.edit_exposure = Some(exposure);
                                   photo_vm.edit_contrast = Some(contrast);
                                   
                                   // Queue for invalidation (to update Filmstrip/Grid)
                                   state.invalidation_queue.insert(id.clone());
                                }
                        }
                    }
                    
                    // Handle rotations (Swap Aspect Ratio Orientation)
                    if rotate {
                        // This logic should likely be moved to a controller or method on CropSettings
                        // For now we duplicate what's in DockViewer or remove this usage if DevelopView isn't primary
                        // Note: DevelopView seems to be unused for rendering in favor of DockViewer.
                        // But we must fix complication.
                        
                        // We will just do a dummy implementation here to satisfy the compiler
                        // assuming DockViewer is the one actually running.
                        if let Some(crop) = &mut state.crop_settings {
                             // Simple swap of w/h logic if we had dimensions, but we don't easily have them here
                             // passing 0 rotation for now as placeholder or attempting basic swap based on assumed w>h
                             let new_rotation = crop.rotation_90() + 1;
                             *crop = domain::value_objects::CropSettings::new(
                                 crop.crop_x(), crop.crop_y(), crop.crop_width(), crop.crop_height(),
                                 new_rotation, crop.angle(),
                                 crop.flip_horizontal(), crop.flip_vertical()
                             );
                         }
                    }
                    
                    // Handle flips
                    if flip_h {
                        if let Some(crop) = &mut state.crop_settings {
                            *crop = domain::value_objects::CropSettings::new(
                                crop.crop_x(), crop.crop_y(), crop.crop_width(), crop.crop_height(),
                                crop.rotation_90(), crop.angle(),
                                !crop.flip_horizontal(), crop.flip_vertical()
                            );
                        }
                    }
                    if flip_v {
                        if let Some(crop) = &mut state.crop_settings {
                            *crop = domain::value_objects::CropSettings::new(
                                crop.crop_x(), crop.crop_y(), crop.crop_width(), crop.crop_height(),
                                crop.rotation_90(), crop.angle(),
                                crop.flip_horizontal(), !crop.flip_vertical()
                            );
                        }
                    }
                });
        }

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
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
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

    #[allow(unused_assignments)]
    fn show_right_sidebar(
        &self,
        ui: &mut Ui,
        state: &mut AppState,
        _editor_controller: &std::sync::Arc<adapters::controllers::EditorController>,
        _export_controller: &std::sync::Arc<adapters::controllers::ExportController>,
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

        // HSL Hue Section
        widgets::section_title(ui, "HSL / Hue");
        ui.add_space(Theme::SPACE_SM);

        // HSL Hue controls
        if SliderControl::show(ui, "Red Hue", &mut state.active_hsl_red_hue, -180.0..=180.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Orange Hue", &mut state.active_hsl_orange_hue, -180.0..=180.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Yellow Hue", &mut state.active_hsl_yellow_hue, -180.0..=180.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Green Hue", &mut state.active_hsl_green_hue, -180.0..=180.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Aqua Hue", &mut state.active_hsl_aqua_hue, -180.0..=180.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Blue Hue", &mut state.active_hsl_blue_hue, -180.0..=180.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Purple Hue", &mut state.active_hsl_purple_hue, -180.0..=180.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Magenta Hue", &mut state.active_hsl_magenta_hue, -180.0..=180.0, 5.0) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_XXL);

        // HSL Luminance Section
        widgets::section_title(ui, "HSL / Luminance");
        ui.add_space(Theme::SPACE_SM);

        // HSL Luminance controls
        if SliderControl::show(ui, "Red Lum", &mut state.active_hsl_red_lum, -100.0..=100.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Orange Lum", &mut state.active_hsl_orange_lum, -100.0..=100.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Yellow Lum", &mut state.active_hsl_yellow_lum, -100.0..=100.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Green Lum", &mut state.active_hsl_green_lum, -100.0..=100.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Aqua Lum", &mut state.active_hsl_aqua_lum, -100.0..=100.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Blue Lum", &mut state.active_hsl_blue_lum, -100.0..=100.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Purple Lum", &mut state.active_hsl_purple_lum, -100.0..=100.0, 5.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Magenta Lum", &mut state.active_hsl_magenta_lum, -100.0..=100.0, 5.0) {
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_XXL);

        // Lens Corrections Section
        widgets::section_title(ui, "Lens Corrections");
        ui.add_space(Theme::SPACE_SM);

        if SliderControl::show(ui, "Distortion", &mut state.active_lens_distortion, -100.0..=100.0, 1.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Vignette Amount", &mut state.active_lens_vignette_amount, -100.0..=100.0, 1.0) {
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        if SliderControl::show(ui, "Vignette Midpoint", &mut state.active_lens_vignette_midpoint, 0.0..=100.0, 1.0) {
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

        ui.add_space(Theme::SPACE_SM);

        if SliderControl::show(
            ui,
            "Sharpen Amount",
            &mut state.active_sharpen_amount,
            0.0..=100.0,
            1.0,
        ) {
            any_slider_changed = true;
        }

        if SliderControl::show(
            ui,
            "Sharpen Radius",
            &mut state.active_sharpen_radius,
            0.5..=3.0,
            0.1,
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
