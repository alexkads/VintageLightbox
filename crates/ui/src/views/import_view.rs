use egui::{Ui, RichText};
use crate::state::AppState;
use adapters::controllers::ImportController;
use adapters::view_models::ImportSource;
use tokio::sync::mpsc;
use std::sync::Arc;

pub struct ImportView {}

impl ImportView {
    pub fn show(
        ctx: &egui::Context, 
        state: &mut AppState, 
        controller: &Arc<ImportController>,
        sender: &mpsc::Sender<(Vec<ImportSource>, Vec<ImportSource>)>
    ) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Top Bar
            ui.horizontal(|ui| {
                ui.heading("Import Photos");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        state.internal_state.current_view = crate::state::CurrentView::Library;
                    }
                });
            });
            ui.separator();

            // Main Content Area (Split: Sources | Grid | Options)
            egui::SidePanel::left("import_sources_panel")
                .resizable(true)
                .default_width(250.0)
                .show_inside(ui, |ui| {
                    Self::render_sources_panel(ui, state, controller, sender, ctx);
                });

            egui::SidePanel::right("import_options_panel")
                .resizable(true)
                .default_width(300.0)
                .show_inside(ui, |ui| {
                    Self::render_options_panel(ui, state);
                });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                Self::render_grid_panel(ui, state);
            });
            
            // Bottom Bar (Actions)
            egui::TopBottomPanel::bottom("import_actions_panel")
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                         ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let count = state.import_view_state.selected_files.len();
                            let label = format!("Import {} Photos", count);
                            if ui.add_enabled(count > 0, egui::Button::new(label)).clicked() {
                                // Trigger Import
                                // TODO: call controller
                                state.internal_state.current_view = crate::state::CurrentView::Library;
                            }
                         });
                    });
                });
        });
    }

    fn render_sources_panel(
        ui: &mut Ui, 
        state: &mut AppState, 
        controller: &Arc<ImportController>,
        sender: &mpsc::Sender<(Vec<ImportSource>, Vec<ImportSource>)>,
        ctx: &egui::Context
    ) {
        ui.label(RichText::new("FROM").strong());
        
        if state.import_view_state.devices.is_empty() && !state.is_busy {
             // Placeholder: Devices will be fetched when user clicks "Refresh Devices"
        }

        egui::CollapsingHeader::new("Devices")
            .default_open(true)
            .show(ui, |ui| {
                if ui.button("Refresh Devices").clicked() {
                    let controller = controller.clone();
                    let sender = sender.clone();
                    let ctx = ctx.clone();
                    
                    state.is_busy = true;
                    
                    tokio::spawn(async move {
                        let sources = controller.get_sources().await;
                        let _ = sender.send(sources).await;
                        ctx.request_repaint();
                    });
                }
                
                for source in &state.import_view_state.devices {
                    let is_selected = state.import_view_state.selected_source_id.as_ref() == Some(&source.id);
                    if ui.selectable_label(is_selected, &source.name).clicked() {
                        state.import_view_state.selected_source_id = Some(source.id.clone());
                        // TODO: Trigger loading files from this source
                    }
                }
            });

        egui::CollapsingHeader::new("Files")
            .default_open(false)
            .show(ui, |ui| {
                 if ui.button("Select Folder...").clicked() {
                     // Open folder picker
                 }
            });
    }

    fn render_grid_panel(ui: &mut Ui, state: &mut AppState) {
        if state.import_view_state.selected_source_id.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("Select a source to view photos");
            });
            return;
        }
        
        ui.label("Photos found:");
        // Placeholder grid
    }

    fn render_options_panel(ui: &mut Ui, state: &mut AppState) {
        ui.label(RichText::new("TO").strong());
        ui.label("My Catalog");
        
        ui.separator();
        
        ui.label(RichText::new("File Handling").strong());
        ui.checkbox(&mut state.import_view_state.options.skip_duplicates, "Don't Import Suspected Duplicates");
    }
}
