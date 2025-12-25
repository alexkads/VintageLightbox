use std::sync::Arc;
use domain::{
    repositories::PhotoRepository,
    value_objects::PhotoId,
    DomainResult, DomainError,
};

/// Use Case: Salvar edições de uma foto
pub struct SavePhotoEditsUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
}

impl SavePhotoEditsUseCase {
    pub fn new(photo_repository: Arc<dyn PhotoRepository>) -> Self {
        Self { photo_repository }
    }

    pub async fn execute(
        &self,
        id: PhotoId,
        exposure: f32,
        contrast: f32,
        temperature: f32,
        tint: f32,
        highlights: f32,
        shadows: f32,
        whites: f32,
        blacks: f32,
        clarity: f32,
        vibrance: f32,
        saturation: f32,
        tone_curve_shadows: f32,
        tone_curve_darks: f32,
        tone_curve_lights: f32,
        tone_curve_highlights: f32,
        hsl_red_sat: f32,
        hsl_orange_sat: f32,
        hsl_yellow_sat: f32,
        hsl_green_sat: f32,
        hsl_aqua_sat: f32,
        hsl_blue_sat: f32,
        hsl_purple_sat: f32,
        hsl_magenta_sat: f32,
        nr_luminance: f32,
        nr_color: f32,
        sharpen_amount: f32,
        sharpen_radius: f32,
    ) -> DomainResult<()> {
        let mut photo = self.photo_repository.find_by_id(&id).await?
            .ok_or(DomainError::PhotoNotFound)?;

        photo.set_edits(
            Some(exposure), Some(contrast), Some(temperature), Some(tint),
            Some(highlights), Some(shadows), Some(whites), Some(blacks),
            Some(clarity), Some(vibrance), Some(saturation),
            Some(tone_curve_shadows), Some(tone_curve_darks),
            Some(tone_curve_lights), Some(tone_curve_highlights),
            Some(hsl_red_sat), Some(hsl_orange_sat), Some(hsl_yellow_sat),
            Some(hsl_green_sat), Some(hsl_aqua_sat), Some(hsl_blue_sat),
            Some(hsl_purple_sat), Some(hsl_magenta_sat),
            Some(nr_luminance), Some(nr_color),
            Some(sharpen_amount), Some(sharpen_radius)
        )?;
        self.photo_repository.update(&photo).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        entities::Photo,
        value_objects::FilePath,
    };
    use mockall::predicate::*;
    use mockall::mock;

    mock! {
        pub PhotoRepository {}
        #[async_trait::async_trait]
        impl PhotoRepository for PhotoRepository {
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
    async fn test_save_edits_success() {
        let mut mock_repo = MockPhotoRepository::new();
        let _photo_id = PhotoId::new();
        
        let id = PhotoId::new();
        let path = FilePath::new("/test.jpg").unwrap();
        let photo = Photo::with_id(id.clone(), path);

        let id_expect = id.clone();
        let photo_clone = photo.clone();
        
        mock_repo.expect_find_by_id()
             .with(eq(id_expect))
             .times(1)
             .returning(move |_| Ok(Some(photo_clone.clone())));

        mock_repo.expect_update()
             .withf(|p| {
                 p.edit_exposure() == Some(1.0) && 
                 p.edit_contrast() == Some(1.2) &&
                 p.edit_tone_curve_shadows() == Some(-30.0) &&
                 p.edit_tone_curve_highlights() == Some(30.0)
             })
             .times(1)
             .returning(|_| Ok(()));

        let use_case = SavePhotoEditsUseCase::new(Arc::new(mock_repo));
        let result = use_case.execute(
            id, 1.0, 1.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            -30.0, -10.0, 10.0, 30.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // HSL
            0.0, 0.0, // NR
            0.0, 1.0  // Sharpening (amount, radius)
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_save_edits_photo_not_found() {
        let mut mock_repo = MockPhotoRepository::new();
        let id = PhotoId::new();

        mock_repo.expect_find_by_id()
             .with(eq(id.clone()))
             .times(1)
             .returning(|_| Ok(None));

        let use_case = SavePhotoEditsUseCase::new(Arc::new(mock_repo));
        let result = use_case.execute(
            id, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // HSL
            0.0, 0.0, // NR
            0.0, 1.0  // Sharpening
        ).await;

        match result {
             Err(DomainError::PhotoNotFound) => (),
             _ => panic!("Expected PhotoNotFound error"),
        }
    }
}
