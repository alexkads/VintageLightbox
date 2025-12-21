use std::path::PathBuf;
use std::sync::Mutex;
use std::fs;
use directories::ProjectDirs;
use image::DynamicImage;
use domain::services::{PreviewStorage, PreviewType};
use domain::value_objects::PhotoId;
use domain::DomainResult;
use rusqlite::{params, Connection, OptionalExtension};

pub struct PreviewManager {
    conn: Mutex<Connection>,
    #[allow(dead_code)]
    cache_dir: PathBuf,
}

impl PreviewManager {
    pub fn new() -> Self {
        let cache_dir = if let Some(proj_dirs) = ProjectDirs::from("com", "vintagelightbox", "app") {
            proj_dirs.cache_dir().join("previews")
        } else {
            PathBuf::from(".cache/previews")
        };
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
        encoder.encode(image.as_bytes(), image.width(), image.height(), image.color())
            .map_err(|e| e.to_string())?;

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
        encoder.encode(image.as_bytes(), image.width(), image.height(), image.color())
            .map_err(|e| e.to_string())?;

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
