use std::sync::Arc;
use domain::value_objects::PhotoId;
use use_cases::SavePhotoEditsUseCase;

pub struct EditorController {
    save_photo_edits_use_case: Arc<SavePhotoEditsUseCase>,
}

impl EditorController {
    pub fn new(save_photo_edits_use_case: Arc<SavePhotoEditsUseCase>) -> Self {
        Self { save_photo_edits_use_case }
    }

    pub async fn save_edits(
        &self,
        id: String,
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
        hsl_red_hue: f32,
        hsl_orange_hue: f32,
        hsl_yellow_hue: f32,
        hsl_green_hue: f32,
        hsl_aqua_hue: f32,
        hsl_blue_hue: f32,
        hsl_purple_hue: f32,
        hsl_magenta_hue: f32,
        hsl_red_lum: f32,
        hsl_orange_lum: f32,
        hsl_yellow_lum: f32,
        hsl_green_lum: f32,
        hsl_aqua_lum: f32,
        hsl_blue_lum: f32,
        hsl_purple_lum: f32,
        hsl_magenta_lum: f32,
        lens_distortion: f32,
        lens_vignette_amount: f32,
        lens_vignette_midpoint: f32,
        nr_luminance: f32,
        nr_color: f32,
        sharpen_amount: f32,
        sharpen_radius: f32,
        crop_x: Option<f32>,
        crop_y: Option<f32>,
        crop_width: Option<f32>,
        crop_height: Option<f32>,
        crop_rotation: Option<i32>,
        crop_angle: Option<f32>,
        crop_flip_h: Option<bool>,
        crop_flip_v: Option<bool>,
        crop_fill_mode: Option<u8>,
    ) -> Result<(), String> {
        let photo_id = PhotoId::from_string(&id).map_err(|e| e.to_string())?;

        self.save_photo_edits_use_case.execute(
            photo_id, exposure, contrast, temperature, tint, highlights, shadows,
            whites, blacks, clarity, vibrance, saturation,
            tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights,
            hsl_red_sat, hsl_orange_sat, hsl_yellow_sat, hsl_green_sat,
            hsl_aqua_sat, hsl_blue_sat, hsl_purple_sat, hsl_magenta_sat,
            hsl_red_hue, hsl_orange_hue, hsl_yellow_hue, hsl_green_hue,
            hsl_aqua_hue, hsl_blue_hue, hsl_purple_hue, hsl_magenta_hue,
            hsl_red_lum, hsl_orange_lum, hsl_yellow_lum, hsl_green_lum,
            hsl_aqua_lum, hsl_blue_lum, hsl_purple_lum, hsl_magenta_lum,
            lens_distortion, lens_vignette_amount, lens_vignette_midpoint,
            nr_luminance, nr_color,

            sharpen_amount, sharpen_radius,
            crop_x,
            crop_y,
            crop_width,
            crop_height,
            crop_rotation,
            crop_angle,
            crop_flip_h,
            crop_flip_v,
            crop_fill_mode
        ).await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        entities::Photo,
        repositories::PhotoRepository,
        value_objects::{PhotoId, FilePath},
        DomainResult,
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
    async fn test_save_edits_success() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        let id_str = "550e8400-e29b-41d4-a716-446655440000";
        let photo_id = PhotoId::from_string(id_str).unwrap();
        
        let path = FilePath::new("/photos/test.jpg").unwrap();
        let photo = Photo::with_id(photo_id.clone(), path);
        let photo_clone = photo.clone();

        mock_repo
            .expect_find_by_id()
            .with(eq(photo_id))
            .returning(move |_| Ok(Some(photo_clone.clone())));

        mock_repo
            .expect_update()
            .withf(|p| {
                // Verify some key edits
                p.edit_exposure() == Some(1.5) &&
                p.edit_contrast() == Some(1.2) &&
                p.edit_crop_rotation() == Some(90)
            })
            .times(1)
            .returning(|_| Ok(()));

        let use_case = Arc::new(SavePhotoEditsUseCase::new(Arc::new(mock_repo)));
        let controller = EditorController::new(use_case);

        // Act
        let result = controller.save_edits(
            id_str.to_string(),
            1.5, 1.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // Basic
            0.0, 0.0, 0.0, 0.0, // Tone Curve
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // HSL Sat
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // HSL Hue
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, // HSL Lum
            0.0, 0.0, 0.0, // Lens
            0.0, 0.0, // NR
            0.0, 0.0, // Sharpen
            None, None, None, None, // Crop Rect
            Some(90), None, // Rotation
            None, None, // Flip
            None // Fill Mode
        ).await;

        // Assert
        assert!(result.is_ok());
    }
}
