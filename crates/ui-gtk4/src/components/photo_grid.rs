//! Photo Grid Component
//!
//! Displays photos in a grid layout using GtkFlowBox with async thumbnail loading.

use gtk4::prelude::*;
use gtk4::gdk;
use relm4::prelude::*;
use relm4::WorkerController;
use std::collections::HashMap;
use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use infrastructure::cache::preview_manager::PreviewManager;
use crate::workers::thumbnail_worker::{ThumbnailWorker, ThumbnailRequest, ThumbnailResult};
use crate::utils::image_conversion::bytes_to_pixbuf;

/// Photo grid component with async thumbnail loading
pub struct PhotoGrid {
    photos: Vec<PhotoViewModel>,
    flow_box: gtk4::FlowBox,
    thumbnail_worker: WorkerController<ThumbnailWorker>,
    texture_cache: HashMap<String, gdk::Texture>,
    // Map photo_id → Picture widget for updating thumbnails
    picture_widgets: HashMap<String, gtk4::Picture>,
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
impl Component for PhotoGrid {
    type Init = Arc<PreviewManager>;
    type Input = PhotoGridMsg;
    type Output = PhotoGridOutput;
    type CommandOutput = ThumbnailResult;

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
        preview_manager: Self::Init,
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

        // Initialize ThumbnailWorker
        let thumbnail_worker = ThumbnailWorker::builder()
            .detach_worker(preview_manager)
            .forward(sender.command_sender(), |result| result);

        let model = PhotoGrid {
            photos: Vec::new(),
            flow_box: flow_box.clone(),
            thumbnail_worker,
            texture_cache: HashMap::new(),
            picture_widgets: HashMap::new(),
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            PhotoGridMsg::SetPhotos(photos) => {
                println!("PhotoGrid: Received {} photos", photos.len());

                // Clear existing children and widgets
                while let Some(child) = self.flow_box.first_child() {
                    self.flow_box.remove(&child);
                }
                self.picture_widgets.clear();

                self.photos = photos;

                for photo in self.photos.iter() {
                    println!("  - Creating card for: {}", photo.name);

                    // Create a card for each photo
                    let card = gtk4::Box::builder()
                        .orientation(gtk4::Orientation::Vertical)
                        .spacing(4)
                        .width_request(150)
                        .height_request(180)
                        .build();

                    // Create Picture widget for thumbnail
                    let picture = gtk4::Picture::builder()
                        .width_request(140)
                        .height_request(120)
                        .can_shrink(false)
                        .build();

                    // Set a placeholder while loading
                    // (Picture will show nothing until we set a paintable)

                    card.append(&picture);

                    // Store reference to update later when thumbnail loads
                    self.picture_widgets.insert(photo.id.clone(), picture);

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

                    // Add to FlowBox
                    self.flow_box.append(&card);
                }

                println!("PhotoGrid: FlowBox now has {} children", self.photos.len());

                // Request thumbnails for all photos (limit to first 50 for performance)
                let photo_ids: Vec<String> = self.photos.iter()
                    .take(50)
                    .map(|p| p.id.clone())
                    .collect();

                if !photo_ids.is_empty() {
                    println!("PhotoGrid: Requesting {} thumbnails", photo_ids.len());
                    self.thumbnail_worker.emit(ThumbnailRequest { photo_ids });
                }

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

    fn update_cmd(&mut self, result: Self::CommandOutput, _sender: ComponentSender<Self>, _root: &Self::Root) {
        // ThumbnailResult received from worker
        let ThumbnailResult { photo_id, bytes } = result;

        println!("PhotoGrid: Received thumbnail for photo_id: {}", photo_id);

        // 1. Convert bytes to Pixbuf
        match bytes_to_pixbuf(&bytes, 300, 300) {
            Ok(pixbuf) => {
                // 2. Convert Pixbuf to Texture
                let texture = gdk::Texture::for_pixbuf(&pixbuf);

                // 3. Cache the texture
                self.texture_cache.insert(photo_id.clone(), texture.clone());

                // 4. Update the Picture widget if it exists
                if let Some(picture) = self.picture_widgets.get(&photo_id) {
                    picture.set_paintable(Some(&texture));
                    println!("  - Thumbnail updated in UI");
                } else {
                    println!("  - Warning: Picture widget not found for {}", photo_id);
                }
            }
            Err(e) => {
                eprintln!("PhotoGrid: Failed to convert thumbnail for {}: {}", photo_id, e);
            }
        }
    }
}
