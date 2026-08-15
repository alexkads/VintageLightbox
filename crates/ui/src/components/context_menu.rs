//! Context Menu Component
//!
//! Reusable right-click context menu for photo actions.
//! Designed to be extensible for future functionality.

use crate::design_system::theme::Theme;
use egui::{Id, Response};

/// Represents an action in the context menu
#[derive(Clone)]
pub struct ContextMenuItem {
    pub label: String,
    pub icon: Option<String>,
    pub shortcut: Option<String>,
    pub enabled: bool,
    pub destructive: bool,
}

impl ContextMenuItem {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            icon: None,
            shortcut: None,
            enabled: true,
            destructive: false,
        }
    }

    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    pub fn with_shortcut(mut self, shortcut: &str) -> Self {
        self.shortcut = Some(shortcut.to_string());
        self
    }

    #[allow(dead_code)]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
}

/// Context menu state
pub struct ContextMenu {
    /// Whether the menu is currently visible
    pub is_open: bool,
    /// Position where the menu should appear
    pub position: egui::Pos2,
    /// ID to distinguish multiple menus
    id: Id,
    /// Time when the menu was opened (to prevent immediate closing)
    last_open_time: f64,
}

impl ContextMenu {
    pub fn new(id_source: &str) -> Self {
        Self {
            is_open: false,
            position: egui::Pos2::ZERO,
            id: Id::new(id_source),
            last_open_time: 0.0,
        }
    }

    /// Open the context menu at the specified position
    pub fn open(&mut self, ctx: &egui::Context, position: egui::Pos2) {
        self.is_open = true;
        self.position = position;
        self.last_open_time = ctx.input(|i| i.time);
    }

    /// Check if a response triggered a context menu (right-click)
    /// Note: Use open() if you need to manually trigger it
    pub fn check_open(&mut self, ctx: &egui::Context, response: &Response) {
        if response.secondary_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.open(ctx, pos);
            }
        }
    }

    /// Show the context menu with the given items
    /// Returns the index of the clicked item, if any
    pub fn show(&mut self, ctx: &egui::Context, items: &[ContextMenuItem]) -> Option<usize> {
        if !self.is_open {
            return None;
        }

        let mut clicked_index = None;

        // Close menu if clicked elsewhere
        if ctx.input(|i| i.pointer.any_click()) {
            let pointer_pos = ctx.input(|i| i.pointer.interact_pos());
            if let Some(_pos) = pointer_pos {
                // Will close after showing
            }
        }

        egui::Area::new(self.id)
            .fixed_pos(self.position)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::same(2))
                    .show(ui, |ui| {
                        ui.set_min_width(140.0);
                        ui.set_max_width(200.0); // Add max width to prevent explosion

                        for (index, item) in items.iter().enumerate() {
                            if item.label == "---" {
                                ui.separator();
                                continue;
                            }

                            let height = 22.0;
                            let (rect, response) = ui.allocate_exact_size(
                                egui::Vec2::new(ui.available_width(), height),
                                if item.enabled {
                                    egui::Sense::click()
                                } else {
                                    egui::Sense::hover()
                                },
                            );

                            // Draw hover background
                            if item.enabled && response.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    egui::CornerRadius::same(4),
                                    Theme::BG_HOVER, // Use standard hover color
                                );
                            }

                            // Layout content
                            let text_color = if item.destructive {
                                egui::Color32::from_rgb(255, 100, 100)
                            } else if item.enabled {
                                Theme::TEXT_PRIMARY
                            } else {
                                Theme::TEXT_MUTED
                            };

                            // Icon
                            let mut x_cursor = rect.min.x + 8.0;
                            if let Some(ref icon) = item.icon {
                                ui.painter().text(
                                    egui::pos2(x_cursor, rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    icon,
                                    egui::FontId::proportional(Theme::FONT_SM),
                                    text_color,
                                );
                                x_cursor += 20.0;
                            } else {
                                // Indent if no icon but others might have?
                                // For now simple left align
                            }

                            // Label
                            ui.painter().text(
                                egui::pos2(x_cursor, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &item.label,
                                egui::FontId::proportional(Theme::FONT_SM),
                                text_color,
                            );

                            // Shortcut (right aligned)
                            if let Some(ref shortcut) = item.shortcut {
                                ui.painter().text(
                                    egui::pos2(rect.max.x - 8.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    shortcut,
                                    egui::FontId::proportional(Theme::FONT_XS),
                                    Theme::TEXT_MUTED,
                                );
                            }

                            if item.enabled && response.clicked() {
                                clicked_index = Some(index);
                                self.is_open = false;
                            }
                        }
                    });
            });

        // Close if clicked outside or Escape pressed
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.is_open = false;
        }

        // Close if clicked outside the menu area
        // Ignore clicks that happened in the same frame/moment as opening
        let time_since_open = ctx.input(|i| i.time) - self.last_open_time;
        if time_since_open > 0.1 && ctx.input(|i| i.pointer.any_click()) && clicked_index.is_none()
        {
            // Delay close to next frame to avoid immediate re-open
            self.is_open = false;
        }

        clicked_index
    }

    /// Close the menu
    #[allow(dead_code)]
    pub fn close(&mut self) {
        self.is_open = false;
    }
}

impl Default for ContextMenu {
    fn default() -> Self {
        Self::new("context_menu")
    }
}
