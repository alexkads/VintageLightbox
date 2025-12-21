use std::path::PathBuf;
use std::fs;
use directories::ProjectDirs;
use image::DynamicImage;

pub struct PreviewManager {
    cache_dir: PathBuf,
}

impl PreviewManager {
    pub fn new() -> Self {
        let cache_dir = if let Some(proj_dirs) = ProjectDirs::from("com", "vintagelightbox", "app") {
            proj_dirs.cache_dir().join("previews")
        } else {
            PathBuf::from(".cache/previews")
        };

        if !cache_dir.exists() {
            let _ = fs::create_dir_all(&cache_dir);
        }

        Self { cache_dir }
    }

    pub fn get_preview_path(&self, photo_id: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.jpg", photo_id))
    }

    pub fn get_preview(&self, photo_id: &str) -> Option<DynamicImage> {
        let path = self.get_preview_path(photo_id);
        if path.exists() {
            image::open(path).ok()
        } else {
            None
        }
    }

    pub fn save_preview(&self, photo_id: &str, image: &DynamicImage) -> Result<(), String> {
        let path = self.get_preview_path(photo_id);
        let file = fs::File::create(path).map_err(|e| e.to_string())?;
        // Use high quality (90) to avoid artifacts
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, 90);
        encoder.encode(image.as_bytes(), image.width(), image.height(), image.color())
            .map_err(|e| e.to_string())
    }
}
