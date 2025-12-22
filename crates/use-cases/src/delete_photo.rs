use std::sync::Arc;
use domain::repositories::PhotoRepository;
use domain::value_objects::PhotoId;
use domain::errors::DomainResult;

pub struct DeletePhotoUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
}

impl DeletePhotoUseCase {
    pub fn new(photo_repository: Arc<dyn PhotoRepository>) -> Self {
        Self { photo_repository }
    }

    pub async fn execute(&self, photo_id: PhotoId) -> DomainResult<()> {
        self.photo_repository.delete(&photo_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{PhotoRepository, entities::Photo, value_objects::FilePath};
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
    async fn test_delete_photo() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let photo = Photo::new(file_path);
        let photo_id = photo.id().clone();

        let mut mock_repo = MockPhotoRepo::new();

        mock_repo
            .expect_delete()
            .with(eq(photo_id.clone()))
            .times(1)
            .returning(|_| Ok(()));

        let use_case = DeletePhotoUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute(photo_id).await;

        // Assert
        assert!(result.is_ok());
    }
}
