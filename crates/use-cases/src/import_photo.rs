//! Import Photo Use Case
//!
//! Caso de uso responsável por importar uma foto para o catálogo.
//! Implementado com TDD e usando mocks para testes.

use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::FilePath,
    services::{MetadataExtractor, ThumbnailGenerator},
    DomainResult,
};
use std::sync::Arc;
use std::path::Path;

/// Use Case para importar fotos
pub struct ImportPhotoUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
    metadata_extractor: Arc<dyn MetadataExtractor>,
    thumbnail_generator: Arc<dyn ThumbnailGenerator>,
}

impl ImportPhotoUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(
        photo_repository: Arc<dyn PhotoRepository>,
        metadata_extractor: Arc<dyn MetadataExtractor>,
        thumbnail_generator: Arc<dyn ThumbnailGenerator>,
    ) -> Self {
        Self {
            photo_repository,
            metadata_extractor,
            thumbnail_generator,
        }
    }

    /// Importa uma foto do sistema de arquivos
    pub async fn execute(&self, file_path: FilePath) -> DomainResult<Photo> {
        // Criar nova entidade Photo
        let mut photo = Photo::new(file_path.clone());

        // Extrair e definir metadados (se falhar, logar e continuar sem metadados)
        if let Ok(metadata) = self.metadata_extractor.extract(&file_path) {
            photo.set_metadata(metadata);
        }

        // Gerar Thumbnail
        // TODO: Mover lógica de persistência de arquivo de thumbnail para infra ou service dedicado?
        // Por enquanto, faremos aqui: salva como <caminho>.thumb.jpg
        match self.thumbnail_generator.generate(&file_path, 300).await {
            Ok(bytes) => {
                let path_ref: &Path = file_path.as_ref();
                let _file_stem = path_ref.file_stem().unwrap_or_default();
                let file_name = path_ref.file_name().unwrap_or_default();
                let parent = path_ref.parent().unwrap_or(Path::new("."));
                
                // Opção A: Salvar no mesmo diretório com sufixo
                // let thumb_name = format!("{}.thumb.jpg", file_name.to_string_lossy());
                // let thumb_path = parent.join(thumb_name);
                
                // Opção B: Pasta global de cache (Mais limpo)
                // Para MVP, vamos usar Opção A pela simplicidade de visualização
                let thumb_path = parent.join(format!("{}.thumb.jpg", file_name.to_string_lossy()));

                if let Err(e) = tokio::fs::write(&thumb_path, bytes).await {
                    eprintln!("Failed to write thumbnail: {}", e);
                } else {
                    if let Some(path_str) = thumb_path.to_str() {
                         if let Ok(path_obj) = FilePath::new(path_str) {
                            photo.set_thumbnail_path(path_obj);
                        }
                    }
                }
            },
            Err(e) => eprintln!("Failed to generate thumbnail: {}", e),
        }
        
        // Persistir no repositório
        self.photo_repository.save(&photo).await?;
        
        Ok(photo)
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

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator)
        );
        let file_path = FilePath::new("/path/to/photo.jpg").unwrap();
        
        // Act
        let result = use_case.execute(file_path.clone()).await;
        
        // Assert
        assert!(result.is_ok());
        let photo = result.unwrap();
        assert_eq!(photo.file_path(), &file_path);
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

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator)
        );
        let file_path = FilePath::new("/path/to/photo.jpg").unwrap();
        
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

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator)
        );
        
        let file_path1 = FilePath::new("/path/to/photo1.jpg").unwrap();
        let file_path2 = FilePath::new("/path/to/photo2.jpg").unwrap();
        
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

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator)
        );
        
        // Act & Assert - diferentes extensões devem funcionar
        let extensions = vec!["jpg", "png", "raw"];
        
        for ext in extensions {
            let path = format!("/photos/image.{}", ext);
            let file_path = FilePath::new(&path).unwrap();
            let result = use_case.execute(file_path).await;
            
            assert!(result.is_ok());
        }
    }
}
