//! Import with Options Use Case
//!
//! Orquestra a importação paralela de fotos com opções configuráveis.
//! Integra detecção de duplicatas, organização de arquivos, e progress reporting.

use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    services::{MetadataExtractor, ThumbnailGenerator, PreviewStorage, FileOrganizer, PreviewType},
    value_objects::{FilePath, ImportMode, ImportOptions},
    DomainResult, DomainError,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{mpsc, Semaphore};
use crate::check_duplicates::CheckDuplicatesUseCase;

/// Progresso da importação
#[derive(Debug, Clone)]
pub enum ImportProgress {
    Starting { total: usize },
    Processing { index: usize, path: FilePath },
    Completed { photo: Photo },
    Failed { path: FilePath, error: String },
    DuplicateSkipped { path: FilePath, existing: Photo },
    Paused { completed: usize, remaining: usize },
    Finished { successful: usize, failed: usize, skipped: usize },
}

/// Request de importação com opções
pub struct ImportRequest {
    pub files: Vec<FilePath>,
    pub options: ImportOptions,
    pub progress_sender: mpsc::UnboundedSender<ImportProgress>,
    pub pause_flag: Arc<AtomicBool>,
    pub cancel_flag: Arc<AtomicBool>,
}

/// Resultado da importação
#[derive(Debug, Clone, PartialEq)]
pub struct ImportResult {
    pub successful: usize,
    pub failed: usize,
    pub skipped: usize,
    pub imported_photos: Vec<Photo>,
}

/// Use Case para importação com opções avançadas
pub struct ImportWithOptionsUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
    metadata_extractor: Arc<dyn MetadataExtractor>,
    thumbnail_generator: Arc<dyn ThumbnailGenerator>,
    preview_storage: Arc<dyn PreviewStorage>,
    file_organizer: Arc<dyn FileOrganizer>,
    check_duplicates: Arc<CheckDuplicatesUseCase>,
}

impl ImportWithOptionsUseCase {
    /// Cria nova instância do Use Case
    pub fn new(
        photo_repository: Arc<dyn PhotoRepository>,
        metadata_extractor: Arc<dyn MetadataExtractor>,
        thumbnail_generator: Arc<dyn ThumbnailGenerator>,
        preview_storage: Arc<dyn PreviewStorage>,
        file_organizer: Arc<dyn FileOrganizer>,
    ) -> Self {
        let check_duplicates = Arc::new(CheckDuplicatesUseCase::new(photo_repository.clone()));

        Self {
            photo_repository,
            metadata_extractor,
            thumbnail_generator,
            preview_storage,
            file_organizer,
            check_duplicates,
        }
    }

    /// Executa importação com opções configuráveis
    pub async fn execute(&self, request: ImportRequest) -> DomainResult<ImportResult> {
        let ImportRequest {
            files,
            options,
            progress_sender,
            pause_flag,
            cancel_flag,
        } = request;

        if files.is_empty() {
            return Ok(ImportResult {
                successful: 0,
                failed: 0,
                skipped: 0,
                imported_photos: Vec::new(),
            });
        }

        // Enviar progresso inicial
        let _ = progress_sender.send(ImportProgress::Starting { total: files.len() });

        // Fase 1: Detectar duplicatas (se habilitado)
        let duplicates = if options.skip_duplicates {
            self.check_duplicates.execute(files.clone()).await?
        } else {
            Vec::new()
        };

        // Criar semaphore para limitar paralelismo (8 concurrent tasks)
        let semaphore = Arc::new(Semaphore::new(8));

        // Contadores compartilhados
        let successful = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let failed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let skipped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let imported_photos = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        // Criar tasks paralelas
        let mut tasks = Vec::new();

        for (index, file_path) in files.iter().enumerate() {
            let permit = semaphore.clone().acquire_owned().await
                .map_err(|e| DomainError::InfrastructureError(format!("Semaphore error: {}", e)))?;

            let file_path = file_path.clone();
            let options = options.clone();
            let progress_sender = progress_sender.clone();
            let pause_flag = pause_flag.clone();
            let cancel_flag = cancel_flag.clone();

            let photo_repository = self.photo_repository.clone();
            let metadata_extractor = self.metadata_extractor.clone();
            let thumbnail_generator = self.thumbnail_generator.clone();
            let preview_storage = self.preview_storage.clone();
            let file_organizer = self.file_organizer.clone();

            let successful = successful.clone();
            let failed = failed.clone();
            let skipped = skipped.clone();
            let imported_photos = imported_photos.clone();

            // Verificar se é duplicata
            let is_duplicate = duplicates.iter()
                .find(|d| d.file_path == file_path)
                .map(|d| d.is_duplicate)
                .unwrap_or(false);

            let existing_photo = duplicates.iter()
                .find(|d| d.file_path == file_path)
                .and_then(|d| d.existing_photo.clone());

            let task = tokio::spawn(async move {
                // Liberar permit ao final
                let _permit = permit;

                // Check pause flag
                while pause_flag.load(Ordering::Relaxed) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }

                // Check cancel flag
                if cancel_flag.load(Ordering::Relaxed) {
                    return;
                }

                // Enviar progresso
                let _ = progress_sender.send(ImportProgress::Processing {
                    index,
                    path: file_path.clone(),
                });

                // Skip duplicata se configurado
                if is_duplicate && options.skip_duplicates {
                    skipped.fetch_add(1, Ordering::Relaxed);
                    if let Some(existing) = existing_photo {
                        let _ = progress_sender.send(ImportProgress::DuplicateSkipped {
                            path: file_path,
                            existing,
                        });
                    }
                    return;
                }

                // Processar importação
                match Self::import_single_file(
                    &file_path,
                    &options,
                    &*metadata_extractor,
                    &*thumbnail_generator,
                    &*preview_storage,
                    &*file_organizer,
                    &*photo_repository,
                ).await {
                    Ok(photo) => {
                        successful.fetch_add(1, Ordering::Relaxed);
                        let _ = progress_sender.send(ImportProgress::Completed {
                            photo: photo.clone(),
                        });
                        imported_photos.lock().await.push(photo);
                    }
                    Err(e) => {
                        failed.fetch_add(1, Ordering::Relaxed);
                        let _ = progress_sender.send(ImportProgress::Failed {
                            path: file_path,
                            error: e.to_string(),
                        });
                    }
                }
            });

            tasks.push(task);
        }

        // Aguardar todas as tasks
        for task in tasks {
            let _ = task.await;
        }

        let successful_count = successful.load(Ordering::Relaxed);
        let failed_count = failed.load(Ordering::Relaxed);
        let skipped_count = skipped.load(Ordering::Relaxed);
        let photos = imported_photos.lock().await.clone();

        // Enviar progresso final
        let _ = progress_sender.send(ImportProgress::Finished {
            successful: successful_count,
            failed: failed_count,
            skipped: skipped_count,
        });

        Ok(ImportResult {
            successful: successful_count,
            failed: failed_count,
            skipped: skipped_count,
            imported_photos: photos,
        })
    }

    /// Importa um único arquivo
    async fn import_single_file(
        source: &FilePath,
        options: &ImportOptions,
        metadata_extractor: &dyn MetadataExtractor,
        thumbnail_generator: &dyn ThumbnailGenerator,
        preview_storage: &dyn PreviewStorage,
        file_organizer: &dyn FileOrganizer,
        photo_repository: &dyn PhotoRepository,
    ) -> DomainResult<Photo> {
        // 1. Extrair metadados
        let metadata = metadata_extractor.extract(source)?;

        // 2. Colocar o arquivo onde ele vai ficar, conforme o modo escolhido
        //
        // `Add` cataloga onde está: nada é copiado, e o caminho gravado é o original.
        // `Copy` e `Move` passam pelo organizador; `Move` apaga a origem **depois** de
        // tudo ter dado certo, nunca antes — um erro no meio não pode deixar o usuário
        // sem o arquivo e sem o registro.
        let dest_path = match options.mode {
            ImportMode::Add => source.clone(),
            ImportMode::Copy | ImportMode::Move => {
                file_organizer
                    .organize_file_with(source, Some(&metadata), options)
                    .await?
            }
        };

        // 3. Criar entidade Photo
        let mut photo = Photo::new(dest_path.clone());
        photo.set_metadata(metadata);

        // 4. Gerar thumbnails
        let thumbnail_300 = thumbnail_generator.generate(&dest_path, 300).await?;
        let preview_2560 = thumbnail_generator.generate(&dest_path, 2560).await?;

        // 5. Salvar no PreviewStorage
        preview_storage.save(&photo.id(), PreviewType::Thumbnail, &thumbnail_300)?;
        preview_storage.save(&photo.id(), PreviewType::Large, &preview_2560)?;

        // 6. Salvar no banco de dados
        photo_repository.save(&photo).await?;

        // 7. Só agora, com a foto registrada e as previews no lugar, o original pode sair
        if options.mode == ImportMode::Move {
            if let Err(e) = tokio::fs::remove_file(source.as_ref() as &std::path::Path).await {
                // Falhar aqui não invalida a importação: a foto está no catálogo e o arquivo
                // está no destino. O que sobrou foi uma cópia órfã na origem.
                eprintln!(
                    "Importação moveu {} mas não conseguiu apagar o original: {}",
                    source, e
                );
            }
        }

        Ok(photo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        repositories::PhotoRepository,
        services::{MetadataExtractor, ThumbnailGenerator, PreviewStorage, FileOrganizer},
        value_objects::{PhotoId, PhotoMetadata, OrganizationStrategy, RenamePattern},
    };
    use mockall::{mock, predicate::*};
    use tempfile::NamedTempFile;
    use std::io::Write;

    // Mocks
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

    mock! {
        pub MetadataExt {}

        impl MetadataExtractor for MetadataExt {
            fn extract(&self, path: &FilePath) -> DomainResult<PhotoMetadata>;
        }
    }

    mock! {
        pub ThumbnailGen {}

        #[async_trait::async_trait]
        impl ThumbnailGenerator for ThumbnailGen {
            async fn generate(&self, path: &FilePath, max_size: u32) -> DomainResult<Vec<u8>>;
        }
    }

    mock! {
        pub PreviewStore {}

        impl PreviewStorage for PreviewStore {
            fn save(&self, id: &PhotoId, preview_type: PreviewType, data: &[u8]) -> DomainResult<()>;
            fn get(&self, id: &PhotoId, preview_type: PreviewType) -> DomainResult<Option<Vec<u8>>>;
            fn has(&self, id: &PhotoId, preview_type: PreviewType) -> DomainResult<bool>;
            fn delete(&self, id: &PhotoId) -> DomainResult<()>;
        }
    }

    // Note: Can't easily mock FileOrganizer with mockall due to lifetime issues
    // Instead, we'll use a simple implementation that just returns the source path
    struct MockFileOrg;

    #[async_trait::async_trait]
    impl FileOrganizer for MockFileOrg {
        async fn organize_file(
            &self,
            source: &FilePath,
            _metadata: Option<&PhotoMetadata>,
            _strategy: OrganizationStrategy,
            _rename_pattern: RenamePattern,
        ) -> DomainResult<FilePath> {
            Ok(source.clone())
        }
    }

    // Helper para criar arquivo temporário
    fn create_temp_file(content: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content).unwrap();
        file.flush().unwrap();
        file
    }

    #[tokio::test]
    async fn test_import_all_succeed() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        let mut mock_metadata = MockMetadataExt::new();
        let mut mock_thumbnail = MockThumbnailGen::new();
        let mut mock_preview = MockPreviewStore::new();
        let mock_organizer = MockFileOrg;

        // Setup mocks - sem duplicatas (skip_duplicates = false, então não chama find_by_content_hash)
        mock_repo
            .expect_save()
            .times(2)
            .returning(|_| Ok(()));

        mock_metadata
            .expect_extract()
            .times(2)
            .returning(|_| Ok(PhotoMetadata::default()));

        mock_thumbnail
            .expect_generate()
            .times(4) // 2 files × 2 sizes
            .returning(|_, _| Ok(vec![0u8; 100]));

        mock_preview
            .expect_save()
            .times(4) // 2 files × 2 preview types
            .returning(|_, _, _| Ok(()));

        let use_case = ImportWithOptionsUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_metadata),
            Arc::new(mock_thumbnail),
            Arc::new(mock_preview),
            Arc::new(mock_organizer),
        );

        let temp1 = create_temp_file(b"file 1");
        let temp2 = create_temp_file(b"file 2");

        let files = vec![
            FilePath::new(temp1.path().to_str().unwrap()).unwrap(),
            FilePath::new(temp2.path().to_str().unwrap()).unwrap(),
        ];

        let options = ImportOptions {
            organization: OrganizationStrategy::ByDate,
            rename_pattern: RenamePattern::Standard,
            skip_duplicates: false,
            ..ImportOptions::default()
        };

        let (tx, mut rx) = mpsc::unbounded_channel();
        let pause_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let request = ImportRequest {
            files,
            options,
            progress_sender: tx,
            pause_flag,
            cancel_flag,
        };

        // Act
        let result = use_case.execute(request).await;

        // Assert
        assert!(result.is_ok());
        let import_result = result.unwrap();
        assert_eq!(import_result.successful, 2);
        assert_eq!(import_result.failed, 0);
        assert_eq!(import_result.skipped, 0);
        assert_eq!(import_result.imported_photos.len(), 2);

        // Verificar mensagens de progresso
        let mut messages = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }

        assert!(messages.iter().any(|m| matches!(m, ImportProgress::Starting { .. })));
        assert!(messages.iter().any(|m| matches!(m, ImportProgress::Finished { .. })));
    }

    #[tokio::test]
    async fn test_import_with_duplicates_skip() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        let mut mock_metadata = MockMetadataExt::new();
        let mut mock_thumbnail = MockThumbnailGen::new();
        let mut mock_preview = MockPreviewStore::new();
        let mock_organizer = MockFileOrg;

        let fake_photo = Photo::new(FilePath::new("/existing/path.jpg").unwrap());
        let fake_photo_clone = fake_photo.clone();

        // Primeira é duplicata, segunda não (baseado na ordem das chamadas)
        let mut call_count = 0;
        mock_repo
            .expect_find_by_content_hash()
            .times(2)
            .returning(move |_hash: &str| {
                call_count += 1;
                if call_count == 1 {
                    Ok(Some(fake_photo_clone.clone()))
                } else {
                    Ok(None)
                }
            });

        // Apenas 1 save (a não-duplicata)
        mock_repo
            .expect_save()
            .times(1)
            .returning(|_| Ok(()));

        mock_metadata
            .expect_extract()
            .times(1)
            .returning(|_| Ok(PhotoMetadata::default()));

        mock_thumbnail
            .expect_generate()
            .times(2) // 1 file × 2 sizes
            .returning(|_, _| Ok(vec![0u8; 100]));

        mock_preview
            .expect_save()
            .times(2) // 1 file × 2 preview types
            .returning(|_, _, _| Ok(()));

        let use_case = ImportWithOptionsUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_metadata),
            Arc::new(mock_thumbnail),
            Arc::new(mock_preview),
            Arc::new(mock_organizer),
        );

        let temp1 = create_temp_file(b"content 1");
        let temp2 = create_temp_file(b"different content 2");

        let files = vec![
            FilePath::new(temp1.path().to_str().unwrap()).unwrap(),
            FilePath::new(temp2.path().to_str().unwrap()).unwrap(),
        ];

        let options = ImportOptions {
            organization: OrganizationStrategy::ByDate,
            rename_pattern: RenamePattern::Standard,
            skip_duplicates: true, // Habilitar skip
            ..ImportOptions::default()
        };

        let (tx, _rx) = mpsc::unbounded_channel();
        let pause_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let request = ImportRequest {
            files,
            options,
            progress_sender: tx,
            pause_flag,
            cancel_flag,
        };

        // Act
        let result = use_case.execute(request).await;

        // Assert
        assert!(result.is_ok());
        let import_result = result.unwrap();
        assert_eq!(import_result.successful, 1);
        assert_eq!(import_result.failed, 0);
        assert_eq!(import_result.skipped, 1);
    }

    #[tokio::test]
    async fn test_import_empty_list() {
        // Arrange
        let mock_repo = MockPhotoRepo::new();
        let mock_metadata = MockMetadataExt::new();
        let mock_thumbnail = MockThumbnailGen::new();
        let mock_preview = MockPreviewStore::new();
        let mock_organizer = MockFileOrg;

        let use_case = ImportWithOptionsUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_metadata),
            Arc::new(mock_thumbnail),
            Arc::new(mock_preview),
            Arc::new(mock_organizer),
        );

        let options = ImportOptions {
            organization: OrganizationStrategy::ByDate,
            rename_pattern: RenamePattern::Standard,
            skip_duplicates: false,
            ..ImportOptions::default()
        };

        let (tx, _rx) = mpsc::unbounded_channel();
        let pause_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let request = ImportRequest {
            files: Vec::new(),
            options,
            progress_sender: tx,
            pause_flag,
            cancel_flag,
        };

        // Act
        let result = use_case.execute(request).await;

        // Assert
        assert!(result.is_ok());
        let import_result = result.unwrap();
        assert_eq!(import_result.successful, 0);
        assert_eq!(import_result.failed, 0);
        assert_eq!(import_result.skipped, 0);
    }

    #[tokio::test]
    async fn test_import_with_failures() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        let mut mock_metadata = MockMetadataExt::new();
        let mut mock_thumbnail = MockThumbnailGen::new();
        let mut mock_preview = MockPreviewStore::new();
        let mock_organizer = MockFileOrg;

        // Primeira falha, segunda sucede
        let mut call_count = 0;
        mock_metadata
            .expect_extract()
            .times(2)
            .returning(move |_| {
                call_count += 1;
                if call_count == 1 {
                    Err(DomainError::InfrastructureError("Failed to extract metadata".to_string()))
                } else {
                    Ok(PhotoMetadata::default())
                }
            });

        mock_repo
            .expect_save()
            .times(1)
            .returning(|_| Ok(()));

        mock_thumbnail
            .expect_generate()
            .times(2)
            .returning(|_, _| Ok(vec![0u8; 100]));

        mock_preview
            .expect_save()
            .times(2) // 1 file × 2 preview types
            .returning(|_, _, _| Ok(()));

        let use_case = ImportWithOptionsUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_metadata),
            Arc::new(mock_thumbnail),
            Arc::new(mock_preview),
            Arc::new(mock_organizer),
        );

        let temp1 = create_temp_file(b"file 1");
        let temp2 = create_temp_file(b"file 2");

        let files = vec![
            FilePath::new(temp1.path().to_str().unwrap()).unwrap(),
            FilePath::new(temp2.path().to_str().unwrap()).unwrap(),
        ];

        let options = ImportOptions {
            organization: OrganizationStrategy::ByDate,
            rename_pattern: RenamePattern::Standard,
            skip_duplicates: false,
            ..ImportOptions::default()
        };

        let (tx, _rx) = mpsc::unbounded_channel();
        let pause_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let request = ImportRequest {
            files,
            options,
            progress_sender: tx,
            pause_flag,
            cancel_flag,
        };

        // Act
        let result = use_case.execute(request).await;

        // Assert
        assert!(result.is_ok());
        let import_result = result.unwrap();
        assert_eq!(import_result.successful, 1);
        assert_eq!(import_result.failed, 1);
        assert_eq!(import_result.skipped, 0);
    }

    #[tokio::test]
    async fn test_import_progress_reporting() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();
        let mut mock_metadata = MockMetadataExt::new();
        let mut mock_thumbnail = MockThumbnailGen::new();
        let mut mock_preview = MockPreviewStore::new();
        let mock_organizer = MockFileOrg;

        mock_repo
            .expect_find_by_content_hash()
            .returning(|_| Ok(None));

        mock_repo
            .expect_save()
            .returning(|_| Ok(()));

        mock_metadata
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));

        mock_thumbnail
            .expect_generate()
            .returning(|_, _| Ok(vec![0u8; 100]));

        mock_preview
            .expect_save()
            .returning(|_, _, _| Ok(()));

        let use_case = ImportWithOptionsUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_metadata),
            Arc::new(mock_thumbnail),
            Arc::new(mock_preview),
            Arc::new(mock_organizer),
        );

        let temp = create_temp_file(b"file");
        let files = vec![FilePath::new(temp.path().to_str().unwrap()).unwrap()];

        let options = ImportOptions {
            organization: OrganizationStrategy::ByDate,
            rename_pattern: RenamePattern::Standard,
            skip_duplicates: false,
            ..ImportOptions::default()
        };

        let (tx, mut rx) = mpsc::unbounded_channel();
        let pause_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let request = ImportRequest {
            files,
            options,
            progress_sender: tx,
            pause_flag,
            cancel_flag,
        };

        // Act
        let _result = use_case.execute(request).await;

        // Assert - verificar mensagens
        let mut messages = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }

        // Deve ter: Starting, Processing, Completed, Finished
        assert!(messages.iter().any(|m| matches!(m, ImportProgress::Starting { total: 1 })));
        assert!(messages.iter().any(|m| matches!(m, ImportProgress::Processing { .. })));
        assert!(messages.iter().any(|m| matches!(m, ImportProgress::Completed { .. })));
        assert!(messages.iter().any(|m| matches!(m, ImportProgress::Finished { successful: 1, failed: 0, skipped: 0 })));
    }
}
