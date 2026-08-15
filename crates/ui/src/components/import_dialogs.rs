//! Import Progress Dialog
//!
//! Acompanha uma importação em andamento: barra, contadores, log e os controles de
//! pausar/cancelar.
//!
//! A escolha do que importar não mora aqui — mora na tela de importação
//! (`views::import_view`), que mostra as fotos em miniatura antes de qualquer decisão.

use adapters::view_models::ImportProgressViewModel;
use egui::{Button, Color32, Context, ProgressBar, RichText, ScrollArea, Vec2, Window};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// State for import progress dialog
pub struct ImportProgressDialog {
    pub open: bool,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub current_file: Option<String>,
    pub status: ImportStatus,
    pub log: Vec<String>, // Event log (last 20 entries)
    pub pause_flag: Arc<AtomicBool>,
    pub cancel_flag: Arc<AtomicBool>,
    pub progress_receiver: Option<mpsc::UnboundedReceiver<ImportProgressViewModel>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportStatus {
    Importing,
    Paused,
    Completed,
    Cancelled,
}

impl ImportProgressDialog {
    pub fn new(
        total: usize,
        pause_flag: Arc<AtomicBool>,
        cancel_flag: Arc<AtomicBool>,
        progress_receiver: mpsc::UnboundedReceiver<ImportProgressViewModel>,
    ) -> Self {
        Self {
            open: true,
            total,
            completed: 0,
            failed: 0,
            skipped: 0,
            current_file: None,
            status: ImportStatus::Importing,
            log: Vec::new(),
            pause_flag,
            cancel_flag,
            progress_receiver: Some(progress_receiver),
        }
    }

    /// Process incoming progress events
    pub fn update(&mut self) {
        // Collect all pending events first to avoid borrow checker issues
        let mut events = Vec::new();
        if let Some(receiver) = &mut self.progress_receiver {
            while let Ok(event) = receiver.try_recv() {
                events.push(event);
            }
        }

        // Process collected events
        for event in events {
            self.process_event(event);
        }
    }

    fn process_event(&mut self, event: ImportProgressViewModel) {
        match event {
            ImportProgressViewModel::Starting { total } => {
                self.total = total;
                self.add_log(format!("Starting import of {} files...", total));
            }
            ImportProgressViewModel::Processing { index, path } => {
                self.current_file = Some(path.clone());
                self.add_log(format!(
                    "[{}/{}] Processing: {}",
                    index + 1,
                    self.total,
                    path
                ));
            }
            ImportProgressViewModel::Completed { photo_id: _, path } => {
                self.completed += 1;
                self.add_log(format!("✓ Completed: {}", path));
            }
            ImportProgressViewModel::Failed { path, error } => {
                self.failed += 1;
                self.add_log(format!("✗ Failed: {} - {}", path, error));
            }
            ImportProgressViewModel::DuplicateSkipped {
                path,
                existing_path: _,
            } => {
                self.skipped += 1;
                self.add_log(format!("⊘ Skipped (duplicate): {}", path));
            }
            ImportProgressViewModel::Paused {
                completed,
                remaining,
            } => {
                self.status = ImportStatus::Paused;
                self.add_log(format!(
                    "⏸ Paused - {}/{} completed, {} remaining",
                    completed, self.total, remaining
                ));
            }
            ImportProgressViewModel::Finished {
                successful,
                failed,
                skipped,
            } => {
                self.status = ImportStatus::Completed;
                self.completed = successful;
                self.failed = failed;
                self.skipped = skipped;
                self.add_log(format!(
                    "✓ Import finished: {} successful, {} failed, {} skipped",
                    successful, failed, skipped
                ));
            }
        }
    }

    fn add_log(&mut self, message: String) {
        self.log.push(message);
        if self.log.len() > 20 {
            self.log.remove(0);
        }
    }

    /// Render the dialog
    pub fn show(&mut self, ctx: &Context) -> bool {
        if !self.open {
            return false;
        }

        // Update progress
        self.update();

        let mut should_close = false;

        Window::new("⏳ Importing Photos")
            .default_size(Vec2::new(600.0, 400.0))
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                // Progress bar
                let progress = if self.total > 0 {
                    (self.completed + self.failed + self.skipped) as f32 / self.total as f32
                } else {
                    0.0
                };

                ui.add(ProgressBar::new(progress).text(format!(
                    "{}/{} files ({:.0}%)",
                    self.completed + self.failed + self.skipped,
                    self.total,
                    progress * 100.0
                )));

                ui.separator();

                // Status
                ui.horizontal(|ui| {
                    let status_text = match self.status {
                        ImportStatus::Importing => {
                            RichText::new("Status: Importing...").color(Color32::LIGHT_BLUE)
                        }
                        ImportStatus::Paused => {
                            RichText::new("Status: Paused").color(Color32::YELLOW)
                        }
                        ImportStatus::Completed => {
                            RichText::new("Status: Completed").color(Color32::LIGHT_GREEN)
                        }
                        ImportStatus::Cancelled => {
                            RichText::new("Status: Cancelled").color(Color32::RED)
                        }
                    };
                    ui.label(status_text.strong());
                });

                if let Some(ref file) = self.current_file {
                    ui.label(format!("Current: {}", file));
                }

                ui.separator();

                // Summary
                ui.horizontal(|ui| {
                    ui.label(format!("✓ Successful: {}", self.completed));
                    ui.label(format!("✗ Failed: {}", self.failed));
                    ui.label(format!("⊘ Skipped: {}", self.skipped));
                });

                ui.separator();

                // Event log
                ui.label(RichText::new("Event Log:").strong());
                ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    for entry in &self.log {
                        ui.label(entry);
                    }
                });

                ui.separator();

                // Controls
                ui.horizontal(|ui| {
                    if self.status == ImportStatus::Importing {
                        if ui
                            .add(Button::new("Pause").min_size(Vec2::new(80.0, 25.0)))
                            .clicked()
                        {
                            self.pause_flag.store(true, Ordering::Relaxed);
                            self.status = ImportStatus::Paused;
                        }
                    } else if self.status == ImportStatus::Paused {
                        if ui
                            .add(Button::new("Resume").min_size(Vec2::new(80.0, 25.0)))
                            .clicked()
                        {
                            self.pause_flag.store(false, Ordering::Relaxed);
                            self.status = ImportStatus::Importing;
                        }
                    }

                    if self.status != ImportStatus::Completed
                        && self.status != ImportStatus::Cancelled
                    {
                        if ui
                            .add(Button::new("Cancel").min_size(Vec2::new(80.0, 25.0)))
                            .clicked()
                        {
                            self.cancel_flag.store(true, Ordering::Relaxed);
                            self.status = ImportStatus::Cancelled;
                        }
                    }

                    if self.status == ImportStatus::Completed
                        || self.status == ImportStatus::Cancelled
                    {
                        if ui
                            .add(Button::new("Close").min_size(Vec2::new(80.0, 25.0)))
                            .clicked()
                        {
                            should_close = true;
                        }
                    }
                });
            });

        // Close dialog if requested
        if should_close {
            self.open = false;
        }

        self.open
    }
}
