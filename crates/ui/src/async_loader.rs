// Async Thumbnail Loader and Image Processor Wrapper
// Delegates to infrastructure logic and handles UI-specific conversions

use std::sync::Arc;
use image::DynamicImage;
use eframe::egui::ColorImage;
use infrastructure::image_processing::async_loader::{
    AsyncImageProcessor as InfraAsyncImageProcessor,
    AsyncEditProcessor as InfraAsyncEditProcessor,
    ImageProcessRequest as InfraImageProcessRequest,
    EditRequest as InfraEditRequest,
};
use infrastructure::image_processing::histogram::HistogramData;
pub use infrastructure::image_processing::async_loader::{
    AsyncThumbnailLoader, ThumbnailRequest, ThumbnailResult,
};
use adapters::view_models::PhotoEdits;

// Re-export or redefine UI structs
// We keep flattened structs for API compatibility with UI code

/// Request to process an image (UI Version - Simplified)
#[derive(Debug, Clone)]
pub struct ImageProcessRequest {
    pub photo_id: String,
    pub path: String,
    pub max_preview_size: u32,
    pub edits: PhotoEdits,
}


impl ImageProcessRequest {
    pub fn to_infra(self) -> InfraImageProcessRequest {
        InfraImageProcessRequest {
            photo_id: self.photo_id,
            path: self.path,
            max_preview_size: self.max_preview_size,
            edits: self.edits,
        }
    }
}

/// Result of image processing (UI Version - with ColorImage)
pub struct ImageProcessResult {
    pub photo_id: String,
    pub preview: ColorImage,
    pub original_preview: DynamicImage,
    pub processed_image: DynamicImage,
    pub histogram: HistogramData,
    pub load_time_ms: f32,
}

/// Async image processor wrapper
pub struct AsyncImageProcessor {
    infra: InfraAsyncImageProcessor,
}

impl AsyncImageProcessor {
    pub fn new(preview_manager: Arc<infrastructure::cache::preview_manager::PreviewManager>) -> Self {
        Self {
            infra: InfraAsyncImageProcessor::new(preview_manager),
        }
    }

    pub fn request_process(&self, request: ImageProcessRequest) {
        self.infra.request_process(request.to_infra());
    }

    pub fn poll_result(&self) -> Option<ImageProcessResult> {
        if let Some(res) = self.infra.poll_result() {
            // Convert to ColorImage
            let preview = crate::image_processing::ImageProcessor::dynamic_to_color_image(&res.processed_image);
            
            Some(ImageProcessResult {
                photo_id: res.photo_id,
                preview,
                original_preview: res.original_preview,
                processed_image: res.processed_image,
                histogram: res.histogram,
                load_time_ms: res.load_time_ms,
            })
        } else {
            None
        }
    }

    pub fn is_processing(&self) -> bool {
        self.infra.is_processing()
    }

    pub fn processing_photo_id(&self) -> Option<String> {
        // Need to expose this in infra or remove if unused in UI
        // Assuming UI uses it for loading indicators
        // For now, return None or update infra
        // Infra has `processing` but it's private.
        // I should stick to public API.
        None 
    }
    
    // Legacy cache methods (if needed by UI components)
    // For now, returning false as cache is internal to infra
    pub fn is_in_cache(&self, _photo_id: &str) -> bool {
        false 
    }
    
    pub fn prefetch(&self, _photo_id: String, _path: String, _max_preview_size: u32) {
         // Prefetch not exposed in infra yet
    }
}


/// Request to apply edits (UI Wrapper - Simplified)
#[derive(Debug, Clone)]
pub struct EditRequest {
    pub request_id: u64,
    pub original: DynamicImage,
    pub edits: PhotoEdits,
}


impl EditRequest {
    pub fn to_infra(self) -> InfraEditRequest {
        InfraEditRequest {
            request_id: self.request_id,
            original: self.original,
            edits: self.edits,
        }
    }
}

/// Result of edit processing
pub struct EditResult {
    pub request_id: u64,
    pub processed: DynamicImage,
}

pub struct AsyncEditProcessor {
    infra: InfraAsyncEditProcessor,
}

impl AsyncEditProcessor {
    pub fn new() -> Self {
        Self {
            infra: InfraAsyncEditProcessor::new(),
        }
    }

    pub fn request_edit(&self, request: EditRequest) -> u64 {
        self.infra.request_edit(request.to_infra())
    }

    pub fn next_request_id(&self) -> u64 {
        self.infra.next_request_id()
    }

    pub fn poll_result(&self) -> Option<EditResult> {
        self.infra.poll_result().map(|res| EditResult {
            request_id: res.request_id,
            processed: res.processed,
        })
    }
}

impl Default for AsyncEditProcessor {
    fn default() -> Self {
        Self::new()
    }
}
