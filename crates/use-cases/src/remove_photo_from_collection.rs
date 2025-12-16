//! Remove Photo from Collection Use Case
//!
//! Caso de uso responsável por remover uma foto de uma coleção.
//! Implementado com TDD.

use domain::{
    entities::Collection,
    repositories::CollectionRepository,
    value_objects::{CollectionId, PhotoId},
    DomainResult, DomainError,
};
use std::sync::Arc;

/// Use Case para remover fotos de coleções
pub struct RemovePhotoFromCollectionUseCase {
    collection_repository: Arc<dyn CollectionRepository>,
}

impl RemovePhotoFromCollectionUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(collection_repository: Arc<dyn CollectionRepository>) -> Self {
        Self {
            collection_repository,
        }
    }

    /// Remove uma foto de uma coleção
    pub async fn execute(&self, collection_id: CollectionId, photo_id: PhotoId) -> DomainResult<Collection> {
        // Buscar coleção
        let collection_option = self.collection_repository.find_by_id(&collection_id).await?;
        let mut collection = collection_option
            .ok_or(DomainError::CollectionNotFound)?;

        // Remover foto da coleção
        let was_removed = collection.remove_photo(&photo_id);
        
        if !was_removed {
            return Err(DomainError::InvalidOperation(
                "Photo not found in collection".to_string()
            ));
        }

        // Persistir mudança
        self.collection_repository.update(&collection).await?;

        Ok(collection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[tokio::test]
    async fn test_remove_photo_from_collection_success() {
        // Arrange
        let mut collection = Collection::new("My Collection");
        let photo_id = PhotoId::new();
        collection.add_photo(photo_id.clone());
        let collection_id = collection.id().clone();

        let mut mock_repo = MockCollectionRepo::new();

        // Mock find collection
        let collection_clone = collection.clone();
        mock_repo
            .expect_find_by_id()
            .with(eq(collection_id.clone()))
            .times(1)
            .returning(move |_| Ok(Some(collection_clone.clone())));

        // Mock update
        mock_repo
            .expect_update()
            .times(1)
            .returning(|_| Ok(()));

        let use_case = RemovePhotoFromCollectionUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute(collection_id, photo_id.clone()).await;

        // Assert
        assert!(result.is_ok());
        let updated_collection = result.unwrap();
        assert!(!updated_collection.contains_photo(&photo_id));
        assert_eq!(updated_collection.photo_count(), 0);
    }

    #[tokio::test]
    async fn test_remove_photo_collection_not_found() {
        // Arrange
        let collection_id = CollectionId::new();
        let photo_id = PhotoId::new();

        let mut mock_repo = MockCollectionRepo::new();

        // Mock collection not found
        mock_repo
            .expect_find_by_id()
            .times(1)
            .returning(|_| Ok(None));

        let use_case = RemovePhotoFromCollectionUseCase::new(Arc::new(mock_repo));

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
    async fn test_remove_nonexistent_photo_from_collection() {
        // Arrange
        let collection = Collection::new("My Collection");
        let collection_id = collection.id().clone();
        let photo_id = PhotoId::new(); // Foto que não está na coleção

        let mut mock_repo = MockCollectionRepo::new();

        // Mock find collection
        let collection_clone = collection.clone();
        mock_repo
            .expect_find_by_id()
            .times(1)
            .returning(move |_| Ok(Some(collection_clone.clone())));

        let use_case = RemovePhotoFromCollectionUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute(collection_id, photo_id).await;

        // Assert
        assert!(result.is_err());
        match result {
            Err(DomainError::InvalidOperation(msg)) => {
                assert_eq!(msg, "Photo not found in collection");
            },
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[tokio::test]
    async fn test_remove_multiple_photos_from_collection() {
        // Arrange
        let mut collection = Collection::new("My Collection");
        let photo1_id = PhotoId::new();
        let photo2_id = PhotoId::new();
        let photo3_id = PhotoId::new();
        
        collection.add_photo(photo1_id.clone());
        collection.add_photo(photo2_id.clone());
        collection.add_photo(photo3_id.clone());
        
        let collection_id = collection.id().clone();

        let mut mock_repo = MockCollectionRepo::new();

        // Mock find collection (2 calls)
        let collection_clone1 = collection.clone();
        let collection_clone2 = collection.clone();
        mock_repo
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
                        col.remove_photo(&PhotoId::new()); // Simula remoção anterior
                        Ok(Some(col))
                    }
                }
            });

        // Mock update (2 calls)
        mock_repo
            .expect_update()
            .times(2)
            .returning(|_| Ok(()));

        let use_case = RemovePhotoFromCollectionUseCase::new(Arc::new(mock_repo));

        // Act
        let result1 = use_case.execute(collection_id.clone(), photo1_id).await;
        let result2 = use_case.execute(collection_id, photo2_id).await;

        // Assert
        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }
}
