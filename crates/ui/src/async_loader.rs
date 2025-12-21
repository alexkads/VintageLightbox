// Async Thumbnail Loader
// Uses Rayon for parallel thumbnail loading without blocking the UI

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use parking_lot::Mutex;
use rayon::prelude::*;
use image::DynamicImage;

/// Request to load a thumbnail
#[derive(Clone)]
pub struct ThumbnailRequest {
    pub photo_id: String,
    pub path: String,
}

/// Result of loading a thumbnail
pub struct ThumbnailResult {
    pub photo_id: String,
    pub image: DynamicImage,
}

/// Async thumbnail loader that processes thumbnails in background threads
pub struct AsyncThumbnailLoader {
    /// Sender for thumbnail requests
    request_sender: Sender<Vec<ThumbnailRequest>>,
    /// Receiver for completed thumbnails
    result_receiver: Receiver<ThumbnailResult>,
    /// Set of photo IDs currently being loaded (to avoid duplicate requests)
    loading: Arc<Mutex<std::collections::HashSet<String>>>,
    /// Set of photo IDs that have been requested (to avoid re-requesting)
    requested: std::collections::HashSet<String>,
}

impl AsyncThumbnailLoader {
    /// Create a new async thumbnail loader
    pub fn new() -> Self {
        let (request_sender, request_receiver) = channel::<Vec<ThumbnailRequest>>();
        let (result_sender, result_receiver) = channel::<ThumbnailResult>();
        let loading = Arc::new(Mutex::new(std::collections::HashSet::new()));
        let loading_clone = loading.clone();

        // Spawn background thread for processing thumbnail requests
        std::thread::spawn(move || {
            Self::background_loader(request_receiver, result_sender, loading_clone);
        });

        Self {
            request_sender,
            result_receiver,
            loading,
            requested: std::collections::HashSet::new(),
        }
    }

    /// Background thread that processes thumbnail requests using Rayon
    fn background_loader(
        receiver: Receiver<Vec<ThumbnailRequest>>,
        sender: Sender<ThumbnailResult>,
        loading: Arc<Mutex<std::collections::HashSet<String>>>,
    ) {
        while let Ok(requests) = receiver.recv() {
            // Mark all as loading
            {
                let mut loading_guard = loading.lock();
                for req in &requests {
                    loading_guard.insert(req.photo_id.clone());
                }
            }

            // Process in parallel using Rayon
            let results: Vec<_> = requests
                .par_iter()
                .filter_map(|req| {
                    match image::open(&req.path) {
                        Ok(img) => Some(ThumbnailResult {
                            photo_id: req.photo_id.clone(),
                            image: img,
                        }),
                        Err(e) => {
                            eprintln!("Failed to load thumbnail {}: {}", req.path, e);
                            None
                        }
                    }
                })
                .collect();

            // Send results and remove from loading set
            {
                let mut loading_guard = loading.lock();
                for result in results {
                    loading_guard.remove(&result.photo_id);
                    let _ = sender.send(result);
                }
            }
        }
    }

    /// Request thumbnails to be loaded (non-blocking)
    /// Only requests thumbnails that haven't been requested yet
    pub fn request_thumbnails(&mut self, requests: Vec<ThumbnailRequest>) {
        // Filter out already requested or loading thumbnails
        let new_requests: Vec<_> = {
            let loading_guard = self.loading.lock();
            requests
                .into_iter()
                .filter(|req| {
                    !self.requested.contains(&req.photo_id) && 
                    !loading_guard.contains(&req.photo_id)
                })
                .collect()
        };

        if new_requests.is_empty() {
            return;
        }

        // Mark as requested
        for req in &new_requests {
            self.requested.insert(req.photo_id.clone());
        }

        // Send to background thread
        let _ = self.request_sender.send(new_requests);
    }

    /// Poll for completed thumbnails (non-blocking)
    /// Returns all thumbnails that have finished loading
    pub fn poll_results(&self) -> Vec<ThumbnailResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.result_receiver.try_recv() {
            results.push(result);
        }
        results
    }

    /// Check if a specific thumbnail is currently loading
    pub fn is_loading(&self, photo_id: &str) -> bool {
        self.loading.lock().contains(photo_id)
    }

    /// Get number of thumbnails currently loading
    pub fn loading_count(&self) -> usize {
        self.loading.lock().len()
    }

    /// Clear the requested set (useful when photos list changes)
    pub fn clear_requested(&mut self) {
        self.requested.clear();
    }
}

impl Default for AsyncThumbnailLoader {
    fn default() -> Self {
        Self::new()
    }
}


/// Async image processor for heavy operations like full image loading and processing
pub struct AsyncImageProcessor {
    /// Sender for image processing requests
    request_sender: Sender<ImageProcessRequest>,
    /// Receiver for processed images
    result_receiver: Receiver<ImageProcessResult>,
    /// Flag indicating if a request is in progress
    processing: Arc<Mutex<Option<String>>>,
}

/// Request to process an image
pub struct ImageProcessRequest {
    pub photo_id: String,
    pub path: String,
    pub exposure: f32,
    pub contrast: f32,
    pub temperature: f32,
    pub tint: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub clarity: f32,
    pub vibrance: f32,
    pub saturation: f32,
    /// Max preview size (width or height)
    pub max_preview_size: u32,
}

/// Result of image processing
pub struct ImageProcessResult {
    pub photo_id: String,
    /// Processed preview image
    pub preview: DynamicImage,
    /// Original (unprocessed) preview for before/after
    pub original_preview: DynamicImage,
    /// Histogram data
    pub histogram: crate::components::histogram::HistogramData,
}

impl AsyncImageProcessor {
    /// Create a new async image processor
    pub fn new() -> Self {
        let (request_sender, request_receiver) = channel::<ImageProcessRequest>();
        let (result_sender, result_receiver) = channel::<ImageProcessResult>();
        let processing = Arc::new(Mutex::new(None));
        let processing_clone = processing.clone();

        // Spawn dedicated thread for image processing
        std::thread::spawn(move || {
            Self::background_processor(request_receiver, result_sender, processing_clone);
        });

        Self {
            request_sender,
            result_receiver,
            processing,
        }
    }

    /// Background thread that processes image requests
    fn background_processor(
        receiver: Receiver<ImageProcessRequest>,
        sender: Sender<ImageProcessResult>,
        processing: Arc<Mutex<Option<String>>>,
    ) {
        while let Ok(request) = receiver.recv() {
            // Mark as processing
            *processing.lock() = Some(request.photo_id.clone());

            // Load and process the image
            if let Ok(img) = image::open(&request.path) {
                // Resize for preview using Rayon-accelerated operations
                let preview_img = crate::image_processing::ImageProcessor::resize_for_preview(
                    &img, 
                    request.max_preview_size
                );

                // Store original for before/after
                let original_preview = preview_img.clone();

                // Calculate histogram from preview
                let histogram = crate::components::histogram::HistogramData::from_image(&preview_img);

                // Apply edits if any
                let has_edits = request.exposure != 0.0 || request.contrast != 1.0 || 
                               request.temperature != 0.0 || request.tint != 0.0 ||
                               request.highlights != 0.0 || request.shadows != 0.0 ||
                               request.whites != 0.0 || request.blacks != 0.0 ||
                               request.clarity != 0.0 || request.vibrance != 0.0 ||
                               request.saturation != 0.0;

                let processed = if has_edits {
                    crate::image_processing::ImageProcessor::process_image(
                        &preview_img,
                        request.exposure,
                        request.contrast,
                        request.temperature,
                        request.tint,
                        request.highlights,
                        request.shadows,
                        request.whites,
                        request.blacks,
                        request.clarity,
                        request.vibrance,
                        request.saturation,
                    )
                } else {
                    preview_img
                };

                let result = ImageProcessResult {
                    photo_id: request.photo_id,
                    preview: processed,
                    original_preview,
                    histogram,
                };

                let _ = sender.send(result);
            }

            // Clear processing flag
            *processing.lock() = None;
        }
    }

    /// Request an image to be processed (non-blocking)
    /// If already processing, this request will be queued
    pub fn request_process(&self, request: ImageProcessRequest) {
        let _ = self.request_sender.send(request);
    }

    /// Poll for completed image processing (non-blocking)
    pub fn poll_result(&self) -> Option<ImageProcessResult> {
        self.result_receiver.try_recv().ok()
    }

    /// Check if currently processing
    pub fn is_processing(&self) -> bool {
        self.processing.lock().is_some()
    }

    /// Get the ID of the photo currently being processed
    pub fn processing_photo_id(&self) -> Option<String> {
        self.processing.lock().clone()
    }
}

impl Default for AsyncImageProcessor {
    fn default() -> Self {
        Self::new()
    }
}


/// Async edit processor for real-time slider adjustments
pub struct AsyncEditProcessor {
    /// Sender for edit requests
    request_sender: Sender<EditRequest>,
    /// Receiver for processed results
    result_receiver: Receiver<EditResult>,
    /// Current request ID (for debouncing - only latest matters)
    current_request_id: Arc<Mutex<u64>>,
}

/// Request to apply edits to an image
pub struct EditRequest {
    pub request_id: u64,
    pub original: DynamicImage,
    pub exposure: f32,
    pub contrast: f32,
    pub temperature: f32,
    pub tint: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub clarity: f32,
    pub vibrance: f32,
    pub saturation: f32,
}

/// Result of applying edits
pub struct EditResult {
    pub request_id: u64,
    pub processed: DynamicImage,
}

impl AsyncEditProcessor {
    /// Create a new async edit processor
    pub fn new() -> Self {
        let (request_sender, request_receiver) = channel::<EditRequest>();
        let (result_sender, result_receiver) = channel::<EditResult>();
        let current_request_id = Arc::new(Mutex::new(0u64));
        let current_request_id_clone = current_request_id.clone();

        // Spawn dedicated thread for edit processing
        std::thread::spawn(move || {
            Self::background_processor(request_receiver, result_sender, current_request_id_clone);
        });

        Self {
            request_sender,
            result_receiver,
            current_request_id,
        }
    }

    /// Background thread that processes edit requests
    fn background_processor(
        receiver: Receiver<EditRequest>,
        sender: Sender<EditResult>,
        current_request_id: Arc<Mutex<u64>>,
    ) {
        while let Ok(request) = receiver.recv() {
            // Check if this request is still current (debouncing)
            let current_id = *current_request_id.lock();
            if request.request_id < current_id {
                // Skip outdated requests
                continue;
            }

            // Process the edit
            let processed = crate::image_processing::ImageProcessor::process_image(
                &request.original,
                request.exposure,
                request.contrast,
                request.temperature,
                request.tint,
                request.highlights,
                request.shadows,
                request.whites,
                request.blacks,
                request.clarity,
                request.vibrance,
                request.saturation,
            );

            let result = EditResult {
                request_id: request.request_id,
                processed,
            };

            let _ = sender.send(result);
        }
    }

    /// Request edits to be applied (non-blocking)
    /// Returns the request ID for tracking
    pub fn request_edit(&self, request: EditRequest) -> u64 {
        let id = request.request_id;
        *self.current_request_id.lock() = id;
        let _ = self.request_sender.send(request);
        id
    }

    /// Generate a new request ID
    pub fn next_request_id(&self) -> u64 {
        let mut guard = self.current_request_id.lock();
        *guard += 1;
        *guard
    }

    /// Poll for completed edit (non-blocking)
    /// Returns only the latest result (discards outdated ones)
    pub fn poll_result(&self) -> Option<EditResult> {
        let mut latest: Option<EditResult> = None;
        
        // Drain all available results, keep only the latest
        while let Ok(result) = self.result_receiver.try_recv() {
            if latest.as_ref().map_or(true, |l| result.request_id > l.request_id) {
                latest = Some(result);
            }
        }
        
        latest
    }
}

impl Default for AsyncEditProcessor {
    fn default() -> Self {
        Self::new()
    }
}
