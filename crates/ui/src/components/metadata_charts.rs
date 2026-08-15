// Metadata Charts Component
// Visualizes photo library statistics

use crate::design_system::theme::Theme;
use adapters::view_models::PhotoViewModel;
use egui::Ui;
use egui_plot::{Bar, BarChart, Plot};

pub struct MetadataCharts;

impl MetadataCharts {
    /// Show metadata visualization charts
    pub fn show(ui: &mut Ui, photos: &[PhotoViewModel]) {
        if photos.is_empty() {
            ui.label(
                egui::RichText::new("No photos to analyze")
                    .size(Theme::FONT_XS)
                    .color(Theme::TEXT_MUTED),
            );
            return;
        }

        ui.vertical(|ui| {
            // Rating distribution chart
            Self::show_rating_distribution(ui, photos);

            ui.add_space(Theme::SPACE_MD);

            // Camera usage chart
            Self::show_camera_usage(ui, photos);
        });
    }

    /// Show rating distribution as bar chart
    fn show_rating_distribution(ui: &mut Ui, photos: &[PhotoViewModel]) {
        ui.label(
            egui::RichText::new("Rating Distribution")
                .size(Theme::FONT_SM)
                .color(Theme::TEXT_MUTED),
        );

        ui.add_space(Theme::SPACE_XS);

        // Count photos by rating
        let mut rating_counts = [0usize; 6]; // 0-5 stars
        for photo in photos {
            let rating = photo.rating.clamp(0, 5) as usize;
            rating_counts[rating] += 1;
        }

        // Create bar chart
        let bars: Vec<Bar> = rating_counts
            .iter()
            .enumerate()
            .map(|(rating, &count)| {
                Bar::new(rating as f64, count as f64)
                    .width(0.8)
                    .fill(egui::Color32::from_rgb(255, 215, 0)) // Gold color for ratings
            })
            .collect();

        Plot::new("rating_distribution")
            .height(120.0)
            .show_axes([true, true])
            .allow_zoom(false)
            .allow_drag(false)
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(BarChart::new(bars).name("Photos"));
            });

        ui.label(
            egui::RichText::new(format!("Total: {} photos", photos.len()))
                .size(Theme::FONT_XS)
                .color(Theme::TEXT_HINT),
        );
    }

    /// Show top cameras used
    fn show_camera_usage(ui: &mut Ui, photos: &[PhotoViewModel]) {
        ui.label(
            egui::RichText::new("Top Cameras")
                .size(Theme::FONT_SM)
                .color(Theme::TEXT_MUTED),
        );

        ui.add_space(Theme::SPACE_XS);

        // Count photos by camera
        use std::collections::HashMap;
        let mut camera_counts: HashMap<String, usize> = HashMap::new();
        for photo in photos {
            if !photo.camera.is_empty() && photo.camera != "Unknown" {
                *camera_counts.entry(photo.camera.clone()).or_insert(0) += 1;
            }
        }

        if camera_counts.is_empty() {
            ui.label(
                egui::RichText::new("No camera metadata available")
                    .size(Theme::FONT_XS)
                    .color(Theme::TEXT_HINT),
            );
            return;
        }

        // Get top 5 cameras
        let mut cameras: Vec<_> = camera_counts.into_iter().collect();
        cameras.sort_by(|a, b| b.1.cmp(&a.1));
        cameras.truncate(5);

        // Create bars
        let bars: Vec<Bar> = cameras
            .iter()
            .enumerate()
            .map(|(i, (_camera, count))| {
                Bar::new(i as f64, *count as f64)
                    .width(0.8)
                    .fill(egui::Color32::from_rgb(137, 180, 250)) // Blue
            })
            .collect();

        Plot::new("camera_usage")
            .height(100.0)
            .show_axes([false, true])
            .allow_zoom(false)
            .allow_drag(false)
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(BarChart::new(bars).name("Photos"));
            });

        // Show camera names below chart
        ui.add_space(Theme::SPACE_XS);
        for (camera, count) in cameras.iter().take(3) {
            ui.label(
                egui::RichText::new(format!("{}: {} photos", camera, count))
                    .size(Theme::FONT_XS)
                    .color(Theme::TEXT_HINT),
            );
        }
    }
}
