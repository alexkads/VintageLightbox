use domain::value_objects::ColorLabel;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct PhotoFilters {
    pub show_flagged: bool,   
    pub show_unflagged: bool, 
    pub show_rejected: bool,  
    pub min_rating: u8,       
    pub color_labels: HashSet<ColorLabel>, 
    pub folder_path: Option<PathBuf>, 
}

impl PhotoFilters {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn is_active(&self) -> bool {
        self.show_flagged || self.show_unflagged || self.show_rejected || 
        self.min_rating > 0 || !self.color_labels.is_empty()
    }

    pub fn apply<'a>(&self, photos: &'a [crate::view_models::PhotoViewModel]) -> Vec<&'a crate::view_models::PhotoViewModel> {
        // If NO attribute filters are active AND no folder path, return all
        if !self.is_active() && self.folder_path.is_none() {
            return photos.iter().collect();
        }

        photos.iter().filter(|photo| {
            // Folder Path Filter (Source)
            if let Some(ref folder_path) = self.folder_path {
                 let photo_path = std::path::Path::new(&photo.path);
                 if !photo_path.starts_with(folder_path) {
                     return false;
                 }
            }

            // Flag Filter
            let flag_filtering_active = self.show_flagged || self.show_unflagged || self.show_rejected;
            
            if flag_filtering_active {
                let status = photo.flag.unwrap_or(0); // 1=Pick, 0=None, -1=Reject
                let match_flag = match status {
                    1 => self.show_flagged,
                    0 => self.show_unflagged,
                    -1 => self.show_rejected,
                    _ => false,
                };
                if !match_flag { return false; }
            }

            // Rating Filter
            if self.min_rating > 0 && photo.rating < self.min_rating as i32 {
                return false;
            }

            // Color Label Filter
            if !self.color_labels.is_empty() {
                match &photo.color_label {
                    Some(s) => {
                         if let Ok(label) = ColorLabel::from_name(s) {
                             if !self.color_labels.contains(&label) {
                                 return false;
                             }
                         } else {
                             return false; 
                         }
                    },
                    None => return false,
                }
            }

            true
        }).collect()
    }

    /// Reset all filters (Attribute/Metadata)
    /// Note: Does not clear folder_path
    pub fn reset(&mut self) {
        self.show_flagged = false;
        self.show_unflagged = false;
        self.show_rejected = false;
        self.min_rating = 0;
        self.color_labels.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_models::PhotoViewModel;
    use std::path::PathBuf;

    fn create_dummy_photo(id: &str, flag: Option<i32>, rating: i32, color: Option<&str>, path: &str) -> PhotoViewModel {
        PhotoViewModel {
            id: id.to_string(),
            name: format!("Photo {}", id),
            path: path.to_string(),
            thumbnail_path: None,
            date: "2023-01-01".to_string(),
            camera: "TestCam".to_string(),
            exposure: "1/100".to_string(),
            rating,
            color_label: color.map(|s| s.to_string()),
            flag,
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
            ..Default::default()
        }
    }

    #[test]
    fn test_filter_flags() {
        let mut filter = PhotoFilters::new();
        let p1 = create_dummy_photo("1", Some(1), 0, None, "/a/b.jpg"); // Pick
        let p2 = create_dummy_photo("2", Some(0), 0, None, "/a/c.jpg"); // Unflagged explicitly
        let p3 = create_dummy_photo("3", Some(-1), 0, None, "/a/d.jpg"); // Reject
        let p4 = create_dummy_photo("4", None, 0, None, "/a/e.jpg");    // No flag (Unflagged implicitly)

        let photos = vec![p1, p2, p3, p4];

        // No filters -> All
        assert_eq!(filter.apply(&photos).len(), 4);

        // Picked only
        filter.show_flagged = true;
        let res = filter.apply(&photos);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "1");

        // Picked OR Rejected
        filter.show_rejected = true;
        let res = filter.apply(&photos);
        assert_eq!(res.len(), 2);
        
        // Reset
        filter.reset();
        assert_eq!(filter.apply(&photos).len(), 4);
        
        // Unflagged (Explicit 0 and None)
        filter.show_unflagged = true;
        let res = filter.apply(&photos);
        assert_eq!(res.len(), 2); // p2 and p4
    }

    #[test]
    fn test_filter_rating() {
        let mut filter = PhotoFilters::new();
        let p1 = create_dummy_photo("1", None, 1, None, "/a.jpg");
        let p2 = create_dummy_photo("2", None, 3, None, "/b.jpg");
        let p3 = create_dummy_photo("3", None, 5, None, "/c.jpg");
        let p4 = create_dummy_photo("4", None, 0, None, "/d.jpg");

        let photos = vec![p1, p2, p3, p4];

        // Min Rating 3
        filter.min_rating = 3;
        let res = filter.apply(&photos);
        assert_eq!(res.len(), 2); // 3 and 5 stars
        
        // Min Rating 5
        filter.min_rating = 5;
        let res = filter.apply(&photos);
        assert_eq!(res.len(), 1);
        
        // Min Rating 1
        filter.min_rating = 1;
        let res = filter.apply(&photos);
        assert_eq!(res.len(), 3);
    }
    
    #[test]
    fn test_filter_color() {
        let mut filter = PhotoFilters::new();
        let p1 = create_dummy_photo("1", None, 0, Some("Red"), "/a.jpg");
        let p2 = create_dummy_photo("2", None, 0, Some("Blue"), "/b.jpg");
        let p3 = create_dummy_photo("3", None, 0, None, "/c.jpg");

        let photos = vec![p1, p2, p3];
        
        // Red
        filter.color_labels.insert(ColorLabel::Red);
        let res = filter.apply(&photos);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "1");
        
        // Red OR Blue
        filter.color_labels.insert(ColorLabel::Blue);
        let res = filter.apply(&photos);
        assert_eq!(res.len(), 2);
    }
    
    #[test]
    fn test_filter_folder() {
        let mut filter = PhotoFilters::new();
        let p1 = create_dummy_photo("1", None, 0, None, "/photos/2023/trip/a.jpg");
        let p2 = create_dummy_photo("2", None, 0, None, "/photos/2023/home/b.jpg");
        let p3 = create_dummy_photo("3", None, 0, None, "/photos/2022/old/c.jpg");

        let photos = vec![p1, p2, p3];

        // Filter /photos/2023
        filter.folder_path = Some(PathBuf::from("/photos/2023"));
        let res = filter.apply(&photos);
        assert_eq!(res.len(), 2);
        
        // Filter /photos/2023/trip
        filter.folder_path = Some(PathBuf::from("/photos/2023/trip"));
        let res = filter.apply(&photos);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "1");
        
        // Combined with Flags
        filter.show_flagged = true; // Assuming p1 is unflagged -> expecting 0?
        // p1 flag is None.
        let res = filter.apply(&photos);
        assert_eq!(res.len(), 0);
    }
}
