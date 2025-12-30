// GPU Image Processor Wrapper
// Delegates to infrastructure logic and handles UI-specific conversions


use image::DynamicImage;
use eframe::egui::ColorImage;
use infrastructure::image_processing::gpu::GpuImageProcessor as InfraGpuImageProcessor;

// Re-export types used by app.rs
pub use infrastructure::image_processing::gpu::{GpuProcessRequest, GpuEditParams};

/// Result of GPU processing for UI (includes texture)
pub struct GpuProcessResult {
    pub request_id: u64,
    pub processed_image: DynamicImage,
    pub preview: ColorImage,
    pub process_time_ms: f32,
}

/// GPU-accelerated image processor wrapper
pub struct GpuImageProcessor {
    infra_processor: InfraGpuImageProcessor,
}

impl GpuImageProcessor {
    /// Create a new GPU image processor wrapper
    pub fn new() -> Self {
        Self {
            infra_processor: InfraGpuImageProcessor::new(),
        }
    }

    /// Request GPU processing (non-blocking)
    pub fn request_process(
        &self,
        request: GpuProcessRequest
    ) -> u64 {
        self.infra_processor.request_process(request)
    }

    /// Generate a new request ID
    pub fn next_request_id(&self) -> u64 {
        self.infra_processor.next_request_id()
    }

    /// Poll for completed result (non-blocking)
    pub fn poll_result(&self) -> Option<GpuProcessResult> {
        if let Some(infra_result) = self.infra_processor.poll_result() {
            let preview = crate::image_processing::ImageProcessor::dynamic_to_color_image(&infra_result.processed_image);
            
            Some(GpuProcessResult {
                request_id: infra_result.request_id,
                processed_image: infra_result.processed_image,
                preview,
                process_time_ms: infra_result.process_time_ms,
            })
        } else {
            None
        }
    }

    /// Check if GPU is available
    pub fn is_gpu_available(&self) -> bool {
        self.infra_processor.is_gpu_available()
    }
}

impl Default for GpuImageProcessor {
    fn default() -> Self {
        Self::new()
    }
}

