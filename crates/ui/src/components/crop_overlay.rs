use egui::{Ui, Rect, Pos2, Vec2, Color32, Stroke, Sense, Shape};
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
        image_rect: Rect, // The layout bounds of the image (axis-aligned)
        viewer_rect: Rect, // The full viewer area
        crop_settings: &mut CropSettings,
        show_grid: bool,
        aspect_ratio: AspectRatio,
    ) -> CropOverlayResponse {
        let mut response = CropOverlayResponse::default();

        // 1. Calculate Crop Rect (Axis-Aligned on Screen)
        // We interpret crop_settings (0..1) directly on the image_rect
        let mut crop_rect = Self::calculate_crop_rect(image_rect, crop_settings);

        // 2. Interaction
        let mouse_pos = ui.input(|i| i.pointer.hover_pos());
        let handles = Self::get_handle_positions(crop_rect);
        let mut hovering_handle = None;
        let handle_radius = 8.0;

        if let Some(pos) = mouse_pos {
            // Check handles
            for (i, &h_pos) in handles.iter().enumerate() {
                if pos.distance(h_pos) <= handle_radius * 1.5 {
                    hovering_handle = Some(i);
                    break;
                }
            }
            
            // Set Cursor
            if let Some(i) = hovering_handle {
                ui.output_mut(|o| o.cursor_icon = Self::get_cursor_for_handle(i));
                if ui.input(|i| i.pointer.primary_down()) {
                    response.handle_dragged = Some(i);
                }
            } else if crop_rect.contains(pos) {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Move);
                if ui.input(|i| i.pointer.primary_down()) {
                    response.crop_dragged = true;
                }
            } else if viewer_rect.contains(pos) {
                // Rotation (Outside crop)
                 ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::None); // Or a rotation icon
                 
                 // Show rotation cursor
                let painter = ui.painter().clone().with_layer_id(egui::LayerId::debug());
                painter.text(
                    pos,
                    egui::Align2::CENTER_CENTER,
                    crate::design_system::icons::ARROWS_CLOCKWISE,
                    egui::FontId::proportional(20.0),
                    Color32::WHITE,
                );

                 if ui.input(|i| i.pointer.primary_down()) {
                     response.rotation_dragged = true;
                 }
            }
            
            // Capture Delta
             if ui.input(|i| i.pointer.primary_down()) {
                 response.drag_delta = ui.input(|i| i.pointer.delta());
            }
        }
        
        // Update Logic (Immediate - normally done by caller but let's decouple if needed)
        // Actually the caller does the update based on response, but previous code updated here.
        // Let's return the response and let caller handle OR handle here provided we have mutable access (we do).
        
        if let Some(handle_idx) = response.handle_dragged {
            Self::update_crop_handle(crop_settings, handle_idx, response.drag_delta, image_rect.size(), &aspect_ratio);
            // Re-calc rect for drawing
             crop_rect = Self::calculate_crop_rect(image_rect, crop_settings);
        } else if response.crop_dragged {
            Self::update_crop_pan(crop_settings, response.drag_delta, image_rect.size());
             crop_rect = Self::calculate_crop_rect(image_rect, crop_settings);
        } else if response.rotation_dragged {
            // Rotation Logic
            let sensitivity = 0.5;
            let delta = response.drag_delta.x * sensitivity;
            let new_angle = crop_settings.angle() + delta;
            
            *crop_settings = CropSettings::new(
                crop_settings.crop_x(), crop_settings.crop_y(), crop_settings.crop_width(), crop_settings.crop_height(),
                crop_settings.rotation_90(), new_angle,
                crop_settings.flip_horizontal(), crop_settings.flip_vertical()
            );
        }

        // 3. Drawing
        Self::draw_darken_overlay(ui, image_rect, crop_rect);
        
        // Crop Box
        ui.painter().rect_stroke(
            crop_rect, 
            0.0, 
            Stroke::new(Self::CROP_LINE_WIDTH, Self::CROP_LINE_COLOR),
            egui::StrokeKind::Middle,
        );

        // Grid
        if show_grid {
            Self::draw_composition_grid(ui, crop_rect);
        }

        // Handles
        for (i, &pos) in handles.iter().enumerate() {
            let rect = Rect::from_center_size(pos, Vec2::splat(Self::HANDLE_SIZE));
            ui.painter().rect_filled(rect, 2.0, Self::HANDLE_COLOR);
            ui.painter().rect_stroke(rect, 2.0, Stroke::new(Self::HANDLE_STROKE, Color32::BLACK), egui::StrokeKind::Middle);
        }

        // Blocker
        ui.interact(viewer_rect, ui.id().with("blocker"), Sense::drag());

        response
    }

    fn calculate_crop_rect(image_rect: Rect, crop: &CropSettings) -> Rect {
        let x = image_rect.min.x + crop.crop_x() * image_rect.width();
        let y = image_rect.min.y + crop.crop_y() * image_rect.height();
        let w = crop.crop_width() * image_rect.width();
        let h = crop.crop_height() * image_rect.height();
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
    }

    fn draw_darken_overlay(ui: &mut Ui, image_rect: Rect, crop_rect: Rect) {
        let painter = ui.painter();
        
        // We draw 4 rects around the crop rect, bounded by image_rect
        // Top
        if crop_rect.min.y > image_rect.min.y {
            painter.rect_filled(
                Rect::from_min_max(image_rect.min, Pos2::new(image_rect.max.x, crop_rect.min.y)),
                0.0,
                Self::DARKEN_COLOR,
            );
        }
        // Bottom
        if crop_rect.max.y < image_rect.max.y {
             painter.rect_filled(
                Rect::from_min_max(Pos2::new(image_rect.min.x, crop_rect.max.y), image_rect.max),
                0.0,
                Self::DARKEN_COLOR,
            );
        }
        // Left
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(image_rect.min.x, crop_rect.min.y),
                Pos2::new(crop_rect.min.x, crop_rect.max.y)
            ),
            0.0,
            Self::DARKEN_COLOR,
        );
        // Right
        painter.rect_filled(
             Rect::from_min_max(
                Pos2::new(crop_rect.max.x, crop_rect.min.y),
                Pos2::new(image_rect.max.x, crop_rect.max.y)
            ),
            0.0,
            Self::DARKEN_COLOR,
        );
    }

    fn draw_composition_grid(ui: &mut Ui, crop_rect: Rect) {
        let painter = ui.painter();
        let stroke = Stroke::new(Self::GRID_LINE_WIDTH, Self::GRID_LINE_COLOR);

        let third_w = crop_rect.width() / 3.0;
        let third_h = crop_rect.height() / 3.0;

        for i in 1..3 {
            let x = crop_rect.min.x + (i as f32 * third_w);
            painter.line_segment([Pos2::new(x, crop_rect.min.y), Pos2::new(x, crop_rect.max.y)], stroke);
            
            let y = crop_rect.min.y + (i as f32 * third_h);
            painter.line_segment([Pos2::new(crop_rect.min.x, y), Pos2::new(crop_rect.max.x, y)], stroke);
        }
    }

    fn get_handle_positions(rect: Rect) -> [Pos2; 8] {
        let center = rect.center();
        [
            rect.left_top(),
            Pos2::new(center.x, rect.min.y),
            rect.right_top(),
            Pos2::new(rect.max.x, center.y),
            rect.right_bottom(),
            Pos2::new(center.x, rect.max.y),
            rect.left_bottom(),
            Pos2::new(rect.min.x, center.y),
        ]
    }

    fn get_cursor_for_handle(index: usize) -> egui::CursorIcon {
        match index {
            0 | 4 => egui::CursorIcon::ResizeNwSe,
            2 | 6 => egui::CursorIcon::ResizeNeSw,
            1 | 5 => egui::CursorIcon::ResizeVertical,
            3 | 7 => egui::CursorIcon::ResizeHorizontal,
            _ => egui::CursorIcon::Default,
        }
    }
    
    // Internal updates for self-contained interaction
    fn update_crop_handle(
        crop: &mut CropSettings,
        index: usize,
        delta: Vec2,
        image_size: Vec2,
        aspect_ratio: &AspectRatio,
    ) {
         let norm_delta = Vec2::new(delta.x / image_size.x, delta.y / image_size.y);
         let cx = crop.crop_x();
         let cy = crop.crop_y();
         let cw = crop.crop_width();
         let ch = crop.crop_height();

         let (nx, ny, nw, nh) = match index {
            0 => (cx + norm_delta.x, cy + norm_delta.y, cw - norm_delta.x, ch - norm_delta.y),
            1 => (cx, cy + norm_delta.y, cw, ch - norm_delta.y),
            2 => (cx, cy + norm_delta.y, cw + norm_delta.x, ch - norm_delta.y),
            3 => (cx, cy, cw + norm_delta.x, ch),
            4 => (cx, cy, cw + norm_delta.x, ch + norm_delta.y),
            5 => (cx, cy, cw, ch + norm_delta.y),
            6 => (cx + norm_delta.x, cy, cw - norm_delta.x, ch + norm_delta.y),
            7 => (cx + norm_delta.x, cy, cw - norm_delta.x, ch),
            _ => (cx, cy, cw, ch),
         };

         // Aspect Ratio Logic would go here (simplified for now)
         *crop = CropSettings::new(
             nx, ny, nw, nh,
             crop.rotation_90(), crop.angle(),
             crop.flip_horizontal(), crop.flip_vertical()
         );
    }

    fn update_crop_pan(crop: &mut CropSettings, delta: Vec2, image_size: Vec2) {
        let norm_delta = Vec2::new(delta.x / image_size.x, delta.y / image_size.y);
        *crop = CropSettings::new(
             crop.crop_x() + norm_delta.x,
             crop.crop_y() + norm_delta.y,
             crop.crop_width(),
             crop.crop_height(),
             crop.rotation_90(),
             crop.angle(),
             crop.flip_horizontal(),
             crop.flip_vertical()
         );
    }
    
    // Public wrappers for compatibility if needed, but we handle internally now
    pub fn update_from_handle_drag(
        crop: &mut CropSettings,
        index: usize,
        delta: Vec2,
        image_size: Vec2,
        aspect_ratio: &AspectRatio,
    ) {
        Self::update_crop_handle(crop, index, delta, image_size, aspect_ratio);
    }
    
    pub fn update_from_crop_drag(
        crop: &mut CropSettings,
        delta: Vec2,
        image_size: Vec2,
    ) {
        Self::update_crop_pan(crop, delta, image_size);
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
