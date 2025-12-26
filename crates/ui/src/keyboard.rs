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
        library_controller: &Arc<adapters::controllers::LibraryController>,
        _editor_controller: &Arc<adapters::controllers::EditorController>,
        _export_controller: &Arc<adapters::controllers::ExportController>,
        _import_controller: &Arc<adapters::controllers::ImportController>,
        photo_sender: &tokio::sync::mpsc::Sender<Result<Vec<adapters::view_models::PhotoViewModel>, String>>,
    ) {
        // ==========================================
        // NAVIGATION SHORTCUTS (Arrow keys) - Must be outside closure for Develop mode
        // ==========================================
        let arrow_right = ctx.input(|i| i.key_pressed(Key::ArrowRight));
        let arrow_left = ctx.input(|i| i.key_pressed(Key::ArrowLeft));

        if arrow_right || arrow_left {
            let direction = if arrow_right { 1 } else { -1 };

            match state.current_view {
                CurrentView::Library => {
                    if let Some(new_id) = state.navigate_library(direction) {
                        state.library_selected_photo_id = Some(new_id);
                        state.clear_selection();
                    }
                }
                CurrentView::Develop => {
                    let visible_photos = state.filmstrip_filter.apply(&state.photos);

                    if let Some(current_id) = &state.develop_selected_photo_id.clone() {
                        if let Some(current_pos) = visible_photos.iter().position(|p| &p.id == current_id) {
                            let new_pos = if direction > 0 {
                                (current_pos + 1).min(visible_photos.len().saturating_sub(1))
                            } else {
                                current_pos.saturating_sub(1)
                            };

                            if new_pos != current_pos {
                                if let Some(photo) = visible_photos.get(new_pos) {
                                    let new_id = photo.id.clone();
                                    // Update both IDs to keep Filmstrip and ImageViewer in sync
                                    state.develop_selected_photo_id = Some(new_id.clone());
                                    state.library_selected_photo_id = Some(new_id);
                                    state.loaded_photo_id = None;
                                    state.reset_viewer();
                                    ctx.request_repaint();
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

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
            self.handle_rating_shortcuts(i, state, photo_controller, library_controller, photo_sender);

            // Color label shortcuts (6-9 and 0 for none, though 0 is shared with unrate)
            self.handle_color_label_shortcuts(i, state, photo_controller, library_controller, photo_sender);

            // Flag shortcuts (P, X, U)
            self.handle_flag_shortcuts(i, state, photo_controller, library_controller, photo_sender);

            // Selection shortcuts (Cmd+A, Cmd+D) - Library only
            if state.current_view == CurrentView::Library {
                // Cmd+A: Select all
                if i.modifiers.command && i.key_pressed(Key::A) {
                    state.select_all();
                }

                // Cmd+D: Deselect all
                if i.modifiers.command && i.key_pressed(Key::D) {
                    state.clear_selection();
                }

                // Delete or Backspace: Show delete confirmation
                if i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace) {
                    if !state.selected_photo_ids.is_empty() {
                        state.show_delete_confirmation = true;
                    }
                }
            }

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
            CurrentView::Library | CurrentView::Print => {
                // In Library and Print, act on multi-selection if exists, otherwise single selection
                if !state.selected_photo_ids.is_empty() {
                    state.selected_photo_ids.iter().cloned().collect()
                } else if let Some(id) = &state.library_selected_photo_id {
                    vec![id.clone()]
                } else {
                    Vec::new()
                }
            }
            CurrentView::Import => Vec::new(),
        }
    }

    /// Handle rating keyboard shortcuts (0-5)
    fn handle_rating_shortcuts(
        &self,
        input: &egui::InputState,
        state: &mut AppState,
        photo_controller: &Arc<PhotoController>,
        library_controller: &Arc<adapters::controllers::LibraryController>,
        photo_sender: &tokio::sync::mpsc::Sender<Result<Vec<adapters::view_models::PhotoViewModel>, String>>,
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
                let lib_controller = library_controller.clone();
                let sender = photo_sender.clone();
                
                tokio::spawn(async move {
                    for id in ids_clone {
                        if let Err(e) = controller.rate_photo(&id, rating).await {
                            eprintln!("Failed to rate photo {}: {}", id, e);
                        }
                    }
                    // Force reload to sync state
                    if let Ok(photos) = lib_controller.get_all_photos().await {
                        let _ = sender.send(Ok(photos)).await;
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
        library_controller: &Arc<adapters::controllers::LibraryController>,
        photo_sender: &tokio::sync::mpsc::Sender<Result<Vec<adapters::view_models::PhotoViewModel>, String>>,
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

                let selected_color = color_opt.map(|s| s.to_string()).unwrap_or_default();
                
                // Check if we should toggle OFF (if all targets already have this color)
                // We handle case-insensitivity ("Red" vs "red")
                let all_already_have_color = target_ids.iter().all(|id| {
                    if let Some(photo) = state.photos.iter().find(|p| p.id == *id) {
                         match &photo.color_label {
                             Some(current) => current.eq_ignore_ascii_case(&selected_color),
                             None => false,
                         }
                    } else {
                        false
                    }
                });

                let (new_color_label, new_color_str) = if all_already_have_color {
                    (None, "".to_string())
                } else {
                    (Some(selected_color.clone()), selected_color)
                };

                // Optimistically update UI
                for id in &target_ids {
                    if let Some(photo) = state.photos.iter_mut().find(|p| p.id == *id) {
                        photo.color_label = new_color_label.clone();
                    }
                    if let Some(meta) = &mut state.detail_metadata {
                        if meta.id == *id {
                            meta.color_label = new_color_label.clone();
                        }
                    }
                }

                // Async update
                let controller = photo_controller.clone();
                let ids_clone = target_ids.clone();
                let label_clone = new_color_str;
                let lib_controller = library_controller.clone();
                let sender = photo_sender.clone();
                
                tokio::spawn(async move {
                    for id in ids_clone {
                        if let Err(e) = controller.set_color_label(&id, &label_clone).await {
                            eprintln!("Failed to set color label for {}: {}", id, e);
                        }
                    }
                    // Force reload to sync state
                    if let Ok(photos) = lib_controller.get_all_photos().await {
                        let _ = sender.send(Ok(photos)).await;
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
        library_controller: &Arc<adapters::controllers::LibraryController>,
        photo_sender: &tokio::sync::mpsc::Sender<Result<Vec<adapters::view_models::PhotoViewModel>, String>>,
    ) {
        let flag_keys = [
            (Key::P, 1),  // Pick
            (Key::X, -1), // Reject
            (Key::U, 0),  // Unflag (absolute)
        ];

        for (key, requested_flag) in flag_keys {
            if input.key_pressed(key) {
                let target_ids = self.get_target_photos(state);
                if target_ids.is_empty() { 
                    continue; 
                }

                // Collect the new flag state for each photo and trigger async updates
                let mut updates: Vec<(String, i32)> = Vec::new();

                // Optimistically update UI
                for id in &target_ids {
                    if let Some(photo) = state.photos.iter_mut().find(|p| p.id == *id) {
                        let current_flag = photo.flag.unwrap_or(0);
                        
                        // Toggle logic: If the requested flag is already set, toggle to 0 (Unflag).
                        // Unless the requested flag is 0 (Unflag shortcut), which always sets to 0.
                        let new_flag = if requested_flag != 0 && current_flag == requested_flag {
                            0 
                        } else {
                            requested_flag
                        };
                        
                        photo.flag = Some(new_flag);
                        updates.push((id.clone(), new_flag));
                    }
                }

                // Async update
                let controller = photo_controller.clone();
                let lib_controller = library_controller.clone();
                let sender = photo_sender.clone();
                
                if !updates.is_empty() {
                    tokio::spawn(async move {
                        for (id, flag) in updates {
                            if let Err(e) = controller.set_flag(&id, flag).await {
                                eprintln!("Failed to set flag for {}: {}", id, e);
                            }
                        }
                        // Force reload to sync state
                        if let Ok(photos) = lib_controller.get_all_photos().await {
                            let _ = sender.send(Ok(photos)).await;
                        }
                    });
                }
            }
        }
    }
}

impl Default for KeyboardHandler {
    fn default() -> Self {
        Self::new()
    }
}
