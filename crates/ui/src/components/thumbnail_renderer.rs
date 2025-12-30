use egui::{Color32, Rect, Vec2, pos2};
use domain::value_objects::{CropSettings, RotationFillMode};
use crate::geometry::{ClipVertex, clip_polygon_to_uv_bounds};
use crate::components::animated_fill::{render_zebra_simple, ZebraPatternConfig};
use crate::geometry::calculate_crop_uvs;
use crate::rendering::traits::ImageRenderer;
use crate::rendering::adapters::EguiRenderer;
use crate::rendering::primitives::{MeshData, Vertex};

/// Renders a thumbnail with support for Crop, Rotation (Mesh-based), Fill Mode, and Aspect Ratio fitting.
pub fn render_thumbnail(
    ui: &mut egui::Ui,
    rect: Rect,
    texture: &egui::TextureHandle,
    crop_settings: Option<&CropSettings>,
) {
    let mut renderer = EguiRenderer::new(ui);
    let texture_size = Vec2::new(texture.size()[0] as f32, texture.size()[1] as f32);

    // 1. Calculate Effective Size & Aspect Ratio (Straightened Crop Result)
    let (target_aspect, rotated_texture_size) = if let Some(crop) = crop_settings {
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

        let view_aspect = (crop.crop_width() / crop.crop_height()) * frame_aspect;
        (view_aspect, rot_tex_size)
    } else {
        let view_aspect = texture_size.x / texture_size.y;
        (view_aspect, texture_size)
    };

    // 2. Calculate Display Rect (Center Inside)
    let cell_aspect = rect.width() / rect.height();
    let display_rect = if target_aspect > cell_aspect {
        // Limited by Width
        let h = rect.width() / target_aspect;
        let y = rect.center().y - h / 2.0;
        Rect::from_min_size(pos2(rect.min.x, y), Vec2::new(rect.width(), h))
    } else {
        // Limited by Height
        let w = rect.height() * target_aspect;
        let x = rect.center().x - w / 2.0;
        Rect::from_min_size(pos2(x, rect.min.y), Vec2::new(w, rect.height()))
    };

    // 3. Render
    if let Some(crop) = crop_settings {
        // A. Calculate UVs
        let uvs = calculate_crop_uvs(crop, rotated_texture_size);

        let screen_corners = [
            display_rect.min,
            pos2(display_rect.max.x, display_rect.min.y),
            display_rect.max,
            pos2(display_rect.min.x, display_rect.max.y),
        ];

        // B. Draw Background Fill (if angle != 0)
        if crop.angle() != 0.0 {
            let fill_color = match crop.fill_mode() {
                RotationFillMode::Black => Color32::BLACK,
                RotationFillMode::White => Color32::WHITE,
                RotationFillMode::Transparent => Color32::TRANSPARENT,
                RotationFillMode::Intelligent => {
                    // Show subtle animated zebra for thumbnails
                    // Note: render_zebra_simple still uses ui directly, we should ideally abstract it too later
                    render_zebra_simple(renderer.ui, display_rect, &ZebraPatternConfig::dark_subtle());
                    Color32::TRANSPARENT
                }
                RotationFillMode::ShrinkToFit => Color32::TRANSPARENT,
            };

            if fill_color != Color32::TRANSPARENT {
                renderer.draw_rect(display_rect, fill_color);
            }
        }

        // C. Clip Polygon
        let input_vertices: Vec<ClipVertex> = screen_corners
            .iter()
            .zip(uvs.iter())
            .map(|(&pos, &uv)| ClipVertex::new(pos, uv))
            .collect();

        let clipped = clip_polygon_to_uv_bounds(&input_vertices);

        // D. Render Mesh
        if clipped.len() >= 3 {
            let mut indices = Vec::new();
            // Fan triangulation
            for i in 1..(clipped.len() - 1) {
                indices.push(0);
                indices.push(i as u32);
                indices.push((i + 1) as u32);
            }

            let vertices = clipped.iter().map(|v| Vertex {
                pos: v.pos,
                uv: v.uv,
                color: Color32::WHITE,
            }).collect();

            let mesh_data = MeshData {
                vertices,
                indices,
                texture_id: texture.id(),
            };
            
            renderer.draw_mesh(mesh_data);
        }

    } else {
        // Simple Image Render
        renderer.draw_image(display_rect, texture.id(), Rect::from_min_max(pos2(0.0,0.0), pos2(1.0,1.0)));
    }
}
