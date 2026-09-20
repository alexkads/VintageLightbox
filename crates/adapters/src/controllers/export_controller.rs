use domain::value_objects::{ExportOptions, PhotoId};
use std::sync::Arc;
use use_cases::ExportPhotoUseCase;

pub struct ExportController {
    export_photo_use_case: Arc<ExportPhotoUseCase>,
}

impl ExportController {
    pub fn new(export_photo_use_case: Arc<ExportPhotoUseCase>) -> Self {
        Self {
            export_photo_use_case,
        }
    }

    pub async fn export_photo(
        &self,
        id: String,
        output_path: String,
        options: &ExportOptions,
    ) -> Result<(), String> {
        let photo_id = PhotoId::from_string(&id).map_err(|e| e.to_string())?;

        self.export_photo_use_case
            .execute(photo_id, output_path, options)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
