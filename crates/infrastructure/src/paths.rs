use std::path::PathBuf;
use directories::UserDirs;

pub struct AppPaths;

impl AppPaths {
    /// Returns the main catalog root directory:
    /// - macOS: ~/Pictures/VintageLightbox/VintageLightbox Catalog
    /// - Windows: C:\Users\Use\Pictures\VintageLightbox\VintageLightbox Catalog
    /// - Linux: ~/Pictures/VintageLightbox/VintageLightbox Catalog
    pub fn catalog_root() -> PathBuf {
        let user_dirs = UserDirs::new().expect("Could not find user directories");
        let picture_dir = user_dirs.picture_dir().expect("Could not find Pictures directory");
        
        picture_dir
            .join("VintageLightbox")
            .join("VintageLightbox Catalog")
    }

    /// Returns the path to the main SQLite database file
    pub fn main_db_path() -> PathBuf {
        Self::catalog_root().join("vintage_lightbox.db")
    }

    /// Returns the path to the Preview Cache directory (Previews.lrdata)
    pub fn preview_cache_dir() -> PathBuf {
        Self::catalog_root().join("Previews.lrdata")
    }

    /// Returns the path to the Models directory for ML/AI models
    pub fn models_dir() -> PathBuf {
        Self::catalog_root().join("Models")
    }
}
