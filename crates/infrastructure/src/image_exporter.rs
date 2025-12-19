use async_trait::async_trait;
use domain::services::ImageExporter;
use domain::entities::Photo;
use domain::value_objects::FilePath;
use domain::{DomainError, DomainResult};
use std::path::Path;

pub struct ImageExporterImpl;

impl ImageExporterImpl {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ImageExporter for ImageExporterImpl {
    async fn export(&self, photo: &Photo, output_path: &FilePath) -> DomainResult<()> {

        let input_path = photo.file_path().as_str()?;
        
        // Load original image
        let img = image::open(Path::new(&input_path))
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to open source image: {}", e)))?;

        // Apply Exposure
        let exposure = photo.edit_exposure().unwrap_or(0.0);
        let brightened = if exposure != 0.0 {
            // Mapping -5.0..5.0 to -50..50
            image::imageops::brighten(&img, (exposure * 10.0) as i32)
        } else {
            img.to_rgba8()
        };

        // Apply Contrast
        let contrast = photo.edit_contrast().unwrap_or(1.0);
        let contrasted = if contrast != 1.0 {
            image::imageops::contrast(&brightened, contrast)
        } else {
            brightened
        };

        // Save
        contrasted.save(Path::new(output_path.as_str()?))
             .map_err(|e| DomainError::InfrastructureError(format!("Failed to save exported image: {}", e)))?;

        Ok(())
    }
}
