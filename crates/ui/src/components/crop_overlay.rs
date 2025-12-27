use egui::{Ui, Rect, Pos2, Vec2, Color32, Stroke, Sense};
use domain::value_objects::{CropSettings, AspectRatio};

/// Visual overlay for crop mode showing crop rectangle, handles, and composition grid
pub struct CropOverlay;

impl CropOverlay {
    const HANDLE_SIZE: f32 = 12.0;
    const HANDLE_COLOR: Color32 = Color32::WHITE;
    const HANDLE_STROKE: f32 = 2.0;
    const CROP_LINE_COLOR: Color32 = Color32::WHITE;
    const CROP_LINE_WIDTH: f32 = 2.0;
    const GRID_LINE_COLOR: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 100);
    const GRID_LINE_WIDTH: f32 = 1.0;
    const DARKEN_COLOR: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 120);

    /// Renders the crop overlay on top of the image
    pub fn show(
        ui: &mut Ui,
        image_rect: Rect,
        viewer_rect: Rect, // New: Full viewer area
        crop_settings: &mut CropSettings,
        show_grid: bool,
    ) -> CropOverlayResponse {
        let mut response = CropOverlayResponse::default();

        // Calculate crop rectangle in screen coordinates
        let crop_rect = Self::calculate_crop_rect(image_rect, crop_settings);

        // Interaction logic handled BEFORE drawing to allow cursor updates
        let handles = Self::get_handle_positions(crop_rect);
        let mut hovering_handle = None;
        let handle_radius = 8.0;

        let mouse_pos = ui.input(|i| i.pointer.hover_pos());

        if let Some(pos) = mouse_pos {
            // Check handles
            for (i, &handle_pos) in handles.iter().enumerate() {
                if pos.distance(handle_pos) <= handle_radius * 1.5 {
                    hovering_handle = Some(i);
                    break;
                }
            }
            
            // Check interaction
            if let Some(i) = hovering_handle {
                ui.output_mut(|o| o.cursor_icon = Self::get_cursor_for_handle(i));
                // Drag handled by handle widget later
            } else if crop_rect.contains(pos) {
                // Inside crop -> Move
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Move);
                
                if ui.input(|i| i.pointer.primary_down()) {
                    response.crop_dragged = true;
                }
            } else if viewer_rect.contains(pos) {
                // Outside crop but inside viewer -> Rotation
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::None);
                
                // Draw custom rotation cursor
                let painter = ui.painter().clone().with_layer_id(egui::LayerId::debug());
                painter.text(
                    pos,
                    egui::Align2::CENTER_CENTER,
                    crate::design_system::icons::ARROWS_CLOCKWISE,
                    egui::FontId::proportional(20.0),
                    Color32::WHITE,
                );
                 
                 // Manual drag handling for rotation since we don't have a widget for the background hole
                 if ui.input(|i| i.pointer.primary_down()) {
                     response.rotation_dragged = true;
                 }
            }
            
            if ui.input(|i| i.pointer.primary_down()) {
                 response.drag_delta = ui.input(|i| i.pointer.delta());
            }
        }

        // Draw darkened area outside crop
        Self::draw_darken_overlay(ui, image_rect, crop_rect);

        // Draw crop rectangle
        ui.painter().add(egui::epaint::RectShape::stroke(
            crop_rect,
            0.0,
            Stroke::new(Self::CROP_LINE_WIDTH, Self::CROP_LINE_COLOR),
            egui::epaint::StrokeKind::Middle,
        ));

        // Draw composition grid
        if show_grid {
            Self::draw_composition_grid(ui, crop_rect);
        }

        // Draw resize handles
        let handles = Self::get_handle_positions(crop_rect);
        for (index, handle_pos) in handles.iter().enumerate() {
            let handle_rect = Rect::from_center_size(*handle_pos, Vec2::splat(Self::HANDLE_SIZE));
            
            // Draw handle
            ui.painter().rect_filled(handle_rect, 2.0, Self::HANDLE_COLOR);
            ui.painter().add(egui::epaint::RectShape::stroke(
                handle_rect,
                2.0,
                Stroke::new(Self::HANDLE_STROKE, Color32::BLACK),
                egui::epaint::StrokeKind::Middle,
            ));

            // Handle interaction
            let handle_response = ui.interact(handle_rect, ui.id().with(index), Sense::drag());
            if handle_response.dragged() {
                response.handle_dragged = Some(index);
                response.drag_delta = handle_response.drag_delta();
            }
            if handle_response.hovered() {
                ui.ctx().set_cursor_icon(Self::get_cursor_for_handle(index));
            }
        }

        // Make crop area draggable
        let crop_response = ui.interact(crop_rect, ui.id().with("crop_drag"), Sense::drag());
        if crop_response.dragged() {
            response.crop_dragged = true;
            response.drag_delta = crop_response.drag_delta();
        }
        if crop_response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
        }

        response
    }

    /// Calculate crop rectangle in screen coordinates from normalized crop settings
    fn calculate_crop_rect(image_rect: Rect, crop: &CropSettings) -> Rect {
        let x = image_rect.min.x + crop.crop_x() * image_rect.width();
        let y = image_rect.min.y + crop.crop_y() * image_rect.height();
        let w = crop.crop_width() * image_rect.width();
        let h = crop.crop_height() * image_rect.height();

        Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
    }

    /// Draw darkened overlay outside the crop area
    fn draw_darken_overlay(ui: &mut Ui, image_rect: Rect, crop_rect: Rect) {
        let painter = ui.painter();

        // Top
        if crop_rect.min.y > image_rect.min.y {
            painter.rect_filled(
                Rect::from_min_max(
                    image_rect.min,
                    Pos2::new(image_rect.max.x, crop_rect.min.y),
                ),
                0.0,
                Self::DARKEN_COLOR,
            );
        }

        // Bottom
        if crop_rect.max.y < image_rect.max.y {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(image_rect.min.x, crop_rect.max.y),
                    image_rect.max,
                ),
                0.0,
                Self::DARKEN_COLOR,
            );
        }

        // Left
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(image_rect.min.x, crop_rect.min.y),
                Pos2::new(crop_rect.min.x, crop_rect.max.y),
            ),
            0.0,
            Self::DARKEN_COLOR,
        );

        // Right
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(crop_rect.max.x, crop_rect.min.y),
                Pos2::new(image_rect.max.x, crop_rect.max.y),
            ),
            0.0,
            Self::DARKEN_COLOR,
        );
    }

    /// Draw composition grid (rule of thirds)
    fn draw_composition_grid(ui: &mut Ui, crop_rect: Rect) {
        let painter = ui.painter();
        let stroke = Stroke::new(Self::GRID_LINE_WIDTH, Self::GRID_LINE_COLOR);

        // Vertical lines (divide into thirds)
        let third_w = crop_rect.width() / 3.0;
        for i in 1..3 {
            let x = crop_rect.min.x + i as f32 * third_w;
            painter.line_segment(
                [Pos2::new(x, crop_rect.min.y), Pos2::new(x, crop_rect.max.y)],
                stroke,
            );
        }

        // Horizontal lines (divide into thirds)
        let third_h = crop_rect.height() / 3.0;
        for i in 1..3 {
            let y = crop_rect.min.y + i as f32 * third_h;
            painter.line_segment(
                [Pos2::new(crop_rect.min.x, y), Pos2::new(crop_rect.max.x, y)],
                stroke,
            );
        }
    }

    /// Get positions of 8 resize handles (corners + midpoints)
    fn get_handle_positions(rect: Rect) -> [Pos2; 8] {
        let center = rect.center();
        [
            rect.left_top(),      // 0: Top-left
            Pos2::new(center.x, rect.min.y), // 1: Top-center
            rect.right_top(),     // 2: Top-right
            Pos2::new(rect.max.x, center.y), // 3: Right-center
            rect.right_bottom(),  // 4: Bottom-right
            Pos2::new(center.x, rect.max.y), // 5: Bottom-center
            rect.left_bottom(),   // 6: Bottom-left
            Pos2::new(rect.min.x, center.y), // 7: Left-center
        ]
    }

    /// Get appropriate cursor icon for each handle
    fn get_cursor_for_handle(index: usize) -> egui::CursorIcon {
        match index {
            0 | 4 => egui::CursorIcon::ResizeNwSe, // Top-left, Bottom-right
            2 | 6 => egui::CursorIcon::ResizeNeSw, // Top-right, Bottom-left
            1 | 5 => egui::CursorIcon::ResizeVertical, // Top, Bottom
            3 | 7 => egui::CursorIcon::ResizeHorizontal, // Right, Left
            _ => egui::CursorIcon::Default,
        }
    }

    /// Update crop settings based on handle drag
    pub fn update_from_handle_drag(
        crop: &mut CropSettings,
        handle_index: usize,
        delta: Vec2,
        image_size: Vec2,
        aspect_ratio: &AspectRatio,
    ) {
        // Convert delta to normalized coordinates
        let norm_delta = Vec2::new(delta.x / image_size.x, delta.y / image_size.y);

        let (new_x, new_y, new_w, new_h) = match handle_index {
            0 => {
                // Top-left: adjust x, y, width, height
                let new_x = crop.crop_x() + norm_delta.x;
                let new_y = crop.crop_y() + norm_delta.y;
                let new_w = crop.crop_width() - norm_delta.x;
                let new_h = crop.crop_height() - norm_delta.y;
                (new_x, new_y, new_w, new_h)
            }
            1 => {
                // Top-center: adjust y, height only
                let new_y = crop.crop_y() + norm_delta.y;
                let new_h = crop.crop_height() - norm_delta.y;
                (crop.crop_x(), new_y, crop.crop_width(), new_h)
            }
            2 => {
                // Top-right: adjust y, height, width
                let new_y = crop.crop_y() + norm_delta.y;
                let new_w = crop.crop_width() + norm_delta.x;
                let new_h = crop.crop_height() - norm_delta.y;
                (crop.crop_x(), new_y, new_w, new_h)
            }
            3 => {
                // Right-center: adjust width only
                let new_w = crop.crop_width() + norm_delta.x;
                (crop.crop_x(), crop.crop_y(), new_w, crop.crop_height())
            }
            4 => {
                // Bottom-right: adjust width, height
                let new_w = crop.crop_width() + norm_delta.x;
                let new_h = crop.crop_height() + norm_delta.y;
                (crop.crop_x(), crop.crop_y(), new_w, new_h)
            }
            5 => {
                // Bottom-center: adjust height only
                let new_h = crop.crop_height() + norm_delta.y;
                (crop.crop_x(), crop.crop_y(), crop.crop_width(), new_h)
            }
            6 => {
                // Bottom-left: adjust x, width, height
                let new_x = crop.crop_x() + norm_delta.x;
                let new_w = crop.crop_width() - norm_delta.x;
                let new_h = crop.crop_height() + norm_delta.y;
                (new_x, crop.crop_y(), new_w, new_h)
            }
            7 => {
                // Left-center: adjust x, width only
                let new_x = crop.crop_x() + norm_delta.x;
                let new_w = crop.crop_width() - norm_delta.x;
                (new_x, crop.crop_y(), new_w, crop.crop_height())
            }
            _ => return,
        };

        // Apply aspect ratio constraint if needed
        let (final_w, final_h) = if *aspect_ratio != AspectRatio::Free && *aspect_ratio != AspectRatio::Original {
            let ratio_value = aspect_ratio.value();
            // Maintain aspect ratio by adjusting height based on width
            (new_w, new_w / ratio_value)
        } else {
            (new_w, new_h)
        };

        *crop = CropSettings::new(
            new_x,
            new_y,
            final_w,
            final_h,
            crop.rotation_90(),
            crop.angle(),
            crop.flip_horizontal(),
            crop.flip_vertical(),
        );
    }

    /// Update crop position based on drag
    pub fn update_from_crop_drag(
        crop: &mut CropSettings,
        delta: Vec2,
        image_size: Vec2,
    ) {
        let norm_delta = Vec2::new(delta.x / image_size.x, delta.y / image_size.y);
        
        *crop = CropSettings::new(
            crop.crop_x() + norm_delta.x,
            crop.crop_y() + norm_delta.y,
            crop.crop_width(),
            crop.crop_height(),
            crop.rotation_90(),
            crop.angle(),
            crop.flip_horizontal(),
            crop.flip_vertical(),
        );
    }
}

/// Response from crop overlay interaction
#[derive(Default)]
pub struct CropOverlayResponse {
    pub handle_dragged: Option<usize>,
    pub crop_dragged: bool,
    pub rotation_dragged: bool,
    pub drag_delta: Vec2,
}
