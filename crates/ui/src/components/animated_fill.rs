//! Animated fill patterns for placeholder rendering
//!
//! Provides elegant animated patterns (like zebra stripes) while
//! intelligent fill or other async operations are processing.

use egui::{Color32, Pos2, Rect, Ui};
use egui::epaint::{Mesh, Vertex};

/// Configuration for the animated zebra stripe pattern
pub struct ZebraPatternConfig {
    /// Width of each stripe in pixels
    pub stripe_width: f32,
    /// Primary color (dark stripe)
    pub color_dark: Color32,
    /// Secondary color (light stripe)
    pub color_light: Color32,
    /// Animation speed (pixels per second)
    pub speed: f32,
    /// Stripe angle in radians (default: 45 degrees = PI/4)
    pub angle: f32,
}

impl Default for ZebraPatternConfig {
    fn default() -> Self {
        Self {
            stripe_width: 20.0,
            color_dark: Color32::from_rgba_unmultiplied(40, 40, 50, 200),
            color_light: Color32::from_rgba_unmultiplied(60, 60, 80, 200),
            speed: 50.0,
            angle: std::f32::consts::FRAC_PI_4, // 45 degrees
        }
    }
}

impl ZebraPatternConfig {
    /// Creates a subtle dark theme variant
    pub fn dark_subtle() -> Self {
        Self {
            stripe_width: 16.0,
            color_dark: Color32::from_rgba_unmultiplied(30, 30, 35, 180),
            color_light: Color32::from_rgba_unmultiplied(50, 50, 60, 180),
            speed: 40.0,
            angle: std::f32::consts::FRAC_PI_4,
        }
    }
    
    /// Creates a more visible processing indicator
    pub fn processing_indicator() -> Self {
        Self {
            stripe_width: 24.0,
            color_dark: Color32::from_rgba_unmultiplied(20, 20, 30, 220),
            color_light: Color32::from_rgba_unmultiplied(45, 45, 65, 220),
            speed: 60.0,
            angle: std::f32::consts::FRAC_PI_4,
        }
    }
}

/// Renders an animated zebra stripe pattern within the given rectangle.
/// 
/// This creates diagonal stripes that animate smoothly, providing
/// visual feedback that processing is in progress.
/// 
/// # Arguments
/// * `ui` - The egui UI context
/// * `rect` - The rectangle to fill with the pattern
/// * `config` - Pattern configuration (colors, speed, etc.)
/// 
/// # Returns
/// Returns true to indicate the UI should request repaint for animation
pub fn render_animated_zebra(ui: &mut Ui, rect: Rect, config: &ZebraPatternConfig) -> bool {
    let time = ui.ctx().input(|i| i.time) as f32;
    let offset = (time * config.speed) % (config.stripe_width * 2.0);
    
    // Create mesh for the pattern
    let mut mesh = Mesh::default();
    
    // Calculate stripe parameters
    let (sin_a, cos_a) = config.angle.sin_cos();
    let _stripe_period = config.stripe_width * 2.0;
    
    // Diagonal length of the rect (for proper coverage)
    let diag = (rect.width().powi(2) + rect.height().powi(2)).sqrt();
    let num_stripes = (diag / config.stripe_width).ceil() as i32 + 4;
    
    // Center of the rectangle
    let center = rect.center();
    
    // Create a clip rect slightly larger than input
    let clip_rect = rect;
    ui.set_clip_rect(clip_rect);
    
    // Draw stripes
    for i in -num_stripes..num_stripes {
        let base_offset = i as f32 * config.stripe_width - offset;
        let is_dark = i % 2 == 0;
        let color = if is_dark { config.color_dark } else { config.color_light };
        
        // Calculate stripe corners with rotation
        // Stripe runs perpendicular to the angle
        let stripe_start = base_offset;
        let stripe_end = stripe_start + config.stripe_width;
        
        // Create rotated quad for the stripe
        let half_length = diag;
        
        // Points along the stripe direction (perpendicular to angle)
        let perp_x = -sin_a;
        let perp_y = cos_a;
        
        // Points along the stripe width direction (parallel to angle)
        let para_x = cos_a;
        let para_y = sin_a;
        
        // Four corners of the stripe quad
        let corners = [
            Pos2::new(
                center.x + stripe_start * para_x - half_length * perp_x,
                center.y + stripe_start * para_y - half_length * perp_y,
            ),
            Pos2::new(
                center.x + stripe_end * para_x - half_length * perp_x,
                center.y + stripe_end * para_y - half_length * perp_y,
            ),
            Pos2::new(
                center.x + stripe_end * para_x + half_length * perp_x,
                center.y + stripe_end * para_y + half_length * perp_y,
            ),
            Pos2::new(
                center.x + stripe_start * para_x + half_length * perp_x,
                center.y + stripe_start * para_y + half_length * perp_y,
            ),
        ];
        
        // Clip corners to rect bounds (simple approach - just add to mesh and let GPU clip)
        let base_idx = mesh.vertices.len() as u32;
        
        for corner in &corners {
            mesh.vertices.push(Vertex {
                pos: *corner,
                uv: Pos2::ZERO,
                color,
            });
        }
        
        // Two triangles for the quad
        mesh.indices.extend_from_slice(&[
            base_idx, base_idx + 1, base_idx + 2,
            base_idx, base_idx + 2, base_idx + 3,
        ]);
    }
    
    // Paint the mesh
    ui.painter().add(egui::Shape::mesh(mesh));
    
    // Request repaint for animation
    ui.ctx().request_repaint();
    true
}

/// Simplified version - renders zebra stripes directly with painter polygons
/// Guarantees full coverage of the rect with animated diagonal stripes
pub fn render_zebra_simple(ui: &Ui, rect: Rect, config: &ZebraPatternConfig) {
    let time = ui.ctx().input(|i| i.time) as f32;
    let offset = (time * config.speed) % (config.stripe_width * 2.0);
    
    let painter = ui.painter();
    
    // Use clip rect for the zebra pattern
    let clip = painter.clip_rect();
    let final_clip = clip.intersect(rect);
    
    // Create a sub-painter with our clip
    let painter = painter.with_clip_rect(final_clip);
    
    // Draw base color first (this ensures full coverage)
    painter.rect_filled(rect, 0.0, config.color_dark);
    
    // Calculate diagonal for proper stripe coverage
    // We need enough stripes to cover from corner to corner
    let diag = (rect.width().powi(2) + rect.height().powi(2)).sqrt();
    let stripe_period = config.stripe_width * 2.0;
    let num_stripes = (diag * 2.0 / stripe_period).ceil() as i32 + 4;
    
    let (sin_a, cos_a) = config.angle.sin_cos();
    let center = rect.center();
    
    // Direction vectors for stripe construction
    // para = direction stripes move (parallel to angle)
    // perp = direction stripes extend (perpendicular to angle)
    let para = egui::vec2(cos_a, sin_a);
    let perp = egui::vec2(-sin_a, cos_a);
    
    // Draw light stripes over the dark base
    // Start from -half of stripes to +half, centered on the rect
    for i in -num_stripes..num_stripes {
        // Each stripe starts at base_pos along the parallel direction from center
        let base_pos = i as f32 * stripe_period + offset - diag;
        
        // Calculate stripe corners as a parallelogram
        // The stripe extends "infinitely" in the perpendicular direction (we use diag * 2)
        let half_extend = diag * 1.5;
        
        let stripe_start = center + para * base_pos;
        let stripe_end = center + para * (base_pos + config.stripe_width);
        
        let points = [
            Pos2::new(
                stripe_start.x - perp.x * half_extend,
                stripe_start.y - perp.y * half_extend,
            ),
            Pos2::new(
                stripe_end.x - perp.x * half_extend,
                stripe_end.y - perp.y * half_extend,
            ),
            Pos2::new(
                stripe_end.x + perp.x * half_extend,
                stripe_end.y + perp.y * half_extend,
            ),
            Pos2::new(
                stripe_start.x + perp.x * half_extend,
                stripe_start.y + perp.y * half_extend,
            ),
        ];
        
        painter.add(egui::Shape::convex_polygon(
            points.to_vec(),
            config.color_light,
            egui::Stroke::NONE,
        ));
    }
    
    // Request repaint for continuous animation
    ui.ctx().request_repaint();
}

/// Compact function to draw processing indicator zebra pattern
/// Uses a gradient fade at edges for a polished look
pub fn draw_processing_zebra(ui: &Ui, rect: Rect) {
    let config = ZebraPatternConfig::processing_indicator();
    render_zebra_simple(ui, rect, &config);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_defaults() {
        let config = ZebraPatternConfig::default();
        assert!(config.stripe_width > 0.0);
        assert!(config.speed > 0.0);
    }
    
    #[test]
    fn test_dark_subtle_config() {
        let config = ZebraPatternConfig::dark_subtle();
        assert!(config.stripe_width > 0.0);
        assert!(config.color_dark.a() > 0);
    }
}
