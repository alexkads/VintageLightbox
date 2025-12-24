//! Minimal working example of LibraryView with photos

use gtk4::prelude::*;
use relm4::prelude::*;

use adapters::view_models::PhotoViewModel;

pub struct LibraryView {
    photos: Vec<PhotoViewModel>,
}

#[derive(Debug)]
pub enum LibraryViewMsg {
    SetPhotos(Vec<PhotoViewModel>),
}

#[derive(Debug)]
pub enum LibraryViewOutput {
    SelectPhoto(String),
    OpenPhoto(String),
    SetRating(i32),
    SetColorLabel(Option<domain::value_objects::ColorLabel>),
}

#[relm4::component(pub)]
impl SimpleComponent for LibraryView {
    type Init = ();
    type Input = LibraryViewMsg;
    type Output = LibraryViewOutput;

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            set_spacing: 0,
            set_hexpand: true,
            set_vexpand: true,
            
            // Header
            gtk4::Box {
                set_orientation: gtk4::Orientation::Horizontal,
                set_margin_all: 12,
                
                gtk4::Label {
                    set_markup: "<span size='large' weight='bold'>Library</span>",
                },
            },
            
            // Main content - ScrolledWindow with FlowBox
            gtk4::ScrolledWindow {
                set_hexpand: true,
                set_vexpand: true,
                set_policy: (gtk4::PolicyType::Never, gtk4::PolicyType::Automatic),
                
                #[name(flow_box)]
                gtk4::FlowBox {
                    set_valign: gtk4::Align::Start,
                    set_max_children_per_line: 5,
                    set_min_children_per_line: 2,
                    set_selection_mode: gtk4::SelectionMode::Single,
                    set_homogeneous: false,
                    set_row_spacing: 16,
                    set_column_spacing: 16,
                    set_margin_all: 16,
                },
            },
            
            // Separator between Grid and Filmstrip
            gtk4::Separator {
                set_orientation: gtk4::Orientation::Horizontal,
            },
            
            // Filmstrip area (Bottom)
            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                set_height_request: 100,
                add_css_class: "filmstrip-container",
                
                gtk4::Label {
                    set_label: "Filmstrip",
                    set_halign: gtk4::Align::Start,
                    set_margin_start: 12,
                    set_margin_top: 4,
                    add_css_class: "caption",
                    set_opacity: 0.7,
                },

                gtk4::ScrolledWindow {
                    set_hexpand: true,
                    set_vexpand: true,
                    set_policy: (gtk4::PolicyType::Automatic, gtk4::PolicyType::Never),
                    
                    #[name(filmstrip_box)]
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 8,
                        set_margin_all: 8,
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = LibraryView {
            photos: Vec::new(),
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            LibraryViewMsg::SetPhotos(photos) => {
                println!("LibraryView: Received {} photos", photos.len());
                self.photos = photos;
                
                // Manually populate the flow_box through the widgets
                // This is the KEY - we need to access widgets.flow_box
            }
        }
    }
    
    fn post_view(&self, widgets: &mut Self::Widgets) {
        // Clear existing children
        while let Some(child) = widgets.flow_box.first_child() {
            widgets.flow_box.remove(&child);
        }
        
        // Add photo cards
        // Populate main grid
        println!("LibraryView: Populating grid with {} cards", self.photos.len());
        for photo in &self.photos {
            let card = create_photo_card(photo);
            widgets.flow_box.append(&card);
        }
        
        // Populate filmstrip
        while let Some(child) = widgets.filmstrip_box.first_child() {
            widgets.filmstrip_box.remove(&child);
        }

        println!("LibraryView: Populating filmstrip with {} items", self.photos.len());
        for photo in &self.photos {
            let item = create_filmstrip_item(photo);
            widgets.filmstrip_box.append(&item);
        }
    }
}

// Helper for filmstrip item (smaller version)
fn create_filmstrip_item(photo: &PhotoViewModel) -> gtk4::Widget {
    let item = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(2)
        .width_request(80)
        .build();

    let frame = gtk4::Frame::builder()
        .width_request(80)
        .height_request(60)
        .build();

    let image_path = photo.thumbnail_path.as_deref().unwrap_or(&photo.path);
    let file = gtk4::gio::File::for_path(image_path);
    
    let picture = gtk4::Picture::builder()
        .file(&file)
        .content_fit(gtk4::ContentFit::Cover)
        .build();
        
    frame.set_child(Some(&picture));
    item.append(&frame);
    
    // Tiny rating
    if photo.rating > 0 {
        let stars = "★".repeat(photo.rating as usize);
        let rating = gtk4::Label::builder()
            .label(&stars)
            .css_classes(["caption", "small-rating"])
            .halign(gtk4::Align::Center)
            .build();
        item.append(&rating);
    }

    item.upcast()
}

fn create_photo_card(photo: &PhotoViewModel) -> gtk4::Widget {
    let card = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(8)
        .width_request(180)
        .build();
    
    // Thumbnail frame
    let frame = gtk4::Frame::builder()
        .width_request(180)
        .height_request(140)
        .build();
    
    // Determine which path to use (prefer thumbnail, fallback to main path)
    let image_path = photo.thumbnail_path.as_deref().unwrap_or(&photo.path);
    let file = gtk4::gio::File::for_path(image_path);
    
    // Create picture widget that loads from file
    let picture = gtk4::Picture::builder()
        .file(&file)
        .content_fit(gtk4::ContentFit::Cover)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .build();
    
    frame.set_child(Some(&picture));
    card.append(&frame);
    
    // Filename
    let filename = std::path::Path::new(&photo.path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown");
    
    let name_label = gtk4::Label::builder()
        .label(filename)
        .ellipsize(gtk4::pango::EllipsizeMode::Middle)
        .max_width_chars(20)
        .wrap(true)
        .justify(gtk4::Justification::Center)
        .build();
    card.append(&name_label);
    
    // Rating
    if photo.rating > 0 {
        let stars = "★".repeat(photo.rating as usize);
        let rating = gtk4::Label::builder()
            .label(&stars)
            .build();
        card.append(&rating);
    }
    
    card.upcast()
}
