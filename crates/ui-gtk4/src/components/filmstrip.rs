//! Filmstrip Component
//!
//! Horizontal strip of thumbnails at the bottom of the window.

use gtk4::prelude::*;
use relm4::prelude::*;

use adapters::view_models::PhotoViewModel;

/// Filmstrip component
pub struct Filmstrip {
    photos: Vec<PhotoViewModel>,
    selected_id: Option<String>,
}

/// Messages for the filmstrip
#[derive(Debug)]
pub enum FilmstripMsg {
    SetPhotos(Vec<PhotoViewModel>),
    SetSelected(Option<String>),
    Select(usize),
}

/// Output messages from the filmstrip
#[derive(Debug)]
pub enum FilmstripOutput {
    PhotoSelected(String),
}

#[relm4::component(pub)]
impl SimpleComponent for Filmstrip {
    type Init = ();
    type Input = FilmstripMsg;
    type Output = FilmstripOutput;

    view! {
        gtk4::ScrolledWindow {
            set_height_request: 100,
            set_policy: (gtk4::PolicyType::Automatic, gtk4::PolicyType::Never),
            add_css_class: "filmstrip",
            
            #[name = "filmstrip_box"]
            gtk4::Box {
                set_orientation: gtk4::Orientation::Horizontal,
                set_spacing: 4,
                set_margin_all: 4,
                
                // Thumbnails will be added dynamically
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Filmstrip {
            photos: Vec::new(),
            selected_id: None,
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            FilmstripMsg::SetPhotos(photos) => {
                self.photos = photos;
                // Note: In a real implementation, we would rebuild the thumbnails here
                // This requires access to the widget, which relm4 handles differently
            }
            FilmstripMsg::SetSelected(id) => {
                self.selected_id = id;
            }
            FilmstripMsg::Select(index) => {
                if let Some(photo) = self.photos.get(index) {
                    self.selected_id = Some(photo.id.clone());
                    let _ = sender.output(FilmstripOutput::PhotoSelected(photo.id.clone()));
                }
            }
        }
    }
}
