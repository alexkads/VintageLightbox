#![allow(dead_code)]

use egui::{Ui, RichText};
use crate::state::AppState;
use crate::design_system::theme::Theme;

pub struct Filmstrip;

impl Filmstrip {
    pub fn show(ui: &mut Ui, _state: &AppState) {
        ui.vertical_centered(|ui| {
            ui.add_space(Theme::SPACE_MD);
            ui.label(
                RichText::new("Filmstrip Placeholder")
                    .color(Theme::TEXT_MUTED)
                    .size(Theme::FONT_SM)
            );
        });
    }
}
