use image::DynamicImage;
use domain::value_objects::PhotoEdits;
use std::sync::Arc;

/// Request to process an image on GPU (Adapter Layer DTO)
#[derive(Clone)]
pub struct GpuProcessRequest {
    pub request_id: u64,
    pub image_data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub params: PhotoEdits, // Using Domain Object directly
}

/// Result of GPU processing
pub struct GpuProcessResult {
    pub request_id: u64,
    pub processed_image: DynamicImage,
    pub process_time_ms: f32,
}

#[cfg_attr(test, mockall::automock)]
pub trait GpuProcessingService {
    /// Request GPU processing (non-blocking)
    fn request_process(&self, request: GpuProcessRequest) -> u64;

    /// Poll for completed result (non-blocking)
    fn poll_result(&self) -> Option<GpuProcessResult>;

    /// Check if GPU is available
    fn is_gpu_available(&self) -> bool;

    /// Generate a new request ID
    fn next_request_id(&self) -> u64;
}
