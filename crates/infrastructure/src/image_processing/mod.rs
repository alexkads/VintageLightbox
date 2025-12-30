//! # Image Processing Module
//!
//! Algoritmos de processamento de imagem independentes de framework UI.
//! Inclui ajustes de cor, redimensionamento, crop e mais.

pub mod algorithms;
pub mod gpu;
pub mod histogram;
pub mod async_loader;
pub mod intelligent_fill;

pub use algorithms::ImageAlgorithms;
pub use gpu::GpuImageProcessor;
pub use histogram::HistogramData;
