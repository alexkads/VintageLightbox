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
        use chrono::{DateTime, Utc};
        use domain::value_objects::PhotoMetadata;

        let id_str: String = row.try_get("id").map_err(|e| {
            DomainError::InvalidOperation(format!("Failed to get id: {}", e))
        })?;
        
        let file_path_str: String = row.try_get("file_path").map_err(|e| {
            DomainError::InvalidOperation(format!("Failed to get file_path: {}", e))
        })?;
        
        let imported_at_str: String = row.try_get("imported_at").map_err(|e| {
            DomainError::InvalidOperation(format!("Failed to get imported_at: {}", e))
        })?;

        let modified_at_str: String = row.try_get("modified_at").map_err(|e| {
            DomainError::InvalidOperation(format!("Failed to get modified_at: {}", e))
        })?;

        let rating_val: Option<i64> = row.try_get("rating").ok();
        let color_label_str: Option<String> = row.try_get("color_label").ok();
        let is_edited: bool = row.try_get("is_edited").unwrap_or(false);
        let thumbnail_path_str: Option<String> = row.try_get("thumbnail_path").ok();
        let preview_path_str: Option<String> = row.try_get("preview_path").ok();
        let edit_exposure: Option<f32> = row.try_get::<Option<f32>, _>("edit_exposure").unwrap_or(None);
        let edit_contrast: Option<f32> = row.try_get::<Option<f32>, _>("edit_contrast").unwrap_or(None);
        
        let edit_temperature: Option<f32> = row.try_get::<Option<f32>, _>("edit_temperature").unwrap_or(None);
        let edit_tint: Option<f32> = row.try_get::<Option<f32>, _>("edit_tint").unwrap_or(None);
        let edit_highlights: Option<f32> = row.try_get::<Option<f32>, _>("edit_highlights").unwrap_or(None);
        let edit_shadows: Option<f32> = row.try_get::<Option<f32>, _>("edit_shadows").unwrap_or(None);
        let edit_whites: Option<f32> = row.try_get::<Option<f32>, _>("edit_whites").unwrap_or(None);
        let edit_blacks: Option<f32> = row.try_get::<Option<f32>, _>("edit_blacks").unwrap_or(None);
        let edit_clarity: Option<f32> = row.try_get::<Option<f32>, _>("edit_clarity").unwrap_or(None);
        let edit_vibrance: Option<f32> = row.try_get::<Option<f32>, _>("edit_vibrance").unwrap_or(None);
        let edit_saturation: Option<f32> = row.try_get::<Option<f32>, _>("edit_saturation").unwrap_or(None);
        let edit_tone_curve_shadows: Option<f32> = row.try_get::<Option<f32>, _>("edit_tone_curve_shadows").unwrap_or(None);
        let edit_tone_curve_darks: Option<f32> = row.try_get::<Option<f32>, _>("edit_tone_curve_darks").unwrap_or(None);
        let edit_tone_curve_lights: Option<f32> = row.try_get::<Option<f32>, _>("edit_tone_curve_lights").unwrap_or(None);
        let edit_tone_curve_highlights: Option<f32> = row.try_get::<Option<f32>, _>("edit_tone_curve_highlights").unwrap_or(None);
        let content_hash: Option<String> = row.try_get("content_hash").ok();



        // Metadata persistido como JSON string
        let metadata_str: Option<String> = row.try_get("metadata").ok();
        let metadata: Option<PhotoMetadata> = match metadata_str {
            Some(s) => serde_json::from_str(&s).ok(), // Se falhar parse, ignora
            None => None,
        };

        // Parse values
        let photo_id = PhotoId::from_string(&id_str)?;
        let file_path = FilePath::new(&file_path_str)?;
        let rating = rating_val.and_then(|v| Rating::new(v as u8).ok());
        let color_label = color_label_str.and_then(|s| ColorLabel::from_name(&s).ok());
        let thumbnail_path = thumbnail_path_str.and_then(|s| FilePath::new(&s).ok());
        let preview_path = preview_path_str.and_then(|s| FilePath::new(&s).ok());

        let imported_at = DateTime::parse_from_rfc3339(&imported_at_str)
            .map_err(|e| DomainError::InvalidOperation(format!("Invalid imported_at date: {}", e)))?
            .with_timezone(&Utc);

        let modified_at = DateTime::parse_from_rfc3339(&modified_at_str)
            .map_err(|e| DomainError::InvalidOperation(format!("Invalid modified_at date: {}", e)))?
            .with_timezone(&Utc);

        // Reconstruct photo with all fields
        Ok(Photo::reconstruct(
            photo_id,
            file_path,
            imported_at,
            modified_at,
            metadata,
            rating,
            color_label,
            is_edited,
            thumbnail_path,
            preview_path,
            edit_exposure,
            edit_contrast,
            edit_temperature,
            edit_tint,
            edit_highlights,
            edit_shadows,
            edit_whites,
            edit_blacks,
            edit_clarity,
            edit_vibrance,
            edit_saturation,
            edit_tone_curve_shadows,
            edit_tone_curve_darks,
            edit_tone_curve_lights,
            edit_tone_curve_highlights,
            content_hash,
        ))
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
        let thumbnail_path = photo.thumbnail_path().map(|p| p.to_string_lossy().to_string());
        let preview_path = photo.preview_path().map(|p| p.to_string_lossy().to_string());
        let edit_exposure = photo.edit_exposure();
        let edit_contrast = photo.edit_contrast();
        let edit_temperature = photo.edit_temperature();
        let edit_tint = photo.edit_tint();
        let edit_highlights = photo.edit_highlights();
        let edit_shadows = photo.edit_shadows();
        let edit_whites = photo.edit_whites();
        let edit_blacks = photo.edit_blacks();
        let edit_clarity = photo.edit_clarity();
        let edit_vibrance = photo.edit_vibrance();
        let edit_saturation = photo.edit_saturation();
        let edit_tone_curve_shadows = photo.edit_tone_curve_shadows();
        let edit_tone_curve_darks = photo.edit_tone_curve_darks();
        let edit_tone_curve_lights = photo.edit_tone_curve_lights();
        let edit_tone_curve_highlights = photo.edit_tone_curve_highlights();
        let content_hash = photo.content_hash().map(|s| s.to_string());

        // Serializar metadata para JSON
        let metadata = photo.metadata()
            .and_then(|m| serde_json::to_string(m).ok());

        sqlx::query(
            "INSERT INTO photos (id, file_path, rating, color_label, is_edited, imported_at, modified_at, metadata, thumbnail_path, preview_path, edit_exposure, edit_contrast, edit_temperature, edit_tint, edit_highlights, edit_shadows, edit_whites, edit_blacks, edit_clarity, edit_vibrance, edit_saturation, edit_tone_curve_shadows, edit_tone_curve_darks, edit_tone_curve_lights, edit_tone_curve_highlights, content_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&file_path)
        .bind(rating)
        .bind(color_label)
        .bind(is_edited)
        .bind(&imported_at)
        .bind(&modified_at)
        .bind(metadata)
        .bind(thumbnail_path)
        .bind(preview_path)
        .bind(edit_exposure)
        .bind(edit_contrast)
        .bind(edit_temperature)
        .bind(edit_tint)
        .bind(edit_highlights)
        .bind(edit_shadows)
        .bind(edit_whites)
        .bind(edit_blacks)
        .bind(edit_clarity)
        .bind(edit_vibrance)
        .bind(edit_saturation)
        .bind(edit_tone_curve_shadows)
        .bind(edit_tone_curve_darks)
        .bind(edit_tone_curve_lights)
        .bind(edit_tone_curve_highlights)
        .bind(content_hash)
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
        let thumbnail_path = photo.thumbnail_path().map(|p| p.to_string_lossy().to_string());
        let preview_path = photo.preview_path().map(|p| p.to_string_lossy().to_string());
        let edit_exposure = photo.edit_exposure();
        let edit_contrast = photo.edit_contrast();
        let edit_temperature = photo.edit_temperature();
        let edit_tint = photo.edit_tint();
        let edit_highlights = photo.edit_highlights();
        let edit_shadows = photo.edit_shadows();
        let edit_whites = photo.edit_whites();
        let edit_blacks = photo.edit_blacks();
        let edit_clarity = photo.edit_clarity();
        let edit_vibrance = photo.edit_vibrance();
        let edit_saturation = photo.edit_saturation();
        let edit_tone_curve_shadows = photo.edit_tone_curve_shadows();
        let edit_tone_curve_darks = photo.edit_tone_curve_darks();
        let edit_tone_curve_lights = photo.edit_tone_curve_lights();
        let edit_tone_curve_highlights = photo.edit_tone_curve_highlights();
        let content_hash = photo.content_hash().map(|s| s.to_string());

        // Serializar metadata para JSON
        let metadata = photo.metadata()
            .and_then(|m| serde_json::to_string(m).ok());

        let result = sqlx::query(
            "UPDATE photos
             SET file_path = ?, rating = ?, color_label = ?, is_edited = ?, modified_at = ?, metadata = ?, thumbnail_path = ?, preview_path = ?, edit_exposure = ?, edit_contrast = ?, edit_temperature = ?, edit_tint = ?, edit_highlights = ?, edit_shadows = ?, edit_whites = ?, edit_blacks = ?, edit_clarity = ?, edit_vibrance = ?, edit_saturation = ?, edit_tone_curve_shadows = ?, edit_tone_curve_darks = ?, edit_tone_curve_lights = ?, edit_tone_curve_highlights = ?, content_hash = ?
             WHERE id = ?"
        )
        .bind(&file_path)
        .bind(rating)
        .bind(color_label)
        .bind(is_edited)
        .bind(&modified_at)
        .bind(metadata)
        .bind(thumbnail_path)
        .bind(preview_path)
        .bind(edit_exposure)
        .bind(edit_contrast)
        .bind(edit_temperature)
        .bind(edit_tint)
        .bind(edit_highlights)
        .bind(edit_shadows)
        .bind(edit_whites)
        .bind(edit_blacks)
        .bind(edit_clarity)
        .bind(edit_vibrance)
        .bind(edit_saturation)
        .bind(edit_tone_curve_shadows)
        .bind(edit_tone_curve_darks)
        .bind(edit_tone_curve_lights)
        .bind(edit_tone_curve_highlights)
        .bind(content_hash)
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

    async fn find_by_content_hash(&self, hash: &str) -> DomainResult<Option<Photo>> {
        let row = sqlx::query("SELECT * FROM photos WHERE content_hash = ? LIMIT 1")
            .bind(hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Failed to find photo by hash: {}", e)))?;

        match row {
            Some(r) => Ok(Some(Self::row_to_photo(&r)?)),
            None => Ok(None),
        }
    }
}
