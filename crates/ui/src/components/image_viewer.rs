// Image Viewer Component
// Displays images with zoom, pan, and navigation controls

use egui::{Ui, Vec2, Rect, Sense, UiBuilder, Color32};
use crate::state::{AppState, CurrentView};
use crate::design_system::{theme::Theme, widgets};

pub struct ImageViewer;

impl ImageViewer {
    /// Show the image viewer with zoom/pan capabilities
    pub fn show(ui: &mut Ui, state: &mut AppState) {
        let (new_zoom, new_pan) = Self::render(
            ui,
            state.detail_image.as_ref(),
            state.thumbnail_preview.as_ref(),
            state.develop_selected_photo_id.is_some(),
            state.zoom_level,
            state.pan_offset,
            true, // interactive
        );

        state.zoom_level = new_zoom;
        state.pan_offset = new_pan;

        // Double-click reset handled in render via return values or we need to pass interaction back? 
        // Actually, render returning modified zoom/pan is cleanest.
        // We also need to handle the "Back" button and overlays, which are specific to the main Interactive viewer.
        
        // UI overlay elements (Main viewer only)
        Self::show_controls(ui, state, ui.max_rect());
    }

    /// Stateless rendering of the image viewer content
    /// Returns (new_zoom, new_pan)
    pub fn render(
        ui: &mut Ui,
        detail_image: Option<&egui::TextureHandle>,
        thumbnail_preview: Option<&egui::TextureHandle>,
        has_selection: bool,
        current_zoom: f32,
        current_pan: Vec2,
        interactive: bool,
    ) -> (f32, Vec2) {
        let available_size = ui.available_size();
        let match_size_arg = if interactive {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(available_size, match_size_arg);

        // Fill background
        ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill);

        let mut zoom = current_zoom;
        let mut pan = current_pan;

        if interactive {
            // Handle zoom with scroll
            if response.hovered() {
                let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
                if scroll_delta != 0.0 {
                    let zoom_delta = scroll_delta * 0.001;
                    zoom = (zoom + zoom_delta).clamp(0.5, 5.0);
                }
            }

            // Handle pan with drag
            if response.dragged() {
                pan += response.drag_delta();
            }

            // Double-click to reset
            if response.double_clicked() {
                zoom = 1.0;
                pan = Vec2::ZERO;
            }
        }

        // Draw image logic
        if let Some(texture) = detail_image {
            // Full resolution image available
            let texture_size = Vec2::new(texture.size()[0] as f32, texture.size()[1] as f32);

            // Calculate scaled size
            let scale = (available_size.x / texture_size.x)
                .min(available_size.y / texture_size.y)
                .min(1.0); // Don't upscale beyond original size

            let base_img_size = texture_size * scale;
            let zoomed_size = base_img_size * zoom;

            let center = rect.center() + pan;
            let img_rect = Rect::from_center_size(center, zoomed_size);

            egui::Image::new(texture).paint_at(ui, img_rect);
            
        } else if let Some(thumbnail) = thumbnail_preview {
            // LIGHTROOM-STYLE: Show thumbnail with effects
            let texture_size = Vec2::new(thumbnail.size()[0] as f32, thumbnail.size()[1] as f32);

            let scale = (available_size.x / texture_size.x)
                .min(available_size.y / texture_size.y);

            let base_img_size = texture_size * scale;
            let zoomed_size = base_img_size * zoom;

            let center = rect.center() + pan;
            let img_rect = Rect::from_center_size(center, zoomed_size);

            egui::Image::new(thumbnail).paint_at(ui, img_rect);
            
            // Show subtle loading indicator
            let loading_rect = Rect::from_min_size(
                rect.right_top() - Vec2::new(120.0, -10.0),
                Vec2::new(110.0, 24.0)
            );
            ui.painter().rect_filled(loading_rect, 4.0, Color32::from_black_alpha(180));
            ui.painter().text(
                loading_rect.center(),
                egui::Align2::CENTER_CENTER,
                "⏳ Loading HD...",
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );
            
            ui.ctx().request_repaint();
        } else if has_selection {
            // No thumbnail available, show spinner
            let time = ui.ctx().input(|i| i.time);
            let spinner_char = match ((time * 8.0) as usize) % 4 {
                0 => "◐",
                1 => "◓",
                2 => "◑",
                _ => "◒",
            };
            
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{} Loading...", spinner_char),
                egui::FontId::proportional(Theme::FONT_XL),
                ui.visuals().weak_text_color(),
            );
            
            ui.ctx().request_repaint();
        } else {
            // No image selected
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Select a photo to view",
                egui::FontId::proportional(Theme::FONT_XL),
                ui.visuals().weak_text_color(),
            );
        }

        (zoom, pan)
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

        // Zoom indicator (bottom-right)
        if state.zoom_level != 1.0 {
            let zoom_text = format!("{}%", (state.zoom_level * 100.0).round());
            let zoom_pos = rect.max - Vec2::new(80.0 + Theme::SPACE_LG, Theme::SPACE_LG + 12.0);
            let zoom_rect = Rect::from_center_size(zoom_pos, Vec2::new(60.0, 24.0));

            ui.painter().rect_filled(
                zoom_rect,
                Theme::RADIUS_SM,
                ui.visuals().window_fill(),
            );
            ui.painter().text(
                zoom_rect.center(),
                egui::Align2::CENTER_CENTER,
                zoom_text,
                egui::FontId::proportional(Theme::FONT_SM),
                ui.visuals().text_color(),
            );
        }
    }
}

