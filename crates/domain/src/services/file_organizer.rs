//! File Organizer Service
//!
//! Trait para serviços de organização de arquivos durante importação.
//! Responsável por copiar arquivos para diretórios organizados seguindo diferentes estratégias.

use crate::{
    value_objects::{FilePath, PhotoMetadata, OrganizationStrategy, RenamePattern},
    DomainResult,
};
use async_trait::async_trait;

/// Trait para organização de arquivos
#[async_trait]
pub trait FileOrganizer: Send + Sync {
    /// Organiza um arquivo copiando para o destino apropriado
    ///
    /// # Arguments
    /// * `source` - Caminho do arquivo original
    /// * `metadata` - Metadados EXIF (opcional, usado para extrair data)
    /// * `strategy` - Estratégia de organização (ByDate ou PreserveStructure)
    /// * `rename_pattern` - Padrão de renomeação do arquivo
    ///
    /// # Returns
    /// Retorna o caminho do arquivo no destino após organização
    async fn organize_file(
        &self,
        source: &FilePath,
        metadata: Option<&PhotoMetadata>,
        strategy: OrganizationStrategy,
        rename_pattern: RenamePattern,
    ) -> DomainResult<FilePath>;
}
