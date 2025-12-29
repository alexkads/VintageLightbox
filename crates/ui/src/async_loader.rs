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
use infrastructure::raw_processing::{is_raw_file, load_raw_as_dynamic_image};

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
                    let img_result = if is_raw_file(&req.path) {
                        load_raw_as_dynamic_image(&req.path)
                            .map_err(|e| image::ImageError::IoError(
                                std::io::Error::other(e)
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
    preview_manager: Arc<infrastructure::cache::preview_manager::PreviewManager>,
    /// In-memory cache for decoded images (avoid repeated JPEG decoding)
    memory_cache: Arc<Mutex<LruCache<String, DecodedImage>>>,
    /// Set of photo IDs currently being prefetched (to avoid duplicate prefetch)
    prefetching: Arc<Mutex<std::collections::HashSet<String>>>,
}

/// Cached decoded image data
struct DecodedImage {
    image: DynamicImage,
    histogram: crate::components::histogram::HistogramData,
    /// Cached processed ColorImage with edit parameters hash
    /// (exposure, contrast, temp, tint, highlights, shadows, whites, blacks, clarity, vibrance, sat)
    processed_cache: Option<ProcessedCache>,
}

/// Cache for processed image to avoid re-processing
struct ProcessedCache {
    /// Hash of edit parameters used to generate this cache
    edits_hash: u64,
    /// Pre-processed ColorImage ready for display
    color_image: ColorImage,
    /// Pre-cloned original for before/after
    original_preview: DynamicImage,
}

impl ImageProcessRequest {
    /// Calculate a hash of the edit parameters for cache invalidation
    fn edits_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        // Hash all edit parameters (using bits to avoid float comparison issues)
        self.exposure.to_bits().hash(&mut hasher);
        self.contrast.to_bits().hash(&mut hasher);
        self.temperature.to_bits().hash(&mut hasher);
        self.tint.to_bits().hash(&mut hasher);
        self.highlights.to_bits().hash(&mut hasher);
        self.shadows.to_bits().hash(&mut hasher);
        self.whites.to_bits().hash(&mut hasher);
        self.blacks.to_bits().hash(&mut hasher);
        self.clarity.to_bits().hash(&mut hasher);
        self.vibrance.to_bits().hash(&mut hasher);
        self.saturation.to_bits().hash(&mut hasher);
        self.tone_curve_shadows.to_bits().hash(&mut hasher);
        self.tone_curve_darks.to_bits().hash(&mut hasher);
        self.tone_curve_lights.to_bits().hash(&mut hasher);
        self.tone_curve_highlights.to_bits().hash(&mut hasher);
        // HSL Sat
        self.hsl_red_sat.to_bits().hash(&mut hasher);
        self.hsl_orange_sat.to_bits().hash(&mut hasher);
        self.hsl_yellow_sat.to_bits().hash(&mut hasher);
        self.hsl_green_sat.to_bits().hash(&mut hasher);
        self.hsl_aqua_sat.to_bits().hash(&mut hasher);
        self.hsl_blue_sat.to_bits().hash(&mut hasher);
        self.hsl_purple_sat.to_bits().hash(&mut hasher);
        self.hsl_magenta_sat.to_bits().hash(&mut hasher);
        // HSL Hue
        self.hsl_red_hue.to_bits().hash(&mut hasher);
        self.hsl_orange_hue.to_bits().hash(&mut hasher);
        self.hsl_yellow_hue.to_bits().hash(&mut hasher);
        self.hsl_green_hue.to_bits().hash(&mut hasher);
        self.hsl_aqua_hue.to_bits().hash(&mut hasher);
        self.hsl_blue_hue.to_bits().hash(&mut hasher);
        self.hsl_purple_hue.to_bits().hash(&mut hasher);
        self.hsl_magenta_hue.to_bits().hash(&mut hasher);
        // HSL Lum
        self.hsl_red_lum.to_bits().hash(&mut hasher);
        self.hsl_orange_lum.to_bits().hash(&mut hasher);
        self.hsl_yellow_lum.to_bits().hash(&mut hasher);
        self.hsl_green_lum.to_bits().hash(&mut hasher);
        self.hsl_aqua_lum.to_bits().hash(&mut hasher);
        self.hsl_blue_lum.to_bits().hash(&mut hasher);
        self.hsl_purple_lum.to_bits().hash(&mut hasher);
        self.hsl_magenta_lum.to_bits().hash(&mut hasher);
        // Lens
        self.lens_distortion.to_bits().hash(&mut hasher);
        self.lens_vignette_amount.to_bits().hash(&mut hasher);
        self.lens_vignette_midpoint.to_bits().hash(&mut hasher);
        // NR
        self.nr_luminance.to_bits().hash(&mut hasher);
        self.nr_color.to_bits().hash(&mut hasher);
        // Sharpening
        self.sharpen_amount.to_bits().hash(&mut hasher);
        self.sharpen_radius.to_bits().hash(&mut hasher);

        hasher.finish()
    }
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
    // Tone curve parametric zones
    pub tone_curve_shadows: f32,
    pub tone_curve_darks: f32,
    pub tone_curve_lights: f32,
    pub tone_curve_highlights: f32,
    // HSL Saturation
    pub hsl_red_sat: f32,
    pub hsl_orange_sat: f32,
    pub hsl_yellow_sat: f32,
    pub hsl_green_sat: f32,
    pub hsl_aqua_sat: f32,
    pub hsl_blue_sat: f32,
    pub hsl_purple_sat: f32,
    pub hsl_magenta_sat: f32,
    // HSL Hue
    pub hsl_red_hue: f32,
    pub hsl_orange_hue: f32,
    pub hsl_yellow_hue: f32,
    pub hsl_green_hue: f32,
    pub hsl_aqua_hue: f32,
    pub hsl_blue_hue: f32,
    pub hsl_purple_hue: f32,
    pub hsl_magenta_hue: f32,
    // HSL Lum
    pub hsl_red_lum: f32,
    pub hsl_orange_lum: f32,
    pub hsl_yellow_lum: f32,
    pub hsl_green_lum: f32,
    pub hsl_aqua_lum: f32,
    pub hsl_blue_lum: f32,
    pub hsl_purple_lum: f32,
    pub hsl_magenta_lum: f32,
    // Lens
    pub lens_distortion: f32,
    pub lens_vignette_amount: f32,
    pub lens_vignette_midpoint: f32,
    // NR
    pub nr_luminance: f32,
    pub nr_color: f32,
    // Sharpening
    pub sharpen_amount: f32,
    pub sharpen_radius: f32,
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
        
        // Cache capacity: 15 images (~600MB for 24MP images)
        // Larger cache improves navigation performance by keeping more recently viewed photos in RAM
        let cache_capacity = NonZeroUsize::new(15).unwrap();
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
            prefetching: Arc::new(Mutex::new(std::collections::HashSet::new())),
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
            let edits_hash = request.edits_hash();

            // 0. Try FULL processed cache first (RAM - Instant, no processing needed!)
            {
                let mut cache = memory_cache.lock();
                if let Some(decoded) = cache.get_mut(&request.photo_id) {
                    if let Some(ref processed) = decoded.processed_cache {
                        if processed.edits_hash == edits_hash {
                            // FULL CACHE HIT! Return immediately without any processing
                            let cache_ms = start_time.elapsed().as_secs_f32() * 1000.0;
                            println!("FULL PROCESSED CACHE HIT: {} ({:.2}ms)", request.photo_id, cache_ms);

                            let result = ImageProcessResult {
                                photo_id: request.photo_id.clone(),
                                preview: processed.color_image.clone(),
                                original_preview: processed.original_preview.clone(),
                                processed_image: decoded.image.clone(), // Not ideal but needed
                                histogram: decoded.histogram.clone(),
                                load_time_ms: cache_ms,
                            };

                            let _ = sender.send(result);
                            *processing.lock() = None;
                            continue;
                        }
                    }
                }
            }

            // 1. Try Memory Cache for base image (RAM - Fast)
            let memory_hit = {
                let mut cache = memory_cache.lock();
                cache.get(&request.photo_id).map(|decoded| (decoded.image.clone(), decoded.histogram.clone()))
            };

            let (preview_img, histogram) = if let Some((img, hist)) = memory_hit {
                 // RAM Cache Hit (but need to process)!
                 let ram_check_ms = start_time.elapsed().as_secs_f32() * 1000.0;
                 println!("RAM CACHE HIT (needs processing): {} ({:.2}ms)", request.photo_id, ram_check_ms);
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
                    let img_result = if is_raw_file(&request.path) {
                        println!("Loading RAW file with demosaic: {}", request.path);
                        load_raw_as_dynamic_image(&request.path)
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
                
                // Store in Memory Cache (without processed cache initially)
                {
                    let mut cache = memory_cache.lock();
                    cache.put(request.photo_id.clone(), DecodedImage {
                        image: img.clone(),
                        histogram: hist.clone(),
                        processed_cache: None,
                    });
                    println!("RAM CACHE STORE: {} (Count: {}) | Hist Calc: {:.2}ms", request.photo_id, cache.len(), hist_ms);
                }
                
                (img, hist)
            };

            // Continue with preview_img (either from cache or just generated)
            {
                let post_cache_start = std::time::Instant::now();

                // Store original for before/after
                // CLONE WARNING: This might be expensive for large images
                let clone_start = std::time::Instant::now();
                let original_preview = preview_img.clone();
                let clone_ms = clone_start.elapsed().as_secs_f32() * 1000.0;

                // Apply edits if any
                let has_edits = request.exposure != 0.0 || request.contrast != 1.0 || 
                               request.temperature != 0.0 || request.tint != 0.0 ||
                               request.highlights != 0.0 || request.shadows != 0.0 ||
                               request.whites != 0.0 || request.blacks != 0.0 ||
                               request.clarity != 0.0 || request.vibrance != 0.0 ||

                               // Tone Curve
                               request.tone_curve_shadows != 0.0 || request.tone_curve_darks != 0.0 ||
                               request.tone_curve_lights != 0.0 || request.tone_curve_highlights != 0.0 ||
                               // HSL Sat
                               request.hsl_red_sat != 0.0 || request.hsl_orange_sat != 0.0 ||
                               request.hsl_yellow_sat != 0.0 || request.hsl_green_sat != 0.0 ||
                               request.hsl_aqua_sat != 0.0 || request.hsl_blue_sat != 0.0 ||
                               request.hsl_purple_sat != 0.0 || request.hsl_magenta_sat != 0.0 ||
                               // HSL Hue
                               request.hsl_red_hue != 0.0 || request.hsl_orange_hue != 0.0 ||
                               request.hsl_yellow_hue != 0.0 || request.hsl_green_hue != 0.0 ||
                               request.hsl_aqua_hue != 0.0 || request.hsl_blue_hue != 0.0 ||
                               request.hsl_purple_hue != 0.0 || request.hsl_magenta_hue != 0.0 ||
                               // HSL Lum
                               request.hsl_red_lum != 0.0 || request.hsl_orange_lum != 0.0 ||
                               request.hsl_yellow_lum != 0.0 || request.hsl_green_lum != 0.0 ||
                               request.hsl_aqua_lum != 0.0 || request.hsl_blue_lum != 0.0 ||
                               request.hsl_purple_lum != 0.0 || request.hsl_magenta_lum != 0.0 ||
                               // Lens
                               request.lens_distortion != 0.0 || request.lens_vignette_amount != 0.0 || request.lens_vignette_midpoint != 0.0 ||
                               // NR
                               request.nr_luminance != 0.0 || request.nr_color != 0.0 ||
                               // Sharpening
                               request.sharpen_amount != 0.0 || request.sharpen_radius != 1.0;

                let edit_start = std::time::Instant::now();
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
                        0.0, 0.0, 0.0, 0.0, // tone curve
                        request.hsl_red_sat, request.hsl_orange_sat, request.hsl_yellow_sat, request.hsl_green_sat,
                        request.hsl_aqua_sat, request.hsl_blue_sat, request.hsl_purple_sat, request.hsl_magenta_sat,
                        // HSL Hue
                        request.hsl_red_hue, request.hsl_orange_hue, request.hsl_yellow_hue, request.hsl_green_hue,
                        request.hsl_aqua_hue, request.hsl_blue_hue, request.hsl_purple_hue, request.hsl_magenta_hue,
                        // HSL Lum
                        request.hsl_red_lum, request.hsl_orange_lum, request.hsl_yellow_lum, request.hsl_green_lum,
                        request.hsl_aqua_lum, request.hsl_blue_lum, request.hsl_purple_lum, request.hsl_magenta_lum,
                        // Lens
                        request.lens_distortion, request.lens_vignette_amount, request.lens_vignette_midpoint,
                        // NR
                        request.nr_luminance, request.nr_color,
                        // Sharpening
                        request.sharpen_amount, request.sharpen_radius,
                    )
                } else {
                    preview_img
                };
                let edit_ms = edit_start.elapsed().as_secs_f32() * 1000.0;

                let convert_start = std::time::Instant::now();
                let processed_color = crate::image_processing::ImageProcessor::dynamic_to_color_image(&processed);
                let convert_ms = convert_start.elapsed().as_secs_f32() * 1000.0;

                let post_cache_ms = post_cache_start.elapsed().as_secs_f32() * 1000.0;
                println!("POST-CACHE: {} | Clone: {:.2}ms, Edits: {:.2}ms, Convert: {:.2}ms, Total: {:.2}ms",
                    request.photo_id, clone_ms, edit_ms, convert_ms, post_cache_ms);

                // Save processed result to cache for instant retrieval next time
                {
                    let mut cache = memory_cache.lock();
                    if let Some(decoded) = cache.get_mut(&request.photo_id) {
                        decoded.processed_cache = Some(ProcessedCache {
                            edits_hash,
                            color_image: processed_color.clone(),
                            original_preview: original_preview.clone(),
                        });
                        println!("PROCESSED CACHE SAVED: {} (hash: {})", request.photo_id, edits_hash);
                    }
                }

                let result = ImageProcessResult {
                    photo_id: request.photo_id,
                    preview: processed_color,
                    original_preview,
                    processed_image: processed,
                    histogram,
                    load_time_ms: start_time.elapsed().as_secs_f32() * 1000.0,
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

    /// Check if a photo is already in the RAM cache (L1)
    pub fn is_in_cache(&self, photo_id: &str) -> bool {
        self.memory_cache.lock().contains(photo_id)
    }

    /// Prefetch a photo into the RAM cache (L1) without processing edits
    /// This runs in a SEPARATE THREAD to not block the main image loading
    pub fn prefetch(&self, photo_id: String, path: String, max_preview_size: u32) {
        // Skip if already in cache
        if self.is_in_cache(&photo_id) {
            return;
        }

        // Skip if already being prefetched
        {
            let mut prefetching = self.prefetching.lock();
            if prefetching.contains(&photo_id) {
                return;
            }
            prefetching.insert(photo_id.clone());
        }

        // Clone what we need for the background thread
        let memory_cache = self.memory_cache.clone();
        let preview_manager = self.preview_manager.clone();
        let prefetching = self.prefetching.clone();
        let photo_id_for_cleanup = photo_id.clone();

        // Spawn a SEPARATE thread for prefetch (doesn't block main queue)
        std::thread::spawn(move || {
            let start_time = std::time::Instant::now();

            // Try to load from SQLite cache first
            let img = if let Some(cached_img) = preview_manager.get_preview(&photo_id) {
                let decode_ms = start_time.elapsed().as_secs_f32() * 1000.0;
                println!("PREFETCH SQLITE HIT: {} ({:.2}ms)", photo_id, decode_ms);
                cached_img
            } else {
                // Cache miss - load from disk
                println!("PREFETCH CACHE MISS: {} - loading from disk", photo_id);
                let load_start = std::time::Instant::now();

                let img_result = if is_raw_file(&path) {
                    load_raw_as_dynamic_image(&path)
                } else {
                    image::open(&path).map_err(|e| e.to_string())
                };

                if let Ok(img) = img_result {
                    let resized = crate::image_processing::ImageProcessor::resize_for_preview(
                        &img,
                        max_preview_size,
                    );

                    // Save to SQLite for next time
                    let _ = preview_manager.save_preview(&photo_id, &resized);

                    let load_ms = load_start.elapsed().as_secs_f32() * 1000.0;
                    println!("PREFETCH GENERATED: {} ({:.2}ms)", photo_id, load_ms);
                    resized
                } else {
                    // Failed to load - remove from prefetching set and return
                    prefetching.lock().remove(&photo_id_for_cleanup);
                    return;
                }
            };

            // Calculate histogram and store in L1 cache
            let hist = crate::components::histogram::HistogramData::from_image(&img);

            {
                let mut cache = memory_cache.lock();
                cache.put(photo_id.clone(), DecodedImage {
                    image: img,
                    histogram: hist,
                    processed_cache: None, // Will be populated on first actual use
                });
                let total_ms = start_time.elapsed().as_secs_f32() * 1000.0;
                println!("PREFETCH CACHED: {} (Total: {:.2}ms, Cache size: {})", photo_id, total_ms, cache.len());
            }

            // Remove from prefetching set
            prefetching.lock().remove(&photo_id_for_cleanup);
        });
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
    // Tone curve parametric zones
    pub tone_curve_shadows: f32,
    pub tone_curve_darks: f32,
    pub tone_curve_lights: f32,
    pub tone_curve_highlights: f32,
    // HSL Saturation
    pub hsl_red_sat: f32,
    pub hsl_orange_sat: f32,
    pub hsl_yellow_sat: f32,
    pub hsl_green_sat: f32,
    pub hsl_aqua_sat: f32,
    pub hsl_blue_sat: f32,
    pub hsl_purple_sat: f32,
    pub hsl_magenta_sat: f32,
    // HSL Hue
    pub hsl_red_hue: f32,
    pub hsl_orange_hue: f32,
    pub hsl_yellow_hue: f32,
    pub hsl_green_hue: f32,
    pub hsl_aqua_hue: f32,
    pub hsl_blue_hue: f32,
    pub hsl_purple_hue: f32,
    pub hsl_magenta_hue: f32,
    // HSL Lum
    pub hsl_red_lum: f32,
    pub hsl_orange_lum: f32,
    pub hsl_yellow_lum: f32,
    pub hsl_green_lum: f32,
    pub hsl_aqua_lum: f32,
    pub hsl_blue_lum: f32,
    pub hsl_purple_lum: f32,
    pub hsl_magenta_lum: f32,
    // Lens
    pub lens_distortion: f32,
    pub lens_vignette_amount: f32,
    pub lens_vignette_midpoint: f32,
    // NR
    pub nr_luminance: f32,
    pub nr_color: f32,
    // Sharpening
    pub sharpen_amount: f32,
    pub sharpen_radius: f32,
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
                request.tone_curve_shadows,
                request.tone_curve_darks,
                request.tone_curve_lights,
                request.tone_curve_highlights,
                request.hsl_red_sat, request.hsl_orange_sat, request.hsl_yellow_sat, request.hsl_green_sat,
                request.hsl_aqua_sat, request.hsl_blue_sat, request.hsl_purple_sat, request.hsl_magenta_sat,
                // HSL Hue
                request.hsl_red_hue, request.hsl_orange_hue, request.hsl_yellow_hue, request.hsl_green_hue,
                request.hsl_aqua_hue, request.hsl_blue_hue, request.hsl_purple_hue, request.hsl_magenta_hue,
                // HSL Lum
                request.hsl_red_lum, request.hsl_orange_lum, request.hsl_yellow_lum, request.hsl_green_lum,
                request.hsl_aqua_lum, request.hsl_blue_lum, request.hsl_purple_lum, request.hsl_magenta_lum,
                // Lens
                request.lens_distortion, request.lens_vignette_amount, request.lens_vignette_midpoint,
                // NR
                request.nr_luminance, request.nr_color,
                // Sharpening
                request.sharpen_amount, request.sharpen_radius,
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
            if latest.as_ref().is_none_or(|l| result.request_id > l.request_id) {
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
    #[allow(unused_imports)]
    use super::{AsyncThumbnailLoader, ThumbnailRequest};
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
        
        // Retrieve and verify all
        for i in 0..10 {
            let photo_id = format!("photo_{}", i);
            let retrieved = preview_manager.get_preview(&photo_id).unwrap();
            assert_eq!(retrieved.width(), 100 + i * 10);
        }
    }
}
