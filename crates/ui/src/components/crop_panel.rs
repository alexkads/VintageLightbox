// Crop Panel Component
// Displays crop controls in a dock panel (Lightroom-style)

use egui::{Ui, ComboBox};
// use image::GenericImageView; // Inherent methods used
use adapters::view_models::AspectRatio;
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
            // Capture change to trigger crop update
            let mut aspect_ratio_changed = false;

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Aspect:").size(Theme::FONT_SM));
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Lock button
                    let lock_icon = if state.selected_aspect_ratio == AspectRatio::Free { "🔓" } else { "🔒" };
                    if ui.small_button(lock_icon).on_hover_text("Lock aspect ratio").clicked() {
                        // Toggle between Free and current ratio
                        if state.selected_aspect_ratio == AspectRatio::Free {
                            state.selected_aspect_ratio = AspectRatio::Original;
                            aspect_ratio_changed = true;
                        } else {
                            state.selected_aspect_ratio = AspectRatio::Free;
                        }
                    }
                    
                    ComboBox::from_id_salt("crop_aspect")
                        .selected_text(state.selected_aspect_ratio.to_string())
                        .width(100.0)
                        .show_ui(ui, |ui| {
                             if ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::Original, "Original").changed() { aspect_ratio_changed = true; }
                             if ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::Free, "Free").changed() { aspect_ratio_changed = true; }
                             ui.separator();
                             if ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::Square, "1:1").changed() { aspect_ratio_changed = true; }
                             if ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::TwoThree, "2:3").changed() { aspect_ratio_changed = true; }
                             if ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::ThreeTwo, "3:2").changed() { aspect_ratio_changed = true; }
                             if ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::FourThree, "4:3").changed() { aspect_ratio_changed = true; }
                             if ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::ThreeFour, "3:4").changed() { aspect_ratio_changed = true; }
                             if ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::FourFive, "4:5").changed() { aspect_ratio_changed = true; }
                             if ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::FiveFour, "5:4").changed() { aspect_ratio_changed = true; }
                             if ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::SixteenNine, "16:9").changed() { aspect_ratio_changed = true; }
                             if ui.selectable_value(&mut state.selected_aspect_ratio, AspectRatio::NineSixteen, "9:16").changed() { aspect_ratio_changed = true; }
                        });
                });
            });
            
            if aspect_ratio_changed {
                Self::enforce_aspect_ratio(state);
            }
            
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
                    *crop = crop.with_angle(angle);
                    
                    // If ShrinkToFit is active, recalculate the crop
                    if crop.fill_mode() == domain::value_objects::RotationFillMode::ShrinkToFit {
                        if let Some(img) = state.original_preview.as_ref() {
                            let (img_w, img_h) = (img.width() as f32, img.height() as f32);
                            *crop = crop.calculate_shrink_to_fit(img_w, img_h);
                            
                            // Re-apply aspect ratio constraint on the new maximized crop
                            if state.selected_aspect_ratio != AspectRatio::Free {
                                Self::enforce_aspect_ratio(state);
                            }
                        }
                    }
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
            
            // Fill Mode selector (for rotation empty areas)
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Fill:").size(Theme::FONT_SM));
                ui.add_space(ui.available_width() - 110.0);
                
                let mut new_fill_mode: Option<domain::value_objects::RotationFillMode> = None;
                
                if let Some(crop) = &state.crop_settings {
                    let current_mode = crop.fill_mode();
                    
                    egui::ComboBox::from_id_salt("crop_fill_mode")
                        .selected_text(current_mode.to_string())
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            for mode in domain::value_objects::RotationFillMode::all() {
                                if ui.selectable_label(*mode == current_mode, mode.to_string()).clicked() {
                                    new_fill_mode = Some(*mode);
                                }
                            }
                        });
                }
                
                // Apply selection after the borrow ends
                if let Some(mode) = new_fill_mode {
                    eprintln!("CropPanel: User selected fill_mode={:?}", mode);
                    if let Some(crop_settings) = &mut state.crop_settings {
                        *crop_settings = crop_settings.with_fill_mode(mode);
                        eprintln!("CropPanel: Updated crop_settings.fill_mode={:?}", crop_settings.fill_mode());
                        
                        // If ShrinkToFit is selected, apply the calculation immediately
                        if mode == domain::value_objects::RotationFillMode::ShrinkToFit {
                            if let Some(img) = state.original_preview.as_ref() {
                                let (img_w, img_h) = (img.width() as f32, img.height() as f32);
                                *crop_settings = crop_settings.calculate_shrink_to_fit(img_w, img_h);

                                // Re-apply aspect ratio constraint on the new maximized crop
                                if state.selected_aspect_ratio != AspectRatio::Free {
                                    Self::enforce_aspect_ratio(state);
                                }
                            }
                        }
                    }
                }
            });
            
            ui.add_space(Theme::SPACE_SM);
            ui.separator();
            ui.add_space(Theme::SPACE_SM);
            
            // Flip buttons
            ui.horizontal(|ui| {
                if ui.button("⇄ Flip H").on_hover_text("Flip horizontal").clicked() {
                    if let Some(crop) = &mut state.crop_settings {
                        *crop = crop.with_flip_horizontal(!crop.flip_horizontal());
                    }
                }
                
                if ui.button("⇅ Flip V").on_hover_text("Flip vertical").clicked() {
                    if let Some(crop) = &mut state.crop_settings {
                        *crop = crop.with_flip_vertical(!crop.flip_vertical());
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
                let reset_shortcut = ui.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::R));
                if ui.button("↺ Reset").on_hover_text("Reset crop (Shift+R)").clicked() || reset_shortcut {
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

    /// Updates the current crop selection to match the selected aspect ratio
    pub fn enforce_aspect_ratio(state: &mut AppState) {
        let current_crop = state.crop_settings.clone().unwrap_or_default();

        // Calculate image ratio
        let (img_w, img_h) = if let Some(img) = state.original_preview.as_ref() {
            (img.width() as f32, img.height() as f32)
        } else {
            (1.0, 1.0)
        };
        let img_ratio = img_w / img_h;

        let target_ratio = match state.selected_aspect_ratio {
            AspectRatio::Original => img_ratio,
            AspectRatio::Free => -1.0,
            _ => state.selected_aspect_ratio.value(),
        };
        
        // If a constraint is active, enforce it
        if target_ratio > 0.0 {
            let current_w_n = current_crop.crop_width();
            let current_h_n = current_crop.crop_height();
            
            // Normalized Target Ratio = TargetRatio * (ImgH / ImgW)
            // Example: Image 2:1 (w=2, h=1). Normalized crop 1:1 (w=1, h=1).
            // Actual pixel ratio = (1*2)/(1*1) = 2.0.
            // Target Ratio = 1.0 (Square).
            // Normalized Target = 1.0 * (1/2) = 0.5.
            
            let normalized_target_ratio = target_ratio * (img_h / img_w);
            let current_normalized_ratio = current_w_n / current_h_n;
            
            let (new_w_n, new_h_n) = if current_normalized_ratio > normalized_target_ratio {
                // Current is wider than target -> Limit Width based on Height
                (current_h_n * normalized_target_ratio, current_h_n)
            } else {
                // Current is taller -> Limit Height based on Width
                (current_w_n, current_w_n / normalized_target_ratio)
            };
            
            // Recenter
            let cx = current_crop.crop_x() + current_w_n / 2.0;
            let cy = current_crop.crop_y() + current_h_n / 2.0;
     
            // Calculate new top-left, clamped to bounds
            // Note: clamping might shift center if we are at edge, which is desired behavior
            // Also clamp width/height to ensure 0-1 range
            let new_x = (cx - new_w_n / 2.0).clamp(0.0, 1.0 - new_w_n);
            let new_y = (cy - new_h_n / 2.0).clamp(0.0, 1.0 - new_h_n);
            
            state.crop_settings = Some(domain::value_objects::CropSettings::new(
                 new_x, new_y, new_w_n, new_h_n,
                 current_crop.rotation_90(), current_crop.angle(),
                 current_crop.flip_horizontal(), current_crop.flip_vertical()
            ).with_fill_mode(current_crop.fill_mode()));
            
            // Ensure crop mode is active if we selected a ratio
            state.crop_mode_active = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adapters::view_models::CropSettings;

    #[test]
    fn test_enforce_aspect_ratio_1_to_1() {
        let mut state = AppState::new();
        // Setup 100x100 image
        let img = image::DynamicImage::new_rgb8(100, 100);
        state.original_preview = Some(img);
        
        // Full initial crop (100x100) -> 0,0,1,1
        state.crop_settings = Some(CropSettings::default());
        
        // Select 1:1
        state.selected_aspect_ratio = AspectRatio::Square;
        
        CropPanel::enforce_aspect_ratio(&mut state);
        
        let crop = state.crop_settings.unwrap();
        // Should be 1.0 x 1.0 still (square image, square crop)
        assert!((crop.crop_width() - 1.0).abs() < 0.001);
        assert!((crop.crop_height() - 1.0).abs() < 0.001);
    }
    
    #[test]
    fn test_enforce_aspect_ratio_square_selection_on_landscape_image() {
          let mut state = AppState::new();
        // Setup 200x100 image (2:1 ratio)
        let img = image::DynamicImage::new_rgb8(200, 100);
        state.original_preview = Some(img);
        
        // Full initial crop (0,0,1,1) -> Covers 200x100 pixels.
        // Aspect Ratio of Crop = 2.0.
        state.crop_settings = Some(CropSettings::default());
        
        // Select 1:1
        state.selected_aspect_ratio = AspectRatio::Square;
        
        CropPanel::enforce_aspect_ratio(&mut state);
        
        // Validation:
        // Target Ratio = 1.0. 
        // Current Pixel Bounds = 200x100.
        // New New Pixel Bounds should be 100x100 (height constrained).
        // New Width = 100 pixels.
        // Normalized Width = 100/200 = 0.5.
        // Normalized Height = 100/100 = 1.0.
        
        let crop = state.crop_settings.unwrap();
        assert!((crop.crop_width() - 0.5).abs() < 0.001, "Width should be 0.5, got {}", crop.crop_width());
        assert!((crop.crop_height() - 1.0).abs() < 0.001, "Height should be 1.0");
        // Center of full is (0.5, 0.5) [Norm coords: x + w/2].
        // Full X=0, W=1 -> Cx = 0.5.
        // New W=0.5. New X = Cx - W/2 = 0.5 - 0.25 = 0.25.
        assert!((crop.crop_x() - 0.25).abs() < 0.001, "X shoud be 0.25");
    }

    #[test]
    fn test_enforce_aspect_ratio_tall_selection_on_landscape_image() {
          let mut state = AppState::new();
        // Setup 200x100 image (2:1 ratio)
        let img = image::DynamicImage::new_rgb8(200, 100);
        state.original_preview = Some(img);
        
        // Select 1:2 (Tall) -> "Portrait" slice.
        // Assume AspectRatio::ThreeFour (3:4 = 0.75).
        state.crop_settings = Some(CropSettings::default());
        state.selected_aspect_ratio = AspectRatio::ThreeFour;
        
        CropPanel::enforce_aspect_ratio(&mut state);
        
        // Target Pix Ratio = 0.75.
        // Current Pix Ratio = 2.0.
        // Must reduce width significantly.
        // New Width Pixels = Height Pixels * 0.75 = 100 * 0.75 = 75.
        // New Width Norm = 75 / 200 = 0.375.
        // Height Norm = 1.0.
         let crop = state.crop_settings.unwrap();
        assert!((crop.crop_width() - 0.375).abs() < 0.001, "Width should be 0.375, got {}", crop.crop_width());
        assert!((crop.crop_height() - 1.0).abs() < 0.001, "Height should be 1.0");
    }

    #[test]
    fn test_enforce_aspect_ratio_preserves_fill_mode() {
        let mut state = AppState::new();
        state.original_preview = Some(image::DynamicImage::new_rgb8(100, 100));
        
        // Initial setup with ShrinkToFit
        state.crop_settings = Some(
            CropSettings::default()
                .with_fill_mode(domain::value_objects::RotationFillMode::ShrinkToFit)
        );
        
        // Trigger enforce aspect ratio
        state.selected_aspect_ratio = AspectRatio::Square;
        CropPanel::enforce_aspect_ratio(&mut state);
        
        // Should preserve fill mode
        let crop = state.crop_settings.unwrap();
        assert_eq!(crop.fill_mode(), domain::value_objects::RotationFillMode::ShrinkToFit);
    }
}
