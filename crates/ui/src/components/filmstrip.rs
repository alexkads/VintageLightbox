// Filmstrip Component
// Horizontal thumbnail navigation bar similar to Lightroom

use egui::{Ui, Vec2, Sense, Color32, Stroke, Rounding, Image};
use std::collections::HashMap;
use adapters::view_models::PhotoViewModel;
use crate::design_system::theme::Theme;

pub struct Filmstrip {
    /// Cache of loaded thumbnail textures
    thumbnail_cache: HashMap<String, egui::TextureHandle>,
}

impl Filmstrip {
    const THUMBNAIL_SIZE: f32 = 80.0;
    const THUMBNAIL_SPACING: f32 = 4.0;
    const SELECTED_BORDER_WIDTH: f32 = 3.0;

    pub fn new() -> Self {
        Self {
            thumbnail_cache: HashMap::new(),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        photos: &[PhotoViewModel],
        selected_photo_id: &Option<String>,
        mut on_select: impl FnMut(String),
    ) {
        // Dark background like Lightroom
        let bg_color = Color32::from_rgb(42, 42, 42);
        ui.painter().rect_filled(
            ui.available_rect_before_wrap(),
            Rounding::ZERO,
            bg_color,
        );

        // Horizontal scroll area
        egui::ScrollArea::horizontal()
            .id_salt("filmstrip_scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(Theme::SPACE_SM);

                    for photo in photos {
                        let is_selected = selected_photo_id.as_ref() == Some(&photo.id);
                        
                        // Load thumbnail if needed
                        self.load_thumbnail_if_needed(photo, ctx);
                        
                        // Reserve space for thumbnail
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(Self::THUMBNAIL_SIZE, Self::THUMBNAIL_SIZE),
                            Sense::click(),
                        );

                        // Draw thumbnail background
                        let thumb_color = if response.hovered() {
                            Color32::from_rgb(60, 60, 60)
                        } else {
                            Color32::from_rgb(50, 50, 50)
                        };
                        
                        ui.painter().rect_filled(
                            rect,
                            Rounding::same(2),
                            thumb_color,
                        );

                        // Draw thumbnail image or placeholder
                        if let Some(texture) = self.thumbnail_cache.get(&photo.id) {
                            // Draw actual thumbnail
                            let img_rect = rect.shrink(2.0); // Small padding
                            
                            // Calculate centered image position preserving aspect ratio
                            let texture_aspect = texture.size()[0] as f32 / texture.size()[1] as f32;
                            let img_aspect = img_rect.width() / img_rect.height();
                            
                            let img_display_rect = if texture_aspect > img_aspect {
                                // Wider than tall - fit width
                                let display_height = img_rect.width() / texture_aspect;
                                let y_offset = (img_rect.height() - display_height) / 2.0;
                                egui::Rect::from_min_size(
                                    img_rect.min + Vec2::new(0.0, y_offset),
                                    Vec2::new(img_rect.width(), display_height),
                                )
                            } else {
                                // Taller than wide - fit height
                                let display_width = img_rect.height() * texture_aspect;
                                let x_offset = (img_rect.width() - display_width) / 2.0;
                                egui::Rect::from_min_size(
                                    img_rect.min + Vec2::new(x_offset, 0.0),
                                    Vec2::new(display_width, img_rect.height()),
                                )
                            };
                            
                            Image::new(texture).paint_at(ui, img_display_rect);
                        } else {
                            // Draw placeholder text
                            let text_pos = rect.center();
                            let short_name = if photo.name.len() > 8 {
                                format!("{}...", &photo.name[..5])
                            } else {
                                photo.name.clone()
                            };
                            ui.painter().text(
                                text_pos,
                                egui::Align2::CENTER_CENTER,
                                short_name,
                                egui::FontId::proportional(9.0),
                                Color32::from_rgb(120, 120, 120),
                            );
                        }

                        // Draw selected border
                        if is_selected {
                            ui.painter().rect_stroke(
                                rect,
                                Rounding::same(2),
                                Stroke::new(Self::SELECTED_BORDER_WIDTH, Color32::WHITE),
                                egui::StrokeKind::Outside,
                            );
                        }

                        // Handle click
                        if response.clicked() {
                            on_select(photo.id.clone());
                        }

                        // Spacing between thumbnails
                        ui.add_space(Self::THUMBNAIL_SPACING);
                    }

                    ui.add_space(Theme::SPACE_SM);
                });
            });
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
                    format!("filmstrip_thumb_{}", photo.id),
                    &img,
                );
                self.thumbnail_cache.insert(photo.id.clone(), texture);
            }
        }
    }

    /// Clear the thumbnail cache
    pub fn clear_cache(&mut self) {
        self.thumbnail_cache.clear();
    }
}

impl Default for Filmstrip {
    fn default() -> Self {
        Self::new()
    }
}
