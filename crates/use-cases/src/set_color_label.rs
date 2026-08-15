//! Set Color Label Use Case
//!
//! Caso de uso responsável por definir ou remover color label de uma foto.
//! Implementado com TDD.

use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::{PhotoId, ColorLabel},
    DomainResult, DomainError,
};
use std::sync::Arc;

/// Use Case para definir color labels em fotos
pub struct SetColorLabelUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
}

impl SetColorLabelUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(photo_repository: Arc<dyn PhotoRepository>) -> Self {
        Self { photo_repository }
    }

    /// Define o color label de uma foto
    pub async fn execute(&self, photo_id: PhotoId, color_label: ColorLabel) -> DomainResult<Photo> {
        // Buscar foto no repositório
        let photo_option = self.photo_repository.find_by_id(&photo_id).await?;
        
        let mut photo = photo_option
            .ok_or(DomainError::PhotoNotFound)?;
        
        // Aplicar color label
        photo.set_color_label(color_label);
        
        // Persistir mudança
        self.photo_repository.update(&photo).await?;
        
        Ok(photo)
    }

    /// Remove o color label de uma foto
    pub async fn clear(&self, photo_id: PhotoId) -> DomainResult<Photo> {
        // Buscar foto no repositório
        let photo_option = self.photo_repository.find_by_id(&photo_id).await?;
        
        let mut photo = photo_option
            .ok_or(DomainError::PhotoNotFound)?;
        
        // Remover color label
        photo.remove_color_label();
        
        // Persistir mudança
        self.photo_repository.update(&photo).await?;
        
        Ok(photo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{repositories::PhotoRepository, value_objects::FilePath};
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

    #[tokio::test]
    async fn test_set_color_label_success() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let photo = Photo::new(file_path);
        let photo_id = photo.id();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        // Mock find_by_id
        let photo_clone = photo.clone();
        mock_repo
            .expect_find_by_id()
            .with(eq(photo_id))
            .times(1)
            .returning(move |_| Ok(Some(photo_clone.clone())));
        
        // Mock update
        mock_repo
            .expect_update()
            .times(1)
            .returning(|_| Ok(()));
        
        let use_case = SetColorLabelUseCase::new(Arc::new(mock_repo));
        let color_label = ColorLabel::Red;
        
        // Act
        let result = use_case.execute(photo_id, color_label).await;
        
        // Assert
        assert!(result.is_ok());
        let updated_photo = result.unwrap();
        assert_eq!(updated_photo.color_label(), Some(color_label));
    }

    #[tokio::test]
    async fn test_set_color_label_photo_not_found() {
        // Arrange
        let photo_id = PhotoId::new();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        // Mock find_by_id retornando None
        mock_repo
            .expect_find_by_id()
            .with(eq(photo_id))
            .times(1)
            .returning(|_| Ok(None));
        
        let use_case = SetColorLabelUseCase::new(Arc::new(mock_repo));
        let color_label = ColorLabel::Blue;
        
        // Act
        let result = use_case.execute(photo_id, color_label).await;
        
        // Assert
        assert!(result.is_err());
        match result {
            Err(DomainError::PhotoNotFound) => {},
            _ => panic!("Expected PhotoNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_clear_color_label_success() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let mut photo = Photo::new(file_path);
        photo.set_color_label(ColorLabel::Yellow);
        let photo_id = photo.id();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        // Mock find_by_id
        let photo_clone = photo.clone();
        mock_repo
            .expect_find_by_id()
            .with(eq(photo_id))
            .times(1)
            .returning(move |_| Ok(Some(photo_clone.clone())));
        
        // Mock update
        mock_repo
            .expect_update()
            .times(1)
            .returning(|_| Ok(()));
        
        let use_case = SetColorLabelUseCase::new(Arc::new(mock_repo));
        
        // Act
        let result = use_case.clear(photo_id).await;
        
        // Assert
        assert!(result.is_ok());
        let updated_photo = result.unwrap();
        assert_eq!(updated_photo.color_label(), None);
    }

    #[tokio::test]
    async fn test_change_color_label_multiple_times() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let photo = Photo::new(file_path);
        let photo_id = photo.id();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        // Mock find_by_id (3 chamadas)
        let photo_clone1 = photo.clone();
        let photo_clone2 = photo.clone();
        let photo_clone3 = photo.clone();
        mock_repo
            .expect_find_by_id()
            .times(3)
            .returning(move |_| {
                static mut CALL_COUNT: usize = 0;
                unsafe {
                    CALL_COUNT += 1;
                    match CALL_COUNT {
                        1 => Ok(Some(photo_clone1.clone())),
                        2 => {
                            let mut p = photo_clone2.clone();
                            p.set_color_label(ColorLabel::Red);
                            Ok(Some(p))
                        },
                        _ => {
                            let mut p = photo_clone3.clone();
                            p.set_color_label(ColorLabel::Green);
                            Ok(Some(p))
                        }
                    }
                }
            });
        
        // Mock update (3 chamadas)
        mock_repo
            .expect_update()
            .times(3)
            .returning(|_| Ok(()));
        
        let use_case = SetColorLabelUseCase::new(Arc::new(mock_repo));
        
        // Act - múltiplas mudanças
        let result1 = use_case.execute(photo_id, ColorLabel::Red).await;
        assert!(result1.is_ok());
        
        let result2 = use_case.execute(photo_id, ColorLabel::Green).await;
        assert!(result2.is_ok());
        
        let result3 = use_case.execute(photo_id, ColorLabel::Purple).await;
        
        // Assert
        assert!(result3.is_ok());
        let final_photo = result3.unwrap();
        assert_eq!(final_photo.color_label(), Some(ColorLabel::Purple));
    }

    #[tokio::test]
    async fn test_set_all_color_labels() {
        // Arrange
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let photo = Photo::new(file_path);
        let photo_id = photo.id();
        
        let mut mock_repo = MockPhotoRepo::new();
        
        // Mock find_by_id (5 cores)
        mock_repo
            .expect_find_by_id()
            .times(5)
            .returning(move |_| {
                let p = Photo::new(FilePath::new("/photos/test.jpg").unwrap());
                Ok(Some(p))
            });
        
        // Mock update (5 cores)
        mock_repo
            .expect_update()
            .times(5)
            .returning(|_| Ok(()));
        
        let use_case = SetColorLabelUseCase::new(Arc::new(mock_repo));
        
        // Act - testar todas as cores
        let colors = vec![
            ColorLabel::Red,
            ColorLabel::Yellow,
            ColorLabel::Green,
            ColorLabel::Blue,
            ColorLabel::Purple,
        ];
        
        for color in colors {
            let result = use_case.execute(photo_id, color).await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap().color_label(), Some(color));
        }
    }
}
