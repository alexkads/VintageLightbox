use std::sync::Arc;
use domain::repositories::PhotoRepository;
use crate::view_models::PhotoViewModel;

pub struct LibraryController {
    photo_repository: Arc<dyn PhotoRepository>,
}

impl LibraryController {
    pub fn new(photo_repository: Arc<dyn PhotoRepository>) -> Self {
        Self { photo_repository }
    }

    pub async fn get_all_photos(&self) -> Result<Vec<PhotoViewModel>, String> {
        let photos = self.photo_repository.find_all().await
            .map_err(|e| e.to_string())?;

        let view_models = photos.into_iter().map(|photo| {
            PhotoViewModel {
                id: photo.id().to_string(),
                path: photo.file_path().as_ref().to_string_lossy().to_string(),
                name: photo.file_path().file_name().unwrap_or_default().to_string(),
                thumbnail_path: photo.thumbnail_path().map(|p| p.to_string_lossy().to_string()),
            }
        }).collect();

        Ok(view_models)
    }
}
