use std::io::Cursor;
use domain::{
    value_objects::FilePath,
    services::ThumbnailGenerator,
    DomainResult,
    DomainError,
};
use async_trait::async_trait;
// `image::io::Reader` virou `image::ImageReader` no 0.25 — o alias antigo ainda
// existe, mas depreciado. O nome local não muda, então nada mais aqui precisa
// saber disso.
use image::ImageReader;
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
        // Implementação simplificada reaproveitando generate_set
        let results = self.generate_set(path, &[max_size]).await?;
        Ok(results.into_iter().next().unwrap())
    }

    async fn generate_set(&self, path: &FilePath, max_sizes: &[u32]) -> DomainResult<Vec<Vec<u8>>> {
        let path_str = path.to_string();
        let path_buf = path.as_ref().to_path_buf();
        let sizes = max_sizes.to_vec();

        // Operações de imagem são intensivas em CPU, rodar em thread separada
        let result = tokio::task::spawn_blocking(move || {
            use crate::raw_processing::{is_raw_file, load_raw_as_dynamic_image, extract_embedded_preview};

            let mut results = Vec::new();
            if is_raw_file(&path_str) {
                // Estratégia RAW:
                // 1. Para thumbnails pequenos, tenta extrair o preview embutido
                // 2. Para previews grandes, decodifica o RAW
                
                // Carrega a imagem full quality uma única vez se necessário
                let mut full_image: Option<image::DynamicImage> = None;

                for &size in &sizes {
                    if size <= 320 {
                        // Tenta extrair embedded preview
                        if let Some(embedded) = extract_embedded_preview(&path_str, size) {
                            // Carrega o embedded preview como imagem para redimensionar se necessário
                            if let Ok(img) = image::load_from_memory(&embedded) {
                                let thumbnail = img.thumbnail(size, size);
                                let mut bytes: Vec<u8> = Vec::new();
                                let mut cursor = Cursor::new(&mut bytes);
                                if thumbnail.write_to(&mut cursor, ImageFormat::Jpeg).is_ok() {
                                    results.push(bytes);
                                    continue;
                                }
                            }
                        }
                    }

                    // Se não tiver full image ainda, carrega agora
                    if full_image.is_none() {
                        full_image = Some(load_raw_as_dynamic_image(&path_str)
                            .map_err(|e| DomainError::InfrastructureError(format!("Failed to decode RAW: {}", e)))?);
                    }

                    if let Some(img) = &full_image {
                        let thumbnail = img.thumbnail(size, size);
                        let mut bytes: Vec<u8> = Vec::new();
                        let mut cursor = Cursor::new(&mut bytes);
                        thumbnail.write_to(&mut cursor, ImageFormat::Jpeg)
                            .map_err(|e| DomainError::InvalidOperation(format!("Failed to write thumbnail: {}", e)))?;
                        results.push(bytes);
                    }
                }
            } else {
                // Estratégia Imagem Comum (JPG/PNG): Carregar uma vez, redimensionar N vezes
                let img = ImageReader::open(&path_buf)
                    .map_err(|e| DomainError::InvalidOperation(format!("Failed to open image: {}", e)))?
                    .with_guessed_format()
                    .map_err(|e| DomainError::InvalidOperation(format!("Failed to guess format: {}", e)))?
                    .decode()
                    .map_err(|e| DomainError::InvalidOperation(format!("Failed to decode image: {}", e)))?;

                for &size in &sizes {
                    let thumbnail = img.thumbnail(size, size);
                    let mut bytes: Vec<u8> = Vec::new();
                    let mut cursor = Cursor::new(&mut bytes);
                    thumbnail.write_to(&mut cursor, ImageFormat::Jpeg)
                        .map_err(|e| DomainError::InvalidOperation(format!("Failed to write thumbnail: {}", e)))?;
                    results.push(bytes);
                }
            }
            Ok(results)
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
