//! Configure Print Job Use Case
//!
//! Caso de uso responsável por configurar um trabalho de impressão.
//! Implementado com TDD.

use domain::{
    entities::PrintJob,
    repositories::PhotoRepository,
    value_objects::{PhotoId, PrintLayout, PrintSettings},
    DomainError, DomainResult,
};
use std::sync::Arc;

/// Use Case para configurar trabalhos de impressão
pub struct ConfigurePrintJobUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
}

impl ConfigurePrintJobUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(photo_repository: Arc<dyn PhotoRepository>) -> Self {
        Self { photo_repository }
    }

    /// Configura um novo trabalho de impressão
    pub async fn execute(
        &self,
        photo_ids: Vec<PhotoId>,
        settings: PrintSettings,
        layout: PrintLayout,
    ) -> DomainResult<PrintJob> {
        // Valida que há pelo menos uma foto
        if photo_ids.is_empty() {
            return Err(DomainError::InvalidOperation(
                "Print job must have at least one photo".to_string(),
            ));
        }

        // Valida que todas as fotos existem
        for photo_id in &photo_ids {
            let exists = self.photo_repository.exists(photo_id).await?;
            if !exists {
                return Err(DomainError::PhotoNotFound);
            }
        }

        // Cria o trabalho de impressão
        PrintJob::new(photo_ids, settings, layout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        entities::Photo,
        repositories::PhotoRepository,
        value_objects::{ColorMode, FilePath, Margins, Orientation, PaperSize},
    };
    use mockall::mock;
    use mockall::predicate::*;

    // Mock do PhotoRepository
    mock! {
        pub PhotoRepo {}

        #[async_trait::async_trait]
        impl PhotoRepository for PhotoRepo {
            async fn save(&self, photo: &Photo) -> DomainResult<()>;
            async fn find_by_id(&self, id: &PhotoId) -> DomainResult<Option<Photo>>;
            async fn find_all(&self) -> DomainResult<Vec<Photo>>;
            async fn update(&self, photo: &Photo) -> DomainResult<()>;
            async fn delete(&self, id: &PhotoId) -> DomainResult<()>;
            async fn exists(&self, id: &PhotoId) -> DomainResult<bool>;
            async fn find_by_content_hash(&self, hash: &str) -> DomainResult<Option<Photo>>;
        }
    }

    fn create_test_settings() -> PrintSettings {
        PrintSettings::new(
            PaperSize::A4,
            Orientation::Portrait,
            Margins::default_margins(),
            300,
            ColorMode::Color,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_configure_print_job_success() {
        // Arrange
        let photo_ids = vec![PhotoId::new(), PhotoId::new(), PhotoId::new()];
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mut mock_repo = MockPhotoRepo::new();

        // Mock exists para todas as fotos
        mock_repo
            .expect_exists()
            .times(3)
            .returning(|_| Ok(true));

        let use_case = ConfigurePrintJobUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute(photo_ids.clone(), settings, layout).await;

        // Assert
        assert!(result.is_ok());
        let print_job = result.unwrap();
        assert_eq!(print_job.photo_ids().len(), 3);
        assert_eq!(print_job.layout(), PrintLayout::Single);
    }

    #[tokio::test]
    async fn test_configure_print_job_with_empty_photos() {
        // Arrange
        let photo_ids = vec![];
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mock_repo = MockPhotoRepo::new();
        let use_case = ConfigurePrintJobUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute(photo_ids, settings, layout).await;

        // Assert
        assert!(result.is_err());
        match result {
            Err(DomainError::InvalidOperation(_)) => {}
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[tokio::test]
    async fn test_configure_print_job_with_nonexistent_photo() {
        // Arrange
        let photo_ids = vec![PhotoId::new(), PhotoId::new()];
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mut mock_repo = MockPhotoRepo::new();

        // Mock exists - primeira foto existe, segunda não
        mock_repo
            .expect_exists()
            .times(2)
            .returning(|_| {
                static mut CALL_COUNT: usize = 0;
                unsafe {
                    CALL_COUNT += 1;
                    Ok(CALL_COUNT == 1)
                }
            });

        let use_case = ConfigurePrintJobUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute(photo_ids, settings, layout).await;

        // Assert
        assert!(result.is_err());
        match result {
            Err(DomainError::PhotoNotFound) => {}
            _ => panic!("Expected PhotoNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_configure_print_job_with_multiple_layout() {
        // Arrange
        let photo_ids = vec![PhotoId::new(); 4];
        let settings = create_test_settings();
        let layout = PrintLayout::multiple(2, 2).unwrap();

        let mut mock_repo = MockPhotoRepo::new();

        mock_repo
            .expect_exists()
            .times(4)
            .returning(|_| Ok(true));

        let use_case = ConfigurePrintJobUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute(photo_ids, settings, layout).await;

        // Assert
        assert!(result.is_ok());
        let print_job = result.unwrap();
        assert_eq!(print_job.photo_count(), 4);
        assert_eq!(print_job.layout(), layout);
        assert_eq!(print_job.page_count(), 1); // 4 fotos em 2x2 = 1 página
    }

    #[tokio::test]
    async fn test_configure_print_job_with_contact_sheet() {
        // Arrange
        let photo_ids = vec![PhotoId::new(); 25];
        let settings = create_test_settings();
        let layout = PrintLayout::contact_sheet(12).unwrap();

        let mut mock_repo = MockPhotoRepo::new();

        mock_repo
            .expect_exists()
            .times(25)
            .returning(|_| Ok(true));

        let use_case = ConfigurePrintJobUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute(photo_ids, settings, layout).await;

        // Assert
        assert!(result.is_ok());
        let print_job = result.unwrap();
        assert_eq!(print_job.photo_count(), 25);
        assert_eq!(print_job.page_count(), 3); // 25 fotos, 12 por página = 3 páginas
    }

    #[tokio::test]
    async fn test_configure_print_job_repository_error() {
        // Arrange
        let photo_ids = vec![PhotoId::new()];
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mut mock_repo = MockPhotoRepo::new();

        // Mock exists com erro
        mock_repo
            .expect_exists()
            .times(1)
            .returning(|_| Err(DomainError::InfrastructureError("DB error".to_string())));

        let use_case = ConfigurePrintJobUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute(photo_ids, settings, layout).await;

        // Assert
        assert!(result.is_err());
    }
}
