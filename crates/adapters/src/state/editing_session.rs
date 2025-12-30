use domain::value_objects::PhotoEdits;
use super::edit_history::EditHistory;

/// Sessão de edição de uma foto
/// Gerencia o estado completo de edição incluindo histórico
#[derive(Debug)]
pub struct EditingSession {
    pub photo_id: String,
    pub current_edits: PhotoEdits,
    pub saved_edits: PhotoEdits,
    pub is_dirty: bool,
    /// Histórico de edições para undo/redo
    pub history: EditHistory,
}

impl EditingSession {
    pub fn new(photo_id: String, initial_edits: PhotoEdits) -> Self {
        let mut session = Self {
            photo_id,
            current_edits: initial_edits.clone(),
            saved_edits: initial_edits.clone(),
            is_dirty: false,
            history: EditHistory::new(),
        };
        // Adiciona estado inicial ao histórico
        session.history.push(initial_edits, "Initial state");
        session
    }
    
    pub fn is_changed(&self) -> bool {
        self.is_dirty
    }
    
    pub fn mark_saved(&mut self) {
        self.saved_edits = self.current_edits.clone();
        self.is_dirty = false;
    }
    
    /// Atualiza edições e adiciona ao histórico
    pub fn update_edits(&mut self, edits: PhotoEdits, description: impl Into<String>) {
        self.current_edits = edits.clone();
        self.is_dirty = true;
        self.history.push(edits, description);
    }
    
    /// Executa undo
    pub fn undo(&mut self) -> Option<PhotoEdits> {
        if let Some(edits) = self.history.undo() {
            self.current_edits = edits.clone();
            self.is_dirty = true;
            Some(edits.clone())
        } else {
            None
        }
    }
    
    /// Executa redo
    pub fn redo(&mut self) -> Option<PhotoEdits> {
        if let Some(edits) = self.history.redo() {
            self.current_edits = edits.clone();
            self.is_dirty = true;
            Some(edits.clone())
        } else {
            None
        }
    }
    
    /// Pode fazer undo?
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }
    
    /// Pode fazer redo?
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
    
    /// Reseta edições para valores padrão
    pub fn reset_to_defaults(&mut self) {
        let default_edits = PhotoEdits::default();
        self.update_edits(default_edits, "Reset to defaults");
    }
    
    /// Retorna referência para edições atuais
    pub fn current_edits(&self) -> &PhotoEdits {
        &self.current_edits
    }
    
    /// Retorna cópia das edições atuais
    pub fn get_current_edits(&self) -> PhotoEdits {
        self.current_edits.clone()
    }
}
