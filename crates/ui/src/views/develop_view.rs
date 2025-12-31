// Develop View
// Image editing view with adjustments panel

use egui::Ui;
use crate::state::AppState;
use crate::design_system::theme::Theme;
use crate::components::image_viewer::ImageViewer;
use crate::components::filmstrip::Filmstrip;
use std::sync::Arc;
use infrastructure::cache::preview_manager::PreviewManager;
use adapters::view_models::{CropSettings, AspectRatio};

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

    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &mut Ui,
        state: &mut AppState,
        editor_service: &mut adapters::services::EditorService,
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
                let selected_id = state.internal_state.develop_selected_id.clone();
                
                let mut pending_selection = None;
                let mut pending_flag = None;

                self.filmstrip.show_develop(
                    ui,
                    ctx,
                    &photos_clone,
                    &selected_id,
                    &mut state.internal_state.photo_filters,
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
                               
                               // Get current edits from EditorService (includes crop when present)
                               let mut current_edits = editor_service.current_edits();
                               if let Some(crop) = state.crop_settings.clone() {
                                   current_edits.crop_settings = Some(crop);
                               }

                               tokio::spawn(async move {
                                   let _ = controller.save_edits_from_vo(&id, current_edits).await;
                               });
                          }
                      }

                    // Select photo and trigger load in Develop view (independent from Library)
                    state.internal_state.develop_selected_id = Some(photo_id.clone());
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
                        state.crop_settings = Some(CropSettings::default());
                        state.selected_aspect_ratio = AspectRatio::Original;
                        state.show_composition_grid = false;
                        // Clear intelligent fill texture since angle is now 0
                        state.intelligent_fill_texture = None;
                    }
                    if apply {
                        println!("DEBUG: Apply button clicked or Enter pressed");
                        // Apply crop and exit crop mode
                        state.crop_mode_active = false;
                        state.pending_auto_save = false; // We are saving immediately

                        // Force immediate save for "Apply" action to ensure persistence
                        if let Some(id) = state.internal_state.develop_selected_id.clone() {
                            println!("DEBUG: Saving crop for photo_id: {}", id);
                            if let Some(crop) = &state.crop_settings {
                                println!("DEBUG: Crop settings: {:?}", crop);
                            } else {
                                println!("DEBUG: Crop settings is NONE!");
                            }
                            
                            let controller = editor_controller.clone();

                            // Sync crop settings to EditorService before getting current edits
                            if let Some(crop) = &state.crop_settings {
                                let _ = editor_service.update_field("Apply Crop", |edits| {
                                    edits.crop_settings = Some(crop.clone());
                                });
                            }

                            // Get current edits from EditorService (now includes crop!)
                            let current_edits = editor_service.current_edits();
                            let id_clone = id.clone();
                            
                            tokio::spawn(async move {
                                   let _ = controller.save_edits_from_vo(&id_clone, current_edits).await;
                                });
                            
                            // Refresh thumbnails/grid after saving crop
                            state.invalidation_queue.insert(id);
                        }
                    }
                    
                    // Handle rotations (Swap Aspect Ratio Orientation)
                    if rotate {
                        if let Some(crop) = &mut state.crop_settings {
                             let new_rotation = crop.rotation_90() + 1;
                             *crop = crop.with_rotation_90(new_rotation);
                         }
                    }
                    
                    // Handle flips
                    if flip_h {
                        if let Some(crop) = &mut state.crop_settings {
                            *crop = crop.with_flip_horizontal(!crop.flip_horizontal());
                        }
                    }
                    if flip_v {
                        if let Some(crop) = &mut state.crop_settings {
                            *crop = crop.with_flip_vertical(!crop.flip_vertical());
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
                    self.show_left_sidebar(ui, state, editor_service);
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
                        self.show_right_sidebar(ui, state, editor_controller, export_controller, photo_controller, library_controller, photo_sender, ctx, editor_service);
                    });
            });

        // Center - Image viewer
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ImageViewer::show(ui, state);
        });
    }

    fn show_left_sidebar(&mut self, ui: &mut Ui, state: &mut AppState, editor_service: &mut adapters::services::EditorService) {
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
             // Apply preset via EditorService
             let _ = editor_service.update_field(&preset.name, |edits| {
                 // Apply only the fields present in the preset
                 if let Some(v) = preset.adjustments.exposure { edits.exposure = v; }
                 if let Some(v) = preset.adjustments.contrast { edits.contrast = v; }
                 if let Some(v) = preset.adjustments.temperature { edits.temperature = v; }
                 if let Some(v) = preset.adjustments.tint { edits.tint = v; }
                 if let Some(v) = preset.adjustments.highlights { edits.highlights = v; }
                 if let Some(v) = preset.adjustments.shadows { edits.shadows = v; }
                 if let Some(v) = preset.adjustments.whites { edits.whites = v; }
                 if let Some(v) = preset.adjustments.blacks { edits.blacks = v; }
                 if let Some(v) = preset.adjustments.clarity { edits.clarity = v; }
                 if let Some(v) = preset.adjustments.vibrance { edits.vibrance = v; }
                 if let Some(v) = preset.adjustments.saturation { edits.saturation = v; }
                 if let Some(v) = preset.adjustments.tone_curve_shadows { edits.tone_curve_shadows = v; }
                 if let Some(v) = preset.adjustments.tone_curve_darks { edits.tone_curve_darks = v; }
                 if let Some(v) = preset.adjustments.tone_curve_lights { edits.tone_curve_lights = v; }
                 if let Some(v) = preset.adjustments.tone_curve_highlights { edits.tone_curve_highlights = v; }
             });
             
             // Sync back to state for UI
             // Sync back to state for UI


             
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

        // History Panel with Undo/Redo
        widgets::section_title(ui, "History");
        ui.add_space(Theme::SPACE_SM);

        // Undo/Redo buttons
        ui.horizontal(|ui| {
            let can_undo = editor_service.can_undo();
            let can_redo = editor_service.can_redo();
            
            ui.add_enabled_ui(can_undo, |ui| {
                if widgets::secondary_button(ui, "⟲ Undo").clicked() {
                    if let Some(_edits) = editor_service.undo() {
                        // Sync back to state
                        // Sync back to state

                        state.pending_auto_save = true;
                        ui.ctx().request_repaint();
                    }
                }
            });
            
            ui.add_space(Theme::SPACE_SM);
            
            ui.add_enabled_ui(can_redo, |ui| {
                if widgets::secondary_button(ui, "⟳ Redo").clicked() {
                    if let Some(_edits) = editor_service.redo() {
                        // Sync back to state
                        // Sync back to state

                        state.pending_auto_save = true;
                        ui.ctx().request_repaint();
                    }
                }
            });
        });

        ui.add_space(Theme::SPACE_SM);
        widgets::menu_item(ui, "Current State", true);
    }

    #[allow(unused_assignments)]
    #[allow(clippy::too_many_arguments)]
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
        editor_service: &mut adapters::services::EditorService,
    ) {
        use crate::design_system::widgets;
        use crate::components::{
            histogram_plot::HistogramPlot,
            tone_curve::ToneCurveEditor,
            slider_control::SliderControl,
            rating_widget::RatingWidget
        };
        
        // Sync EditorService → AppState (in case of undo/redo)
        // Sync EditorService → AppState (in case of undo/redo)


        // Interactive Histogram
        HistogramPlot::show(ui, state.histogram_data.as_ref());
        ui.add_space(Theme::SPACE_MD);

        // Get current edits from service (moved to top)
        let current_edits = editor_service.current_edits();

        // Tone Curve Visualization
        ToneCurveEditor::show(ui, &current_edits);
        ui.add_space(Theme::SPACE_MD);

        // Basic Adjustments
        widgets::section_title(ui, "Basic");
        ui.add_space(Theme::SPACE_SM);

        // Track if any slider changed for auto-save
        let mut any_slider_changed = false;

        // Get current edits from service
        // let current_edits = editor_service.current_edits(); // Moved to top
        
        // Exposure slider
        let mut exposure = current_edits.exposure;
        if SliderControl::show(ui, "Exposure", &mut exposure, -2.0..=2.0, 0.1) {
            let _ = editor_service.update_field("Exposure", |e| e.exposure = exposure);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Contrast slider
        let mut contrast = current_edits.contrast;
        if SliderControl::show(ui, "Contrast", &mut contrast, 0.5..=1.5, 0.05) {
            let _ = editor_service.update_field("Contrast", |e| e.contrast = contrast);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Temperature slider
        let mut temperature = current_edits.temperature;
        if SliderControl::show(ui, "Temperature", &mut temperature, -10.0..=10.0, 0.5) {
            let _ = editor_service.update_field("Temperature", |e| e.temperature = temperature);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Tint slider
        let mut tint = current_edits.tint;
        if SliderControl::show(ui, "Tint", &mut tint, -10.0..=10.0, 0.5) {
            let _ = editor_service.update_field("Tint", |e| e.tint = tint);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Highlights slider
        let mut highlights = current_edits.highlights;
        if SliderControl::show(ui, "Highlights", &mut highlights, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("Highlights", |e| e.highlights = highlights);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Shadows slider
        let mut shadows = current_edits.shadows;
        if SliderControl::show(ui, "Shadows", &mut shadows, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("Shadows", |e| e.shadows = shadows);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Whites slider
        let mut whites = current_edits.whites;
        if SliderControl::show(ui, "Whites", &mut whites, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("Whites", |e| e.whites = whites);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Blacks slider
        let mut blacks = current_edits.blacks;
        if SliderControl::show(ui, "Blacks", &mut blacks, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("Blacks", |e| e.blacks = blacks);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Clarity slider
        let mut clarity = current_edits.clarity;
        if SliderControl::show(ui, "Clarity", &mut clarity, -1.0..=1.0, 0.05) {
            let _ = editor_service.update_field("Clarity", |e| e.clarity = clarity);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Vibrance slider
        let mut vibrance = current_edits.vibrance;
        if SliderControl::show(ui, "Vibrance", &mut vibrance, -1.0..=1.0, 0.05) {
            let _ = editor_service.update_field("Vibrance", |e| e.vibrance = vibrance);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Saturation slider
        let mut saturation = current_edits.saturation;
        if SliderControl::show(ui, "Saturation", &mut saturation, -1.0..=1.0, 0.05) {
            let _ = editor_service.update_field("Saturation", |e| e.saturation = saturation);
            any_slider_changed = true;
        }

        // Saturation slider
        // REMOVED DUPLICATE

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
        let mut tone_curve_shadows = current_edits.tone_curve_shadows;
        if SliderControl::show(ui, "Shadows", &mut tone_curve_shadows, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("Tone Curve Shadows", |e| e.tone_curve_shadows = tone_curve_shadows);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Darks (dark midtones)
        let mut tone_curve_darks = current_edits.tone_curve_darks;
        if SliderControl::show(ui, "Darks", &mut tone_curve_darks, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("Tone Curve Darks", |e| e.tone_curve_darks = tone_curve_darks);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Lights (light midtones)
        let mut tone_curve_lights = current_edits.tone_curve_lights;
        if SliderControl::show(ui, "Lights", &mut tone_curve_lights, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("Tone Curve Lights", |e| e.tone_curve_lights = tone_curve_lights);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Highlights (brightest tones)
        let mut tone_curve_highlights = current_edits.tone_curve_highlights;
        if SliderControl::show(ui, "Highlights (Curve)", &mut tone_curve_highlights, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("Tone Curve Highlights", |e| e.tone_curve_highlights = tone_curve_highlights);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_LG);

        // HSL / Color Section
        widgets::section_title(ui, "HSL / Color");
        ui.add_space(Theme::SPACE_SM);

        // Red saturation
        let mut hsl_red_sat = current_edits.hsl_red_sat;
        if SliderControl::show(ui, "Red", &mut hsl_red_sat, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Red Sat", |e| e.hsl_red_sat = hsl_red_sat);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Orange saturation
        let mut hsl_orange_sat = current_edits.hsl_orange_sat;
        if SliderControl::show(ui, "Orange", &mut hsl_orange_sat, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Orange Sat", |e| e.hsl_orange_sat = hsl_orange_sat);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Yellow saturation
        let mut hsl_yellow_sat = current_edits.hsl_yellow_sat;
        if SliderControl::show(ui, "Yellow", &mut hsl_yellow_sat, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Yellow Sat", |e| e.hsl_yellow_sat = hsl_yellow_sat);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Green saturation
        let mut hsl_green_sat = current_edits.hsl_green_sat;
        if SliderControl::show(ui, "Green", &mut hsl_green_sat, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Green Sat", |e| e.hsl_green_sat = hsl_green_sat);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Aqua saturation
        let mut hsl_aqua_sat = current_edits.hsl_aqua_sat;
        if SliderControl::show(ui, "Aqua", &mut hsl_aqua_sat, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Aqua Sat", |e| e.hsl_aqua_sat = hsl_aqua_sat);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Blue saturation
        let mut hsl_blue_sat = current_edits.hsl_blue_sat;
        if SliderControl::show(ui, "Blue", &mut hsl_blue_sat, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Blue Sat", |e| e.hsl_blue_sat = hsl_blue_sat);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Purple saturation
        let mut hsl_purple_sat = current_edits.hsl_purple_sat;
        if SliderControl::show(ui, "Purple", &mut hsl_purple_sat, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Purple Sat", |e| e.hsl_purple_sat = hsl_purple_sat);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        // Magenta saturation
        let mut hsl_magenta_sat = current_edits.hsl_magenta_sat;
        if SliderControl::show(ui, "Magenta", &mut hsl_magenta_sat, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Magenta Sat", |e| e.hsl_magenta_sat = hsl_magenta_sat);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_XXL);

        // HSL Hue Section
        widgets::section_title(ui, "HSL / Hue");
        ui.add_space(Theme::SPACE_SM);

        // HSL Hue controls
        let mut hsl_red_hue = current_edits.hsl_red_hue;
        if SliderControl::show(ui, "Red Hue", &mut hsl_red_hue, -180.0..=180.0, 5.0) {
            let _ = editor_service.update_field("HSL Red Hue", |e| e.hsl_red_hue = hsl_red_hue);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_orange_hue = current_edits.hsl_orange_hue;
        if SliderControl::show(ui, "Orange Hue", &mut hsl_orange_hue, -180.0..=180.0, 5.0) {
            let _ = editor_service.update_field("HSL Orange Hue", |e| e.hsl_orange_hue = hsl_orange_hue);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_yellow_hue = current_edits.hsl_yellow_hue;
        if SliderControl::show(ui, "Yellow Hue", &mut hsl_yellow_hue, -180.0..=180.0, 5.0) {
            let _ = editor_service.update_field("HSL Yellow Hue", |e| e.hsl_yellow_hue = hsl_yellow_hue);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_green_hue = current_edits.hsl_green_hue;
        if SliderControl::show(ui, "Green Hue", &mut hsl_green_hue, -180.0..=180.0, 5.0) {
            let _ = editor_service.update_field("HSL Green Hue", |e| e.hsl_green_hue = hsl_green_hue);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_aqua_hue = current_edits.hsl_aqua_hue;
        if SliderControl::show(ui, "Aqua Hue", &mut hsl_aqua_hue, -180.0..=180.0, 5.0) {
            let _ = editor_service.update_field("HSL Aqua Hue", |e| e.hsl_aqua_hue = hsl_aqua_hue);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_blue_hue = current_edits.hsl_blue_hue;
        if SliderControl::show(ui, "Blue Hue", &mut hsl_blue_hue, -180.0..=180.0, 5.0) {
            let _ = editor_service.update_field("HSL Blue Hue", |e| e.hsl_blue_hue = hsl_blue_hue);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_purple_hue = current_edits.hsl_purple_hue;
        if SliderControl::show(ui, "Purple Hue", &mut hsl_purple_hue, -180.0..=180.0, 5.0) {
            let _ = editor_service.update_field("HSL Purple Hue", |e| e.hsl_purple_hue = hsl_purple_hue);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_magenta_hue = current_edits.hsl_magenta_hue;
        if SliderControl::show(ui, "Magenta Hue", &mut hsl_magenta_hue, -180.0..=180.0, 5.0) {
            let _ = editor_service.update_field("HSL Magenta Hue", |e| e.hsl_magenta_hue = hsl_magenta_hue);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_XXL);

        // HSL Luminance Section
        widgets::section_title(ui, "HSL / Luminance");
        ui.add_space(Theme::SPACE_SM);

        // HSL Luminance controls
        let mut hsl_red_lum = current_edits.hsl_red_lum;
        if SliderControl::show(ui, "Red Lum", &mut hsl_red_lum, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Red Lum", |e| e.hsl_red_lum = hsl_red_lum);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_orange_lum = current_edits.hsl_orange_lum;
        if SliderControl::show(ui, "Orange Lum", &mut hsl_orange_lum, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Orange Lum", |e| e.hsl_orange_lum = hsl_orange_lum);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_yellow_lum = current_edits.hsl_yellow_lum;
        if SliderControl::show(ui, "Yellow Lum", &mut hsl_yellow_lum, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Yellow Lum", |e| e.hsl_yellow_lum = hsl_yellow_lum);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_green_lum = current_edits.hsl_green_lum;
        if SliderControl::show(ui, "Green Lum", &mut hsl_green_lum, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Green Lum", |e| e.hsl_green_lum = hsl_green_lum);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_aqua_lum = current_edits.hsl_aqua_lum;
        if SliderControl::show(ui, "Aqua Lum", &mut hsl_aqua_lum, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Aqua Lum", |e| e.hsl_aqua_lum = hsl_aqua_lum);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_blue_lum = current_edits.hsl_blue_lum;
        if SliderControl::show(ui, "Blue Lum", &mut hsl_blue_lum, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Blue Lum", |e| e.hsl_blue_lum = hsl_blue_lum);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_purple_lum = current_edits.hsl_purple_lum;
        if SliderControl::show(ui, "Purple Lum", &mut hsl_purple_lum, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Purple Lum", |e| e.hsl_purple_lum = hsl_purple_lum);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut hsl_magenta_lum = current_edits.hsl_magenta_lum;
        if SliderControl::show(ui, "Magenta Lum", &mut hsl_magenta_lum, -100.0..=100.0, 5.0) {
            let _ = editor_service.update_field("HSL Magenta Lum", |e| e.hsl_magenta_lum = hsl_magenta_lum);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_XXL);

        // Lens Corrections Section
        widgets::section_title(ui, "Lens Corrections");
        ui.add_space(Theme::SPACE_SM);

        let mut lens_distortion = current_edits.lens_distortion;
        if SliderControl::show(ui, "Distortion", &mut lens_distortion, -100.0..=100.0, 1.0) {
            let _ = editor_service.update_field("Lens Distortion", |e| e.lens_distortion = lens_distortion);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut lens_vignette_amount = current_edits.lens_vignette_amount;
        if SliderControl::show(ui, "Vignette Amount", &mut lens_vignette_amount, -100.0..=100.0, 1.0) {
            let _ = editor_service.update_field("Vignette Amount", |e| e.lens_vignette_amount = lens_vignette_amount);
            any_slider_changed = true;
        }
        ui.add_space(Theme::SPACE_SM);
        
        let mut lens_vignette_midpoint = current_edits.lens_vignette_midpoint;
        if SliderControl::show(ui, "Vignette Midpoint", &mut lens_vignette_midpoint, 0.0..=100.0, 1.0) {
            let _ = editor_service.update_field("Vignette Midpoint", |e| e.lens_vignette_midpoint = lens_vignette_midpoint);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_XXL);

        // Detail (Noise Reduction)
        widgets::section_title(ui, "Detail");
        ui.add_space(Theme::SPACE_SM);

        let mut nr_luminance = current_edits.nr_luminance;
        if SliderControl::show(ui, "Luminance NR", &mut nr_luminance, 0.0..=100.0, 1.0) {
            let _ = editor_service.update_field("Luminance NR", |e| e.nr_luminance = nr_luminance);
            any_slider_changed = true;
        }

        let mut nr_color = current_edits.nr_color;
        if SliderControl::show(ui, "Color NR", &mut nr_color, 0.0..=100.0, 1.0) {
            let _ = editor_service.update_field("Color NR", |e| e.nr_color = nr_color);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_SM);

        let mut sharpen_amount = current_edits.sharpen_amount;
        if SliderControl::show(ui, "Sharpen Amount", &mut sharpen_amount, 0.0..=100.0, 1.0) {
            let _ = editor_service.update_field("Sharpen Amount", |e| e.sharpen_amount = sharpen_amount);
            any_slider_changed = true;
        }

        let mut sharpen_radius = current_edits.sharpen_radius;
        if SliderControl::show(ui, "Sharpen Radius", &mut sharpen_radius, 0.5..=3.0, 0.1) {
            let _ = editor_service.update_field("Sharpen Radius", |e| e.sharpen_radius = sharpen_radius);
            any_slider_changed = true;
        }

        ui.add_space(Theme::SPACE_XXL);

        // Reset button
        if widgets::secondary_button(ui, "Reset").clicked() {
            // Reset adjustments to default values
            let _ = editor_service.update_field("Reset All", |e| *e = Default::default());

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
            if let Some(photo_id) = &state.internal_state.develop_selected_id {
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
                state.internal_state.develop_selected_id = None;
                state.loaded_photo_id = None;
                state.detail_image = None;
                state.detail_metadata = None;
                state.internal_state.current_view = crate::state::CurrentView::Library;
            }
        }

        ui.add_space(Theme::SPACE_XL);

        // Rating
        widgets::section_title(ui, "Rating");
        ui.add_space(Theme::SPACE_SM);

        if let Some(metadata) = &mut state.detail_metadata {
            if let Some(new_rating) = RatingWidget::show(ui, &mut metadata.rating, 20.0) {
                // Persist rating change to database
                if let Some(photo_id) = &state.internal_state.develop_selected_id {
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
        
        // Sync AppState → EditorService (if any changes)
        // if any_slider_changed {

        // }
    }
}

// Default implementation removed because PreviewManager is required
// impl Default for DevelopView { ... }
