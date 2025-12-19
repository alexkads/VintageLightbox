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

    pub async fn save_edits(&self, id: String, exposure: f32, contrast: f32) -> Result<(), String> {
        let photo_id = PhotoId::from_string(&id).map_err(|e| e.to_string())?;
        
        self.save_photo_edits_use_case.execute(photo_id, exposure, contrast).await
            .map_err(|e| e.to_string())
    }
}
