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
