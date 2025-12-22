// Interactive Histogram Component using egui_plot
// Provides zoom, pan, and hover tooltips

use egui::Ui;
use egui_plot::{Plot, PlotPoints, Line, Legend};
use crate::components::histogram::HistogramData;
use crate::design_system::theme::Theme;

#[allow(dead_code)]
pub struct HistogramPlot;

#[allow(dead_code)]
impl HistogramPlot {
    /// Show interactive histogram widget with zoom and hover
    pub fn show(ui: &mut Ui, histogram_data: Option<&HistogramData>) {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Histogram (Interactive)")
                    .size(Theme::FONT_SM)
                    .color(Theme::TEXT_MUTED)
            );

            ui.add_space(Theme::SPACE_XS);

            if let Some(data) = histogram_data {
                Self::render_interactive_histogram(ui, data);
            } else {
                // Placeholder when no image is loaded
                ui.label(
                    egui::RichText::new("No image loaded")
                        .size(Theme::FONT_XS)
                        .color(Theme::TEXT_MUTED)
                );
            }
        });
    }

    /// Render interactive histogram using egui_plot
    fn render_interactive_histogram(ui: &mut Ui, data: &HistogramData) {
        // Convert histogram data to plot points
        let red_points: PlotPoints = (0..256)
            .map(|i| {
                let normalized = data.red[i] as f64 / data.max_value as f64;
                [i as f64, normalized]
            })
            .collect();

        let green_points: PlotPoints = (0..256)
            .map(|i| {
                let normalized = data.green[i] as f64 / data.max_value as f64;
                [i as f64, normalized]
            })
            .collect();

        let blue_points: PlotPoints = (0..256)
            .map(|i| {
                let normalized = data.blue[i] as f64 / data.max_value as f64;
                [i as f64, normalized]
            })
            .collect();

        Plot::new("histogram_plot")
            .height(100.0)
            .legend(Legend::default())
            .show_axes([false, false])
            .show_grid([false, false])
            .allow_zoom(true)
            .allow_drag(true)
            .allow_scroll(true)
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(red_points)
                        .color(egui::Color32::from_rgb(255, 80, 80))
                        .name("Red")
                        .width(1.0)
                );

                plot_ui.line(
                    Line::new(green_points)
                        .color(egui::Color32::from_rgb(80, 255, 80))
                        .name("Green")
                        .width(1.0)
                );

                plot_ui.line(
                    Line::new(blue_points)
                        .color(egui::Color32::from_rgb(80, 80, 255))
                        .name("Blue")
                        .width(1.0)
                );
            });
    }
}
