//! Editing Orchestrator E2E Tests
//!
//! Testes de integração que verificam o contrato completo do EditingOrchestrator
//! usando componentes reais (banco de dados SQLite, PreviewManager).
//!
//! ## Garantias Testadas
//! 
//! 1. Após apply_edits(), as edições DEVEM estar no banco
//! 2. Após apply_edits(), o cache de preview DEVE estar invalidado
//! 3. Observers DEVEM ser notificados em ordem correta
//! 4. preview_edits() NÃO DEVE salvar no banco
//! 5. reset_edits() DEVE retornar valores default

use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use async_trait::async_trait;
use domain::{
    entities::Photo,
    ports::{EditingOrchestrator, EditObserver, EditEvent},
    repositories::PhotoRepository,
    services::{PreviewStorage, PreviewType},
    value_objects::{FilePath, PhotoEdits, PhotoId},
};
use infrastructure::{
    create_pool, run_migrations,
    PhotoRepositoryImpl,
    SqliteUnitOfWork,
    cache::preview_manager::PreviewManager,
    services::EditingOrchestratorImpl,
};
use tempfile::tempdir;
use tokio::sync::Mutex;

/// Observer que registra todos os eventos recebidos
struct TestObserver {
    events: Mutex<Vec<EditEvent>>,
    edits_applied_count: AtomicUsize,
    preview_invalidated_count: AtomicUsize,
    thumbnail_invalidated_count: AtomicUsize,
}

impl TestObserver {
    fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            edits_applied_count: AtomicUsize::new(0),
            preview_invalidated_count: AtomicUsize::new(0),
            thumbnail_invalidated_count: AtomicUsize::new(0),
        }
    }

    async fn events(&self) -> Vec<EditEvent> {
        self.events.lock().await.clone()
    }
}

#[async_trait]
impl EditObserver for TestObserver {
    async fn on_edit_event(&self, event: EditEvent) {
        match &event {
            EditEvent::EditsApplied { .. } => {
                self.edits_applied_count.fetch_add(1, Ordering::SeqCst);
            }
            EditEvent::PreviewInvalidated { .. } => {
                self.preview_invalidated_count.fetch_add(1, Ordering::SeqCst);
            }
            EditEvent::ThumbnailInvalidated { .. } => {
                self.thumbnail_invalidated_count.fetch_add(1, Ordering::SeqCst);
            }
            EditEvent::EditsReset { .. } => {}
        }
        self.events.lock().await.push(event);
    }
}

/// Cria uma estrutura de teste com componentes reais
async fn setup_test_environment() -> (
    Arc<EditingOrchestratorImpl<PreviewManager>>,
    Arc<PhotoRepositoryImpl>,
    Arc<PreviewManager>,
    tempfile::TempDir,
) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    
    // Criar banco de dados SQLite em memória
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = create_pool(&db_url).await.expect("Failed to create pool");
    run_migrations(&pool).await.expect("Failed to run migrations");
    
    let photo_repository = Arc::new(PhotoRepositoryImpl::new(pool.clone()));
    let unit_of_work = Arc::new(SqliteUnitOfWork::new(pool));
    
    // Criar PreviewManager com diretório temporário
    let preview_dir = temp_dir.path().join("previews");
    std::fs::create_dir_all(&preview_dir).unwrap();
    let preview_manager = Arc::new(PreviewManager::new_with_path(preview_dir));
    
    let orchestrator = Arc::new(EditingOrchestratorImpl::new(
        photo_repository.clone(),
        preview_manager.clone(),
        unit_of_work,
    ));
    
    (orchestrator, photo_repository, preview_manager, temp_dir)
}

/// Cria uma foto de teste e salva no repositório
async fn create_and_save_test_photo(
    repo: &PhotoRepositoryImpl,
    file_path: &str,
) -> Photo {
    let photo = Photo::new(FilePath::new(file_path).unwrap());
    repo.save(&photo).await.expect("Failed to save photo");
    photo
}

#[tokio::test]
async fn test_apply_edits_persists_to_database() {
    let (orchestrator, repo, _, _temp_dir) = setup_test_environment().await;
    
    let photo = create_and_save_test_photo(&repo, "/test/photo1.jpg").await;
    let photo_id = photo.id().clone();
    
    // Aplicar edições
    let edits = PhotoEdits {
        exposure: 1.5,
        contrast: 1.2,
        temperature: -10.0,
        ..PhotoEdits::default()
    };
    
    orchestrator.apply_edits(&photo_id, edits.clone()).await
        .expect("apply_edits should succeed");
    
    // Verificar que as edições foram salvas no banco
    let saved_photo = repo.find_by_id(&photo_id).await
        .expect("find_by_id should succeed")
        .expect("Photo should exist");
    
    let saved_edits = saved_photo.get_photo_edits();
    assert_eq!(saved_edits.exposure, 1.5, "Exposure should be saved");
    assert_eq!(saved_edits.contrast, 1.2, "Contrast should be saved");
    assert_eq!(saved_edits.temperature, -10.0, "Temperature should be saved");
}

#[tokio::test]
async fn test_apply_edits_invalidates_preview_cache() {
    let (orchestrator, repo, preview_manager, _temp_dir) = setup_test_environment().await;
    
    let photo = create_and_save_test_photo(&repo, "/test/photo2.jpg").await;
    let photo_id = photo.id().clone();
    
    // Criar um preview fake no cache
    let fake_preview = vec![0u8; 100];
    preview_manager.save(&photo_id, PreviewType::Large, &fake_preview)
        .expect("save preview should succeed");
    preview_manager.save(&photo_id, PreviewType::Thumbnail, &fake_preview)
        .expect("save thumbnail should succeed");
    
    // Verificar que existe no cache
    assert!(preview_manager.has(&photo_id, PreviewType::Large).unwrap());
    assert!(preview_manager.has(&photo_id, PreviewType::Thumbnail).unwrap());
    
    // Aplicar edições
    let edits = PhotoEdits {
        exposure: 0.5,
        ..PhotoEdits::default()
    };
    
    let result = orchestrator.apply_edits(&photo_id, edits).await
        .expect("apply_edits should succeed");
    
    // Verificar que o cache foi invalidado
    assert!(result.preview_invalidated, "Preview should be invalidated");
    assert!(result.thumbnail_invalidated, "Thumbnail should be invalidated");
    
    // Verificar que não existe mais no cache
    assert!(!preview_manager.has(&photo_id, PreviewType::Large).unwrap());
    assert!(!preview_manager.has(&photo_id, PreviewType::Thumbnail).unwrap());
}

#[tokio::test]
async fn test_observers_receive_correct_events() {
    let (orchestrator, repo, _, _temp_dir) = setup_test_environment().await;
    
    let photo = create_and_save_test_photo(&repo, "/test/photo3.jpg").await;
    let photo_id = photo.id().clone();
    
    // Registrar observer
    let observer = Arc::new(TestObserver::new());
    orchestrator.register_observer(observer.clone());
    
    // Aplicar edições
    let edits = PhotoEdits {
        exposure: 0.5,
        ..PhotoEdits::default()
    };
    
    orchestrator.apply_edits(&photo_id, edits).await
        .expect("apply_edits should succeed");
    
    // Verificar eventos
    assert_eq!(
        observer.edits_applied_count.load(Ordering::SeqCst), 
        1, 
        "Should have 1 EditsApplied event"
    );
    assert_eq!(
        observer.preview_invalidated_count.load(Ordering::SeqCst), 
        1, 
        "Should have 1 PreviewInvalidated event"
    );
    assert_eq!(
        observer.thumbnail_invalidated_count.load(Ordering::SeqCst), 
        1, 
        "Should have 1 ThumbnailInvalidated event"
    );
    
    // Verificar ordem dos eventos
    let events = observer.events().await;
    assert_eq!(events.len(), 3, "Should have 3 events total");
    
    // Primeiro evento deve ser EditsApplied
    match &events[0] {
        EditEvent::EditsApplied { photo_id: pid, edits: e } => {
            assert_eq!(pid, &photo_id);
            assert_eq!(e.exposure, 0.5);
        }
        _ => panic!("First event should be EditsApplied"),
    }
    
    // Segundo evento deve ser PreviewInvalidated
    match &events[1] {
        EditEvent::PreviewInvalidated { photo_id: pid } => {
            assert_eq!(pid, &photo_id);
        }
        _ => panic!("Second event should be PreviewInvalidated"),
    }
    
    // Terceiro evento deve ser ThumbnailInvalidated
    match &events[2] {
        EditEvent::ThumbnailInvalidated { photo_id: pid } => {
            assert_eq!(pid, &photo_id);
        }
        _ => panic!("Third event should be ThumbnailInvalidated"),
    }
}

#[tokio::test]
async fn test_preview_edits_does_not_persist() {
    let (orchestrator, repo, _, _temp_dir) = setup_test_environment().await;
    
    let photo = create_and_save_test_photo(&repo, "/test/photo4.jpg").await;
    let photo_id = photo.id().clone();
    
    // Preview edits (não deve salvar)
    let edits = PhotoEdits {
        exposure: 2.0,
        contrast: 1.5,
        ..PhotoEdits::default()
    };
    
    orchestrator.preview_edits(&photo_id, &edits).await
        .expect("preview_edits should succeed");
    
    // Verificar que NÃO foi salvo no banco
    let saved_photo = repo.find_by_id(&photo_id).await
        .expect("find_by_id should succeed")
        .expect("Photo should exist");
    
    let saved_edits = saved_photo.get_photo_edits();
    assert_eq!(saved_edits.exposure, 0.0, "Exposure should NOT be saved");
    assert_eq!(saved_edits.contrast, 1.0, "Contrast should NOT be saved");
    
    // Verificar que está marcado como unsaved
    assert!(
        orchestrator.has_unsaved_edits(&photo_id).await.unwrap(),
        "Should have unsaved edits"
    );
}

#[tokio::test]
async fn test_reset_edits_restores_defaults() {
    let (orchestrator, repo, _, _temp_dir) = setup_test_environment().await;
    
    let photo = create_and_save_test_photo(&repo, "/test/photo5.jpg").await;
    let photo_id = photo.id().clone();
    
    // Primeiro aplicar edições
    let edits = PhotoEdits {
        exposure: 2.0,
        contrast: 1.5,
        saturation: 0.5,
        ..PhotoEdits::default()
    };
    
    orchestrator.apply_edits(&photo_id, edits).await
        .expect("apply_edits should succeed");
    
    // Resetar edições
    let result = orchestrator.reset_edits(&photo_id).await
        .expect("reset_edits should succeed");
    
    // Verificar que retornou valores default
    assert_eq!(result.applied_edits.exposure, 0.0);
    assert_eq!(result.applied_edits.contrast, 1.0);
    assert_eq!(result.applied_edits.saturation, 0.0);
    
    // Verificar no banco
    let saved_photo = repo.find_by_id(&photo_id).await
        .expect("find_by_id should succeed")
        .expect("Photo should exist");
    
    let saved_edits = saved_photo.get_photo_edits();
    assert_eq!(saved_edits.exposure, 0.0);
    assert_eq!(saved_edits.contrast, 1.0);
    assert_eq!(saved_edits.saturation, 0.0);
}

#[tokio::test]
async fn test_full_editing_workflow() {
    let (orchestrator, repo, preview_manager, _temp_dir) = setup_test_environment().await;
    
    // 1. Criar foto
    let photo = create_and_save_test_photo(&repo, "/test/workflow.jpg").await;
    let photo_id = photo.id().clone();
    
    // 2. Registrar observer
    let observer = Arc::new(TestObserver::new());
    orchestrator.register_observer(observer.clone());
    
    // 3. Simular preview em tempo real (múltiplos preview_edits)
    for exposure in [0.1, 0.2, 0.3, 0.4, 0.5] {
        let edits = PhotoEdits {
            exposure,
            ..PhotoEdits::default()
        };
        orchestrator.preview_edits(&photo_id, &edits).await.unwrap();
    }
    
    // Verificar que ainda não salvou
    let photo_in_db = repo.find_by_id(&photo_id).await.unwrap().unwrap();
    assert_eq!(photo_in_db.get_photo_edits().exposure, 0.0);
    
    // 4. Aplicar edição final (debounce terminou)
    let final_edits = PhotoEdits {
        exposure: 0.5,
        contrast: 1.2,
        ..PhotoEdits::default()
    };
    
    let result = orchestrator.apply_edits(&photo_id, final_edits.clone()).await.unwrap();
    
    // 5. Verificar estado final
    assert!(result.preview_invalidated);
    assert!(result.thumbnail_invalidated);
    
    // Edições salvas no banco
    let photo_in_db = repo.find_by_id(&photo_id).await.unwrap().unwrap();
    assert_eq!(photo_in_db.get_photo_edits().exposure, 0.5);
    assert_eq!(photo_in_db.get_photo_edits().contrast, 1.2);
    
    // Não tem mais edições pendentes
    assert!(!orchestrator.has_unsaved_edits(&photo_id).await.unwrap());
    
    // Cache foi invalidado
    assert!(!preview_manager.has(&photo_id, PreviewType::Large).unwrap());
}

#[tokio::test]
async fn test_multiple_photos_independence() {
    let (orchestrator, repo, _, _temp_dir) = setup_test_environment().await;
    
    // Criar duas fotos
    let photo1 = create_and_save_test_photo(&repo, "/test/photo_a.jpg").await;
    let photo2 = create_and_save_test_photo(&repo, "/test/photo_b.jpg").await;
    
    // Editar foto 1
    let edits1 = PhotoEdits {
        exposure: 1.0,
        ..PhotoEdits::default()
    };
    orchestrator.apply_edits(&photo1.id(), edits1).await.unwrap();
    
    // Editar foto 2 de forma diferente
    let edits2 = PhotoEdits {
        exposure: -1.0,
        contrast: 2.0,
        ..PhotoEdits::default()
    };
    orchestrator.apply_edits(&photo2.id(), edits2).await.unwrap();
    
    // Verificar que cada foto tem suas próprias edições
    let saved1 = repo.find_by_id(&photo1.id()).await.unwrap().unwrap();
    let saved2 = repo.find_by_id(&photo2.id()).await.unwrap().unwrap();
    
    assert_eq!(saved1.get_photo_edits().exposure, 1.0);
    assert_eq!(saved1.get_photo_edits().contrast, 1.0); // default
    
    assert_eq!(saved2.get_photo_edits().exposure, -1.0);
    assert_eq!(saved2.get_photo_edits().contrast, 2.0);
}

#[tokio::test]
async fn test_error_handling_nonexistent_photo() {
    let (orchestrator, _, _, _temp_dir) = setup_test_environment().await;
    
    let fake_id = PhotoId::new();
    let edits = PhotoEdits::default();
    
    let result = orchestrator.apply_edits(&fake_id, edits).await;
    
    assert!(result.is_err(), "Should fail for nonexistent photo");
}
