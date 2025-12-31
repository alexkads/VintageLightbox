// Tone Curve Editor Component
// Visualizes how adjustments affect the tone curve

use egui::Ui;
use egui_plot::{Plot, PlotPoints, Line};
use adapters::view_models::PhotoEdits;
use crate::design_system::theme::Theme;

pub struct ToneCurveEditor;

impl ToneCurveEditor {
    /// Show tone curve visualization
    /// Returns true if the curve was modified
    pub fn show(ui: &mut Ui, edits: &PhotoEdits) -> bool {
        let changed = false;

        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Tone Curve")
                    .size(Theme::FONT_SM)
                    .color(Theme::TEXT_MUTED)
            );

            ui.add_space(Theme::SPACE_XS);

            // Generate curve based on current adjustments
            let curve_points = Self::generate_curve_from_adjustments(edits);

            Plot::new("tone_curve_plot")
                .height(180.0)
                .width(ui.available_width().min(280.0))
                .data_aspect(1.0)
                .show_axes([true, true])
                .show_grid([true, true])
                .allow_zoom(false)
                .allow_drag(false)
                .allow_scroll(false)
                .show(ui, |plot_ui| {
                    // Draw diagonal baseline (no adjustment)
                    let baseline: PlotPoints = (0..=100)
                        .map(|i| {
                            let val = i as f64 / 100.0;
                            [val, val]
                        })
                        .collect();

                    plot_ui.line(
                        Line::new(baseline)
                            .color(egui::Color32::from_gray(100))
                            .name("No Adjustment")
                            .width(1.0)
                            .style(egui_plot::LineStyle::Dashed { length: 5.0 })
                    );

                    // Draw adjusted curve
                    plot_ui.line(
                        Line::new(curve_points)
                            .color(egui::Color32::from_rgb(137, 180, 250))
                            .name("Current Adjustment")
                            .width(2.0)
                    );
                });

            ui.add_space(Theme::SPACE_XS);
            ui.label(
                egui::RichText::new("Visualizes combined effect of all adjustments")
                    .size(Theme::FONT_XS)
                    .color(Theme::TEXT_HINT)
            );
        });

        changed
    }

    /// Generate curve points based on current adjustments
    /// This creates a simplified representation of how adjustments affect the tone curve
    fn generate_curve_from_adjustments(edits: &PhotoEdits) -> PlotPoints {
        let points: Vec<[f64; 2]> = (0..=100)
            .map(|i| {
                let input = i as f64 / 100.0;
                let mut output = input;

                // Apply exposure (affects entire curve)
                output += edits.exposure as f64 * 0.1;

                // Apply contrast (S-curve around midpoint)
                let contrast_factor = 1.0 + (edits.contrast as f64 - 1.0);
                output = (output - 0.5) * contrast_factor + 0.5;

                // Apply highlights (affects upper range)
                if input > 0.5 {
                    let highlight_factor = (input - 0.5) * 2.0; // 0 to 1 for upper half
                    output += edits.highlights as f64 * 0.05 * highlight_factor;
                }

                // Apply shadows (affects lower range)
                if input < 0.5 {
                    let shadow_factor = (0.5 - input) * 2.0; // 0 to 1 for lower half
                    output += edits.shadows as f64 * 0.05 * shadow_factor;
                }

                // Apply whites (affects extreme highlights)
                if input > 0.75 {
                    let white_factor = (input - 0.75) * 4.0; // 0 to 1 for top quarter
                    output += edits.whites as f64 * 0.03 * white_factor;
                }

                // Apply blacks (affects extreme shadows)
                if input < 0.25 {
                    let black_factor = (0.25 - input) * 4.0; // 0 to 1 for bottom quarter
                    output += edits.blacks as f64 * 0.03 * black_factor;
                }

                // Clamp output to valid range
                output = output.clamp(0.0, 1.0);

                [input, output]
            })
            .collect();

        points.into()
    }
}
