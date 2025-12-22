// Slider Control Component
// Labeled slider with value display

use egui::{Ui, Slider};
use crate::design_system::theme::Theme;

#[allow(dead_code)]
pub struct SliderControl;

#[allow(dead_code)]
impl SliderControl {
    /// Show a labeled slider with value display
    pub fn show(
        ui: &mut Ui,
        label: &str,
        value: &mut f32,
        range: std::ops::RangeInclusive<f32>,
        step: f32,
    ) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(Theme::FONT_MD)
                    .color(Theme::TEXT_SECONDARY)
            );
            ui.add_space(Theme::SPACE_SM);
        });

        let response = ui.add(
            Slider::new(value, range)
                .step_by(step as f64)
                .show_value(true)
        );

        if response.changed() {
            changed = true;
        }

        changed
    }
}
