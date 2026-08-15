use domain::import_source::{ImportSource, ImportSourceType};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;

// Simple in-memory history for now, can be upgraded to DB or JSON file
pub struct ImportHistoryRepository {
    history: Arc<Mutex<Vec<PathBuf>>>, // Store paths
}

impl Default for ImportHistoryRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportHistoryRepository {
    pub fn new() -> Self {
        Self {
            history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_recent(&self, path: PathBuf) {
        if let Ok(mut history) = self.history.lock() {
            // Remove if exists to move to top
            history.retain(|p| p != &path);
            history.insert(0, path);
            // Limit to 5
            if history.len() > 5 {
                history.truncate(5);
            }
        }
    }

    pub fn get_recent(&self) -> Vec<ImportSource> {
        if let Ok(history) = self.history.lock() {
            history.iter().map(|path| {
                ImportSource {
                    id: format!("hist://{}", path.display()),
                    name: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                    path: path.clone(),
                    source_type: ImportSourceType::History,
                }
            }).collect()
        } else {
            Vec::new()
        }
    }
}
