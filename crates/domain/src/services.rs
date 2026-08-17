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

    /// Gera múltiplos thumbnails de uma vez, otimizando a leitura do arquivo
    /// Retorna os bytes de cada thumbnail na ordem solicitada
    async fn generate_set(&self, path: &FilePath, max_sizes: &[u32]) -> DomainResult<Vec<Vec<u8>>> {
        let mut results = Vec::new();
        for size in max_sizes {
            results.push(self.generate(path, *size).await?);
        }
        Ok(results)
    }
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
    /// Exporta a foto aplicando as edições para o caminho de destino.
    ///
    /// 🔑 **As opções não são opcionais.** Elas carregam a marca d'água, que é a
    /// regra que separa entregar de mostrar: a foto comprada vai inteira, a que
    /// ficou para trás vai marcada. Um `export` sem opções deixaria "sem marca"
    /// como caminho mais curto — e o caminho mais curto é o que se pega no dia
    /// em que a atenção falta.
    async fn export(
        &self,
        photo: &crate::entities::Photo,
        output_path: &FilePath,
        options: &crate::value_objects::ExportOptions,
    ) -> DomainResult<()>;
}

pub mod preview_storage;
pub use preview_storage::*;

pub mod file_organizer;
pub use file_organizer::FileOrganizer;

pub mod source_scanner;
pub use source_scanner::SourceScanner;
