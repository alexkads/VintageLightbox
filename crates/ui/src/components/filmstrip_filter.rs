use crate::design_system::theme::Theme;
use domain::value_objects::ColorLabel;
use egui::{Color32, RichText, Sense, Ui};
// use crate::design_system::icons;
use adapters::view_models::PhotoViewModel;
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct FilmstripFilter {
    pub show_flagged: bool,   // Picked
    pub show_unflagged: bool, // No flag
    pub show_rejected: bool,  // Rejected

    pub min_rating: u8, // 0 means "any" (or >= 0)

    pub color_labels: HashSet<ColorLabel>, // If empty, no color filter.

    pub folder_path: Option<std::path::PathBuf>, // Navigation context (source)
}

impl FilmstripFilter {
    pub fn new() -> Self {
        Self {
            show_flagged: false,
            show_unflagged: false,
            show_rejected: false,
            min_rating: 0,
            color_labels: HashSet::new(),
            folder_path: None,
        }
    }

    /// Check if any filter (Attribute/Metadata) is currently active
    /// Note: Does not include folder_path as that is considered "Source" not "Filter"
    pub fn is_active(&self) -> bool {
        self.show_flagged
            || self.show_unflagged
            || self.show_rejected
            || self.min_rating > 0
            || !self.color_labels.is_empty()
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

    /// Apply filters to a list of photos, returning a new vector of filtered photos
    pub fn apply<'a>(&self, photos: &'a [PhotoViewModel]) -> Vec<&'a PhotoViewModel> {
        // If NO attribute filters are active AND no folder path, return all
        if !self.is_active() && self.folder_path.is_none() {
            return photos.iter().collect();
        }

        photos
            .iter()
            .filter(|photo| {
                // Folder Path Filter (Source)
                if let Some(ref folder_path) = self.folder_path {
                    let photo_path = std::path::Path::new(&photo.path);
                    if !photo_path.starts_with(folder_path) {
                        return false;
                    }
                }

                // Flag Filter
                let flag_filtering_active =
                    self.show_flagged || self.show_unflagged || self.show_rejected;

                if flag_filtering_active {
                    let status = photo.flag.unwrap_or(0); // 1=Pick, 0=None, -1=Reject
                    let match_flag = match status {
                        1 => self.show_flagged,
                        0 => self.show_unflagged,
                        -1 => self.show_rejected,
                        _ => false,
                    };
                    if !match_flag {
                        return false;
                    }
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
                        }
                        None => return false,
                    }
                }

                true
            })
            .collect()
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Filter:");

            // Flags
            // Using icons. We can assume we have icons for P, U, X or similar.
            // In lieu of verified icon constants for filter-specific states, I'll use text/emoji for now
            // and replace with `icons::*` if I can confirm them or standard ones.
            // Looking at filmstrip.rs, it uses manual painting for flags.
            // We'll use selectable labels or standard buttons.

            let mut toggle_flag = |pixel: &str, active: &mut bool, color: Color32| {
                let btn = egui::Button::new(RichText::new(pixel).color(if *active {
                    Color32::WHITE
                } else {
                    Color32::GRAY
                }))
                .fill(if *active { color } else { Color32::TRANSPARENT })
                .frame(true); // Always frame to look like a toggle button

                if ui.add(btn).clicked() {
                    *active = !*active;
                }
            };

            // Pick
            toggle_flag("P", &mut self.show_flagged, Theme::ACCENT_SUCCESS); // P for Pick
                                                                             // Unflagged
            toggle_flag("U", &mut self.show_unflagged, Color32::from_gray(100)); // U for Unflagged
                                                                                 // Reject
            toggle_flag("X", &mut self.show_rejected, Theme::ACCENT_ERROR); // X for Reject

            ui.separator();

            // Rating
            // Click to set min rating. Click current rating again to toggle off?
            // Or click "0" stars explicitly?
            // Usually stars are: click 3rd star -> min_rating = 3.
            // If min_rating is currently 3 and click 3 again -> min_rating = 0 (off) or just stay 3?
            // Let's implement: Click N -> Sets to N. If N is already set -> Sets to 0.

            ui.label("≥");
            for i in 1..=5 {
                let active = self.min_rating >= i;
                let color = if active {
                    Theme::RATING_ACTIVE
                } else {
                    Color32::GRAY
                };
                if ui
                    .add(egui::Label::new(RichText::new("★").color(color)).sense(Sense::click()))
                    .clicked()
                {
                    if self.min_rating == i {
                        self.min_rating = 0; // Toggle off
                    } else {
                        self.min_rating = i;
                    }
                }
            }

            ui.separator();

            // Colors
            let all_colors = [
                (ColorLabel::Red, Theme::LABEL_RED),
                (ColorLabel::Yellow, Theme::LABEL_YELLOW),
                (ColorLabel::Green, Theme::LABEL_GREEN),
                (ColorLabel::Blue, Theme::LABEL_BLUE),
                (ColorLabel::Purple, Theme::LABEL_PURPLE),
            ];

            for (label, color) in all_colors {
                let active = self.color_labels.contains(&label);

                // Color block button
                let size = egui::Vec2::new(16.0, 16.0);
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());

                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    // Draw box
                    painter.rect_filled(
                        rect,
                        egui::CornerRadius::same(2),
                        if active {
                            color
                        } else {
                            color.gamma_multiply(0.3)
                        },
                    );

                    if active {
                        painter.rect_stroke(
                            rect.expand(2.0),
                            egui::CornerRadius::same(3),
                            egui::Stroke::new(1.0, Color32::WHITE),
                            egui::StrokeKind::Outside,
                        );
                    }
                }

                if response.clicked() {
                    if active {
                        self.color_labels.remove(&label);
                    } else {
                        self.color_labels.insert(label);
                    }
                }
                ui.add_space(2.0);
            }

            // Reset button if any active
            if self.is_active() {
                ui.separator();
                // Assuming we might have a combobox for "Filters Off" like LR, but a simple text button works.
                egui::ComboBox::from_id_salt("filter_preset")
                    .selected_text(RichText::new("Custom Filter").italics())
                    .show_ui(ui, |ui| {
                        if ui.button("Filters Off").clicked() {
                            self.reset();
                            ui.close_menu();
                        }
                    });

                if ui.button("❎").on_hover_text("Turn off filters").clicked() {
                    self.reset();
                }
            } else {
                ui.separator();
                ui.label(RichText::new("Filters Off").weak());
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::adapters::view_models::PhotoViewModel;
    use std::path::PathBuf;

    fn create_dummy_photo(
        id: &str,
        flag: Option<i32>,
        rating: i32,
        color: Option<&str>,
        path: &str,
    ) -> PhotoViewModel {
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
        let mut filter = FilmstripFilter::new();
        let p1 = create_dummy_photo("1", Some(1), 0, None, "/a/b.jpg"); // Pick
        let p2 = create_dummy_photo("2", Some(0), 0, None, "/a/c.jpg"); // Unflagged explicitly
        let p3 = create_dummy_photo("3", Some(-1), 0, None, "/a/d.jpg"); // Reject
        let p4 = create_dummy_photo("4", None, 0, None, "/a/e.jpg"); // No flag (Unflagged implicitly)

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
        let mut filter = FilmstripFilter::new();
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
        let mut filter = FilmstripFilter::new();
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
        let mut filter = FilmstripFilter::new();
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
