use use_cases::{
    ImportPhotoUseCase,
    PreviewBeforeImportUseCase,
    CheckDuplicatesUseCase,
    ImportWithOptionsUseCase,
    ImportRequest,
    ImportProgress,
};
use domain::value_objects::{FilePath, ImportOptions};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::mpsc;
use crate::view_models::{ImportPreviewItemViewModel, DuplicateCheckViewModel, ImportProgressViewModel};

pub struct ImportController {
    import_photo_use_case: Arc<ImportPhotoUseCase>,
    preview_use_case: Arc<PreviewBeforeImportUseCase>,
    check_duplicates_use_case: Arc<CheckDuplicatesUseCase>,
    import_with_options_use_case: Arc<ImportWithOptionsUseCase>,
    get_import_sources_use_case: Arc<use_cases::GetImportSourcesUseCase>,
}

impl ImportController {
    pub fn new(
        import_photo_use_case: Arc<ImportPhotoUseCase>,
        preview_use_case: Arc<PreviewBeforeImportUseCase>,
        check_duplicates_use_case: Arc<CheckDuplicatesUseCase>,
        import_with_options_use_case: Arc<ImportWithOptionsUseCase>,
        get_import_sources_use_case: Arc<use_cases::GetImportSourcesUseCase>,
    ) -> Self {
        Self {
            import_photo_use_case,
            preview_use_case,
            check_duplicates_use_case,
            import_with_options_use_case,
            get_import_sources_use_case,
        }
    }

    /// Simple import (legacy method)
    pub async fn import_files(&self, files: Vec<String>) -> Result<(), String> {
        for file in files {
            match FilePath::new(&file) {
                Ok(path) => {
                    if let Err(e) = self.import_photo_use_case.execute(path).await {
                        eprintln!("Error importing file {}: {}", file, e);
                    } else {
                        println!("Successfully imported: {}", file);
                    }
                }
                Err(e) => eprintln!("Invalid file path {}: {}", file, e),
            }
        }
        Ok(())
    }

    /// Preview files before import
    pub async fn preview_import(&self, files: Vec<String>) -> Result<Vec<ImportPreviewItemViewModel>, String> {
        // Convert strings to FilePaths
        let file_paths: Result<Vec<FilePath>, _> = files
            .iter()
            .map(|f| FilePath::new(f))
            .collect();

        let file_paths = file_paths.map_err(|e| format!("Invalid file path: {}", e))?;

        // Execute preview use case
        let previews = self.preview_use_case.execute(file_paths)
            .await
            .map_err(|e| format!("Preview failed: {}", e))?;

        // Convert to view models
        Ok(previews.into_iter().map(|item| {
            let camera = match (&item.metadata.camera_make, &item.metadata.camera_model) {
                (Some(make), Some(model)) => format!("{} {}", make, model),
                (Some(make), None) => make.clone(),
                (None, Some(model)) => model.clone(),
                (None, None) => "Unknown".to_string(),
            };

            let dimensions = match (item.metadata.width, item.metadata.height) {
                (Some(w), Some(h)) => Some(format!("{}x{}", w, h)),
                _ => None,
            };

            ImportPreviewItemViewModel {
                file_path: item.file_path.to_string(),
                thumbnail_data: item.thumbnail,
                file_size: item.file_size,
                is_raw: item.is_raw,
                camera,
                date_time: item.metadata.date_time.unwrap_or_else(|| "Unknown".to_string()),
                dimensions,
            }
        }).collect())
    }

    /// Check for duplicate files
    pub async fn check_duplicates(&self, files: Vec<String>) -> Result<Vec<DuplicateCheckViewModel>, String> {
        // Convert strings to FilePaths
        let file_paths: Result<Vec<FilePath>, _> = files
            .iter()
            .map(|f| FilePath::new(f))
            .collect();

        let file_paths = file_paths.map_err(|e| format!("Invalid file path: {}", e))?;

        // Execute duplicate check
        let results = self.check_duplicates_use_case.execute(file_paths)
            .await
            .map_err(|e| format!("Duplicate check failed: {}", e))?;

        // Convert to view models
        Ok(results.into_iter().map(|result| DuplicateCheckViewModel {
            file_path: result.file_path.to_string(),
            content_hash: result.content_hash,
            is_duplicate: result.is_duplicate,
            existing_photo_path: result.existing_photo.map(|p| p.file_path().to_string()),
        }).collect())
    }

    /// Import with advanced options
    pub async fn import_with_options(
        &self,
        files: Vec<String>,
        options: ImportOptions,
        progress_sender: mpsc::UnboundedSender<ImportProgressViewModel>,
        pause_flag: Arc<AtomicBool>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<String, String> {
        // Convert strings to FilePaths
        let file_paths: Result<Vec<FilePath>, _> = files
            .iter()
            .map(|f| FilePath::new(f))
            .collect();

        let file_paths = file_paths.map_err(|e| format!("Invalid file path: {}", e))?;

        // Create channel for domain progress events
        let (domain_tx, mut domain_rx) = mpsc::unbounded_channel::<ImportProgress>();

        // Spawn task to convert domain events to view model events
        let view_sender = progress_sender.clone();
        tokio::spawn(async move {
            while let Some(progress) = domain_rx.recv().await {
                let vm = match progress {
                    ImportProgress::Starting { total } => {
                        ImportProgressViewModel::Starting { total }
                    }
                    ImportProgress::Processing { index, path } => {
                        ImportProgressViewModel::Processing {
                            index,
                            path: path.to_string()
                        }
                    }
                    ImportProgress::Completed { photo } => {
                        ImportProgressViewModel::Completed {
                            photo_id: photo.id().to_string(),
                            path: photo.file_path().to_string(),
                        }
                    }
                    ImportProgress::Failed { path, error } => {
                        ImportProgressViewModel::Failed {
                            path: path.to_string(),
                            error
                        }
                    }
                    ImportProgress::DuplicateSkipped { path, existing } => {
                        ImportProgressViewModel::DuplicateSkipped {
                            path: path.to_string(),
                            existing_path: existing.file_path().to_string(),
                        }
                    }
                    ImportProgress::Paused { completed, remaining } => {
                        ImportProgressViewModel::Paused { completed, remaining }
                    }
                    ImportProgress::Finished { successful, failed, skipped } => {
                        ImportProgressViewModel::Finished { successful, failed, skipped }
                    }
                };

                let _ = view_sender.send(vm);
            }
        });

        // Create request
        let request = ImportRequest {
            files: file_paths,
            options,
            progress_sender: domain_tx,
            pause_flag,
            cancel_flag,
        };

        // Execute import
        let result = self.import_with_options_use_case.execute(request)
            .await
            .map_err(|e| format!("Import failed: {}", e))?;

        Ok(format!(
            "Import complete: {} successful, {} failed, {} skipped",
            result.successful, result.failed, result.skipped
        ))
    }

    pub async fn get_sources(&self) -> (Vec<domain::import_source::ImportSource>, Vec<domain::import_source::ImportSource>) {
        self.get_import_sources_use_case.execute().await
    }
}
