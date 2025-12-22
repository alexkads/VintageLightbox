//! Add Photo to Collection Use Case
//!
//! Caso de uso responsável por adicionar uma foto a uma coleção.
//! Implementado com TDD.

use domain::{
    entities::Collection,
    repositories::{CollectionRepository, PhotoRepository},
    value_objects::{CollectionId, PhotoId},
    DomainResult, DomainError,
};
use std::sync::Arc;

/// Use Case para adicionar fotos a coleções
pub struct AddPhotoToCollectionUseCase {
    collection_repository: Arc<dyn CollectionRepository>,
    photo_repository: Arc<dyn PhotoRepository>,
}

impl AddPhotoToCollectionUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(
        collection_repository: Arc<dyn CollectionRepository>,
        photo_repository: Arc<dyn PhotoRepository>,
    ) -> Self {
        Self {
            collection_repository,
            photo_repository,
        }
    }

    /// Adiciona uma foto a uma coleção
    pub async fn execute(&self, collection_id: CollectionId, photo_id: PhotoId) -> DomainResult<Collection> {
        // Verificar se a foto existe
        let photo_exists = self.photo_repository.exists(&photo_id).await?;
        if !photo_exists {
            return Err(DomainError::PhotoNotFound);
        }

        // Buscar coleção
        let collection_option = self.collection_repository.find_by_id(&collection_id).await?;
        let mut collection = collection_option
            .ok_or(DomainError::CollectionNotFound)?;

        // Adicionar foto à coleção
        collection.add_photo(photo_id);

        // Persistir mudança
        self.collection_repository.update(&collection).await?;

        Ok(collection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{entities::Photo, value_objects::FilePath};
    use mockall::mock;
    use mockall::predicate::*;

    // Mock do CollectionRepository
    mock! {
        pub CollectionRepo {}

        #[async_trait::async_trait]
        impl CollectionRepository for CollectionRepo {
            async fn save(&self, collection: &Collection) -> DomainResult<()>;
            async fn find_by_id(&self, id: &CollectionId) -> DomainResult<Option<Collection>>;
            async fn find_all(&self) -> DomainResult<Vec<Collection>>;
            async fn update(&self, collection: &Collection) -> DomainResult<()>;
            async fn delete(&self, id: &CollectionId) -> DomainResult<()>;
            async fn find_by_photo(&self, photo_id: &PhotoId) -> DomainResult<Vec<Collection>>;
        }
    }

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
    async fn test_add_photo_to_collection_success() {
        // Arrange
        let collection = Collection::new("My Collection");
        let collection_id = collection.id().clone();

        let photo = Photo::new(FilePath::new("/photos/test.jpg").unwrap());
        let photo_id = photo.id().clone();

        let mut mock_photo_repo = MockPhotoRepo::new();
        let mut mock_collection_repo = MockCollectionRepo::new();

        // Mock photo exists
        mock_photo_repo
            .expect_exists()
            .with(eq(photo_id.clone()))
            .times(1)
            .returning(|_| Ok(true));

        // Mock find collection
        let collection_clone = collection.clone();
        mock_collection_repo
            .expect_find_by_id()
            .with(eq(collection_id.clone()))
            .times(1)
            .returning(move |_| Ok(Some(collection_clone.clone())));

        // Mock update
        mock_collection_repo
            .expect_update()
            .times(1)
            .returning(|_| Ok(()));

        let use_case = AddPhotoToCollectionUseCase::new(
            Arc::new(mock_collection_repo),
            Arc::new(mock_photo_repo),
        );

        // Act
        let result = use_case.execute(collection_id, photo_id.clone()).await;

        // Assert
        assert!(result.is_ok());
        let updated_collection = result.unwrap();
        assert!(updated_collection.contains_photo(&photo_id));
        assert_eq!(updated_collection.photo_count(), 1);
    }

    #[tokio::test]
    async fn test_add_photo_to_collection_photo_not_found() {
        // Arrange
        let collection_id = CollectionId::new();
        let photo_id = PhotoId::new();

        let mut mock_photo_repo = MockPhotoRepo::new();
        let mock_collection_repo = MockCollectionRepo::new();

        // Mock photo does not exist
        mock_photo_repo
            .expect_exists()
            .with(eq(photo_id.clone()))
            .times(1)
            .returning(|_| Ok(false));

        let use_case = AddPhotoToCollectionUseCase::new(
            Arc::new(mock_collection_repo),
            Arc::new(mock_photo_repo),
        );

        // Act
        let result = use_case.execute(collection_id, photo_id).await;

        // Assert
        assert!(result.is_err());
        match result {
            Err(DomainError::PhotoNotFound) => {},
            _ => panic!("Expected PhotoNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_add_photo_to_collection_collection_not_found() {
        // Arrange
        let collection_id = CollectionId::new();
        let photo_id = PhotoId::new();

        let mut mock_photo_repo = MockPhotoRepo::new();
        let mut mock_collection_repo = MockCollectionRepo::new();

        // Mock photo exists
        mock_photo_repo
            .expect_exists()
            .times(1)
            .returning(|_| Ok(true));

        // Mock collection not found
        mock_collection_repo
            .expect_find_by_id()
            .times(1)
            .returning(|_| Ok(None));

        let use_case = AddPhotoToCollectionUseCase::new(
            Arc::new(mock_collection_repo),
            Arc::new(mock_photo_repo),
        );

        // Act
        let result = use_case.execute(collection_id, photo_id).await;

        // Assert
        assert!(result.is_err());
        match result {
            Err(DomainError::CollectionNotFound) => {},
            _ => panic!("Expected CollectionNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_add_multiple_photos_to_collection() {
        // Arrange
        let collection = Collection::new("My Collection");
        let collection_id = collection.id().clone();

        let photo1_id = PhotoId::new();
        let photo2_id = PhotoId::new();

        let mut mock_photo_repo = MockPhotoRepo::new();
        let mut mock_collection_repo = MockCollectionRepo::new();

        // Mock photo exists (2 calls)
        mock_photo_repo
            .expect_exists()
            .times(2)
            .returning(|_| Ok(true));

        // Mock find collection (2 calls)
        let collection_clone1 = collection.clone();
        let collection_clone2 = collection.clone();
        mock_collection_repo
            .expect_find_by_id()
            .times(2)
            .returning(move |_| {
                static mut CALL_COUNT: usize = 0;
                unsafe {
                    CALL_COUNT += 1;
                    if CALL_COUNT == 1 {
                        Ok(Some(collection_clone1.clone()))
                    } else {
                        let mut col = collection_clone2.clone();
                        col.add_photo(PhotoId::new()); // Simula que já tem 1 foto
                        Ok(Some(col))
                    }
                }
            });

        // Mock update (2 calls)
        mock_collection_repo
            .expect_update()
            .times(2)
            .returning(|_| Ok(()));

        let use_case = AddPhotoToCollectionUseCase::new(
            Arc::new(mock_collection_repo),
            Arc::new(mock_photo_repo),
        );

        // Act
        let result1 = use_case.execute(collection_id.clone(), photo1_id).await;
        let result2 = use_case.execute(collection_id, photo2_id).await;

        // Assert
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap().photo_count(), 2);
    }

    #[tokio::test]
    async fn test_add_duplicate_photo_to_collection() {
        // Arrange
        let mut collection = Collection::new("My Collection");
        let photo_id = PhotoId::new();
        collection.add_photo(photo_id.clone());
        let collection_id = collection.id().clone();

        let mut mock_photo_repo = MockPhotoRepo::new();
        let mut mock_collection_repo = MockCollectionRepo::new();

        // Mock photo exists
        mock_photo_repo
            .expect_exists()
            .times(1)
            .returning(|_| Ok(true));

        // Mock find collection (já contém a foto)
        let collection_clone = collection.clone();
        mock_collection_repo
            .expect_find_by_id()
            .times(1)
            .returning(move |_| Ok(Some(collection_clone.clone())));

        // Mock update
        mock_collection_repo
            .expect_update()
            .times(1)
            .returning(|_| Ok(()));

        let use_case = AddPhotoToCollectionUseCase::new(
            Arc::new(mock_collection_repo),
            Arc::new(mock_photo_repo),
        );

        // Act
        let result = use_case.execute(collection_id, photo_id.clone()).await;

        // Assert - deve ter sucesso, mas não duplicar
        assert!(result.is_ok());
        let updated_collection = result.unwrap();
        assert_eq!(updated_collection.photo_count(), 1); // Ainda 1 foto (não duplica)
    }
}
