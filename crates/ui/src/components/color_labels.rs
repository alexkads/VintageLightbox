#![allow(dead_code)]

// Color Labels Component
// 5 circular color buttons for photo labeling

use egui::{Ui, Vec2, Sense, Color32, Stroke};
use crate::design_system::theme::Theme;

pub struct ColorLabels;

impl ColorLabels {
    /// Available color label options
    const COLORS: [(&'static str, Color32); 5] = [
        ("red", Theme::LABEL_RED),
        ("yellow", Theme::LABEL_YELLOW),
        ("green", Theme::LABEL_GREEN),
        ("blue", Theme::LABEL_BLUE),
        ("purple", Theme::LABEL_PURPLE),
    ];

    /// Show interactive color labels
    /// Returns Some(color_name) if a color was clicked
    pub fn show(
        ui: &mut Ui,
        selected_color: &Option<String>,
        interactive: bool,
    ) -> Option<String> {
        let mut clicked_color = None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = Theme::SPACE_SM;

            for (name, color) in &Self::COLORS {
                let size = Vec2::new(16.0, 16.0);
                let (rect, response) = ui.allocate_exact_size(
                    size,
                    if interactive { Sense::click() } else { Sense::hover() },
                );

                let is_selected = selected_color.as_deref() == Some(*name);
                let opacity = if is_selected || (interactive && response.hovered()) {
                    1.0
                } else {
                    0.7
                };

                let color_with_opacity = Color32::from_rgba_premultiplied(
                    color.r(),
                    color.g(),
                    color.b(),
                    (255.0 * opacity) as u8,
                );

                ui.painter().circle_filled(rect.center(), 8.0, color_with_opacity);

                if is_selected {
                    ui.painter().circle_stroke(
                        rect.center(),
                        9.0,
                        Stroke::new(2.0, Theme::TEXT_PRIMARY),
                    );
                }

                if interactive && response.clicked() {
                    clicked_color = Some(name.to_string());
                }
            }
        });

        clicked_color
    }

    /// Get the color for a given label name
    pub fn get_color(label: &str) -> Option<Color32> {
        Self::COLORS
            .iter()
            .find(|(name, _)| *name == label)
            .map(|(_, color)| *color)
    }

    /// Show a single color label indicator
    pub fn show_indicator(ui: &mut Ui, label: &Option<String>) {
        if let Some(label_name) = label {
            if let Some(color) = Self::get_color(label_name) {
                let size = Vec2::new(12.0, 12.0);
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                ui.painter().circle_filled(rect.center(), 6.0, color);
            }
        }
    }
}
