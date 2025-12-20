use domain::value_objects::{PhotoId, ColorLabel, Rating};
use use_cases::{RatePhotoUseCase, SetColorLabelUseCase};
use std::sync::Arc;

/// Controller for photo operations (rating, color labels, etc.)
pub struct PhotoController {
    rate_photo_use_case: Arc<RatePhotoUseCase>,
    set_color_label_use_case: Arc<SetColorLabelUseCase>,
}

impl PhotoController {
    pub fn new(
        rate_photo_use_case: Arc<RatePhotoUseCase>,
        set_color_label_use_case: Arc<SetColorLabelUseCase>,
    ) -> Self {
        Self {
            rate_photo_use_case,
            set_color_label_use_case,
        }
    }

    /// Rate a photo with the given rating (0-5)
    pub async fn rate_photo(&self, photo_id: &str, rating: i32) -> Result<(), String> {
        // Convert string ID to PhotoId
        let photo_id = PhotoId::from_string(photo_id)
            .map_err(|e| format!("Invalid photo ID: {}", e))?;

        // Execute appropriate use case method
        if rating == 0 {
            // Remove rating
            self.rate_photo_use_case
                .unrate(photo_id)
                .await
                .map_err(|e| format!("Failed to remove rating: {}", e))?;
        } else {
            // Set rating (1-5)
            let rating_value = Rating::new(rating as u8)
                .map_err(|e| format!("Invalid rating: {}", e))?;
            
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
        let photo_id = PhotoId::from_string(photo_id)
            .map_err(|e| format!("Invalid photo ID: {}", e))?;

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
}

// TODO: Add unit tests once mockall lifetime issues are resolved
// For now, relying on integration tests and manual verification
