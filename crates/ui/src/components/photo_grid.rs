#![allow(dead_code)]

// Photo Grid Component
// Displays photos in a 5-column grid layout with thumbnails
// Uses async thumbnail loading for smooth UI

use egui::{Ui, Vec2, Sense, Image, Rect, Color32};
use std::collections::HashMap;

use crate::state::AppState;
use crate::design_system::theme::Theme;
use crate::async_loader::{AsyncThumbnailLoader, ThumbnailRequest};
use crate::components::context_menu::{ContextMenu, ContextMenuItem};
use adapters::view_models::PhotoViewModel;
use std::sync::Arc;
use infrastructure::cache::preview_manager::PreviewManager;

pub struct PhotoGrid {
    /// Cache of loaded thumbnail textures
    thumbnail_cache: HashMap<String, egui::TextureHandle>,
    /// Track last selected photo to detect selection changes
    last_selected_id: Option<String>,
    /// Async thumbnail loader (Rayon-powered)
    thumbnail_loader: AsyncThumbnailLoader,
    /// Context menu for right-click actions
    context_menu: ContextMenu,
}

impl PhotoGrid {
    pub fn new(preview_manager: Arc<PreviewManager>) -> Self {
        Self {
            thumbnail_cache: HashMap::new(),
            last_selected_id: None,
            thumbnail_loader: AsyncThumbnailLoader::new(preview_manager),
            context_menu: ContextMenu::new("photo_grid_context"),
        }
    }

    /// Show the photo grid
    pub fn show(
        &mut self,
        ui: &mut Ui,
        state: &mut AppState,
        ctx: &egui::Context,
    ) {
        // Poll for completed thumbnails (non-blocking)
        let results = self.thumbnail_loader.poll_results();
        for result in results {
            // Find the photo's edit values to apply effects to thumbnail
            let processed_image = if let Some(photo) = state.photos.iter().find(|p| p.id == result.photo_id) {
                let exposure = photo.edit_exposure.unwrap_or(0.0);
                let contrast = photo.edit_contrast.unwrap_or(1.0);
                let temperature = photo.edit_temperature.unwrap_or(0.0);
                let tint = photo.edit_tint.unwrap_or(0.0);
                let highlights = photo.edit_highlights.unwrap_or(0.0);
                let shadows = photo.edit_shadows.unwrap_or(0.0);
                let whites = photo.edit_whites.unwrap_or(0.0);
                let blacks = photo.edit_blacks.unwrap_or(0.0);
                let clarity = photo.edit_clarity.unwrap_or(0.0);
                let vibrance = photo.edit_vibrance.unwrap_or(0.0);
                let saturation = photo.edit_saturation.unwrap_or(0.0);
                
                // Only apply effects if there are actual edits
                let has_edits = exposure != 0.0 || contrast != 1.0 || temperature != 0.0 || 
                               tint != 0.0 || saturation != 0.0 || vibrance != 0.0;
                
                if has_edits {
                    crate::image_processing::ImageProcessor::process_image(
                        &result.image, exposure, contrast, temperature, tint,
                        highlights, shadows, whites, blacks, clarity, vibrance, saturation
                    )
                } else {
                    result.image.clone()
                }
            } else {
                result.image.clone()
            };
            
            // Create texture from the image
            let resized_image = crate::image_processing::ImageProcessor::resize_for_preview(&processed_image, 300);
            
            let texture = crate::image_processing::ImageProcessor::load_texture(
                ctx,
                format!("grid_thumb_{}", result.photo_id),
                &resized_image
            );
            self.thumbnail_cache.insert(result.photo_id, texture);
            // Request repaint to show the newly cached thumbnail
            ctx.request_repaint();
        }

        // Request repaint if thumbnails are still loading
        if self.thumbnail_loader.loading_count() > 0 {
            ctx.request_repaint();
        }

        // Get filtered photos
        let filtered_photos = state.get_filtered_photos();

        if filtered_photos.is_empty() {
            self.show_empty_state(ui);
            return;
        }

        // Request thumbnails for visible photos (async, non-blocking)
        self.request_visible_thumbnails(&filtered_photos);

        // Check if selection changed (e.g., from filmstrip)
        let selection_changed = state.library_selected_photo_id != self.last_selected_id;
        self.last_selected_id = state.library_selected_photo_id.clone();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.show_grid(ui, state, ctx, &filtered_photos, selection_changed);
            });
        
        // Show context menu if open
        let menu_items = vec![
            ContextMenuItem::new("Open in Develop").with_icon("🖼").with_shortcut("Enter"),
            ContextMenuItem::new("---"),
            ContextMenuItem::new("Select All").with_shortcut("⌘A"),
            ContextMenuItem::new("Deselect All").with_shortcut("⌘D"),
            ContextMenuItem::new("---"),
            ContextMenuItem::new("Delete").with_icon("🗑").with_shortcut("⌫").destructive(),
        ];
        
        if let Some(clicked_index) = self.context_menu.show(ctx, &menu_items) {
            match clicked_index {
                0 => {
                    // Open in Develop
                    if let Some(photo_id) = state.library_selected_photo_id.clone() {
                        state.develop_selected_photo_id = Some(photo_id);
                        state.current_view = crate::state::CurrentView::Develop;
                        state.loaded_photo_id = None;
                    }
                }
                2 => state.select_all(),
                3 => state.clear_selection(),
                5 => {
                    if !state.selected_photo_ids.is_empty() {
                        state.show_delete_confirmation = true;
                    }
                }
                _ => {}
            }
        }
    }

    /// Request async loading of thumbnails for visible photos
    fn request_visible_thumbnails(&mut self, photos: &[PhotoViewModel]) {
        let requests: Vec<ThumbnailRequest> = photos
            .iter()
            .filter(|p| !self.thumbnail_cache.contains_key(&p.id))
            .map(|p| ThumbnailRequest {
                photo_id: p.id.clone(),
                path: p.path.clone(),
            })
            .take(20) // Limit batch size to avoid overwhelming
            .collect();

        if !requests.is_empty() {
            self.thumbnail_loader.request_thumbnails(requests);
        }
    }

    /// Show the grid of photo tiles
    fn show_grid(
        &mut self,
        ui: &mut Ui,
        state: &mut AppState,
        ctx: &egui::Context,
        photos: &[PhotoViewModel],
        selection_changed: bool,
    ) {
        let columns = state.grid_columns.max(1).min(5); // Clamp to 1-5
        let spacing = Theme::SPACE_SM;
        let available_width = ui.available_width();
        
        // Calculate tile size based on columns
        let tile_width = if columns == 1 {
            // Single column: use full width with max height
            available_width - spacing * 2.0
        } else {
            (available_width - (spacing * (columns - 1) as f32)) / columns as f32
        };
        
        // Adjust tile height based on column count
        let tile_height = if columns == 1 {
            (ui.available_height() - 100.0).max(300.0) // Full view mode
        } else {
            Theme::TILE_HEIGHT
        };

        ui.spacing_mut().item_spacing = Vec2::new(spacing, spacing);

        // Layout photos in rows
        let mut global_index = 0usize;
        for chunk in photos.chunks(columns) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = spacing;

                for photo in chunk {
                    // Check if this is the selected photo (in Library view)
                    let is_selected = state.library_selected_photo_id.as_ref() == Some(&photo.id);
                    
                    // Show the tile with index for multi-selection
                    self.show_tile_with_height(ui, photo, state, ctx, tile_width, tile_height, global_index);
                    
                    // Auto-scroll to selected photo when selection changes
                    if is_selected && selection_changed {
                        ui.scroll_to_cursor(Some(egui::Align::Center));
                    }
                    global_index += 1;
                }
            });
        }
    }

    fn show_tile_with_height(
        &mut self,
        ui: &mut Ui,
        photo: &PhotoViewModel,
        state: &mut AppState,
        _ctx: &egui::Context,  // Kept for API compatibility
        tile_width: f32,
        tile_height: f32,
        index: usize,
    ) {
        let tile_size = Vec2::new(tile_width, tile_height);

        let (rect, response) = ui.allocate_exact_size(tile_size, Sense::click());

        // Handle right-click - open context menu
        if response.secondary_clicked() {
            // If right-clicking on a non-selected photo, select it first
            if !state.is_photo_selected(&photo.id) {
                state.single_select(&photo.id, index);
            }
            self.context_menu.check_open(&response);
        }

        // Handle left click with modifier keys for multi-selection
        if response.clicked() {
            let modifiers = ui.input(|i| i.modifiers);
            
            if modifiers.command {
                // Cmd+click: toggle individual selection
                state.toggle_selection(&photo.id);
                state.last_clicked_index = Some(index);
            } else if modifiers.shift {
                // Shift+click: range selection
                state.select_range(index);
            } else {
                // Regular click: single select
                state.single_select(&photo.id, index);
            }
            
            // Populate metadata for detail view (for the primary selection)
            state.library_selected_photo_id = Some(photo.id.clone());
            state.detail_metadata = Some(crate::state::DetailMetadata {
                id: photo.id.clone(),
                name: photo.name.clone(),
                date: photo.date.clone(),
                camera: photo.camera.clone(),
                exposure: photo.exposure.clone(),
                rating: photo.rating,
                color_label: photo.color_label.clone(),
            });
        }
        
        // Check if photo is in multi-selection OR is the primary selection
        let is_multi_selected = state.is_photo_selected(&photo.id);
        let is_primary_selected = state.library_selected_photo_id.as_ref() == Some(&photo.id);
        let is_selected = is_multi_selected || is_primary_selected;
        
        // Background - highlight selected photo
        let bg_color = if is_selected {
            Theme::BG_ACTIVE
        } else if response.hovered() {
            Theme::BG_HOVER
        } else {
            Theme::BG_SURFACE
        };
        ui.painter().rect_filled(rect, Theme::RADIUS_MD, bg_color);

        // Display thumbnail (loaded asynchronously)

        if let Some(texture) = self.thumbnail_cache.get(&photo.id) {
            // Calculate image area - leave space for name at bottom (proportional to tile height)
            let name_space = (tile_height * 0.15).max(25.0).min(40.0);
            let img_height = tile_height - name_space - Theme::SPACE_XS * 2.0;
            
            let img_rect = Rect::from_min_size(
                rect.min + Vec2::new(Theme::SPACE_XS, Theme::SPACE_XS),
                Vec2::new(tile_width - Theme::SPACE_XS * 2.0, img_height),
            );

            // Calculate centered image position preserving aspect ratio
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

        // File name - positioned at bottom of tile
        let name_height = 25.0;
        let name_rect = Rect::from_min_size(
            rect.min + Vec2::new(0.0, tile_height - name_height),
            Vec2::new(tile_width, name_height),
        );

        // Truncate name based on tile width
        let max_chars = ((tile_width / 8.0) as usize).max(5);
        let display_name = if photo.name.len() > max_chars {
            format!("{}...", &photo.name[..(max_chars - 3)])
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

        // Check if file exists and show warning triangle if missing
        let file_exists = std::path::Path::new(&photo.path).exists();
        if !file_exists {
            // Log to console
            if response.hovered() {
                eprintln!("⚠️ File not found: {}", photo.path);
            }
            
            // Draw warning triangle
            let warning_pos = rect.min + Vec2::new(8.0, 8.0);
            
            // Background circle
            ui.painter().circle_filled(
                warning_pos + Vec2::new(10.0, 10.0),
                14.0,
                Color32::from_rgb(255, 100, 50),
            );
            
            // Warning icon
            ui.painter().text(
                warning_pos + Vec2::new(10.0, 10.0),
                egui::Align2::CENTER_CENTER,
                "⚠",
                egui::FontId::proportional(16.0),
                Color32::WHITE,
            );
        }

        // Selection border - show for selected OR hovered
        // Different border for multi-selection vs primary selection
        if is_primary_selected {
            ui.painter().rect_stroke(
                rect,
                Theme::RADIUS_MD,
                egui::Stroke::new(3.0, egui::Color32::WHITE),
                egui::StrokeKind::Outside,
            );
        } else if is_multi_selected {
            ui.painter().rect_stroke(
                rect,
                Theme::RADIUS_MD,
                egui::Stroke::new(2.0, Theme::ACCENT_PRIMARY),
                egui::StrokeKind::Outside,
            );
        } else if response.hovered() {
            ui.painter().rect_stroke(
                rect,
                Theme::RADIUS_MD,
                egui::Stroke::new(2.0, Theme::ACCENT_PRIMARY),
                egui::StrokeKind::Outside,
            );
        }
        
        // Show checkmark for multi-selected items
        if is_multi_selected && state.selection_count() > 1 {
            let check_pos = rect.min + Vec2::new(8.0, 8.0);
            ui.painter().circle_filled(check_pos, 10.0, Theme::ACCENT_PRIMARY);
            ui.painter().text(
                check_pos,
                egui::Align2::CENTER_CENTER,
                "✓",
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );
        }
    }

    // Note: Thumbnail loading is now handled asynchronously by request_visible_thumbnails()

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

// Default implementation removed because PreviewManager is required
// impl Default for PhotoGrid { ... }
