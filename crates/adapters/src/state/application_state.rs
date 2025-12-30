use crate::view_models::PhotoViewModel;
use super::editing_session::EditingSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentView {
    Library,
    Develop,
    Print,
    Import,
}

/// Estado da aplicação (independente de framework UI)
pub struct ApplicationState {
    // Navegação
    pub current_view: CurrentView,
    
    // Biblioteca
    pub photos: Vec<PhotoViewModel>,
    pub library_selected_id: Option<String>,
    pub develop_selected_id: Option<String>,
    
    // Edição
    pub editing_session: Option<EditingSession>,
    
    // Filtros e Navegação
    pub photo_filters: super::photo_filters::PhotoFilters,
}

impl ApplicationState {
    pub fn new() -> Self {
        Self {
            current_view: CurrentView::Library,
            photos: Vec::new(),
            library_selected_id: None,
            develop_selected_id: None,
            editing_session: None,
            photo_filters: super::photo_filters::PhotoFilters::new(),
        }
    }
    
    pub fn set_view(&mut self, view: CurrentView) {
        self.current_view = view;
    }
    
    pub fn selected_photo_id(&self) -> Option<&String> {
        match self.current_view {
            CurrentView::Library => self.library_selected_id.as_ref(),
            CurrentView::Develop => self.develop_selected_id.as_ref(),
            CurrentView::Print => self.library_selected_id.as_ref(),
            _ => None,
        }
    }
}
