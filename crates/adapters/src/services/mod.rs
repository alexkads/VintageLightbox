//! # Services Module
//!
//! Serviços de aplicação para operações comuns.

pub mod navigation_service;
pub mod image_processing_service;
pub mod gpu_processing_service;

pub use navigation_service::NavigationService;
pub use image_processing_service::ImageProcessingService;
pub use gpu_processing_service::{GpuProcessingService, GpuProcessRequest, GpuProcessResult};
