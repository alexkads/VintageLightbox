//! # Edit History
//!
//! Gerencia histórico de edições para undo/redo.

use super::editing_session::PhotoEdits;

/// Snapshot de estado para undo/redo
#[derive(Debug, Clone)]
pub struct EditSnapshot {
    /// Edições no momento do snapshot
    pub edits: PhotoEdits,
    /// Descrição da ação (para UI)
    pub description: String,
}

/// Histórico de edições com suporte a undo/redo
#[derive(Debug, Default)]
pub struct EditHistory {
    /// Stack de snapshots
    history: Vec<EditSnapshot>,
    /// Índice atual no histórico (None = sem histórico)
    current_index: Option<usize>,
    /// Tamanho máximo do histórico
    max_size: usize,
}

impl EditHistory {
    /// Cria um novo histórico com tamanho máximo padrão (50)
    pub fn new() -> Self {
        Self::with_max_size(50)
    }

    /// Cria um novo histórico com tamanho máximo customizado
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            history: Vec::with_capacity(max_size),
            current_index: None,
            max_size,
        }
    }

    /// Adiciona um snapshot ao histórico
    pub fn push(&mut self, edits: PhotoEdits, description: impl Into<String>) {
        // Se estamos no meio do histórico, remove tudo após o índice atual
        if let Some(idx) = self.current_index {
            if idx + 1 < self.history.len() {
                self.history.truncate(idx + 1);
            }
        }

        // Remove o mais antigo se atingiu o limite
        if self.history.len() >= self.max_size {
            self.history.remove(0);
            if let Some(idx) = &mut self.current_index {
                *idx = idx.saturating_sub(1);
            }
        }

        let snapshot = EditSnapshot {
            edits,
            description: description.into(),
        };

        self.history.push(snapshot);
        self.current_index = Some(self.history.len() - 1);
    }

    /// Pode fazer undo?
    pub fn can_undo(&self) -> bool {
        self.current_index.map_or(false, |idx| idx > 0)
    }

    /// Pode fazer redo?
    pub fn can_redo(&self) -> bool {
        self.current_index.map_or(false, |idx| idx + 1 < self.history.len())
    }

    /// Executa undo e retorna as edições anteriores
    pub fn undo(&mut self) -> Option<&PhotoEdits> {
        if !self.can_undo() {
            return None;
        }

        let new_idx = self.current_index.unwrap() - 1;
        self.current_index = Some(new_idx);
        self.history.get(new_idx).map(|s| &s.edits)
    }

    /// Executa redo e retorna as próximas edições
    pub fn redo(&mut self) -> Option<&PhotoEdits> {
        if !self.can_redo() {
            return None;
        }

        let new_idx = self.current_index.unwrap() + 1;
        self.current_index = Some(new_idx);
        self.history.get(new_idx).map(|s| &s.edits)
    }

    /// Limpa todo o histórico
    pub fn clear(&mut self) {
        self.history.clear();
        self.current_index = None;
    }

    /// Retorna o tamanho atual do histórico
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Verifica se o histórico está vazio
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Retorna a descrição da última ação (para UI)
    pub fn current_description(&self) -> Option<&str> {
        self.current_index
            .and_then(|idx| self.history.get(idx))
            .map(|s| s.description.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_history_is_empty() {
        let history = EditHistory::new();
        assert!(history.is_empty());
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_push_and_undo() {
        let mut history = EditHistory::new();
        
        let mut edits1 = PhotoEdits::new();
        edits1.exposure = 0.0;
        history.push(edits1, "Initial");

        let mut edits2 = PhotoEdits::new();
        edits2.exposure = 0.5;
        history.push(edits2, "Adjust exposure");

        assert!(history.can_undo());
        let undone = history.undo().unwrap();
        assert_eq!(undone.exposure, 0.0);
    }

    #[test]
    fn test_undo_and_redo() {
        let mut history = EditHistory::new();
        
        let mut edits1 = PhotoEdits::new();
        edits1.exposure = 0.0;
        history.push(edits1, "Initial");

        let mut edits2 = PhotoEdits::new();
        edits2.exposure = 1.0;
        history.push(edits2, "Exposure +1");

        history.undo();
        assert!(history.can_redo());
        
        let redone = history.redo().unwrap();
        assert_eq!(redone.exposure, 1.0);
    }

    #[test]
    fn test_push_after_undo_clears_redo() {
        let mut history = EditHistory::new();
        
        history.push(PhotoEdits::new(), "1");
        history.push(PhotoEdits::new(), "2");
        history.push(PhotoEdits::new(), "3");

        history.undo(); // Volta para 2
        
        history.push(PhotoEdits::new(), "new");
        
        // Redo não deve estar disponível
        assert!(!history.can_redo());
        assert_eq!(history.len(), 3); // 1, 2, new
    }

    #[test]
    fn test_max_size_limit() {
        let mut history = EditHistory::with_max_size(3);
        
        for i in 0..5 {
            let mut edits = PhotoEdits::new();
            edits.exposure = i as f32;
            history.push(edits, format!("Edit {}", i));
        }

        assert_eq!(history.len(), 3);
        
        // Primeiro item deve ser "Edit 2" (os primeiros foram removidos)
        while history.can_undo() {
            history.undo();
        }
        assert_eq!(history.current_description(), Some("Edit 2"));
    }
}
