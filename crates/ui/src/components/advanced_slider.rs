// Advanced Slider Component
// Professional slider with editable value and double-click reset

use egui::{Ui, Sense, RichText, TextEdit};
use crate::design_system::theme::Theme;

/// An advanced slider control with:
/// - Label and value display
/// - Click on value to edit it directly
/// - Double-click on slider to reset to default
pub struct AdvancedSlider;

impl AdvancedSlider {
    /// Show an advanced slider with editable value and double-click reset
    /// 
    /// # Arguments
    /// * `ui` - The UI to add the slider to
    /// * `id_source` - Unique ID for the slider (used for edit state tracking)
    /// * `label` - Display label
    /// * `value` - Current value (will be modified)
    /// * `range` - Valid range for the value
    /// * `default_value` - Value to reset to on double-click
    /// * `decimals` - Number of decimal places to show
    /// 
    /// # Returns
    /// `true` if the value changed
    pub fn show(
        ui: &mut Ui,
        id_source: &str,
        label: &str,
        value: &mut f32,
        range: std::ops::RangeInclusive<f32>,
        default_value: f32,
        decimals: usize,
    ) -> bool {
        let mut changed = false;
        let id = ui.make_persistent_id(id_source);
        
        // Check if we're in edit mode for this slider
        let edit_mode_id = ui.make_persistent_id(format!("{}_edit", id_source));
        let mut is_editing = ui.memory(|mem| mem.data.get_temp::<bool>(edit_mode_id).unwrap_or(false));
        let mut edit_string = ui.memory(|mem| {
            mem.data.get_temp::<String>(id).unwrap_or_else(|| format!("{:.prec$}", *value, prec = decimals))
        });
        
        // Check for Alt/Option modifier key
        let alt_pressed = ui.input(|i| i.modifiers.alt);
        
        ui.horizontal(|ui| {
            // Padding Left
            ui.add_space(Theme::SPACE_XS);

            // Label - Alt+Click to reset
            let label_response = ui.add(
                egui::Label::new(
                    RichText::new(label)
                        .size(Theme::FONT_SM)
                ).sense(Sense::click())
            );
            
            if (alt_pressed && label_response.clicked()) || label_response.double_clicked() {
                *value = default_value;
                changed = true;
                is_editing = false;
                edit_string = format!("{:.prec$}", default_value, prec = decimals);
            }
            
            label_response.on_hover_text("Double-click or Alt+Click to reset");
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Padding Right - prevents value from touching the edge
                ui.add_space(Theme::SPACE_MD);

                // Value display/edit
                if is_editing {
                    // Show text input when editing
                    let response = ui.add(
                        TextEdit::singleline(&mut edit_string)
                            .desired_width(50.0)
                            .font(egui::TextStyle::Body)
                    );
                    
                    // Handle enter or focus loss
                    if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Ok(new_value) = edit_string.parse::<f32>() {
                            let clamped = new_value.clamp(*range.start(), *range.end());
                            if (*value - clamped).abs() > f32::EPSILON {
                                *value = clamped;
                                changed = true;
                            }
                        }
                        is_editing = false;
                    }
                    
                    // Handle escape to cancel
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        is_editing = false;
                    }
                    
                    // Request focus on the text edit
                    if !response.has_focus() {
                        response.request_focus();
                    }
                } else {
                    // Show value as clickable label
                    let format_str = if decimals == 0 {
                        format!("{:.0}", *value)
                    } else if decimals == 1 {
                        format!("{:.1}", *value)
                    } else {
                        format!("{:.2}", *value)
                    };
                    
                    let value_response = ui.add(
                        egui::Label::new(
                            RichText::new(&format_str)
                                .size(Theme::FONT_SM)
                                .color(ui.visuals().hyperlink_color)
                        ).sense(Sense::click())
                    );
                    
                    // Alt+Click or Double-Click to reset, regular click to edit
                    if (alt_pressed && value_response.clicked()) || value_response.double_clicked() {
                        *value = default_value;
                        changed = true;
                        edit_string = format!("{:.prec$}", default_value, prec = decimals);
                    } else if value_response.clicked() {
                        is_editing = true;
                        edit_string = format_str;
                    }
                    
                    // Tooltip on hover
                    value_response.on_hover_text("Click to edit\nDouble-click or Alt+Click to reset");
                }
            });
        });
        
        // Larger space between label and slider for breathing room
        ui.add_space(Theme::SPACE_XS);
        
        // Slider with proper horizontal padding
        let slider_response = ui.horizontal(|ui| {
            ui.add_space(Theme::SPACE_XS);
            let s = ui.add(
                egui::Slider::new(value, range.clone())
                    .show_value(false)
            );
            ui.add_space(Theme::SPACE_MD); // Match the value padding
            s
        }).inner;
        
        if slider_response.changed() {
            changed = true;
        }
        
        // Reset Logic: Double-click OR Alt+Click OR Right-click
        if slider_response.double_clicked() || (alt_pressed && slider_response.clicked()) || slider_response.secondary_clicked() {
            *value = default_value;
            changed = true;
            edit_string = format!("{:.prec$}", default_value, prec = decimals);
        }
        
        // Show reset hint on hover
        slider_response.on_hover_text("Double-click, Right-click or Alt+Click to reset");
        
        // Store edit state
        ui.memory_mut(|mem| {
            mem.data.insert_temp(edit_mode_id, is_editing);
            mem.data.insert_temp(id, edit_string);
        });
        
        // Much larger padding at bottom for clear visual separation
        ui.add_space(Theme::SPACE_LG);
        
        changed
    }
    
    /// Simplified version with fewer parameters (uses sensible defaults)
    pub fn simple(
        ui: &mut Ui,
        label: &str,
        value: &mut f32,
        range: std::ops::RangeInclusive<f32>,
        default_value: f32,
    ) -> bool {
        Self::show(ui, label, label, value, range, default_value, 2)
    }
}
