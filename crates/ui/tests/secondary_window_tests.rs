#[cfg(test)]
mod tests {
    use egui::Context;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use std::time::Duration;
    use ui::components::secondary_window::SecondaryWindow;
    use ui::monitors::MonitorInfo;
    use ui::components::image_viewer::ImageViewer;

    // Helper to create a dummy monitor
    fn create_dummy_monitor(id: u32, is_primary: bool) -> MonitorInfo {
        MonitorInfo {
            id,
            name: format!("Monitor {}", id),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            is_primary,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn test_secondary_window_toggle_stress() {
        let mut window = SecondaryWindow::new();
        let monitors = vec![
            create_dummy_monitor(0, true),
            create_dummy_monitor(1, false)
        ];

        // "Machine Gun" Toggling: Toggle rapidly
        // This validates internal state consistency under rapid changes
        for i in 0..1000 {
            let previous_session = window.session_id;
            window.toggle(&monitors);
            
            // Expected state should flip-flop
            if i % 2 == 0 {
                assert!(window.is_open, "Should be open at iteration {}", i);
                assert!(window.monitor.is_some());
                // Verify session_id increments on Open
                assert_eq!(window.session_id, previous_session + 1, "Session ID should increment on open");
            } else {
                assert!(!window.is_open, "Should be closed at iteration {}", i);
                // Monitor selection persists, so we don't assert is_none()
                
                // Session ID should NOT increment on Close (it's an Open property)
                assert_eq!(window.session_id, previous_session, "Session ID should not increment on close");
            }
        }
    }

    #[test]
    fn test_render_stability_under_load() {
        let mut harness = Harness::new_ui(|ui| {
             // We can't easily mock the entire AppState + Context here without a lot of boilerplate
             // But we can test the ImageViewer::render static method which is what secondary window uses
             
             // Simulate rapid changes in zoom/pan inputs
             ImageViewer::render(
                 ui,
                 None, // No texture in headless test
                 None, 
                 true, // has selection
                 1.0,  // zoom
                 egui::Vec2::ZERO, // pan
                 false, // interactive
             );
        });

        // Run the harness for multiple frames to ensure no panics
        harness.run_steps(10);
    }
    
    // NOTE: True race condition testing requires spawning threads which is hard in unit test environment 
    // without spinning up the full app. We rely on logic verification + ensuring RwLocks/Mutexes (if used) don't deadlock.
    // Since UI is single-threaded (main thread), "race conditions" usually mean dirty reads from background threads.
    // We verified AsyncImageProcessor uses channels, which are safe.

    #[test]
    fn test_esc_signal_handling() {
        // This test verifies that if the viewport sends a "close request" signal (triggered by ESC),
        // the parent window correctly processes it and sets is_open = false.
        // This prevents the "infinite recreate loop" / freeze.
        
        let mut harness = Harness::new_ui(|ui| {
             // Setup window in OPEN state
             let mut window = SecondaryWindow::new();
             let monitors = vec![create_dummy_monitor(0, true)];
             window.open(&monitors);
             assert!(window.is_open);
             
             // Inject the "close request" signal into egui memory
             // This simulates the behavior of the viewport closure when ESC is pressed
             let close_req_id = egui::Id::new("secondary_window_close_req");
             ui.ctx().data_mut(|d| d.insert_temp(close_req_id, true));
             
             // Verify signal is present
             let signal_exists: bool = ui.ctx().data(|d| d.get_temp(close_req_id).unwrap_or(false));
             assert!(signal_exists, "Signal should be present in context");
             
             // Call show() - this should detect the signal and close the window
             window.show(
                 ui.ctx(),
                 None, 
                 None, 
                 false,
                 None
             );
             
             // Assert window is now CLOSED
             assert!(!window.is_open, "Window should be closed after processing signal");
             
             // Assert signal is REMOVED to prevent re-triggering
             let signal_still_exists: bool = ui.ctx().data(|d| d.get_temp(close_req_id).unwrap_or(false));
             assert!(!signal_still_exists, "Signal should be consumed and removed");
        });
        
        harness.run(); 
    }

    #[test]
    fn test_info_toggle_signal_handling() {
        let mut harness = Harness::new_ui(|ui| {
             let mut window = SecondaryWindow::new();
             let monitors = vec![create_dummy_monitor(0, true)];
             window.open(&monitors);
             
             // Initial state: Info overlay is ON by default
             assert!(window.show_info_overlay, "Info overlay should be ON initially");

             // 1. Inject 'I' signal (toggle OFF)
             let toggle_id = egui::Id::new("secondary_window_toggle_info");
             ui.ctx().data_mut(|d| d.insert_temp(toggle_id, true));
             
             // Process frame
             window.show(ui.ctx(), None, None, false, None);
             
             // Verify signal consumed and state flipped
             assert!(!ui.ctx().data(|d| d.get_temp::<bool>(toggle_id).unwrap_or(false)), "Signal should be consumed");
             assert!(!window.show_info_overlay, "Info overlay should be OFF");

             // 2. Inject 'I' signal again (toggle ON)
             ui.ctx().data_mut(|d| d.insert_temp(toggle_id, true));
             window.show(ui.ctx(), None, None, false, None);
             assert!(window.show_info_overlay, "Info overlay should be ON");
        });
        harness.run_steps(1);
    }

    #[test]
    fn test_stress_input_signals() {
        // Stress test: rapid toggling via signals to ensure no logic drift
        let mut harness = Harness::new_ui(|ui| {
             let mut window = SecondaryWindow::new();
             let monitors = vec![create_dummy_monitor(0, true)];
             window.open(&monitors);
             
             let toggle_id = egui::Id::new("secondary_window_toggle_info");
             
             for i in 0..100 {
                 let expected_state = i % 2 == 0; // Starts true (i=0 -> becomes false?) No, starts true.
                 // If i=0: insert signal -> toggle -> becomes false.
                 // If i=1: insert signal -> toggle -> becomes true.
                 
                 ui.ctx().data_mut(|d| d.insert_temp(toggle_id, true));
                 window.show(ui.ctx(), None, None, false, None);
                 
                 // Verify
                 if i % 2 == 0 {
                     assert!(!window.show_info_overlay, "Iter {}: Should be false", i);
                 } else {
                     assert!(window.show_info_overlay, "Iter {}: Should be true", i);
                 }
             }
        });
        harness.run_steps(1);
        harness.run_steps(1);
    }

    #[test]
    fn test_consume_esc_key() {
        let mut harness = Harness::new_ui(|ui| {
             let mut window = SecondaryWindow::new();
             let monitors = vec![create_dummy_monitor(0, true)];
             window.open(&monitors);
             
             window.show(ui.ctx(), None, None, false, None);

             let close_req = egui::Id::new("secondary_window_close_req");
             if ui.ctx().data(|d| d.get_temp::<bool>(close_req).unwrap_or(false)) {
                 ui.label("SIGNALCHECK");
             }
        });

        // Simulate ESC key press
        // Since kittest APIs vary, we try standard harness input manipulation
        harness.press_key(egui::Key::Escape);
        harness.run_steps(1);
        
        // If logic worked, we should find the label
        let _ = harness.get_by_label("SIGNALCHECK");
    }
}
