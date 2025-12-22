// Filmstrip Component
// Horizontal thumbnail navigation bar similar to Lightroom
// Uses async thumbnail loading for smooth UI

use egui::{Ui, Vec2, Sense, Color32, Stroke, CornerRadius, Image};
use std::collections::HashMap;
use adapters::view_models::PhotoViewModel;
use crate::design_system::theme::Theme;
use crate::async_loader::{AsyncThumbnailLoader, ThumbnailRequest};
use crate::state::AppState;
use crate::components::context_menu::{ContextMenu, ContextMenuItem};
use std::sync::Arc;
use infrastructure::cache::preview_manager::PreviewManager;

/// Represents an action triggered by the context menu
#[derive(Clone, Debug, PartialEq)]
pub enum FilmstripAction {
    OpenInDevelop,
    Export,
    Delete,
    SelectAll,
    DeselectAll,
    SetFlag(String, i32),
}

pub struct Filmstrip {
    /// Cache of loaded thumbnail textures
    thumbnail_cache: HashMap<String, egui::TextureHandle>,
    /// Async thumbnail loader (Rayon-powered)
    thumbnail_loader: AsyncThumbnailLoader,
    /// Context menu for right-click actions
    context_menu: ContextMenu,
}

impl Filmstrip {
    const THUMBNAIL_SIZE: f32 = 80.0;
    const THUMBNAIL_SPACING: f32 = 4.0;
    const SELECTED_BORDER_WIDTH: f32 = 3.0;

    pub fn new(preview_manager: Arc<PreviewManager>) -> Self {
        Self {
            thumbnail_cache: HashMap::new(),
            thumbnail_loader: AsyncThumbnailLoader::new(preview_manager),
            context_menu: ContextMenu::new("filmstrip_context_menu"),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        photos: &[PhotoViewModel],
        state: &mut AppState,
    ) -> Option<FilmstripAction> {
        let mut action: Option<FilmstripAction> = None;
        // Poll for completed thumbnails (non-blocking)
        let results = self.thumbnail_loader.poll_results();
        for result in results {
            // Find the photo's edit values to apply effects to thumbnail
            let processed_image = if let Some(photo) = photos.iter().find(|p| p.id == result.photo_id) {
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
            
            let texture = crate::image_processing::ImageProcessor::load_texture(
                ctx,
                format!("filmstrip_thumb_{}", result.photo_id),
                &processed_image
            );
            self.thumbnail_cache.insert(result.photo_id, texture);
        }

        // Request repaint if thumbnails are still loading
        if self.thumbnail_loader.loading_count() > 0 {
            ctx.request_repaint();
        }

        // Request thumbnails for visible photos (async, non-blocking)
        self.request_visible_thumbnails(photos);

        // Dark background like Lightroom
        let bg_color = Color32::from_rgb(42, 42, 42);
        ui.painter().rect_filled(
            ui.available_rect_before_wrap(),
            CornerRadius::ZERO,
            bg_color,
        );

        // Horizontal scroll area
        egui::ScrollArea::horizontal()
            .id_salt("filmstrip_scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(Theme::SPACE_SM);

                    for (index, photo) in photos.iter().enumerate() {
                        let is_multi_selected = state.is_photo_selected(&photo.id);
                        let is_primary_selected = state.library_selected_photo_id.as_ref() == Some(&photo.id);
                        
                        // Reserve space for thumbnail
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(Self::THUMBNAIL_SIZE, Self::THUMBNAIL_SIZE),
                            Sense::click(),
                        );

                        // Handle click with modifier keys for multi-selection
                        // Note: We check this AFTER flags to allow flags to steal clicks if needed
                        let mut thumb_clicked = response.clicked();

                        // Interactive Flags (Pick/Reject) - Pre-calculation & Interaction
                        // We handle interaction here to intercept clicks, but draw later to be on top
                        let current_flag = photo.flag.unwrap_or(0);
                        let is_hovered = response.hovered();
                        let show_flags = is_hovered || current_flag != 0;
                        
                        let mut pick_rect = egui::Rect::NOTHING;
                        let mut reject_rect = egui::Rect::NOTHING;
                        let mut pick_hovered = false;
                        let mut reject_hovered = false;

                        if show_flags {
                            let flag_size = 14.0;
                            let padding = 4.0;
                            let spacing = 2.0;

                            // Pick Icon [P]
                            pick_rect = egui::Rect::from_min_size(
                                rect.min + Vec2::new(padding, padding),
                                Vec2::new(flag_size, flag_size)
                            );
                            
                            // Reject Icon [X]
                            reject_rect = egui::Rect::from_min_size(
                                rect.min + Vec2::new(padding + flag_size + spacing, padding),
                                Vec2::new(flag_size, flag_size)
                            );

                            let pick_response = ui.interact(pick_rect, egui::Id::new(format!("pick_{}", photo.id)), Sense::click());
                            let reject_response = ui.interact(reject_rect, egui::Id::new(format!("reject_{}", photo.id)), Sense::click());
                            
                            pick_hovered = pick_response.hovered();
                            reject_hovered = reject_response.hovered();

                            // Handle Flag Clicks
                            if pick_response.clicked() {
                                thumb_clicked = false; // Consume click
                                let new_flag = if current_flag == 1 { 0 } else { 1 }; // Toggle
                                action = Some(FilmstripAction::SetFlag(photo.id.clone(), new_flag));
                            } else if reject_response.clicked() {
                                thumb_clicked = false; // Consume click
                                let new_flag = if current_flag == -1 { 0 } else { -1 }; // Toggle
                                action = Some(FilmstripAction::SetFlag(photo.id.clone(), new_flag));
                            }
                        }

                        if thumb_clicked {
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
                            state.library_selected_photo_id = Some(photo.id.clone());
                        }

                        // Handle right-click or Control+click (macOS trackpad) - open context menu
                        let is_control_click = response.clicked() && ui.input(|i| i.modifiers.ctrl);
                        if response.secondary_clicked() || is_control_click {
                            // If right-clicking on a non-selected photo, select it first
                            if !state.is_photo_selected(&photo.id) {
                                state.single_select(&photo.id, index);
                            }
                            state.library_selected_photo_id = Some(photo.id.clone());
                            // Open context menu at click position
                            if let Some(pos) = response.interact_pointer_pos() {
                                self.context_menu.open(ctx, pos);
                            }
                        }

                        // Draw thumbnail background
                        let thumb_color = if is_multi_selected || is_primary_selected {
                            Color32::from_rgb(70, 70, 70)
                        } else if response.hovered() {
                            Color32::from_rgb(60, 60, 60)
                        } else {
                            Color32::from_rgb(50, 50, 50)
                        };
                        
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(2),
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

                        // Draw Rating Stars
                        if photo.rating > 0 {
                            let star_size = 10.0;
                            let total_stars_width = star_size * 5.0;
                            let start_x = rect.center().x - total_stars_width / 2.0;
                            let star_y = rect.max.y - star_size; // Bottom

                            // Draw subtle background
                            let bg_rect = egui::Rect::from_min_size(
                                egui::pos2(start_x - 2.0, star_y - star_size/2.0),
                                egui::Vec2::new(total_stars_width + 4.0, star_size)
                            );
                            ui.painter().rect_filled(bg_rect, 4.0, Color32::from_black_alpha(100));

                            for i in 0..5 {
                                let star_x = start_x + (i as f32 * star_size);
                                let star_pos = egui::pos2(star_x + star_size / 2.0, star_y);
                                let color = if i < photo.rating as usize { Theme::RATING_ACTIVE } else { Color32::from_gray(80) };
                                ui.painter().text(star_pos, egui::Align2::CENTER_CENTER, "★", egui::FontId::proportional(star_size), color);
                            }
                        }

                        // Draw selection border
                        if is_primary_selected {
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(2),
                                Stroke::new(Self::SELECTED_BORDER_WIDTH, Color32::WHITE),
                                egui::StrokeKind::Outside,
                            );
                        } else if is_multi_selected {
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(2),
                                Stroke::new(2.0, Theme::ACCENT_PRIMARY),
                                egui::StrokeKind::Outside,
                            );
                        }
                        
                        // Show checkmark for multi-selected items
                        if is_multi_selected && state.selection_count() > 1 {
                            let check_pos = rect.min + Vec2::new(6.0, 6.0);
                            ui.painter().circle_filled(check_pos, 8.0, Theme::ACCENT_PRIMARY);
                            ui.painter().text(
                                check_pos,
                                egui::Align2::CENTER_CENTER,
                                "✓",
                                egui::FontId::proportional(10.0),
                                Color32::WHITE,
                            );
                        }

                        // Draw Interactive Flags (Last layer)
                        if show_flags {
                            let flag_size = 14.0;
                            // Draw Pick Icon
                            let pick_bg = if current_flag == 1 { Theme::ACCENT_SUCCESS } else if pick_hovered { Color32::from_gray(100) } else { Color32::from_black_alpha(100) };
                            let pick_fg = if current_flag == 1 { Color32::WHITE } else { Color32::from_gray(200) };
                            
                            ui.painter().circle_filled(pick_rect.center(), flag_size / 2.0, pick_bg);
                            ui.painter().text(
                                pick_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "P",
                                egui::FontId::proportional(9.0),
                                pick_fg,
                            );

                            // Draw Reject Icon
                            let reject_bg = if current_flag == -1 { Theme::ACCENT_ERROR } else if reject_hovered { Color32::from_gray(100) } else { Color32::from_black_alpha(100) };
                            let reject_fg = if current_flag == -1 { Color32::WHITE } else { Color32::from_gray(200) };
                            
                            ui.painter().circle_filled(reject_rect.center(), flag_size / 2.0, reject_bg);
                            ui.painter().text(
                                reject_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "X", 
                                egui::FontId::proportional(9.0),
                                reject_fg,
                            );
                        }

                        // Spacing between thumbnails
                        ui.add_space(Self::THUMBNAIL_SPACING);
                    }

                    ui.add_space(Theme::SPACE_SM);
                });
            });

        // Show context menu if open
        let selection_count = state.selection_count();
        let items = vec![
            ContextMenuItem::new("Open in Develop")
                .with_icon("🖼️")
                .with_shortcut("Enter"),
            ContextMenuItem::new("Export JPEG...")
                .with_icon("📤")
                .with_shortcut("⌘E"),
            ContextMenuItem::new("---"),  // Separator
            ContextMenuItem::new("Select All")
                .with_icon("☑️")
                .with_shortcut("⌘A"),
            ContextMenuItem::new("Deselect All")
                .with_icon("☐")
                .with_shortcut("⌘D"),
            ContextMenuItem::new("---"),  // Separator
            ContextMenuItem::new(&format!("Delete {} Photo{}", selection_count, if selection_count == 1 { "" } else { "s" }))
                .with_icon("🗑️")
                .with_shortcut("⌫")
                .destructive(),
        ];

        if let Some(clicked_index) = self.context_menu.show(ctx, &items) {
            action = match clicked_index {
                0 => Some(FilmstripAction::OpenInDevelop),
                1 => Some(FilmstripAction::Export),
                3 => Some(FilmstripAction::SelectAll),
                4 => Some(FilmstripAction::DeselectAll),
                6 => Some(FilmstripAction::Delete),
                _ => None,
            };
        }

        action
    }
    
    /// Show filmstrip for Develop view (single selection mode with callback)
    pub fn show_develop(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        photos: &[PhotoViewModel],
        selected_photo_id: &Option<String>,
        mut on_select: impl FnMut(String),
        mut on_flag: impl FnMut(String, i32),
    ) {
        // Poll for completed thumbnails (non-blocking)
        let results = self.thumbnail_loader.poll_results();
        for result in results {
            // Find the photo's edit values to apply effects to thumbnail
            let processed_image = if let Some(photo) = photos.iter().find(|p| p.id == result.photo_id) {
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
            
            let texture = crate::image_processing::ImageProcessor::load_texture(
                ctx,
                format!("filmstrip_thumb_{}", result.photo_id),
                &processed_image
            );
            self.thumbnail_cache.insert(result.photo_id, texture);
        }

        // Request repaint if thumbnails are still loading
        if self.thumbnail_loader.loading_count() > 0 {
            ctx.request_repaint();
        }

        // Request thumbnails for visible photos (async, non-blocking)
        self.request_visible_thumbnails(photos);

        // Dark background like Lightroom
        let bg_color = Color32::from_rgb(42, 42, 42);
        ui.painter().rect_filled(
            ui.available_rect_before_wrap(),
            CornerRadius::ZERO,
            bg_color,
        );

        // Horizontal scroll area
        egui::ScrollArea::horizontal()
            .id_salt("filmstrip_scroll_develop")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(Theme::SPACE_SM);

                    for photo in photos {
                        let is_selected = selected_photo_id.as_ref() == Some(&photo.id);
                        
                        // Reserve space for thumbnail
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(Self::THUMBNAIL_SIZE, Self::THUMBNAIL_SIZE),
                            Sense::click(),
                        );

                        // Draw thumbnail background
                        let thumb_color = if is_selected {
                            Color32::from_rgb(70, 70, 70)
                        } else if response.hovered() {
                            Color32::from_rgb(60, 60, 60)
                        } else {
                            Color32::from_rgb(50, 50, 50)
                        };
                        
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(2),
                            thumb_color,
                        );

                        // Draw thumbnail image or placeholder
                        if let Some(texture) = self.thumbnail_cache.get(&photo.id) {
                            let img_rect = rect.shrink(2.0);
                            let texture_aspect = texture.size()[0] as f32 / texture.size()[1] as f32;
                            let img_aspect = img_rect.width() / img_rect.height();
                            
                            let img_display_rect = if texture_aspect > img_aspect {
                                let display_height = img_rect.width() / texture_aspect;
                                let y_offset = (img_rect.height() - display_height) / 2.0;
                                egui::Rect::from_min_size(
                                    img_rect.min + Vec2::new(0.0, y_offset),
                                    Vec2::new(img_rect.width(), display_height),
                                )
                            } else {
                                let display_width = img_rect.height() * texture_aspect;
                                let x_offset = (img_rect.width() - display_width) / 2.0;
                                egui::Rect::from_min_size(
                                    img_rect.min + Vec2::new(x_offset, 0.0),
                                    Vec2::new(display_width, img_rect.height()),
                                )
                            };
                            
                            Image::new(texture).paint_at(ui, img_display_rect);
                        } else {
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

                        // Draw Rating Stars
                        if photo.rating > 0 {
                            let star_size = 10.0;
                            let total_stars_width = star_size * 5.0;
                            let start_x = rect.center().x - total_stars_width / 2.0;
                            let star_y = rect.max.y - star_size;

                            // Draw subtle background
                            let bg_rect = egui::Rect::from_min_size(
                                egui::pos2(start_x - 2.0, star_y - star_size/2.0),
                                egui::Vec2::new(total_stars_width + 4.0, star_size)
                            );
                            ui.painter().rect_filled(bg_rect, 4.0, Color32::from_black_alpha(100));

                            for i in 0..5 {
                                let star_x = start_x + (i as f32 * star_size);
                                let star_pos = egui::pos2(star_x + star_size / 2.0, star_y);
                                let color = if i < photo.rating as usize { Theme::RATING_ACTIVE } else { Color32::from_gray(80) };
                                ui.painter().text(star_pos, egui::Align2::CENTER_CENTER, "★", egui::FontId::proportional(star_size), color);
                            }
                        }

                        // Draw selected border
                        if is_selected {
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(2),
                                Stroke::new(Self::SELECTED_BORDER_WIDTH, Color32::WHITE),
                                egui::StrokeKind::Outside,
                            );
                        }

                        // Interactive Flags (Pick/Reject) - Top Left
                        // Always show if set, or show ghosts if hovered
                        let is_hovered = response.hovered();
                        let current_flag = photo.flag.unwrap_or(0);
                        let mut thumb_clicked = response.clicked();

                        if is_hovered || current_flag != 0 {
                            let flag_size = 14.0;
                            let padding = 4.0;
                            let spacing = 2.0;

                            // Pick Icon [P]
                            let pick_rect = egui::Rect::from_min_size(
                                rect.min + Vec2::new(padding, padding),
                                Vec2::new(flag_size, flag_size)
                            );
                            
                            // Reject Icon [X]
                            let reject_rect = egui::Rect::from_min_size(
                                rect.min + Vec2::new(padding + flag_size + spacing, padding),
                                Vec2::new(flag_size, flag_size)
                            );

                            let pick_response = ui.interact(pick_rect, egui::Id::new(format!("dv_pick_{}", photo.id)), Sense::click());
                            let reject_response = ui.interact(reject_rect, egui::Id::new(format!("dv_reject_{}", photo.id)), Sense::click());
                            
                            // Handle Flag Clicks
                            if pick_response.clicked() {
                                thumb_clicked = false; // Consume click
                                let new_flag = if current_flag == 1 { 0 } else { 1 }; // Toggle
                                on_flag(photo.id.clone(), new_flag);
                            } else if reject_response.clicked() {
                                thumb_clicked = false; // Consume click
                                let new_flag = if current_flag == -1 { 0 } else { -1 }; // Toggle
                                on_flag(photo.id.clone(), new_flag);
                            }

                            // Draw Pick Icon
                            let pick_bg = if current_flag == 1 { Theme::ACCENT_SUCCESS } else if pick_response.hovered() { Color32::from_gray(100) } else { Color32::from_black_alpha(100) };
                            let pick_fg = if current_flag == 1 { Color32::WHITE } else { Color32::from_gray(200) };
                            
                            ui.painter().circle_filled(pick_rect.center(), flag_size / 2.0, pick_bg);
                            ui.painter().text(
                                pick_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "P",
                                egui::FontId::proportional(9.0),
                                pick_fg,
                            );

                            // Draw Reject Icon
                            let reject_bg = if current_flag == -1 { Theme::ACCENT_ERROR } else if reject_response.hovered() { Color32::from_gray(100) } else { Color32::from_black_alpha(100) };
                            let reject_fg = if current_flag == -1 { Color32::WHITE } else { Color32::from_gray(200) };
                            
                            ui.painter().circle_filled(reject_rect.center(), flag_size / 2.0, reject_bg);
                            ui.painter().text(
                                reject_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "X", 
                                egui::FontId::proportional(9.0),
                                reject_fg,
                            );
                        }

                        // Handle click
                        if thumb_clicked {
                             on_select(photo.id.clone());
                        }

                        ui.add_space(Self::THUMBNAIL_SPACING);
                    }

                    ui.add_space(Theme::SPACE_SM);
                });
            });
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
            .take(30) // Limit batch size
            .collect();

        if !requests.is_empty() {
            self.thumbnail_loader.request_thumbnails(requests);
        }
    }

    // Note: Thumbnail loading is now handled asynchronously by request_visible_thumbnails()

    /// Clear the thumbnail cache
    #[allow(dead_code)]
    pub fn clear_cache(&mut self) {
        self.thumbnail_cache.clear();
        self.thumbnail_loader.clear_requested();
    }

    /// Invalidate a specific photo's cached thumbnail (forces regeneration)
    pub fn invalidate_thumbnail(&mut self, photo_id: &str) {
        self.thumbnail_cache.remove(photo_id);
        // Also clear from requested set so it can be re-requested
        self.thumbnail_loader.clear_requested_for(photo_id);
    }
}

// Default implementation removed because PreviewManager is required
// impl Default for Filmstrip { ... }
