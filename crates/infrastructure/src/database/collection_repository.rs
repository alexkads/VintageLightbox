//! SQLite Collection Repository Implementation
//!
//! Implementação concreta do CollectionRepository usando SQLite via SQLx.

use async_trait::async_trait;
use domain::{
    entities::Collection,
    repositories::CollectionRepository,
    value_objects::{CollectionId, PhotoId},
    DomainError, DomainResult,
};
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;

/// Implementação SQLite do CollectionRepository
pub struct CollectionRepositoryImpl {
    pool: SqlitePool,
}

impl CollectionRepositoryImpl {
    /// Cria uma nova instância do repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Helper para converter row do banco em Collection
    async fn row_to_collection(&self, row: &sqlx::sqlite::SqliteRow) -> DomainResult<Collection> {
        let id_str: String = row
            .try_get("id")
            .map_err(|e| DomainError::InvalidOperation(format!("Failed to get id: {}", e)))?;

        let name: String = row
            .try_get("name")
            .map_err(|e| DomainError::InvalidOperation(format!("Failed to get name: {}", e)))?;

        let description: Option<String> = row
            .try_get("description")
            .ok()
            .filter(|s: &String| !s.is_empty());

        // Parse values
        let collection_id = CollectionId::from_string(&id_str)?;

        // Buscar photo_ids da tabela de junção
        let photo_ids = self.fetch_photo_ids(&collection_id).await?;

        // Criar collection com ID
        let collection = Collection::new(name);

        // Reconstruir com ID correto e descrição
        let collection = Collection::with_id(
            collection_id,
            collection.name().to_string(),
            description,
            photo_ids,
            collection.created_at(),
            collection.modified_at(),
        );

        Ok(collection)
    }

    /// Busca os IDs de fotos associados a uma coleção
    async fn fetch_photo_ids(
        &self,
        collection_id: &CollectionId,
    ) -> DomainResult<HashSet<PhotoId>> {
        let collection_id_str = collection_id.to_string();

        let rows = sqlx::query("SELECT photo_id FROM collection_photos WHERE collection_id = ?")
            .bind(&collection_id_str)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                DomainError::InvalidOperation(format!("Failed to fetch photo_ids: {}", e))
            })?;

        let mut photo_ids = HashSet::new();
        for row in rows {
            let photo_id_str: String = row.try_get("photo_id").map_err(|e| {
                DomainError::InvalidOperation(format!("Failed to get photo_id: {}", e))
            })?;
            let photo_id = PhotoId::from_string(&photo_id_str)?;
            photo_ids.insert(photo_id);
        }

        Ok(photo_ids)
    }

    /// Salva os photo_ids na tabela de junção
    async fn save_photo_ids(
        &self,
        collection_id: &CollectionId,
        photo_ids: &HashSet<PhotoId>,
    ) -> DomainResult<()> {
        let collection_id_str = collection_id.to_string();

        // Primeiro, remover todas as associações existentes
        sqlx::query("DELETE FROM collection_photos WHERE collection_id = ?")
            .bind(&collection_id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                DomainError::InvalidOperation(format!("Failed to delete photo associations: {}", e))
            })?;

        // Inserir novas associações
        for photo_id in photo_ids {
            let photo_id_str = photo_id.to_string();
            sqlx::query("INSERT INTO collection_photos (collection_id, photo_id) VALUES (?, ?)")
                .bind(&collection_id_str)
                .bind(&photo_id_str)
                .execute(&self.pool)
                .await
                .map_err(|e| {
                    DomainError::InvalidOperation(format!(
                        "Failed to insert photo association: {}",
                        e
                    ))
                })?;
        }

        Ok(())
    }
}

#[async_trait]
impl CollectionRepository for CollectionRepositoryImpl {
    async fn save(&self, collection: &Collection) -> DomainResult<()> {
        let id = collection.id().to_string();
        let name = collection.name().to_string();
        let description = collection.description().map(|d| d.to_string());
        let created_at = collection.created_at().to_rfc3339();
        let modified_at = collection.modified_at().to_rfc3339();

        sqlx::query(
            "INSERT INTO collections (id, name, description, created_at, modified_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&name)
        .bind(&description)
        .bind(&created_at)
        .bind(&modified_at)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Failed to save collection: {}", e)))?;

        // Salvar photo_ids
        self.save_photo_ids(collection.id(), collection.photo_ids())
            .await?;

        Ok(())
    }

    async fn find_by_id(&self, id: &CollectionId) -> DomainResult<Option<Collection>> {
        let id_str = id.to_string();

        let row = sqlx::query("SELECT * FROM collections WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                DomainError::InvalidOperation(format!("Failed to find collection: {}", e))
            })?;

        match row {
            Some(r) => Ok(Some(self.row_to_collection(&r).await?)),
            None => Ok(None),
        }
    }

    async fn find_all(&self) -> DomainResult<Vec<Collection>> {
        let rows = sqlx::query("SELECT * FROM collections ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                DomainError::InvalidOperation(format!("Failed to fetch collections: {}", e))
            })?;

        let mut collections = Vec::new();
        for row in rows {
            collections.push(self.row_to_collection(&row).await?);
        }

        Ok(collections)
    }

    async fn update(&self, collection: &Collection) -> DomainResult<()> {
        let id = collection.id().to_string();
        let name = collection.name().to_string();
        let description = collection.description().map(|d| d.to_string());
        let modified_at = collection.modified_at().to_rfc3339();

        let result = sqlx::query(
            "UPDATE collections
             SET name = ?, description = ?, modified_at = ?
             WHERE id = ?",
        )
        .bind(&name)
        .bind(&description)
        .bind(&modified_at)
        .bind(&id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            DomainError::InvalidOperation(format!("Failed to update collection: {}", e))
        })?;

        if result.rows_affected() == 0 {
            return Err(DomainError::CollectionNotFound);
        }

        // Atualizar photo_ids
        self.save_photo_ids(collection.id(), collection.photo_ids())
            .await?;

        Ok(())
    }

    async fn delete(&self, id: &CollectionId) -> DomainResult<()> {
        let id_str = id.to_string();

        let result = sqlx::query("DELETE FROM collections WHERE id = ?")
            .bind(&id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                DomainError::InvalidOperation(format!("Failed to delete collection: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(DomainError::CollectionNotFound);
        }

        // As associações em collection_photos são deletadas automaticamente por CASCADE

        Ok(())
    }

    async fn find_by_photo(&self, photo_id: &PhotoId) -> DomainResult<Vec<Collection>> {
        let photo_id_str = photo_id.to_string();

        let rows = sqlx::query(
            "SELECT c.* FROM collections c
             INNER JOIN collection_photos cp ON c.id = cp.collection_id
             WHERE cp.photo_id = ?
             ORDER BY c.created_at DESC",
        )
        .bind(&photo_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            DomainError::InvalidOperation(format!("Failed to find collections by photo: {}", e))
        })?;

        let mut collections = Vec::new();
        for row in rows {
            collections.push(self.row_to_collection(&row).await?);
        }

        Ok(collections)
    }
}
