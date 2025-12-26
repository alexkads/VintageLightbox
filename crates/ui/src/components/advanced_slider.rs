// Advanced Slider Component
// Professional slider with editable value, custom styling, and double-click reset

use egui::{Ui, Sense, RichText, TextEdit, Color32, Rect, Rounding, Stroke, Vec2, pos2};
use crate::design_system::theme::Theme;

/// An advanced slider control with:
/// - Label and value display with accent colors
/// - Click on value to edit it directly
/// - Double-click on slider to reset to default
/// - Custom styled slider rail with gradient fill
pub struct AdvancedSlider;

impl AdvancedSlider {
    // Slider visual constants
    const RAIL_HEIGHT: f32 = 4.0;
    const KNOB_RADIUS: f32 = 6.0;
    const RAIL_ROUNDING: f32 = 2.0;
    
    /// Show an advanced slider with editable value and double-click reset
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
        
        // Check if value differs from default (for accent color)
        let is_modified = (*value - default_value).abs() > f32::EPSILON;
        
        ui.horizontal(|ui| {
            // Padding Left
            ui.add_space(Theme::SPACE_SM);

            // Label - with subtle indicator if modified
            let label_color = if is_modified {
                Theme::TEXT_PRIMARY
            } else {
                Theme::TEXT_SECONDARY
            };
            
            let label_response = ui.add(
                egui::Label::new(
                    RichText::new(label)
                        .size(Theme::FONT_SM)
                        .color(label_color)
                ).sense(Sense::click())
            );
            
            if (alt_pressed && label_response.clicked()) || label_response.double_clicked() {
                *value = default_value;
                changed = true;
                is_editing = false;
                edit_string = format!("{:.prec$}", default_value, prec = decimals);
            }
            
            label_response.on_hover_text("Double-click to reset");
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Padding Right
                ui.add_space(Theme::SPACE_MD);

                // Value display/edit
                if is_editing {
                    // Show text input when editing
                    let response = ui.add(
                        TextEdit::singleline(&mut edit_string)
                            .desired_width(45.0)
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
                    // Show value as clickable label with accent color when modified
                    let format_str = if decimals == 0 {
                        format!("{:.0}", *value)
                    } else if decimals == 1 {
                        format!("{:.1}", *value)
                    } else {
                        format!("{:.2}", *value)
                    };
                    
                    let value_color = if is_modified {
                        Theme::ACCENT_PRIMARY  // Blue accent when modified
                    } else {
                        Theme::TEXT_MUTED  // Muted when at default
                    };
                    
                    let value_response = ui.add(
                        egui::Label::new(
                            RichText::new(&format_str)
                                .size(Theme::FONT_SM)
                                .color(value_color)
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
                    value_response.on_hover_text("Click to edit • Double-click to reset");
                }
            });
        });
        
        // Custom styled slider
        let available_width = ui.available_width() - Theme::SPACE_SM * 2.0;
        let slider_height = Self::KNOB_RADIUS * 2.0 + 4.0;
        
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(available_width + Theme::SPACE_SM * 2.0, slider_height),
            Sense::click_and_drag()
        );
        
        // Adjust rect for padding
        let rail_rect = Rect::from_min_size(
            pos2(rect.min.x + Theme::SPACE_SM, rect.center().y - Self::RAIL_HEIGHT / 2.0),
            Vec2::new(available_width, Self::RAIL_HEIGHT)
        );
        
        let painter = ui.painter();
        
        // Calculate normalized position (0.0 to 1.0)
        let range_size = *range.end() - *range.start();
        let normalized = if range_size.abs() > f32::EPSILON {
            ((*value - *range.start()) / range_size).clamp(0.0, 1.0)
        } else {
            0.5
        };
        
        // Calculate center point for bipolar sliders (where default is in the middle)
        let default_normalized = if range_size.abs() > f32::EPSILON {
            ((default_value - *range.start()) / range_size).clamp(0.0, 1.0)
        } else {
            0.5
        };
        
        // Draw rail background
        painter.rect_filled(
            rail_rect,
            Rounding::from(Self::RAIL_ROUNDING),
            Theme::BG_ELEVATED
        );
        
        // Draw fill from center/zero point to current value
        let fill_start_x = rail_rect.left() + default_normalized * rail_rect.width();
        let fill_end_x = rail_rect.left() + normalized * rail_rect.width();
        
        let (fill_left, fill_right) = if fill_end_x >= fill_start_x {
            (fill_start_x, fill_end_x)
        } else {
            (fill_end_x, fill_start_x)
        };
        
        if is_modified {
            let fill_rect = Rect::from_min_max(
                pos2(fill_left, rail_rect.top()),
                pos2(fill_right, rail_rect.bottom())
            );
            painter.rect_filled(
                fill_rect,
                Rounding::ZERO,
                Theme::ACCENT_PRIMARY.linear_multiply(0.6)
            );
        }
        
        // Draw knob
        let knob_x = rail_rect.left() + normalized * rail_rect.width();
        let knob_center = pos2(knob_x, rect.center().y);
        
        // Knob shadow (subtle)
        painter.circle_filled(
            pos2(knob_center.x, knob_center.y + 1.0),
            Self::KNOB_RADIUS,
            Color32::from_rgba_unmultiplied(0, 0, 0, 40)
        );
        
        // Knob fill
        let knob_color = if response.hovered() || response.dragged() {
            Color32::WHITE
        } else if is_modified {
            Color32::from_rgb(230, 230, 230)
        } else {
            Color32::from_rgb(200, 200, 200)
        };
        
        painter.circle_filled(knob_center, Self::KNOB_RADIUS, knob_color);
        
        // Knob border
        if response.hovered() || response.dragged() {
            painter.circle_stroke(
                knob_center,
                Self::KNOB_RADIUS,
                Stroke::new(1.5, Theme::ACCENT_PRIMARY)
            );
        }
        
        // Handle interaction
        if response.dragged() {
            let pointer_pos = response.interact_pointer_pos();
            if let Some(pos) = pointer_pos {
                let new_normalized = ((pos.x - rail_rect.left()) / rail_rect.width()).clamp(0.0, 1.0);
                let new_value = *range.start() + new_normalized * range_size;
                if (*value - new_value).abs() > f32::EPSILON {
                    *value = new_value;
                    changed = true;
                }
            }
        }
        
        // Reset Logic: Double-click OR Alt+Click OR Right-click
        if response.double_clicked() || (alt_pressed && response.clicked()) || response.secondary_clicked() {
            *value = default_value;
            changed = true;
            edit_string = format!("{:.prec$}", default_value, prec = decimals);
        }
        
        // Request repaint if dragging for smooth updates
        if response.dragged() {
            ui.ctx().request_repaint();
        }
        
        // Show reset hint on hover (must be last as it consumes response)
        response.on_hover_text("Double-click to reset");
        
        // Store edit state
        ui.memory_mut(|mem| {
            mem.data.insert_temp(edit_mode_id, is_editing);
            mem.data.insert_temp(id, edit_string);
        });
        
        // Compact padding at bottom
        ui.add_space(Theme::SPACE_SM);
        
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
