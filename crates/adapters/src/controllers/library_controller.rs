use std::sync::Arc;
use domain::repositories::PhotoRepository;
use crate::view_models::PhotoViewModel;

pub struct LibraryController {
    photo_repository: Arc<dyn PhotoRepository>,
}

impl LibraryController {
    pub fn new(photo_repository: Arc<dyn PhotoRepository>) -> Self {
        Self { photo_repository }
    }

    pub async fn get_all_photos(&self) -> Result<Vec<PhotoViewModel>, String> {
        let photos = self.photo_repository.find_all().await
            .map_err(|e| e.to_string())?;

        let view_models = photos.into_iter().map(|photo| {
                    let metadata = photo.metadata();
                    let date = metadata.and_then(|m| m.date_time.clone()).unwrap_or_default();
                    let camera = metadata.and_then(|m| m.camera_model.clone()).unwrap_or_default();
                    let exposure = if let Some(meta) = metadata {
                        format!("ISO {} {} {}", 
                            meta.iso.unwrap_or(0), 
                            meta.aperture.map(|f| format!("f/{:.1}", f)).unwrap_or_default(), 
                            meta.shutter_speed.clone().unwrap_or_default()
                        ).trim().to_string()
                    } else {
                        String::new()
                    };
                    let rating = photo.rating().map(|r| r.value() as i32).unwrap_or(0);

                    PhotoViewModel {
                        id: photo.id().to_string(),
                        name: photo.file_name().unwrap_or_default().to_string(),
                        path: photo.file_path().to_string(),
                        thumbnail_path: photo.thumbnail_path().map(|p| p.to_string()),
                        date,
                        camera,
                        exposure,
                        rating,
                        color_label: photo.color_label().map(|c| c.to_string()),
                        flag: photo.flag().map(|f| f.as_code()),
                        width: metadata.and_then(|m| m.width),
                        height: metadata.and_then(|m| m.height),
                        edit_exposure: photo.edit_exposure(),
                        edit_contrast: photo.edit_contrast(),
                        edit_temperature: photo.edit_temperature(),
                        edit_tint: photo.edit_tint(),
                        edit_highlights: photo.edit_highlights(),
                        edit_shadows: photo.edit_shadows(),
                        edit_whites: photo.edit_whites(),
                        edit_blacks: photo.edit_blacks(),
                        edit_clarity: photo.edit_clarity(),
                        edit_vibrance: photo.edit_vibrance(),
                        edit_saturation: photo.edit_saturation(),
                        edit_tone_curve_shadows: photo.edit_tone_curve_shadows(),
                        edit_tone_curve_darks: photo.edit_tone_curve_darks(),
                        edit_tone_curve_lights: photo.edit_tone_curve_lights(),
                        edit_tone_curve_highlights: photo.edit_tone_curve_highlights(),
                        // HSL Saturation
                        edit_hsl_red_sat: photo.edit_hsl_red_sat(),
                        edit_hsl_orange_sat: photo.edit_hsl_orange_sat(),
                        edit_hsl_yellow_sat: photo.edit_hsl_yellow_sat(),
                        edit_hsl_green_sat: photo.edit_hsl_green_sat(),
                        edit_hsl_aqua_sat: photo.edit_hsl_aqua_sat(),
                        edit_hsl_blue_sat: photo.edit_hsl_blue_sat(),
                        edit_hsl_purple_sat: photo.edit_hsl_purple_sat(),
                        edit_hsl_magenta_sat: photo.edit_hsl_magenta_sat(),
                        // HSL Hue
                        edit_hsl_red_hue: photo.edit_hsl_red_hue(),
                        edit_hsl_orange_hue: photo.edit_hsl_orange_hue(),
                        edit_hsl_yellow_hue: photo.edit_hsl_yellow_hue(),
                        edit_hsl_green_hue: photo.edit_hsl_green_hue(),
                        edit_hsl_aqua_hue: photo.edit_hsl_aqua_hue(),
                        edit_hsl_blue_hue: photo.edit_hsl_blue_hue(),
                        edit_hsl_purple_hue: photo.edit_hsl_purple_hue(),
                        edit_hsl_magenta_hue: photo.edit_hsl_magenta_hue(),
                        // HSL Lum
                        edit_hsl_red_lum: photo.edit_hsl_red_lum(),
                        edit_hsl_orange_lum: photo.edit_hsl_orange_lum(),
                        edit_hsl_yellow_lum: photo.edit_hsl_yellow_lum(),
                        edit_hsl_green_lum: photo.edit_hsl_green_lum(),
                        edit_hsl_aqua_lum: photo.edit_hsl_aqua_lum(),
                        edit_hsl_blue_lum: photo.edit_hsl_blue_lum(),
                        edit_hsl_purple_lum: photo.edit_hsl_purple_lum(),
                        edit_hsl_magenta_lum: photo.edit_hsl_magenta_lum(),
                        // Lens
                        edit_lens_distortion: photo.edit_lens_distortion(),
                        edit_lens_vignette_amount: photo.edit_lens_vignette_amount(),
                        edit_lens_vignette_midpoint: photo.edit_lens_vignette_midpoint(),
                        // NR
                        edit_nr_luminance: photo.edit_nr_luminance(),
                        edit_nr_color: photo.edit_nr_color(),
                        // Sharpening
                        edit_sharpen_amount: photo.edit_sharpen_amount(),
                        edit_sharpen_radius: photo.edit_sharpen_radius(),
                        // Crop & Rotation
                        edit_crop_x: photo.edit_crop_x(),
                        edit_crop_y: photo.edit_crop_y(),
                        edit_crop_width: photo.edit_crop_width(),
                        edit_crop_height: photo.edit_crop_height(),
                        edit_crop_rotation: photo.edit_crop_rotation(),
                        edit_crop_angle: photo.edit_crop_angle(),
                        edit_crop_flip_h: photo.edit_crop_flip_h(),
                        edit_crop_flip_v: photo.edit_crop_flip_v(),
                        edit_crop_fill_mode: photo.edit_crop_fill_mode(),
                        file_missing: !std::path::Path::new(&photo.file_path().to_string()).exists(),
                    }
                })
                .collect();

        Ok(view_models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        entities::Photo,
        value_objects::{PhotoId, FilePath, Rating, ColorLabel},
        DomainResult, DomainError,
    };
    use mockall::mock;
    use mockall::predicate::*;

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
    async fn test_get_all_photos_success() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        let path1 = FilePath::new("/photos/1.jpg").unwrap();
        let mut photo1 = Photo::new(path1.clone());
        photo1.rate(Rating::new(5).unwrap()).unwrap();
        let id1 = photo1.id(); // Capture the actual ID
        
        let path2 = FilePath::new("/photos/2.jpg").unwrap();
        let mut photo2 = Photo::new(path2.clone());
        photo2.set_color_label(ColorLabel::Red);
        let id2 = photo2.id(); // Capture the actual ID

        // A mock implementation of repository returning 2 photos
        mock_repo
            .expect_find_all()
            .times(1)
            .returning(move || Ok(vec![photo1.clone(), photo2.clone()]));

        let controller = LibraryController::new(Arc::new(mock_repo));

        // Act
        let result = controller.get_all_photos().await;

        // Assert
        assert!(result.is_ok());
        let view_models = result.unwrap();
        assert_eq!(view_models.len(), 2);
        
        // Verificações básicas de mapeamento
        let vm1 = view_models.iter().find(|vm| vm.id == id1.to_string());
        assert!(vm1.is_some());
        assert_eq!(vm1.unwrap().rating, 5);

        let vm2 = view_models.iter().find(|vm| vm.id == id2.to_string());
        assert!(vm2.is_some());
        assert_eq!(vm2.unwrap().color_label, Some("red".to_string()));
    }

    #[tokio::test]
    async fn test_get_all_photos_empty() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        mock_repo
            .expect_find_all()
            .times(1)
            .returning(|| Ok(Vec::new()));

        let controller = LibraryController::new(Arc::new(mock_repo));

        // Act
        let result = controller.get_all_photos().await;

        // Assert
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_get_all_photos_error() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        
        mock_repo
            .expect_find_all()
            .times(1)
            .returning(|| Err(DomainError::InfrastructureError("DB Error".to_string())));

        let controller = LibraryController::new(Arc::new(mock_repo));

        // Act
        let result = controller.get_all_photos().await;

        // Assert
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Erro de infraestrutura: DB Error");
    }
}
