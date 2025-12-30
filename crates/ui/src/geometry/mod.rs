//! Geometry utilities for rendering
//!
//! This module contains geometric algorithms used for image rendering,
//! such as polygon clipping for rotated images.

mod polygon_clip;
mod uv_mapping;

pub use polygon_clip::{ClipVertex, clip_polygon_to_uv_bounds};
pub use uv_mapping::{calculate_crop_uvs, calculate_full_image_uvs};
