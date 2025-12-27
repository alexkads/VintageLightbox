// Image Viewer Component
// Displays images with zoom, pan, and navigation controls

use egui::{Ui, Vec2, Rect, Sense, UiBuilder, Color32};
use crate::state::{AppState, CurrentView};
use crate::design_system::{theme::Theme, widgets};

pub struct ImageViewer;

impl ImageViewer {
    /// Show the image viewer with zoom/pan capabilities
    pub fn show(ui: &mut Ui, state: &mut AppState) {
        let (new_zoom, new_pan, painted_image_rect, viewer_rect) = Self::render(
            ui,
            state.detail_image.as_ref(),
            state.thumbnail_preview.as_ref(),
            state.develop_selected_photo_id.is_some(),
            state.zoom_level,
            state.pan_offset,
            true, // interactive
            !state.crop_mode_active, // allow_pan: Disable pan in crop mode (unless Space is held)
            // If crop mode is active, we show the original image (untransformed) so the user can edit the crop.
            // If crop mode is INACTIVE, we show the applied crop.
            if state.crop_mode_active { None } else { state.crop_settings.as_ref() },
        );

        state.zoom_level = new_zoom;
        state.pan_offset = new_pan;

        // Show crop overlay if in crop mode
        if state.crop_mode_active {
            if let Some(crop_settings) = &mut state.crop_settings {
                if let Some(img_rect) = painted_image_rect {
                    // We also need the texture size to handle drag deltas correctly
                    // We can infer it or get it from state again
                     if let Some(texture) = state.detail_image.as_ref().or(state.thumbnail_preview.as_ref()) {
                        let texture_size = Vec2::new(texture.size()[0] as f32, texture.size()[1] as f32);

                        // Show crop overlay
                        use crate::components::crop_overlay::CropOverlay;
                        let overlay_response = CropOverlay::show(
                            ui,
                            img_rect,
                            viewer_rect, 
                            crop_settings,
                            state.show_composition_grid,
                        );

                        // Handle overlay interactions
                        if let Some(handle_index) = overlay_response.handle_dragged {
                            CropOverlay::update_from_handle_drag(
                                crop_settings,
                                handle_index,
                                overlay_response.drag_delta,
                                texture_size,
                                &state.selected_aspect_ratio,
                            );
                        } else if overlay_response.crop_dragged {
                            CropOverlay::update_from_crop_drag(
                                crop_settings,
                                overlay_response.drag_delta,
                                texture_size,
                            );
                        } else if overlay_response.rotation_dragged {
                            // Straighten / Rotation Logic
                            // Calculate angle change based on x/y delta or angular movement
                            // Simple horizontal drag for rotation is common in sliders, but on image maybe specialized behavior
                            // For simplicity, let's map horizontal drag to rotation angle
                            let sensitivity = 0.5; // degrees per pixel
                            let delta_degrees = overlay_response.drag_delta.x * sensitivity;
                            
                            // Update crop angle
                            let new_angle = crop_settings.angle() + delta_degrees;
                            // Clamp angle if needed? Usually straighten is limited (e.g. +/- 45 deg)
                            
                            *crop_settings = domain::value_objects::CropSettings::new(
                                crop_settings.crop_x(), crop_settings.crop_y(), crop_settings.crop_width(), crop_settings.crop_height(),
                                crop_settings.rotation_90(), new_angle,
                                crop_settings.flip_horizontal(), crop_settings.flip_vertical()
                            );
                        }
                    }
                }
            }
        }
        
        // UI overlay elements (Main viewer only)
        Self::show_controls(ui, state, ui.max_rect());
    }

    /// Stateless rendering of the image viewer content
    /// Returns (new_zoom, new_pan, painted_image_rect)
    pub fn render(
        ui: &mut Ui,
        detail_image: Option<&egui::TextureHandle>,
        thumbnail_preview: Option<&egui::TextureHandle>,
        has_selection: bool,
        current_zoom: f32,
        current_pan: Vec2,
        interactive: bool,
        allow_pan: bool, // Restored parameter
        crop_settings: Option<&domain::value_objects::CropSettings>, // Updated for crop support
    ) -> (f32, Vec2, Option<Rect>, Rect) {
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
        let mut painted_rect = None;

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
            // Allow pan if explicitly allowed OR if Spacebar is held (Space+Drag to Pan override)
            let space_held = ui.input(|i| i.key_down(egui::Key::Space));
            let should_pan = allow_pan || space_held;
            
            if should_pan && response.dragged() {
                pan += response.drag_delta();
            }

            // Set cursor for pan mode
            if space_held && response.hovered() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                if response.dragged() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
                }
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

            let mut img = egui::Image::new(texture);

            // Apply crop and rotation if settings are provided
            // AND we are not in crop mode (in crop mode we show full image with overlay)
            // But wait, the `render` function receives `crop_settings`.
            // The caller handles logic: 
            // - If in interactive mode (crop mode), caller might pass None or handle it differently?
            // - Actually, ImageViewer::show passes `!state.crop_mode_active` as `allow_pan`. 
            // - And previously it passed angle. 
            // - If `crop_mode_active` is true, we want FULL image.
            // - If `crop_mode_active` is false, we want CROPPED image.
            // - So we should pass `crop_settings` ONLY if we want them applied.
            
            if let Some(crop) = crop_settings {
                // Calculate UV
                // Note: UV coordinates are (0,0) top-left to (1,1) bottom-right
                // crop_x/y are top-left relative to image
                // TODO: Handle flip_h/flip_v if egui supports it via UV swapping? 
                // egui::Rect enforces min <= max, so standard Rect can't represent flip.
                // We might need to rotate 180 for flips or similar? 
                // For now, implementing crop and rotation.
                
                let uv = Rect::from_min_size(
                    egui::pos2(crop.crop_x(), crop.crop_y()), 
                    egui::vec2(crop.crop_width(), crop.crop_height())
                );
                img = img.uv(uv);
                
                // Rotation
                // Sum rotation_90 and fine angle
                let total_degrees = (crop.rotation_90() as f32 * 90.0) + crop.angle();
                img = img.rotate(total_degrees.to_radians(), Vec2::splat(0.5));
            }

            img.paint_at(ui, img_rect);
            painted_rect = Some(img_rect);
            
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
            painted_rect = Some(img_rect);
            
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

        (zoom, pan, painted_rect, rect)
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

