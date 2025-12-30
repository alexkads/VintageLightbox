use egui::Ui;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use crate::design_system::theme::Theme;
use adapters::view_models::FolderNode;

const ICON_FOLDER: &str = "📁";
const ICON_FOLDER_OPEN: &str = "📂";

// FolderNode struct and impl moved to adapters::view_models

pub struct FolderTree<'a> {
    pub selected_path: Option<&'a Path>,
    pub expanded_paths: &'a mut HashSet<String>,
}

impl<'a> FolderTree<'a> {
    pub fn new(
        selected_path: Option<&'a Path>, 
        expanded_paths: &'a mut HashSet<String>
    ) -> Self {
        Self {
            selected_path,
            expanded_paths,
        }
    }

    pub fn show(&mut self, ui: &mut Ui, roots: &[FolderNode]) -> Option<PathBuf> {
        let mut clicked_path = None;

        for node in roots {
            if let Some(clicked) = self.show_node(ui, node) {
                clicked_path = Some(clicked);
            }
        }

        clicked_path
    }

    fn show_node(&mut self, ui: &mut Ui, node: &FolderNode) -> Option<PathBuf> {
        let mut clicked_path = None;
        
        let path_str = node.path.to_string_lossy().to_string();
        let is_selected = self.selected_path.is_some_and(|p| p == node.path);
        let icon_color = if is_selected { Theme::ACCENT_PRIMARY } else { Theme::TEXT_MUTED };
        
        let id = ui.make_persistent_id(&path_str);
        
        let state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(), 
            id, 
            false
        );
        
        let is_open = state.is_open();

        let header_response = state.show_header(ui, |ui| {
            ui.horizontal(|ui| {
                // Folder Icon
                ui.label(
                     egui::RichText::new(if is_open { ICON_FOLDER_OPEN } else { ICON_FOLDER })
                        .color(icon_color)
                );
                
                // Name (Selectable)
                let text = format!("{} ({})", node.name, node.photo_count);
                let label = ui.selectable_label(is_selected, text);
                if label.clicked() {
                    clicked_path = Some(node.path.clone());
                }
            });
        });

        header_response.body(|ui| {
            for child in &node.children {
                if let Some(clicked) = self.show_node(ui, child) {
                   clicked_path = Some(clicked);
                }
            }
        });

        // Sync expansion state
        if is_open {
             self.expanded_paths.insert(path_str.clone());
        } else {
             self.expanded_paths.remove(&path_str);
        }

        clicked_path
    }
}


