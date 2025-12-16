//! Integration tests for CollectionRepository
//!
//! Testes de integração usando banco SQLite em memória.

use domain::{
    entities::Collection,
    repositories::CollectionRepository,
    value_objects::PhotoId,
};
use infrastructure::{create_pool, run_migrations, CollectionRepositoryImpl};

/// Helper para criar um repository de teste com banco em memória
async fn create_test_repository() -> CollectionRepositoryImpl {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("Failed to create pool");
    
    run_migrations(&pool)
        .await
        .expect("Failed to run migrations");
    
    CollectionRepositoryImpl::new(pool)
}

#[tokio::test]
async fn test_save_and_find_collection() {
    // Arrange
    let repo = create_test_repository().await;
    let collection = Collection::new("My Collection");
    let collection_id = collection.id().clone();

    // Act - Save
    let save_result = repo.save(&collection).await;
    assert!(save_result.is_ok());

    // Act - Find
    let found = repo.find_by_id(&collection_id).await.unwrap();

    // Assert
    assert!(found.is_some());
    let found_collection = found.unwrap();
    assert_eq!(found_collection.id(), &collection_id);
    assert_eq!(found_collection.name(), "My Collection");
    assert_eq!(found_collection.description(), None);
}

#[tokio::test]
async fn test_save_collection_with_description() {
    // Arrange
    let repo = create_test_repository().await;
    let mut collection = Collection::new("My Collection");
    collection.set_description("A test collection");
    let collection_id = collection.id().clone();

    // Act
    repo.save(&collection).await.unwrap();
    let found = repo.find_by_id(&collection_id).await.unwrap().unwrap();

    // Assert
    assert_eq!(found.description(), Some("A test collection"));
}

#[tokio::test]
async fn test_add_photos_to_collection() {
    // Arrange
    let repo = create_test_repository().await;
    let mut collection = Collection::new("My Collection");
    let photo1_id = PhotoId::new();
    let photo2_id = PhotoId::new();
    
    collection.add_photo(photo1_id.clone());
    collection.add_photo(photo2_id.clone());
    let collection_id = collection.id().clone();

    // Act
    repo.save(&collection).await.unwrap();
    let found = repo.find_by_id(&collection_id).await.unwrap().unwrap();

    // Assert
    assert_eq!(found.photo_count(), 2);
    assert!(found.contains_photo(&photo1_id));
    assert!(found.contains_photo(&photo2_id));
}

#[tokio::test]
async fn test_update_collection() {
    // Arrange
    let repo = create_test_repository().await;
    let mut collection = Collection::new("Original Name");
    let collection_id = collection.id().clone();

    repo.save(&collection).await.unwrap();

    // Act - Update
    collection.rename("New Name");
    collection.set_description("Updated description");
    repo.update(&collection).await.unwrap();

    // Assert
    let found = repo.find_by_id(&collection_id).await.unwrap().unwrap();
    assert_eq!(found.name(), "New Name");
    assert_eq!(found.description(), Some("Updated description"));
}

#[tokio::test]
async fn test_delete_collection() {
    // Arrange
    let repo = create_test_repository().await;
    let collection = Collection::new("To Delete");
    let collection_id = collection.id().clone();

    repo.save(&collection).await.unwrap();

    // Act
    let delete_result = repo.delete(&collection_id).await;

    // Assert
    assert!(delete_result.is_ok());
    let found = repo.find_by_id(&collection_id).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_find_all_collections() {
    // Arrange
    let repo = create_test_repository().await;
    
    let collection1 = Collection::new("Collection 1");
    let collection2 = Collection::new("Collection 2");
    let collection3 = Collection::new("Collection 3");

    repo.save(&collection1).await.unwrap();
    repo.save(&collection2).await.unwrap();
    repo.save(&collection3).await.unwrap();

    // Act
    let all_collections = repo.find_all().await.unwrap();

    // Assert
    assert_eq!(all_collections.len(), 3);
}

#[tokio::test]
async fn test_find_collections_by_photo() {
    // Arrange
    let repo = create_test_repository().await;
    let photo_id = PhotoId::new();
    
    let mut collection1 = Collection::new("Collection 1");
    collection1.add_photo(photo_id.clone());
    
    let mut collection2 = Collection::new("Collection 2");
    collection2.add_photo(photo_id.clone());
    
    let collection3 = Collection::new("Collection 3");

    repo.save(&collection1).await.unwrap();
    repo.save(&collection2).await.unwrap();
    repo.save(&collection3).await.unwrap();

    // Act
    let collections = repo.find_by_photo(&photo_id).await.unwrap();

    // Assert
    assert_eq!(collections.len(), 2);
    assert!(collections.iter().any(|c| c.name() == "Collection 1"));
    assert!(collections.iter().any(|c| c.name() == "Collection 2"));
}

#[tokio::test]
async fn test_remove_photo_from_collection() {
    // Arrange
    let repo = create_test_repository().await;
    let mut collection = Collection::new("My Collection");
    let photo1_id = PhotoId::new();
    let photo2_id = PhotoId::new();
    
    collection.add_photo(photo1_id.clone());
    collection.add_photo(photo2_id.clone());
    let collection_id = collection.id().clone();

    repo.save(&collection).await.unwrap();

    // Act - Remove one photo
    collection.remove_photo(&photo1_id);
    repo.update(&collection).await.unwrap();

    // Assert
    let found = repo.find_by_id(&collection_id).await.unwrap().unwrap();
    assert_eq!(found.photo_count(), 1);
    assert!(!found.contains_photo(&photo1_id));
    assert!(found.contains_photo(&photo2_id));
}

#[tokio::test]
async fn test_update_nonexistent_collection() {
    // Arrange
    let repo = create_test_repository().await;
    let collection = Collection::new("Nonexistent");

    // Act
    let result = repo.update(&collection).await;

    // Assert
    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_nonexistent_collection() {
    // Arrange
    let repo = create_test_repository().await;
    let collection_id = domain::value_objects::CollectionId::new();

    // Act
    let result = repo.delete(&collection_id).await;

    // Assert
    assert!(result.is_err());
}
