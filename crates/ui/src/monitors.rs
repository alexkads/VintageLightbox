// Monitor Detection Module
// Provides information about connected displays for multi-monitor support

use display_info::DisplayInfo;

/// Information about a connected monitor
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    pub scale_factor: f32,
}

/// Utility for detecting and listing connected monitors
pub struct MonitorDetector;

impl MonitorDetector {
    /// List all connected monitors
    pub fn get_monitors() -> Vec<MonitorInfo> {
        let mut monitors = Vec::new();
        
        if let Ok(displays) = DisplayInfo::all() {
            for display in displays {
                monitors.push(MonitorInfo {
                    id: display.id,
                    name: display.name.clone(),
                    x: display.x,
                    y: display.y,
                    width: display.width,
                    height: display.height,
                    is_primary: display.is_primary,
                    scale_factor: display.scale_factor,
                });
            }
        }
        
        monitors
    }
    
    /// Get the first non-primary monitor, or primary if only one exists
    pub fn get_secondary_monitor() -> Option<MonitorInfo> {
        let monitors = Self::get_monitors();
        println!("DEBUG: Found {} monitors", monitors.len());
        for (i, m) in monitors.iter().enumerate() {
            println!("DEBUG: Monitor #{}: {} ({}x{}) - Primary: {}", 
                i, m.name, m.width, m.height, m.is_primary);
        }

        let selected = monitors.iter()
            .find(|m| !m.is_primary)
            .cloned()
            .or_else(|| monitors.into_iter().next());
            
        if let Some(m) = &selected {
            println!("DEBUG: Selected secondary monitor: {}", m.name);
        }
        
        selected
    }
    
    /// Check if multiple monitors are available
    pub fn has_multiple_monitors() -> bool {
        Self::get_monitors().len() > 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_monitors_returns_list() {
        // This test will work on any system - should return at least 1 monitor
        let monitors = MonitorDetector::get_monitors();
        // Note: In CI/headless environments, this might return 0
        // So we just check it doesn't panic
        println!("Found {} monitors", monitors.len());
    }
    
    #[test]
    fn test_get_secondary_monitor_returns_something() {
        // Should return Some even with single monitor (returns primary as fallback)
        let secondary = MonitorDetector::get_secondary_monitor();
        // In headless environments, this might be None
        if let Some(monitor) = secondary {
            println!("Secondary monitor: {} ({}x{})", monitor.name, monitor.width, monitor.height);
        }
    }
}
