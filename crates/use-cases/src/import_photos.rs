//! Import Photos Use Case (Batch)
//!
//! Caso de uso responsável por importar múltiplas fotos de uma vez.
//! Implementado com TDD.

use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::FilePath,
    services::MetadataExtractor,
    DomainResult,
};
use std::sync::Arc;

/// Resultado de importação em lote
#[derive(Debug, Clone)]
pub struct BatchImportResult {
    pub successful: Vec<Photo>,
    pub failed: Vec<(FilePath, String)>,
}

impl BatchImportResult {
    pub fn success_count(&self) -> usize {
        self.successful.len()
    }

    pub fn failure_count(&self) -> usize {
        self.failed.len()
    }

    pub fn total_count(&self) -> usize {
        self.success_count() + self.failure_count()
    }
}

/// Use Case para importar múltiplas fotos
pub struct ImportPhotosUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
    metadata_extractor: Arc<dyn MetadataExtractor>,
}

impl ImportPhotosUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(
        photo_repository: Arc<dyn PhotoRepository>,
        metadata_extractor: Arc<dyn MetadataExtractor>,
    ) -> Self {
        Self {
            photo_repository,
            metadata_extractor,
        }
    }

    /// Importa múltiplas fotos do sistema de arquivos
    /// 
    /// Continua importando mesmo se algumas fotos falharem.
    /// Retorna resultado com sucessos e falhas.
    pub async fn execute(&self, file_paths: Vec<FilePath>) -> DomainResult<BatchImportResult> {
        let mut successful = Vec::new();
        let mut failed = Vec::new();

        for file_path in file_paths {
            match self.import_single(&file_path).await {
                Ok(photo) => successful.push(photo),
                Err(e) => failed.push((file_path, e.to_string())),
            }
        }

        Ok(BatchImportResult {
            successful,
            failed,
        })
    }

    async fn import_single(&self, file_path: &FilePath) -> DomainResult<Photo> {
        let mut photo = Photo::new(file_path.clone());

        // Extrair metadados
        if let Ok(metadata) = self.metadata_extractor.extract(file_path) {
            photo.set_metadata(metadata);
        }

        self.photo_repository.save(&photo).await?;
        Ok(photo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        DomainError, PhotoRepository,
        value_objects::PhotoMetadata,
    };
    use mockall::mock;
    use mockall::predicate::*;

    // Mock do MetadataExtractor
    mock! {
        pub MetadataExtractor {}

        impl MetadataExtractor for MetadataExtractor {
            fn extract(&self, path: &FilePath) -> DomainResult<PhotoMetadata>;
        }
    }

    // Mock do PhotoRepository
    mock! {
        pub PhotoRepo {}

        #[async_trait::async_trait]
        impl PhotoRepository for PhotoRepo {
            async fn save(&self, photo: &Photo) -> DomainResult<()>;
            async fn find_by_id(&self, id: &domain::value_objects::PhotoId) -> DomainResult<Option<Photo>>;
            async fn find_all(&self) -> DomainResult<Vec<Photo>>;
            async fn update(&self, photo: &Photo) -> DomainResult<()>;
            async fn delete(&self, id: &domain::value_objects::PhotoId) -> DomainResult<()>;
            async fn exists(&self, id: &domain::value_objects::PhotoId) -> DomainResult<bool>;
        }
    }

    #[tokio::test]
    async fn test_import_multiple_photos_all_success() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        // Espera-se que save seja chamado 3 vezes
        mock_repo
            .expect_save()
            .times(3)
            .returning(|_| Ok(()));

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));
        
        let use_case = ImportPhotosUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor)
        );
        
        let file_paths = vec![
            FilePath::new("/photos/img1.jpg").unwrap(),
            FilePath::new("/photos/img2.jpg").unwrap(),
            FilePath::new("/photos/img3.jpg").unwrap(),
        ];
        
        // Act
        let result = use_case.execute(file_paths).await;
        
        // Assert
        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.success_count(), 3);
        assert_eq!(batch_result.failure_count(), 0);
        assert_eq!(batch_result.total_count(), 3);
    }

    #[tokio::test]
    async fn test_import_multiple_photos_partial_failure() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        // Primeira foto: sucesso
        // Segunda foto: falha
        // Terceira foto: sucesso
        let mut call_count = 0;
        mock_repo
            .expect_save()
            .times(3)
            .returning(move |_| {
                call_count += 1;
                if call_count == 2 {
                    Err(DomainError::InvalidFilePath("Disk full".to_string()))
                } else {
                    Ok(())
                }
            });

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));
        
        let use_case = ImportPhotosUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor)
        );
        
        let file_paths = vec![
            FilePath::new("/photos/img1.jpg").unwrap(),
            FilePath::new("/photos/img2.jpg").unwrap(),
            FilePath::new("/photos/img3.jpg").unwrap(),
        ];
        
        // Act
        let result = use_case.execute(file_paths).await;
        
        // Assert
        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.success_count(), 2);
        assert_eq!(batch_result.failure_count(), 1);
        assert_eq!(batch_result.total_count(), 3);
        
        // Verificar que a falha foi registrada
        assert_eq!(batch_result.failed.len(), 1);
        assert_eq!(batch_result.failed[0].0, FilePath::new("/photos/img2.jpg").unwrap());
    }

    #[tokio::test]
    async fn test_import_empty_list() {
        // Arrange
        let mock_repo = MockPhotoRepo::new();
        let mock_extractor = MockMetadataExtractor::new();
        let use_case = ImportPhotosUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor)
        );
        
        // Act
        let result = use_case.execute(vec![]).await;
        
        // Assert
        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.success_count(), 0);
        assert_eq!(batch_result.failure_count(), 0);
        assert_eq!(batch_result.total_count(), 0);
    }

    #[tokio::test]
    async fn test_import_all_fail() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        // Todas as fotos falham
        mock_repo
            .expect_save()
            .times(2)
            .returning(|_| Err(DomainError::InvalidFilePath("Permission denied".to_string())));

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));
        
        let use_case = ImportPhotosUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor)
        );
        
        let file_paths = vec![
            FilePath::new("/photos/img1.jpg").unwrap(),
            FilePath::new("/photos/img2.jpg").unwrap(),
        ];
        
        // Act
        let result = use_case.execute(file_paths).await;
        
        // Assert
        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.success_count(), 0);
        assert_eq!(batch_result.failure_count(), 2);
    }

    #[tokio::test]
    async fn test_import_creates_unique_ids() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        mock_repo
            .expect_save()
            .times(3)
            .returning(|_| Ok(()));

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));
        
        let use_case = ImportPhotosUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor)
        );
        
        let file_paths = vec![
            FilePath::new("/photos/img1.jpg").unwrap(),
            FilePath::new("/photos/img2.jpg").unwrap(),
            FilePath::new("/photos/img3.jpg").unwrap(),
        ];
        
        // Act
        let result = use_case.execute(file_paths).await.unwrap();
        
        // Assert
        let photos = &result.successful;
        assert_eq!(photos.len(), 3);
        
        // Todos os IDs devem ser únicos
        assert_ne!(photos[0].id(), photos[1].id());
        assert_ne!(photos[1].id(), photos[2].id());
        assert_ne!(photos[0].id(), photos[2].id());
    }
}
