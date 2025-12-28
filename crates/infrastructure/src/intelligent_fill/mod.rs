//! Intelligent Fill Module
//!
//! Implementação do serviço de preenchimento inteligente usando
//! ONNX Runtime para inferência de modelo de inpainting.

mod model_loader;
mod onnx_inpainter;

pub use model_loader::{ModelLoader, ModelInfo};
pub use onnx_inpainter::OnnxInpainterImpl;
