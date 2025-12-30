//! Intelligent Fill Module
//!
//! GPU-accelerated intelligent fill for rotation edges using
//! wgpu compute shaders and neural network inference.

mod processor;

pub use processor::{
    IntelligentFillProcessor, IntelligentFillRequest, IntelligentFillProcessResult,
};
