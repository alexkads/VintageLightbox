use domain::value_objects::CropSettings;
use egui::{Color32, Rect, Vec2};

/// Renders a thumbnail with support for Crop, Rotation (Mesh-based), and Aspect Ratio fitting.
pub fn render_thumbnail(
    ui: &mut egui::Ui,
    rect: Rect,
    texture: &egui::TextureHandle,
    crop_settings: Option<&CropSettings>,
) {
    let texture_size = Vec2::new(texture.size()[0] as f32, texture.size()[1] as f32);

    // 1. Calculate Effective Size (account for 90-degree rotation)
    // If we have crop settings, the "Image" we are viewing is the Cropped result.
    // The Aspect Ratio of the VIEW is crop_width / crop_height (relative to frame).
    // The Frame Aspect Ratio depends on Rotation90.

    // Actually, following ImageViewer logic:
    // We want to fit the "Straightened Crop Result" into 'rect'.

    let (target_aspect, rotated_texture_size) = if let Some(crop) = crop_settings {
        // Frame Aspect Ratio
        let frame_aspect = if crop.rotation_90() % 2 != 0 {
            texture_size.y / texture_size.x
        } else {
            texture_size.x / texture_size.y
        };

        let rot_tex_size = if crop.rotation_90() % 2 != 0 {
            Vec2::new(texture_size.y, texture_size.x)
        } else {
            texture_size
        };

        // Crop Aspect Ratio (The visible window)
        // crop w/h are normalized to Frame dimensions.
        // So Width in pixels = crop.w * FrameW.
        // Aspect = (crop.w * FrameW) / (crop.h * FrameH)
        //        = (crop.w / crop.h) * FrameAspect
        let view_aspect = (crop.crop_width() / crop.crop_height()) * frame_aspect;

        (view_aspect, rot_tex_size)
    } else {
        // No crop: Just fits the texture.
        let view_aspect = texture_size.x / texture_size.y;
        (view_aspect, texture_size)
    };

    // 2. Calculate Display Rect (Center Inside)
    let cell_aspect = rect.width() / rect.height();
    let display_rect = if target_aspect > cell_aspect {
        // Limited by Width
        let h = rect.width() / target_aspect;
        let y = rect.center().y - h / 2.0;
        Rect::from_min_size(egui::pos2(rect.min.x, y), Vec2::new(rect.width(), h))
    } else {
        // Limited by Height
        let w = rect.height() * target_aspect;
        let x = rect.center().x - w / 2.0;
        Rect::from_min_size(egui::pos2(x, rect.min.y), Vec2::new(w, rect.height()))
    };

    // 3. Render
    if let Some(crop) = crop_settings {
        // Mesh Rendering (Copied from ImageViewer "Neutral Viewer")
        use egui::epaint::{Mesh, Vertex};

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

        let angle_rad = crop.angle().to_radians();
        let aspect = rotated_texture_size.x / rotated_texture_size.y;
        let center = egui::pos2(0.5, 0.5);

        let mut uvs = [egui::Pos2::ZERO; 4];

        for (i, &p_frame) in corners_frame.iter().enumerate() {
            let p_centered = p_frame - center;
            let p_phys = egui::vec2(p_centered.x * aspect, p_centered.y);
            let (sin, cos) = (-angle_rad).sin_cos();
            let p_rot = egui::vec2(
                p_phys.x * cos - p_phys.y * sin,
                p_phys.x * sin + p_phys.y * cos,
            );
            let p_rot_norm = egui::vec2(p_rot.x / aspect, p_rot.y);
            let uv_r90 = center + p_rot_norm;

            let uv_orig = match crop.rotation_90() % 4 {
                0 => uv_r90,
                1 => egui::pos2(uv_r90.y, 1.0 - uv_r90.x),
                2 => egui::pos2(1.0 - uv_r90.x, 1.0 - uv_r90.y),
                3 => egui::pos2(1.0 - uv_r90.y, uv_r90.x),
                _ => uv_r90,
            };

            let mut final_u = uv_orig.x;
            let mut final_v = uv_orig.y;

            if crop.flip_horizontal() {
                final_u = 1.0 - final_u;
            }
            if crop.flip_vertical() {
                final_v = 1.0 - final_v;
            }

            uvs[i] = egui::pos2(final_u, final_v);
        }

        let mut mesh = Mesh::with_texture(texture.id());
        let screen_corners = [
            display_rect.min,
            egui::pos2(display_rect.max.x, display_rect.min.y),
            display_rect.max,
            egui::pos2(display_rect.min.x, display_rect.max.y),
        ];

        for (i, &pos) in screen_corners.iter().enumerate() {
            mesh.vertices.push(Vertex {
                pos,
                uv: uvs[i],
                color: Color32::WHITE,
            });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);

        ui.painter().add(egui::Shape::mesh(mesh));
    } else {
        // Simple Image Render
        egui::Image::new(texture).paint_at(ui, display_rect);
    }
}
