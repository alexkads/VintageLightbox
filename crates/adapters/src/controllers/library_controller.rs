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
                    let metadata = photo.metadata();
                    let date = metadata.and_then(|m| m.date_time.clone()).unwrap_or_default();
                    let camera = metadata.and_then(|m| m.camera_model.clone()).unwrap_or_default();
                    let exposure = if let Some(meta) = metadata {
                        format!("ISO {} {} {}", 
                            meta.iso.unwrap_or(0), 
                            meta.aperture.map(|f| format!("f/{:.1}", f)).unwrap_or_default(), 
                            meta.shutter_speed.clone().unwrap_or_default()
                        ).trim().to_string()
                    } else {
                        String::new()
                    };
                    let rating = photo.rating().map(|r| r.value() as i32).unwrap_or(0);

                    PhotoViewModel {
                        id: photo.id().to_string(),
                        name: photo.file_name().unwrap_or_default().to_string(),
                        path: photo.file_path().to_string(),
                        thumbnail_path: photo.thumbnail_path().map(|p| p.to_string()),
                        date,
                        camera,
                        exposure,
                        rating,
                    }
                })
                .collect();

        Ok(view_models)
    }
}
