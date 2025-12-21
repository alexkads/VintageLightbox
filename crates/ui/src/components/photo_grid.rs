#![allow(dead_code)]

// Photo Grid Component
// Displays photos in a 5-column grid layout with thumbnails

use egui::{Ui, Vec2, Sense, Image, Rect, Color32};
use std::collections::HashMap;

use crate::state::{AppState, CurrentView};
use crate::design_system::theme::Theme;
use adapters::view_models::PhotoViewModel;

pub struct PhotoGrid {
    /// Cache of loaded thumbnail textures
    thumbnail_cache: HashMap<String, egui::TextureHandle>,
}

impl PhotoGrid {
    pub fn new() -> Self {
        Self {
            thumbnail_cache: HashMap::new(),
        }
    }

    /// Show the photo grid
    pub fn show(
        &mut self,
        ui: &mut Ui,
        state: &mut AppState,
        ctx: &egui::Context,
    ) {
        // Get filtered photos
        let filtered_photos = state.get_filtered_photos();

        if filtered_photos.is_empty() {
            self.show_empty_state(ui);
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.show_grid(ui, state, ctx, &filtered_photos);
            });
    }

    /// Show the grid of photo tiles
    fn show_grid(
        &mut self,
        ui: &mut Ui,
        state: &mut AppState,
        ctx: &egui::Context,
        photos: &[PhotoViewModel],
    ) {
        let columns = 5;
        let spacing = Theme::SPACE_SM;
        let available_width = ui.available_width();
        let tile_width = (available_width - (spacing * (columns - 1) as f32)) / columns as f32;

        ui.spacing_mut().item_spacing = Vec2::new(spacing, spacing);

        // Layout photos in rows
        for chunk in photos.chunks(columns) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = spacing;

                for photo in chunk {
                    self.show_tile(ui, photo, state, ctx, tile_width);
                }
            });
        }
    }

    /// Show a single photo tile
    fn show_tile(
        &mut self,
        ui: &mut Ui,
        photo: &PhotoViewModel,
        state: &mut AppState,
        ctx: &egui::Context,
        tile_width: f32,
    ) {
        let tile_size = Vec2::new(tile_width, Theme::TILE_HEIGHT);

        let (rect, response) = ui.allocate_exact_size(tile_size, Sense::click());

        // Handle click
        if response.clicked() {
            state.selected_photo_id = Some(photo.id.clone());
            
            // Populate metadata for detail view
            state.detail_metadata = Some(crate::state::DetailMetadata {
                id: photo.id.clone(),
                name: photo.name.clone(),
                date: photo.date.clone(),
                camera: photo.camera.clone(),
                exposure: photo.exposure.clone(),
                rating: photo.rating,
                color_label: photo.color_label.clone(),
            });
            
            // Load full image asynchronously
            let photo_path = photo.path.clone();
            let _photo_id = photo.id.clone();
            let ctx_clone = ctx.clone();
            let exposure = photo.edit_exposure.unwrap_or(0.0);
            let contrast = photo.edit_contrast.unwrap_or(1.0);
            
            // Update active edit values
            state.active_exposure = exposure;
            state.active_contrast = contrast;
            
            tokio::spawn(async move {
                // Load image from disk
                if let Ok(_img) = image::open(&photo_path) {
                    // Store in active_image for processing
                    // Note: Texture creation must happen on main thread
                    // We'll trigger a repaint and the texture will be created in the next frame
                    ctx_clone.request_repaint();
                }
            });
            
            // Stay in Library view - user must explicitly switch to Develop
        }

        // Background
        let bg_color = if response.hovered() {
            Theme::BG_HOVER
        } else {
            Theme::BG_SURFACE
        };
        ui.painter().rect_filled(rect, Theme::RADIUS_MD, bg_color);

        // Load and display thumbnail
        self.load_thumbnail_if_needed(photo, ctx);

        if let Some(texture) = self.thumbnail_cache.get(&photo.id) {
            let img_height = 110.0;
            let img_rect = Rect::from_min_size(
                rect.min + Vec2::new(Theme::SPACE_XS, Theme::SPACE_XS),
                Vec2::new(tile_width - Theme::SPACE_XS * 2.0, img_height),
            );

            // Calculate centered image position
            let texture_aspect = texture.size()[0] as f32 / texture.size()[1] as f32;
            let img_aspect = img_rect.width() / img_rect.height();

            let img_display_rect = if texture_aspect > img_aspect {
                // Wider than tall - fit width
                let display_height = img_rect.width() / texture_aspect;
                let y_offset = (img_rect.height() - display_height) / 2.0;
                Rect::from_min_size(
                    img_rect.min + Vec2::new(0.0, y_offset),
                    Vec2::new(img_rect.width(), display_height),
                )
            } else {
                // Taller than wide - fit height
                let display_width = img_rect.height() * texture_aspect;
                let x_offset = (img_rect.width() - display_width) / 2.0;
                Rect::from_min_size(
                    img_rect.min + Vec2::new(x_offset, 0.0),
                    Vec2::new(display_width, img_rect.height()),
                )
            };

            Image::new(texture).paint_at(ui, img_display_rect);
        }

        // File name
        let name_y = rect.min.y + 120.0;
        let name_rect = Rect::from_min_size(
            rect.min + Vec2::new(0.0, name_y),
            Vec2::new(tile_width, 20.0),
        );

        // Truncate name if too long
        let display_name = if photo.name.len() > 20 {
            format!("{}...", &photo.name[..17])
        } else {
            photo.name.clone()
        };

        ui.painter().text(
            name_rect.center(),
            egui::Align2::CENTER_CENTER,
            display_name,
            egui::FontId::proportional(Theme::FONT_XS),
            Theme::TEXT_MUTED,
        );

        // Hover overlay with rating
        if response.hovered() && photo.rating > 0 {
            let overlay_rect = Rect::from_min_size(
                rect.min + Vec2::new(0.0, 90.0),
                Vec2::new(tile_width, 20.0),
            );
            ui.painter().rect_filled(
                overlay_rect,
                0.0,
                Color32::from_rgba_premultiplied(0, 0, 0, 170),
            );

            // Draw rating stars
            let star_size = 12.0;
            let total_stars_width = star_size * 5.0;
            let start_x = overlay_rect.center().x - total_stars_width / 2.0;

            for i in 0..5 {
                let star_x = start_x + (i as f32 * star_size);
                let star_pos = egui::pos2(star_x + star_size / 2.0, overlay_rect.center().y);

                let color = if i < photo.rating as usize {
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
        }

        // Selection border
        if response.hovered() {
            ui.painter().rect_stroke(
                rect,
                Theme::RADIUS_MD,
                egui::Stroke::new(2.0, Theme::ACCENT_PRIMARY),
                egui::StrokeKind::Outside,
            );
        }
    }

    /// Load thumbnail texture if not already cached
    fn load_thumbnail_if_needed(&mut self, photo: &PhotoViewModel, ctx: &egui::Context) {
        if self.thumbnail_cache.contains_key(&photo.id) {
            return;
        }

        if let Some(thumb_path) = &photo.thumbnail_path {
            if let Ok(img) = image::open(thumb_path) {
                let texture = crate::image_processing::ImageProcessor::load_texture(
                    ctx,
                    format!("thumb_{}", photo.id),
                    &img,
                );
                self.thumbnail_cache.insert(photo.id.clone(), texture);
            }
        }
    }

    /// Show empty state when no photos are loaded
    fn show_empty_state(&self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            ui.label(egui::RichText::new("📷").size(Theme::FONT_DISPLAY));
            ui.label(
                egui::RichText::new("No photos in your catalog")
                    .size(Theme::FONT_XL)
                    .color(Theme::TEXT_MUTED),
            );
            ui.label(
                egui::RichText::new("Click Import to add photos")
                    .size(Theme::FONT_LG)
                    .color(Theme::TEXT_MUTED),
            );
        });
    }

    /// Clear the thumbnail cache (useful for memory management)
    pub fn clear_cache(&mut self) {
        self.thumbnail_cache.clear();
    }
}

impl Default for PhotoGrid {
    fn default() -> Self {
        Self::new()
    }
}
