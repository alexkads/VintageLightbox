//! Collection Entity
//!
//! Entidade representando uma coleção (álbum) de fotos.
//! Implementado usando TDD.

use crate::value_objects::{CollectionId, PhotoId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Entidade Collection - representa uma coleção/álbum de fotos
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    /// Identificador único da coleção
    id: CollectionId,
    
    /// Nome da coleção
    name: String,
    
    /// Descrição opcional da coleção
    description: Option<String>,
    
    /// IDs das fotos nesta coleção
    photo_ids: HashSet<PhotoId>,
    
    /// Data de criação
    created_at: DateTime<Utc>,
    
    /// Data da última modificação
    modified_at: DateTime<Utc>,
}

impl Collection {
    /// Cria uma nova coleção vazia
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        
        Self {
            id: CollectionId::new(),
            name: name.into(),
            description: None,
            photo_ids: HashSet::new(),
            created_at: now,
            modified_at: now,
        }
    }

    /// Cria uma coleção com ID específico (para reconstrução)
    pub fn with_id(
        id: CollectionId,
        name: impl Into<String>,
        description: Option<String>,
        photo_ids: HashSet<PhotoId>,
        created_at: DateTime<Utc>,
        modified_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            description,
            photo_ids,
            created_at,
            modified_at,
        }
    }

    /// Retorna o ID da coleção
    pub fn id(&self) -> &CollectionId {
        &self.id
    }

    /// Retorna o nome da coleção
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Atualiza o nome da coleção
    pub fn rename(&mut self, new_name: impl Into<String>) {
        self.name = new_name.into();
        self.modified_at = Utc::now();
    }

    /// Retorna a descrição da coleção
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Define a descrição da coleção
    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = Some(description.into());
        self.modified_at = Utc::now();
    }

    /// Remove a descrição da coleção
    pub fn clear_description(&mut self) {
        self.description = None;
        self.modified_at = Utc::now();
    }

    /// Retorna os IDs das fotos na coleção
    pub fn photo_ids(&self) -> &HashSet<PhotoId> {
        &self.photo_ids
    }

    /// Adiciona uma foto à coleção
    pub fn add_photo(&mut self, photo_id: PhotoId) -> bool {
        let added = self.photo_ids.insert(photo_id);
        if added {
            self.modified_at = Utc::now();
        }
        added
    }

    /// Remove uma foto da coleção
    pub fn remove_photo(&mut self, photo_id: &PhotoId) -> bool {
        let removed = self.photo_ids.remove(photo_id);
        if removed {
            self.modified_at = Utc::now();
        }
        removed
    }

    /// Verifica se a coleção contém uma foto específica
    pub fn contains_photo(&self, photo_id: &PhotoId) -> bool {
        self.photo_ids.contains(photo_id)
    }

    /// Retorna o número de fotos na coleção
    pub fn photo_count(&self) -> usize {
        self.photo_ids.len()
    }

    /// Verifica se a coleção está vazia
    pub fn is_empty(&self) -> bool {
        self.photo_ids.is_empty()
    }

    /// Limpa todas as fotos da coleção
    pub fn clear_photos(&mut self) {
        if !self.photo_ids.is_empty() {
            self.photo_ids.clear();
            self.modified_at = Utc::now();
        }
    }

    /// Retorna a data de criação
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Retorna a data de última modificação
    pub fn modified_at(&self) -> DateTime<Utc> {
        self.modified_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_creation() {
        let collection = Collection::new("My Photos");
        
        assert_eq!(collection.name(), "My Photos");
        assert_eq!(collection.description(), None);
        assert_eq!(collection.photo_count(), 0);
        assert!(collection.is_empty());
    }

    #[test]
    fn test_collection_rename() {
        let mut collection = Collection::new("Old Name");
        let initial_modified = collection.modified_at();
        
        std::thread::sleep(std::time::Duration::from_millis(10));
        collection.rename("New Name");
        
        assert_eq!(collection.name(), "New Name");
        assert!(collection.modified_at() > initial_modified);
    }

    #[test]
    fn test_collection_set_description() {
        let mut collection = Collection::new("My Collection");
        
        collection.set_description("A beautiful collection");
        
        assert_eq!(collection.description(), Some("A beautiful collection"));
    }

    #[test]
    fn test_collection_clear_description() {
        let mut collection = Collection::new("My Collection");
        collection.set_description("Test description");
        
        collection.clear_description();
        
        assert_eq!(collection.description(), None);
    }

    #[test]
    fn test_collection_add_photo() {
        let mut collection = Collection::new("Test");
        let photo_id = PhotoId::new();
        
        let added = collection.add_photo(photo_id);
        
        assert!(added);
        assert_eq!(collection.photo_count(), 1);
        assert!(collection.contains_photo(&photo_id));
    }

    #[test]
    fn test_collection_add_duplicate_photo() {
        let mut collection = Collection::new("Test");
        let photo_id = PhotoId::new();
        
        collection.add_photo(photo_id);
        let added_again = collection.add_photo(photo_id);
        
        assert!(!added_again);
        assert_eq!(collection.photo_count(), 1);
    }

    #[test]
    fn test_collection_remove_photo() {
        let mut collection = Collection::new("Test");
        let photo_id = PhotoId::new();
        collection.add_photo(photo_id);
        
        let removed = collection.remove_photo(&photo_id);
        
        assert!(removed);
        assert_eq!(collection.photo_count(), 0);
        assert!(!collection.contains_photo(&photo_id));
    }

    #[test]
    fn test_collection_remove_nonexistent_photo() {
        let mut collection = Collection::new("Test");
        let photo_id = PhotoId::new();
        
        let removed = collection.remove_photo(&photo_id);
        
        assert!(!removed);
    }

    #[test]
    fn test_collection_contains_photo() {
        let mut collection = Collection::new("Test");
        let photo_id1 = PhotoId::new();
        let photo_id2 = PhotoId::new();
        
        collection.add_photo(photo_id1);
        
        assert!(collection.contains_photo(&photo_id1));
        assert!(!collection.contains_photo(&photo_id2));
    }

    #[test]
    fn test_collection_photo_count() {
        let mut collection = Collection::new("Test");
        
        assert_eq!(collection.photo_count(), 0);
        
        collection.add_photo(PhotoId::new());
        assert_eq!(collection.photo_count(), 1);
        
        collection.add_photo(PhotoId::new());
        assert_eq!(collection.photo_count(), 2);
    }

    #[test]
    fn test_collection_is_empty() {
        let mut collection = Collection::new("Test");
        
        assert!(collection.is_empty());
        
        collection.add_photo(PhotoId::new());
        assert!(!collection.is_empty());
    }

    #[test]
    fn test_collection_clear_photos() {
        let mut collection = Collection::new("Test");
        collection.add_photo(PhotoId::new());
        collection.add_photo(PhotoId::new());
        
        collection.clear_photos();
        
        assert!(collection.is_empty());
        assert_eq!(collection.photo_count(), 0);
    }

    #[test]
    fn test_collection_modification_updates_date() {
        let mut collection = Collection::new("Test");
        let initial_modified = collection.modified_at();
        
        std::thread::sleep(std::time::Duration::from_millis(10));
        collection.add_photo(PhotoId::new());
        
        assert!(collection.modified_at() > initial_modified);
    }

    #[test]
    fn test_collection_equality() {
        let id = CollectionId::new();
        let created_at = Utc::now();
        let modified_at = Utc::now();
        
        let collection1 = Collection::with_id(
            id,
            "Test",
            None,
            HashSet::new(),
            created_at,
            modified_at,
        );
        
        let collection2 = Collection::with_id(
            id,
            "Test",
            None,
            HashSet::new(),
            created_at,
            modified_at,
        );
        
        assert_eq!(collection1, collection2);
    }

    #[test]
    fn test_collection_clone() {
        let mut collection1 = Collection::new("Test");
        collection1.add_photo(PhotoId::new());
        
        let collection2 = collection1.clone();
        
        assert_eq!(collection1, collection2);
    }

    #[test]
    fn test_collection_created_and_modified_dates() {
        let collection = Collection::new("Test");
        
        assert_eq!(collection.created_at(), collection.modified_at());
    }
}

#[cfg(test)]
mod business_logic_tests {
    use super::*;

    #[test]
    fn test_collection_workflow() {
        // Criar coleção
        let mut collection = Collection::new("Vacation 2024");
        collection.set_description("Photos from summer vacation");
        
        // Adicionar fotos
        let photo1 = PhotoId::new();
        let photo2 = PhotoId::new();
        let photo3 = PhotoId::new();
        
        collection.add_photo(photo1);
        collection.add_photo(photo2);
        collection.add_photo(photo3);
        
        assert_eq!(collection.photo_count(), 3);
        
        // Remover uma foto
        collection.remove_photo(&photo2);
        assert_eq!(collection.photo_count(), 2);
        assert!(!collection.contains_photo(&photo2));
        
        // Renomear
        collection.rename("Summer Vacation 2024");
        assert_eq!(collection.name(), "Summer Vacation 2024");
    }

    #[test]
    fn test_multiple_collections_same_photo() {
        let photo_id = PhotoId::new();
        
        let mut collection1 = Collection::new("Collection 1");
        let mut collection2 = Collection::new("Collection 2");
        
        // Mesma foto pode estar em múltiplas coleções
        collection1.add_photo(photo_id);
        collection2.add_photo(photo_id);
        
        assert!(collection1.contains_photo(&photo_id));
        assert!(collection2.contains_photo(&photo_id));
        
        // Remover de uma não afeta a outra
        collection1.remove_photo(&photo_id);
        assert!(!collection1.contains_photo(&photo_id));
        assert!(collection2.contains_photo(&photo_id));
    }

    #[test]
    fn test_collection_bulk_operations() {
        let mut collection = Collection::new("Bulk Test");
        
        // Adicionar várias fotos
        let photo_ids: Vec<PhotoId> = (0..100).map(|_| PhotoId::new()).collect();
        
        for &photo_id in &photo_ids {
            collection.add_photo(photo_id);
        }
        
        assert_eq!(collection.photo_count(), 100);
        
        // Limpar todas
        collection.clear_photos();
        assert!(collection.is_empty());
    }
}
