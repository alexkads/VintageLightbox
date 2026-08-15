//! Rate Photo Use Case
//!
//! Caso de uso responsável por classificar uma foto com rating (0-5 estrelas).
//! Implementado com TDD.

use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::{PhotoId, Rating},
    DomainResult, DomainError,
};
use std::sync::Arc;

/// Use Case para classificar fotos
pub struct RatePhotoUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
}

impl RatePhotoUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(photo_repository: Arc<dyn PhotoRepository>) -> Self {
        Self { photo_repository }
    }

    /// Classifica uma foto com rating (0-5 estrelas)
    pub async fn execute(&self, photo_id: PhotoId, rating: Rating) -> DomainResult<Photo> {
        // Buscar foto no repositório
        let photo_option = self.photo_repository.find_by_id(&photo_id).await?;
        
        let mut photo = photo_option
            .ok_or(DomainError::PhotoNotFound)?;
        
        // Aplicar rating
        photo.rate(rating)?;
        
        // Persistir mudança
        self.photo_repository.update(&photo).await?;
        
        Ok(photo)
    }

    /// Remove rating de uma foto
    pub async fn unrate(&self, photo_id: PhotoId) -> DomainResult<Photo> {
        // Buscar foto no repositório
        let photo_option = self.photo_repository.find_by_id(&photo_id).await?;
        
        let mut photo = photo_option
            .ok_or(DomainError::PhotoNotFound)?;
        
        // Remover rating
        photo.unrate();
        
        // Persistir mudança
        self.photo_repository.update(&photo).await?;
        
        Ok(photo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{repositories::PhotoRepository, value_objects::FilePath};
    use mockall::mock;
    use mockall::predicate::*;

    // Mock do PhotoRepository
    mock! {
        pub PhotoRepo {}

        #[async_trait::async_trait]
        impl PhotoRepository for PhotoRepo {
            async fn save(&self, photo: &Photo) -> DomainResult<()>;
            async fn find_by_id(&self, id: &PhotoId) -> DomainResult<Option<Photo>>;
            async fn find_all(&self) -> DomainResult<Vec<Photo>>;
            async fn update(&self, photo: &Photo) -> DomainResult<()>;
            async fn delete(&self, id: &PhotoId) -> DomainResult<()>;
            async fn exists(&self, id: &PhotoId) -> DomainResult<bool>;
            async fn find_by_content_hash(&self, hash: &str) -> DomainResult<Option<Photo>>;
        }
    }

    #[tokio::test]
    async fn test_rate_photo_success() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let photo = Photo::new(file_path);
        let photo_id = photo.id();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        // Mock find_by_id
        let photo_clone = photo.clone();
        mock_repo
            .expect_find_by_id()
            .with(eq(photo_id))
            .times(1)
            .returning(move |_| Ok(Some(photo_clone.clone())));
        
        // Mock update
        mock_repo
            .expect_update()
            .times(1)
            .returning(|_| Ok(()));
        
        let use_case = RatePhotoUseCase::new(Arc::new(mock_repo));
        let rating = Rating::new(5).unwrap();
        
        // Act
        let result = use_case.execute(photo_id, rating).await;
        
        // Assert
        assert!(result.is_ok());
        let updated_photo = result.unwrap();
        assert_eq!(updated_photo.rating(), Some(rating));
    }

    #[tokio::test]
    async fn test_rate_photo_not_found() {
        // Arrange
        let photo_id = PhotoId::new();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        // Mock find_by_id retornando None
        mock_repo
            .expect_find_by_id()
            .with(eq(photo_id))
            .times(1)
            .returning(|_| Ok(None));
        
        let use_case = RatePhotoUseCase::new(Arc::new(mock_repo));
        let rating = Rating::new(3).unwrap();
        
        // Act
        let result = use_case.execute(photo_id, rating).await;
        
        // Assert
        assert!(result.is_err());
        match result {
            Err(DomainError::PhotoNotFound) => {},
            _ => panic!("Expected PhotoNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_rate_photo_repository_error() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let photo = Photo::new(file_path);
        let photo_id = photo.id();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        // Mock find_by_id com erro
        mock_repo
            .expect_find_by_id()
            .times(1)
            .returning(|_| Err(DomainError::InvalidFilePath("DB error".to_string())));
        
        let use_case = RatePhotoUseCase::new(Arc::new(mock_repo));
        let rating = Rating::new(4).unwrap();
        
        // Act
        let result = use_case.execute(photo_id, rating).await;
        
        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unrate_photo_success() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let mut photo = Photo::new(file_path);
        photo.rate(Rating::new(5).unwrap()).unwrap();
        let photo_id = photo.id();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        // Mock find_by_id
        let photo_clone = photo.clone();
        mock_repo
            .expect_find_by_id()
            .with(eq(photo_id))
            .times(1)
            .returning(move |_| Ok(Some(photo_clone.clone())));
        
        // Mock update
        mock_repo
            .expect_update()
            .times(1)
            .returning(|_| Ok(()));
        
        let use_case = RatePhotoUseCase::new(Arc::new(mock_repo));
        
        // Act
        let result = use_case.unrate(photo_id).await;
        
        // Assert
        assert!(result.is_ok());
        let updated_photo = result.unwrap();
        assert_eq!(updated_photo.rating(), None);
    }

    #[tokio::test]
    async fn test_rate_multiple_times() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let photo = Photo::new(file_path);
        let photo_id = photo.id();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        // Mock find_by_id (2 chamadas)
        let photo_clone1 = photo.clone();
        let photo_clone2 = photo.clone();
        mock_repo
            .expect_find_by_id()
            .times(2)
            .returning(move |_| {
                static mut CALL_COUNT: usize = 0;
                unsafe {
                    CALL_COUNT += 1;
                    if CALL_COUNT == 1 {
                        Ok(Some(photo_clone1.clone()))
                    } else {
                        let mut p = photo_clone2.clone();
                        p.rate(Rating::new(3).unwrap()).unwrap();
                        Ok(Some(p))
                    }
                }
            });
        
        // Mock update (2 chamadas)
        mock_repo
            .expect_update()
            .times(2)
            .returning(|_| Ok(()));
        
        let use_case = RatePhotoUseCase::new(Arc::new(mock_repo));
        
        // Act - primeira classificação
        let result1 = use_case.execute(photo_id, Rating::new(3).unwrap()).await;
        assert!(result1.is_ok());
        
        // Act - segunda classificação (sobrescreve)
        let result2 = use_case.execute(photo_id, Rating::new(5).unwrap()).await;
        
        // Assert
        assert!(result2.is_ok());
        let final_photo = result2.unwrap();
        assert_eq!(final_photo.rating(), Some(Rating::new(5).unwrap()));
    }
}
