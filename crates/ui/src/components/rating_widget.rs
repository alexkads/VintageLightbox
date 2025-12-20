#![allow(dead_code)]

// Rating Widget Component
// Interactive star rating (0-5 stars)

use egui::{Ui, Vec2, Sense};
use crate::design_system::theme::Theme;

pub struct RatingWidget;

impl RatingWidget {
    /// Show an interactive rating widget
    /// Returns Some(new_rating) if the rating was changed
    pub fn show(ui: &mut Ui, rating: &mut i32, star_size: f32) -> Option<i32> {
        Self::show_impl(ui, rating, true, star_size)
    }

    /// Show a non-interactive rating display
    pub fn show_readonly(ui: &mut Ui, rating: i32, star_size: f32) {
        let mut r = rating;
        Self::show_impl(ui, &mut r, false, star_size);
    }

    /// Internal implementation for both interactive and readonly modes
    fn show_impl(ui: &mut Ui, rating: &mut i32, interactive: bool, star_size: f32) -> Option<i32> {
        let spacing = Theme::SPACE_XS;
        let total_width = (star_size * 5.0) + (spacing * 4.0);

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(total_width, star_size),
            if interactive { Sense::click() } else { Sense::hover() },
        );

        let mut changed_rating: Option<i32> = None;
        let mut hover_star: Option<usize> = None;

        // Check which star is being hovered
        if interactive && response.hovered() {
            if let Some(hover_pos) = response.hover_pos() {
                let relative_x = hover_pos.x - rect.min.x;
                hover_star = Some((relative_x / (star_size + spacing)).floor() as usize);
            }
        }

        // Draw stars
        for i in 0..5 {
            let star_x = rect.min.x + (i as f32 * (star_size + spacing));
            let star_pos = egui::pos2(star_x + star_size / 2.0, rect.center().y);

            let is_filled = i < *rating as usize;
            let is_hover_preview = interactive && hover_star.map_or(false, |h| i <= h);

            let color = if is_filled || is_hover_preview {
                Theme::RATING_ACTIVE
            } else {
                Theme::RATING_INACTIVE
            };

            ui.painter().text(
                star_pos,
                egui::Align2::CENTER_CENTER,
                "★",
                egui::FontId::proportional(star_size),
                color,
            );
        }

        // Handle clicks
        if interactive && response.clicked() {
            if let Some(click_pos) = response.interact_pointer_pos() {
                let relative_x = click_pos.x - rect.min.x;
                let clicked_star = (relative_x / (star_size + spacing)).floor() as i32;
                let new_rating = (clicked_star + 1).clamp(0, 5);

                if new_rating != *rating {
                    *rating = new_rating;
                    changed_rating = Some(new_rating);
                }
            }
        }

        changed_rating
    }

    /// Compact inline display for thumbnails (non-interactive)
    pub fn show_compact(ui: &mut Ui, rating: i32) {
        if rating == 0 {
            return;
        }

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            for i in 0..5 {
                let color = if i < rating {
                    Theme::RATING_ACTIVE
                } else {
                    Theme::RATING_INACTIVE
                };
                ui.label(egui::RichText::new("★").size(11.0).color(color));
            }
        });
    }
}
