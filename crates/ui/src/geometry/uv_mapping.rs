use egui::{Pos2, Vec2, pos2, vec2};
use adapters::view_models::CropSettings;

/// Calculate UV coordinates for the "Neutral Viewer" mode (Lightroom style).
/// The Viewer renders the final cropped result as a straight rectangle.
/// We calculate the UV coordinates of the 4 corners of the crop in the original texture,
/// accounting for rotation, aspect ratio, and flips.
pub fn calculate_crop_uvs(
    crop: &CropSettings,
    rotated_texture_size: Vec2,
) -> [Pos2; 4] {
    // 1. Define corners of the Crop Window in Frame Space (0.0 - 1.0)
    let cx = crop.crop_x();
    let cy = crop.crop_y();
    let cw = crop.crop_width();
    let ch = crop.crop_height();

    let corners_frame = [
        pos2(cx, cy),           // Top-Left
        pos2(cx + cw, cy),      // Top-Right
        pos2(cx + cw, cy + ch), // Bottom-Right
        pos2(cx, cy + ch),      // Bottom-Left
    ];

    // 2. Map Frame Space -> Original Texture UV Space
    let angle_rad = crop.angle().to_radians();
    let aspect = rotated_texture_size.x / rotated_texture_size.y;
    let center = pos2(0.5, 0.5);

    let mut uvs = [Pos2::ZERO; 4];
    
    for (i, &p_frame) in corners_frame.iter().enumerate() {
        // A. Center relative to 0.5
        let p_centered = p_frame - center;

        // B. Correct for Aspect Ratio (to rotate geometrically correct)
        let p_phys = vec2(p_centered.x * aspect, p_centered.y);

        // C. Inverse Rotate by 'Angle' (Frame -> Image Content)
        let (sin, cos) = (-angle_rad).sin_cos();
        let p_rot = vec2(
            p_phys.x * cos - p_phys.y * sin,
            p_phys.x * sin + p_phys.y * cos
        );

        // D. Restore Aspect Ratio normalization
        let p_rot_norm = vec2(p_rot.x / aspect, p_rot.y);
        
        // E. Uncenter -> UV in Rotated-90 Space
        let uv_r90 = center + p_rot_norm;

        // F. Undo Rotation 90 (Map Rotated-90 Space -> Original Space)
        let uv_orig = match crop.rotation_90() % 4 {
            0 => uv_r90,
            1 => pos2(uv_r90.y, 1.0 - uv_r90.x), // 90 CW
            2 => pos2(1.0 - uv_r90.x, 1.0 - uv_r90.y), // 180
            3 => pos2(1.0 - uv_r90.y, uv_r90.x), // 270 CW (90 CCW)
            _ => uv_r90,
        };

        // G. Undo Flips (Map Final UV -> Source UV)
        let mut final_u = uv_orig.x;
        let mut final_v = uv_orig.y;

        if crop.flip_horizontal() { final_u = 1.0 - final_u; }
        if crop.flip_vertical() { final_v = 1.0 - final_v; }

        uvs[i] = pos2(final_u, final_v);
    }

    uvs
}

/// Calculate UV coordinates for the full image in Edit Mode (rotated/flipped).
/// Returns UVs for the 4 corners of the full image.
pub fn calculate_full_image_uvs(
    crop: &CropSettings,
    texture_aspect: f32, // width / height
) -> [Pos2; 4] {
    let angle_rad = crop.angle().to_radians();
    let center_uv = pos2(0.5, 0.5);

    // Map screen corners (0..1) to UVs accounting for rotation
    let frame_corners = [
        pos2(0.0, 0.0), // Top-Left
        pos2(1.0, 0.0), // Top-Right
        pos2(1.0, 1.0), // Bottom-Right
        pos2(0.0, 1.0), // Bottom-Left
    ];

    let mut uvs = [Pos2::ZERO; 4];
    for (i, &p_frame) in frame_corners.iter().enumerate() {
        // Center relative to 0.5
        let p_centered = p_frame - center_uv;

        // Correct for aspect ratio
        let p_phys = vec2(p_centered.x * texture_aspect, p_centered.y);

        // Inverse rotate by angle (Frame -> Image Content)
        let (sin, cos) = (-angle_rad).sin_cos();
        let p_rot = vec2(
            p_phys.x * cos - p_phys.y * sin,
            p_phys.x * sin + p_phys.y * cos
        );

        // Restore aspect ratio normalization
        let p_rot_norm = vec2(p_rot.x / texture_aspect, p_rot.y);

        // Uncenter -> UV in rotated space
        let mut uv = center_uv + p_rot_norm;

        // Apply rotation_90 transformation
        uv = match crop.rotation_90() % 4 {
            0 => uv,
            1 => pos2(uv.y, 1.0 - uv.x), // 90 CW
            2 => pos2(1.0 - uv.x, 1.0 - uv.y), // 180
            3 => pos2(1.0 - uv.y, uv.x), // 270 CW
            _ => uv,
        };

        // Apply flips
        if crop.flip_horizontal() { uv.x = 1.0 - uv.x; }
        if crop.flip_vertical() { uv.y = 1.0 - uv.y; }

        uvs[i] = uv;
    }

    uvs
}
