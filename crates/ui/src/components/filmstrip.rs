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
    #[allow(dead_code)]
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
        state: &mut AppState,
    ) -> Option<FilmstripAction> {
        let mut action: Option<FilmstripAction> = None;

        // Render Filter Toolbox
        ui.horizontal(|ui| {
            ui.add_space(Theme::SPACE_SM);
            state.filmstrip_filter.ui(ui);
        });
        ui.separator();

        // Note: apply returns a list of references derived from state.photos
        // This locks state.photos for reading, preventing us from calling methods on state that borrow it mutably.
        // We must access disjoint fields (like selected_photo_ids) directly.
        let visible_photos = state.filmstrip_filter.apply(&state.photos);

        // Poll for completed thumbnails (non-blocking)
        let results = self.thumbnail_loader.poll_results();
        for result in results {
            // Find the photo's edit values to apply effects to thumbnail
            // Use state.photos to find edits (we need to find by ID)
            // Since visible_photos is a subset, checking state.photos is more robust/correct if edits updated
            // But visible_photos refs point to state.photos anyway.
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
                
                // Tone Curve
                let tone_curve_shadows = photo.edit_tone_curve_shadows.unwrap_or(0.0);
                let tone_curve_darks = photo.edit_tone_curve_darks.unwrap_or(0.0);
                let tone_curve_lights = photo.edit_tone_curve_lights.unwrap_or(0.0);
                let tone_curve_highlights = photo.edit_tone_curve_highlights.unwrap_or(0.0);
                
                // HSL Saturation
                let hsl_red_sat = photo.edit_hsl_red_sat.unwrap_or(0.0);
                let hsl_orange_sat = photo.edit_hsl_orange_sat.unwrap_or(0.0);
                let hsl_yellow_sat = photo.edit_hsl_yellow_sat.unwrap_or(0.0);
                let hsl_green_sat = photo.edit_hsl_green_sat.unwrap_or(0.0);
                let hsl_aqua_sat = photo.edit_hsl_aqua_sat.unwrap_or(0.0);
                let hsl_blue_sat = photo.edit_hsl_blue_sat.unwrap_or(0.0);
                let hsl_purple_sat = photo.edit_hsl_purple_sat.unwrap_or(0.0);
                let hsl_magenta_sat = photo.edit_hsl_magenta_sat.unwrap_or(0.0);
                // HSL Hue
                let hsl_red_hue = photo.edit_hsl_red_hue.unwrap_or(0.0);
                let hsl_orange_hue = photo.edit_hsl_orange_hue.unwrap_or(0.0);
                let hsl_yellow_hue = photo.edit_hsl_yellow_hue.unwrap_or(0.0);
                let hsl_green_hue = photo.edit_hsl_green_hue.unwrap_or(0.0);
                let hsl_aqua_hue = photo.edit_hsl_aqua_hue.unwrap_or(0.0);
                let hsl_blue_hue = photo.edit_hsl_blue_hue.unwrap_or(0.0);
                let hsl_purple_hue = photo.edit_hsl_purple_hue.unwrap_or(0.0);
                let hsl_magenta_hue = photo.edit_hsl_magenta_hue.unwrap_or(0.0);
                // HSL Lum
                let hsl_red_lum = photo.edit_hsl_red_lum.unwrap_or(0.0);
                let hsl_orange_lum = photo.edit_hsl_orange_lum.unwrap_or(0.0);
                let hsl_yellow_lum = photo.edit_hsl_yellow_lum.unwrap_or(0.0);
                let hsl_green_lum = photo.edit_hsl_green_lum.unwrap_or(0.0);
                let hsl_aqua_lum = photo.edit_hsl_aqua_lum.unwrap_or(0.0);
                let hsl_blue_lum = photo.edit_hsl_blue_lum.unwrap_or(0.0);
                let hsl_purple_lum = photo.edit_hsl_purple_lum.unwrap_or(0.0);
                let hsl_magenta_lum = photo.edit_hsl_magenta_lum.unwrap_or(0.0);
                // Lens
                let lens_distortion = photo.edit_lens_distortion.unwrap_or(0.0);
                let lens_vignette_amount = photo.edit_lens_vignette_amount.unwrap_or(0.0);
                let lens_vignette_midpoint = photo.edit_lens_vignette_midpoint.unwrap_or(0.0);
                // NR
                let nr_luminance = photo.edit_nr_luminance.unwrap_or(0.0);
                let nr_color = photo.edit_nr_color.unwrap_or(0.0);
                // Sharpening
                let sharpen_amount = photo.edit_sharpen_amount.unwrap_or(0.0);
                let sharpen_radius = photo.edit_sharpen_radius.unwrap_or(1.0);
                
                // Only apply effects if there are actual edits
                let has_edits = exposure != 0.0 || contrast != 1.0 || temperature != 0.0 || 
                               tint != 0.0 || saturation != 0.0 || vibrance != 0.0;
                
                if has_edits {
                    crate::image_processing::ImageProcessor::process_image(
                        &result.image, exposure, contrast, temperature, tint,
                        highlights, shadows, whites, blacks, clarity, vibrance, saturation,
                        tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
                        hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
                        hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
                        // HSL Hue
                        hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue,
                        hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
                        // HSL Lum
                        hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum,
                        hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
                        // Lens
                        lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
                        // NR
                        nr_luminance, nr_color,
                        // Sharpening
                        sharpen_amount, sharpen_radius,
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
        let requests: Vec<ThumbnailRequest> = visible_photos
            .iter()
            .filter(|p| !self.thumbnail_cache.contains_key(&p.id))
            .map(|p| ThumbnailRequest {
                photo_id: p.id.clone(),
                path: p.path.clone(),
            })
            .take(30)
            .collect();

        if !requests.is_empty() {
            self.thumbnail_loader.request_thumbnails(requests);
        }

        // Dark background like Lightroom
        let bg_color = ui.visuals().panel_fill;
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

                    for (index, photo) in visible_photos.iter().enumerate() {
                        // Access fields directly to avoid borrow conflict
                        let is_multi_selected = state.selected_photo_ids.contains(&photo.id);
                        let is_primary_selected = state.library_selected_photo_id.as_ref() == Some(&photo.id);
                        
                        // Reserve space for thumbnail
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(Self::THUMBNAIL_SIZE, Self::THUMBNAIL_SIZE),
                            Sense::click(),
                        );

                        // Handle click with modifier keys for multi-selection
                        let mut thumb_clicked = response.clicked();

                        // Interactive Flags logic ...
                        let current_flag = photo.flag.unwrap_or(0);
                        let is_hovered = response.hovered();

                       // ... (Flag interaction code same as before) ...
                        let flag_size = 14.0;
                        let padding = 4.0;
                        let spacing = 2.0;

                        let pick_rect = egui::Rect::from_min_size(
                            rect.min + Vec2::new(padding, padding),
                            Vec2::new(flag_size, flag_size)
                        );
                        
                        let reject_rect = egui::Rect::from_min_size(
                            rect.min + Vec2::new(padding + flag_size + spacing, padding),
                            Vec2::new(flag_size, flag_size)
                        );

                        let pick_response = ui.interact(pick_rect, egui::Id::new(format!("pick_{}", photo.id)), Sense::click());
                        let reject_response = ui.interact(reject_rect, egui::Id::new(format!("reject_{}", photo.id)), Sense::click());
                        
                        let pick_hovered = pick_response.hovered();
                        let reject_hovered = reject_response.hovered();

                        if pick_response.clicked() {
                            thumb_clicked = false;
                            let new_flag = if current_flag == 1 { 0 } else { 1 };
                            action = Some(FilmstripAction::SetFlag(photo.id.clone(), new_flag));
                        } else if reject_response.clicked() {
                            thumb_clicked = false;
                            let new_flag = if current_flag == -1 { 0 } else { -1 };
                            action = Some(FilmstripAction::SetFlag(photo.id.clone(), new_flag));
                        }

                        let show_flags = is_hovered || current_flag != 0 || pick_hovered || reject_hovered;

                        if show_flags {
                            let painter = ui.painter();
                            let pick_color = if current_flag == 1 { Theme::ACCENT_SUCCESS } else if pick_hovered { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
                            painter.text(pick_rect.center(), egui::Align2::CENTER_CENTER, crate::design_system::icons::FLAG_PICK, egui::FontId::proportional(12.0), pick_color);

                            let reject_color = if current_flag == -1 { Theme::ACCENT_ERROR } else if reject_hovered { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
                            painter.text(reject_rect.center(), egui::Align2::CENTER_CENTER, crate::design_system::icons::FLAG_REJECT, egui::FontId::proportional(12.0), reject_color);
                        }


                        if thumb_clicked {
                            let modifiers = ui.input(|i| i.modifiers);
                            
                            if modifiers.command {
                                // Cmd+click: toggle individual selection
                                if state.selected_photo_ids.contains(&photo.id) {
                                    state.selected_photo_ids.remove(&photo.id);
                                } else {
                                    state.selected_photo_ids.insert(photo.id.clone());
                                }
                                state.last_clicked_index = Some(index); // Use visible index? Might be confusing if refiltered.
                            } else if modifiers.shift {
                                // Range selection in FILTERED view
                                // Find where the last selected photo is in the current visible list
                                if let Some(last_id) = &state.library_selected_photo_id {
                                    if let Some(start_idx) = visible_photos.iter().position(|p| &p.id == last_id) {
                                        let start = start_idx.min(index);
                                        let end = start_idx.max(index);
                                        
                                        state.selected_photo_ids.clear();
                                        // Select everything in between
                                        for i in start..=end {
                                            state.selected_photo_ids.insert(visible_photos[i].id.clone());
                                        }
                                    } else {
                                        // Last selected not visible, treating as single select
                                        state.selected_photo_ids.clear();
                                        state.selected_photo_ids.insert(photo.id.clone());
                                    }
                                } else {
                                     state.selected_photo_ids.clear();
                                     state.selected_photo_ids.insert(photo.id.clone());
                                }
                            } else {
                                // Regular click: single select
                                state.selected_photo_ids.clear();
                                state.selected_photo_ids.insert(photo.id.clone());
                                state.last_clicked_index = Some(index);
                            }
                            state.library_selected_photo_id = Some(photo.id.clone());
                        }

                        // Handle right-click
                        let is_control_click = response.clicked() && ui.input(|i| i.modifiers.ctrl);
                        if response.secondary_clicked() || is_control_click {
                            if !state.selected_photo_ids.contains(&photo.id) {
                                state.selected_photo_ids.clear();
                                state.selected_photo_ids.insert(photo.id.clone());
                                state.last_clicked_index = Some(index);
                            }
                            state.library_selected_photo_id = Some(photo.id.clone());
                            if let Some(pos) = response.interact_pointer_pos() {
                                self.context_menu.open(ctx, pos);
                            }
                        }

                        // ... (Rendering thumbnail, etc. same as before) ...
                        
                        // Initialize the color mapping for labels
                        let label_color = match photo.color_label.as_deref() {
                            Some("Red") | Some("red") => Some(Theme::LABEL_RED),
                            Some("Yellow") | Some("yellow") => Some(Theme::LABEL_YELLOW),
                            Some("Green") | Some("green") => Some(Theme::LABEL_GREEN),
                            Some("Blue") | Some("blue") => Some(Theme::LABEL_BLUE),
                            Some("Purple") | Some("purple") => Some(Theme::LABEL_PURPLE),
                            _ => None,
                        };

                        // Draw thumbnail background
                        let thumb_color = ui.visuals().widgets.inactive.bg_fill;
                        
                        let bg_fill = if let Some(color) = label_color {
                            if is_multi_selected || is_primary_selected {
                                color.gamma_multiply(0.4)
                            } else {
                                color.gamma_multiply(0.2)
                            }
                        } else {
                            thumb_color
                        };

                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(2),
                            bg_fill,
                        );

                        if let Some(color) = label_color {
                            ui.painter().rect_stroke(
                                rect.shrink(1.0),
                                CornerRadius::same(2),
                                Stroke::new(3.0, color),
                                egui::StrokeKind::Outside,
                            );
                        }

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

                        if is_primary_selected {
                            // Double border for primary selection
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(2),
                                Stroke::new(1.0, Color32::WHITE),
                                egui::StrokeKind::Inside,
                            );
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(2),
                                Stroke::new(3.0, ui.visuals().selection.bg_fill),
                                egui::StrokeKind::Outside,
                            );
                        } else if is_multi_selected {
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(2),
                                Stroke::new(2.0, ui.visuals().text_color()),
                                egui::StrokeKind::Outside,
                            );
                        }
                        
                        if is_multi_selected && state.selected_photo_ids.len() > 1 {
                            let check_pos = rect.min + Vec2::new(6.0, 6.0);
                            ui.painter().circle_filled(check_pos, 8.0, ui.visuals().text_color());
                            ui.painter().text(
                                check_pos,
                                egui::Align2::CENTER_CENTER,
                                "✓",
                                egui::FontId::proportional(10.0),
                                ui.visuals().panel_fill, // Text color inverse (background)
                            );
                        }

                        // Flags (Last layer)
                        if show_flags {
                            let flag_size = 14.0;
                            let pick_bg = if current_flag == 1 { Theme::ACCENT_SUCCESS } else if pick_hovered { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
                            let pick_fg = if current_flag == 1 { Color32::WHITE } else { ui.visuals().text_color() };
                            ui.painter().circle_filled(pick_rect.center(), flag_size / 2.0, pick_bg);
                            ui.painter().text(pick_rect.center(), egui::Align2::CENTER_CENTER, "P", egui::FontId::proportional(9.0), pick_fg);

                            let reject_bg = if current_flag == -1 { Theme::ACCENT_ERROR } else if reject_hovered { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
                            let reject_fg = if current_flag == -1 { Color32::WHITE } else { ui.visuals().text_color() };
                            ui.painter().circle_filled(reject_rect.center(), flag_size / 2.0, reject_bg);
                            ui.painter().text(reject_rect.center(), egui::Align2::CENTER_CENTER, "X", egui::FontId::proportional(9.0), reject_fg);
                        }

                        ui.add_space(Self::THUMBNAIL_SPACING);
                    }

                    ui.add_space(Theme::SPACE_SM);
                });
            });

        // Show context menu if open
        // Access state.selected_photo_ids directly
        let selection_count = state.selected_photo_ids.len();
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
        filter: &mut crate::components::filmstrip_filter::FilmstripFilter,
        mut on_select: impl FnMut(String),
        mut on_flag: impl FnMut(String, i32),
    ) {
        // Render Filter Toolbox
        ui.horizontal(|ui| {
            ui.add_space(Theme::SPACE_SM);
            filter.ui(ui);
        });
        ui.separator();

        // Apply Filter
        let visible_photos = filter.apply(photos);

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

                // Tone Curve
                let tone_curve_shadows = photo.edit_tone_curve_shadows.unwrap_or(0.0);
                let tone_curve_darks = photo.edit_tone_curve_darks.unwrap_or(0.0);
                let tone_curve_lights = photo.edit_tone_curve_lights.unwrap_or(0.0);
                let tone_curve_highlights = photo.edit_tone_curve_highlights.unwrap_or(0.0);

                // HSL Saturation
                let hsl_red_sat = photo.edit_hsl_red_sat.unwrap_or(0.0);
                let hsl_orange_sat = photo.edit_hsl_orange_sat.unwrap_or(0.0);
                let hsl_yellow_sat = photo.edit_hsl_yellow_sat.unwrap_or(0.0);
                let hsl_green_sat = photo.edit_hsl_green_sat.unwrap_or(0.0);
                let hsl_aqua_sat = photo.edit_hsl_aqua_sat.unwrap_or(0.0);
                let hsl_blue_sat = photo.edit_hsl_blue_sat.unwrap_or(0.0);
                let hsl_purple_sat = photo.edit_hsl_purple_sat.unwrap_or(0.0);
                let hsl_magenta_sat = photo.edit_hsl_magenta_sat.unwrap_or(0.0);
                // HSL Hue
                let hsl_red_hue = photo.edit_hsl_red_hue.unwrap_or(0.0);
                let hsl_orange_hue = photo.edit_hsl_orange_hue.unwrap_or(0.0);
                let hsl_yellow_hue = photo.edit_hsl_yellow_hue.unwrap_or(0.0);
                let hsl_green_hue = photo.edit_hsl_green_hue.unwrap_or(0.0);
                let hsl_aqua_hue = photo.edit_hsl_aqua_hue.unwrap_or(0.0);
                let hsl_blue_hue = photo.edit_hsl_blue_hue.unwrap_or(0.0);
                let hsl_purple_hue = photo.edit_hsl_purple_hue.unwrap_or(0.0);
                let hsl_magenta_hue = photo.edit_hsl_magenta_hue.unwrap_or(0.0);
                // HSL Lum
                let hsl_red_lum = photo.edit_hsl_red_lum.unwrap_or(0.0);
                let hsl_orange_lum = photo.edit_hsl_orange_lum.unwrap_or(0.0);
                let hsl_yellow_lum = photo.edit_hsl_yellow_lum.unwrap_or(0.0);
                let hsl_green_lum = photo.edit_hsl_green_lum.unwrap_or(0.0);
                let hsl_aqua_lum = photo.edit_hsl_aqua_lum.unwrap_or(0.0);
                let hsl_blue_lum = photo.edit_hsl_blue_lum.unwrap_or(0.0);
                let hsl_purple_lum = photo.edit_hsl_purple_lum.unwrap_or(0.0);
                let hsl_magenta_lum = photo.edit_hsl_magenta_lum.unwrap_or(0.0);
                // Lens
                let lens_distortion = photo.edit_lens_distortion.unwrap_or(0.0);
                let lens_vignette_amount = photo.edit_lens_vignette_amount.unwrap_or(0.0);
                let lens_vignette_midpoint = photo.edit_lens_vignette_midpoint.unwrap_or(0.0);
                // NR
                let nr_luminance = photo.edit_nr_luminance.unwrap_or(0.0);
                let nr_color = photo.edit_nr_color.unwrap_or(0.0);
                // Sharpening
                let sharpen_amount = photo.edit_sharpen_amount.unwrap_or(0.0);
                let sharpen_radius = photo.edit_sharpen_radius.unwrap_or(1.0);
                
                // Only apply effects if there are actual edits
                let has_edits = exposure != 0.0 || contrast != 1.0 || temperature != 0.0 || 
                               tint != 0.0 || saturation != 0.0 || vibrance != 0.0;
                
                if has_edits {
                    crate::image_processing::ImageProcessor::process_image(
                        &result.image, exposure, contrast, temperature, tint,
                        highlights, shadows, whites, blacks, clarity, vibrance, saturation,
                        tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
                        hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
                        hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
                        // HSL Hue
                        hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue,
                        hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
                        // HSL Lum
                        hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum,
                        hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
                        // Lens
                        lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
                        // NR
                        nr_luminance, nr_color,
                        // Sharpening
                        sharpen_amount, sharpen_radius,
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
        // Optimization: only request for *visible* (filtered) photos
       
        let requests: Vec<ThumbnailRequest> = visible_photos
            .iter()
            .filter(|p| !self.thumbnail_cache.contains_key(&p.id))
            .map(|p| ThumbnailRequest {
                photo_id: p.id.clone(),
                path: p.path.clone(),
            })
            .take(30)
            .collect();

        if !requests.is_empty() {
            self.thumbnail_loader.request_thumbnails(requests);
        }

        // Dark background like Lightroom
        let bg_color = ui.visuals().panel_fill;
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

                    for photo in visible_photos {
                        let is_selected = selected_photo_id.as_ref() == Some(&photo.id);
                        
                        // Reserve space for thumbnail
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(Self::THUMBNAIL_SIZE, Self::THUMBNAIL_SIZE),
                            Sense::click(),
                        );

                        // Initialize the color mapping for labels
                        let label_color = match photo.color_label.as_deref() {
                            Some("Red") | Some("red") => Some(Theme::LABEL_RED),
                            Some("Yellow") | Some("yellow") => Some(Theme::LABEL_YELLOW),
                            Some("Green") | Some("green") => Some(Theme::LABEL_GREEN),
                            Some("Blue") | Some("blue") => Some(Theme::LABEL_BLUE),
                            Some("Purple") | Some("purple") => Some(Theme::LABEL_PURPLE),
                            _ => None,
                        };

                        // Draw thumbnail background
                        let thumb_color = ui.visuals().widgets.inactive.bg_fill;
                        
                        // Fill background (tinted if color label exists)
                        let bg_fill = if let Some(color) = label_color {
                            if is_selected {
                                color.gamma_multiply(0.4)
                            } else {
                                color.gamma_multiply(0.2)
                            }
                        } else {
                            thumb_color
                        };
                        
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(2),
                            bg_fill,
                        );

                        // Draw color label border if present
                        if let Some(color) = label_color {
                            ui.painter().rect_stroke(
                                rect.shrink(1.0),
                                CornerRadius::same(2),
                                Stroke::new(3.0, color),
                                egui::StrokeKind::Outside,
                            );
                        }

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
                                ui.visuals().weak_text_color(),
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

                        // Draw selected border
                        if is_selected {
                            // Filmstrip develop mode (single selection conceptually, but double border for consistency if needed)
                            // Or just simple border if it's the active edited photo.
                            // The user requested primary selection identification, which implies library/multi-select context.
                            // But consistency is good.
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(2),
                                Stroke::new(1.0, Color32::WHITE),
                                egui::StrokeKind::Inside,
                            );
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(2),
                                Stroke::new(3.0, ui.visuals().selection.bg_fill),
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
                            let pick_bg = if current_flag == 1 { Theme::ACCENT_SUCCESS } else if pick_response.hovered() { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
                            let pick_fg = if current_flag == 1 { Color32::WHITE } else { ui.visuals().text_color() };
                            
                            ui.painter().circle_filled(pick_rect.center(), flag_size / 2.0, pick_bg);
                            ui.painter().text(
                                pick_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "P",
                                egui::FontId::proportional(9.0),
                                pick_fg,
                            );

                            // Draw Reject Icon
                            let reject_bg = if current_flag == -1 { Theme::ACCENT_ERROR } else if reject_response.hovered() { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
                            let reject_fg = if current_flag == -1 { Color32::WHITE } else { ui.visuals().text_color() };
                            
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
