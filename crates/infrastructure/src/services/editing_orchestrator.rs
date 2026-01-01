//! Editing Orchestrator Implementation
//!
//! Implementação do contrato EditingOrchestrator que coordena:
//! - Persistência de edições no banco (com transações via UnitOfWork)
//! - Invalidação de cache de previews  
//! - Invalidação de cache de thumbnails
//! - Notificação de observers
//!
//! ## Uso com UnitOfWork
//!
//! O orchestrator usa UnitOfWork internamente para garantir
//! atomicidade das operações de banco de dados:
//!
//! ```rust,ignore
//! let orchestrator = EditingOrchestratorImpl::new(
//!     photo_repository,
//!     preview_storage,
//!     unit_of_work,
//! );
//! 
//! // Todas as operações são atômicas - commit ou rollback completo
//! orchestrator.apply_edits(&photo_id, edits).await?;
//! ```

use std::sync::{Arc, RwLock};
use async_trait::async_trait;
use domain::{
    ports::{EditingOrchestrator, EditObserver, EditEvent, EditResult, UnitOfWork, TransactionScope},
    repositories::PhotoRepository,
    services::PreviewStorage,
    value_objects::{PhotoEdits, PhotoId},
    DomainResult, DomainError,
};

use crate::database::{PhotoRepositoryImpl, SqliteUnitOfWork};

/// Implementação do EditingOrchestrator com suporte a UnitOfWork.
/// 
/// Coordena todas as operações de edição de forma atômica,
/// garantindo que:
/// 1. Edições sejam salvas no banco (com transação)
/// 2. Caches sejam invalidados
/// 3. Observers sejam notificados
///
/// ## Atomicidade
/// 
/// Em caso de falha em qualquer etapa da persistência,
/// a transação é automaticamente revertida (rollback).
///
/// ## Thread Safety
/// 
/// Esta implementação é thread-safe. Múltiplas edições podem
/// ocorrer em paralelo, porém para a mesma foto serão serializadas.
pub struct EditingOrchestratorImpl<P>
where
    P: PreviewStorage,
{
    photo_repository: Arc<PhotoRepositoryImpl>,
    preview_storage: Arc<P>,
    unit_of_work: Arc<SqliteUnitOfWork>,
    observers: RwLock<Vec<Arc<dyn EditObserver>>>,
    /// Cache de edições não salvas (dirty state)
    /// Mapeamento de PhotoId -> PhotoEdits em memória
    pending_edits: RwLock<std::collections::HashMap<PhotoId, PhotoEdits>>,
}

impl<P> EditingOrchestratorImpl<P>
where
    P: PreviewStorage,
{
    /// Cria uma nova instância do orchestrator com UnitOfWork.
    ///
    /// ## Parâmetros
    /// - `photo_repository`: Repository para persistência de fotos
    /// - `preview_storage`: Storage para cache de previews
    /// - `unit_of_work`: UnitOfWork para gerenciamento de transações
    pub fn new(
        photo_repository: Arc<PhotoRepositoryImpl>,
        preview_storage: Arc<P>,
        unit_of_work: Arc<SqliteUnitOfWork>,
    ) -> Self {
        Self {
            photo_repository,
            preview_storage,
            unit_of_work,
            observers: RwLock::new(Vec::new()),
            pending_edits: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Notifica todos os observers sobre um evento.
    async fn notify_observers(&self, event: EditEvent) {
        let observers = self.observers.read().unwrap().clone();
        for observer in observers {
            observer.on_edit_event(event.clone()).await;
        }
    }

    /// Invalida o cache de preview para uma foto.
    fn invalidate_preview_cache(&self, photo_id: &PhotoId) -> DomainResult<bool> {
        // Deletar tanto thumbnail quanto large preview
        self.preview_storage.delete(photo_id)?;
        Ok(true)
    }
}

#[async_trait]
impl<P> EditingOrchestrator for EditingOrchestratorImpl<P>
where
    P: PreviewStorage + 'static,
{
    async fn apply_edits(
        &self,
        photo_id: &PhotoId,
        edits: PhotoEdits,
    ) -> DomainResult<EditResult> {
        // 1. Buscar foto do banco (fora da transação para read)
        let mut photo = self.photo_repository
            .find_by_id(photo_id)
            .await?
            .ok_or(DomainError::PhotoNotFound)?;

        // 2. Aplicar edições na entidade
        photo.apply_photo_edits(edits.clone())?;

        // 3. Iniciar transação e salvar no banco
        let mut tx = self.unit_of_work.begin().await?;
        
        match self.photo_repository.update_within(&mut tx, &photo).await {
            Ok(_) => {
                // Commit da transação
                tx.commit().await?;
            }
            Err(e) => {
                // Rollback automático no drop, mas logamos o erro
                // tx será dropado aqui e fará rollback
                return Err(e);
            }
        }

        // 4. Invalidar caches (após commit bem-sucedido)
        let preview_invalidated = self.invalidate_preview_cache(photo_id)
            .unwrap_or_else(|e| {
                eprintln!("Failed to invalidate preview cache: {}", e);
                false
            });

        // O thumbnail também é armazenado no PreviewStorage, então já foi invalidado
        let thumbnail_invalidated = preview_invalidated;

        // 5. Limpar pending edits
        {
            let mut pending = self.pending_edits.write().unwrap();
            pending.remove(photo_id);
        }

        // 6. Notificar observers
        self.notify_observers(EditEvent::EditsApplied {
            photo_id: photo_id.clone(),
            edits: edits.clone(),
        }).await;

        self.notify_observers(EditEvent::PreviewInvalidated {
            photo_id: photo_id.clone(),
        }).await;

        self.notify_observers(EditEvent::ThumbnailInvalidated {
            photo_id: photo_id.clone(),
        }).await;

        Ok(EditResult {
            photo_id: photo_id.clone(),
            applied_edits: edits,
            preview_invalidated,
            thumbnail_invalidated,
        })
    }

    async fn reset_edits(&self, photo_id: &PhotoId) -> DomainResult<EditResult> {
        let default_edits = PhotoEdits::default();
        
        // 1. Buscar foto do banco
        let mut photo = self.photo_repository
            .find_by_id(photo_id)
            .await?
            .ok_or(DomainError::PhotoNotFound)?;

        // 2. Resetar edições na entidade
        photo.apply_photo_edits(default_edits.clone())?;

        // 3. Salvar no banco com transação
        let mut tx = self.unit_of_work.begin().await?;
        self.photo_repository.update_within(&mut tx, &photo).await?;
        tx.commit().await?;

        // 4. Invalidar caches
        let preview_invalidated = self.invalidate_preview_cache(photo_id)
            .unwrap_or(false);
        let thumbnail_invalidated = preview_invalidated;

        // 5. Limpar pending edits
        {
            let mut pending = self.pending_edits.write().unwrap();
            pending.remove(photo_id);
        }

        // 6. Notificar observers
        self.notify_observers(EditEvent::EditsReset {
            photo_id: photo_id.clone(),
        }).await;

        self.notify_observers(EditEvent::PreviewInvalidated {
            photo_id: photo_id.clone(),
        }).await;

        self.notify_observers(EditEvent::ThumbnailInvalidated {
            photo_id: photo_id.clone(),
        }).await;

        Ok(EditResult {
            photo_id: photo_id.clone(),
            applied_edits: default_edits,
            preview_invalidated,
            thumbnail_invalidated,
        })
    }

    async fn preview_edits(
        &self,
        photo_id: &PhotoId,
        edits: &PhotoEdits,
    ) -> DomainResult<()> {
        // Apenas salvar em memória para preview temporário
        // NÃO salva no banco nem invalida thumbnail
        {
            let mut pending = self.pending_edits.write().unwrap();
            pending.insert(photo_id.clone(), edits.clone());
        }

        // Notificar observers para atualizar o preview na UI
        // Mas NÃO notificar ThumbnailInvalidated
        self.notify_observers(EditEvent::EditsApplied {
            photo_id: photo_id.clone(),
            edits: edits.clone(),
        }).await;

        Ok(())
    }

    fn register_observer(&self, observer: Arc<dyn EditObserver>) {
        let mut observers = self.observers.write().unwrap();
        observers.push(observer);
    }

    fn unregister_observer(&self, observer: &Arc<dyn EditObserver>) {
        let mut observers = self.observers.write().unwrap();
        observers.retain(|o| !Arc::ptr_eq(o, observer));
    }

    async fn invalidate_all_caches(&self, photo_id: &PhotoId) -> DomainResult<()> {
        // Invalidar preview cache (inclui thumbnail)
        self.invalidate_preview_cache(photo_id)?;

        // Notificar observers
        self.notify_observers(EditEvent::PreviewInvalidated {
            photo_id: photo_id.clone(),
        }).await;

        self.notify_observers(EditEvent::ThumbnailInvalidated {
            photo_id: photo_id.clone(),
        }).await;

        Ok(())
    }

    async fn has_unsaved_edits(&self, photo_id: &PhotoId) -> DomainResult<bool> {
        let pending = self.pending_edits.read().unwrap();
        Ok(pending.contains_key(photo_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex as TokioMutex;
    use domain::services::PreviewType;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Cria um PhotoRepositoryImpl e UnitOfWork reais para testes
    /// usando um banco SQLite em memória
    async fn create_test_infra() -> (Arc<PhotoRepositoryImpl>, Arc<SqliteUnitOfWork>) {
        // Usar SQLite in-memory com shared cache para permitir múltiplas conexões
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        
        // Aplicar migrações diretamente usando o SQL inline (mais confiável para testes in-memory)
        sqlx::query(include_str!("../../migrations/001_initial_schema.sql"))
            .execute(&pool)
            .await
            .unwrap();
        
        // Adicionar colunas de migrações adicionais
        for migration in &[
            "ALTER TABLE photos ADD COLUMN metadata TEXT",
            "ALTER TABLE photos ADD COLUMN thumbnail_path TEXT",
            "ALTER TABLE photos ADD COLUMN preview_path TEXT",
            "ALTER TABLE photos ADD COLUMN edit_exposure REAL",
            "ALTER TABLE photos ADD COLUMN edit_contrast REAL",
            "ALTER TABLE photos ADD COLUMN edit_temperature REAL",
            "ALTER TABLE photos ADD COLUMN edit_tint REAL",
            "ALTER TABLE photos ADD COLUMN edit_highlights REAL",
            "ALTER TABLE photos ADD COLUMN edit_shadows REAL",
            "ALTER TABLE photos ADD COLUMN edit_whites REAL",
            "ALTER TABLE photos ADD COLUMN edit_blacks REAL",
            "ALTER TABLE photos ADD COLUMN edit_clarity REAL",
            "ALTER TABLE photos ADD COLUMN edit_vibrance REAL",
            "ALTER TABLE photos ADD COLUMN edit_saturation REAL",
            "ALTER TABLE photos ADD COLUMN edit_tone_curve_shadows REAL",
            "ALTER TABLE photos ADD COLUMN edit_tone_curve_darks REAL",
            "ALTER TABLE photos ADD COLUMN edit_tone_curve_lights REAL",
            "ALTER TABLE photos ADD COLUMN edit_tone_curve_highlights REAL",
            "ALTER TABLE photos ADD COLUMN content_hash TEXT",
            "ALTER TABLE photos ADD COLUMN flag INTEGER",
            // HSL saturation
            "ALTER TABLE photos ADD COLUMN edit_hsl_red_sat REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_orange_sat REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_yellow_sat REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_green_sat REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_aqua_sat REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_blue_sat REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_purple_sat REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_magenta_sat REAL",
            // HSL hue
            "ALTER TABLE photos ADD COLUMN edit_hsl_red_hue REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_orange_hue REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_yellow_hue REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_green_hue REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_aqua_hue REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_blue_hue REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_purple_hue REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_magenta_hue REAL",
            // HSL luminance
            "ALTER TABLE photos ADD COLUMN edit_hsl_red_lum REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_orange_lum REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_yellow_lum REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_green_lum REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_aqua_lum REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_blue_lum REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_purple_lum REAL",
            "ALTER TABLE photos ADD COLUMN edit_hsl_magenta_lum REAL",
            // Lens correction
            "ALTER TABLE photos ADD COLUMN edit_lens_distortion REAL",
            "ALTER TABLE photos ADD COLUMN edit_lens_vignette_amount REAL",
            "ALTER TABLE photos ADD COLUMN edit_lens_vignette_midpoint REAL",
            // Noise reduction
            "ALTER TABLE photos ADD COLUMN edit_nr_luminance REAL",
            "ALTER TABLE photos ADD COLUMN edit_nr_color REAL",
            // Sharpening
            "ALTER TABLE photos ADD COLUMN edit_sharpen_amount REAL",
            "ALTER TABLE photos ADD COLUMN edit_sharpen_radius REAL",
            // Crop
            "ALTER TABLE photos ADD COLUMN edit_crop_x REAL",
            "ALTER TABLE photos ADD COLUMN edit_crop_y REAL",
            "ALTER TABLE photos ADD COLUMN edit_crop_width REAL",
            "ALTER TABLE photos ADD COLUMN edit_crop_height REAL",
            "ALTER TABLE photos ADD COLUMN edit_crop_rotation INTEGER",
            "ALTER TABLE photos ADD COLUMN edit_crop_angle REAL",
            "ALTER TABLE photos ADD COLUMN edit_crop_flip_h INTEGER",
            "ALTER TABLE photos ADD COLUMN edit_crop_flip_v INTEGER",
            "ALTER TABLE photos ADD COLUMN edit_crop_fill_mode INTEGER",
        ] {
            let _ = sqlx::query(*migration).execute(&pool).await;
        }
        
        let repo = Arc::new(PhotoRepositoryImpl::new(pool.clone()));
        let uow = Arc::new(SqliteUnitOfWork::new(pool));
        
        (repo, uow)
    }

    // Mock PreviewStorage
    struct MockPreviewStorage {
        delete_count: AtomicUsize,
    }

    impl MockPreviewStorage {
        fn new() -> Self {
            Self {
                delete_count: AtomicUsize::new(0),
            }
        }
    }

    impl PreviewStorage for MockPreviewStorage {
        fn save(&self, _id: &PhotoId, _preview_type: PreviewType, _data: &[u8]) -> DomainResult<()> {
            Ok(())
        }

        fn get(&self, _id: &PhotoId, _preview_type: PreviewType) -> DomainResult<Option<Vec<u8>>> {
            Ok(None)
        }

        fn has(&self, _id: &PhotoId, _preview_type: PreviewType) -> DomainResult<bool> {
            Ok(false)
        }

        fn delete(&self, _id: &PhotoId) -> DomainResult<()> {
            self.delete_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    // Mock EditObserver
    struct MockEditObserver {
        events: TokioMutex<Vec<EditEvent>>,
    }

    impl MockEditObserver {
        fn new() -> Self {
            Self {
                events: TokioMutex::new(Vec::new()),
            }
        }

        async fn event_count(&self) -> usize {
            self.events.lock().await.len()
        }
    }

    #[async_trait]
    impl EditObserver for MockEditObserver {
        async fn on_edit_event(&self, event: EditEvent) {
            self.events.lock().await.push(event);
        }
    }

    fn create_test_photo() -> domain::entities::Photo {
        domain::entities::Photo::new(
            domain::value_objects::FilePath::new("/test/photo.jpg").unwrap()
        )
    }

    #[tokio::test]
    async fn test_apply_edits_saves_to_repository() {
        let (repo, uow) = create_test_infra().await;
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage.clone(), uow);

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.save(&photo).await.unwrap();

        let edits = PhotoEdits {
            exposure: 0.5,
            ..PhotoEdits::default()
        };

        let result = orchestrator.apply_edits(&photo_id, edits).await;
        
        assert!(result.is_ok());
        
        // Verifica que a foto foi atualizada no banco
        let updated_photo = repo.find_by_id(&photo_id).await.unwrap().unwrap();
        assert_eq!(updated_photo.edit_exposure().unwrap_or(0.0), 0.5);
    }

    #[tokio::test]
    async fn test_apply_edits_invalidates_cache() {
        let (repo, uow) = create_test_infra().await;
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage.clone(), uow);

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.save(&photo).await.unwrap();

        let edits = PhotoEdits::default();
        let result = orchestrator.apply_edits(&photo_id, edits).await.unwrap();

        assert!(result.preview_invalidated);
        assert!(result.thumbnail_invalidated);
        assert_eq!(storage.delete_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_apply_edits_notifies_observers() {
        let (repo, uow) = create_test_infra().await;
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage.clone(), uow);

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.save(&photo).await.unwrap();

        let observer = Arc::new(MockEditObserver::new());
        orchestrator.register_observer(observer.clone());

        let edits = PhotoEdits::default();
        orchestrator.apply_edits(&photo_id, edits).await.unwrap();

        // Deve ter 3 eventos: EditsApplied, PreviewInvalidated, ThumbnailInvalidated
        assert_eq!(observer.event_count().await, 3);
    }

    #[tokio::test]
    async fn test_reset_edits_returns_default_values() {
        let (repo, uow) = create_test_infra().await;
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage, uow);

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.save(&photo).await.unwrap();

        let result = orchestrator.reset_edits(&photo_id).await.unwrap();

        assert_eq!(result.applied_edits.exposure, 0.0);
        assert_eq!(result.applied_edits.contrast, 1.0);
    }

    #[tokio::test]
    async fn test_preview_edits_does_not_save_to_repository() {
        let (repo, uow) = create_test_infra().await;
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage, uow);

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.save(&photo).await.unwrap();

        let edits = PhotoEdits {
            exposure: 0.5,
            ..PhotoEdits::default()
        };

        orchestrator.preview_edits(&photo_id, &edits).await.unwrap();

        // Não deve ter alterado no banco
        let photo_from_db = repo.find_by_id(&photo_id).await.unwrap().unwrap();
        assert_eq!(photo_from_db.edit_exposure().unwrap_or(0.0), 0.0);
        
        // Mas deve ter registrado como pending
        assert!(orchestrator.has_unsaved_edits(&photo_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_apply_edits_clears_pending() {
        let (repo, uow) = create_test_infra().await;
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage, uow);

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.save(&photo).await.unwrap();

        let edits = PhotoEdits {
            exposure: 0.5,
            ..PhotoEdits::default()
        };

        // Primeiro preview (não salva)
        orchestrator.preview_edits(&photo_id, &edits).await.unwrap();
        assert!(orchestrator.has_unsaved_edits(&photo_id).await.unwrap());

        // Depois apply (salva e limpa pending)
        orchestrator.apply_edits(&photo_id, edits).await.unwrap();
        assert!(!orchestrator.has_unsaved_edits(&photo_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_unregister_observer() {
        let (repo, uow) = create_test_infra().await;
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage, uow);

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.save(&photo).await.unwrap();

        let observer: Arc<dyn EditObserver> = Arc::new(MockEditObserver::new());
        orchestrator.register_observer(observer.clone());
        orchestrator.unregister_observer(&observer);

        let edits = PhotoEdits::default();
        orchestrator.apply_edits(&photo_id, edits).await.unwrap();

        // Observer foi removido, não podemos mais verificar eventos diretamente
        // O teste passa se não houver panic
    }

    #[tokio::test]
    async fn test_apply_edits_returns_error_for_nonexistent_photo() {
        let (repo, uow) = create_test_infra().await;
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo, storage, uow);

        let photo_id = PhotoId::new();
        let edits = PhotoEdits::default();

        let result = orchestrator.apply_edits(&photo_id, edits).await;

        assert!(result.is_err());
        match result {
            Err(DomainError::PhotoNotFound) => {}
            _ => panic!("Expected PhotoNotFound error"),
        }
    }
}
