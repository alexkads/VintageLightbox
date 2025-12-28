use egui::{Ui, ComboBox};
use domain::value_objects::AspectRatio;
use crate::design_system::icons;

/// Toolbar for crop controls in Develop mode
pub struct CropToolbar;

impl CropToolbar {
    pub fn show(
        ui: &mut Ui,
        selected_ratio: &mut AspectRatio,
        show_grid: &mut bool,
        on_rotate: &mut bool,
        on_flip_h: &mut bool,
        on_flip_v: &mut bool,
        on_reset: &mut bool,
        on_apply: &mut bool,
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            
            // Aspect Ratio Selector
            ui.label("Aspect:");
            ComboBox::from_id_salt("crop_aspect_ratio")
                .selected_text(selected_ratio.to_string())
                .show_ui(ui, |ui| {
                    ui.selectable_value(selected_ratio, AspectRatio::Original, AspectRatio::Original.to_string());
                    ui.selectable_value(selected_ratio, AspectRatio::Free, AspectRatio::Free.to_string());
                    ui.separator();
                    ui.selectable_value(selected_ratio, AspectRatio::Square, AspectRatio::Square.to_string());
                    ui.selectable_value(selected_ratio, AspectRatio::TwoThree, AspectRatio::TwoThree.to_string());
                    ui.selectable_value(selected_ratio, AspectRatio::ThreeTwo, AspectRatio::ThreeTwo.to_string());
                    ui.selectable_value(selected_ratio, AspectRatio::FourThree, AspectRatio::FourThree.to_string());
                    ui.selectable_value(selected_ratio, AspectRatio::ThreeFour, AspectRatio::ThreeFour.to_string());
                    ui.selectable_value(selected_ratio, AspectRatio::FourFive, AspectRatio::FourFive.to_string());
                    ui.selectable_value(selected_ratio, AspectRatio::FiveFour, AspectRatio::FiveFour.to_string());
                    ui.selectable_value(selected_ratio, AspectRatio::FiveSeven, AspectRatio::FiveSeven.to_string());
                    ui.selectable_value(selected_ratio, AspectRatio::SevenFive, AspectRatio::SevenFive.to_string());
                    ui.selectable_value(selected_ratio, AspectRatio::SixteenNine, AspectRatio::SixteenNine.to_string());
                    ui.selectable_value(selected_ratio, AspectRatio::NineSixteen, AspectRatio::NineSixteen.to_string());
                });

            ui.separator();

            // Rotate button (Swap Orientation)
            // Using a rotation with arrows indicating 90 degrees or orientation swap
            *on_rotate = ui.button(format!("{} Rotate", icons::ARROWS_CLOCKWISE))
                .on_hover_text("Rotate Crop Orientation (X)")
                .clicked();

            ui.separator();

            // Flip buttons
            *on_flip_h = ui.button("⇄ Flip H")
                .on_hover_text("Flip horizontal (H)")
                .clicked();

            *on_flip_v = ui.button("⇅ Flip V")
                .on_hover_text("Flip vertical (V)")
                .clicked();

            ui.separator();

            // Grid toggle
            ui.checkbox(show_grid, "Grid")
                .on_hover_text("Show composition grid (O)");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Apply button (Enter)
                if ui.button("✓ Apply")
                    .on_hover_text("Apply crop (Enter)")
                    .clicked()
                {
                    *on_apply = true;
                }

                // Reset button
                if ui.button("↺ Reset")
                    .on_hover_text("Reset to original")
                    .clicked()
                {
                    *on_reset = true;
                }
            });
        });
    }
}
