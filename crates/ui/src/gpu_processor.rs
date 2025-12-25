// GPU Image Processor
// Uses WGPU compute shaders for fast image processing

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use parking_lot::Mutex;
use image::DynamicImage;
use lru::LruCache;
use std::num::NonZeroUsize;
use eframe::egui::ColorImage;

/// Parameters for image adjustments (must match WGSL struct layout)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuEditParams {
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
    // HSL color channel saturations (-100 to +100)
    pub hsl_red_sat: f32,
    pub hsl_orange_sat: f32,
    pub hsl_yellow_sat: f32,
    pub hsl_green_sat: f32,
    pub hsl_aqua_sat: f32,
    pub hsl_blue_sat: f32,
    pub hsl_purple_sat: f32,
    pub hsl_magenta_sat: f32,
    // Noise Reduction
    pub nr_luminance: f32,
    pub nr_color: f32,
    // Sharpening
    pub sharpen_amount: f32,
    pub sharpen_radius: f32,
}

impl Default for GpuEditParams {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 1.0,
            temperature: 0.0,
            tint: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            clarity: 0.0,
            vibrance: 0.0,
            saturation: 0.0,
            tone_curve_shadows: 0.0,
            tone_curve_darks: 0.0,
            tone_curve_lights: 0.0,
            tone_curve_highlights: 0.0,
            hsl_red_sat: 0.0,
            hsl_orange_sat: 0.0,
            hsl_yellow_sat: 0.0,
            hsl_green_sat: 0.0,
            hsl_aqua_sat: 0.0,
            hsl_blue_sat: 0.0,
            hsl_purple_sat: 0.0,
            hsl_magenta_sat: 0.0,
            nr_luminance: 0.0,
            nr_color: 0.0,
            sharpen_amount: 0.0,
            sharpen_radius: 1.0,
        }
    }
}

/// Request to process an image on GPU
pub struct GpuProcessRequest {
    pub request_id: u64,
    pub image_data: Arc<Vec<u8>>, // Use Arc to avoid cloning massive image data
    pub width: u32,
    pub height: u32,
    pub params: GpuEditParams,
}

/// Result of GPU processing
pub struct GpuProcessResult {
    pub request_id: u64,
    pub processed_image: DynamicImage,
    pub preview: ColorImage,
    pub process_time_ms: f32,
}

struct GpuResources {
    input_texture: wgpu::Texture,
    output_texture: wgpu::Texture,
    params_buffer: wgpu::Buffer,
    output_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    padded_bytes_per_row: u32,
    unpadded_bytes_per_row: u32,
    last_image_data: Option<Arc<Vec<u8>>>, // To detect if input changed
}

/// GPU-accelerated image processor
pub struct GpuImageProcessor {
    request_sender: Sender<GpuProcessRequest>,
    result_receiver: Receiver<GpuProcessResult>,
    current_request_id: Arc<Mutex<u64>>,
    gpu_available: bool,
}

impl GpuImageProcessor {
    /// Create a new GPU image processor
    pub fn new() -> Self {
        let (request_sender, request_receiver) = channel::<GpuProcessRequest>();
        let (result_sender, result_receiver) = channel::<GpuProcessResult>();
        let current_request_id = Arc::new(Mutex::new(0u64));
        let current_request_id_clone = current_request_id.clone();

        // Try to initialize GPU in background thread
        let _gpu_available = std::thread::spawn(move || {
            Self::gpu_processor_thread(request_receiver, result_sender, current_request_id_clone)
        }).is_finished() == false; // Thread started successfully

        Self {
            request_sender,
            result_receiver,
            current_request_id,
            gpu_available: true, // Assume true, will fail gracefully if not
        }
    }

    /// Background thread that processes images on GPU
    fn gpu_processor_thread(
        receiver: Receiver<GpuProcessRequest>,
        sender: Sender<GpuProcessResult>,
        current_request_id: Arc<Mutex<u64>>,
    ) {
        // Initialize WGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        let adapter = match adapter {
            Some(a) => a,
            None => {
                eprintln!("GPU: No suitable adapter found, falling back to CPU");
                // Fall back to CPU processing
                Self::cpu_fallback_loop(receiver, sender, current_request_id);
                return;
            }
        };

        let (device, queue) = match pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("VintageLightbox GPU"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )) {
            Ok(dq) => dq,
            Err(e) => {
                eprintln!("GPU: Failed to create device: {}, falling back to CPU", e);
                Self::cpu_fallback_loop(receiver, sender, current_request_id);
                return;
            }
        };

        // Load shader
        let shader_source = include_str!("shaders/image_adjustments.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Image Adjustments Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Image Processing Pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        println!("GPU: Initialized successfully with {}", adapter.get_info().name);



        // LRU Cache for GPU resources (textures, buffers, etc.)
        // Cache up to 5 different image sizes/resolutions to avoid recreation when switching photos
        let mut resources_cache: LruCache<(u32, u32), GpuResources> = LruCache::new(NonZeroUsize::new(5).unwrap());

        // Process requests
        while let Ok(request) = receiver.recv() {
            // Check if this request is still current (debouncing)
            let current_id = *current_request_id.lock();
            if request.request_id < current_id {
                continue; // Skip outdated requests
            }

            // Process on GPU
            let start_time = std::time::Instant::now();
            if let Some(result) = Self::process_on_gpu(
                &device,
                &queue,
                &pipeline,
                &request,
                &mut resources_cache,
            ) {
                let preview = crate::image_processing::ImageProcessor::dynamic_to_color_image(&result);
                let _ = sender.send(GpuProcessResult {
                    request_id: request.request_id,
                    processed_image: result,
                    preview,
                    process_time_ms: start_time.elapsed().as_secs_f32() * 1000.0,
                });
            }
        }
    }

    /// Process a single image on GPU
    fn process_on_gpu(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipeline: &wgpu::ComputePipeline,
        request: &GpuProcessRequest,
        resources_cache: &mut LruCache<(u32, u32), GpuResources>,
    ) -> Option<DynamicImage> {
        let width = request.width;
        let height = request.height;
        
        // Check if we have resources for this size
        if !resources_cache.contains(&(width, height)) {
             // WGPU requires bytes_per_row to be aligned to 256 bytes
            const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;
            let unpadded_bytes_per_row = 4 * width;
            let padded_bytes_per_row = ((unpadded_bytes_per_row + COPY_BYTES_PER_ROW_ALIGNMENT - 1) 
                / COPY_BYTES_PER_ROW_ALIGNMENT) * COPY_BYTES_PER_ROW_ALIGNMENT;

            println!("GPU CACHE: Creating new resources for {}x{} (padded row: {})", 
                width, height, padded_bytes_per_row);

            // Create input texture
            let input_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Input Texture"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            // Create output texture (storage)
            let output_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Output Texture"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });

            // Create uniform buffer for parameters
            // Usage COPY_DST to allow updates
            let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Params Buffer"),
                size: std::mem::size_of::<GpuEditParams>() as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            // Create bind group
            let bind_group_layout = pipeline.get_bind_group_layout(0);
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &input_texture.create_view(&wgpu::TextureViewDescriptor::default())
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &output_texture.create_view(&wgpu::TextureViewDescriptor::default())
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params_buffer.as_entire_binding(),
                    },
                ],
            });

            // Create output buffer with padded size for reading back
            let output_buffer_size = (padded_bytes_per_row * height) as wgpu::BufferAddress;
            let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Output Buffer"),
                size: output_buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            resources_cache.put((width, height), GpuResources {
                input_texture,
                output_texture,
                params_buffer,
                output_buffer,
                bind_group,
                padded_bytes_per_row,
                unpadded_bytes_per_row,
                last_image_data: None,
            });
        }

        // Get resources from cache
        let resources = resources_cache.get_mut(&(width, height)).unwrap();

        // Check if image data needs upload (pointer equality check)
        // If it's a different Arc, or same Arc but we just created resources, we upload.
        // Actually, if we just created resources, last_image_data is None.
        let need_upload = match &resources.last_image_data {
            Some(last) => !Arc::ptr_eq(last, &request.image_data),
            None => true,
        };

        if need_upload {
            // Upload image data
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &resources.input_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &request.image_data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(resources.unpadded_bytes_per_row),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            );
            resources.last_image_data = Some(request.image_data.clone());
        }

        // Update params
        queue.write_buffer(&resources.params_buffer, 0, bytemuck::bytes_of(&request.params));

        // Create command encoder
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder"),
        });

        // Dispatch compute shader
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Image Processing Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, &resources.bind_group, &[]);
            
            // Dispatch workgroups (16x16 threads each)
            let workgroups_x = (width + 15) / 16;
            let workgroups_y = (height + 15) / 16;
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        // Copy output texture to buffer
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &resources.output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &resources.output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(resources.padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );

        // Submit commands
        queue.submit(std::iter::once(encoder.finish()));

        // Read back results
        let buffer_slice = resources.output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device.poll(wgpu::Maintain::Wait);

        if rx.recv().ok()?.is_ok() {
            let data = buffer_slice.get_mapped_range();
            
            // Remove padding from each row
            let mut result_data: Vec<u8> = Vec::with_capacity((resources.unpadded_bytes_per_row * height) as usize);
            for y in 0..height {
                let start = (y * resources.padded_bytes_per_row) as usize;
                let end = start + resources.unpadded_bytes_per_row as usize;
                result_data.extend_from_slice(&data[start..end]);
            }
            
            drop(data);
            resources.output_buffer.unmap();

            // Convert to DynamicImage
            let img_buffer = image::RgbaImage::from_raw(width, height, result_data)?;
            Some(DynamicImage::ImageRgba8(img_buffer))
        } else {
            None
        }
    }

    /// CPU fallback when GPU is not available
    fn cpu_fallback_loop(
        receiver: Receiver<GpuProcessRequest>,
        sender: Sender<GpuProcessResult>,
        current_request_id: Arc<Mutex<u64>>,
    ) {
        while let Ok(request) = receiver.recv() {
            let current_id = *current_request_id.lock();
            if request.request_id < current_id {
                continue;
            }

            // Convert back to DynamicImage and process on CPU
            let start_time = std::time::Instant::now();
            if let Some(img) = image::RgbaImage::from_raw(
                request.width,
                request.height,
                request.image_data.as_ref().clone(),
            ) {
                let dynamic_img = DynamicImage::ImageRgba8(img);
                let processed = crate::image_processing::ImageProcessor::process_image(
                    &dynamic_img,
                    request.params.exposure,
                    request.params.contrast,
                    request.params.temperature,
                    request.params.tint,
                    request.params.highlights,
                    request.params.shadows,
                    request.params.whites,
                    request.params.blacks,
                    request.params.clarity,
                    request.params.vibrance,
                    request.params.saturation,
                    request.params.tone_curve_shadows,
                    request.params.tone_curve_darks,
                    request.params.tone_curve_lights,
                    request.params.tone_curve_highlights,
                    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // HSL
                );

                let preview = crate::image_processing::ImageProcessor::dynamic_to_color_image(&processed);
                let _ = sender.send(GpuProcessResult {
                    request_id: request.request_id,
                    processed_image: processed,
                    preview,
                    process_time_ms: start_time.elapsed().as_secs_f32() * 1000.0,
                });
            }
        }
    }

    /// Request GPU processing (non-blocking)
    pub fn request_process(&self, request: GpuProcessRequest) -> u64 {
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

    /// Poll for completed result (non-blocking)
    pub fn poll_result(&self) -> Option<GpuProcessResult> {
        let mut latest: Option<GpuProcessResult> = None;
        
        while let Ok(result) = self.result_receiver.try_recv() {
            if latest.as_ref().is_none_or(|l| result.request_id > l.request_id) {
                latest = Some(result);
            }
        }
        
        latest
    }

    /// Check if GPU is available
    pub fn is_gpu_available(&self) -> bool {
        self.gpu_available
    }
}

impl Default for GpuImageProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// We need buffer init descriptor

