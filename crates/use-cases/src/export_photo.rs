use domain::repositories::PhotoRepository;
use domain::services::ImageExporter;
use domain::value_objects::{FilePath, PhotoId};
use domain::DomainResult;
use std::sync::Arc;

pub struct ExportPhotoUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
    image_exporter: Arc<dyn ImageExporter>,
}

impl ExportPhotoUseCase {
    pub fn new(
        photo_repository: Arc<dyn PhotoRepository>,
        image_exporter: Arc<dyn ImageExporter>,
    ) -> Self {
        Self {
            photo_repository,
            image_exporter,
        }
    }

    pub async fn execute(&self, id: PhotoId, output_path: String) -> DomainResult<()> {
        // 1. Fetch photo
        let photo = self
            .photo_repository
            .find_by_id(&id)
            .await?
            .ok_or(domain::DomainError::PhotoNotFound)?;

        // 2. Validate output path (basic)
        let output_path_vo = FilePath::new(output_path)?;

        // 3. Export
        self.image_exporter.export(&photo, &output_path_vo).await
    }
}
