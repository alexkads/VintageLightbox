//! Create Collection Use Case
//!
//! Caso de uso responsável por criar uma nova coleção.
//! Implementado com TDD.

use domain::{entities::Collection, repositories::CollectionRepository, DomainResult};
use std::sync::Arc;

/// Use Case para criar coleções
pub struct CreateCollectionUseCase {
    collection_repository: Arc<dyn CollectionRepository>,
}

impl CreateCollectionUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(collection_repository: Arc<dyn CollectionRepository>) -> Self {
        Self {
            collection_repository,
        }
    }

    /// Cria uma nova coleção
    pub async fn execute(
        &self,
        name: String,
        description: Option<String>,
    ) -> DomainResult<Collection> {
        // Criar nova entidade Collection
        let mut collection = Collection::new(name);

        // Definir descrição se fornecida
        if let Some(desc) = description {
            collection.set_description(desc);
        }

        // Persistir no repositório
        self.collection_repository.save(&collection).await?;

        Ok(collection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{repositories::CollectionRepository, value_objects::PhotoId};
    use mockall::mock;

    // Mock do CollectionRepository
    mock! {
        pub CollectionRepo {}

        #[async_trait::async_trait]
        impl CollectionRepository for CollectionRepo {
            async fn save(&self, collection: &Collection) -> DomainResult<()>;
            async fn find_by_id(&self, id: &domain::value_objects::CollectionId) -> DomainResult<Option<Collection>>;
            async fn find_all(&self) -> DomainResult<Vec<Collection>>;
            async fn update(&self, collection: &Collection) -> DomainResult<()>;
            async fn delete(&self, id: &domain::value_objects::CollectionId) -> DomainResult<()>;
            async fn find_by_photo(&self, photo_id: &PhotoId) -> DomainResult<Vec<Collection>>;
        }
    }

    #[tokio::test]
    async fn test_create_collection_success() {
        // Arrange
        let mut mock_repo = MockCollectionRepo::new();

        mock_repo.expect_save().times(1).returning(|_| Ok(()));

        let use_case = CreateCollectionUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case
            .execute("Vacation 2024".to_string(), Some("Summer trip".to_string()))
            .await;

        // Assert
        assert!(result.is_ok());
        let collection = result.unwrap();
        assert_eq!(collection.name(), "Vacation 2024");
        assert_eq!(collection.description(), Some("Summer trip"));
        assert_eq!(collection.photo_count(), 0);
    }

    #[tokio::test]
    async fn test_create_collection_without_description() {
        // Arrange
        let mut mock_repo = MockCollectionRepo::new();

        mock_repo.expect_save().times(1).returning(|_| Ok(()));

        let use_case = CreateCollectionUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute("Best Photos".to_string(), None).await;

        // Assert
        assert!(result.is_ok());
        let collection = result.unwrap();
        assert_eq!(collection.name(), "Best Photos");
        assert_eq!(collection.description(), None);
    }

    #[tokio::test]
    async fn test_create_collection_repository_error() {
        // Arrange
        let mut mock_repo = MockCollectionRepo::new();

        mock_repo
            .expect_save()
            .times(1)
            .returning(|_| Err(domain::DomainError::InvalidFilePath("DB error".to_string())));

        let use_case = CreateCollectionUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute("Test".to_string(), None).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_multiple_collections_unique_ids() {
        // Arrange
        let mut mock_repo = MockCollectionRepo::new();

        mock_repo.expect_save().times(3).returning(|_| Ok(()));

        let use_case = CreateCollectionUseCase::new(Arc::new(mock_repo));

        // Act
        let col1 = use_case
            .execute("Collection 1".to_string(), None)
            .await
            .unwrap();
        let col2 = use_case
            .execute("Collection 2".to_string(), None)
            .await
            .unwrap();
        let col3 = use_case
            .execute("Collection 3".to_string(), None)
            .await
            .unwrap();

        // Assert - todos os IDs devem ser únicos
        assert_ne!(col1.id(), col2.id());
        assert_ne!(col2.id(), col3.id());
        assert_ne!(col1.id(), col3.id());
    }
}
