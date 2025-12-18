use std::io::Cursor;
use domain::{
    value_objects::FilePath,
    services::ThumbnailGenerator,
    DomainResult,
    DomainError,
};
use async_trait::async_trait;
use image::io::Reader as ImageReader;
use image::ImageFormat;

/// Implementação do ThumbnailGenerator usando a crate `image`
pub struct ThumbnailGeneratorImpl;

impl ThumbnailGeneratorImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ThumbnailGeneratorImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ThumbnailGenerator for ThumbnailGeneratorImpl {
    async fn generate(&self, path: &FilePath, max_size: u32) -> DomainResult<Vec<u8>> {
        let path_buf = path.as_ref().to_path_buf();
        
        // Operações de imagem são intensivas em CPU, rodar em thread separada
        let result = tokio::task::spawn_blocking(move || {
            // Tenta abrir e decodificar a imagem
            let img = ImageReader::open(&path_buf)
                .map_err(|e| DomainError::InvalidOperation(format!("Failed to open image: {}", e)))?
                .with_guessed_format()
                .map_err(|e| DomainError::InvalidOperation(format!("Failed to guess format: {}", e)))?
                .decode()
                .map_err(|e| DomainError::InvalidOperation(format!("Failed to decode image: {}", e)))?;

            // Gera o thumbnail mantendo aspect ratio
            let thumbnail = img.thumbnail(max_size, max_size);
            
            let mut bytes: Vec<u8> = Vec::new();
            let mut cursor = Cursor::new(&mut bytes);
            
            // Salva como JPEG
            thumbnail.write_to(&mut cursor, ImageFormat::Jpeg)
                .map_err(|e| DomainError::InvalidOperation(format!("Failed to write thumbnail: {}", e)))?;
                
            Ok(bytes)
        }).await.map_err(|e| DomainError::InfrastructureError(format!("Task join error: {}", e)))??;
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_test_image(width: u32, height: u32) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        let img = image::ImageBuffer::<image::Rgb<u8>, Vec<u8>>::new(width, height);
        let img = image::DynamicImage::ImageRgb8(img);
        
        // Write to file
        img.write_to(&mut file, image::ImageFormat::Jpeg).unwrap();
        file
    }

    #[tokio::test]
    async fn test_generate_thumbnail_success() {
        let generator = ThumbnailGeneratorImpl::new();
        // Create 200x200 image
        let file = create_test_image(200, 200);
        let path = FilePath::new(file.path().to_str().unwrap()).unwrap();
        
        // Generate 100x100 thumbnail
        let result = generator.generate(&path, 100).await;
        
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
        
        // Verify output is a valid image
        let img = image::load_from_memory(&bytes).unwrap();
        assert!(img.width() <= 100);
        assert!(img.height() <= 100);
    }

    #[tokio::test]
    async fn test_generate_thumbnail_invalid_path() {
        let generator = ThumbnailGeneratorImpl::new();
        let path = FilePath::new("/nonexistent/file.jpg").unwrap();
        
        let result = generator.generate(&path, 100).await;
        assert!(result.is_err());
    }
}
