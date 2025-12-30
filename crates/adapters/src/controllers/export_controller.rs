use std::sync::Arc;
use use_cases::ExportPhotoUseCase;
use domain::{value_objects::PhotoId, ports::system_gateway::SystemGateway};

pub struct ExportController {
    export_photo_use_case: Arc<ExportPhotoUseCase>,
    system_gateway: Arc<dyn SystemGateway>,
}

impl ExportController {
    pub fn new(
        export_photo_use_case: Arc<ExportPhotoUseCase>,
        system_gateway: Arc<dyn SystemGateway>,
    ) -> Self {
        Self {
            export_photo_use_case,
            system_gateway,
        }
    }

    pub async fn export_photo(&self, id: String, output_path: String) -> Result<(), String> {
        let photo_id = PhotoId::from_string(&id).map_err(|e| e.to_string())?;

        self.export_photo_use_case
            .execute(photo_id, output_path)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn open_export_location(&self, path: String) -> Result<(), String> {
        // Extract parent directory to open
        let path_obj = std::path::Path::new(&path);
        let parent = path_obj.parent()
            .ok_or_else(|| "Invalid path".to_string())?;
        
        self.system_gateway.open_path(&parent.to_string_lossy()).await
    }
}
