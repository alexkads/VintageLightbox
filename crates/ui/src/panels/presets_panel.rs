use egui::{Ui, ScrollArea};
use crate::design_system::{
    theme::Theme,
    widgets::{self, PanelHeader},
};
use adapters::view_models::Preset;

pub struct PresetsPanel {
    system_presets: Vec<Preset>,
    user_presets: Vec<Preset>,
    system_presets_collapsed: bool,
    user_presets_collapsed: bool,
}

impl Default for PresetsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl PresetsPanel {
    pub fn new() -> Self {
        Self {
            system_presets: Vec::new(),
            user_presets: Vec::new(),
            system_presets_collapsed: false,
            user_presets_collapsed: false,
        }
    }

    pub fn set_presets(&mut self, system: Vec<Preset>, user: Vec<Preset>) {
        self.system_presets = system;
        self.user_presets = user;
    }

    pub fn ui(&mut self, ui: &mut Ui, _theme: &crate::design_system::theme_selector::ThemeVariant) -> Option<Preset> {
        let mut applied_preset = None;

        ui.add_space(Theme::SPACE_SM);
        widgets::section_title(ui, "PRESETS");
        ui.add_space(Theme::SPACE_SM);

        ScrollArea::vertical().show(ui, |ui| {
            // System Presets Section
            let mut header = PanelHeader::new(self.system_presets_collapsed);
            header.show(ui, "System Presets");
            self.system_presets_collapsed = header.collapsed;

            if !self.system_presets_collapsed {
                ui.indent("system_presets_indent", |ui| {
                    for preset in &self.system_presets {
                        if widgets::menu_item(ui, &preset.name, false).clicked() {
                           applied_preset = Some(preset.clone());
                        }
                    }
                });
            }

            ui.add_space(Theme::SPACE_MD);

            // User Presets Section
            let mut header = PanelHeader::new(self.user_presets_collapsed);
            header.show(ui, "User Presets");
            self.user_presets_collapsed = header.collapsed;

            if !self.user_presets_collapsed {
                 ui.indent("user_presets_indent", |ui| {
                    if self.user_presets.is_empty() {
                         widgets::label_text(ui, "No user presets");
                    } else {
                        for preset in &self.user_presets {
                             let response = widgets::menu_item(ui, &preset.name, false);
                             
                             if response.clicked() {
                                applied_preset = Some(preset.clone());
                            }
                            // Context menu for delete
                            response.context_menu(|ui| {
                                if ui.button("Delete").clicked() {
                                    // Handle delete logic via callback or event
                                    // For now just close menu
                                    ui.close_menu();
                                }
                            });
                        }
                    }
                });
            }
        });

        applied_preset
    }
}
