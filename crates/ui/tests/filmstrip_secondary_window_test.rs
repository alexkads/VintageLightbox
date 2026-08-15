use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use std::cell::Cell;
use std::rc::Rc;
use ui::components::filmstrip_secondary_windows::{
    FilmstripSecondaryWindows, SecondaryWindowAction,
};

#[test]
fn test_secondary_window_control_renders_and_clicks() {
    // Shared state to capture the action emitted by the component
    let last_action: Rc<Cell<Option<SecondaryWindowAction>>> = Rc::new(Cell::new(None));
    let last_action_clone = last_action.clone();

    let mut harness = Harness::new_ui(move |ui| {
        // Render the component
        if let Some(act) = FilmstripSecondaryWindows::show(ui) {
            last_action_clone.set(Some(act));
        }
    });

    // Initial render
    harness.run();

    // Verify default state (no action)
    assert_eq!(last_action.get(), None);

    // Find the toggle button by its icon text
    // Note: If this fails, we might need to verify exactly how RichText is handled by kittest,
    // or add semantic labels/IDs to the component.
    let btn = harness.get_by_label("🖥");

    // Simulate click
    btn.click();

    // Run frame to process click
    harness.run();

    // Verify action was emitted
    assert_eq!(last_action.get(), Some(SecondaryWindowAction::Toggle));
}
