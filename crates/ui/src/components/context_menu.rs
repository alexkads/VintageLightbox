//! Context Menu Component
//!
//! Reusable right-click context menu for photo actions.
//! Designed to be extensible for future functionality.

use egui::{Response, Id};
use crate::design_system::theme::Theme;

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
}

impl ContextMenu {
    pub fn new(id_source: &str) -> Self {
        Self {
            is_open: false,
            position: egui::Pos2::ZERO,
            id: Id::new(id_source),
        }
    }
    
    /// Check if a response triggered a context menu (right-click)
    pub fn check_open(&mut self, response: &Response) {
        if response.secondary_clicked() {
            self.is_open = true;
            if let Some(pos) = response.interact_pointer_pos() {
                self.position = pos;
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
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        ui.set_min_width(180.0);
                        
                        for (index, item) in items.iter().enumerate() {
                            if item.label == "---" {
                                ui.separator();
                                continue;
                            }
                            
                            let text_color = if item.destructive {
                                egui::Color32::from_rgb(255, 100, 100)
                            } else if item.enabled {
                                Theme::TEXT_PRIMARY
                            } else {
                                Theme::TEXT_MUTED
                            };
                            
                            ui.horizontal(|ui| {
                                // Icon
                                if let Some(ref icon) = item.icon {
                                    ui.label(egui::RichText::new(icon).color(text_color));
                                }
                                
                                // Label
                                let label = egui::RichText::new(&item.label)
                                    .size(Theme::FONT_SM)
                                    .color(text_color);
                                
                                let response = if item.enabled {
                                    ui.add(egui::Label::new(label).sense(egui::Sense::click()))
                                } else {
                                    ui.label(label)
                                };
                                
                                // Fill remaining space
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if let Some(ref shortcut) = item.shortcut {
                                        ui.label(
                                            egui::RichText::new(shortcut)
                                                .size(Theme::FONT_XS)
                                                .color(Theme::TEXT_MUTED)
                                        );
                                    }
                                });
                                
                                if item.enabled && response.clicked() {
                                    clicked_index = Some(index);
                                    self.is_open = false;
                                }
                            });
                        }
                    });
            });
        
        // Close if clicked outside or Escape pressed
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.is_open = false;
        }
        
        // Close if clicked outside the menu area
        if ctx.input(|i| i.pointer.any_click()) && clicked_index.is_none() {
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
