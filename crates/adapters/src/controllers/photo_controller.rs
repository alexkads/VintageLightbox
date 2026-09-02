use domain::value_objects::{ColorLabel, Flag, PhotoId, Rating};
use std::sync::Arc;
use use_cases::{
    DeletePhotoUseCase, MarcarCompradaUseCase, RatePhotoUseCase, SetColorLabelUseCase,
    SetFlagUseCase,
};

/// Controller for photo operations (rating, color labels, etc.)
pub struct PhotoController {
    rate_photo_use_case: Arc<RatePhotoUseCase>,
    set_color_label_use_case: Arc<SetColorLabelUseCase>,
    set_flag_use_case: Arc<SetFlagUseCase>,
    delete_photo_use_case: Arc<DeletePhotoUseCase>,
    marcar_comprada_use_case: Arc<MarcarCompradaUseCase>,
}

impl PhotoController {
    pub fn new(
        rate_photo_use_case: Arc<RatePhotoUseCase>,
        set_color_label_use_case: Arc<SetColorLabelUseCase>,
        set_flag_use_case: Arc<SetFlagUseCase>,
        delete_photo_use_case: Arc<DeletePhotoUseCase>,
        marcar_comprada_use_case: Arc<MarcarCompradaUseCase>,
    ) -> Self {
        Self {
            rate_photo_use_case,
            set_color_label_use_case,
            set_flag_use_case,
            delete_photo_use_case,
            marcar_comprada_use_case,
        }
    }

    /// Levada no balcão (`true`) ou deixada para trás (`false`).
    pub async fn set_comprada(&self, photo_id: &str, comprada: bool) -> Result<(), String> {
        let photo_id =
            PhotoId::from_string(photo_id).map_err(|e| format!("Invalid photo ID: {}", e))?;
        self.marcar_comprada_use_case
            .execute(photo_id, comprada)
            .await
            .map_err(|e| format!("Failed to mark as purchased: {}", e))?;
        Ok(())
    }

    /// Rate a photo with the given rating (0-5)
    pub async fn rate_photo(&self, photo_id: &str, rating: i32) -> Result<(), String> {
        // Convert string ID to PhotoId
        let photo_id =
            PhotoId::from_string(photo_id).map_err(|e| format!("Invalid photo ID: {}", e))?;

        // Execute appropriate use case method
        if rating == 0 {
            // Remove rating
            self.rate_photo_use_case
                .unrate(photo_id)
                .await
                .map_err(|e| format!("Failed to remove rating: {}", e))?;
        } else {
            // Set rating (1-5)
            let rating_value =
                Rating::new(rating as u8).map_err(|e| format!("Invalid rating: {}", e))?;

            self.rate_photo_use_case
                .execute(photo_id, rating_value)
                .await
                .map_err(|e| format!("Failed to rate photo: {}", e))?;
        }

        Ok(())
    }

    /// Set color label on a photo
    pub async fn set_color_label(&self, photo_id: &str, label: &str) -> Result<(), String> {
        // Convert string ID to PhotoId
        let photo_id =
            PhotoId::from_string(photo_id).map_err(|e| format!("Invalid photo ID: {}", e))?;

        // Execute appropriate use case method
        if label.is_empty() || label.eq_ignore_ascii_case("none") {
            // Remove color label
            self.set_color_label_use_case
                .clear(photo_id)
                .await
                .map_err(|e| format!("Failed to clear color label: {}", e))?;
        } else {
            // Set color label
            let color_label = ColorLabel::from_name(label)
                .map_err(|e| format!("Invalid color label '{}': {}", label, e))?;

            self.set_color_label_use_case
                .execute(photo_id, color_label)
                .await
                .map_err(|e| format!("Failed to set color label: {}", e))?;
        }

        Ok(())
    }

    /// Delete a photo from the catalog
    pub async fn delete_photo(&self, photo_id: &str) -> Result<(), String> {
        // Convert string ID to PhotoId
        let photo_id =
            PhotoId::from_string(photo_id).map_err(|e| format!("Invalid photo ID: {}", e))?;

        // Execute delete use case
        self.delete_photo_use_case
            .execute(photo_id)
            .await
            .map_err(|e| format!("Failed to delete photo: {}", e))?;

        Ok(())
    }

    /// Set flag on a photo
    pub async fn set_flag(&self, photo_id: &str, flag_code: i32) -> Result<(), String> {
        // Convert string ID to PhotoId
        let photo_id =
            PhotoId::from_string(photo_id).map_err(|e| format!("Invalid photo ID: {}", e))?;

        // Execute appropriate use case method
        if flag_code == 0 {
            // Remove flag
            self.set_flag_use_case
                .remove_flag(photo_id)
                .await
                .map_err(|e| format!("Failed to remove flag: {}", e))?;
        } else {
            // Set flag
            let flag = Flag::from_code(flag_code)
                .ok_or_else(|| format!("Invalid flag code '{}'", flag_code))?;

            self.set_flag_use_case
                .execute(photo_id, flag)
                .await
                .map_err(|e| format!("Failed to set flag: {}", e))?;
        }

        Ok(())
    }
}

// TODO: Add unit tests once mockall lifetime issues are resolved
// For now, relying on integration tests and manual verification
