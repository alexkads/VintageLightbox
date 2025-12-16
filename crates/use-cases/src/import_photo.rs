//! Import Photo Use Case
//!
//! Caso de uso responsável por importar uma foto para o catálogo.
//! Implementado com TDD e usando mocks para testes.

use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::FilePath,
    DomainResult,
};
use std::sync::Arc;

/// Use Case para importar fotos
pub struct ImportPhotoUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
}

impl ImportPhotoUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(photo_repository: Arc<dyn PhotoRepository>) -> Self {
        Self { photo_repository }
    }

    /// Importa uma foto do sistema de arquivos
    pub async fn execute(&self, file_path: FilePath) -> DomainResult<Photo> {
        // Criar nova entidade Photo
        let photo = Photo::new(file_path);
        
        // Persistir no repositório
        self.photo_repository.save(&photo).await?;
        
        Ok(photo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{DomainError, PhotoRepository};
    use mockall::mock;
    use mockall::predicate::*;

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
    async fn test_import_photo_success() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        // Espera-se que save seja chamado uma vez e retorne Ok
        mock_repo
            .expect_save()
            .times(1)
            .returning(|_| Ok(()));
        
        let use_case = ImportPhotoUseCase::new(Arc::new(mock_repo));
        let file_path = FilePath::new("/path/to/photo.jpg").unwrap();
        
        // Act
        let result = use_case.execute(file_path.clone()).await;
        
        // Assert
        assert!(result.is_ok());
        let photo = result.unwrap();
        assert_eq!(photo.file_path(), &file_path);
    }

    #[tokio::test]
    async fn test_import_photo_repository_error() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        // Simular erro no repositório
        mock_repo
            .expect_save()
            .times(1)
            .returning(|_| Err(DomainError::InvalidFilePath("DB error".to_string())));
        
        let use_case = ImportPhotoUseCase::new(Arc::new(mock_repo));
        let file_path = FilePath::new("/path/to/photo.jpg").unwrap();
        
        // Act
        let result = use_case.execute(file_path).await;
        
        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_import_photo_creates_new_id() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        mock_repo
            .expect_save()
            .times(2)
            .returning(|_| Ok(()));
        
        let use_case = ImportPhotoUseCase::new(Arc::new(mock_repo));
        
        let file_path1 = FilePath::new("/path/to/photo1.jpg").unwrap();
        let file_path2 = FilePath::new("/path/to/photo2.jpg").unwrap();
        
        // Act
        let photo1 = use_case.execute(file_path1).await.unwrap();
        let photo2 = use_case.execute(file_path2).await.unwrap();
        
        // Assert
        // Cada foto deve ter um ID único
        assert_ne!(photo1.id(), photo2.id());
    }

    #[tokio::test]
    async fn test_import_photo_with_different_extensions() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        mock_repo
            .expect_save()
            .times(3)
            .returning(|_| Ok(()));
        
        let use_case = ImportPhotoUseCase::new(Arc::new(mock_repo));
        
        // Act & Assert - diferentes extensões devem funcionar
        let extensions = vec!["jpg", "png", "raw"];
        
        for ext in extensions {
            let path = format!("/photos/image.{}", ext);
            let file_path = FilePath::new(&path).unwrap();
            let result = use_case.execute(file_path).await;
            
            assert!(result.is_ok());
        }
    }
}
