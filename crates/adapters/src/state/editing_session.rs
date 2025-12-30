use domain::value_objects::PhotoEdits;

/// Sessão de edição de uma foto
#[derive(Debug, Clone)]
pub struct EditingSession {
    pub photo_id: String,
    pub current_edits: PhotoEdits,
    pub saved_edits: PhotoEdits,
    pub is_dirty: bool,
}

impl EditingSession {
    pub fn new(photo_id: String, initial_edits: PhotoEdits) -> Self {
        Self {
            photo_id,
            current_edits: initial_edits.clone(),
            saved_edits: initial_edits,
            is_dirty: false,
        }
    }
    
    pub fn is_changed(&self) -> bool {
        self.is_dirty
    }
    
    pub fn mark_saved(&mut self) {
        self.saved_edits = self.current_edits.clone();
        self.is_dirty = false;
    }
}
