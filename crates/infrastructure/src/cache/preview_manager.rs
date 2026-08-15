use std::path::PathBuf;
use std::sync::Mutex;
use std::fs;
// use directories::ProjectDirs;
use image::DynamicImage;
use domain::services::{PreviewStorage, PreviewType};
use domain::value_objects::PhotoId;
use domain::DomainResult;
use rusqlite::{params, Connection, OptionalExtension};

/// Statistics about the preview cache for UI display
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of thumbnail entries (type 0)
    pub thumbnail_count: u64,
    /// Number of large preview entries (type 1)
    pub large_preview_count: u64,
    /// Total size of all cache data in bytes
    pub total_size_bytes: u64,
    /// Path to the cache database
    pub db_path: PathBuf,
}

pub struct PreviewManager {
    conn: Mutex<Connection>,
    #[allow(dead_code)]
    cache_dir: PathBuf,
}

/// Grava a imagem como JPEG, convertendo para RGB8 antes.
///
/// # Por que a conversão não é opcional
///
/// **JPEG não tem canal alfa.** Até o `image` 0.24 os dois métodos aqui
/// passavam `image.color()` direto para o encoder, e uma foto RGBA era aceita
/// sem reclamação — gravando bytes de 4 canais num formato de 3. O 0.25 recusa
/// explicitamente:
///
/// ```text
/// The encoder or decoder for Jpeg does not support the color type `Rgba8`
/// ```
///
/// O `image_exporter.rs`, no mesmo crate, sempre fez `to_rgb8()` antes de
/// encodar — eram dois caminhos para a mesma decisão, e só um estava certo.
/// Isto aqui é o caminho certo, agora num lugar só.
///
/// ⚠️ **Descartar o alfa é a única saída, e é o que já acontecia.** Miniatura e
/// preview são para exibição, não para reedição — o pixel editável vem do RAW.
/// Preservar transparência exigiria trocar o formato do cache, que é outra
/// decisão e outro custo.
fn encode_jpeg<W: std::io::Write>(
    encoder: &mut image::codecs::jpeg::JpegEncoder<W>,
    image: &DynamicImage,
) -> Result<(), String> {
    let rgb = image.to_rgb8();
    encoder
        .encode(
            &rgb,
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| e.to_string())
}


impl PreviewManager {
    pub fn new() -> Self {
        let cache_dir = crate::paths::AppPaths::preview_cache_dir();
        Self::new_with_path(cache_dir)
    }

    pub fn new_with_path(cache_dir: PathBuf) -> Self {
        if !cache_dir.exists() {
            let _ = fs::create_dir_all(&cache_dir);
        }

        let db_path = cache_dir.join("preview_cache.db");
        // Connect to cache database
        let conn = Connection::open(&db_path).expect("Failed to open preview database");

        // Optimize SQLite Performance
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "synchronous", "NORMAL").ok();
        conn.pragma_update(None, "cache_size", "-64000").ok(); // 64MB cache

        conn.execute(
            "CREATE TABLE IF NOT EXISTS previews (
                photo_id TEXT NOT NULL,
                type INTEGER NOT NULL,
                data BLOB NOT NULL,
                created_at INTEGER NOT NULL,
                last_accessed_at INTEGER NOT NULL,
                PRIMARY KEY (photo_id, type)
            )",
            [],
        ).expect("Failed to initialize preview database");

        Self {
            conn: Mutex::new(conn),
            cache_dir,
        }
    }

    /// Helper to get a thumbnail as DynamicImage
    pub fn get_thumbnail(&self, photo_id_str: &str) -> Option<DynamicImage> {
        let conn = self.conn.lock().unwrap();
        // Type 0 = Thumbnail
        let mut stmt = conn.prepare("SELECT data FROM previews WHERE photo_id = ?1 AND type = 0").ok()?;
        let data: Option<Vec<u8>> = stmt.query_row(params![photo_id_str], |row| row.get(0)).optional().ok()?;
        
        if let Some(bytes) = data {
             // Update access time
             let now = chrono::Utc::now().timestamp();
             let _ = conn.execute(
                 "UPDATE previews SET last_accessed_at = ?1 WHERE photo_id = ?2 AND type = 0", 
                 params![now, photo_id_str]
             );
             
             image::load_from_memory(&bytes).ok()
        } else {
            None
        }
    }

    /// Helper to save thumbnail
    pub fn save_thumbnail(&self, photo_id_str: &str, image: &DynamicImage) -> Result<(), String> {
        let mut bytes: Vec<u8> = Vec::new();
        // Medium quality JPEG for thumbnails
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 80);
        encode_jpeg(&mut encoder, image)?;

        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        // Type 0 = Thumbnail
        conn.execute(
            "INSERT OR REPLACE INTO previews (photo_id, type, data, created_at, last_accessed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![photo_id_str, 0, bytes, now, now],
        ).map_err(|e| e.to_string())?;
        
        Ok(())
    }

    /// Helper to get a preview as DynamicImage (Legacy API support)
    /// Loads the 'Large' preview type (Smart Preview)
    pub fn get_preview(&self, photo_id_str: &str) -> Option<DynamicImage> {
        let conn = self.conn.lock().unwrap();
        // Type 1 = Large
        let mut stmt = conn.prepare("SELECT data FROM previews WHERE photo_id = ?1 AND type = 1").ok()?;
        let data: Option<Vec<u8>> = stmt.query_row(params![photo_id_str], |row| row.get(0)).optional().ok()?;
        
        if let Some(bytes) = data {
             // Update access time
             let now = chrono::Utc::now().timestamp();
             let _ = conn.execute(
                 "UPDATE previews SET last_accessed_at = ?1 WHERE photo_id = ?2 AND type = 1", 
                 params![now, photo_id_str]
             );
             
             image::load_from_memory(&bytes).ok()
        } else {
            None
        }
    }

    /// Helper to save preview (Legacy API support)
    /// Saves as 'Large' preview type
    pub fn save_preview(&self, photo_id_str: &str, image: &DynamicImage) -> Result<(), String> {
        let mut bytes: Vec<u8> = Vec::new();
        // High quality JPEG for previews
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 90);
        encode_jpeg(&mut encoder, image)?;

        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        // Type 1 = Large
        conn.execute(
            "INSERT OR REPLACE INTO previews (photo_id, type, data, created_at, last_accessed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![photo_id_str, 1, bytes, now, now],
        ).map_err(|e| e.to_string())?;
        
        Ok(())
    }
    
    /// Cleanup old previews if cache exceeds size limit
    pub fn cleanup_lru(&self, max_size_bytes: u64) -> Result<u64, String> {
        let conn = self.conn.lock().unwrap();
        
        // Check current size
        // This is a rough estimate summing Blob sizes
        let size: i64 = conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(data)), 0) FROM previews",
            [],
            |row| row.get(0)
        ).unwrap_or(0);

        if size as u64 <= max_size_bytes {
            return Ok(0);
        }

        // Delete oldest accessed
        // We delete in chunks until size is under limit, or just simplistic approach:
        // Delete oldest 10%? 
        // Or delete strictly strictly oldest until satisfied.
        
        let target_size = max_size_bytes as i64;
        let diff = size - target_size;
        
        if diff <= 0 { return Ok(0); }

        // Find items to delete
        // We want to delete rows with oldest last_accessed_at until we free 'diff' bytes.
        // This logic is complex in SQL alone without iteration.
        // Simplification: Delete oldest N items.
        
        // Let's just delete the oldest 50 items and repeat or just one pass.
        // Better: Delete where last_accessed_at < some_threshold? 
        
        // For now, let's just delete the 20 oldest items if we are over limit, as a maintenance step.
        let deleted = conn.execute(
            "DELETE FROM previews WHERE photo_id IN (
                SELECT photo_id FROM previews ORDER BY last_accessed_at ASC LIMIT 50
            )",
            []
        ).map_err(|e| e.to_string())?;
        
        Ok(deleted as u64)
    }

    /// Get cache statistics for UI display
    pub fn get_stats(&self) -> CacheStats {
        let conn = self.conn.lock().unwrap();
        
        // Count thumbnails (type 0)
        let thumbnail_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM previews WHERE type = 0",
            [],
            |row| row.get(0)
        ).unwrap_or(0);
        
        // Count large previews (type 1)
        let large_preview_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM previews WHERE type = 1",
            [],
            |row| row.get(0)
        ).unwrap_or(0);
        
        // Total size of all data
        let total_size_bytes: i64 = conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(data)), 0) FROM previews",
            [],
            |row| row.get(0)
        ).unwrap_or(0);
        
        CacheStats {
            thumbnail_count: thumbnail_count as u64,
            large_preview_count: large_preview_count as u64,
            total_size_bytes: total_size_bytes as u64,
            db_path: self.cache_dir.join("preview_cache.db"),
        }
    }

    /// Clear all cache entries
    /// Returns the number of deleted entries
    pub fn clear_all(&self) -> Result<u64, String> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute("DELETE FROM previews", [])
            .map_err(|e| e.to_string())?;
        
        // VACUUM to reclaim disk space
        conn.execute("VACUUM", []).ok();
        
        Ok(deleted as u64)
    }

    /// Clear only thumbnail entries (type 0)
    /// Returns the number of deleted entries
    pub fn clear_thumbnails(&self) -> Result<u64, String> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute("DELETE FROM previews WHERE type = 0", [])
            .map_err(|e| e.to_string())?;
        Ok(deleted as u64)
    }

    /// Clear only large preview entries (type 1)
    /// Returns the number of deleted entries
    pub fn clear_previews(&self) -> Result<u64, String> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute("DELETE FROM previews WHERE type = 1", [])
            .map_err(|e| e.to_string())?;
        Ok(deleted as u64)
    }
}


impl PreviewStorage for PreviewManager {
    fn save(&self, id: &PhotoId, preview_type: PreviewType, data: &[u8]) -> DomainResult<()> {
        let conn = self.conn.lock().unwrap();
        let type_id = match preview_type {
            PreviewType::Thumbnail => 0,
            PreviewType::Large => 1,
        };
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT OR REPLACE INTO previews (photo_id, type, data, created_at, last_accessed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id.to_string(), type_id, data, now, now],
        ).map_err(|e| domain::DomainError::InfrastructureError(format!("DB error: {}", e)))?;

        Ok(())
    }

    fn get(&self, id: &PhotoId, preview_type: PreviewType) -> DomainResult<Option<Vec<u8>>> {
        let conn = self.conn.lock().unwrap();
        let type_id = match preview_type {
            PreviewType::Thumbnail => 0,
            PreviewType::Large => 1,
        };
        
        let mut stmt = conn.prepare("SELECT data FROM previews WHERE photo_id = ?1 AND type = ?2")
             .map_err(|e| domain::DomainError::InfrastructureError(format!("DB error: {}", e)))?;
             
        let data: Option<Vec<u8>> = stmt.query_row(params![id.to_string(), type_id], |row| row.get(0)).optional()
             .map_err(|e| domain::DomainError::InfrastructureError(format!("DB error: {}", e)))?;

        if data.is_some() {
             let now = chrono::Utc::now().timestamp();
             let _ = conn.execute(
                 "UPDATE previews SET last_accessed_at = ?1 WHERE photo_id = ?2 AND type = ?3", 
                 params![now, id.to_string(), type_id]
             );
        }

        Ok(data)
    }

    fn has(&self, id: &PhotoId, preview_type: PreviewType) -> DomainResult<bool> {
        let conn = self.conn.lock().unwrap();
        let type_id = match preview_type {
            PreviewType::Thumbnail => 0,
            PreviewType::Large => 1,
        };
        
        let count: i64 = conn.query_row(
            "SELECT COUNT(1) FROM previews WHERE photo_id = ?1 AND type = ?2",
            params![id.to_string(), type_id],
            |row| row.get(0),
        ).map_err(|e| domain::DomainError::InfrastructureError(format!("DB error: {}", e)))?;

        Ok(count > 0)
    }

    fn delete(&self, id: &PhotoId) -> DomainResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM previews WHERE photo_id = ?1",
            params![id.to_string()],
        ).map_err(|e| domain::DomainError::InfrastructureError(format!("DB error: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use image::{DynamicImage, RgbaImage};

    // Helper para criar imagem dummy
    fn create_dummy_image(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(width, height))
    }

    #[test]
    fn test_cache_stats_and_clearing() {
        // Setup com diretório temporário
        let dir = tempdir().unwrap();
        let manager = PreviewManager::new_with_path(dir.path().to_path_buf());
        let photo_id = "test_photo_1";

        // Inicial: stats vazios
        let stats = manager.get_stats();
        assert_eq!(stats.thumbnail_count, 0);
        assert_eq!(stats.large_preview_count, 0);

        // Inserir dados
        let img = create_dummy_image(100, 100);
        
        // Salvar Thumbnail
        manager.save_thumbnail(photo_id, &img).unwrap();
        
        // Salvar Preview
        manager.save_preview(photo_id, &img).unwrap();

        // Verificar stats pós-inserção
        let stats = manager.get_stats();
        assert_eq!(stats.thumbnail_count, 1);
        assert_eq!(stats.large_preview_count, 1);
        assert!(stats.total_size_bytes > 0);

        // Testar Clear Thumbnails
        let count = manager.clear_thumbnails().unwrap();
        assert_eq!(count, 1);
        
        let stats = manager.get_stats();
        assert_eq!(stats.thumbnail_count, 0, "Thumbnails should be gone");
        assert_eq!(stats.large_preview_count, 1, "Previews should remain");

        // Recolocar thumbnail para testar Clear All
        manager.save_thumbnail(photo_id, &img).unwrap();
        
        // Testar Clear Previews
        let count = manager.clear_previews().unwrap();
        assert_eq!(count, 1);

        let stats = manager.get_stats();
        assert_eq!(stats.thumbnail_count, 1, "Thumbnails should remain");
        assert_eq!(stats.large_preview_count, 0, "Previews should be gone");

        // Testar Clear All
        let count = manager.clear_all().unwrap();
        assert!(count >= 1); // Pode deletar metas

        let stats = manager.get_stats();
        assert_eq!(stats.thumbnail_count, 0);
        assert_eq!(stats.large_preview_count, 0);
    }
}
