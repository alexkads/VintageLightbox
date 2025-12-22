//! Preview Before Import Use Case
//!
//! Caso de uso responsável por gerar previews e metadados antes da importação.
//! Permite ao usuário visualizar as fotos e decidir quais importar.

use domain::{
    services::{MetadataExtractor, ThumbnailGenerator},
    value_objects::{FilePath, PhotoMetadata},
    DomainResult,
};
use std::sync::Arc;

/// Item de preview para exibição antes da importação
#[derive(Debug, Clone)]
pub struct ImportPreviewItem {
    /// Caminho do arquivo original
    pub file_path: FilePath,
    /// Thumbnail em bytes JPEG (300px)
    pub thumbnail: Vec<u8>,
    /// Metadados EXIF extraídos
    pub metadata: PhotoMetadata,
    /// Tamanho do arquivo em bytes
    pub file_size: u64,
    /// Se é arquivo RAW
    pub is_raw: bool,
}

/// Use Case para preview antes de importar
pub struct PreviewBeforeImportUseCase {
    metadata_extractor: Arc<dyn MetadataExtractor>,
    thumbnail_generator: Arc<dyn ThumbnailGenerator>,
}

impl PreviewBeforeImportUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(
        metadata_extractor: Arc<dyn MetadataExtractor>,
        thumbnail_generator: Arc<dyn ThumbnailGenerator>,
    ) -> Self {
        Self {
            metadata_extractor,
            thumbnail_generator,
        }
    }

    /// Gera previews para uma lista de arquivos
    ///
    /// Processa arquivos em paralelo usando Tokio tasks para melhor performance.
    /// Continua processando mesmo se alguns arquivos falharem.
    pub async fn execute(&self, files: Vec<FilePath>) -> DomainResult<Vec<ImportPreviewItem>> {
        if files.is_empty() {
            return Ok(Vec::new());
        }

        // Processar arquivos em paralelo usando Tokio
        let tasks: Vec<_> = files
            .into_iter()
            .map(|path| {
                let thumbnail_generator = self.thumbnail_generator.clone();
                let metadata_extractor = self.metadata_extractor.clone();

                tokio::spawn(async move {
                    // Tentar gerar thumbnail
                    let thumbnail = match thumbnail_generator.generate(&path, 300).await {
                        Ok(bytes) => bytes,
                        Err(_) => return None,
                    };

                    // Extrair metadados (pode falhar gracefully)
                    let metadata = metadata_extractor
                        .extract(&path)
                        .unwrap_or_default();

                    // Obter tamanho do arquivo
                    let file_size = std::fs::metadata(path.as_ref())
                        .map(|m| m.len())
                        .unwrap_or(0);

                    // Determinar se é RAW
                    let is_raw = Self::is_raw_file(&path);

                    Some(ImportPreviewItem {
                        file_path: path,
                        thumbnail,
                        metadata,
                        file_size,
                        is_raw,
                    })
                })
            })
            .collect();

        // Aguardar todas as tasks e coletar resultados
        let results = futures::future::join_all(tasks).await;

        // Filtrar None (falhas) e erros de join, retornar apenas sucessos
        Ok(results
            .into_iter()
            .filter_map(|r| r.ok())
            .flatten()
            .collect())
    }

    /// Verifica se o arquivo é RAW baseado na extensão
    fn is_raw_file(path: &FilePath) -> bool {
        let raw_extensions = ["cr2", "nef", "arw", "dng", "raf", "orf", "rw2"];
        if let Some(ext) = std::path::Path::new(path.as_ref()).extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            raw_extensions.contains(&ext_str.as_str())
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{DomainError, value_objects::PhotoMetadata};
    use mockall::{mock, predicate::*};
    use tempfile::NamedTempFile;
    use std::io::Write;

    // Mock do MetadataExtractor
    mock! {
        pub MetadataExtractor {}

        impl MetadataExtractor for MetadataExtractor {
            fn extract(&self, path: &FilePath) -> DomainResult<PhotoMetadata>;
        }
    }

    // Mock do ThumbnailGenerator
    mock! {
        pub ThumbnailGenerator {}
        #[async_trait::async_trait]
        impl ThumbnailGenerator for ThumbnailGenerator {
            async fn generate(&self, path: &FilePath, max_dimension: u32) -> DomainResult<Vec<u8>>;
        }
    }

    // Helper para criar arquivo temporário
    fn create_temp_file(extension: &str) -> NamedTempFile {
        let mut builder = tempfile::Builder::new();
        builder.suffix(extension);
        let mut file = builder.tempfile().unwrap();
        // Escrever alguns bytes para ter tamanho > 0
        file.write_all(b"test data").unwrap();
        file
    }

    #[tokio::test]
    async fn test_preview_all_succeed() {
        // Arrange
        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .times(2)
            .returning(|_| Ok(PhotoMetadata::default()));

        let mut mock_generator = MockThumbnailGenerator::new();
        mock_generator
            .expect_generate()
            .with(always(), eq(300))
            .times(2)
            .returning(|_, _| Ok(vec![1, 2, 3, 4])); // JPEG bytes fictícios

        let use_case = PreviewBeforeImportUseCase::new(
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
        );

        let temp1 = create_temp_file(".jpg");
        let temp2 = create_temp_file(".png");
        let files = vec![
            FilePath::new(temp1.path().to_str().unwrap()).unwrap(),
            FilePath::new(temp2.path().to_str().unwrap()).unwrap(),
        ];

        // Act
        let result = use_case.execute(files).await;

        // Assert
        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].thumbnail, vec![1, 2, 3, 4]);
        assert!(items[0].file_size > 0);
        assert!(!items[0].is_raw);
    }

    #[tokio::test]
    async fn test_preview_partial_failures() {
        // Arrange
        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));

        let mut mock_generator = MockThumbnailGenerator::new();
        // Primeiro arquivo falha, segundo sucede
        mock_generator
            .expect_generate()
            .times(2)
            .returning(|path, _| {
                let path_str = path.as_ref().to_string_lossy();
                if path_str.contains("jpg") {
                    Err(DomainError::InfrastructureError("Failed".to_string()))
                } else {
                    Ok(vec![1, 2, 3])
                }
            });

        let use_case = PreviewBeforeImportUseCase::new(
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
        );

        let temp1 = create_temp_file(".jpg");
        let temp2 = create_temp_file(".png");
        let files = vec![
            FilePath::new(temp1.path().to_str().unwrap()).unwrap(),
            FilePath::new(temp2.path().to_str().unwrap()).unwrap(),
        ];

        // Act
        let result = use_case.execute(files).await;

        // Assert
        assert!(result.is_ok());
        let items = result.unwrap();
        // Apenas 1 item deveria ser retornado (o que não falhou)
        assert_eq!(items.len(), 1);
        let path_str = items[0].file_path.as_ref().to_string_lossy();
        assert!(path_str.contains("png"));
    }

    #[tokio::test]
    async fn test_preview_empty_input() {
        // Arrange
        let mock_extractor = MockMetadataExtractor::new();
        let mock_generator = MockThumbnailGenerator::new();

        let use_case = PreviewBeforeImportUseCase::new(
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
        );

        // Act
        let result = use_case.execute(vec![]).await;

        // Assert
        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 0);
    }

    #[tokio::test]
    async fn test_preview_raw_vs_jpeg() {
        // Arrange
        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .times(2)
            .returning(|_| Ok(PhotoMetadata::default()));

        let mut mock_generator = MockThumbnailGenerator::new();
        mock_generator
            .expect_generate()
            .times(2)
            .returning(|_, _| Ok(vec![1, 2, 3]));

        let use_case = PreviewBeforeImportUseCase::new(
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
        );

        let temp_raw = create_temp_file(".cr2");
        let temp_jpeg = create_temp_file(".jpg");
        let files = vec![
            FilePath::new(temp_raw.path().to_str().unwrap()).unwrap(),
            FilePath::new(temp_jpeg.path().to_str().unwrap()).unwrap(),
        ];

        // Act
        let result = use_case.execute(files).await;

        // Assert
        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 2);

        // Encontrar item RAW e JPEG
        let raw_item = items
            .iter()
            .find(|i| i.file_path.as_ref().to_string_lossy().contains("cr2"))
            .unwrap();
        let jpeg_item = items
            .iter()
            .find(|i| i.file_path.as_ref().to_string_lossy().contains("jpg"))
            .unwrap();

        assert!(raw_item.is_raw);
        assert!(!jpeg_item.is_raw);
    }

    #[tokio::test]
    async fn test_preview_parallel_processing() {
        // Arrange
        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .times(10)
            .returning(|_| Ok(PhotoMetadata::default()));

        let mut mock_generator = MockThumbnailGenerator::new();
        mock_generator
            .expect_generate()
            .times(10)
            .returning(|_, _| Ok(vec![1, 2, 3]));

        let use_case = PreviewBeforeImportUseCase::new(
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
        );

        // Criar 10 arquivos temporários
        let temp_files: Vec<_> = (0..10).map(|_| create_temp_file(".jpg")).collect();
        let files: Vec<FilePath> = temp_files
            .iter()
            .map(|f| FilePath::new(f.path().to_str().unwrap()).unwrap())
            .collect();

        // Act
        let start = std::time::Instant::now();
        let result = use_case.execute(files).await;
        let duration = start.elapsed();

        // Assert
        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 10);

        // Com processamento paralelo, deveria ser rápido
        // (Este é um teste de performance básico)
        println!("Processed 10 files in {:?}", duration);
    }
}
