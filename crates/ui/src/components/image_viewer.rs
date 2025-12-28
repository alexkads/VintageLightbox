// Image Viewer Component
// Displays images with zoom, pan, and navigation controls

use egui::{Ui, Vec2, Rect, Sense, UiBuilder, Color32};
use crate::state::{AppState, CurrentView};
use crate::design_system::{theme::Theme, widgets};
use crate::geometry::{ClipVertex, clip_polygon_to_uv_bounds};

pub struct ImageViewer;

impl ImageViewer {
    /// Show the image viewer with zoom/pan capabilities
    pub fn show(ui: &mut Ui, state: &mut AppState) {
        let (new_zoom, new_pan, painted_image_rect, viewer_rect) = Self::render(
            ui,
            state.detail_image.as_ref(),
            state.thumbnail_preview.as_ref(),
            state.intelligent_fill_texture.as_ref(), // Pass intelligent fill texture
            state.develop_selected_photo_id.is_some(),
            state.zoom_level,
            state.pan_offset,
            true, // interactive
            !state.crop_mode_active, // allow_pan: Disable pan in crop mode (unless Space is held)
            state.crop_settings.as_ref(), // Always pass crop settings so we can get rotation
            !state.crop_mode_active, // apply_crop_clip: Only clip UVs if NOT in crop editing mode
        );

        state.zoom_level = new_zoom;
        state.pan_offset = new_pan;

        // Show crop overlay if in crop mode
        if state.crop_mode_active {
            if let Some(crop_settings) = &mut state.crop_settings {
                if let Some(img_rect) = painted_image_rect {
                    // Show crop overlay
                    use crate::components::crop_overlay::CropOverlay;
                    CropOverlay::show(
                        ui,
                        img_rect,
                        viewer_rect, 
                        crop_settings,
                        state.show_composition_grid,
                        state.selected_aspect_ratio.clone(),
                    );
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
        intelligent_fill_texture: Option<&egui::TextureHandle>, // Intelligent fill result
        has_selection: bool,
        current_zoom: f32,
        current_pan: Vec2,
        interactive: bool,
        allow_pan: bool, // Restored parameter
        crop_settings: Option<&domain::value_objects::CropSettings>, // Updated for crop support
        apply_crop_clip: bool, // New parameter: if true, applies UV crop. If false, shows full image but rotated.
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
        // Determine which texture to draw
        let (texture_handle, is_thumbnail) = if let Some(texture) = detail_image {
            (Some(texture), false)
        } else if let Some(thumbnail) = thumbnail_preview {
            (Some(thumbnail), true)
        } else {
            (None, false)
        };

        if let Some(texture) = texture_handle {
            // Unified drawing logic for both Full Res and Thumbnail
            // This ensures identical aspect ratio and positioning calculations
            
            let texture_size = Vec2::new(texture.size()[0] as f32, texture.size()[1] as f32);
            
            // Calculate the effective size based on rotation
            let rotated_texture_size = if let Some(crop) = crop_settings {
                 if crop.rotation_90() % 2 != 0 {
                     Vec2::new(texture_size.y, texture_size.x)
                 } else {
                     texture_size
                 }
            } else {
                 texture_size
            };

            // Calculate the effective size considering crop (for aspect ratio correction)
            let effective_size = if let Some(crop) = crop_settings {
                if apply_crop_clip {
                    // When cropped AND CLIPPED, the displayed portion is the crop dimensions.
                    // Crop coordinates are relative to the rotated image dimensions.
                    Vec2::new(
                        rotated_texture_size.x * crop.crop_width(),
                        rotated_texture_size.y * crop.crop_height()
                    )
                } else {
                    // When editing crop, show full rotated image
                    rotated_texture_size
                }
            } else {
                texture_size
            };

            // Calculate scaled size based on effective (cropped) dimensions to fit available space
            // NOTE: We allow upscaling (remove .min(1.0)) so that small thumbnails 
            // stretch to fill the screen, acting as proper placeholders for the HD image.
            let scale = (available_size.x / effective_size.x)
                .min(available_size.y / effective_size.y);

            let base_img_size = effective_size * scale;
            let zoomed_size = base_img_size * zoom;

            let center = rect.center() + pan;
            let img_rect = Rect::from_center_size(center, zoomed_size);

            let mut img = egui::Image::new(texture);

            if let Some(crop) = crop_settings {
                eprintln!("[ImageViewer] apply_crop_clip={}, angle={}, rotation_90={}",
                    apply_crop_clip, crop.angle(), crop.rotation_90());
                if apply_crop_clip {
                    // "Neutral Viewer" implementation (Lightroom style):
                    // The Viewer renders the final cropped result as a straight rectangle.
                    // We calculate the UV coordinates of the 4 corners of the crop in the original texture,
                    // accounting for rotation, aspect ratio, and flips.
                    // We draw a Mesh with these UVs, so the egui::Image widget itself is NOT rotated.



                    // 1. Define corners of the Crop Window in Frame Space (0.0 - 1.0)
                    // The Frame corresponds to the image rotated by 90-degree steps.
                    let cx = crop.crop_x();
                    let cy = crop.crop_y();
                    let cw = crop.crop_width();
                    let ch = crop.crop_height();

                    let corners_frame = [
                        egui::pos2(cx, cy),           // Top-Left
                        egui::pos2(cx + cw, cy),      // Top-Right
                        egui::pos2(cx + cw, cy + ch), // Bottom-Right
                        egui::pos2(cx, cy + ch),      // Bottom-Left
                    ];

                    // 2. Map Frame Space -> Original Texture UV Space
                    let angle_rad = crop.angle().to_radians();
                    let aspect = rotated_texture_size.x / rotated_texture_size.y;
                    let center = egui::pos2(0.5, 0.5);

                    // Map texture coordinates to UVs
                    let mut uvs = [egui::Pos2::ZERO; 4];
                    
                    for (i, &p_frame) in corners_frame.iter().enumerate() {
                        // A. Center relative to 0.5
                        let p_centered = p_frame - center;

                        // B. Correct for Aspect Ratio (to rotate geometrically correct)
                        // Treat the image as a physical plane with aspect ratio 'aspect'
                        let p_phys = egui::vec2(p_centered.x * aspect, p_centered.y);

                        // C. Inverse Rotate by 'Angle' (Frame -> Image Content)
                        // We rotated Image by +Angle to get Frame View.
                        // So to find Source Pixel from Frame Pixel, we rotate by -Angle.
                        let (sin, cos) = (-angle_rad).sin_cos();
                        let p_rot = egui::vec2(
                            p_phys.x * cos - p_phys.y * sin,
                            p_phys.x * sin + p_phys.y * cos
                        );

                        // D. Restore Aspect Ratio normalization
                        let p_rot_norm = egui::vec2(p_rot.x / aspect, p_rot.y);
                        
                        // E. Uncenter -> UV in Rotated-90 Space
                        let uv_r90 = center + p_rot_norm;

                        // F. Undo Rotation 90 (Map Rotated-90 Space -> Original Space)
                        // Rotation 90 logic implies transformation of coordinates.
                        // rot90 = 1 (90 deg CW visually). Accessing (u,v) on Rot90 matches (v, 1-u) on Original?
                        // Let's verify common conventions or check logic.
                        // If we don't have explicit logic, assume standard CW steps.
                        // For now, let's assume usage of `coord` mapping.
                        
                        let uv_orig = match crop.rotation_90() % 4 {
                            0 => uv_r90,
                            1 => egui::pos2(uv_r90.y, 1.0 - uv_r90.x), // 90 CW
                            2 => egui::pos2(1.0 - uv_r90.x, 1.0 - uv_r90.y), // 180
                            3 => egui::pos2(1.0 - uv_r90.y, uv_r90.x), // 270 CW (90 CCW)
                            _ => uv_r90,
                        };

                        // G. Undo Flips (Map Final UV -> Source UV)
                        // Flips happen BEFORE Rotation in Edit Mode.
                        // So we modify the target UV.
                        let mut final_u = uv_orig.x;
                        let mut final_v = uv_orig.y;

                        if crop.flip_horizontal() { final_u = 1.0 - final_u; }
                        if crop.flip_vertical() { final_v = 1.0 - final_v; }

                        uvs[i] = egui::pos2(final_u, final_v);
                    }

                    // 3. Draw fill background and construct clipped mesh

                    // Draw fill background for empty areas created by rotation
                    // Only needed when angle is non-zero (non 90-degree rotation creates gaps)
                    eprintln!("[ImageViewer:View] crop.angle()={}, rotation_90={}, fill_mode={:?}",
                        crop.angle(), crop.rotation_90(), crop.fill_mode());

                    if crop.angle() != 0.0 {
                        // Check if we should use intelligent fill texture
                        let use_intelligent_fill = crop.fill_mode() == domain::value_objects::RotationFillMode::Intelligent
                            && intelligent_fill_texture.is_some();

                        eprintln!("[ImageViewer:View] Drawing fill! use_intelligent={}, img_rect={:?}", use_intelligent_fill, img_rect);

                        if use_intelligent_fill {
                            // Use the intelligent fill texture as background
                            if let Some(fill_tex) = intelligent_fill_texture {
                                let fill_img = egui::Image::new(fill_tex)
                                    .fit_to_exact_size(img_rect.size());
                                fill_img.paint_at(ui, img_rect);
                            }
                        } else {
                            // Use solid color fill
                            let fill_color = match crop.fill_mode() {
                                domain::value_objects::RotationFillMode::Black => Color32::BLACK,
                                domain::value_objects::RotationFillMode::White => Color32::WHITE,
                                domain::value_objects::RotationFillMode::Transparent => Color32::TRANSPARENT,
                                domain::value_objects::RotationFillMode::Intelligent => {
                                    // Processing or not available - show placeholder (magenta for debug visibility)
                                    Color32::from_rgb(255, 0, 255)
                                },
                                domain::value_objects::RotationFillMode::ShrinkToFit => {
                                    // ShrinkToFit doesn't need fill
                                    Color32::TRANSPARENT
                                },
                            };

                            eprintln!("[ImageViewer:View] Painting fill color {:?} at {:?}", fill_color, img_rect);
                            if fill_color != Color32::TRANSPARENT {
                                ui.painter().rect_filled(img_rect, 0.0, fill_color);
                            }
                        }
                    }
                    
                    // Create input vertices with screen position + UV
                    let screen_corners = [
                        img_rect.min,
                        egui::pos2(img_rect.max.x, img_rect.min.y),
                        img_rect.max,
                        egui::pos2(img_rect.min.x, img_rect.max.y),
                    ];

                    let input_vertices: Vec<ClipVertex> = screen_corners
                        .iter()
                        .zip(uvs.iter())
                        .map(|(&pos, &uv)| ClipVertex::new(pos, uv))
                        .collect();

                    // Clip polygon to valid UV bounds [0,1]
                    let clipped = clip_polygon_to_uv_bounds(&input_vertices);

                    // Only draw texture mesh if we have valid vertices after clipping
                    if clipped.len() >= 3 {
                        use egui::epaint::{Mesh, Vertex};
                        let mut mesh = Mesh::with_texture(texture.id());

                        // Add clipped vertices to mesh
                        for v in &clipped {
                            mesh.vertices.push(Vertex {
                                pos: v.pos,
                                uv: v.uv,
                                color: Color32::WHITE,
                            });
                        }

                        // Triangulate using fan triangulation (works for convex polygons)
                        for i in 1..(clipped.len() - 1) {
                            mesh.add_triangle(0, i as u32, (i + 1) as u32);
                        }

                        // Draw clipped texture mesh
                        ui.painter().add(egui::Shape::mesh(mesh));
                    }
                    
                    painted_rect = Some(img_rect);
                    
                    // Return early as we handled painting
                    return (zoom, pan, painted_rect, rect);
                } else {
                    // Edit Mode: Show full image with rotation and flips
                    // Use mesh-based rendering when there's non-90° rotation for proper fill support

                    let total_degrees = (crop.rotation_90() as f32 * 90.0) + crop.angle();

                    if crop.angle() != 0.0 {
                        // Use mesh-based approach for proper fill background support
                        // (egui's Image::rotate() doesn't leave transparent corners)

                        let angle_rad = crop.angle().to_radians();
                        let aspect = texture_size.x / texture_size.y;
                        let center_uv = egui::pos2(0.5, 0.5);

                        // Compute rotated UV coordinates for full image (not cropped)
                        let screen_corners = [
                            img_rect.min,
                            egui::pos2(img_rect.max.x, img_rect.min.y),
                            img_rect.max,
                            egui::pos2(img_rect.min.x, img_rect.max.y),
                        ];

                        // For edit mode, we show the full texture but rotated
                        // Map screen corners to UVs accounting for rotation
                        let frame_corners = [
                            egui::pos2(0.0, 0.0), // Top-Left
                            egui::pos2(1.0, 0.0), // Top-Right
                            egui::pos2(1.0, 1.0), // Bottom-Right
                            egui::pos2(0.0, 1.0), // Bottom-Left
                        ];

                        let mut uvs = [egui::Pos2::ZERO; 4];
                        for (i, &p_frame) in frame_corners.iter().enumerate() {
                            // Center relative to 0.5
                            let p_centered = p_frame - center_uv;

                            // Correct for aspect ratio
                            let p_phys = egui::vec2(p_centered.x * aspect, p_centered.y);

                            // Inverse rotate by angle (Frame -> Image Content)
                            let (sin, cos) = (-angle_rad).sin_cos();
                            let p_rot = egui::vec2(
                                p_phys.x * cos - p_phys.y * sin,
                                p_phys.x * sin + p_phys.y * cos
                            );

                            // Restore aspect ratio normalization
                            let p_rot_norm = egui::vec2(p_rot.x / aspect, p_rot.y);

                            // Uncenter -> UV in rotated space
                            let mut uv = center_uv + p_rot_norm;

                            // Apply rotation_90 transformation
                            uv = match crop.rotation_90() % 4 {
                                0 => uv,
                                1 => egui::pos2(uv.y, 1.0 - uv.x), // 90 CW
                                2 => egui::pos2(1.0 - uv.x, 1.0 - uv.y), // 180
                                3 => egui::pos2(1.0 - uv.y, uv.x), // 270 CW
                                _ => uv,
                            };

                            // Apply flips
                            if crop.flip_horizontal() { uv.x = 1.0 - uv.x; }
                            if crop.flip_vertical() { uv.y = 1.0 - uv.y; }

                            uvs[i] = uv;
                        }

                        // Draw fill background first
                        let use_intelligent_fill = crop.fill_mode() == domain::value_objects::RotationFillMode::Intelligent
                            && intelligent_fill_texture.is_some();

                        if use_intelligent_fill {
                            if let Some(fill_tex) = intelligent_fill_texture {
                                let fill_img = egui::Image::new(fill_tex)
                                    .fit_to_exact_size(img_rect.size());
                                fill_img.paint_at(ui, img_rect);
                            }
                        } else {
                            let fill_color = match crop.fill_mode() {
                                domain::value_objects::RotationFillMode::Black => Color32::BLACK,
                                domain::value_objects::RotationFillMode::White => Color32::WHITE,
                                domain::value_objects::RotationFillMode::Transparent => Color32::TRANSPARENT,
                                domain::value_objects::RotationFillMode::Intelligent => {
                                    // Processing or not available - show placeholder
                                    Color32::from_gray(60)
                                },
                                domain::value_objects::RotationFillMode::ShrinkToFit => {
                                    Color32::TRANSPARENT
                                },
                            };

                            if fill_color != Color32::TRANSPARENT {
                                ui.painter().rect_filled(img_rect, 0.0, fill_color);
                            }
                        }

                        // Create input vertices for clipping
                        let input_vertices: Vec<ClipVertex> = screen_corners
                            .iter()
                            .zip(uvs.iter())
                            .map(|(&pos, &uv)| ClipVertex::new(pos, uv))
                            .collect();

                        // Clip polygon to valid UV bounds [0,1]
                        let clipped = clip_polygon_to_uv_bounds(&input_vertices);

                        // Draw clipped texture mesh
                        if clipped.len() >= 3 {
                            use egui::epaint::{Mesh, Vertex};
                            let mut mesh = Mesh::with_texture(texture.id());

                            for v in &clipped {
                                mesh.vertices.push(Vertex {
                                    pos: v.pos,
                                    uv: v.uv,
                                    color: Color32::WHITE,
                                });
                            }

                            // Fan triangulation
                            for i in 1..(clipped.len() - 1) {
                                mesh.add_triangle(0, i as u32, (i + 1) as u32);
                            }

                            ui.painter().add(egui::Shape::mesh(mesh));
                        }

                        painted_rect = Some(img_rect);

                        // Skip normal img.paint_at since we used mesh rendering
                        return (zoom, pan, painted_rect, rect);
                    } else if total_degrees != 0.0 {
                        // Only 90° rotations, no angle - use simpler approach
                        let mut min = egui::pos2(0.0, 0.0);
                        let mut max = egui::pos2(1.0, 1.0);
                        if crop.flip_horizontal() { std::mem::swap(&mut min.x, &mut max.x); }
                        if crop.flip_vertical() { std::mem::swap(&mut min.y, &mut max.y); }
                        if crop.flip_horizontal() || crop.flip_vertical() {
                            img = img.uv(Rect::from_min_max(min, max));
                        }
                        img = img.rotate(total_degrees.to_radians(), Vec2::splat(0.5));
                    } else {
                        // No rotation at all, just apply flips
                        let mut min = egui::pos2(0.0, 0.0);
                        let mut max = egui::pos2(1.0, 1.0);
                        if crop.flip_horizontal() { std::mem::swap(&mut min.x, &mut max.x); }
                        if crop.flip_vertical() { std::mem::swap(&mut min.y, &mut max.y); }
                        if crop.flip_horizontal() || crop.flip_vertical() {
                            img = img.uv(Rect::from_min_max(min, max));
                        }
                    }
                }
            }

            img.paint_at(ui, img_rect);
            painted_rect = Some(img_rect);

            // If it's a thumbnail, show loading indicator
            if is_thumbnail {
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
            }
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

