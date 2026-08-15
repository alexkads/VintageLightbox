//! File Organizer Service
//!
//! Trait para serviços de organização de arquivos durante importação.
//! Responsável por copiar arquivos para diretórios organizados seguindo diferentes estratégias.

use crate::{
    value_objects::{FilePath, ImportOptions, PhotoMetadata, OrganizationStrategy, RenamePattern},
    DomainResult,
};
use async_trait::async_trait;

/// Trait para organização de arquivos
#[async_trait]
pub trait FileOrganizer: Send + Sync {
    /// Organiza um arquivo respeitando todas as opções do lote
    ///
    /// É a entrada usada pela importação: além de estratégia e renomeação, leva o destino
    /// escolhido pelo usuário e a raiz de origem (que `PreserveStructure` precisa para saber
    /// qual pedaço do caminho preservar). A implementação padrão ignora esses dois e delega
    /// para [`FileOrganizer::organize_file`], para não quebrar implementações antigas.
    async fn organize_file_with(
        &self,
        source: &FilePath,
        metadata: Option<&PhotoMetadata>,
        options: &ImportOptions,
    ) -> DomainResult<FilePath> {
        self.organize_file(
            source,
            metadata,
            options.organization,
            options.rename_pattern.clone(),
        )
        .await
    }

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
