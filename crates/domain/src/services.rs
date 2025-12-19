//! Domain Services
//!
//! Serviços que não pertencem estritamente a uma entidade ou value object.

use crate::value_objects::{FilePath, PhotoMetadata};
use crate::DomainResult;
use async_trait::async_trait;

/// Serviço para extração de metadados de fotos
#[async_trait]
pub trait MetadataExtractor: Send + Sync {
    /// Extrai metadados do arquivo especificado
    fn extract(&self, path: &FilePath) -> DomainResult<PhotoMetadata>;
}

/// Serviço para geração de thumbnails
#[async_trait]
pub trait ThumbnailGenerator: Send + Sync {
    /// Gera um thumbnail para a imagem especificada
    /// Retorna os bytes da imagem (JPEG) redimensionada
    async fn generate(&self, path: &FilePath, max_size: u32) -> DomainResult<Vec<u8>>;
}

/// Representa uma imagem RAW decodificada
pub struct RawImage {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u16>, // Dados RAW geralmente são 12-14 bits, cabem em u16
    pub cpp: usize,     // Components per pixel (usually 1 for bayer)
}

/// Serviço para decodificação de arquivos RAW
#[async_trait]
pub trait RawDecoder: Send + Sync {
    /// Decodifica um arquivo RAW retornando os dados brutos
    fn decode(&self, path: &FilePath) -> DomainResult<RawImage>;
}

/// Serviço para exportação de imagens processadas
#[async_trait]
pub trait ImageExporter: Send + Sync {
    /// Exporta a foto aplicando as edições para o caminho de destino
    async fn export(&self, photo: &crate::entities::Photo, output_path: &FilePath) -> DomainResult<()>;
}
