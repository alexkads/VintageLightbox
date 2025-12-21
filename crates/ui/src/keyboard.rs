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
            // Global shortcuts (work in any view)

            // Escape - Return to Library view
            if i.key_pressed(Key::Escape) && state.current_view == CurrentView::Develop {
                state.current_view = CurrentView::Library;
                state.reset_viewer();
            }

            // Only handle develop view shortcuts when in develop mode
            if state.current_view != CurrentView::Develop {
                return;
            }

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

            // Navigation shortcuts (Arrow keys) - only in Develop view
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

            // Rating shortcuts (0-5)
            self.handle_rating_shortcuts(i, state, photo_controller);

            // Color label shortcuts (6-9 and 0 for none)
            self.handle_color_label_shortcuts(i, state, photo_controller);
        });
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
            if input.key_pressed(key) {
                if let Some(metadata) = &mut state.detail_metadata {
                    metadata.rating = rating;

                    // Update via controller
                    let controller = photo_controller.clone();
                    let photo_id = metadata.id.clone();
                    tokio::spawn(async move {
                        if let Err(e) = controller.rate_photo(&photo_id, rating).await {
                            eprintln!("Failed to rate photo: {}", e);
                        }
                    });

                    // Also update in photos list
                    if let Some(photo) = state.photos.iter_mut().find(|p| p.id == metadata.id) {
                        photo.rating = rating;
                    }
                }
            }
        }
    }

    /// Handle color label keyboard shortcuts (6-9 for colors, 0 to remove)
    fn handle_color_label_shortcuts(
        &self,
        input: &egui::InputState,
        state: &mut AppState,
        photo_controller: &Arc<PhotoController>,
    ) {
        let color_keys = [
            (Key::Num6, Some("red")),
            (Key::Num7, Some("yellow")),
            (Key::Num8, Some("green")),
            (Key::Num9, Some("blue")),
            // Note: Purple doesn't have a number key in the original
        ];

        for (key, color) in color_keys {
            if input.key_pressed(key) {
                if let Some(metadata) = &mut state.detail_metadata {
                    metadata.color_label = color.map(|s| s.to_string());

                    // Update via controller
                    let controller = photo_controller.clone();
                    let photo_id = metadata.id.clone();
                    let color_str = color.unwrap_or("").to_string();

                    tokio::spawn(async move {
                        if let Err(e) = controller.set_color_label(&photo_id, &color_str).await {
                            eprintln!("Failed to set color label: {}", e);
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
