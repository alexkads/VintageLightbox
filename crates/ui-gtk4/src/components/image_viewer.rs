//! Image Viewer Component
//!
//! Displays the main photo with zoom/pan capabilities.

use gtk4::prelude::*;
use relm4::prelude::*;

/// Image viewer component
pub struct ImageViewer {
    current_image_path: Option<String>,
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
            
            // Scrollable image container
            gtk4::ScrolledWindow {
                set_hexpand: true,
                set_vexpand: true,
                set_policy: (gtk4::PolicyType::Automatic, gtk4::PolicyType::Automatic),
                
                #[name = "picture"]
                gtk4::Picture {
                    set_hexpand: true,
                    set_vexpand: true,
                    set_content_fit: gtk4::ContentFit::Contain,
                    add_css_class: "image-viewer",
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
            
            // Navigation arrows
            add_overlay = &gtk4::Box {
                set_orientation: gtk4::Orientation::Horizontal,
                set_halign: gtk4::Align::Fill,
                set_valign: gtk4::Align::Center,
                
                gtk4::Button {
                    set_icon_name: "go-previous-symbolic",
                    set_tooltip_text: Some("Previous Photo (Left Arrow)"),
                    set_halign: gtk4::Align::Start,
                    set_margin_start: 16,
                    add_css_class: "circular",
                    add_css_class: "osd",
                    // Navigation handled by parent
                },
                
                gtk4::Box {
                    set_hexpand: true,
                },
                
                gtk4::Button {
                    set_icon_name: "go-next-symbolic",
                    set_tooltip_text: Some("Next Photo (Right Arrow)"),
                    set_halign: gtk4::Align::End,
                    set_margin_end: 16,
                    add_css_class: "circular",
                    add_css_class: "osd",
                    // Navigation handled by parent
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = ImageViewer {
            current_image_path: None,
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
                self.current_image_path = Some(path);
                self.zoom_level = 1.0;
                self.pan_x = 0.0;
                self.pan_y = 0.0;
                // Note: Actual image loading would happen here
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
