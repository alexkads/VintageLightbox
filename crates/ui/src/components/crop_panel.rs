// Crop Panel Component
// Displays crop controls in a dock panel (Lightroom-style)

use egui::{Ui, ComboBox};
use domain::value_objects::AspectRatio;
use crate::design_system::theme::Theme;
use crate::state::AppState;

/// Dock panel for crop controls (Lightroom-style)
pub struct CropPanel;

impl CropPanel {
    pub fn show(ui: &mut Ui, state: &mut AppState) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(Theme::SPACE_SM);
            
            // Tool Title
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Tool:").size(Theme::FONT_SM).color(ui.visuals().weak_text_color()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("Crop & Straighten").size(Theme::FONT_SM).strong());
                });
            });
            
            ui.add_space(Theme::SPACE_MD);
            ui.separator();
            ui.add_space(Theme::SPACE_SM);
            
            // Aspect Ratio
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Aspect:").size(Theme::FONT_SM));
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Lock button
                    let lock_icon = if state.selected_aspect_ratio == AspectRatio::Free { "🔓" } else { "🔒" };
                    if ui.small_button(lock_icon).on_hover_text("Lock aspect ratio").clicked() {
                        // Toggle between Free and current ratio
                        if state.selected_aspect_ratio == AspectRatio::Free {
                            state.selected_aspect_ratio = AspectRatio::Original;
                        } else {
                            state.selected_aspect_ratio = AspectRatio::Free;
                        }
                    }
                    
                    ComboBox::from_id_salt("crop_aspect")
                        .selected_text(state.selected_aspect_ratio.to_string())
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::Original, "Original");
                            ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::Free, "Free");
                            ui.separator();
                            ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::Square, "1:1");
                            ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::TwoThree, "2:3");
                            ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::ThreeTwo, "3:2");
                            ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::FourThree, "4:3");
                            ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::ThreeFour, "3:4");
                            ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::FourFive, "4:5");
                            ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::FiveFour, "5:4");
                            ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::SixteenNine, "16:9");
                            ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::NineSixteen, "9:16");
                        });
                });
            });
            
            ui.add_space(Theme::SPACE_MD);
            
            // Angle Slider
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Angle:").size(Theme::FONT_SM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(crop) = &state.crop_settings {
                        ui.label(format!("{:.2}°", crop.angle()));
                    } else {
                        ui.label("0.00°");
                    }
                });
            });
            
            // Angle slider
            if let Some(crop) = &mut state.crop_settings {
                let mut angle = crop.angle();
                if ui.add(egui::Slider::new(&mut angle, -45.0..=45.0).show_value(false)).changed() {
                    *crop = domain::value_objects::CropSettings::new(
                        crop.crop_x(), crop.crop_y(), crop.crop_width(), crop.crop_height(),
                        crop.rotation_90(), angle,
                        crop.flip_horizontal(), crop.flip_vertical()
                    );
                }
            }
            
            ui.add_space(Theme::SPACE_SM);
            
            // Auto button (placeholder - would need horizon detection)
            ui.horizontal(|ui| {
                ui.add_space(ui.available_width() - 50.0);
                if ui.button("Auto").on_hover_text("Auto-straighten (not implemented)").clicked() {
                    // TODO: Implement horizon detection
                }
            });
            
            ui.add_space(Theme::SPACE_MD);
            ui.separator();
            ui.add_space(Theme::SPACE_SM);
            
            // Flip buttons
            ui.horizontal(|ui| {
                if ui.button("⇄ Flip H").on_hover_text("Flip horizontal").clicked() {
                    if let Some(crop) = &mut state.crop_settings {
                        *crop = domain::value_objects::CropSettings::new(
                            crop.crop_x(), crop.crop_y(), crop.crop_width(), crop.crop_height(),
                            crop.rotation_90(), crop.angle(),
                            !crop.flip_horizontal(), crop.flip_vertical()
                        );
                    }
                }
                
                if ui.button("⇅ Flip V").on_hover_text("Flip vertical").clicked() {
                    if let Some(crop) = &mut state.crop_settings {
                        *crop = domain::value_objects::CropSettings::new(
                            crop.crop_x(), crop.crop_y(), crop.crop_width(), crop.crop_height(),
                            crop.rotation_90(), crop.angle(),
                            crop.flip_horizontal(), !crop.flip_vertical()
                        );
                    }
                }
            });
            
            ui.add_space(Theme::SPACE_SM);
            
            // Grid toggle
            ui.checkbox(&mut state.show_composition_grid, "Show Grid");
            
            ui.add_space(Theme::SPACE_MD);
            ui.separator();
            ui.add_space(Theme::SPACE_SM);
            
            // Action buttons
            ui.horizontal(|ui| {
                if ui.button("↺ Reset").on_hover_text("Reset crop (Shift+R)").clicked() {
                    state.crop_settings = Some(domain::value_objects::CropSettings::default());
                    state.selected_aspect_ratio = AspectRatio::Original;
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Apply button - check for Enter key as well
                    let enter_pressed = ui.ctx().input(|i| i.key_pressed(egui::Key::Enter));
                    
                    if ui.button("✓ Apply").on_hover_text("Apply crop (Enter)").clicked() || enter_pressed {
                        // Mark for saving - the actual save happens in parent context
                        state.pending_crop_apply = true;
                        state.crop_mode_active = false;
                    }
                    
                    if ui.button("Cancel").on_hover_text("Exit without applying (Esc)").clicked() {
                        state.crop_mode_active = false;
                    }
                });
            });
            
            ui.add_space(Theme::SPACE_SM);
        });
        
    }
}
