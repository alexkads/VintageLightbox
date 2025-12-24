//! Image Viewer Component
//!
//! Displays the main photo with zoom/pan capabilities using Cairo.

use gtk4::prelude::*;
use relm4::prelude::*;
use gdk_pixbuf::Pixbuf;

use crate::utils::image_conversion::{load_image_from_file, dynamic_image_to_pixbuf};

/// Image viewer component
pub struct ImageViewer {
    current_image_path: Option<String>,
    current_pixbuf: Option<Pixbuf>,
    zoom_level: f64,
    pan_x: f64,
    pan_y: f64,
}

/// Messages for the image viewer
#[derive(Debug)]
pub enum ImageViewerMsg {
    LoadImage(String),
    SetZoom(f64),
    ZoomIn,
    ZoomOut,
    ZoomFit,
    Pan { dx: f64, dy: f64 },
    ResetView,
}

/// Output messages from the image viewer
#[derive(Debug)]
pub enum ImageViewerOutput {
    // Currently no outputs needed
}

#[relm4::component(pub)]
impl SimpleComponent for ImageViewer {
    type Init = ();
    type Input = ImageViewerMsg;
    type Output = ImageViewerOutput;

    view! {
        gtk4::Overlay {
            set_hexpand: true,
            set_vexpand: true,
            
            // Drawing area for custom image rendering
            #[name = "drawing_area"]
            gtk4::DrawingArea {
                set_hexpand: true,
                set_vexpand: true,
                add_css_class: "image-viewer",
                
                // Custom drawing function
                set_draw_func: {
                    let pixbuf_opt = model.current_pixbuf.clone();
                    let zoom = model.zoom_level;
                    let pan_x = model.pan_x;
                    let pan_y = model.pan_y;
                    
                    move |_, cr, width, height| {
                        let width = width as f64;
                        let height = height as f64;
                        
                        if let Some(pixbuf) = &pixbuf_opt {
                            let img_w = pixbuf.width() as f64;
                            let img_h = pixbuf.height() as f64;

                            // Calculate scale to fit
                            let scale_w = width / img_w;
                            let scale_h = height / img_h;
                            let base_scale = scale_w.min(scale_h);
                            let scale = base_scale * zoom;

                            // Apply transformations
                            cr.translate(width / 2.0 + pan_x, height / 2.0 + pan_y);
                            cr.scale(scale, scale);
                            cr.set_source_pixbuf(pixbuf, -img_w / 2.0, -img_h / 2.0);
                            
                            if let Err(e) = cr.paint() {
                                eprintln!("Failed to paint image: {}", e);
                            }
                        } else {
                             // Draw placeholder text if no image
                             cr.set_source_rgb(0.5, 0.5, 0.5);
                             cr.select_font_face("Sans", gtk4::cairo::FontSlant::Normal, gtk4::cairo::FontWeight::Normal);
                             cr.set_font_size(20.0);
                             let text = "No image selected";
                             let extents = cr.text_extents(text).unwrap();
                             cr.move_to(width / 2.0 - extents.width() / 2.0, height / 2.0 + extents.height() / 2.0);
                             let _ = cr.show_text(text);
                        }
                    }
                },
                
                // Zoom scroll controller
                add_controller = gtk4::EventControllerScroll {
                    set_flags: gtk4::EventControllerScrollFlags::VERTICAL,
                    connect_scroll[sender] => move |_, _dx, dy| {
                        if dy > 0.0 {
                            sender.input(ImageViewerMsg::ZoomOut);
                        } else {
                            sender.input(ImageViewerMsg::ZoomIn);
                        }
                        gtk4::glib::Propagation::Stop
                    },
                },
                
                // Pan drag controller
                add_controller = gtk4::GestureDrag {
                    connect_drag_update[sender] => move |_, dx, dy| {
                        sender.input(ImageViewerMsg::Pan { dx, dy });
                    },
                },
            },
            
            // Zoom controls overlay
            add_overlay = &gtk4::Box {
                set_orientation: gtk4::Orientation::Horizontal,
                set_halign: gtk4::Align::End,
                set_valign: gtk4::Align::Start,
                set_margin_all: 8,
                set_spacing: 4,
                add_css_class: "zoom-controls",
                
                gtk4::Button {
                    set_icon_name: "zoom-out-symbolic",
                    set_tooltip_text: Some("Zoom Out"),
                    add_css_class: "circular",
                    connect_clicked => ImageViewerMsg::ZoomOut,
                },
                
                gtk4::Label {
                    #[watch]
                    set_label: &format!("{:.0}%", model.zoom_level * 100.0),
                    set_width_chars: 5,
                },
                
                gtk4::Button {
                    set_icon_name: "zoom-in-symbolic",
                    set_tooltip_text: Some("Zoom In"),
                    add_css_class: "circular",
                    connect_clicked => ImageViewerMsg::ZoomIn,
                },
                
                gtk4::Button {
                    set_icon_name: "zoom-fit-best-symbolic",
                    set_tooltip_text: Some("Fit to Window"),
                    add_css_class: "circular",
                    connect_clicked => ImageViewerMsg::ZoomFit,
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = ImageViewer {
            current_image_path: None,
            current_pixbuf: None,
            zoom_level: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            ImageViewerMsg::LoadImage(path) => {
                self.current_image_path = Some(path.clone());
                self.zoom_level = 1.0;
                self.pan_x = 0.0;
                self.pan_y = 0.0;
                
                // Load image into Pixbuf
                match load_image_from_file(&path) {
                    Ok(img) => {
                         match dynamic_image_to_pixbuf(&img) {
                             Ok(pixbuf) => self.current_pixbuf = Some(pixbuf),
                             Err(e) => {
                                 eprintln!("Failed to convert image to pixbuf: {}", e);
                                 self.current_pixbuf = None;
                             }
                         }
                    },
                    Err(e) => {
                        eprintln!("Failed to load image: {}", e);
                        self.current_pixbuf = None;
                    }
                }
            }
            ImageViewerMsg::SetZoom(level) => {
                self.zoom_level = level.clamp(0.1, 10.0);
            }
            ImageViewerMsg::ZoomIn => {
                self.zoom_level = (self.zoom_level * 1.25).min(10.0);
            }
            ImageViewerMsg::ZoomOut => {
                self.zoom_level = (self.zoom_level / 1.25).max(0.1);
            }
            ImageViewerMsg::ZoomFit => {
                self.zoom_level = 1.0;
                self.pan_x = 0.0;
                self.pan_y = 0.0;
            }
            ImageViewerMsg::Pan { dx, dy } => {
                self.pan_x += dx;
                self.pan_y += dy;
            }
            ImageViewerMsg::ResetView => {
                self.zoom_level = 1.0;
                self.pan_x = 0.0;
                self.pan_y = 0.0;
            }
        }
    }
}
