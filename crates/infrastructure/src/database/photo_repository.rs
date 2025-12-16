//! SQLite Photo Repository Implementation
//!
//! Implementação concreta do PhotoRepository usando SQLite via SQLx.

use async_trait::async_trait;
use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::{ColorLabel, FilePath, PhotoId, Rating},
    DomainError, DomainResult,
};
use sqlx::{Row, SqlitePool};

/// Implementação SQLite do PhotoRepository
pub struct PhotoRepositoryImpl {
    pool: SqlitePool,
}

impl PhotoRepositoryImpl {
    /// Cria uma nova instância do repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Helper para converter row do banco em Photo
    fn row_to_photo(row: &sqlx::sqlite::SqliteRow) -> DomainResult<Photo> {
        let id_str: String = row.try_get("id").map_err(|e| {
            DomainError::InvalidOperation(format!("Failed to get id: {}", e))
        })?;
        
        let file_path_str: String = row.try_get("file_path").map_err(|e| {
            DomainError::InvalidOperation(format!("Failed to get file_path: {}", e))
        })?;
        
        let rating_val: Option<i64> = row.try_get("rating").ok();
        let color_label_str: Option<String> = row.try_get("color_label").ok();
        let is_edited: bool = row.try_get("is_edited").unwrap_or(false);

        // Parse values
        let photo_id = PhotoId::from_string(&id_str)?;
        let file_path = FilePath::new(&file_path_str)?;
        let rating = rating_val.and_then(|v| Rating::new(v as u8).ok());
        let color_label = color_label_str.and_then(|s| ColorLabel::from_name(&s).ok());

        // Create photo with ID
        let mut photo = Photo::with_id(photo_id, file_path);
        
        // Apply rating if present
        if let Some(r) = rating {
            photo.rate(r)?;
        }
        
        // Apply color label if present
        if let Some(c) = color_label {
            photo.set_color_label(c);
        }
        
        // Mark as edited if needed
        if is_edited {
            photo.mark_as_edited();
        }

        Ok(photo)
    }
}

#[async_trait]
impl PhotoRepository for PhotoRepositoryImpl {
    async fn save(&self, photo: &Photo) -> DomainResult<()> {
        let id = photo.id().to_string();
        let file_path = photo.file_path().to_string_lossy().to_string();
        let rating = photo.rating().map(|r| r.value() as i64);
        let color_label = photo.color_label().map(|c| c.name().to_string());
        let is_edited = photo.is_edited();
        let imported_at = photo.imported_at().to_rfc3339();
        let modified_at = photo.modified_at().to_rfc3339();

        sqlx::query(
            "INSERT INTO photos (id, file_path, rating, color_label, is_edited, imported_at, modified_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&file_path)
        .bind(rating)
        .bind(color_label)
        .bind(is_edited)
        .bind(&imported_at)
        .bind(&modified_at)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Failed to save photo: {}", e)))?;

        Ok(())
    }

    async fn find_by_id(&self, id: &PhotoId) -> DomainResult<Option<Photo>> {
        let id_str = id.to_string();

        let row = sqlx::query("SELECT * FROM photos WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Failed to find photo: {}", e)))?;

        match row {
            Some(r) => Ok(Some(Self::row_to_photo(&r)?)),
            None => Ok(None),
        }
    }

    async fn find_all(&self) -> DomainResult<Vec<Photo>> {
        let rows = sqlx::query("SELECT * FROM photos ORDER BY imported_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Failed to fetch photos: {}", e)))?;

        rows.iter()
            .map(|row| Self::row_to_photo(row))
            .collect()
    }

    async fn update(&self, photo: &Photo) -> DomainResult<()> {
        let id = photo.id().to_string();
        let file_path = photo.file_path().to_string_lossy().to_string();
        let rating = photo.rating().map(|r| r.value() as i64);
        let color_label = photo.color_label().map(|c| c.name().to_string());
        let is_edited = photo.is_edited();
        let modified_at = photo.modified_at().to_rfc3339();

        let result = sqlx::query(
            "UPDATE photos 
             SET file_path = ?, rating = ?, color_label = ?, is_edited = ?, modified_at = ?
             WHERE id = ?"
        )
        .bind(&file_path)
        .bind(rating)
        .bind(color_label)
        .bind(is_edited)
        .bind(&modified_at)
        .bind(&id)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Failed to update photo: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(DomainError::PhotoNotFound);
        }

        Ok(())
    }

    async fn delete(&self, id: &PhotoId) -> DomainResult<()> {
        let id_str = id.to_string();

        let result = sqlx::query("DELETE FROM photos WHERE id = ?")
            .bind(&id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Failed to delete photo: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(DomainError::PhotoNotFound);
        }

        Ok(())
    }

    async fn exists(&self, id: &PhotoId) -> DomainResult<bool> {
        let id_str = id.to_string();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM photos WHERE id = ?")
            .bind(&id_str)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Failed to check existence: {}", e)))?;

        Ok(count > 0)
    }
}
