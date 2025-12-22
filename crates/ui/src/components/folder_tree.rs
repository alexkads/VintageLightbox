use egui::Ui;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use crate::design_system::theme::Theme;
use adapters::view_models::PhotoViewModel;

const ICON_FOLDER: &str = "📁";
const ICON_FOLDER_OPEN: &str = "📂";

#[derive(Debug, Clone)]
pub struct FolderNode {
    pub name: String,
    pub path: PathBuf,
    pub photo_count: usize,
    pub children: Vec<FolderNode>,
}

impl FolderNode {
    pub fn new(path: PathBuf) -> Self {
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
        let mut _root_nodes: HashMap<PathBuf, FolderNode> = HashMap::new();
        
        // 1. Identify all unique folders containing photos and count photos per folder
        let mut folder_counts: HashMap<PathBuf, usize> = HashMap::new();
        let mut all_paths: Vec<PathBuf> = Vec::new();
        
        for photo in photos {
            let path = Path::new(&photo.path);
            if let Some(parent) = path.parent() {
                if !folder_counts.contains_key(parent) {
                    all_paths.push(parent.to_path_buf());
                }
                *folder_counts.entry(parent.to_path_buf()).or_insert(0) += 1;
            }
        }
        
        // 2. Calculate Longest Common Prefix (LCP)
        let mut lcp: Option<PathBuf> = None;
        if !all_paths.is_empty() {
            let mut prefix = all_paths[0].clone();
            for path in all_paths.iter().skip(1) {
                while !path.starts_with(&prefix) {
                    if !prefix.pop() {
                        break;
                    }
                }
            }
            if prefix.as_os_str().len() > 0 {
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
        let mut nodes: HashMap<PathBuf, FolderNode> = HashMap::new();
        
        // Create nodes for all actual folders
        for (path, count) in &folder_counts {
            let mut node = FolderNode::new(path.clone());
            node.photo_count = *count;
            nodes.insert(path.clone(), node);
        }
        
        // Create intermediate nodes up to the Stop point
        let keys: Vec<PathBuf> = nodes.keys().cloned().collect();
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
        let mut all_node_paths: Vec<PathBuf> = nodes.keys().cloned().collect();
        all_node_paths.sort_by(|a, b| b.as_os_str().len().cmp(&a.as_os_str().len()));

        for path in all_node_paths {
            if let Some(node) = nodes.remove(&path) {
                let parent_exists = path.parent().map_or(false, |p| nodes.contains_key(p));
                
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

pub struct FolderTree<'a> {
    pub selected_path: Option<&'a Path>,
    pub expanded_paths: &'a mut HashSet<String>,
}

impl<'a> FolderTree<'a> {
    pub fn new(
        selected_path: Option<&'a Path>, 
        expanded_paths: &'a mut HashSet<String>
    ) -> Self {
        Self {
            selected_path,
            expanded_paths,
        }
    }

    pub fn show(&mut self, ui: &mut Ui, roots: &[FolderNode]) -> Option<PathBuf> {
        let mut clicked_path = None;

        for node in roots {
            if let Some(clicked) = self.show_node(ui, node) {
                clicked_path = Some(clicked);
            }
        }

        clicked_path
    }

    fn show_node(&mut self, ui: &mut Ui, node: &FolderNode) -> Option<PathBuf> {
        let mut clicked_path = None;
        
        let path_str = node.path.to_string_lossy().to_string();
        let is_selected = self.selected_path.map_or(false, |p| p == node.path);
        let icon_color = if is_selected { Theme::ACCENT_PRIMARY } else { Theme::TEXT_MUTED };
        
        let id = ui.make_persistent_id(&path_str);
        
        let state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(), 
            id, 
            false
        );
        
        let is_open = state.is_open();

        let header_response = state.show_header(ui, |ui| {
            ui.horizontal(|ui| {
                // Folder Icon
                ui.label(
                     egui::RichText::new(if is_open { ICON_FOLDER_OPEN } else { ICON_FOLDER })
                        .color(icon_color)
                );
                
                // Name (Selectable)
                let text = format!("{} ({})", node.name, node.photo_count);
                let label = ui.selectable_label(is_selected, text);
                if label.clicked() {
                    clicked_path = Some(node.path.clone());
                }
            });
        });

        header_response.body(|ui| {
            for child in &node.children {
                if let Some(clicked) = self.show_node(ui, child) {
                   clicked_path = Some(clicked);
                }
            }
        });

        // Sync expansion state
        if is_open {
             self.expanded_paths.insert(path_str.clone());
        } else {
             self.expanded_paths.remove(&path_str);
        }

        clicked_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adapters::view_models::PhotoViewModel;

    fn create_mock_photo(path: &str) -> PhotoViewModel {
        PhotoViewModel {
            id: "id".to_string(),
            name: "photo.jpg".to_string(),
            path: path.to_string(),
            thumbnail_path: None,
            date: "2023-01-01".to_string(),
            camera: "Camera".to_string(),
            exposure: "f/1.8".to_string(),
            rating: 0,
            flag: Some(0),
            color_label: None,
            edit_exposure: None,
            edit_contrast: None,
            edit_temperature: None,
            edit_tint: None,
            edit_highlights: None,
            edit_shadows: None,
            edit_whites: None,
            edit_blacks: None,
            edit_clarity: None,
            edit_vibrance: None,
            edit_saturation: None,
            edit_tone_curve_shadows: None,
            edit_tone_curve_darks: None,
            edit_tone_curve_lights: None,
            edit_tone_curve_highlights: None,
        }
    }

    #[test]
    fn test_build_tree_flat() {
        let photos = vec![
            create_mock_photo("/photos/a/1.jpg"),
            create_mock_photo("/photos/b/2.jpg"),
        ];

        let roots = FolderNode::build_tree(&photos);
        
        // LCP is "/photos". 
        // LCP does not end in Year.
        // Effective Root is "/photos".
        // Tree processing:
        // LCP /photos is "Stop At" parent.
        // /photos/a parent is /photos.
        // /photos parent is /. Match?
        // Wait, logic: stop_at = lcp.parent().
        // LCP = /photos. stop_at = /.
        // Process /photos/a. Parent /photos. != /. Add /photos.
        // Process /photos. Parent /. == /. Break.
        // /photos is created.
        // Roots linkage:
        // /photos parent /. Not in nodes? 
        // Actually / is not in nodes map.
        // So /photos is a root.
        
        println!("Roots: {:?}", roots.iter().map(|n| &n.name).collect::<Vec<_>>());
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
        
        // LCP: /Volumes/SSD/Photos
        // StopAt: /Volumes/SSD
        // Root: Photos
        // Children: 2023, 2024
        
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
        // LCP: /Volumes/SSD/Photos/2023/Trip
        // StopAt: /Volumes/SSD/Photos/2023
        // Root: Trip
        
        let roots = FolderNode::build_tree(&_photos);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "Trip");
    }
}
