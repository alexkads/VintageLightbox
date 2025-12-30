/// Import Dialogs
///
/// Dialogs for advanced import workflow:
/// - ImportPreviewDialog: Preview files before import with selection
/// - ImportProgressDialog: Show import progress with pause/cancel

use egui::{Context, Window, Vec2, ScrollArea, Button, ProgressBar, Color32, RichText};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use adapters::view_models::{ImportPreviewItemViewModel, ImportProgressViewModel};
use adapters::view_models::{ImportOptions, OrganizationStrategy, RenamePattern};

/// State for import preview dialog
#[derive(Default)]
pub struct ImportPreviewDialog {
    pub open: bool,
    pub items: Vec<ImportPreviewItemViewModel>,
    pub selected: Vec<bool>, // Selection state for each item
    pub duplicates: Vec<bool>, // Duplicate flags for each item
    pub organization: OrganizationStrategy,
    pub rename_pattern: RenamePattern,
    pub skip_duplicates: bool,
}

impl ImportPreviewDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            items: Vec::new(),
            selected: Vec::new(),
            duplicates: Vec::new(),
            organization: OrganizationStrategy::ByDate,
            rename_pattern: RenamePattern::Standard,
            skip_duplicates: true,
        }
    }

    /// Set items to preview
    pub fn set_items(&mut self, items: Vec<ImportPreviewItemViewModel>, duplicates: Vec<bool>) {
        self.selected = vec![true; items.len()]; // Select all by default
        self.duplicates = duplicates;
        self.items = items;
        self.open = true;
    }

    /// Get selected files
    pub fn get_selected_files(&self) -> Vec<String> {
        self.items.iter()
            .enumerate()
            .filter(|(i, _)| self.selected.get(*i).copied().unwrap_or(false))
            .map(|(_, item)| item.file_path.clone())
            .collect()
    }

    /// Get import options
    pub fn get_options(&self) -> ImportOptions {
        ImportOptions {
            organization: self.organization,
            rename_pattern: self.rename_pattern.clone(),
            skip_duplicates: self.skip_duplicates,
        }
    }

    /// Render the dialog
    pub fn show(&mut self, ctx: &Context) -> Option<ImportDialogAction> {
        if !self.open {
            return None;
        }

        let mut action: Option<ImportDialogAction> = None;

        Window::new("📥 Import Preview")
            .default_size(Vec2::new(800.0, 600.0))
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading(format!("{} files selected for import", self.items.len()));
                ui.separator();

                // Options panel
                ui.horizontal(|ui| {
                    ui.label("Organization:");
                    if ui.selectable_label(
                        matches!(self.organization, OrganizationStrategy::ByDate),
                        "By Date (YYYY/MM/DD)"
                    ).clicked() {
                        self.organization = OrganizationStrategy::ByDate;
                    }
                    if ui.selectable_label(
                        matches!(self.organization, OrganizationStrategy::PreserveStructure),
                        "Preserve Structure"
                    ).clicked() {
                        self.organization = OrganizationStrategy::PreserveStructure;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Rename:");
                    if ui.selectable_label(
                        matches!(self.rename_pattern, RenamePattern::Standard),
                        "Standard (photo-YYYY-MM-DD-001)"
                    ).clicked() {
                        self.rename_pattern = RenamePattern::Standard;
                    }
                    if ui.selectable_label(
                        matches!(self.rename_pattern, RenamePattern::KeepOriginal),
                        "Keep Original"
                    ).clicked() {
                        self.rename_pattern = RenamePattern::KeepOriginal;
                    }
                });

                ui.checkbox(&mut self.skip_duplicates, "Skip duplicates automatically");

                ui.separator();

                // Selection controls
                ui.horizontal(|ui| {
                    if ui.button("Select All").clicked() {
                        self.selected.iter_mut().for_each(|s| *s = true);
                    }
                    if ui.button("Deselect All").clicked() {
                        self.selected.iter_mut().for_each(|s| *s = false);
                    }
                    let selected_count = self.selected.iter().filter(|&&s| s).count();
                    ui.label(format!("{}/{} selected", selected_count, self.items.len()));
                });

                ui.separator();

                // Preview grid (simplified)
                ScrollArea::vertical().show(ui, |ui| {
                    for (i, item) in self.items.iter().enumerate() {
                        let is_duplicate = self.duplicates.get(i).copied().unwrap_or(false);
                        let is_selected = self.selected.get(i).copied().unwrap_or(false);

                        ui.horizontal(|ui| {
                            // Checkbox
                            let mut sel = is_selected;
                            if ui.checkbox(&mut sel, "").changed() {
                                if let Some(s) = self.selected.get_mut(i) {
                                    *s = sel;
                                }
                            }

                            // Duplicate badge
                            if is_duplicate {
                                ui.label(RichText::new("DUPLICATE").color(Color32::RED).strong());
                            }

                            // File info
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&item.file_path).strong());
                                ui.label(format!("{} | {} | {}",
                                    item.camera,
                                    item.date_time,
                                    format_file_size(item.file_size)
                                ));
                                if let Some(ref dims) = item.dimensions {
                                    ui.label(dims);
                                }
                                if item.is_raw {
                                    ui.label(RichText::new("RAW").color(Color32::LIGHT_BLUE));
                                }
                            });
                        });

                        ui.separator();
                    }
                });

                ui.separator();

                // Action buttons
                ui.horizontal(|ui| {
                    if ui.add(Button::new(RichText::new("Import").strong()).min_size(Vec2::new(100.0, 30.0))).clicked() {
                        action = Some(ImportDialogAction::Import);
                    }
                    if ui.add(Button::new("Cancel").min_size(Vec2::new(100.0, 30.0))).clicked() {
                        action = Some(ImportDialogAction::Cancel);
                    }
                });
            });

        // Close dialog if action was taken
        if action.is_some() {
            self.open = false;
        }

        action
    }
}

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
                self.add_log(format!("[{}/{}] Processing: {}", index + 1, self.total, path));
            }
            ImportProgressViewModel::Completed { photo_id: _, path } => {
                self.completed += 1;
                self.add_log(format!("✓ Completed: {}", path));
            }
            ImportProgressViewModel::Failed { path, error } => {
                self.failed += 1;
                self.add_log(format!("✗ Failed: {} - {}", path, error));
            }
            ImportProgressViewModel::DuplicateSkipped { path, existing_path: _ } => {
                self.skipped += 1;
                self.add_log(format!("⊘ Skipped (duplicate): {}", path));
            }
            ImportProgressViewModel::Paused { completed, remaining } => {
                self.status = ImportStatus::Paused;
                self.add_log(format!("⏸ Paused - {}/{} completed, {} remaining", completed, self.total, remaining));
            }
            ImportProgressViewModel::Finished { successful, failed, skipped } => {
                self.status = ImportStatus::Completed;
                self.completed = successful;
                self.failed = failed;
                self.skipped = skipped;
                self.add_log(format!("✓ Import finished: {} successful, {} failed, {} skipped", successful, failed, skipped));
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

                ui.add(ProgressBar::new(progress).text(
                    format!("{}/{} files ({:.0}%)",
                        self.completed + self.failed + self.skipped,
                        self.total,
                        progress * 100.0
                    )
                ));

                ui.separator();

                // Status
                ui.horizontal(|ui| {
                    let status_text = match self.status {
                        ImportStatus::Importing => RichText::new("Status: Importing...").color(Color32::LIGHT_BLUE),
                        ImportStatus::Paused => RichText::new("Status: Paused").color(Color32::YELLOW),
                        ImportStatus::Completed => RichText::new("Status: Completed").color(Color32::LIGHT_GREEN),
                        ImportStatus::Cancelled => RichText::new("Status: Cancelled").color(Color32::RED),
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
                        if ui.add(Button::new("Pause").min_size(Vec2::new(80.0, 25.0))).clicked() {
                            self.pause_flag.store(true, Ordering::Relaxed);
                            self.status = ImportStatus::Paused;
                        }
                    } else if self.status == ImportStatus::Paused && ui.add(Button::new("Resume").min_size(Vec2::new(80.0, 25.0))).clicked() {
                        self.pause_flag.store(false, Ordering::Relaxed);
                        self.status = ImportStatus::Importing;
                    }

                    if self.status != ImportStatus::Completed && self.status != ImportStatus::Cancelled && ui.add(Button::new("Cancel").min_size(Vec2::new(80.0, 25.0))).clicked() {
                        self.cancel_flag.store(true, Ordering::Relaxed);
                        self.status = ImportStatus::Cancelled;
                    }

                    if (self.status == ImportStatus::Completed || self.status == ImportStatus::Cancelled) && ui.add(Button::new("Close").min_size(Vec2::new(80.0, 25.0))).clicked() {
                        should_close = true;
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

/// Action result from import preview dialog
#[derive(Debug, Clone, PartialEq)]
pub enum ImportDialogAction {
    Import,
    Cancel,
}

/// Helper to format file sizes
fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
