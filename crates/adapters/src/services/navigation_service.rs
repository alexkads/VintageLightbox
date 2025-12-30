//! # Navigation Service
//!
//! Lógica de navegação entre fotos e views,
//! extraída da camada UI para ser independente de framework.

use crate::state::{ApplicationState, CurrentView};

/// Serviço de navegação da aplicação
pub struct NavigationService;

impl NavigationService {
    /// Cria uma nova instância do serviço
    pub fn new() -> Self {
        Self
    }

    /// Navega para a próxima foto na Develop view
    /// Retorna o ID da nova foto se a navegação foi bem-sucedida
    pub fn navigate_next(&self, state: &mut ApplicationState) -> Option<String> {
        self.navigate_develop(state, 1)
    }

    /// Navega para a foto anterior na Develop view
    /// Retorna o ID da nova foto se a navegação foi bem-sucedida
    pub fn navigate_previous(&self, state: &mut ApplicationState) -> Option<String> {
        self.navigate_develop(state, -1)
    }

    /// Navega na Develop view (direção: 1 = próximo, -1 = anterior)
    fn navigate_develop(&self, state: &mut ApplicationState, direction: i32) -> Option<String> {
        let current_id = state.develop_selected_id.as_ref()?;
        let current_pos = state.photos.iter().position(|p| &p.id == current_id)?;

        let new_pos = if direction > 0 {
            (current_pos + 1).min(state.photos.len().saturating_sub(1))
        } else {
            current_pos.saturating_sub(1)
        };

        if new_pos != current_pos {
            state.photos.get(new_pos).map(|p| p.id.clone())
        } else {
            None
        }
    }

    /// Navega para a próxima foto na Library view
    pub fn navigate_library_next(&self, state: &mut ApplicationState) -> Option<String> {
        self.navigate_library(state, 1)
    }

    /// Navega para a foto anterior na Library view
    pub fn navigate_library_previous(&self, state: &mut ApplicationState) -> Option<String> {
        self.navigate_library(state, -1)
    }

    /// Navega na Library view
    fn navigate_library(&self, state: &mut ApplicationState, direction: i32) -> Option<String> {
        let current_id = state.library_selected_id.as_ref()?;
        let current_pos = state.photos.iter().position(|p| &p.id == current_id)?;

        let new_pos = if direction > 0 {
            (current_pos + 1).min(state.photos.len().saturating_sub(1))
        } else {
            current_pos.saturating_sub(1)
        };

        if new_pos != current_pos {
            let new_id = state.photos.get(new_pos)?.id.clone();
            state.library_selected_id = Some(new_id.clone());
            Some(new_id)
        } else {
            None
        }
    }

    /// Muda para uma view específica
    pub fn switch_view(&self, state: &mut ApplicationState, view: CurrentView) {
        // Ao ir para Develop, copiar a seleção da Library se não houver
        if view == CurrentView::Develop && state.develop_selected_id.is_none() {
            state.develop_selected_id = state.library_selected_id.clone();
        }

        state.current_view = view;
    }

    /// Seleciona uma foto específica na view atual
    pub fn select_photo(&self, state: &mut ApplicationState, photo_id: String) {
        match state.current_view {
            CurrentView::Library | CurrentView::Print => {
                state.library_selected_id = Some(photo_id);
            }
            CurrentView::Develop => {
                state.develop_selected_id = Some(photo_id);
            }
            CurrentView::Import => {}
        }
    }

    /// Seleciona uma foto e vai para Develop mode
    pub fn open_in_develop(&self, state: &mut ApplicationState, photo_id: String) {
        state.develop_selected_id = Some(photo_id);
        state.current_view = CurrentView::Develop;
    }

    /// Retorna para Library mode
    pub fn back_to_library(&self, state: &mut ApplicationState) {
        // Sincroniza a seleção
        if let Some(dev_id) = &state.develop_selected_id {
            state.library_selected_id = Some(dev_id.clone());
        }
        state.current_view = CurrentView::Library;
    }
}

impl Default for NavigationService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::PhotoViewModel;

    fn create_test_photos(count: usize) -> Vec<PhotoViewModel> {
        (0..count)
            .map(|i| PhotoViewModel {
                id: format!("photo-{}", i),
                name: format!("Photo {}", i),
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn test_navigate_next() {
        let service = NavigationService::new();
        let mut state = ApplicationState::new();
        state.photos = create_test_photos(5);
        state.develop_selected_id = Some("photo-2".to_string());

        let result = service.navigate_next(&mut state);
        
        assert_eq!(result, Some("photo-3".to_string()));
    }

    #[test]
    fn test_navigate_previous() {
        let service = NavigationService::new();
        let mut state = ApplicationState::new();
        state.photos = create_test_photos(5);
        state.develop_selected_id = Some("photo-2".to_string());

        let result = service.navigate_previous(&mut state);
        
        assert_eq!(result, Some("photo-1".to_string()));
    }

    #[test]
    fn test_navigate_at_bounds() {
        let service = NavigationService::new();
        let mut state = ApplicationState::new();
        state.photos = create_test_photos(3);
        
        // No início
        state.develop_selected_id = Some("photo-0".to_string());
        assert!(service.navigate_previous(&mut state).is_none());

        // No fim
        state.develop_selected_id = Some("photo-2".to_string());
        assert!(service.navigate_next(&mut state).is_none());
    }

    #[test]
    fn test_switch_view_syncs_selection() {
        let service = NavigationService::new();
        let mut state = ApplicationState::new();
        state.library_selected_id = Some("photo-1".to_string());
        state.develop_selected_id = None;

        service.switch_view(&mut state, CurrentView::Develop);

        assert_eq!(state.current_view, CurrentView::Develop);
        assert_eq!(state.develop_selected_id, Some("photo-1".to_string()));
    }

    #[test]
    fn test_open_in_develop() {
        let service = NavigationService::new();
        let mut state = ApplicationState::new();

        service.open_in_develop(&mut state, "photo-5".to_string());

        assert_eq!(state.current_view, CurrentView::Develop);
        assert_eq!(state.develop_selected_id, Some("photo-5".to_string()));
    }

    #[test]
    fn test_back_to_library_syncs_selection() {
        let service = NavigationService::new();
        let mut state = ApplicationState::new();
        state.current_view = CurrentView::Develop;
        state.develop_selected_id = Some("photo-3".to_string());

        service.back_to_library(&mut state);

        assert_eq!(state.current_view, CurrentView::Library);
        assert_eq!(state.library_selected_id, Some("photo-3".to_string()));
    }
}
