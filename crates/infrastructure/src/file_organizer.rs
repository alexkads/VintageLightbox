//! File Organizer Implementation
//!
//! Implementação do serviço de organização de arquivos.
//! Suporta diferentes estratégias de organização e padrões de renomeação.

use domain::{
    services::FileOrganizer,
    value_objects::{FilePath, PhotoMetadata, OrganizationStrategy, RenamePattern},
    DomainError, DomainResult,
};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Implementação do FileOrganizer
pub struct FileOrganizerImpl {
    /// Diretório base onde os arquivos serão organizados
    base_dir: PathBuf,
}

impl FileOrganizerImpl {
    /// Cria uma nova instância do FileOrganizer
    ///
    /// # Arguments
    /// * `base_dir` - Diretório base (ex: ~/Pictures/VintageLightbox)
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Extrai a data dos metadados EXIF ou usa a data atual
    fn extract_date(metadata: Option<&PhotoMetadata>) -> (String, String, String) {
        if let Some(meta) = metadata {
            if let Some(ref dt_str) = meta.date_time {
                // Parse EXIF date format "YYYY:MM:DD HH:MM:SS"
                let parts: Vec<&str> = dt_str.split(' ').next()
                    .unwrap_or("")
                    .split(':')
                    .collect();

                if parts.len() >= 3 {
                    return (
                        parts[0].to_string(),
                        parts[1].to_string(),
                        parts[2].to_string(),
                    );
                }
            }
        }

        // Fallback para data atual
        let now = chrono::Local::now();
        (
            now.format("%Y").to_string(),
            now.format("%m").to_string(),
            now.format("%d").to_string(),
        )
    }

    /// Gera nome de arquivo sequencial
    async fn generate_sequential_name(
        dir: &Path,
        year: &str,
        month: &str,
        day: &str,
        extension: &str,
    ) -> String {
        let mut sequential = 1;
        loop {
            let name = format!("photo-{}-{}-{}-{:03}.{}", year, month, day, sequential, extension);
            let path = dir.join(&name);
            if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
                return name;
            }
            sequential += 1;
        }
    }

    /// Gera nome único adicionando sufixo _1, _2, etc.
    async fn generate_unique_name(dir: &Path, original_name: &str) -> String {
        let path = Path::new(original_name);
        let stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let extension = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg");

        // Primeiro tenta sem sufixo
        let first_try = format!("{}.{}", stem, extension);
        if !tokio::fs::try_exists(dir.join(&first_try)).await.unwrap_or(false) {
            return first_try;
        }

        // Adiciona sufixo numérico
        let mut counter = 1;
        loop {
            let name = format!("{}_{}.{}", stem, counter, extension);
            if !tokio::fs::try_exists(dir.join(&name)).await.unwrap_or(false) {
                return name;
            }
            counter += 1;
        }
    }
}

#[async_trait]
impl FileOrganizer for FileOrganizerImpl {
    async fn organize_file(
        &self,
        source: &FilePath,
        metadata: Option<&PhotoMetadata>,
        strategy: OrganizationStrategy,
        rename_pattern: RenamePattern,
    ) -> DomainResult<FilePath> {
        let source_path: &Path = source.as_ref();

        // Extrair extensão
        let extension = source_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg")
            .to_lowercase();

        match strategy {
            OrganizationStrategy::ByDate => {
                // Extrair data dos metadados
                let (year, month, day) = Self::extract_date(metadata);

                // Criar diretório: base_dir/YYYY/MM/DD/
                let dest_dir = self.base_dir.join(&year).join(&month).join(&day);
                tokio::fs::create_dir_all(&dest_dir).await
                    .map_err(|e| DomainError::InfrastructureError(
                        format!("Failed to create directory {}: {}", dest_dir.display(), e)
                    ))?;

                // Gerar nome do arquivo baseado no padrão
                let filename = match rename_pattern {
                    RenamePattern::Standard => {
                        Self::generate_sequential_name(&dest_dir, &year, &month, &day, &extension).await
                    }
                    RenamePattern::KeepOriginal => {
                        let original_name = source_path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("photo.jpg");
                        Self::generate_unique_name(&dest_dir, original_name).await
                    }
                    RenamePattern::Custom(_) => {
                        // v2 feature - por enquanto usa Standard
                        Self::generate_sequential_name(&dest_dir, &year, &month, &day, &extension).await
                    }
                };

                let dest_path = dest_dir.join(&filename);

                // Copiar arquivo
                tokio::fs::copy(source_path, &dest_path).await
                    .map_err(|e| DomainError::InfrastructureError(
                        format!("Failed to copy file: {}", e)
                    ))?;

                FilePath::new(dest_path.to_str().unwrap())
            }

            OrganizationStrategy::PreserveStructure => {
                // Preservar estrutura de diretórios original
                // Extrai o caminho relativo do arquivo (se possível)
                let original_name = source_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("photo.jpg");

                // Para preservar estrutura, usamos apenas o nome do arquivo
                // (uma implementação mais complexa poderia preservar hierarquia completa)
                let dest_dir = self.base_dir.clone();
                tokio::fs::create_dir_all(&dest_dir).await
                    .map_err(|e| DomainError::InfrastructureError(
                        format!("Failed to create directory: {}", e)
                    ))?;

                let filename = match rename_pattern {
                    RenamePattern::KeepOriginal => {
                        Self::generate_unique_name(&dest_dir, original_name).await
                    }
                    RenamePattern::Standard => {
                        let (year, month, day) = Self::extract_date(metadata);
                        Self::generate_sequential_name(&dest_dir, &year, &month, &day, &extension).await
                    }
                    RenamePattern::Custom(_) => {
                        Self::generate_unique_name(&dest_dir, original_name).await
                    }
                };

                let dest_path = dest_dir.join(&filename);

                // Copiar arquivo
                tokio::fs::copy(source_path, &dest_path).await
                    .map_err(|e| DomainError::InfrastructureError(
                        format!("Failed to copy file: {}", e)
                    ))?;

                FilePath::new(dest_path.to_str().unwrap())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::io::Write;

    // Helper para criar arquivo temporário
    fn create_test_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content).unwrap();
        path
    }

    #[tokio::test]
    async fn test_by_date_with_exif() {
        // Arrange
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let source_file = create_test_file(temp_source.path(), "photo.jpg", b"test content");
        let source_path = FilePath::new(source_file.to_str().unwrap()).unwrap();

        let mut metadata = PhotoMetadata::default();
        metadata.date_time = Some("2024:03:15 10:30:45".to_string());

        let organizer = FileOrganizerImpl::new(temp_dest.path().to_path_buf());

        // Act
        let result = organizer.organize_file(
            &source_path,
            Some(&metadata),
            OrganizationStrategy::ByDate,
            RenamePattern::Standard,
        ).await;

        // Assert
        assert!(result.is_ok());
        let dest_path = result.unwrap();
        let dest_str = dest_path.to_string();

        assert!(dest_str.contains("2024"));
        assert!(dest_str.contains("03"));
        assert!(dest_str.contains("15"));
        assert!(dest_str.contains("photo-2024-03-15-001.jpg"));

        // Verificar que arquivo foi copiado
        assert!(tokio::fs::try_exists(dest_path.as_ref()).await.unwrap());
    }

    #[tokio::test]
    async fn test_by_date_without_exif() {
        // Arrange
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let source_file = create_test_file(temp_source.path(), "photo.jpg", b"test content");
        let source_path = FilePath::new(source_file.to_str().unwrap()).unwrap();

        let organizer = FileOrganizerImpl::new(temp_dest.path().to_path_buf());

        // Act
        let result = organizer.organize_file(
            &source_path,
            None, // Sem metadados
            OrganizationStrategy::ByDate,
            RenamePattern::Standard,
        ).await;

        // Assert
        assert!(result.is_ok());
        let dest_path = result.unwrap();

        // Deve usar data atual
        let now = chrono::Local::now();
        let dest_str = dest_path.to_string();
        assert!(dest_str.contains(&now.format("%Y").to_string()));

        // Verificar que arquivo foi copiado
        assert!(tokio::fs::try_exists(dest_path.as_ref()).await.unwrap());
    }

    #[tokio::test]
    async fn test_preserve_structure() {
        // Arrange
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let source_file = create_test_file(temp_source.path(), "original_name.jpg", b"test content");
        let source_path = FilePath::new(source_file.to_str().unwrap()).unwrap();

        let organizer = FileOrganizerImpl::new(temp_dest.path().to_path_buf());

        // Act
        let result = organizer.organize_file(
            &source_path,
            None,
            OrganizationStrategy::PreserveStructure,
            RenamePattern::KeepOriginal,
        ).await;

        // Assert
        assert!(result.is_ok());
        let dest_path = result.unwrap();
        let dest_str = dest_path.to_string();

        assert!(dest_str.contains("original_name.jpg"));
        assert!(tokio::fs::try_exists(dest_path.as_ref()).await.unwrap());
    }

    #[tokio::test]
    async fn test_filename_collision_handling() {
        // Arrange
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let source_file1 = create_test_file(temp_source.path(), "photo.jpg", b"content 1");
        let source_file2 = create_test_file(temp_source.path(), "photo2.jpg", b"content 2");

        let source_path1 = FilePath::new(source_file1.to_str().unwrap()).unwrap();
        let source_path2 = FilePath::new(source_file2.to_str().unwrap()).unwrap();

        let mut metadata = PhotoMetadata::default();
        metadata.date_time = Some("2024:03:15 10:30:45".to_string());

        let organizer = FileOrganizerImpl::new(temp_dest.path().to_path_buf());

        // Act - Import dois arquivos com mesma data
        let result1 = organizer.organize_file(
            &source_path1,
            Some(&metadata),
            OrganizationStrategy::ByDate,
            RenamePattern::Standard,
        ).await;

        let result2 = organizer.organize_file(
            &source_path2,
            Some(&metadata),
            OrganizationStrategy::ByDate,
            RenamePattern::Standard,
        ).await;

        // Assert
        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let path1 = result1.unwrap().to_string();
        let path2 = result2.unwrap().to_string();

        // Nomes devem ser diferentes (sequencial)
        assert_ne!(path1, path2);
        assert!(path1.contains("001"));
        assert!(path2.contains("002"));
    }

    #[tokio::test]
    async fn test_directory_creation() {
        // Arrange
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let source_file = create_test_file(temp_source.path(), "photo.jpg", b"test");
        let source_path = FilePath::new(source_file.to_str().unwrap()).unwrap();

        let mut metadata = PhotoMetadata::default();
        metadata.date_time = Some("2024:03:15 10:30:45".to_string());

        let organizer = FileOrganizerImpl::new(temp_dest.path().to_path_buf());

        // Act
        let result = organizer.organize_file(
            &source_path,
            Some(&metadata),
            OrganizationStrategy::ByDate,
            RenamePattern::Standard,
        ).await;

        // Assert
        assert!(result.is_ok());

        // Verificar que diretórios foram criados
        let expected_dir = temp_dest.path().join("2024").join("03").join("15");
        assert!(tokio::fs::try_exists(&expected_dir).await.unwrap());
    }
}
