//! Polygon Clipping using Sutherland-Hodgman Algorithm
//!
//! This module provides polygon clipping functionality for rendering rotated images.
//! When an image is rotated, the UV coordinates may extend beyond the valid [0,1] range,
//! causing GPU edge-clamping artifacts. This clipper ensures only valid texture regions
//! are rendered.

use egui::Pos2;

/// Vertex with screen position and texture UV coordinates for clipping operations
#[derive(Clone, Copy, Debug)]
pub struct ClipVertex {
    /// Screen position (where the vertex appears on screen)
    pub pos: Pos2,
    /// Texture UV coordinates (where to sample from the texture)
    pub uv: Pos2,
}

impl ClipVertex {
    /// Create a new ClipVertex with the given screen position and UV coordinates
    pub fn new(pos: Pos2, uv: Pos2) -> Self {
        Self { pos, uv }
    }
}

/// Linearly interpolate between two vertices
///
/// # Arguments
/// * `a` - Start vertex
/// * `b` - End vertex
/// * `t` - Interpolation factor (0.0 = a, 1.0 = b)
fn lerp_vertex(a: &ClipVertex, b: &ClipVertex, t: f32) -> ClipVertex {
    ClipVertex {
        pos: Pos2::new(
            a.pos.x + (b.pos.x - a.pos.x) * t,
            a.pos.y + (b.pos.y - a.pos.y) * t,
        ),
        uv: Pos2::new(
            a.uv.x + (b.uv.x - a.uv.x) * t,
            a.uv.y + (b.uv.y - a.uv.y) * t,
        ),
    }
}

/// Clip polygon against a single edge using Sutherland-Hodgman algorithm
///
/// # Arguments
/// * `vertices` - Input polygon vertices
/// * `inside` - Function that returns true if a vertex is inside the clipping edge
/// * `intersect_t` - Function that returns the interpolation factor for edge intersection
fn clip_against_edge<F, G>(vertices: &[ClipVertex], inside: F, intersect_t: G) -> Vec<ClipVertex>
where
    F: Fn(&ClipVertex) -> bool,
    G: Fn(&ClipVertex, &ClipVertex) -> f32,
{
    if vertices.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::with_capacity(vertices.len() + 1);
    let n = vertices.len();

    for i in 0..n {
        let current = &vertices[i];
        let next = &vertices[(i + 1) % n];

        let current_inside = inside(current);
        let next_inside = inside(next);

        if current_inside {
            output.push(*current);
            if !next_inside {
                // Edge exits the clipping region: add intersection point
                let t = intersect_t(current, next);
                output.push(lerp_vertex(current, next, t));
            }
        } else if next_inside {
            // Edge enters the clipping region: add intersection point
            let t = intersect_t(current, next);
            output.push(lerp_vertex(current, next, t));
        }
        // If both vertices are outside, add nothing
    }

    output
}

/// Clip polygon to UV bounds [0,1] x [0,1]
///
/// Uses the Sutherland-Hodgman algorithm to clip a polygon against all four edges
/// of the unit square. This ensures that all UV coordinates in the output polygon
/// are within valid texture bounds.
///
/// # Arguments
/// * `vertices` - Input polygon vertices with screen positions and UV coordinates
///
/// # Returns
/// A new polygon with all vertices having UV coordinates within [0,1].
/// The polygon may have more vertices than the input (up to 8 for a quad clipped
/// against all 4 edges). Returns an empty vector if the polygon is entirely
/// outside the clipping region.
///
/// # Example
/// ```ignore
/// let input = vec![
///     ClipVertex::new(pos2(0.0, 0.0), pos2(-0.1, 0.0)),  // UV.x < 0
///     ClipVertex::new(pos2(100.0, 0.0), pos2(1.1, 0.0)), // UV.x > 1
///     ClipVertex::new(pos2(100.0, 100.0), pos2(1.1, 1.0)),
///     ClipVertex::new(pos2(0.0, 100.0), pos2(-0.1, 1.0)),
/// ];
/// let clipped = clip_polygon_to_uv_bounds(&input);
/// // clipped will have vertices with UV.x clamped to [0,1]
/// ```
pub fn clip_polygon_to_uv_bounds(vertices: &[ClipVertex]) -> Vec<ClipVertex> {
    let mut result = vertices.to_vec();

    // Clip against u = 0 (left edge): keep vertices where uv.x >= 0
    result = clip_against_edge(
        &result,
        |v| v.uv.x >= 0.0,
        |a, b| (0.0 - a.uv.x) / (b.uv.x - a.uv.x),
    );

    // Clip against u = 1 (right edge): keep vertices where uv.x <= 1
    result = clip_against_edge(
        &result,
        |v| v.uv.x <= 1.0,
        |a, b| (1.0 - a.uv.x) / (b.uv.x - a.uv.x),
    );

    // Clip against v = 0 (top edge): keep vertices where uv.y >= 0
    result = clip_against_edge(
        &result,
        |v| v.uv.y >= 0.0,
        |a, b| (0.0 - a.uv.y) / (b.uv.y - a.uv.y),
    );

    // Clip against v = 1 (bottom edge): keep vertices where uv.y <= 1
    result = clip_against_edge(
        &result,
        |v| v.uv.y <= 1.0,
        |a, b| (1.0 - a.uv.y) / (b.uv.y - a.uv.y),
    );

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn test_no_clipping_needed() {
        // Polygon entirely within bounds
        let input = vec![
            ClipVertex::new(pos2(0.0, 0.0), pos2(0.0, 0.0)),
            ClipVertex::new(pos2(100.0, 0.0), pos2(1.0, 0.0)),
            ClipVertex::new(pos2(100.0, 100.0), pos2(1.0, 1.0)),
            ClipVertex::new(pos2(0.0, 100.0), pos2(0.0, 1.0)),
        ];

        let clipped = clip_polygon_to_uv_bounds(&input);

        assert_eq!(clipped.len(), 4);
        for v in &clipped {
            assert!(v.uv.x >= 0.0 && v.uv.x <= 1.0);
            assert!(v.uv.y >= 0.0 && v.uv.y <= 1.0);
        }
    }

    #[test]
    fn test_clip_left_edge() {
        // Polygon extends past left edge (uv.x < 0)
        let input = vec![
            ClipVertex::new(pos2(0.0, 0.0), pos2(-0.5, 0.0)),
            ClipVertex::new(pos2(100.0, 0.0), pos2(0.5, 0.0)),
            ClipVertex::new(pos2(100.0, 100.0), pos2(0.5, 1.0)),
            ClipVertex::new(pos2(0.0, 100.0), pos2(-0.5, 1.0)),
        ];

        let clipped = clip_polygon_to_uv_bounds(&input);

        assert!(clipped.len() >= 3);
        for v in &clipped {
            assert!(v.uv.x >= 0.0, "UV.x should be >= 0, got {}", v.uv.x);
        }
    }

    #[test]
    fn test_clip_all_edges() {
        // Polygon extends past all edges
        let input = vec![
            ClipVertex::new(pos2(0.0, 0.0), pos2(-0.2, -0.2)),
            ClipVertex::new(pos2(100.0, 0.0), pos2(1.2, -0.2)),
            ClipVertex::new(pos2(100.0, 100.0), pos2(1.2, 1.2)),
            ClipVertex::new(pos2(0.0, 100.0), pos2(-0.2, 1.2)),
        ];

        let clipped = clip_polygon_to_uv_bounds(&input);

        assert!(clipped.len() >= 3);
        for v in &clipped {
            assert!(v.uv.x >= 0.0 && v.uv.x <= 1.0, "UV.x out of bounds: {}", v.uv.x);
            assert!(v.uv.y >= 0.0 && v.uv.y <= 1.0, "UV.y out of bounds: {}", v.uv.y);
        }
    }

    #[test]
    fn test_entirely_outside() {
        // Polygon entirely outside bounds
        let input = vec![
            ClipVertex::new(pos2(0.0, 0.0), pos2(2.0, 2.0)),
            ClipVertex::new(pos2(100.0, 0.0), pos2(3.0, 2.0)),
            ClipVertex::new(pos2(100.0, 100.0), pos2(3.0, 3.0)),
            ClipVertex::new(pos2(0.0, 100.0), pos2(2.0, 3.0)),
        ];

        let clipped = clip_polygon_to_uv_bounds(&input);

        assert!(clipped.is_empty() || clipped.len() < 3);
    }
}
