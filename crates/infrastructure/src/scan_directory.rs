//! Scan Directory Use Case
//!
//! Escaneia um diretório e importa todas as fotos encontradas.

use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::FilePath,
    DomainResult,
};
use std::sync::Arc;
use std::path::Path;

/// Resultado do scan de diretório
#[derive(Debug)]
pub struct ScanDirectoryResult {
    /// Fotos importadas com sucesso
    pub imported: Vec<Photo>,
    /// Caminhos que falharam ao importar
    pub failed: Vec<(String, String)>, // (path, error)
    /// Total de arquivos encontrados
    pub total_found: usize,
}

/// Use case para escanear diretório e importar fotos
pub struct ScanDirectoryUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
}

impl ScanDirectoryUseCase {
    /// Cria uma nova instância do use case
    pub fn new(photo_repository: Arc<dyn PhotoRepository>) -> Self {
        Self { photo_repository }
    }

    /// Escaneia um diretório e importa todas as fotos encontradas
    pub async fn execute(&self, directory_path: &Path) -> DomainResult<ScanDirectoryResult> {
        // Usar FileScanner para encontrar arquivos
        let scanner = crate::file_system::FileScanner::new();
        let files = scanner.scan_directory(directory_path)?;

        let total_found = files.len();
        let mut imported = Vec::new();
        let mut failed = Vec::new();

        // Tentar importar cada arquivo encontrado
        for file_path in files {
            match self.import_photo(&file_path).await {
                Ok(photo) => imported.push(photo),
                Err(e) => {
                    failed.push((
                        file_path.to_string_lossy().to_string(),
                        e.to_string(),
                    ));
                }
            }
        }

        Ok(ScanDirectoryResult {
            imported,
            failed,
            total_found,
        })
    }

    /// Importa uma única foto
    async fn import_photo(&self, path: &Path) -> DomainResult<Photo> {
        let file_path = FilePath::new(path.to_string_lossy().as_ref())?;
        let photo = Photo::new(file_path);
        self.photo_repository.save(&photo).await?;
        Ok(photo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::repositories::PhotoRepository;
    use mockall::mock;
    use mockall::predicate::*;

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
    async fn test_scan_directory_success() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        // Expect save to be called for each photo found
        mock_repo
            .expect_save()
            .returning(|_| Ok(()))
            .times(..); // Allow any number of calls

        let use_case = ScanDirectoryUseCase::new(Arc::new(mock_repo));

        // Create a temp directory with test files
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("photo1.jpg"), b"test").unwrap();
        std::fs::write(temp_dir.path().join("photo2.png"), b"test").unwrap();

        // Act
        let result = use_case.execute(temp_dir.path()).await.unwrap();

        // Assert
        assert_eq!(result.total_found, 2);
        assert_eq!(result.imported.len(), 2);
        assert_eq!(result.failed.len(), 0);
    }

    #[tokio::test]
    async fn test_scan_directory_with_failures() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        // First save succeeds, second fails
        mock_repo
            .expect_save()
            .times(1)
            .returning(|_| Ok(()));
        
        mock_repo
            .expect_save()
            .times(1)
            .returning(|_| Err(domain::DomainError::InvalidOperation("Save failed".to_string())));

        let use_case = ScanDirectoryUseCase::new(Arc::new(mock_repo));

        // Create temp directory
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("photo1.jpg"), b"test").unwrap();
        std::fs::write(temp_dir.path().join("photo2.jpg"), b"test").unwrap();

        // Act
        let result = use_case.execute(temp_dir.path()).await.unwrap();

        // Assert
        assert_eq!(result.total_found, 2);
        assert_eq!(result.imported.len(), 1);
        assert_eq!(result.failed.len(), 1);
    }

    #[tokio::test]
    async fn test_scan_empty_directory() {
        // Arrange
        let mock_repo = MockPhotoRepo::new();
        let use_case = ScanDirectoryUseCase::new(Arc::new(mock_repo));

        // Create empty temp directory
        let temp_dir = tempfile::TempDir::new().unwrap();

        // Act
        let result = use_case.execute(temp_dir.path()).await.unwrap();

        // Assert
        assert_eq!(result.total_found, 0);
        assert_eq!(result.imported.len(), 0);
        assert_eq!(result.failed.len(), 0);
    }

    #[tokio::test]
    async fn test_scan_nonexistent_directory() {
        // Arrange
        let mock_repo = MockPhotoRepo::new();
        let use_case = ScanDirectoryUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute(Path::new("/nonexistent/path")).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_scan_directory_recursive() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        mock_repo
            .expect_save()
            .returning(|_| Ok(()))
            .times(..);

        let use_case = ScanDirectoryUseCase::new(Arc::new(mock_repo));

        // Create directory structure
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        std::fs::write(temp_dir.path().join("photo1.jpg"), b"test").unwrap();
        std::fs::write(temp_dir.path().join("subdir/photo2.jpg"), b"test").unwrap();

        // Act
        let result = use_case.execute(temp_dir.path()).await.unwrap();

        // Assert
        assert_eq!(result.total_found, 2);
        assert_eq!(result.imported.len(), 2);
    }
}
