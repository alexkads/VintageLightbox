//! Editing Orchestrator Port
//!
//! Define o contrato para orquestração completa de edições de fotos.
//! Este trait garante que todas as operações de edição sejam coordenadas
//! de forma atômica: salvar no banco, invalidar caches e notificar observers.
//!
//! ## Problema que resolve
//! 
//! Antes, o fluxo de edição era fragmentado:
//! - SavePhotoEditsUseCase apenas salvava no banco
//! - Invalidação de thumbnail era feita manualmente em app.rs
//! - PreviewManager não era notificado
//! - "Corrige uma coisa e quebra outra"
//!
//! ## Garantias do Contrato
//!
//! Ao chamar `apply_edits()`, o implementador DEVE:
//! 1. Salvar as edições no banco de dados
//! 2. Invalidar o cache de preview para a foto
//! 3. Invalidar o cache de thumbnail para a foto
//! 4. Notificar todos os observers registrados
//!
//! Se qualquer etapa falhar, as anteriores devem ser revertidas (transacional).

use crate::value_objects::{PhotoEdits, PhotoId};
use crate::DomainResult;
use async_trait::async_trait;
use std::sync::Arc;

/// Resultado de uma operação de edição bem-sucedida
#[derive(Debug, Clone)]
pub struct EditResult {
    /// ID da foto editada
    pub photo_id: PhotoId,
    /// As edições que foram aplicadas
    pub applied_edits: PhotoEdits,
    /// Se o preview foi invalidado com sucesso
    pub preview_invalidated: bool,
    /// Se o thumbnail foi invalidado com sucesso
    pub thumbnail_invalidated: bool,
}

/// Evento emitido quando uma edição é aplicada
#[derive(Debug, Clone)]
pub enum EditEvent {
    /// Edições foram aplicadas e salvas com sucesso
    EditsApplied {
        photo_id: PhotoId,
        edits: PhotoEdits,
    },
    /// Edições foram resetadas para os valores padrão
    EditsReset {
        photo_id: PhotoId,
    },
    /// Preview foi invalidado e precisa ser regenerado
    PreviewInvalidated {
        photo_id: PhotoId,
    },
    /// Thumbnail foi invalidado e precisa ser regenerado
    ThumbnailInvalidated {
        photo_id: PhotoId,
    },
}

/// Observer que recebe notificações de eventos de edição.
/// 
/// Implementado pela UI para reagir a mudanças de estado.
#[async_trait]
pub trait EditObserver: Send + Sync {
    /// Chamado quando um evento de edição ocorre
    async fn on_edit_event(&self, event: EditEvent);
}

/// Orquestrador de edições de fotos.
///
/// Este é o contrato principal que garante a consistência do fluxo de edição.
/// A implementação deve coordenar:
/// - PhotoRepository (persistência)
/// - PreviewStorage (cache de previews)
/// - ThumbnailService (cache de thumbnails)
/// - EditObservers (notificações)
///
/// ## Exemplo de Uso
///
/// ```rust,ignore
/// let orchestrator: Arc<dyn EditingOrchestrator> = /* ... */;
/// 
/// // Aplicar edições - tudo é coordenado automaticamente
/// let result = orchestrator.apply_edits(&photo_id, edits).await?;
/// 
/// // A UI será notificada via EditObserver para atualizar
/// // Não precisa mais chamar invalidate_thumbnail manualmente!
/// ```
#[async_trait]
pub trait EditingOrchestrator: Send + Sync {
    /// Aplica edições a uma foto de forma coordenada.
    ///
    /// Esta é a operação principal. Após chamar este método:
    /// 1. As edições estarão salvas no banco
    /// 2. O cache de preview estará invalidado
    /// 3. O cache de thumbnail estará invalidado
    /// 4. Todos os observers terão sido notificados
    ///
    /// ## Transacionalidade
    /// 
    /// Se qualquer etapa falhar, as anteriores devem ser revertidas
    /// (quando possível) para manter consistência.
    ///
    /// ## Parâmetros
    /// - `photo_id`: ID da foto a ser editada
    /// - `edits`: As edições a serem aplicadas
    ///
    /// ## Retorno
    /// - `Ok(EditResult)`: Operação bem-sucedida com detalhes
    /// - `Err(DomainError)`: Falha em alguma etapa
    async fn apply_edits(
        &self,
        photo_id: &PhotoId,
        edits: PhotoEdits,
    ) -> DomainResult<EditResult>;

    /// Reseta as edições de uma foto para os valores padrão.
    ///
    /// Equivalente a chamar `apply_edits` com `PhotoEdits::default()`,
    /// mas pode ter otimizações específicas.
    async fn reset_edits(&self, photo_id: &PhotoId) -> DomainResult<EditResult>;

    /// Aplica edições sem salvar no banco (apenas preview temporário).
    ///
    /// Útil para preview em tempo real enquanto o usuário arrasta sliders.
    /// As edições só serão persistidas quando `apply_edits` for chamado.
    ///
    /// ## Nota
    /// Este método NÃO invalida o thumbnail, apenas o preview em memória.
    async fn preview_edits(
        &self,
        photo_id: &PhotoId,
        edits: &PhotoEdits,
    ) -> DomainResult<()>;

    /// Registra um observer para receber eventos de edição.
    ///
    /// O observer será notificado sempre que:
    /// - Edições forem aplicadas
    /// - Edições forem resetadas
    /// - Previews forem invalidados
    /// - Thumbnails forem invalidados
    fn register_observer(&self, observer: Arc<dyn EditObserver>);

    /// Remove um observer registrado.
    fn unregister_observer(&self, observer: &Arc<dyn EditObserver>);

    /// Força invalidação de todos os caches para uma foto.
    ///
    /// Útil quando a foto original foi modificada externamente.
    async fn invalidate_all_caches(&self, photo_id: &PhotoId) -> DomainResult<()>;

    /// Verifica se uma foto tem edições não salvas (dirty state).
    ///
    /// Compara as edições em memória com as persistidas no banco.
    async fn has_unsaved_edits(&self, photo_id: &PhotoId) -> DomainResult<bool>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock para testes
    #[derive(Default)]
    struct MockEditObserver {
        events: std::sync::Mutex<Vec<EditEvent>>,
    }

    #[async_trait]
    impl EditObserver for MockEditObserver {
        async fn on_edit_event(&self, event: EditEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    #[test]
    fn test_edit_result_creation() {
        let photo_id = PhotoId::new();
        let edits = PhotoEdits::default();
        
        let result = EditResult {
            photo_id: photo_id.clone(),
            applied_edits: edits,
            preview_invalidated: true,
            thumbnail_invalidated: true,
        };

        assert_eq!(result.photo_id, photo_id);
        assert!(result.preview_invalidated);
        assert!(result.thumbnail_invalidated);
    }

    #[test]
    fn test_edit_event_variants() {
        let photo_id = PhotoId::new();
        let edits = PhotoEdits::default();

        // Teste de criação de cada variante
        let event1 = EditEvent::EditsApplied {
            photo_id: photo_id.clone(),
            edits: edits.clone(),
        };
        
        let event2 = EditEvent::EditsReset {
            photo_id: photo_id.clone(),
        };
        
        let event3 = EditEvent::PreviewInvalidated {
            photo_id: photo_id.clone(),
        };
        
        let event4 = EditEvent::ThumbnailInvalidated {
            photo_id: photo_id.clone(),
        };

        // Verificar que todos são criados corretamente
        match event1 {
            EditEvent::EditsApplied { photo_id: pid, .. } => {
                assert_eq!(pid, photo_id);
            }
            _ => panic!("Wrong variant"),
        }

        match event2 {
            EditEvent::EditsReset { photo_id: pid } => {
                assert_eq!(pid, photo_id);
            }
            _ => panic!("Wrong variant"),
        }

        match event3 {
            EditEvent::PreviewInvalidated { photo_id: pid } => {
                assert_eq!(pid, photo_id);
            }
            _ => panic!("Wrong variant"),
        }

        match event4 {
            EditEvent::ThumbnailInvalidated { photo_id: pid } => {
                assert_eq!(pid, photo_id);
            }
            _ => panic!("Wrong variant"),
        }
    }
}
