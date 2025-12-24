//! Photo Grid Component
//!
//! Displays photos in a grid layout using GtkFlowBox for simpler rendering.

use gtk4::prelude::*;
use gtk4::gio;
use gtk4::glib;
use relm4::prelude::*;

use adapters::view_models::PhotoViewModel;

/// Photo grid component using FlowBox for simpler rendering
pub struct PhotoGrid {
    photos: Vec<PhotoViewModel>,
    flow_box: gtk4::FlowBox,
}

/// Messages for the photo grid
#[derive(Debug)]
pub enum PhotoGridMsg {
    SetPhotos(Vec<PhotoViewModel>),
    Select(u32),
    Activate(u32),
}

/// Output messages from the photo grid
#[derive(Debug)]
pub enum PhotoGridOutput {
    PhotoSelected(String),
    PhotoActivated(String),
}

#[relm4::component(pub)]
impl SimpleComponent for PhotoGrid {
    type Init = ();
    type Input = PhotoGridMsg;
    type Output = PhotoGridOutput;

    view! {
        gtk4::ScrolledWindow {
            set_hexpand: true,
            set_vexpand: true,
            set_policy: (gtk4::PolicyType::Never, gtk4::PolicyType::Automatic),
            set_min_content_height: 200,
            
            #[local_ref]
            flow_box -> gtk4::FlowBox {
                set_valign: gtk4::Align::Start,
                set_max_children_per_line: 6,
                set_min_children_per_line: 2,
                set_selection_mode: gtk4::SelectionMode::Single,
                set_homogeneous: true,
                set_row_spacing: 8,
                set_column_spacing: 8,
                set_margin_all: 8,
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let flow_box = gtk4::FlowBox::new();
        flow_box.set_valign(gtk4::Align::Start);
        flow_box.set_max_children_per_line(6);
        flow_box.set_min_children_per_line(2);
        flow_box.set_selection_mode(gtk4::SelectionMode::Single);
        flow_box.set_homogeneous(true);
        flow_box.set_row_spacing(8);
        flow_box.set_column_spacing(8);
        flow_box.set_margin_start(8);
        flow_box.set_margin_end(8);
        flow_box.set_margin_top(8);
        flow_box.set_margin_bottom(8);

        let model = PhotoGrid {
            photos: Vec::new(),
            flow_box: flow_box.clone(),
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            PhotoGridMsg::SetPhotos(photos) => {
                println!("PhotoGrid: Received {} photos", photos.len());
                
                // Clear existing children
                while let Some(child) = self.flow_box.first_child() {
                    self.flow_box.remove(&child);
                }
                
                self.photos = photos;
                
                for (idx, photo) in self.photos.iter().enumerate() {
                    println!("  - Creating card for: {}", photo.name);
                    
                    // Create a card for each photo
                    let card = gtk4::Box::builder()
                        .orientation(gtk4::Orientation::Vertical)
                        .spacing(4)
                        .width_request(150)
                        .height_request(180)
                        .build();
                    
                    // Frame for the thumbnail placeholder
                    let thumb_frame = gtk4::Frame::builder()
                        .width_request(140)
                        .height_request(120)
                        .build();
                    
                    // Placeholder icon
                    let placeholder = gtk4::Label::builder()
                        .label("📷")
                        .build();
                    // Make the placeholder larger with Pango markup
                    placeholder.set_markup("<span size='xx-large'>📷</span>");
                    
                    thumb_frame.set_child(Some(&placeholder));
                    card.append(&thumb_frame);
                    
                    // Filename label
                    let filename = std::path::Path::new(&photo.path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown");
                    
                    let name_label = gtk4::Label::builder()
                        .label(filename)
                        .ellipsize(gtk4::pango::EllipsizeMode::Middle)
                        .max_width_chars(16)
                        .halign(gtk4::Align::Center)
                        .build();
                    card.append(&name_label);
                    
                    // Rating stars
                    if photo.rating > 0 {
                        let stars = "★".repeat(photo.rating as usize);
                        let rating_label = gtk4::Label::builder()
                            .label(&stars)
                            .halign(gtk4::Align::Center)
                            .build();
                        card.append(&rating_label);
                    }
                    
                    // Wrap in a FlowBoxChild
                    self.flow_box.append(&card);
                }
                
                println!("PhotoGrid: FlowBox now has {} children", 
                    self.photos.len());
                
                // Force a redraw
                self.flow_box.queue_draw();
            }
            PhotoGridMsg::Select(position) => {
                if let Some(photo) = self.photos.get(position as usize) {
                    let _ = sender.output(PhotoGridOutput::PhotoSelected(photo.id.clone()));
                }
            }
            PhotoGridMsg::Activate(position) => {
                if let Some(photo) = self.photos.get(position as usize) {
                    let _ = sender.output(PhotoGridOutput::PhotoActivated(photo.id.clone()));
                }
            }
        }
    }
}
