//! Import Photo Use Case
//!
//! Caso de uso responsável por importar uma foto para o catálogo.
//! Implementado com TDD e usando mocks para testes.

use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::FilePath,
    services::{MetadataExtractor, ThumbnailGenerator, PreviewStorage, PreviewType},
    DomainResult,
};
use std::sync::Arc;
use std::path::Path;

/// Use Case para importar fotos
pub struct ImportPhotoUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
    metadata_extractor: Arc<dyn MetadataExtractor>,
    thumbnail_generator: Arc<dyn ThumbnailGenerator>,
    preview_storage: Arc<dyn PreviewStorage>,
}

impl ImportPhotoUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(
        photo_repository: Arc<dyn PhotoRepository>,
        metadata_extractor: Arc<dyn MetadataExtractor>,
        thumbnail_generator: Arc<dyn ThumbnailGenerator>,
        preview_storage: Arc<dyn PreviewStorage>,
    ) -> Self {
        Self {
            photo_repository,
            metadata_extractor,
            thumbnail_generator,
            preview_storage,
        }
    }

    /// Importa uma foto do sistema de arquivos, copiando para diretório organizado
    pub async fn execute(&self, file_path: FilePath) -> DomainResult<Photo> {
        // Extrair metadados primeiro para obter a data
        let metadata_result = self.metadata_extractor.extract(&file_path);
        
        // Determinar a data para organização das pastas
        let (year, month, day) = if let Ok(ref metadata) = metadata_result {
            if let Some(ref dt) = metadata.date_time {
                // Tentar parsear EXIF date format "YYYY:MM:DD HH:MM:SS"
                let parts: Vec<&str> = dt.split(' ').next()
                    .unwrap_or("")
                    .split(':')
                    .collect();
                if parts.len() >= 3 {
                    (parts[0].to_string(), parts[1].to_string(), parts[2].to_string())
                } else {
                    Self::current_date()
                }
            } else {
                Self::current_date()
            }
        } else {
            Self::current_date()
        };
        
        // Obter diretório base: ~/Pictures/VintageLightbox
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let base_dir = std::path::PathBuf::from(&home)
            .join("Pictures")
            .join("VintageLightbox");
        
        // Criar subdiretório baseado na data: YYYY/MM/DD
        let dest_dir = base_dir.join(&year).join(&month).join(&day);
        
        // Criar diretórios se não existirem
        if !dest_dir.exists() {
            tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| {
                domain::DomainError::InfrastructureError(format!(
                    "Failed to create directory {}: {}", dest_dir.display(), e
                ))
            })?;
        }
        
        // Obter extensão do arquivo original
        let source_path: &Path = file_path.as_ref();
        let extension = source_path.extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "jpg".to_string());
        
        // Gerar nome padronizado: photo-YYYY-MM-DD-NNN
        // Encontrar o próximo número sequencial disponível
        let mut sequential = 1;
        loop {
            let new_name = format!("photo-{}-{}-{}-{:03}.{}", year, month, day, sequential, extension);
            let dest_path = dest_dir.join(&new_name);
            if !dest_path.exists() {
                break;
            }
            sequential += 1;
        }
        
        let new_file_name = format!("photo-{}-{}-{}-{:03}.{}", year, month, day, sequential, extension);
        let dest_path = dest_dir.join(&new_file_name);
        
        // Copiar o arquivo para o destino com novo nome
        let final_path = if dest_path.exists() {
            println!("File already exists at: {}", dest_path.display());
            dest_path
        } else {
            tokio::fs::copy(source_path, &dest_path).await.map_err(|e| {
                domain::DomainError::InfrastructureError(format!(
                    "Failed to copy file to {}: {}", dest_path.display(), e
                ))
            })?;
            println!("Copied to: {}", dest_path.display());
            dest_path
        };
        

        
        // Criar FilePath com o novo caminho
        let new_file_path = FilePath::new(final_path.to_string_lossy().as_ref())?;
        
        // Criar nova entidade Photo com o caminho copiado
        let mut photo = Photo::new(new_file_path.clone());

        // Definir metadados se disponíveis
        if let Ok(metadata) = metadata_result {
            photo.set_metadata(metadata);
        }

        // Gerar Thumbnail no subdiretório thumb
        // Gerar Thumbnail no storage
        match self.thumbnail_generator.generate(&new_file_path, 300).await {
            Ok(bytes) => {
                if let Err(e) = self.preview_storage.save(&photo.id(), PreviewType::Thumbnail, &bytes) {
                    eprintln!("Failed to save thumbnail: {}", e);
                }
            },
            Err(e) => eprintln!("Failed to generate thumbnail: {}", e),
        }

        // Gerar Preview (Otimizado para edição fluida)
        // Usamos 2560px como um bom balanço entre qualidade e performance (Matches Roadmap)
        // Gerar Preview (Otimizado para edição fluida)
        match self.thumbnail_generator.generate(&new_file_path, 2560).await {
            Ok(bytes) => {
                if let Err(e) = self.preview_storage.save(&photo.id(), PreviewType::Large, &bytes) {
                    eprintln!("Failed to save preview: {}", e);
                }
            },
            Err(e) => eprintln!("Failed to generate preview: {}", e),
        }
        
        // Persistir no repositório
        self.photo_repository.save(&photo).await?;
        
        Ok(photo)
    }
    
    /// Retorna data atual formatada como (YYYY, MM, DD)
    fn current_date() -> (String, String, String) {
        let now = chrono::Local::now();
        (
            now.format("%Y").to_string(),
            now.format("%m").to_string(),
            now.format("%d").to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        DomainError, PhotoRepository,
        value_objects::PhotoMetadata,
    };
    use mockall::mock;
    use mockall::predicate::*;
    use tempfile::NamedTempFile;

    // Mock do MetadataExtractor
    mock! {
        pub MetadataExtractor {}

        impl MetadataExtractor for MetadataExtractor {
            fn extract(&self, path: &FilePath) -> DomainResult<PhotoMetadata>;
        }
    }

    // Mock do ThumbnailGenerator
    mock! {
        pub ThumbnailGenerator {}
        #[async_trait::async_trait]
        impl ThumbnailGenerator for ThumbnailGenerator {
            async fn generate(&self, path: &FilePath, max_dimension: u32) -> DomainResult<Vec<u8>>;
        }
    }

    // Mock do PreviewStorage
    mock! {
        pub PreviewStorage {}
        impl PreviewStorage for PreviewStorage {
            fn save(&self, id: &domain::value_objects::PhotoId, preview_type: PreviewType, data: &[u8]) -> DomainResult<()>;
            fn get(&self, id: &domain::value_objects::PhotoId, preview_type: PreviewType) -> DomainResult<Option<Vec<u8>>>;
            fn has(&self, id: &domain::value_objects::PhotoId, preview_type: PreviewType) -> DomainResult<bool>;
            fn delete(&self, id: &domain::value_objects::PhotoId) -> DomainResult<()>;
        }
    }

    // Mock do PhotoRepository
    mock! {
        pub PhotoRepo {}

        #[async_trait::async_trait]
        impl PhotoRepository for PhotoRepo {
            async fn save(&self, photo: &Photo) -> DomainResult<()>;
            async fn find_by_id(&self, id: &domain::value_objects::PhotoId) -> DomainResult<Option<Photo>>;
            async fn find_all(&self) -> DomainResult<Vec<Photo>>;
            async fn update(&self, photo: &Photo) -> DomainResult<()>;
            async fn delete(&self, id: &domain::value_objects::PhotoId) -> DomainResult<()>;
            async fn exists(&self, id: &domain::value_objects::PhotoId) -> DomainResult<bool>;
            async fn find_by_content_hash(&self, hash: &str) -> DomainResult<Option<Photo>>;
        }
    }

    #[tokio::test]
    async fn test_import_photo_success() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        // Espera-se que save seja chamado uma vez e retorne Ok
        mock_repo
            .expect_save()
            .times(1)
            .returning(|_| Ok(()));

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));
        
        let mut mock_generator = MockThumbnailGenerator::new();
        mock_generator
            .expect_generate()
            .returning(|_, _| Ok(vec![]));

        let mut mock_preview_storage = MockPreviewStorage::new();
        mock_preview_storage.expect_save().returning(|_, _, _| Ok(()));

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
            Arc::new(mock_preview_storage)
        );
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();
        let file_path = FilePath::new(path).unwrap();
        
        // Act
        let result = use_case.execute(file_path.clone()).await;
        
        // Assert
        assert!(result.is_ok());
        let photo = result.unwrap();
        let path_str = photo.file_path().to_string();
        assert!(path_str.contains("VintageLightbox"));
        assert_ne!(photo.file_path(), &file_path);
    }

    #[tokio::test]
    async fn test_import_photo_repository_error() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        // Simular erro no repositório
        mock_repo
            .expect_save()
            .times(1)
            .returning(|_| Err(DomainError::InvalidFilePath("DB error".to_string())));

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));
        
        let mut mock_generator = MockThumbnailGenerator::new();
        mock_generator
            .expect_generate()
            .returning(|_, _| Ok(vec![]));

        let mut mock_preview_storage = MockPreviewStorage::new();
        mock_preview_storage.expect_save().returning(|_, _, _| Ok(()));

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
            Arc::new(mock_preview_storage)
        );
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();
        let file_path = FilePath::new(path).unwrap();
        
        // Act
        let result = use_case.execute(file_path).await;
        
        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_import_photo_creates_new_id() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        mock_repo
            .expect_save()
            .times(2)
            .returning(|_| Ok(()));

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));
        
        let mut mock_generator = MockThumbnailGenerator::new();
        mock_generator
            .expect_generate()
            .returning(|_, _| Ok(vec![]));

        let mut mock_preview_storage = MockPreviewStorage::new();
        mock_preview_storage.expect_save().returning(|_, _, _| Ok(()));

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
            Arc::new(mock_preview_storage)
        );
        
        let temp_file1 = NamedTempFile::new().unwrap();
        let file_path1 = FilePath::new(temp_file1.path().to_str().unwrap()).unwrap();
        
        let temp_file2 = NamedTempFile::new().unwrap();
        let file_path2 = FilePath::new(temp_file2.path().to_str().unwrap()).unwrap();
        
        // Act
        let photo1 = use_case.execute(file_path1).await.unwrap();
        let photo2 = use_case.execute(file_path2).await.unwrap();
        
        // Assert
        // Cada foto deve ter um ID único
        assert_ne!(photo1.id(), photo2.id());
    }

    #[tokio::test]
    async fn test_import_photo_with_different_extensions() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        mock_repo
            .expect_save()
            .times(3)
            .returning(|_| Ok(()));

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));
        
        let mut mock_generator = MockThumbnailGenerator::new();
        mock_generator
            .expect_generate()
            .returning(|_, _| Ok(vec![]));

        let mut mock_preview_storage = MockPreviewStorage::new();
        mock_preview_storage.expect_save().returning(|_, _, _| Ok(()));

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
            Arc::new(mock_preview_storage)
        );
        
        // Act & Assert - diferentes extensões devem funcionar
        let extensions = vec!["jpg", "png", "raw"];
        
        for ext in extensions {
            let mut builder = tempfile::Builder::new();
            let suffix = format!(".{}", ext);
            builder.suffix(&suffix);
            let temp_file = builder.tempfile().unwrap();
            let path_str = temp_file.path().to_str().unwrap();
            let file_path = FilePath::new(path_str).unwrap();
            
            let result = use_case.execute(file_path).await;
            
            assert!(result.is_ok());
        }
    }
}
