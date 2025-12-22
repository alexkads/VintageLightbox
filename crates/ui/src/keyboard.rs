// Keyboard Shortcut Handler
// Handles all keyboard shortcuts for the application

use egui::{Context, Key};
use crate::state::{AppState, CurrentView};
use std::sync::Arc;
use adapters::controllers::PhotoController;

pub struct KeyboardHandler;

impl KeyboardHandler {
    pub fn new() -> Self {
        Self
    }

    /// Handle keyboard input for the application
    pub fn handle_input(
        &self,
        ctx: &Context,
        state: &mut AppState,
        photo_controller: &Arc<PhotoController>,
    ) {
        ctx.input(|i| {
            // ==========================================
            // GLOBAL SHORTCUTS (Work in all views)
            // ==========================================

            // Escape - Return to Library view
            if i.key_pressed(Key::Escape) && state.current_view == CurrentView::Develop {
                state.current_view = CurrentView::Library;
                state.reset_viewer();
            }

            // Rating shortcuts (0-5)
            self.handle_rating_shortcuts(i, state, photo_controller);

            // Color label shortcuts (6-9 and 0 for none, though 0 is shared with unrate)
            self.handle_color_label_shortcuts(i, state, photo_controller);

            // Flag shortcuts (P, X, U)
            self.handle_flag_shortcuts(i, state, photo_controller);

            // ==========================================
            // DEVELOP VIEW SPECIFIC
            // ==========================================
            if state.current_view == CurrentView::Develop {
                // Backslash - Toggle Before/After view
                if i.key_pressed(Key::Backslash) {
                    state.show_before = !state.show_before;
                }

                // Undo/Redo shortcuts (Cmd+Z / Cmd+Shift+Z)
                let cmd_pressed = i.modifiers.command;
                let shift_pressed = i.modifiers.shift;

                if cmd_pressed && shift_pressed && i.key_pressed(Key::Z) {
                    // Redo
                    state.redo();
                } else if cmd_pressed && i.key_pressed(Key::Z) {
                    // Undo
                    state.undo();
                }

                // Navigation shortcuts (Arrow keys)
                if i.key_pressed(Key::ArrowRight) {
                    if let Some(new_id) = state.navigate_develop(1) {
                        state.develop_selected_photo_id = Some(new_id.clone());
                        state.loaded_photo_id = None; // Force reload
                    }
                }

                if i.key_pressed(Key::ArrowLeft) {
                    if let Some(new_id) = state.navigate_develop(-1) {
                        state.develop_selected_photo_id = Some(new_id.clone());
                        state.loaded_photo_id = None; // Force reload
                    }
                }
            }
        });
    }

    /// Helper to get target photo IDs based on current selection and view
    fn get_target_photos(&self, state: &AppState) -> Vec<String> {
        match state.current_view {
            CurrentView::Develop => {
                // In Develop, act on the currently loaded photo
                if let Some(id) = &state.develop_selected_photo_id {
                    vec![id.clone()]
                } else {
                    Vec::new()
                }
            }
            CurrentView::Library => {
                // In Library, act on multi-selection if exists, otherwise single selection
                if !state.selected_photo_ids.is_empty() {
                    state.selected_photo_ids.iter().cloned().collect()
                } else if let Some(id) = &state.library_selected_photo_id {
                    vec![id.clone()]
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// Handle rating keyboard shortcuts (0-5)
    fn handle_rating_shortcuts(
        &self,
        input: &egui::InputState,
        state: &mut AppState,
        photo_controller: &Arc<PhotoController>,
    ) {
        let rating_keys = [
            (Key::Num0, 0),
            (Key::Num1, 1),
            (Key::Num2, 2),
            (Key::Num3, 3),
            (Key::Num4, 4),
            (Key::Num5, 5),
        ];

        for (key, rating) in rating_keys {
            // Check both standard Num keys and Numpad keys if possible (egui maps them usually)
            if input.key_pressed(key) {
                let target_ids = self.get_target_photos(state);
                if target_ids.is_empty() { continue; }

                // Optimistically update UI state first
                for id in &target_ids {
                    // Update main list
                    if let Some(photo) = state.photos.iter_mut().find(|p| p.id == *id) {
                        photo.rating = rating;
                    }
                    // Update detail metadata if matched
                    if let Some(meta) = &mut state.detail_metadata {
                        if meta.id == *id {
                            meta.rating = rating;
                        }
                    }
                }

                // Perform async updates
                let controller = photo_controller.clone();
                let ids_clone = target_ids.clone();
                tokio::spawn(async move {
                    for id in ids_clone {
                        if let Err(e) = controller.rate_photo(&id, rating).await {
                            eprintln!("Failed to rate photo {}: {}", id, e);
                        }
                    }
                });
            }
        }
    }

    /// Handle color label keyboard shortcuts (6-9)
    fn handle_color_label_shortcuts(
        &self,
        input: &egui::InputState,
        state: &mut AppState,
        photo_controller: &Arc<PhotoController>,
    ) {
        let color_keys = [
            (Key::Num6, Some("Red")),
            (Key::Num7, Some("Yellow")),
            (Key::Num8, Some("Green")),
            (Key::Num9, Some("Blue")),
            // No shortcut for Purple in standard map
        ];

        for (key, color_opt) in color_keys {
            if input.key_pressed(key) {
                let target_ids = self.get_target_photos(state);
                if target_ids.is_empty() { continue; }

                let color_label = color_opt.map(|s| s.to_string());
                let color_str = color_label.clone().unwrap_or_default();

                // Optimistically update UI
                for id in &target_ids {
                    if let Some(photo) = state.photos.iter_mut().find(|p| p.id == *id) {
                        photo.color_label = color_label.clone();
                    }
                    if let Some(meta) = &mut state.detail_metadata {
                        if meta.id == *id {
                            meta.color_label = color_label.clone();
                        }
                    }
                }

                // Async update
                let controller = photo_controller.clone();
                let ids_clone = target_ids.clone();
                let label_clone = color_str.clone();
                
                tokio::spawn(async move {
                    for id in ids_clone {
                        if let Err(e) = controller.set_color_label(&id, &label_clone).await {
                            eprintln!("Failed to set color label for {}: {}", id, e);
                        }
                    }
                });
            }
        }
    }

    /// Handle flag shortcuts (P, X, U)
    fn handle_flag_shortcuts(
        &self,
        input: &egui::InputState,
        state: &mut AppState,
        photo_controller: &Arc<PhotoController>,
    ) {
        let flag_keys = [
            (Key::P, 1),  // Pick
            (Key::X, -1), // Reject
            (Key::U, 0),  // Unflag
        ];

        for (key, flag) in flag_keys {
            if input.key_pressed(key) {
                let target_ids = self.get_target_photos(state);
                if target_ids.is_empty() { continue; }

                // Optimistically update UI
                for id in &target_ids {
                    if let Some(photo) = state.photos.iter_mut().find(|p| p.id == *id) {
                        photo.flag = Some(flag);
                    }
                    // Detail metadata doesn't usually store flag in this app version yet?
                    // Checked PhotoViewModel: it has flag.
                    // Checked DetailMetadata: let's verify if it has flag.
                    // If DetailMetadata struct doesn't have flag, we can't update it there, but PhotoViewModel is enough for Grid/Filmstrip.
                }

                // Async update
                let controller = photo_controller.clone();
                let ids_clone = target_ids.clone();
                
                tokio::spawn(async move {
                    for id in ids_clone {
                        if let Err(e) = controller.set_flag(&id, flag).await {
                             eprintln!("Failed to set flag for {}: {}", id, e);
                        }
                    }
                });
                
                // If in Library view, we might want to auto-advance? (Lightroom feature, maybe later)
            }
        }
    }
}

impl Default for KeyboardHandler {
    fn default() -> Self {
        Self::new()
    }
}
