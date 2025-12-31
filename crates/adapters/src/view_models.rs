use serde::{Serialize, Deserialize};

// =============================================================================
// Re-exports for UI Layer
// =============================================================================
// These re-exports allow UI to import from adapters instead of domain directly,
// maintaining clean architecture boundaries while avoiding code duplication
// for simple value objects that are already DTOs.

// Photo editing types
pub use domain::value_objects::PhotoEdits;
pub use domain::value_objects::CropSettings;
pub use domain::value_objects::AspectRatio;
pub use domain::value_objects::RotationFillMode;

// Classification types
pub use domain::value_objects::ColorLabel;
pub use domain::value_objects::Flag;

// Import types
pub use domain::import_source::ImportSource;
pub use domain::value_objects::{ImportOptions, OrganizationStrategy, RenamePattern};

// Entity types (when needed as DTOs in UI)
pub use domain::entities::Preset;
pub use domain::entities::preset::PresetAdjustments;

// Services types (when UI needs to interact with service status)
pub use domain::services::intelligent_fill::ModelStatus;

// =============================================================================
// Dedicated ViewModels
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PhotoViewModel {
    pub id: String,
    pub name: String,
    pub path: String,
    pub thumbnail_path: Option<String>,
    pub date: String,
    pub camera: String,
    pub exposure: String,
    pub rating: i32,
    pub color_label: Option<String>,
    pub flag: Option<i32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub edit_exposure: Option<f32>,
    pub edit_contrast: Option<f32>,
    pub edit_temperature: Option<f32>,
    pub edit_tint: Option<f32>,
    pub edit_highlights: Option<f32>,
    pub edit_shadows: Option<f32>,
    pub edit_whites: Option<f32>,
    pub edit_blacks: Option<f32>,
    pub edit_clarity: Option<f32>,
    pub edit_vibrance: Option<f32>,
    pub edit_saturation: Option<f32>,
    pub edit_tone_curve_shadows: Option<f32>,
    pub edit_tone_curve_darks: Option<f32>,
    pub edit_tone_curve_lights: Option<f32>,
    pub edit_tone_curve_highlights: Option<f32>,
    // HSL Saturation
    pub edit_hsl_red_sat: Option<f32>,
    pub edit_hsl_orange_sat: Option<f32>,
    pub edit_hsl_yellow_sat: Option<f32>,
    pub edit_hsl_green_sat: Option<f32>,
    pub edit_hsl_aqua_sat: Option<f32>,
    pub edit_hsl_blue_sat: Option<f32>,
    pub edit_hsl_purple_sat: Option<f32>,
    pub edit_hsl_magenta_sat: Option<f32>,
    // HSL Hue
    pub edit_hsl_red_hue: Option<f32>,
    pub edit_hsl_orange_hue: Option<f32>,
    pub edit_hsl_yellow_hue: Option<f32>,
    pub edit_hsl_green_hue: Option<f32>,
    pub edit_hsl_aqua_hue: Option<f32>,
    pub edit_hsl_blue_hue: Option<f32>,
    pub edit_hsl_purple_hue: Option<f32>,
    pub edit_hsl_magenta_hue: Option<f32>,
    // HSL Lum
    pub edit_hsl_red_lum: Option<f32>,
    pub edit_hsl_orange_lum: Option<f32>,
    pub edit_hsl_yellow_lum: Option<f32>,
    pub edit_hsl_green_lum: Option<f32>,
    pub edit_hsl_aqua_lum: Option<f32>,
    pub edit_hsl_blue_lum: Option<f32>,
    pub edit_hsl_purple_lum: Option<f32>,
    pub edit_hsl_magenta_lum: Option<f32>,
    // Lens
    pub edit_lens_distortion: Option<f32>,
    pub edit_lens_vignette_amount: Option<f32>,
    pub edit_lens_vignette_midpoint: Option<f32>,
    // NR
    pub edit_nr_luminance: Option<f32>,
    pub edit_nr_color: Option<f32>,
    // Sharpening
    pub edit_sharpen_amount: Option<f32>,
    pub edit_sharpen_radius: Option<f32>,
    // Crop & Rotation
    pub edit_crop_x: Option<f32>,
    pub edit_crop_y: Option<f32>,
    pub edit_crop_width: Option<f32>,
    pub edit_crop_height: Option<f32>,
    pub edit_crop_rotation: Option<i32>,
    pub edit_crop_angle: Option<f32>,
    pub edit_crop_flip_h: Option<bool>,
    pub edit_crop_flip_v: Option<bool>,
    pub edit_crop_fill_mode: Option<u8>,
    pub file_missing: bool,
}

impl PhotoViewModel {
    /// Convert edit fields to a PhotoEdits struct for image processing
    pub fn to_edits(&self) -> PhotoEdits {
        let crop_settings = if let (Some(x), Some(y), Some(w), Some(h)) = (
            self.edit_crop_x, self.edit_crop_y,
            self.edit_crop_width, self.edit_crop_height
        ) {
            let fill_mode = RotationFillMode::try_from(self.edit_crop_fill_mode.unwrap_or(0))
                .unwrap_or_default();
            Some(CropSettings::with_fill_mode_value(
                x, y, w, h,
                self.edit_crop_rotation.unwrap_or(0),
                self.edit_crop_angle.unwrap_or(0.0),
                self.edit_crop_flip_h.unwrap_or(false),
                self.edit_crop_flip_v.unwrap_or(false),
                fill_mode,
            ))
        } else {
            None
        };

        PhotoEdits {
            exposure: self.edit_exposure.unwrap_or(0.0),
            contrast: self.edit_contrast.unwrap_or(1.0),
            temperature: self.edit_temperature.unwrap_or(0.0),
            tint: self.edit_tint.unwrap_or(0.0),
            highlights: self.edit_highlights.unwrap_or(0.0),
            shadows: self.edit_shadows.unwrap_or(0.0),
            whites: self.edit_whites.unwrap_or(0.0),
            blacks: self.edit_blacks.unwrap_or(0.0),
            clarity: self.edit_clarity.unwrap_or(0.0),
            vibrance: self.edit_vibrance.unwrap_or(0.0),
            saturation: self.edit_saturation.unwrap_or(0.0),
            tone_curve_shadows: self.edit_tone_curve_shadows.unwrap_or(0.0),
            tone_curve_darks: self.edit_tone_curve_darks.unwrap_or(0.0),
            tone_curve_lights: self.edit_tone_curve_lights.unwrap_or(0.0),
            tone_curve_highlights: self.edit_tone_curve_highlights.unwrap_or(0.0),
            hsl_red_sat: self.edit_hsl_red_sat.unwrap_or(0.0),
            hsl_orange_sat: self.edit_hsl_orange_sat.unwrap_or(0.0),
            hsl_yellow_sat: self.edit_hsl_yellow_sat.unwrap_or(0.0),
            hsl_green_sat: self.edit_hsl_green_sat.unwrap_or(0.0),
            hsl_aqua_sat: self.edit_hsl_aqua_sat.unwrap_or(0.0),
            hsl_blue_sat: self.edit_hsl_blue_sat.unwrap_or(0.0),
            hsl_purple_sat: self.edit_hsl_purple_sat.unwrap_or(0.0),
            hsl_magenta_sat: self.edit_hsl_magenta_sat.unwrap_or(0.0),
            hsl_red_hue: self.edit_hsl_red_hue.unwrap_or(0.0),
            hsl_orange_hue: self.edit_hsl_orange_hue.unwrap_or(0.0),
            hsl_yellow_hue: self.edit_hsl_yellow_hue.unwrap_or(0.0),
            hsl_green_hue: self.edit_hsl_green_hue.unwrap_or(0.0),
            hsl_aqua_hue: self.edit_hsl_aqua_hue.unwrap_or(0.0),
            hsl_blue_hue: self.edit_hsl_blue_hue.unwrap_or(0.0),
            hsl_purple_hue: self.edit_hsl_purple_hue.unwrap_or(0.0),
            hsl_magenta_hue: self.edit_hsl_magenta_hue.unwrap_or(0.0),
            hsl_red_lum: self.edit_hsl_red_lum.unwrap_or(0.0),
            hsl_orange_lum: self.edit_hsl_orange_lum.unwrap_or(0.0),
            hsl_yellow_lum: self.edit_hsl_yellow_lum.unwrap_or(0.0),
            hsl_green_lum: self.edit_hsl_green_lum.unwrap_or(0.0),
            hsl_aqua_lum: self.edit_hsl_aqua_lum.unwrap_or(0.0),
            hsl_blue_lum: self.edit_hsl_blue_lum.unwrap_or(0.0),
            hsl_purple_lum: self.edit_hsl_purple_lum.unwrap_or(0.0),
            hsl_magenta_lum: self.edit_hsl_magenta_lum.unwrap_or(0.0),
            lens_distortion: self.edit_lens_distortion.unwrap_or(0.0),
            lens_vignette_amount: self.edit_lens_vignette_amount.unwrap_or(0.0),
            lens_vignette_midpoint: self.edit_lens_vignette_midpoint.unwrap_or(0.0),
            nr_luminance: self.edit_nr_luminance.unwrap_or(0.0),
            nr_color: self.edit_nr_color.unwrap_or(0.0),
            sharpen_amount: self.edit_sharpen_amount.unwrap_or(0.0),
            sharpen_radius: self.edit_sharpen_radius.unwrap_or(1.0),
            crop_settings,
        }
    }
    
    /// Check if the photo has any edits applied
    pub fn has_edits(&self) -> bool {
        self.edit_exposure.unwrap_or(0.0) != 0.0 ||
        self.edit_contrast.unwrap_or(1.0) != 1.0 ||
        self.edit_temperature.unwrap_or(0.0) != 0.0 ||
        self.edit_tint.unwrap_or(0.0) != 0.0 ||
        self.edit_saturation.unwrap_or(0.0) != 0.0 ||
        self.edit_vibrance.unwrap_or(0.0) != 0.0
    }
}

#[derive(Debug, Clone)]
pub struct FolderNode {
    pub name: String,
    pub path: std::path::PathBuf,
    pub photo_count: usize,
    pub children: Vec<FolderNode>,
}

impl FolderNode {
    pub fn new(path: std::path::PathBuf) -> Self {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());
        
        Self {
            name,
            path,
            photo_count: 0,
            children: Vec::new(),
        }
    }

    /// Recursively build a tree from a list of photos
    pub fn build_tree(photos: &[PhotoViewModel]) -> Vec<FolderNode> {
        let mut _root_nodes: std::collections::HashMap<std::path::PathBuf, FolderNode> = std::collections::HashMap::new();
        
        // 1. Identify all unique folders containing photos and count photos per folder
        let mut folder_counts: std::collections::HashMap<std::path::PathBuf, usize> = std::collections::HashMap::new();
        let mut all_paths: Vec<std::path::PathBuf> = Vec::new();
        
        for photo in photos {
            let path = std::path::Path::new(&photo.path);
            if let Some(parent) = path.parent() {
                if !folder_counts.contains_key(parent) {
                    all_paths.push(parent.to_path_buf());
                }
                *folder_counts.entry(parent.to_path_buf()).or_insert(0) += 1;
            }
        }
        
        // 2. Calculate Longest Common Prefix (LCP)
        let mut lcp: Option<std::path::PathBuf> = None;
        if !all_paths.is_empty() {
            let mut prefix = all_paths[0].clone();
            for path in all_paths.iter().skip(1) {
                while !path.starts_with(&prefix) {
                    if !prefix.pop() {
                        break;
                    }
                }
            }
            if !prefix.as_os_str().is_empty() {
                lcp = Some(prefix);
            }
        }

        // 3. Simple Stop Point
        // Stop at the parent of the common prefix, so the common prefix itself is the root node.
        // e.g. if LCP is /Users/alex/Pictures/VintageLightbox
        // stop_at = /Users/alex/Pictures
        // Root node will be "VintageLightbox"
        let stop_at = lcp.as_ref().and_then(|p| p.parent().map(|p| p.to_path_buf()));

        // 4. Build Nodes
        let mut nodes: std::collections::HashMap<std::path::PathBuf, FolderNode> = std::collections::HashMap::new();
        
        // Create nodes for all actual folders
        for (path, count) in &folder_counts {
            let mut node = FolderNode::new(path.clone());
            node.photo_count = *count;
            nodes.insert(path.clone(), node);
        }
        
        // Create intermediate nodes up to the Stop point
        let keys: Vec<std::path::PathBuf> = nodes.keys().cloned().collect();
        for path in keys {
            let mut current = path.clone();
            while let Some(parent) = current.parent() {
                // STOP CONDITION: If parent matches our stop point (LCP parent)
                if let Some(ref stop) = stop_at {
                    if parent == stop { break; }
                } else {
                    // Fallback to preventing root from being added if LCP is None? 
                    // No, if LCP is None (multiple drives), we want full paths up to roots.
                    if parent.as_os_str().is_empty() { break; }
                }
                
                // Also stop if we hit root on filesystem
                 if parent.as_os_str().is_empty() { break; }

                if !nodes.contains_key(parent) {
                    let node = FolderNode::new(parent.to_path_buf());
                    nodes.insert(parent.to_path_buf(), node);
                }
                current = parent.to_path_buf();
            }
        }
        

        // 5. Link nodes
        let mut roots: Vec<FolderNode> = Vec::new();
        // Sort by length desc so we process children before parents
        let mut all_node_paths: Vec<std::path::PathBuf> = nodes.keys().cloned().collect();
        all_node_paths.sort_by_key(|b| std::cmp::Reverse(b.as_os_str().len()));

        for path in all_node_paths {
            if let Some(node) = nodes.remove(&path) {
                let parent_exists = path.parent().is_some_and(|p| nodes.contains_key(p));
                
                if parent_exists {
                     let parent_path = path.parent().unwrap();
                     // Unwrap is safe because we checked contains_key
                     let parent_node = nodes.get_mut(parent_path).unwrap();
                     parent_node.children.push(node);
                     parent_node.children.sort_by(|a, b| a.name.cmp(&b.name));
                } else {
                    roots.push(node);
                }
            }
        }
        
        roots.sort_by(|a, b| a.name.cmp(&b.name));
        roots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_photo(path: &str) -> PhotoViewModel {
        PhotoViewModel {
            id: "id".to_string(),
            name: "photo.jpg".to_string(),
            path: path.to_string(),
            // Default fills the rest
            ..Default::default()
        }
    }

    #[test]
    fn test_build_tree_flat() {
        let photos = vec![
            create_mock_photo("/photos/a/1.jpg"),
            create_mock_photo("/photos/b/2.jpg"),
        ];

        let roots = FolderNode::build_tree(&photos);
        
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "photos");
        assert_eq!(roots[0].children.len(), 2);
    }

    #[test]
    fn test_build_tree_with_year() {
        let photos = vec![
            create_mock_photo("/Volumes/SSD/Photos/2023/Trip/1.jpg"),
            create_mock_photo("/Volumes/SSD/Photos/2023/Party/2.jpg"),
            create_mock_photo("/Volumes/SSD/Photos/2024/Jan/3.jpg"),
        ];
        
        let roots = FolderNode::build_tree(&photos);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "Photos");
        assert_eq!(roots[0].children.len(), 2);
    }
    
    #[test]
    fn test_build_tree_single_year() {
         let _photos = vec![
            create_mock_photo("/Volumes/SSD/Photos/2023/Trip/1.jpg"),
        ];
        
        let roots = FolderNode::build_tree(&_photos);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "Trip");
    }
}

/// ViewModel for import preview items
#[derive(Debug, Clone)]
pub struct ImportPreviewItemViewModel {
    pub file_path: String,
    pub thumbnail_data: Vec<u8>,
    pub file_size: u64,
    pub is_raw: bool,
    pub camera: String,
    pub date_time: String,
    pub dimensions: Option<String>,
}

/// ViewModel for duplicate check results
#[derive(Debug, Clone)]
pub struct DuplicateCheckViewModel {
    pub file_path: String,
    pub content_hash: String,
    pub is_duplicate: bool,
    pub existing_photo_path: Option<String>,
}

/// ViewModel for import progress events
#[derive(Debug, Clone)]
pub enum ImportProgressViewModel {
    Starting { total: usize },
    Processing { index: usize, path: String },
    Completed { photo_id: String, path: String },
    Failed { path: String, error: String },
    DuplicateSkipped { path: String, existing_path: String },
    Paused { completed: usize, remaining: usize },
    Finished { successful: usize, failed: usize, skipped: usize },
}
