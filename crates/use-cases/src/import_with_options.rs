//! Import with Options Use Case
//!
//! Orquestra a importação paralela de fotos com opções configuráveis.
//! Integra detecção de duplicatas, organização de arquivos, e progress reporting.

use crate::check_duplicates::CheckDuplicatesUseCase;
use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    services::{FileOrganizer, MetadataExtractor, PreviewStorage, PreviewType, ThumbnailGenerator},
    value_objects::{FilePath, ImportMode, ImportOptions},
    DomainError, DomainResult,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};

/// Progresso da importação
#[derive(Debug, Clone)]
pub enum ImportProgress {
    Starting {
        total: usize,
    },
    Processing {
        index: usize,
        path: FilePath,
    },
    Completed {
        photo: Photo,
    },
    Failed {
        path: FilePath,
        error: String,
    },
    DuplicateSkipped {
        path: FilePath,
        existing: Photo,
    },
    Paused {
        completed: usize,
        remaining: usize,
    },
    Finished {
        successful: usize,
        failed: usize,
        skipped: usize,
    },
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
    /// As vagas de decodificação, **do app inteiro** — ver [`vagas_de_decodificacao`].
    vagas: Arc<Semaphore>,
}

/// Quantas fotos o app decodifica ao mesmo tempo, somando todas as levas.
///
/// Decodificar prévias grandes consome CPU e memória. Reservar ao menos um
/// núcleo para a interface evita que o progresso fique sem repintar.
///
/// 🚨 **Uma conta para o app, e não uma por leva.** Até 26/set/2026 cada
/// `execute` abria o próprio semáforo: duas sessões copiando ao mesmo tempo (a
/// cópia em segundo plano de uma, a importação de outra) decodificavam o dobro.
/// Medido com 5 levas de 50 fotos de 24 MB: 2,4 GB de pico, contra ~0,8 GB de
/// uma leva — num balcão de 8 GB, com a GPU revelando ao lado, é onde o
/// sistema começa a matar processo.
fn vagas_de_decodificacao() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).clamp(1, 4))
        .unwrap_or(2)
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
            vagas: Arc::new(Semaphore::new(vagas_de_decodificacao())),
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
            self.check_duplicates
                .execute(files.clone(), options.sessao_id.as_deref())
                .await?
        } else {
            Vec::new()
        };

        let semaphore = self.vagas.clone();

        // Contadores compartilhados
        let successful = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let failed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let skipped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let imported_photos = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        // Criar tasks paralelas
        let mut tasks = Vec::new();

        for (index, file_path) in files.iter().enumerate() {
            // 🔑 Cancelar para de **abrir** trabalho, e não só de fazê-lo. Sem
            // esta saída, cancelar um lote de 2.000 arquivos ainda criaria as
            // 2.000 tarefas — cada uma para desistir na primeira linha.
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            // Pausada, nem pede vaga: as que ela soltou são das outras levas.
            while pause_flag.load(Ordering::Relaxed) && !cancel_flag.load(Ordering::Relaxed) {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }

            let permit =
                semaphore.clone().acquire_owned().await.map_err(|e| {
                    DomainError::InfrastructureError(format!("Semaphore error: {}", e))
                })?;

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
            let conferida = duplicates.iter().find(|d| d.file_path == file_path);
            let is_duplicate = conferida.is_some_and(|d| d.is_duplicate);
            let existing_photo = conferida.and_then(|d| d.existing_photo.clone());
            // 🔑 O hash da fase 1 vai junto para a foto: é o que a próxima
            // conferência procura, e recalculá-lo seria ler o arquivo de novo.
            let content_hash = conferida
                .map(|d| d.content_hash.clone())
                .filter(|h| !h.is_empty());

            let vagas = semaphore.clone();
            let task = tokio::spawn(async move {
                // Liberar permit ao final
                let mut vaga = Some(permit);

                // 🔑 **Pausada, a leva devolve a vaga.** As vagas são do app
                // inteiro: segurá-las dormindo pararia a cópia das outras
                // sessões junto com esta.
                if pause_flag.load(Ordering::Relaxed) && !cancel_flag.load(Ordering::Relaxed) {
                    vaga = None;
                }

                // 🚨 A espera da pausa também olha o cancelamento.
                //
                // Sem a segunda condição, pausar e **depois** cancelar trava o
                // lote para sempre: as 8 tarefas que seguram as permissões do
                // semáforo dormem em `pause_flag`, o laço de cima nunca ganha
                // permissão, `Finished` nunca é mandado — e a tela fica em
                // "cancelando…" pelo resto da sessão.
                while pause_flag.load(Ordering::Relaxed) && !cancel_flag.load(Ordering::Relaxed) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }

                if cancel_flag.load(Ordering::Relaxed) {
                    return;
                }
                if vaga.is_none() {
                    vaga = vagas.acquire_owned().await.ok();
                }
                let _vaga = vaga;

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
                    content_hash,
                    &options,
                    &*metadata_extractor,
                    &*thumbnail_generator,
                    &*preview_storage,
                    &*file_organizer,
                    &*photo_repository,
                )
                .await
                {
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
    ///
    /// `content_hash` é o SHA-256 que a conferência de duplicatas já calculou —
    /// `None` quando ela não rodou (`skip_duplicates` desligado) ou não
    /// conseguiu ler o arquivo.
    #[allow(clippy::too_many_arguments)]
    async fn import_single_file(
        source: &FilePath,
        content_hash: Option<String>,
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
        // 🚨 **O nome de origem é gravado aqui, e é a única chance.** Com
        // `RenamePattern::Uuid` o `dest_path` já é `<uuid>.jpg`, e daqui para a
        // frente não há de onde tirar `DSC_2571.JPG`: `source` é a última coisa
        // que ainda sabe. Sem esta linha o operador perderia o nome na grade, na
        // tira, no painel, na Revelação e na exportação — e o cliente veria um
        // UUID na galeria dele (migration 022).
        photo.definir_nome_original(
            std::path::Path::new(&source.to_string_lossy().to_string())
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string),
        );
        // 🚨 O ensaio entra **na criação**, e não depois: uma foto que chega ao
        // catálogo sem ensaio não aparece na grade da sessão que a importou, e o
        // sintoma é a importação "não ter funcionado".
        photo.definir_sessao(options.sessao_id.clone());
        // 🚨 **Sem o hash gravado, a conferência de duplicatas não acha nada.**
        // Até 26/set/2026 esta linha não existia: a fase 1 calculava o SHA-256
        // de cada arquivo e procurava num catálogo onde nenhuma foto importada
        // tinha hash — reimportar 40 fotos na mesma sessão dava 80 linhas.
        if let Some(hash) = content_hash {
            photo.set_content_hash(hash);
        }

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
        services::{FileOrganizer, MetadataExtractor, PreviewStorage, ThumbnailGenerator},
        value_objects::{OrganizationStrategy, PhotoId, PhotoMetadata, RenamePattern},
    };
    use mockall::{mock, predicate::*};
    use std::io::Write;
    use tempfile::NamedTempFile;

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
            async fn find_by_content_hash_na_sessao(&self, hash: &str, sessao_id: Option<String>) -> DomainResult<Option<Photo>>;
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
        mock_repo.expect_save().times(2).returning(|_| Ok(()));

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

        assert!(messages
            .iter()
            .any(|m| matches!(m, ImportProgress::Starting { .. })));
        assert!(messages
            .iter()
            .any(|m| matches!(m, ImportProgress::Finished { .. })));
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
            .expect_find_by_content_hash_na_sessao()
            .times(2)
            .returning(move |_hash: &str, _| {
                call_count += 1;
                if call_count == 1 {
                    Ok(Some(fake_photo_clone.clone()))
                } else {
                    Ok(None)
                }
            });

        // Apenas 1 save (a não-duplicata)
        mock_repo.expect_save().times(1).returning(|_| Ok(()));

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
        mock_metadata.expect_extract().times(2).returning(move |_| {
            call_count += 1;
            if call_count == 1 {
                Err(DomainError::InfrastructureError(
                    "Failed to extract metadata".to_string(),
                ))
            } else {
                Ok(PhotoMetadata::default())
            }
        });

        mock_repo.expect_save().times(1).returning(|_| Ok(()));

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

        mock_repo.expect_save().returning(|_| Ok(()));

        mock_metadata
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));

        mock_thumbnail
            .expect_generate()
            .returning(|_, _| Ok(vec![0u8; 100]));

        mock_preview.expect_save().returning(|_, _, _| Ok(()));

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
        assert!(messages
            .iter()
            .any(|m| matches!(m, ImportProgress::Starting { total: 1 })));
        assert!(messages
            .iter()
            .any(|m| matches!(m, ImportProgress::Processing { .. })));
        assert!(messages
            .iter()
            .any(|m| matches!(m, ImportProgress::Completed { .. })));
        assert!(messages.iter().any(|m| matches!(
            m,
            ImportProgress::Finished {
                successful: 1,
                failed: 0,
                skipped: 0
            }
        )));
    }

    /// 🚨 Cancelar **enquanto pausado** tem de terminar o lote.
    ///
    /// Este era o motivo de a pausa e o cancelamento continuarem sem botão: as
    /// tarefas que seguram as 8 permissões do semáforo dormiam em `pause_flag`
    /// sem olhar `cancel_flag`, o laço que abre trabalho novo nunca ganhava
    /// permissão de volta, e `Finished` nunca saía. Sem o `timeout` abaixo,
    /// este teste **não falha: ele pendura**.
    #[tokio::test]
    async fn cancelar_enquanto_pausado_termina_o_lote() {
        // Nenhuma expectativa nos mocks: se qualquer arquivo for processado, o
        // mockall entra em pânico — que é a segunda coisa afirmada aqui.
        let use_case = ImportWithOptionsUseCase::new(
            Arc::new(MockPhotoRepo::new()),
            Arc::new(MockMetadataExt::new()),
            Arc::new(MockThumbnailGen::new()),
            Arc::new(MockPreviewStore::new()),
            Arc::new(MockFileOrg),
        );

        let arquivos: Vec<_> = (0..12).map(|_| create_temp_file(b"raw")).collect();
        let files = arquivos
            .iter()
            .map(|a| FilePath::new(a.path().to_str().unwrap()).unwrap())
            .collect();

        let (tx, _rx) = mpsc::unbounded_channel();
        // Já nasce pausado: as primeiras 8 tarefas vão direto para o laço da espera.
        let pause_flag = Arc::new(AtomicBool::new(true));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let cancelar = cancel_flag.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            cancelar.store(true, Ordering::Relaxed);
        });

        let resultado = tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            use_case.execute(ImportRequest {
                files,
                options: ImportOptions {
                    skip_duplicates: false,
                    ..ImportOptions::default()
                },
                progress_sender: tx,
                pause_flag,
                cancel_flag,
            }),
        )
        .await
        .expect("o lote pausado e depois cancelado tem de terminar, e não pendurar");

        let importacao = resultado.expect("cancelar não é erro");
        assert_eq!(
            importacao.successful, 0,
            "nada entra depois do cancelamento"
        );
    }

    /// Um gerador que conta quantas decodificações correm ao mesmo tempo.
    #[derive(Default)]
    struct GeradorQueConta {
        agora: std::sync::atomic::AtomicUsize,
        pico: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl ThumbnailGenerator for GeradorQueConta {
        async fn generate(&self, _path: &FilePath, _max_size: u32) -> DomainResult<Vec<u8>> {
            let agora = self.agora.fetch_add(1, Ordering::SeqCst) + 1;
            self.pico.fetch_max(agora, Ordering::SeqCst);
            tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
            self.agora.fetch_sub(1, Ordering::SeqCst);
            Ok(vec![0u8; 8])
        }
    }

    /// 🚨 **Duas levas ao mesmo tempo dividem as mesmas vagas.** Cada `execute`
    /// abria o próprio semáforo, e duas sessões copiando decodificavam o dobro
    /// de fotos de 24 MB ao mesmo tempo (5 levas: 2,4 GB de pico).
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn duas_levas_juntas_nao_passam_das_vagas_do_app() {
        let mut repo = MockPhotoRepo::new();
        repo.expect_save().returning(|_| Ok(()));
        let mut metadados = MockMetadataExt::new();
        metadados
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));
        let mut previas = MockPreviewStore::new();
        previas.expect_save().returning(|_, _, _| Ok(()));
        let gerador = Arc::new(GeradorQueConta::default());
        let use_case = Arc::new(ImportWithOptionsUseCase::new(
            Arc::new(repo),
            Arc::new(metadados),
            gerador.clone(),
            Arc::new(previas),
            Arc::new(MockFileOrg),
        ));

        let arquivos: Vec<NamedTempFile> = (0..24).map(|i| create_temp_file(&[i])).collect();
        let leva = |de: usize| {
            let files = arquivos[de..de + 12]
                .iter()
                .map(|a| FilePath::new(a.path().to_str().unwrap()).unwrap())
                .collect();
            let use_case = use_case.clone();
            async move {
                let (tx, _rx) = mpsc::unbounded_channel();
                use_case
                    .execute(ImportRequest {
                        files,
                        options: ImportOptions {
                            mode: ImportMode::Add,
                            skip_duplicates: false,
                            ..ImportOptions::default()
                        },
                        progress_sender: tx,
                        pause_flag: Arc::default(),
                        cancel_flag: Arc::default(),
                    })
                    .await
                    .unwrap()
            }
        };
        let (a, b) = tokio::join!(leva(0), leva(12));
        assert_eq!(a.successful + b.successful, 24);
        let pico = gerador.pico.load(Ordering::SeqCst);
        assert!(
            pico <= vagas_de_decodificacao(),
            "{pico} decodificações juntas, com {} vagas no app",
            vagas_de_decodificacao()
        );
    }
}
