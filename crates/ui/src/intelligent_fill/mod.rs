//! Intelligent Fill Module
//!
//! GPU-accelerated intelligent fill for rotation edges using
//! wgpu compute shaders and neural network inference.

mod processor;
mod mask_generator;

pub use processor::{
    IntelligentFillProcessor, IntelligentFillRequest, IntelligentFillProcessResult,
};
pub use mask_generator::MaskGenerator;
