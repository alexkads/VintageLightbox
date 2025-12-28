//! Intelligent Fill Service
//!
//! Serviço para preenchimento inteligente de bordas criadas por rotação
//! usando content-aware inpainting baseado em redes neurais.

use crate::value_objects::CropSettings;
use crate::DomainResult;
use async_trait::async_trait;

/// Parâmetros para operação de preenchimento inteligente
#[derive(Debug, Clone)]
pub struct IntelligentFillParams {
    /// Configurações de crop contendo o ângulo de rotação
    pub crop_settings: CropSettings,
    /// Largura da imagem em pixels
    pub width: u32,
    /// Altura da imagem em pixels
    pub height: u32,
    /// Nível de qualidade (0.0 = preview rápido, 1.0 = export alta qualidade)
    pub quality: f32,
}

impl IntelligentFillParams {
    /// Cria novos parâmetros para preview rápido
    pub fn preview(crop_settings: CropSettings, width: u32, height: u32) -> Self {
        Self {
            crop_settings,
            width,
            height,
            quality: 0.0,
        }
    }

    /// Cria novos parâmetros para export de alta qualidade
    pub fn export(crop_settings: CropSettings, width: u32, height: u32) -> Self {
        Self {
            crop_settings,
            width,
            height,
            quality: 1.0,
        }
    }
}

/// Resultado da operação de preenchimento inteligente
#[derive(Debug)]
pub struct IntelligentFillResult {
    /// Dados da imagem preenchida (RGBA bytes)
    pub image_data: Vec<u8>,
    /// Largura do resultado
    pub width: u32,
    /// Altura do resultado
    pub height: u32,
    /// Tempo de processamento em milissegundos
    pub process_time_ms: f32,
}

/// Status do modelo de inpainting
#[derive(Debug, Clone, PartialEq)]
pub enum ModelStatus {
    /// Modelo ainda não carregado
    NotLoaded,
    /// Modelo carregando
    Loading,
    /// Modelo pronto para uso
    Ready,
    /// Falha ao carregar modelo
    Failed(String),
}

impl Default for ModelStatus {
    fn default() -> Self {
        Self::NotLoaded
    }
}

/// Serviço para preenchimento inteligente de bordas de rotação
///
/// Este serviço utiliza uma rede neural para preencher as áreas vazias
/// criadas quando uma imagem é rotacionada em ângulos não múltiplos de 90°.
#[async_trait]
pub trait IntelligentFillService: Send + Sync {
    /// Verifica se o serviço está disponível (modelo carregado, GPU pronta)
    fn is_available(&self) -> bool;

    /// Gera preenchimento inteligente para bordas de rotação
    ///
    /// # Arguments
    /// * `image_data` - Bytes da imagem fonte (RGBA)
    /// * `params` - Parâmetros de preenchimento incluindo crop settings
    ///
    /// # Returns
    /// Resultado contendo os dados da imagem preenchida
    async fn fill(
        &self,
        image_data: &[u8],
        params: &IntelligentFillParams,
    ) -> DomainResult<IntelligentFillResult>;

    /// Obtém o status atual do modelo
    fn model_status(&self) -> ModelStatus;

    /// Estima tempo de processamento antes da execução (para feedback UX)
    fn estimate_fill_time(&self, width: u32, height: u32) -> u32 {
        // Heurística: imagens maiores levam mais tempo
        // Base ~100ms + escala com área
        100 + (width * height / 1_000_000)
    }
}
