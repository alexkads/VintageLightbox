//! Intelligent Fill Processor (UI Wrapper)
//!
//! Delegates to infrastructure logic and handles UI-specific conversions (ColorImage).

use std::sync::Arc;
use image::DynamicImage;
use eframe::egui::ColorImage;

use infrastructure::image_processing::intelligent_fill::{
    IntelligentFillProcessor as InfraProcessor,
    IntelligentFillRequest as InfraRequest,
};

pub use domain::value_objects::CropSettings;
pub use domain::services::intelligent_fill::ModelStatus;

/// Request para processamento de intelligent fill
#[derive(Clone)]
pub struct IntelligentFillRequest {
    pub request_id: u64,
    pub image_data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub crop_settings: CropSettings,
}

impl IntelligentFillRequest {
    pub fn to_infra(self) -> InfraRequest {
        InfraRequest {
            request_id: self.request_id,
            image_data: self.image_data,
            width: self.width,
            height: self.height,
            crop_settings: self.crop_settings,
        }
    }
}

/// Resultado do processamento de intelligent fill
pub struct IntelligentFillProcessResult {
    pub request_id: u64,
    pub filled_image: DynamicImage,
    pub preview: ColorImage,
    pub process_time_ms: f32,
}

/// Processador Intelligent Fill Wrapper
pub struct IntelligentFillProcessor {
    infra: InfraProcessor,
}

impl IntelligentFillProcessor {
    /// Cria um novo IntelligentFillProcessor
    pub fn new() -> Self {
        Self {
            infra: InfraProcessor::new(),
        }
    }

    /// Envia request para processamento (non-blocking)
    pub fn request_fill(&self, request: IntelligentFillRequest) -> u64 {
        self.infra.request_fill(request.to_infra())
    }

    /// Poll para resultado (non-blocking)
    pub fn poll_result(&self) -> Option<IntelligentFillProcessResult> {
        if let Some(res) = self.infra.poll_result() {
             // Convert to ColorImage for UI
             let preview = crate::image_processing::ImageProcessor::dynamic_to_color_image(&res.filled_image);
             
             Some(IntelligentFillProcessResult {
                 request_id: res.request_id,
                 filled_image: res.filled_image,
                 preview, // UI specific
                 process_time_ms: res.process_time_ms,
             })
        } else {
            None
        }
    }

    /// Obtém status do modelo
    pub fn model_status(&self) -> ModelStatus {
        self.infra.model_status()
    }

    /// Verifica se GPU está disponível
    pub fn is_gpu_available(&self) -> bool {
        self.infra.is_gpu_available()
    }

    /// Gera próximo request ID
    pub fn next_request_id(&self) -> u64 {
        self.infra.next_request_id()
    }
}

impl Default for IntelligentFillProcessor {
    fn default() -> Self {
        Self::new()
    }
}
