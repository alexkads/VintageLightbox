use egui::{Align, Layout, RichText, Ui};

/// Actions triggered by the secondary window controls
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SecondaryWindowAction {
    Toggle,
    // Future: SetMode(SecondaryWindowMode),
}

/// Component for controlling the secondary window (toggle, view modes, etc.)
pub struct FilmstripSecondaryWindows;

impl FilmstripSecondaryWindows {
    /// Render the controls inside the given UI.
    /// Typically placed in a horizontal layout (e.g. toolbar).
    pub fn show(ui: &mut Ui) -> Option<SecondaryWindowAction> {
        let mut action = None;

        // Render aligned to the right, or just as a block if the parent handles layout?
        // The previous code used right_to_left. To make it reusable, maybe we should just render buttons
        // and let the parent decide layout direction?
        // However, the user wants it "like FilmstripFilter", which manages its own internal layout but is placed by parent.
        // Given the requirement "ao lado do Filter" and "compensates separate responsibilities",
        // the standard way here is to just render the widgets.
        // BUT, to keep the exact visual from before (far right), I should probably keep the right-to-left layout here
        // OR the parent `Filmstrip` should put it in a right-aligned container.

        // Let's stick to the previous successfully implemented layout for now within this component
        // to ensure it stays on the right side of the filmstrip bar.

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .button(RichText::new("🖥").size(14.0))
                .on_hover_text("Toggle Secondary Window (F)")
                .clicked()
            {
                action = Some(SecondaryWindowAction::Toggle);
            }
            // Separator to distinguish from other tools
            ui.separator();
        });

        action
    }
}
