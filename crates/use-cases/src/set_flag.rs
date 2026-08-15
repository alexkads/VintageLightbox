//! Set Flag Use Case
//!
//! Caso de uso para definir ou remover uma flag de uma foto (Pick/Reject).
//! Implementado com TDD.

use std::sync::Arc;
use domain::DomainResult;
use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::{Flag, PhotoId},
    DomainError,
};

/// Caso de uso para definir ou remover uma flag de uma foto.
pub struct SetFlagUseCase {
    repo: Arc<dyn PhotoRepository>,
}

impl SetFlagUseCase {
    pub fn new(repo: Arc<dyn PhotoRepository>) -> Self {
        Self { repo }
    }

    /// Define a flag para uma foto
    pub async fn execute(&self, id: PhotoId, flag: Flag) -> DomainResult<Photo> {
        let photo_option = self.repo.find_by_id(&id).await?;
        
        let mut photo = photo_option
            .ok_or(DomainError::PhotoNotFound)?;
            
        photo.set_flag(flag);
        
        self.repo.update(&photo).await?;
        
        Ok(photo)
    }

    /// Remove a flag de uma foto
    pub async fn remove_flag(&self, id: PhotoId) -> DomainResult<Photo> {
        let photo_option = self.repo.find_by_id(&id).await?;
        
        let mut photo = photo_option
            .ok_or(DomainError::PhotoNotFound)?;
            
        photo.remove_flag();
        
        self.repo.update(&photo).await?;
        
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
    async fn test_set_flag_pick() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let photo = Photo::new(file_path);
        let photo_id = photo.id();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        let photo_clone = photo.clone();
        mock_repo
            .expect_find_by_id()
            .with(eq(photo_id))
            .times(1)
            .returning(move |_| Ok(Some(photo_clone.clone())));
            
        mock_repo
            .expect_update()
            .times(1)
            .returning(|_| Ok(()));
            
        let use_case = SetFlagUseCase::new(Arc::new(mock_repo));
        
        // Act
        let result = use_case.execute(photo_id, Flag::Pick).await;
        
        // Assert
        assert!(result.is_ok());
        let photo = result.unwrap();
        assert_eq!(photo.flag(), Some(Flag::Pick));
    }

    #[tokio::test]
    async fn test_set_flag_reject() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let photo = Photo::new(file_path);
        let photo_id = photo.id();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        let photo_clone = photo.clone();
        mock_repo
            .expect_find_by_id()
            .with(eq(photo_id))
            .times(1)
            .returning(move |_| Ok(Some(photo_clone.clone())));
            
        mock_repo
            .expect_update()
            .times(1)
            .returning(|_| Ok(()));
            
        let use_case = SetFlagUseCase::new(Arc::new(mock_repo));
        
        // Act
        let result = use_case.execute(photo_id, Flag::Reject).await;
        
        // Assert
        assert!(result.is_ok());
        let photo = result.unwrap();
        assert_eq!(photo.flag(), Some(Flag::Reject));
    }

    #[tokio::test]
    async fn test_remove_flag() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let mut photo = Photo::new(file_path);
        photo.set_flag(Flag::Pick);
        let photo_id = photo.id();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        let photo_clone = photo.clone();
        mock_repo
            .expect_find_by_id()
            .with(eq(photo_id))
            .times(1)
            .returning(move |_| Ok(Some(photo_clone.clone())));
            
        mock_repo
            .expect_update()
            .times(1)
            .returning(|_| Ok(()));
            
        let use_case = SetFlagUseCase::new(Arc::new(mock_repo));
        
        // Act
        let result = use_case.remove_flag(photo_id).await;
        
        // Assert
        assert!(result.is_ok());
        let photo = result.unwrap();
        assert_eq!(photo.flag(), None);
    }
}
