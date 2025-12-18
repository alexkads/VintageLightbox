use use_cases::ImportPhotoUseCase;
use domain::value_objects::FilePath;
use std::sync::Arc;

pub struct ImportController {
    import_photo_use_case: Arc<ImportPhotoUseCase>,
    // import_photos_use_case: Arc<ImportPhotosUseCase>, // Future use
}

impl ImportController {
    pub fn new(import_photo_use_case: Arc<ImportPhotoUseCase>) -> Self {
        Self {
            import_photo_use_case,
        }
    }

    pub async fn import_files(&self, files: Vec<String>) -> Result<(), String> {
        for file in files {
            match FilePath::new(&file) {
                Ok(path) => {
                    if let Err(e) = self.import_photo_use_case.execute(path).await {
                        eprintln!("Error importing file {}: {}", file, e);
                        // In a real app, we might collect errors or emit events
                    } else {
                        println!("Successfully imported: {}", file);
                    }
                }
                Err(e) => eprintln!("Invalid file path {}: {}", file, e),
            }
        }
        Ok(())
    }
}
