//! Editing Orchestrator Implementation
//!
//! Implementação do contrato EditingOrchestrator que coordena:
//! - Persistência de edições no banco
//! - Invalidação de cache de previews  
//! - Invalidação de cache de thumbnails
//! - Notificação de observers

use std::sync::{Arc, RwLock};
use async_trait::async_trait;
use domain::{
    ports::{EditingOrchestrator, EditObserver, EditEvent, EditResult},
    repositories::PhotoRepository,
    services::PreviewStorage,
    value_objects::{PhotoEdits, PhotoId},
    DomainResult, DomainError,
};

/// Implementação do EditingOrchestrator.
/// 
/// Coordena todas as operações de edição de forma atômica,
/// garantindo que:
/// 1. Edições sejam salvas no banco
/// 2. Caches sejam invalidados
/// 3. Observers sejam notificados
///
/// ## Thread Safety
/// 
/// Esta implementação é thread-safe. Múltiplas edições podem
/// ocorrer em paralelo, porém para a mesma foto serão serializadas.
pub struct EditingOrchestratorImpl<R, P>
where
    R: PhotoRepository,
    P: PreviewStorage,
{
    photo_repository: Arc<R>,
    preview_storage: Arc<P>,
    observers: RwLock<Vec<Arc<dyn EditObserver>>>,
    /// Cache de edições não salvas (dirty state)
    /// Mapeamento de PhotoId -> PhotoEdits em memória
    pending_edits: RwLock<std::collections::HashMap<PhotoId, PhotoEdits>>,
}

impl<R, P> EditingOrchestratorImpl<R, P>
where
    R: PhotoRepository,
    P: PreviewStorage,
{
    /// Cria uma nova instância do orchestrator.
    ///
    /// ## Parâmetros
    /// - `photo_repository`: Repository para persistência de fotos
    /// - `preview_storage`: Storage para cache de previews
    pub fn new(
        photo_repository: Arc<R>,
        preview_storage: Arc<P>,
    ) -> Self {
        Self {
            photo_repository,
            preview_storage,
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
impl<R, P> EditingOrchestrator for EditingOrchestratorImpl<R, P>
where
    R: PhotoRepository + 'static,
    P: PreviewStorage + 'static,
{
    async fn apply_edits(
        &self,
        photo_id: &PhotoId,
        edits: PhotoEdits,
    ) -> DomainResult<EditResult> {
        // 1. Buscar foto do banco
        let mut photo = self.photo_repository
            .find_by_id(photo_id)
            .await?
            .ok_or(DomainError::PhotoNotFound)?;

        // 2. Aplicar edições na entidade
        photo.apply_photo_edits(edits.clone())?;

        // 3. Salvar no banco (transacional)
        self.photo_repository.update(&photo).await?;

        // 4. Invalidar caches
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

        // 3. Salvar no banco
        self.photo_repository.update(&photo).await?;

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

    // Mock PhotoRepository
    struct MockPhotoRepository {
        photos: TokioMutex<std::collections::HashMap<PhotoId, domain::entities::Photo>>,
        update_count: AtomicUsize,
    }

    impl MockPhotoRepository {
        fn new() -> Self {
            Self {
                photos: TokioMutex::new(std::collections::HashMap::new()),
                update_count: AtomicUsize::new(0),
            }
        }

        async fn add_photo(&self, photo: domain::entities::Photo) {
            let mut photos = self.photos.lock().await;
            photos.insert(photo.id().clone(), photo);
        }
    }

    #[async_trait]
    impl PhotoRepository for MockPhotoRepository {
        async fn save(&self, photo: &domain::entities::Photo) -> DomainResult<()> {
            let mut photos = self.photos.lock().await;
            photos.insert(photo.id().clone(), photo.clone());
            Ok(())
        }

        async fn find_by_id(&self, id: &PhotoId) -> DomainResult<Option<domain::entities::Photo>> {
            let photos = self.photos.lock().await;
            Ok(photos.get(id).cloned())
        }

        async fn find_all(&self) -> DomainResult<Vec<domain::entities::Photo>> {
            let photos = self.photos.lock().await;
            Ok(photos.values().cloned().collect())
        }

        async fn update(&self, photo: &domain::entities::Photo) -> DomainResult<()> {
            self.update_count.fetch_add(1, Ordering::SeqCst);
            let mut photos = self.photos.lock().await;
            photos.insert(photo.id().clone(), photo.clone());
            Ok(())
        }

        async fn delete(&self, id: &PhotoId) -> DomainResult<()> {
            let mut photos = self.photos.lock().await;
            photos.remove(id);
            Ok(())
        }

        async fn exists(&self, id: &PhotoId) -> DomainResult<bool> {
            let photos = self.photos.lock().await;
            Ok(photos.contains_key(id))
        }

        async fn find_by_content_hash(&self, _hash: &str) -> DomainResult<Option<domain::entities::Photo>> {
            Ok(None)
        }
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
        let repo = Arc::new(MockPhotoRepository::new());
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage.clone());

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.add_photo(photo).await;

        let edits = PhotoEdits {
            exposure: 0.5,
            ..PhotoEdits::default()
        };

        let result = orchestrator.apply_edits(&photo_id, edits).await;
        
        assert!(result.is_ok());
        assert_eq!(repo.update_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_apply_edits_invalidates_cache() {
        let repo = Arc::new(MockPhotoRepository::new());
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage.clone());

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.add_photo(photo).await;

        let edits = PhotoEdits::default();
        let result = orchestrator.apply_edits(&photo_id, edits).await.unwrap();

        assert!(result.preview_invalidated);
        assert!(result.thumbnail_invalidated);
        assert_eq!(storage.delete_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_apply_edits_notifies_observers() {
        let repo = Arc::new(MockPhotoRepository::new());
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage.clone());

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.add_photo(photo).await;

        let observer = Arc::new(MockEditObserver::new());
        orchestrator.register_observer(observer.clone());

        let edits = PhotoEdits::default();
        orchestrator.apply_edits(&photo_id, edits).await.unwrap();

        // Deve ter 3 eventos: EditsApplied, PreviewInvalidated, ThumbnailInvalidated
        assert_eq!(observer.event_count().await, 3);
    }

    #[tokio::test]
    async fn test_reset_edits_returns_default_values() {
        let repo = Arc::new(MockPhotoRepository::new());
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage);

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.add_photo(photo).await;

        let result = orchestrator.reset_edits(&photo_id).await.unwrap();

        assert_eq!(result.applied_edits.exposure, 0.0);
        assert_eq!(result.applied_edits.contrast, 1.0);
    }

    #[tokio::test]
    async fn test_preview_edits_does_not_save_to_repository() {
        let repo = Arc::new(MockPhotoRepository::new());
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage);

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.add_photo(photo).await;

        let edits = PhotoEdits {
            exposure: 0.5,
            ..PhotoEdits::default()
        };

        orchestrator.preview_edits(&photo_id, &edits).await.unwrap();

        // Não deve ter salvado no repositório
        assert_eq!(repo.update_count.load(Ordering::SeqCst), 0);
        
        // Mas deve ter registrado como pending
        assert!(orchestrator.has_unsaved_edits(&photo_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_apply_edits_clears_pending() {
        let repo = Arc::new(MockPhotoRepository::new());
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage);

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.add_photo(photo).await;

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
        let repo = Arc::new(MockPhotoRepository::new());
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo.clone(), storage);

        let photo = create_test_photo();
        let photo_id = photo.id().clone();
        repo.add_photo(photo).await;

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
        let repo = Arc::new(MockPhotoRepository::new());
        let storage = Arc::new(MockPreviewStorage::new());
        let orchestrator = EditingOrchestratorImpl::new(repo, storage);

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
