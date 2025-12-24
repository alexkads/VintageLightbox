//! Develop View
//!
//! Photo editing view with image viewer and adjustment panels.

use gtk4::prelude::*;
use relm4::prelude::*;

use adapters::view_models::PhotoViewModel;
use crate::components::image_viewer::{ImageViewer, ImageViewerMsg};
use crate::components::histogram::{Histogram, HistogramMsg};
use crate::components::slider_control::{SliderControl, SliderInit, SliderOutput};

/// Develop view component
pub struct DevelopView {
    photos: Vec<PhotoViewModel>,
    
    image_viewer: Controller<ImageViewer>,
    histogram: Controller<Histogram>,
    
    // Sliders
    exposure_slider: Controller<SliderControl>,
    contrast_slider: Controller<SliderControl>,
    temperature_slider: Controller<SliderControl>,
    tint_slider: Controller<SliderControl>,
    highlights_slider: Controller<SliderControl>,
    shadows_slider: Controller<SliderControl>,
    whites_slider: Controller<SliderControl>,
    blacks_slider: Controller<SliderControl>,
    clarity_slider: Controller<SliderControl>,
    vibrance_slider: Controller<SliderControl>,
    saturation_slider: Controller<SliderControl>,
}

/// Messages for the develop view
#[derive(Debug)]
pub enum DevelopViewMsg {
    SetPhotos(Vec<PhotoViewModel>),

    // Slider changes
    ExposureChanged(f64),
    ContrastChanged(f64),
    TemperatureChanged(f64),
    TintChanged(f64),
    HighlightsChanged(f64),
    ShadowsChanged(f64),
    WhitesChanged(f64),
    BlacksChanged(f64),
    ClarityChanged(f64),
    VibranceChanged(f64),
    SaturationChanged(f64),
    
    // Actions
    ResetAdjustments,
    NavigateNext,
    NavigatePrevious,
    PhotoSelected(String),
}

/// Output messages from the develop view
#[derive(Debug)]
pub enum DevelopViewOutput {
    ExposureChanged(f32),
    ContrastChanged(f32),
    TemperatureChanged(f32),
    TintChanged(f32),
    HighlightsChanged(f32),
    ShadowsChanged(f32),
    WhitesChanged(f32),
    BlacksChanged(f32),
    ClarityChanged(f32),
    VibranceChanged(f32),
    SaturationChanged(f32),
    ResetAdjustments,
    NavigateNext,
    NavigatePrevious,
}

#[relm4::component(pub)]
impl SimpleComponent for DevelopView {
    type Init = ();
    type Input = DevelopViewMsg;
    type Output = DevelopViewOutput;

    view! {
        gtk4::Paned {
            set_orientation: gtk4::Orientation::Horizontal,
            set_position: 1000,
            set_shrink_start_child: false,
            set_shrink_end_child: false,
            
            // Main content: Image viewer + Filmstrip
            #[wrap(Some)]
            set_start_child = &gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                set_hexpand: true,
                
                // Image viewer
                model.image_viewer.widget() {
                    set_vexpand: true,
                },
                
                gtk4::Separator {
                    set_orientation: gtk4::Orientation::Horizontal,
                },
                
                // Filmstrip area (Bottom)
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Vertical,
                    set_height_request: 100,
                    add_css_class: "filmstrip-container",
                    
                    gtk4::ScrolledWindow {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_policy: (gtk4::PolicyType::Automatic, gtk4::PolicyType::Never),
                        
                        #[name(filmstrip_box)]
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Horizontal,
                            set_spacing: 8,
                            set_margin_all: 8,
                            set_halign: gtk4::Align::Center,
                        },
                    },
                },
            },
            
            // Right sidebar: Adjustment panels
            #[wrap(Some)]
            set_end_child = &gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                set_width_request: 320,
                add_css_class: "sidebar",
                add_css_class: "develop-panel",
                
                gtk4::ScrolledWindow {
                    set_vexpand: true,
                    set_policy: (gtk4::PolicyType::Never, gtk4::PolicyType::Automatic),
                    
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Vertical,
                        set_margin_all: 8,
                        set_spacing: 16,
                        
                        // Histogram
                        model.histogram.widget() {},
                        
                        gtk4::Separator {
                            set_orientation: gtk4::Orientation::Horizontal,
                        },
                        
                        // Basic adjustments section
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            
                            gtk4::Box {
                                set_orientation: gtk4::Orientation::Horizontal,
                                set_margin_bottom: 8,
                                
                                gtk4::Label {
                                    set_label: "Basic",
                                    add_css_class: "heading",
                                    set_halign: gtk4::Align::Start,
                                    set_hexpand: true,
                                },
                                
                                gtk4::Button {
                                    set_label: "Reset",
                                    add_css_class: "flat",
                                    connect_clicked => DevelopViewMsg::ResetAdjustments,
                                },
                            },
                            
                            // Exposure
                            model.exposure_slider.widget() {},
                            
                            // Contrast
                            model.contrast_slider.widget() {},
                        },
                        
                        gtk4::Separator {
                            set_orientation: gtk4::Orientation::Horizontal,
                        },
                        
                        // White Balance section
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            
                            gtk4::Label {
                                set_label: "White Balance",
                                add_css_class: "heading",
                                set_halign: gtk4::Align::Start,
                                set_margin_bottom: 8,
                            },
                            
                            // Temperature
                            model.temperature_slider.widget() {},
                            
                            // Tint
                            model.tint_slider.widget() {},
                        },
                        
                        gtk4::Separator {
                            set_orientation: gtk4::Orientation::Horizontal,
                        },
                        
                        // Tone section
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            
                            gtk4::Label {
                                set_label: "Tone",
                                add_css_class: "heading",
                                set_halign: gtk4::Align::Start,
                                set_margin_bottom: 8,
                            },
                            
                            // Highlights
                            model.highlights_slider.widget() {},
                            
                            // Shadows
                            model.shadows_slider.widget() {},
                            
                            // Whites
                            model.whites_slider.widget() {},
                            
                            // Blacks
                            model.blacks_slider.widget() {},
                        },
                        
                        gtk4::Separator {
                            set_orientation: gtk4::Orientation::Horizontal,
                        },
                        
                        // Presence section
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            
                            gtk4::Label {
                                set_label: "Presence",
                                add_css_class: "heading",
                                set_halign: gtk4::Align::Start,
                                set_margin_bottom: 8,
                            },
                            
                            // Clarity
                            model.clarity_slider.widget() {},
                            
                            // Vibrance
                            model.vibrance_slider.widget() {},
                            
                            // Saturation
                            model.saturation_slider.widget() {},
                        },
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Create image viewer
        let image_viewer = ImageViewer::builder()
            .launch(())
            .detach();
        
        // Create histogram
        let histogram = Histogram::builder()
            .launch(())
            .detach();
        
        // Create sliders
        let exposure_slider = SliderControl::builder()
            .launch(SliderInit { label: "Exposure".to_string(), min: -5.0, max: 5.0, default: 0.0 })
            .forward(sender.input_sender(), |msg| {
                match msg {
                    SliderOutput::ValueChanged(v) => DevelopViewMsg::ExposureChanged(v),
                }
            });
        
        let contrast_slider = SliderControl::builder()
            .launch(SliderInit { label: "Contrast".to_string(), min: -100.0, max: 100.0, default: 0.0 })
            .forward(sender.input_sender(), |msg| {
                match msg {
                    SliderOutput::ValueChanged(v) => DevelopViewMsg::ContrastChanged(v),
                }
            });
        
        let temperature_slider = SliderControl::builder()
            .launch(SliderInit { label: "Temp".to_string(), min: -10.0, max: 10.0, default: 0.0 })
            .forward(sender.input_sender(), |msg| {
                match msg {
                    SliderOutput::ValueChanged(v) => DevelopViewMsg::TemperatureChanged(v),
                }
            });
        
        let tint_slider = SliderControl::builder()
            .launch(SliderInit { label: "Tint".to_string(), min: -10.0, max: 10.0, default: 0.0 })
            .forward(sender.input_sender(), |msg| {
                match msg {
                    SliderOutput::ValueChanged(v) => DevelopViewMsg::TintChanged(v),
                }
            });
        
        let highlights_slider = SliderControl::builder()
            .launch(SliderInit { label: "Highlights".to_string(), min: -100.0, max: 100.0, default: 0.0 })
            .forward(sender.input_sender(), |msg| {
                match msg {
                    SliderOutput::ValueChanged(v) => DevelopViewMsg::HighlightsChanged(v),
                }
            });
        
        let shadows_slider = SliderControl::builder()
            .launch(SliderInit { label: "Shadows".to_string(), min: -100.0, max: 100.0, default: 0.0 })
            .forward(sender.input_sender(), |msg| {
                match msg {
                    SliderOutput::ValueChanged(v) => DevelopViewMsg::ShadowsChanged(v),
                }
            });
        
        let whites_slider = SliderControl::builder()
            .launch(SliderInit { label: "Whites".to_string(), min: -100.0, max: 100.0, default: 0.0 })
            .forward(sender.input_sender(), |msg| {
                match msg {
                    SliderOutput::ValueChanged(v) => DevelopViewMsg::WhitesChanged(v),
                }
            });
        
        let blacks_slider = SliderControl::builder()
            .launch(SliderInit { label: "Blacks".to_string(), min: -100.0, max: 100.0, default: 0.0 })
            .forward(sender.input_sender(), |msg| {
                match msg {
                    SliderOutput::ValueChanged(v) => DevelopViewMsg::BlacksChanged(v),
                }
            });
        
        let clarity_slider = SliderControl::builder()
            .launch(SliderInit { label: "Clarity".to_string(), min: -1.0, max: 1.0, default: 0.0 })
            .forward(sender.input_sender(), |msg| {
                match msg {
                    SliderOutput::ValueChanged(v) => DevelopViewMsg::ClarityChanged(v),
                }
            });
        
        let vibrance_slider = SliderControl::builder()
            .launch(SliderInit { label: "Vibrance".to_string(), min: -1.0, max: 1.0, default: 0.0 })
            .forward(sender.input_sender(), |msg| {
                match msg {
                    SliderOutput::ValueChanged(v) => DevelopViewMsg::VibranceChanged(v),
                }
            });
        
        let saturation_slider = SliderControl::builder()
            .launch(SliderInit { label: "Saturation".to_string(), min: -1.0, max: 1.0, default: 0.0 })
            .forward(sender.input_sender(), |msg| {
                match msg {
                    SliderOutput::ValueChanged(v) => DevelopViewMsg::SaturationChanged(v),
                }
            });
        
        let model = DevelopView {
            photos: Vec::new(),
            image_viewer,
            histogram,
            exposure_slider,
            contrast_slider,
            temperature_slider,
            tint_slider,
            highlights_slider,
            shadows_slider,
            whites_slider,
            blacks_slider,
            clarity_slider,
            vibrance_slider,
            saturation_slider,
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            DevelopViewMsg::SetPhotos(photos) => {
                println!("DevelopView: Received {} photos", photos.len());
                self.photos = photos;
            }
            DevelopViewMsg::ExposureChanged(v) => {
                let _ = sender.output(DevelopViewOutput::ExposureChanged(v as f32));
            }
            DevelopViewMsg::ContrastChanged(v) => {
                let _ = sender.output(DevelopViewOutput::ContrastChanged(v as f32));
            }
            DevelopViewMsg::TemperatureChanged(v) => {
                let _ = sender.output(DevelopViewOutput::TemperatureChanged(v as f32));
            }
            DevelopViewMsg::TintChanged(v) => {
                let _ = sender.output(DevelopViewOutput::TintChanged(v as f32));
            }
            DevelopViewMsg::HighlightsChanged(v) => {
                let _ = sender.output(DevelopViewOutput::HighlightsChanged(v as f32));
            }
            DevelopViewMsg::ShadowsChanged(v) => {
                let _ = sender.output(DevelopViewOutput::ShadowsChanged(v as f32));
            }
            DevelopViewMsg::WhitesChanged(v) => {
                let _ = sender.output(DevelopViewOutput::WhitesChanged(v as f32));
            }
            DevelopViewMsg::BlacksChanged(v) => {
                let _ = sender.output(DevelopViewOutput::BlacksChanged(v as f32));
            }
            DevelopViewMsg::ClarityChanged(v) => {
                let _ = sender.output(DevelopViewOutput::ClarityChanged(v as f32));
            }
            DevelopViewMsg::VibranceChanged(v) => {
                let _ = sender.output(DevelopViewOutput::VibranceChanged(v as f32));
            }
            DevelopViewMsg::SaturationChanged(v) => {
                let _ = sender.output(DevelopViewOutput::SaturationChanged(v as f32));
            }
            DevelopViewMsg::ResetAdjustments => {
                let _ = sender.output(DevelopViewOutput::ResetAdjustments);
            }
            DevelopViewMsg::NavigateNext => {
                let _ = sender.output(DevelopViewOutput::NavigateNext);
            }
            DevelopViewMsg::NavigatePrevious => {
                let _ = sender.output(DevelopViewOutput::NavigatePrevious);
            }
            DevelopViewMsg::PhotoSelected(_id) => {
                // TODO: Handle selection
            }
        }
    }
    
    fn post_view(&self, widgets: &mut Self::Widgets) {
        // Populate filmstrip
        while let Some(child) = widgets.filmstrip_box.first_child() {
            widgets.filmstrip_box.remove(&child);
        }

        println!("DevelopView: Populating filmstrip with {} items", self.photos.len());
        for photo in &self.photos {
            let item = create_filmstrip_item(photo);
            widgets.filmstrip_box.append(&item);
        }
    }
}

// Helper for filmstrip item (same as LibraryView)
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
