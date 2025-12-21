// Image Viewer Component
// Displays images with zoom, pan, and navigation controls

use egui::{Ui, Vec2, Rect, Sense, UiBuilder};
use crate::state::{AppState, CurrentView};
use crate::design_system::{theme::Theme, widgets};

pub struct ImageViewer;

impl ImageViewer {
    /// Show the image viewer with zoom/pan capabilities
    pub fn show(ui: &mut Ui, state: &mut AppState) {
        let available_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());

        // Fill background
        ui.painter().rect_filled(rect, 0.0, Theme::BG_APP);

        // Handle zoom with scroll
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
            if scroll_delta != 0.0 {
                let zoom_delta = scroll_delta * 0.001;
                state.zoom_level = (state.zoom_level + zoom_delta).clamp(0.5, 5.0);
            }
        }

        // Handle pan with drag
        if response.dragged() {
            state.pan_offset += response.drag_delta();
        }

        // Double-click to reset
        if response.double_clicked() {
            state.reset_viewer();
        }

        // Draw image if available
        if let Some(texture) = &state.detail_image {
            let texture_size = Vec2::new(texture.size()[0] as f32, texture.size()[1] as f32);

            // Calculate scaled size
            let scale = (available_size.x / texture_size.x)
                .min(available_size.y / texture_size.y)
                .min(1.0); // Don't upscale beyond original size

            let base_img_size = texture_size * scale;
            let zoomed_size = base_img_size * state.zoom_level;

            let center = rect.center() + state.pan_offset;
            let img_rect = Rect::from_center_size(center, zoomed_size);

            egui::Image::new(texture).paint_at(ui, img_rect);
        } else {
            // No image loaded
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No image loaded",
                egui::FontId::proportional(Theme::FONT_XL),
                Theme::TEXT_MUTED,
            );
        }

        // UI overlay elements
        Self::show_controls(ui, state, rect);
    }

    /// Show viewer controls (back button, navigation, zoom indicator)
    fn show_controls(ui: &mut Ui, state: &mut AppState, rect: Rect) {
        // Back to Library button (top-left)
        let back_pos = rect.min + Vec2::new(Theme::SPACE_LG, Theme::SPACE_LG);
        let back_size = Vec2::new(100.0, 32.0);
        let back_rect = Rect::from_min_size(back_pos, back_size);

        ui.allocate_new_ui(UiBuilder::new().max_rect(back_rect), |ui| {
            if widgets::secondary_button(ui, "← Library").clicked() {
                state.current_view = CurrentView::Library;
                state.reset_viewer();
            }
        });

        // Navigation arrows
        if state.has_photos() {
            // Left arrow
            let left_pos = rect.min + Vec2::new(Theme::SPACE_LG, rect.height() / 2.0 - 20.0);
            let left_rect = Rect::from_min_size(left_pos, Vec2::new(40.0, 40.0));

            ui.allocate_new_ui(UiBuilder::new().max_rect(left_rect), |ui| {
                if widgets::icon_button(ui, "‹").clicked() {
                    if let Some(new_id) = state.navigate(-1) {
                        state.selected_photo_id = Some(new_id);
                    }
                }
            });

            // Right arrow
            let right_pos = rect.max - Vec2::new(40.0 + Theme::SPACE_LG, rect.height() / 2.0 + 20.0);
            let right_rect = Rect::from_min_size(right_pos, Vec2::new(40.0, 40.0));

            ui.allocate_new_ui(UiBuilder::new().max_rect(right_rect), |ui| {
                if widgets::icon_button(ui, "›").clicked() {
                    if let Some(new_id) = state.navigate(1) {
                        state.selected_photo_id = Some(new_id);
                    }
                }
            });
        }

        // Zoom indicator (bottom-right)
        if state.zoom_level != 1.0 {
            let zoom_text = format!("{}%", (state.zoom_level * 100.0).round());
            let zoom_pos = rect.max - Vec2::new(80.0 + Theme::SPACE_LG, Theme::SPACE_LG + 12.0);
            let zoom_rect = Rect::from_center_size(zoom_pos, Vec2::new(60.0, 24.0));

            ui.painter().rect_filled(
                zoom_rect,
                Theme::RADIUS_SM,
                Theme::BG_ELEVATED,
            );
            ui.painter().text(
                zoom_rect.center(),
                egui::Align2::CENTER_CENTER,
                zoom_text,
                egui::FontId::proportional(Theme::FONT_SM),
                Theme::TEXT_SECONDARY,
            );
        }
    }
}
