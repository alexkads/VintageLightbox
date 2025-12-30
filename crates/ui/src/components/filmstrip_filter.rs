use egui::{Ui, Color32, RichText, Sense};
use adapters::view_models::ColorLabel;
use crate::design_system::theme::Theme;
use adapters::state::PhotoFilters;

/// Render the filter UI controls
pub fn render(ui: &mut Ui, filter: &mut PhotoFilters) {
    ui.horizontal(|ui| {
        ui.label("Filter:");

        // Flags
        let mut toggle_flag = |pixel: &str, active: &mut bool, color: Color32| {
                let btn = egui::Button::new(RichText::new(pixel).color(if *active { Color32::WHITE } else { Color32::GRAY }))
                .fill(if *active { color } else { Color32::TRANSPARENT })
                .frame(true); 
                
                if ui.add(btn).clicked() {
                    *active = !*active;
                }
        };
        
        // Pick
        toggle_flag("P", &mut filter.show_flagged, Theme::ACCENT_SUCCESS);
        // Unflagged
        toggle_flag("U", &mut filter.show_unflagged, Color32::from_gray(100));
        // Reject
        toggle_flag("X", &mut filter.show_rejected, Theme::ACCENT_ERROR);

        ui.separator();

        // Rating
        ui.label("≥");
        for i in 1..=5 {
            let active = filter.min_rating >= i;
            let color = if active { Theme::RATING_ACTIVE } else { Color32::GRAY };
            if ui.add(egui::Label::new(RichText::new("★").color(color)).sense(Sense::click())).clicked() {
                if filter.min_rating == i {
                        filter.min_rating = 0; // Toggle off
                } else {
                        filter.min_rating = i;
                }
            }
        }

        ui.separator();

        // Colors
        let all_colors = [
            (ColorLabel::Red, Theme::LABEL_RED),
            (ColorLabel::Yellow, Theme::LABEL_YELLOW),
            (ColorLabel::Green, Theme::LABEL_GREEN),
            (ColorLabel::Blue, Theme::LABEL_BLUE),
            (ColorLabel::Purple, Theme::LABEL_PURPLE),
        ];

        for (label, color) in all_colors {
            let active = filter.color_labels.contains(&label);
            
            // Color block button
            let size = egui::Vec2::new(16.0, 16.0);
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                // Draw box
                    painter.rect_filled(
                    rect, 
                    egui::CornerRadius::same(2), 
                    if active { color } else { color.gamma_multiply(0.3) }
                );
                
                if active {
                        painter.rect_stroke(rect.expand(2.0), egui::CornerRadius::same(3), egui::Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Outside);
                }
            }
            
            if response.clicked() {
                if active {
                    filter.color_labels.remove(&label);
                } else {
                    filter.color_labels.insert(label);
                }
            }
            ui.add_space(2.0);
        }
        
        // Reset button if any active
            if filter.is_active() {
            ui.separator();
            egui::ComboBox::from_id_salt("filter_preset")
                .selected_text(RichText::new("Custom Filter").italics())
                .show_ui(ui, |ui| {
                    if ui.button("Filters Off").clicked() {
                        filter.reset();
                        ui.close_menu();
                    }
                });
            
            if ui.button("❎").on_hover_text("Turn off filters").clicked() {
                filter.reset();
            }
        } else {
                ui.separator();
                ui.label(RichText::new("Filters Off").weak());
        }

    });
}
