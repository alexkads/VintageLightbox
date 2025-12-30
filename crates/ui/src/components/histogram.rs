// Histogram Component
// Displays image histogram with RGB channels

use egui::{Ui, Vec2, Rect, Color32};
use crate::design_system::theme::Theme;
// Re-export infrastructure definition to avoid breaking existing users
pub use infrastructure::image_processing::HistogramData;

// Removed local Definition of HistogramData since it's now in infrastructure

pub struct Histogram;

impl Histogram {
    /// Show histogram widget
    /// If histogram_data is provided, displays real histogram
    /// Otherwise shows placeholder
    pub fn show(ui: &mut Ui, histogram_data: Option<&HistogramData>) {
        ui.vertical(|ui| {
            ui.set_height(100.0);
            ui.set_width(ui.available_width());
            
            ui.label(
                egui::RichText::new("Histogram")
                    .size(Theme::FONT_SM)
                    .color(Theme::TEXT_MUTED)
            );
            
            ui.add_space(Theme::SPACE_XS);
            
            if let Some(data) = histogram_data {
                Self::render_histogram(ui, data);
            } else {
                // Placeholder when no image is loaded
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(
                    rect,
                    Theme::RADIUS_SM,
                    Color32::from_rgb(30, 30, 35),
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No image",
                    egui::FontId::proportional(Theme::FONT_XS),
                    Theme::TEXT_MUTED,
                );
            }
        });
    }
    
    /// Render the actual histogram bars
    fn render_histogram(ui: &mut Ui, data: &HistogramData) {
        let available_rect = ui.available_rect_before_wrap();
        let width = available_rect.width();
        let height = 70.0;
        
        let rect = Rect::from_min_size(
            available_rect.min,
            Vec2::new(width, height),
        );
        
        // Background
        ui.painter().rect_filled(
            rect,
            Theme::RADIUS_SM,
            Color32::from_rgb(20, 20, 25),
        );
        
        // Draw histogram bars
        let bar_width = width / 256.0;
        
        for i in 0..256 {
            let x = rect.min.x + (i as f32 * bar_width);
            
            // Normalize values
            let red_height = (data.red[i] as f32 / data.max_value as f32) * height;
            let green_height = (data.green[i] as f32 / data.max_value as f32) * height;
            let blue_height = (data.blue[i] as f32 / data.max_value as f32) * height;
            
            // Draw bars from bottom up
            if red_height > 0.0 {
                let bar_rect = Rect::from_min_size(
                    egui::pos2(x, rect.max.y - red_height),
                    Vec2::new(bar_width.max(1.0), red_height),
                );
                ui.painter().rect_filled(
                    bar_rect,
                    0.0,
                    Color32::from_rgba_premultiplied(255, 0, 0, 100),
                );
            }
            
            if green_height > 0.0 {
                let bar_rect = Rect::from_min_size(
                    egui::pos2(x, rect.max.y - green_height),
                    Vec2::new(bar_width.max(1.0), green_height),
                );
                ui.painter().rect_filled(
                    bar_rect,
                    0.0,
                    Color32::from_rgba_premultiplied(0, 255, 0, 100),
                );
            }
            
            if blue_height > 0.0 {
                let bar_rect = Rect::from_min_size(
                    egui::pos2(x, rect.max.y - blue_height),
                    Vec2::new(bar_width.max(1.0), blue_height),
                );
                ui.painter().rect_filled(
                    bar_rect,
                    0.0,
                    Color32::from_rgba_premultiplied(0, 0, 255, 100),
                );
            }
        }
        
        ui.allocate_space(Vec2::new(width, height));
    }
}
