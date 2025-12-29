//! Intelligent Fill Processor
//!
//! Orquestra a geração de máscara, inferência neural e compositing
//! usando um background thread similar ao GpuImageProcessor.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use parking_lot::Mutex;
use image::DynamicImage;
use lru::LruCache;
use std::num::NonZeroUsize;
use eframe::egui::ColorImage;

use domain::services::intelligent_fill::ModelStatus;
use domain::value_objects::CropSettings;

use super::mask_generator::{MaskGenerator, MaskParams, MaskResources};

/// Request para processamento de intelligent fill
#[derive(Clone)]
pub struct IntelligentFillRequest {
    pub request_id: u64,
    pub image_data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub crop_settings: CropSettings,
}

/// Resultado do processamento de intelligent fill
pub struct IntelligentFillProcessResult {
    pub request_id: u64,
    pub filled_image: DynamicImage,
    pub preview: ColorImage,
    pub process_time_ms: f32,
}

/// Recursos GPU cached para uma dimensão específica
struct FillResources {
    mask_resources: MaskResources,
    #[allow(dead_code)] // Reservado para cache de imagem futura
    last_image_data: Option<Arc<Vec<u8>>>,
    last_angle: f32,
    cached_mask: Option<Vec<u8>>,
}

/// Processador GPU para intelligent fill
pub struct IntelligentFillProcessor {
    request_sender: Sender<IntelligentFillRequest>,
    result_receiver: Receiver<IntelligentFillProcessResult>,
    current_request_id: Arc<Mutex<u64>>,
    model_status: Arc<Mutex<ModelStatus>>,
    gpu_available: bool,
}

impl IntelligentFillProcessor {
    /// Cria um novo IntelligentFillProcessor
    pub fn new() -> Self {
        let (request_sender, request_receiver) = channel::<IntelligentFillRequest>();
        let (result_sender, result_receiver) = channel::<IntelligentFillProcessResult>();
        let current_request_id = Arc::new(Mutex::new(0u64));
        let model_status = Arc::new(Mutex::new(ModelStatus::NotLoaded));

        let current_request_id_clone = current_request_id.clone();
        let model_status_clone = model_status.clone();

        // Spawn background thread para processamento
        std::thread::spawn(move || {
            Self::processor_thread(
                request_receiver,
                result_sender,
                current_request_id_clone,
                model_status_clone,
            )
        });

        Self {
            request_sender,
            result_receiver,
            current_request_id,
            model_status,
            gpu_available: true,
        }
    }

    /// Background thread que processa requests
    fn processor_thread(
        receiver: Receiver<IntelligentFillRequest>,
        sender: Sender<IntelligentFillProcessResult>,
        current_request_id: Arc<Mutex<u64>>,
        model_status: Arc<Mutex<ModelStatus>>,
    ) {
        // Inicializar WGPU
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
                eprintln!("IntelligentFill: No GPU adapter, using CPU fallback");
                Self::cpu_fallback_loop(receiver, sender, current_request_id);
                return;
            }
        };

        let (device, queue) = match pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("IntelligentFill GPU"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )) {
            Ok(dq) => dq,
            Err(e) => {
                eprintln!("IntelligentFill: GPU device error: {}, using CPU fallback", e);
                Self::cpu_fallback_loop(receiver, sender, current_request_id);
                return;
            }
        };

        // Criar mask generator
        let mask_generator = MaskGenerator::new(&device);

        // Atualizar status
        *model_status.lock() = ModelStatus::Ready;

        #[cfg(debug_assertions)]
        eprintln!(
            "IntelligentFill: GPU initialized with {}",
            adapter.get_info().name
        );

        // Cache de recursos (até 5 dimensões diferentes)
        let mut resources_cache: LruCache<(u32, u32), FillResources> =
            LruCache::new(NonZeroUsize::new(5).unwrap());

        // Loop de processamento
        while let Ok(request) = receiver.recv() {
            // Debouncing: verificar se request ainda é atual
            let current_id = *current_request_id.lock();
            if request.request_id < current_id {
                continue;
            }

            let start_time = std::time::Instant::now();

            // Verificar se há rotação (sem rotação = não precisa de fill)
            if request.crop_settings.angle() == 0.0 {
                // Sem rotação - retornar imagem original
                if let Some(result) = Self::create_passthrough_result(&request) {
                    let _ = sender.send(result);
                }
                continue;
            }

            // Obter ou criar recursos para esta dimensão
            let key = (request.width, request.height);
            if !resources_cache.contains(&key) {
                let mask_resources =
                    mask_generator.create_resources(&device, request.width, request.height);
                resources_cache.put(
                    key,
                    FillResources {
                        mask_resources,
                        last_image_data: None,
                        last_angle: 0.0,
                        cached_mask: None,
                    },
                );
            }

            let resources = resources_cache.get_mut(&key).unwrap();

            // Verificar se precisamos regenerar a máscara
            let needs_new_mask = resources.cached_mask.is_none()
                || (resources.last_angle - request.crop_settings.angle()).abs() > 0.001;

            let mask = if needs_new_mask {
                let params = MaskParams::new(
                    request.crop_settings.angle(),
                    request.crop_settings.crop_x(),
                    request.crop_settings.crop_y(),
                    request.crop_settings.crop_width(),
                    request.crop_settings.crop_height(),
                    request.width,
                    request.height,
                );

                let new_mask = mask_generator.generate(
                    &device,
                    &queue,
                    &resources.mask_resources,
                    &params,
                    request.width,
                    request.height,
                );

                resources.last_angle = request.crop_settings.angle();
                resources.cached_mask = Some(new_mask.clone());
                new_mask
            } else {
                resources.cached_mask.clone().unwrap()
            };

            // Verificar se há área para preencher
            let needs_fill = mask.iter().any(|&v| v > 0);

            let result_image = if needs_fill {
                // Aplicar inpainting (por enquanto, usando extrapolação de bordas)
                Self::apply_edge_extrapolation(&request.image_data, &mask, request.width, request.height)
            } else {
                request.image_data.as_ref().clone()
            };

            // Criar imagem de resultado
            if let Some(result) = Self::create_result(&request, result_image, start_time.elapsed().as_secs_f32() * 1000.0) {
                let _ = sender.send(result);
            }
        }
    }

    /// Loop de fallback CPU quando GPU não disponível
    fn cpu_fallback_loop(
        receiver: Receiver<IntelligentFillRequest>,
        sender: Sender<IntelligentFillProcessResult>,
        current_request_id: Arc<Mutex<u64>>,
    ) {
        while let Ok(request) = receiver.recv() {
            let current_id = *current_request_id.lock();
            if request.request_id < current_id {
                continue;
            }

            let start_time = std::time::Instant::now();

            // Sem rotação - passthrough
            if request.crop_settings.angle() == 0.0 {
                if let Some(result) = Self::create_passthrough_result(&request) {
                    let _ = sender.send(result);
                }
                continue;
            }

            // Gerar máscara em CPU
            let params = MaskParams::new(
                request.crop_settings.angle(),
                request.crop_settings.crop_x(),
                request.crop_settings.crop_y(),
                request.crop_settings.crop_width(),
                request.crop_settings.crop_height(),
                request.width,
                request.height,
            );

            let mask = MaskGenerator::generate_cpu(&params, request.width, request.height);

            let needs_fill = mask.iter().any(|&v| v > 0);
            let result_image = if needs_fill {
                Self::apply_edge_extrapolation(&request.image_data, &mask, request.width, request.height)
            } else {
                request.image_data.as_ref().clone()
            };

            if let Some(result) = Self::create_result(&request, result_image, start_time.elapsed().as_secs_f32() * 1000.0) {
                let _ = sender.send(result);
            }
        }
    }

    /// Aplica extrapolação de bordas para preenchimento
    fn apply_edge_extrapolation(
        image_data: &[u8],
        mask: &[u8],
        width: u32,
        height: u32,
    ) -> Vec<u8> {
        let mut result = image_data.to_vec();

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if mask[idx] > 0 {
                    let pixel_idx = idx * 4;

                    // Encontrar pixel válido mais próximo
                    if let Some((r, g, b, a)) =
                        Self::find_nearest_valid_pixel(image_data, mask, x, y, width, height)
                    {
                        result[pixel_idx] = r;
                        result[pixel_idx + 1] = g;
                        result[pixel_idx + 2] = b;
                        result[pixel_idx + 3] = a;
                    }
                }
            }
        }

        result
    }

    /// Encontra pixel válido mais próximo usando busca em espiral
    fn find_nearest_valid_pixel(
        image_data: &[u8],
        mask: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Option<(u8, u8, u8, u8)> {
        for radius in 1_i32..100 {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dy.abs() != radius {
                        continue;
                    }

                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let idx = (ny as u32 * width + nx as u32) as usize;
                        if mask[idx] == 0 {
                            let pixel_idx = idx * 4;
                            return Some((
                                image_data[pixel_idx],
                                image_data[pixel_idx + 1],
                                image_data[pixel_idx + 2],
                                image_data[pixel_idx + 3],
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    /// Cria resultado passthrough (sem modificação)
    fn create_passthrough_result(request: &IntelligentFillRequest) -> Option<IntelligentFillProcessResult> {
        Self::create_result(request, request.image_data.as_ref().clone(), 0.0)
    }

    /// Cria resultado a partir dos dados processados
    fn create_result(
        request: &IntelligentFillRequest,
        image_data: Vec<u8>,
        process_time_ms: f32,
    ) -> Option<IntelligentFillProcessResult> {
        // Criar DynamicImage a partir dos dados RGBA
        let img = image::RgbaImage::from_raw(request.width, request.height, image_data.clone())?;
        let dynamic_img = DynamicImage::ImageRgba8(img);

        // Criar ColorImage para preview
        let preview = ColorImage::from_rgba_unmultiplied(
            [request.width as usize, request.height as usize],
            &image_data,
        );

        Some(IntelligentFillProcessResult {
            request_id: request.request_id,
            filled_image: dynamic_img,
            preview,
            process_time_ms,
        })
    }

    /// Envia request para processamento (non-blocking)
    pub fn request_fill(&self, request: IntelligentFillRequest) -> u64 {
        let id = request.request_id;
        *self.current_request_id.lock() = id;
        let _ = self.request_sender.send(request);
        id
    }

    /// Poll para resultado (non-blocking)
    pub fn poll_result(&self) -> Option<IntelligentFillProcessResult> {
        self.result_receiver.try_recv().ok()
    }

    /// Obtém status do modelo
    pub fn model_status(&self) -> ModelStatus {
        self.model_status.lock().clone()
    }

    /// Verifica se GPU está disponível
    pub fn is_gpu_available(&self) -> bool {
        self.gpu_available
    }

    /// Gera próximo request ID
    pub fn next_request_id(&self) -> u64 {
        let mut guard = self.current_request_id.lock();
        *guard += 1;
        *guard
    }
}

impl Default for IntelligentFillProcessor {
    fn default() -> Self {
        Self::new()
    }
}
