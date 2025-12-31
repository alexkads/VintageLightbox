// Secondary Window Component
// Displays the selected photo in fullscreen on a secondary monitor

use egui::{Context, ViewportBuilder, ViewportId, ViewportCommand, Pos2, Color32, RichText, Align2};
use crate::monitors::MonitorInfo;
use adapters::view_models::CropSettings;

/// Manages the secondary fullscreen window for client viewing
pub struct SecondaryWindow {
    /// Whether the secondary window is currently open
    pub is_open: bool,
    /// The monitor where the window is displayed
    pub monitor: Option<MonitorInfo>,
    /// ID of the currently displayed photo
    pub current_photo_id: Option<String>,
    /// Whether to show photo info overlay
    pub show_info_overlay: bool,
    /// A unique ID for the current session of the secondary window being open. Increments each time it's opened.
    pub session_id: u64,
}

impl Default for SecondaryWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl SecondaryWindow {
    pub fn new() -> Self {
        Self {
            is_open: false,
            monitor: None,
            current_photo_id: None,
            show_info_overlay: true,
            session_id: 0,
        }
    }
    
    /// Toggle the secondary window on/off
    pub fn toggle(&mut self, monitors: &[MonitorInfo]) {
        if self.is_open {
            self.close();
        } else {
            self.open(monitors);
        }
    }
    
    /// Open secondary window on the best available monitor
    pub fn open(&mut self, monitors: &[MonitorInfo]) {
        println!("DEBUG: Opening secondary window. Available monitors: {}", monitors.len());
        for (i, m) in monitors.iter().enumerate() {
            println!("DEBUG: Monitor #{}: {} ({}x{}) @ ({},{}) Primary: {}", 
                i, m.name, m.width, m.height, m.x, m.y, m.is_primary);
        }
        
        // Prefer secondary monitor, fallback to primary
        self.monitor = monitors.iter()
            .find(|m| !m.is_primary)
            .or_else(|| monitors.first())
            .cloned();
            
        if let Some(m) = &self.monitor {
            println!("DEBUG: Selected monitor for secondary window: {} @ ({},{})", m.name, m.x, m.y);
        } else {
            println!("DEBUG: No monitor selected!");
        }
        
        self.is_open = true;
        self.session_id += 1;
    }
    
    /// Close the secondary window
    pub fn close(&mut self) {
        self.is_open = false;
        self.current_photo_id = None;
    }
    
    /// Update the photo being displayed
    pub fn set_photo(&mut self, photo_id: Option<String>) {
        self.current_photo_id = photo_id;
    }
    
    /// Toggle the info overlay visibility
    pub fn toggle_info_overlay(&mut self) {
        self.show_info_overlay = !self.show_info_overlay;
    }
    
    /// Show the secondary viewport (call from main app update)
    /// 
    /// # Arguments
    /// * `ctx` - The egui context
    /// * `image` - Optional texture handle for the photo to display
    /// * `photo_info`    /// Show the secondary viewport (call from main app update)
    pub fn show(
        &mut self,
        ctx: &Context,
        detail_image: Option<&egui::TextureHandle>,
        thumbnail_preview: Option<&egui::TextureHandle>,
        has_selection: bool,
        crop_settings: Option<&CropSettings>,
        photo_info: Option<(&str, &str)>, // (filename, rating)
    ) {
        if !self.is_open {
            return;
        }
        
        // Check if the viewport requested to close itself (e.g. user pressed ESC inside it)
        // This is necessary because the viewport runs in a separate context/closure but shares egui::Memory
        let close_req_id = egui::Id::new("secondary_window_close_req");
        if ctx.data(|d| d.get_temp(close_req_id).unwrap_or(false)) {
            self.close();
            ctx.data_mut(|d| d.remove_temp::<bool>(close_req_id));
            return;
        }
        
        // Check if the viewport requested to toggle info
        let toggle_info_id = egui::Id::new("secondary_window_toggle_info");
        if ctx.data(|d| d.get_temp(toggle_info_id).unwrap_or(false)) {
            self.show_info_overlay = !self.show_info_overlay;
            // Debounce/consume signal
            ctx.data_mut(|d| d.remove_temp::<bool>(toggle_info_id));
        }

        let Some(monitor) = &self.monitor else {
            return;
        };
        
        let viewport_id = ViewportId::from_hash_of("secondary_fullscreen");
        
        // Calculate position at the start of the secondary monitor
        // Add an offset to ensure the window is clearly "inside" the monitor to prevent OS from snapping it to primary
        let offset = 100; 
        let position = Pos2::new((monitor.x + offset) as f32, (monitor.y + offset) as f32);
        let size = egui::vec2(monitor.width as f32, monitor.height as f32);
        
        // Clone data for the closure (fix lifetime issues)
        let detail_clone = detail_image.cloned();
        let thumb_clone = thumbnail_preview.cloned();
        let photo_info_clone = photo_info.map(|(n, r)| (n.to_string(), r.to_string()));
        let crop_settings = crop_settings.cloned(); // Clone value object
        let show_info = self.show_info_overlay;
        let session_id = self.session_id;
        
        // Use deferred rendering to avoid blocking the main thread
        ctx.show_viewport_deferred(
            viewport_id,
            ViewportBuilder::default()
                .with_title("VintageLightbox - Client View")
                .with_position(position)
                .with_inner_size(size)
                .with_fullscreen(false) // Start non-fullscreen to ensure positioning works
                .with_decorations(false),
            move |ctx, _class| {
                // Force fullscreen ONLY ONCE per session to avoid spam/lag
                // We use egui's temporary memory to track if we've requested it for this viewport session
                let fs_req_id = egui::Id::new("secondary_fs_init").with(session_id);
                let requested: bool = ctx.data(|d| d.get_temp(fs_req_id).unwrap_or(false));

                if !requested {
                    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));
                    ctx.data_mut(|d| d.insert_temp(fs_req_id, true));
                }

                // Force constant repaint to ensure we pick up state changes from the main window immediately
                // preventing "lag" or "stale" images when navigating in the main window.
                ctx.request_repaint();

                // Black background for photo viewing
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(Color32::BLACK))
                    .show(ctx, |ui| {
                        // Reuse ImageViewer rendering logic
                        // We use interactive=false to enforce "Fit to Screen" mode for presentation
                        crate::components::image_viewer::ImageViewer::render(
                            ui,
                            detail_clone.as_ref(),
                            thumb_clone.as_ref(),
                            None, // intelligent_fill_texture: Not needed for presentation
                            has_selection,
                            1.0, // zoom
                            egui::Vec2::ZERO, // pan
                            false, // interactive
                            false, // allow_pan
                            crop_settings.as_ref(), // Pass crop settings ref
                            true, // apply_crop_clip: Always clip in secondary window (presentation mode)
                        );
                        
                        // Optional overlay with photo info (bottom left)
                        if show_info {
                            if let Some((filename, rating)) = &photo_info_clone {
                                egui::Area::new(egui::Id::new("photo_info_overlay"))
                                    .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(20.0, -20.0))
                                    .show(ctx, |ui| {
                                        egui::Frame::NONE
                                            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 180))
                                            .inner_margin(egui::Margin::same(8))
                                            .corner_radius(4.0)
                                            .show(ui, |ui| {
                                                ui.label(RichText::new(filename)
                                                    .size(14.0)
                                                    .color(Color32::WHITE));
                                                ui.label(RichText::new(rating)
                                                    .size(12.0)
                                                    .color(Color32::LIGHT_GRAY));
                                            });
                                    });
                            }
                        }
                    });
                
                // Instructions overlay (top right)
                egui::Area::new(egui::Id::new("secondary_instructions_overlay"))
                    .anchor(Align2::RIGHT_TOP, egui::vec2(-20.0, 20.0))
                    .show(ctx, |ui| {
                        egui::Frame::NONE
                            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 120))
                            .inner_margin(egui::Margin::same(8))
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                                ui.label(RichText::new("ESC to close • I to toggle info")
                                    .size(11.0)
                                    .color(Color32::from_gray(180)));
                            });
                    });
                
                // Handle keyboard shortcuts
                // Explicitly consume keys to prevent backend/default handling conflicts
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                     ctx.data_mut(|d| d.insert_temp(egui::Id::new("secondary_window_close_req"), true));
                }
                
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::I)) {
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("secondary_window_toggle_info"), true));
                }
            }
        );

    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_secondary_window_new() {
        let window = SecondaryWindow::new();
        assert!(!window.is_open);
        assert!(window.monitor.is_none());
        assert!(window.current_photo_id.is_none());
        assert!(window.show_info_overlay);
    }
    
    #[test]
    fn test_toggle_opens_and_closes() {
        let mut window = SecondaryWindow::new();
        let monitors = vec![MonitorInfo {
            id: 0,
            name: "Test".to_string(),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            is_primary: true,
            scale_factor: 1.0,
        }];
        
        // Initially closed
        assert!(!window.is_open);
        
        // Toggle opens
        window.toggle(&monitors);
        assert!(window.is_open);
        assert!(window.monitor.is_some());
        
        // Toggle closes
        window.toggle(&monitors);
        assert!(!window.is_open);
    }
    
    #[test]
    fn test_prefers_secondary_monitor() {
        let mut window = SecondaryWindow::new();
        let monitors = vec![
            MonitorInfo {
                id: 0,
                name: "Primary".to_string(),
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                is_primary: true,
                scale_factor: 1.0,
            },
            MonitorInfo {
                id: 1,
                name: "Secondary".to_string(),
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
                is_primary: false,
                scale_factor: 1.0,
            },
        ];
        
        window.open(&monitors);
        
        assert!(window.is_open);
        let selected = window.monitor.as_ref().unwrap();
        assert_eq!(selected.name, "Secondary");
        assert!(!selected.is_primary);
    }
    
    #[test]
    fn test_set_photo() {
        let mut window = SecondaryWindow::new();
        
        assert!(window.current_photo_id.is_none());
        
        window.set_photo(Some("photo-123".to_string()));
        assert_eq!(window.current_photo_id, Some("photo-123".to_string()));
        
        window.set_photo(None);
        assert!(window.current_photo_id.is_none());
    }
    
    #[test]
    fn test_toggle_info_overlay() {
        let mut window = SecondaryWindow::new();
        
        assert!(window.show_info_overlay);
        
        window.toggle_info_overlay();
        assert!(!window.show_info_overlay);
        
        window.toggle_info_overlay();
        assert!(window.show_info_overlay);
    }
}
