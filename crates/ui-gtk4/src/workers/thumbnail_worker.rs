//! Thumbnail Worker
//!
//! Background worker for loading thumbnails using Rayon for parallel processing.

use std::sync::Arc;
use relm4::Worker;
use rayon::prelude::*;

use infrastructure::cache::preview_manager::PreviewManager;

/// Thumbnail loading worker
pub struct ThumbnailWorker {
    preview_manager: Arc<PreviewManager>,
}

/// Request to load thumbnails
#[derive(Debug, Clone)]
pub struct ThumbnailRequest {
    pub photo_ids: Vec<String>,
}

/// Result of thumbnail loading
/// Returns bytes instead of Pixbuf because Pixbuf is not Send-safe
#[derive(Debug, Clone)]
pub struct ThumbnailResult {
    pub photo_id: String,
    pub bytes: Vec<u8>,
}

impl Worker for ThumbnailWorker {
    type Init = Arc<PreviewManager>;
    type Input = ThumbnailRequest;
    type Output = ThumbnailResult;

    fn init(preview_manager: Self::Init, _sender: relm4::ComponentSender<Self>) -> Self {
        Self { preview_manager }
    }

    fn update(&mut self, request: Self::Input, sender: relm4::ComponentSender<Self>) {
        let preview_manager = self.preview_manager.clone();

        // Process thumbnails in parallel using Rayon
        request.photo_ids.par_iter().for_each(|photo_id| {
            // Try to load from PreviewManager cache
            if let Some(dynamic_image) = preview_manager.get_thumbnail(photo_id) {
                // Convert DynamicImage to PNG bytes for thread-safe transfer
                let mut bytes = Vec::new();
                if let Ok(_) = dynamic_image.write_to(
                    &mut std::io::Cursor::new(&mut bytes),
                    image::ImageFormat::Png,
                ) {
                    let _ = sender.output(ThumbnailResult {
                        photo_id: photo_id.clone(),
                        bytes,
                    });
                } else {
                    eprintln!("Failed to encode thumbnail for: {}", photo_id);
                }
            } else {
                eprintln!("Thumbnail not found in cache for: {}", photo_id);
            }
        });
    }
}
