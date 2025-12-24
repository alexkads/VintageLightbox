// Settings Dialog Component
// Provides cache management and application settings

use egui::{Context, Window, Ui, RichText, Vec2, Button, ProgressBar};
// use infrastructure::cache::CacheStats;
use crate::state::AppState;
use crate::design_system::theme::Theme;
use crate::design_system::icons;

/// Actions that can be performed from the settings dialog
#[derive(Debug, Clone)]
pub enum SettingsAction {
    ClearThumbnails,
    ClearPreviews,
    ClearAllCache,
}

pub struct SettingsDialog;

impl SettingsDialog {
    /// Show the settings dialog
    /// Returns Some(action) when the user requests a cache action
    pub fn show(ctx: &Context, state: &mut AppState) -> Option<SettingsAction> {
        if !state.show_settings_dialog {
            return None;
        }

        let mut action: Option<SettingsAction> = None;
        let mut close_dialog = false;

        Window::new(format!("{} Settings", icons::ACTION_SETTINGS))
            .default_size(Vec2::new(500.0, 400.0))
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                // Tab bar (for future expansion)
                ui.horizontal(|ui| {
                    let _ = ui.selectable_label(true, "Cache & Previews");
                    // Future tabs: Theme, Shortcuts, etc.
                });

                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Cache Statistics Section
                    Self::show_cache_statistics(ui, state);

                    ui.add_space(Theme::SPACE_LG);

                    // Clear Cache Section
                    if let Some(requested_action) = Self::show_clear_cache_section(ui, state) {
                        action = Some(requested_action);
                    }

                    ui.add_space(Theme::SPACE_LG);

                    // Info Section
                    Self::show_info_section(ui);
                });

                ui.separator();

                // Close Button
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(Button::new("Close").min_size(Vec2::new(80.0, 28.0))).clicked() {
                            close_dialog = true;
                        }
                    });
                });
            });

        if close_dialog {
            state.show_settings_dialog = false;
        }

        action
    }

    /// Show cache statistics section
    fn show_cache_statistics(ui: &mut Ui, state: &AppState) {
        ui.heading(RichText::new("📊 Cache Statistics").color(ui.visuals().strong_text_color()));
        ui.add_space(Theme::SPACE_SM);

        egui::Frame::default()
            .fill(ui.visuals().window_fill())
            .corner_radius(Theme::RADIUS_SM)
            .inner_margin(egui::Margin::same(Theme::SPACE_MD as i8))
            .show(ui, |ui| {
                if let Some(stats) = &state.cache_stats {
                    egui::Grid::new("cache_stats_grid")
                        .num_columns(2)
                        .spacing([20.0, 8.0])
                        .show(ui, |ui| {
                            // Thumbnails
                            ui.label(RichText::new("Thumbnails:").color(ui.visuals().weak_text_color()));
                            ui.label(RichText::new(format!(
                                "{} items",
                                stats.thumbnail_count
                            )).color(ui.visuals().text_color()));
                            ui.end_row();

                            // Large Previews
                            ui.label(RichText::new("Large Previews:").color(ui.visuals().weak_text_color()));
                            ui.label(RichText::new(format!(
                                "{} items",
                                stats.large_preview_count
                            )).color(ui.visuals().text_color()));
                            ui.end_row();

                            // Total Size
                            ui.label(RichText::new("Total Size:").color(ui.visuals().weak_text_color()));
                            ui.label(RichText::new(
                                Self::format_bytes(stats.total_size_bytes)
                            ).color(ui.visuals().strong_text_color()).strong());
                            ui.end_row();

                            // Location
                            ui.label(RichText::new("Location:").color(ui.visuals().weak_text_color()));
                            ui.label(RichText::new(
                                stats.db_path.to_string_lossy().to_string()
                            ).color(ui.visuals().text_color()).size(Theme::FONT_XS));
                            ui.end_row();
                        });
                } else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Loading statistics...");
                    });
                }
            });
    }

    /// Show clear cache buttons section
    fn show_clear_cache_section(ui: &mut Ui, state: &AppState) -> Option<SettingsAction> {
        let mut action = None;

        ui.heading(RichText::new("🗑️ Clear Cache").color(ui.visuals().strong_text_color()));
        ui.add_space(Theme::SPACE_SM);

        egui::Frame::default()
            .fill(ui.visuals().window_fill())
            .corner_radius(Theme::RADIUS_SM)
            .inner_margin(egui::Margin::same(Theme::SPACE_MD as i8))
            .show(ui, |ui| {
                let stats = state.cache_stats.as_ref();
                let has_thumbnails = stats.map_or(false, |s| s.thumbnail_count > 0);
                let has_previews = stats.map_or(false, |s| s.large_preview_count > 0);
                let has_cache = has_thumbnails || has_previews;

                ui.horizontal(|ui| {
                    // Clear Thumbnails
                    let thumb_btn = ui.add_enabled(
                        has_thumbnails,
                        Button::new("Clear Thumbnails").min_size(Vec2::new(120.0, 28.0))
                    );
                    if thumb_btn.clicked() {
                        action = Some(SettingsAction::ClearThumbnails);
                    }

                    // Clear Previews
                    let preview_btn = ui.add_enabled(
                        has_previews,
                        Button::new("Clear Previews").min_size(Vec2::new(120.0, 28.0))
                    );
                    if preview_btn.clicked() {
                        action = Some(SettingsAction::ClearPreviews);
                    }

                    // Clear All
                    let all_btn = ui.add_enabled(
                        has_cache,
                        Button::new(RichText::new("Clear All").color(egui::Color32::from_rgb(255, 120, 120)))
                            .min_size(Vec2::new(100.0, 28.0))
                    );
                    if all_btn.clicked() {
                        action = Some(SettingsAction::ClearAllCache);
                    }
                });
            });

        action
    }

    /// Show info section with helpful tips
    fn show_info_section(ui: &mut Ui) {
        egui::Frame::default()
            .fill(ui.visuals().window_fill())
            .corner_radius(Theme::RADIUS_SM)
            .inner_margin(egui::Margin::same(Theme::SPACE_SM as i8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icons::ACTION_INFO).color(ui.visuals().strong_text_color()));
                    ui.label(RichText::new(
                        "Clearing cache will require regenerating previews when photos are next opened. \
                        This is safe and does not affect your original photos."
                    ).color(ui.visuals().weak_text_color()).size(Theme::FONT_SM));
                });
            });
    }

    /// Format bytes into human-readable format
    fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", bytes)
        }
    }
}

/// Show cache building progress in toolbar (called from app.rs)
pub fn show_cache_progress(ui: &mut Ui, state: &AppState) {
    if let Some(progress) = &state.cache_building_progress {
        let pct = if progress.total > 0 {
            progress.completed as f32 / progress.total as f32
        } else {
            0.0
        };

        ui.horizontal(|ui| {
            ui.add(ProgressBar::new(pct)
                .desired_width(150.0)
                .text(format!(
                    "Building {}... ({}/{})",
                    progress.preview_type,
                    progress.completed,
                    progress.total
                ))
            );
        });
    }
}
