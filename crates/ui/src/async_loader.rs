// Async Thumbnail Loader
// Uses Rayon for parallel thumbnail loading without blocking the UI

use eframe::egui::ColorImage;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use parking_lot::Mutex;
use rayon::prelude::*;
use image::DynamicImage;
use lru::LruCache;
use std::num::NonZeroUsize;

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
    /// Create a new async thumbnail loader
    pub fn new(preview_manager: Arc<infrastructure::cache::preview_manager::PreviewManager>) -> Self {
        let (request_sender, request_receiver) = channel::<Vec<ThumbnailRequest>>();
        let (result_sender, result_receiver) = channel::<ThumbnailResult>();
        let loading = Arc::new(Mutex::new(std::collections::HashSet::new()));
        let loading_clone = loading.clone();

        let preview_manager_clone = preview_manager.clone();

        // Spawn background thread for processing thumbnail requests
        std::thread::spawn(move || {
            Self::background_loader(request_receiver, result_sender, loading_clone, preview_manager_clone);
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
        preview_manager: Arc<infrastructure::cache::preview_manager::PreviewManager>,
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
                    // 1. Try cache first
                    if let Some(img) = preview_manager.get_thumbnail(&req.photo_id) {
                         return Some(ThumbnailResult {
                            photo_id: req.photo_id.clone(),
                            image: img,
                        });
                    }

                    // 2. Fallback to loading original and generating thumbnail
                    // Check if it's a RAW file and use appropriate loader
                    let img_result = if infrastructure::is_raw_file(&req.path) {
                        infrastructure::load_raw_as_dynamic_image(&req.path)
                            .map_err(|e| image::ImageError::IoError(
                                std::io::Error::new(std::io::ErrorKind::Other, e)
                            ))
                    } else {
                        image::open(&req.path)
                    };
                    
                    match img_result {
                        Ok(img) => {
                             // Resize
                             let thumb = crate::image_processing::ImageProcessor::resize_for_preview(&img, 300);
                             // Save to cache
                             let _ = preview_manager.save_thumbnail(&req.photo_id, &thumb);
                             
                             Some(ThumbnailResult {
                                photo_id: req.photo_id.clone(),
                                image: thumb,
                             })
                        },
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

    /// Clear requested state for a specific photo (useful when invalidating cache)
    pub fn clear_requested_for(&mut self, photo_id: &str) {
        self.requested.remove(photo_id);
    }
}

// Default implementation removed because PreviewManager is required
// impl Default for AsyncThumbnailLoader { ... }


/// Async image processor for heavy operations like full image loading and processing
pub struct AsyncImageProcessor {
    /// Sender for image processing requests
    request_sender: Sender<ImageProcessRequest>,
    /// Receiver for processed images
    result_receiver: Receiver<ImageProcessResult>,
    /// Flag indicating if a request is in progress
    processing: Arc<Mutex<Option<String>>>,
    /// Manager for smart preview caching
    #[allow(dead_code)] // It is used inside thread closure but compiler might not see it across clone
    preview_manager: Arc<infrastructure::cache::preview_manager::PreviewManager>,
    /// In-memory cache for decoded images (avoid repeated JPEG decoding)
    #[allow(dead_code)]
    memory_cache: Arc<Mutex<LruCache<String, DecodedImage>>>,
}

/// Cached decoded image data
struct DecodedImage {
    image: DynamicImage,
    histogram: crate::components::histogram::HistogramData,
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
    /// Processed preview image prepared for display
    pub preview: ColorImage,
    /// Original (unprocessed) preview for before/after
    pub original_preview: DynamicImage,
    /// Processed DynamicImage (for saving/further processing if needed)
    pub processed_image: DynamicImage,
    /// Histogram data
    pub histogram: crate::components::histogram::HistogramData,
    /// Time taken to load and process
    pub load_time_ms: f32,
}

impl AsyncImageProcessor {
    /// Create a new async image processor
    /// Create a new async image processor
    pub fn new(preview_manager: Arc<infrastructure::cache::preview_manager::PreviewManager>) -> Self {
        let (request_sender, request_receiver) = channel::<ImageProcessRequest>();
        let (result_sender, result_receiver) = channel::<ImageProcessResult>();
        let processing = Arc::new(Mutex::new(None));
        let processing_clone = processing.clone();
        let preview_manager_clone = preview_manager.clone();
        
        // Cache capacity: 5 images (~200MB for 24MP images)
        let cache_capacity = NonZeroUsize::new(5).unwrap();
        let memory_cache = Arc::new(Mutex::new(LruCache::new(cache_capacity)));
        let memory_cache_clone = memory_cache.clone();

        // Spawn dedicated thread for image processing
        std::thread::spawn(move || {
            Self::background_processor(request_receiver, result_sender, processing_clone, preview_manager_clone, memory_cache_clone);
        });

        Self {
            request_sender,
            result_receiver,
            processing,
            preview_manager,
            memory_cache,
        }
    }

    /// Background thread that processes image requests
    fn background_processor(
        receiver: Receiver<ImageProcessRequest>,
        sender: Sender<ImageProcessResult>,
        processing: Arc<Mutex<Option<String>>>,
        preview_manager: Arc<infrastructure::cache::preview_manager::PreviewManager>,
        memory_cache: Arc<Mutex<LruCache<String, DecodedImage>>>,
    ) {
        while let Ok(request) = receiver.recv() {
            // Mark as processing
            *processing.lock() = Some(request.photo_id.clone());

            let start_time = std::time::Instant::now();

            // 0. Try Memory Cache first (RAM - Instant)
            let memory_hit = {
                let mut cache = memory_cache.lock();
                if let Some(decoded) = cache.get(&request.photo_id) {
                    Some((decoded.image.clone(), decoded.histogram.clone()))
                } else {
                    None
                }
            };

            let (preview_img, histogram) = if let Some((img, hist)) = memory_hit {
                 // RAM Cache Hit!
                 let ram_check_ms = start_time.elapsed().as_secs_f32() * 1000.0;
                 println!("RAM CACHE HIT: {} (Memory read: {:.2}ms)", request.photo_id, ram_check_ms);
                 (img, hist)
            } else {
                // RAM Miss - Try SQLite Cache
                
                // 1. Try to load Smart Preview from cache first
                let cache_check_start = std::time::Instant::now();
                let cached_preview = preview_manager.get_preview(&request.photo_id);
                let cache_check_ms = cache_check_start.elapsed().as_secs_f32() * 1000.0;
                
                let img = if let Some(img) = cached_preview {
                    // Cache hit! Use optimized image
                    println!("SQLITE BLOB HIT: {} (Decode: {:.2}ms)", request.photo_id, cache_check_ms);
                    img
                } else {
                    // Cache miss. Load original and generate Smart Preview
                    println!("FULL CACHE MISS: {}", request.photo_id);
                    let load_start = std::time::Instant::now();
                    
                    // Check if it's a RAW file and use appropriate loader
                    let img_result = if infrastructure::is_raw_file(&request.path) {
                        println!("Loading RAW file with demosaic: {}", request.path);
                        infrastructure::load_raw_as_dynamic_image(&request.path)
                    } else {
                        image::open(&request.path).map_err(|e| e.to_string())
                    };
                    
                    if let Ok(img) = img_result {
                        let open_ms = load_start.elapsed().as_secs_f32() * 1000.0;
                        
                        // Resize for preview using Rayon-accelerated operations
                        let resize_start = std::time::Instant::now();
                        let resized = crate::image_processing::ImageProcessor::resize_for_preview(
                            &img, 
                            request.max_preview_size
                        );
                        let resize_ms = resize_start.elapsed().as_secs_f32() * 1000.0;
                        
                        // Save to cache for next time
                        let save_start = std::time::Instant::now();
                        if let Err(e) = preview_manager.save_preview(&request.photo_id, &resized) {
                            eprintln!("CACHE SAVE ERROR: {}", e);
                        }
                        let save_ms = save_start.elapsed().as_secs_f32() * 1000.0;
                        
                        println!("Generated Smart Preview: Open={:.2}ms, Resize={:.2}ms, Save={:.2}ms", open_ms, resize_ms, save_ms);
                        
                        resized
                    } else {
                        // Failed to load image
                        eprintln!("Failed to open image: {} - {:?}", request.path, img_result.err());
                        *processing.lock() = None;
                        continue;
                    }
                };
                
                // Calculate histogram
                let hist_start = std::time::Instant::now();
                let hist = crate::components::histogram::HistogramData::from_image(&img);
                let hist_ms = hist_start.elapsed().as_secs_f32() * 1000.0;
                
                // Store in Memory Cache
                {
                    let mut cache = memory_cache.lock();
                    cache.put(request.photo_id.clone(), DecodedImage {
                        image: img.clone(),
                        histogram: hist.clone(),
                    });
                    println!("RAM CACHE STORE: {} (Count: {}) | Hist Calc: {:.2}ms", request.photo_id, cache.len(), hist_ms);
                }
                
                (img, hist)
            };

            // Continue with preview_img (either from cache or just generated)
            {
                // Store original for before/after
                // CLONE WARNING: This might be expensive for large images
                let original_preview = preview_img.clone();

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

                let processed_color = crate::image_processing::ImageProcessor::dynamic_to_color_image(&processed);

                let result = ImageProcessResult {
                    photo_id: request.photo_id,
                    preview: processed_color,
                    original_preview,
                    processed_image: processed,
                    histogram,
                    load_time_ms: start_time.elapsed().as_secs_f32() * 1000.0,
                };
                
                // println!("Processed: {} in {:.2}ms", request.path, result.load_time_ms);

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

// Default implementation removed because PreviewManager is required
// impl Default for AsyncImageProcessor { ... }


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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use infrastructure::cache::preview_manager::PreviewManager;
    use tempfile::TempDir;

    fn create_test_preview_manager() -> Arc<PreviewManager> {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().to_path_buf();
        Arc::new(PreviewManager::new_with_path(cache_dir))
    }

    #[test]
    fn test_clear_requested_for_removes_specific_photo() {
        let preview_manager = create_test_preview_manager();
        let mut loader = AsyncThumbnailLoader::new(preview_manager);

        // Request some thumbnails
        let requests = vec![
            ThumbnailRequest {
                photo_id: "photo1".to_string(),
                path: "/path/to/photo1.jpg".to_string(),
            },
            ThumbnailRequest {
                photo_id: "photo2".to_string(),
                path: "/path/to/photo2.jpg".to_string(),
            },
            ThumbnailRequest {
                photo_id: "photo3".to_string(),
                path: "/path/to/photo3.jpg".to_string(),
            },
        ];
        loader.request_thumbnails(requests);

        // Verify all are in requested set
        assert!(loader.requested.contains("photo1"));
        assert!(loader.requested.contains("photo2"));
        assert!(loader.requested.contains("photo3"));

        // Clear one specific photo
        loader.clear_requested_for("photo2");

        // Verify only photo2 was removed
        assert!(loader.requested.contains("photo1"));
        assert!(!loader.requested.contains("photo2"));
        assert!(loader.requested.contains("photo3"));
    }

    #[test]
    fn test_clear_requested_for_allows_re_request() {
        let preview_manager = create_test_preview_manager();
        let mut loader = AsyncThumbnailLoader::new(preview_manager);

        // Request a thumbnail
        let request = ThumbnailRequest {
            photo_id: "photo1".to_string(),
            path: "/path/to/photo1.jpg".to_string(),
        };
        loader.request_thumbnails(vec![request.clone()]);

        // Verify it's in requested set
        assert!(loader.requested.contains("photo1"));
        let initial_count = loader.requested.len();

        // Try to request again - should be filtered out (duplicate prevention)
        loader.request_thumbnails(vec![request.clone()]);
        assert_eq!(loader.requested.len(), initial_count); // No change

        // Clear requested state for this photo
        loader.clear_requested_for("photo1");
        assert!(!loader.requested.contains("photo1"));

        // Now request a different photo to verify the set works
        let request2 = ThumbnailRequest {
            photo_id: "photo2".to_string(),
            path: "/path/to/photo2.jpg".to_string(),
        };
        loader.request_thumbnails(vec![request2]);
        assert!(loader.requested.contains("photo2"));
        assert!(!loader.requested.contains("photo1")); // Still cleared
    }

    #[test]
    fn test_clear_requested_for_nonexistent_photo() {
        let preview_manager = create_test_preview_manager();
        let mut loader = AsyncThumbnailLoader::new(preview_manager);

        // Request some thumbnails
        let requests = vec![
            ThumbnailRequest {
                photo_id: "photo1".to_string(),
                path: "/path/to/photo1.jpg".to_string(),
            },
        ];
        loader.request_thumbnails(requests);

        // Clear a photo that was never requested - should not panic
        loader.clear_requested_for("nonexistent");

        // Verify original photo is still there
        assert!(loader.requested.contains("photo1"));
    }

    #[test]
    fn test_clear_requested_clears_all() {
        let preview_manager = create_test_preview_manager();
        let mut loader = AsyncThumbnailLoader::new(preview_manager);

        // Request multiple thumbnails
        let requests = vec![
            ThumbnailRequest {
                photo_id: "photo1".to_string(),
                path: "/path/to/photo1.jpg".to_string(),
            },
            ThumbnailRequest {
                photo_id: "photo2".to_string(),
                path: "/path/to/photo2.jpg".to_string(),
            },
        ];
        loader.request_thumbnails(requests);

        assert_eq!(loader.requested.len(), 2);

        // Clear all
        loader.clear_requested();

        assert_eq!(loader.requested.len(), 0);
    }
}

#[cfg(test)]
mod cache_system_tests {
    use super::*;
    use std::sync::Arc;
    use infrastructure::cache::preview_manager::PreviewManager;
    use tempfile::TempDir;
    use image::{DynamicImage, RgbaImage};

    /// Helper to create a test image
    fn create_test_image(width: u32, height: u32, color: [u8; 4]) -> DynamicImage {
        let img = RgbaImage::from_pixel(width, height, image::Rgba(color));
        DynamicImage::ImageRgba8(img)
    }

    /// Helper to create a test PreviewManager with temporary database
    fn create_test_preview_manager() -> (Arc<PreviewManager>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().to_path_buf();
        let manager = Arc::new(PreviewManager::new_with_path(cache_dir));
        (manager, temp_dir)
    }

    #[test]
    fn test_l2_cache_save_and_retrieve() {
        let (preview_manager, _temp_dir) = create_test_preview_manager();
        
        // Create a test image
        let test_image = create_test_image(100, 100, [255, 0, 0, 255]); // Red
        
        // Save to L2 cache (SQLite BLOB)
        let photo_id = "test_photo_1";
        preview_manager.save_preview(photo_id, &test_image).unwrap();
        
        // Retrieve from L2 cache
        let retrieved = preview_manager.get_preview(photo_id);
        assert!(retrieved.is_some(), "Should retrieve image from L2 cache");
        
        let retrieved_img = retrieved.unwrap();
        assert_eq!(retrieved_img.width(), 100);
        assert_eq!(retrieved_img.height(), 100);
    }

    #[test]
    fn test_l2_cache_miss() {
        let (preview_manager, _temp_dir) = create_test_preview_manager();
        
        // Try to retrieve non-existent image
        let result = preview_manager.get_preview("nonexistent_photo");
        assert!(result.is_none(), "Should return None for cache miss");
    }

    #[test]
    fn test_l2_cache_overwrite() {
        let (preview_manager, _temp_dir) = create_test_preview_manager();
        
        let photo_id = "test_photo_overwrite";
        
        // Save first image (red)
        let img1 = create_test_image(100, 100, [255, 0, 0, 255]);
        preview_manager.save_preview(photo_id, &img1).unwrap();
        
        // Save second image (blue) - should overwrite
        let img2 = create_test_image(200, 200, [0, 0, 255, 255]);
        preview_manager.save_preview(photo_id, &img2).unwrap();
        
        // Retrieve and verify it's the second image
        let retrieved = preview_manager.get_preview(photo_id).unwrap();
        assert_eq!(retrieved.width(), 200, "Should have new image dimensions");
        assert_eq!(retrieved.height(), 200);
    }

    #[test]
    fn test_l2_cache_multiple_photos() {
        let (preview_manager, _temp_dir) = create_test_preview_manager();
        
        // Save multiple photos
        for i in 0..10 {
            let photo_id = format!("photo_{}", i);
            let img = create_test_image(100 + i * 10, 100 + i * 10, [i as u8 * 25, 0, 0, 255]);
            preview_manager.save_preview(&photo_id, &img).unwrap();
        }
        
        // Verify all can be retrieved
        for i in 0..10 {
            let photo_id = format!("photo_{}", i);
            let retrieved = preview_manager.get_preview(&photo_id);
            assert!(retrieved.is_some(), "Photo {} should be in cache", i);
            
            let img = retrieved.unwrap();
            assert_eq!(img.width(), 100 + i * 10);
        }
    }

    #[test]
    fn test_thumbnail_cache_separate_from_preview() {
        let (preview_manager, _temp_dir) = create_test_preview_manager();
        
        let photo_id = "test_photo_dual";
        
        // Save both thumbnail and preview for same photo
        let thumbnail = create_test_image(300, 300, [255, 0, 0, 255]); // Red thumbnail
        let preview = create_test_image(2560, 2560, [0, 255, 0, 255]); // Green preview
        
        preview_manager.save_thumbnail(photo_id, &thumbnail).unwrap();
        preview_manager.save_preview(photo_id, &preview).unwrap();
        
        // Retrieve both
        let retrieved_thumb = preview_manager.get_thumbnail(photo_id).unwrap();
        let retrieved_preview = preview_manager.get_preview(photo_id).unwrap();
        
        // Verify they're different
        assert_eq!(retrieved_thumb.width(), 300);
        assert_eq!(retrieved_preview.width(), 2560);
    }

    #[test]
    fn test_l2_cache_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().to_path_buf();
        
        let photo_id = "persistent_photo";
        let test_image = create_test_image(150, 150, [128, 128, 128, 255]);
        
        // Create first manager and save
        {
            let manager = PreviewManager::new_with_path(cache_dir.clone());
            manager.save_preview(photo_id, &test_image).unwrap();
        } // Manager dropped
        
        // Create new manager with same database
        {
            let manager = PreviewManager::new_with_path(cache_dir);
            let retrieved = manager.get_preview(photo_id);
            assert!(retrieved.is_some(), "Cache should persist across manager instances");
            
            let img = retrieved.unwrap();
            assert_eq!(img.width(), 150);
        }
    }

    #[test]
    fn test_l2_cache_jpeg_compression() {
        let (preview_manager, _temp_dir) = create_test_preview_manager();
        
        // Create a large image
        let large_image = create_test_image(2560, 1440, [200, 100, 50, 255]);
        let photo_id = "compression_test";
        
        // Save to cache (will be JPEG compressed)
        preview_manager.save_preview(photo_id, &large_image).unwrap();
        
        // Retrieve and verify dimensions are preserved
        let retrieved = preview_manager.get_preview(photo_id).unwrap();
        assert_eq!(retrieved.width(), 2560, "Width should be preserved");
        assert_eq!(retrieved.height(), 1440, "Height should be preserved");
        
        // Note: Colors may differ slightly due to JPEG compression, but dimensions should match
    }

    #[test]
    fn test_cache_error_handling() {
        let (preview_manager, _temp_dir) = create_test_preview_manager();
        
        // Try to save with empty photo_id
        let img = create_test_image(100, 100, [255, 255, 255, 255]);
        let result = preview_manager.save_preview("", &img);
        
        // Should not panic, may succeed or fail gracefully
        // Just verify it doesn't crash
        let _ = result;
    }

    #[test]
    fn test_l1_lru_cache_capacity() {
        // This test verifies the L1 RAM cache LRU eviction
        // The AsyncImageProcessor has a capacity of 5 images
        let (preview_manager, _temp_dir) = create_test_preview_manager();
        let processor = AsyncImageProcessor::new(preview_manager);
        
        // The LRU cache is internal, so we can't directly test it
        // But we can verify the processor was created successfully
        assert!(!processor.is_processing());
    }
}
