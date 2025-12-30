//! # Editor Service
//!
//! Serviço que gerencia sessões de edição de fotos.
//! Centraliza a lógica de edição, undo/redo e detecção de mudanças.

use domain::value_objects::PhotoEdits;
use crate::state::editing_session::EditingSession;

/// Serviço de edição de fotos
/// Gerencia a sessão de edição ativa e fornece uma API limpa para a UI
pub struct EditorService {
    /// Sessão de edição ativa (se houver)
    active_session: Option<EditingSession>,
}

impl EditorService {
    /// Cria um novo EditorService
    pub fn new() -> Self {
        Self {
            active_session: None,
        }
    }
    
    /// Inicia uma nova sessão de edição
    pub fn start_editing(&mut self, photo_id: String, initial_edits: PhotoEdits) {
        self.active_session = Some(EditingSession::new(photo_id, initial_edits));
    }
    
    /// Finaliza a sessão de edição atual
    pub fn end_editing(&mut self) {
        self.active_session = None;
    }
    
    /// Verifica se há uma sessão ativa
    pub fn has_active_session(&self) -> bool {
        self.active_session.is_some()
    }
    
    /// Retorna o ID da foto sendo editada (se houver)
    pub fn active_photo_id(&self) -> Option<&str> {
        self.active_session.as_ref().map(|s| s.photo_id.as_str())
    }
    
    /// Retorna as edições atuais (ou padrão se não houver sessão)
    pub fn current_edits(&self) -> PhotoEdits {
        self.active_session
            .as_ref()
            .map(|s| s.get_current_edits())
            .unwrap_or_default()
    }
    
    /// Atualiza as edições da sessão ativa
    pub fn update_edits(&mut self, edits: PhotoEdits, description: impl Into<String>) -> Result<(), String> {
        if let Some(session) = &mut self.active_session {
            session.update_edits(edits, description);
            Ok(())
        } else {
            Err("No active editing session".to_string())
        }
    }
    
    /// Atualiza um campo individual de edição
    pub fn update_field<F>(&mut self, description: impl Into<String>, update_fn: F) -> Result<(), String>
    where
        F: FnOnce(&mut PhotoEdits),
    {
        if let Some(session) = &mut self.active_session {
            let mut edits = session.get_current_edits();
            update_fn(&mut edits);
            session.update_edits(edits, description);
            Ok(())
        } else {
            Err("No active editing session".to_string())
        }
    }
    
    /// Executa undo
    pub fn undo(&mut self) -> Option<PhotoEdits> {
        self.active_session.as_mut().and_then(|s| s.undo())
    }
    
    /// Executa redo
    pub fn redo(&mut self) -> Option<PhotoEdits> {
        self.active_session.as_mut().and_then(|s| s.redo())
    }
    
    /// Pode fazer undo?
    pub fn can_undo(&self) -> bool {
        self.active_session.as_ref().map_or(false, |s| s.can_undo())
    }
    
    /// Pode fazer redo?
    pub fn can_redo(&self) -> bool {
        self.active_session.as_ref().map_or(false, |s| s.can_redo())
    }
    
    /// Reseta edições para valores padrão
    pub fn reset_to_defaults(&mut self) -> Result<(), String> {
        if let Some(session) = &mut self.active_session {
            session.reset_to_defaults();
            Ok(())
        } else {
            Err("No active editing session".to_string())
        }
    }
    
    /// Verifica se há mudanças não salvas
    pub fn has_unsaved_changes(&self) -> bool {
        self.active_session.as_ref().map_or(false, |s| s.is_changed())
    }
    
    /// Marca a sessão como salva
    pub fn mark_saved(&mut self) {
        if let Some(session) = &mut self.active_session {
            session.mark_saved();
        }
    }
    
    /// Retorna referência à sessão ativa (para casos avançados)
    pub fn active_session(&self) -> Option<&EditingSession> {
        self.active_session.as_ref()
    }
    
    /// Retorna referência mutável à sessão ativa (para casos avançados)
    pub fn active_session_mut(&mut self) -> Option<&mut EditingSession> {
        self.active_session.as_mut()
    }
}

impl Default for EditorService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_start_and_end_session() {
        let mut service = EditorService::new();
        assert!(!service.has_active_session());
        
        service.start_editing("photo1".to_string(), PhotoEdits::default());
        assert!(service.has_active_session());
        assert_eq!(service.active_photo_id(), Some("photo1"));
        
        service.end_editing();
        assert!(!service.has_active_session());
    }
    
    #[test]
    fn test_update_edits() {
        let mut service = EditorService::new();
        service.start_editing("photo1".to_string(), PhotoEdits::default());
        
        let mut new_edits = PhotoEdits::default();
        new_edits.exposure = 1.0;
        
        service.update_edits(new_edits.clone(), "Adjust exposure").unwrap();
        
        let current = service.current_edits();
        assert_eq!(current.exposure, 1.0);
        assert!(service.has_unsaved_changes());
    }
    
    #[test]
    fn test_undo_redo() {
        let mut service = EditorService::new();
        service.start_editing("photo1".to_string(), PhotoEdits::default());
        
        // Make first edit
        let mut edit1 = PhotoEdits::default();
        edit1.exposure = 1.0;
        service.update_edits(edit1, "Edit 1").unwrap();
        
        // Make second edit
        let mut edit2 = PhotoEdits::default();
        edit2.exposure = 2.0;
        service.update_edits(edit2, "Edit 2").unwrap();
        
        assert!(service.can_undo());
        
        // Undo to edit1
        service.undo();
        assert_eq!(service.current_edits().exposure, 1.0);
        
        // Redo to edit2
        assert!(service.can_redo());
        service.redo();
        assert_eq!(service.current_edits().exposure, 2.0);
    }
    
    #[test]
    fn test_update_field() {
        let mut service = EditorService::new();
        service.start_editing("photo1".to_string(), PhotoEdits::default());
        
        service.update_field("Adjust exposure", |edits| {
            edits.exposure = 0.5;
        }).unwrap();
        
        assert_eq!(service.current_edits().exposure, 0.5);
    }
    
    #[test]
    fn test_reset_to_defaults() {
        let mut service = EditorService::new();
        service.start_editing("photo1".to_string(), PhotoEdits::default());
        
        service.update_field("Adjust exposure", |edits| {
            edits.exposure = 2.0;
            edits.contrast = 1.5;
        }).unwrap();
        
        service.reset_to_defaults().unwrap();
        
        let current = service.current_edits();
        assert_eq!(current.exposure, 0.0);
        assert_eq!(current.contrast, 1.0);
    }
}
