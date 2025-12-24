//! Histogram Component
//!
//! Displays RGB histogram of the current image.

use gtk4::prelude::*;
use relm4::prelude::*;

/// Histogram component
pub struct Histogram {
    data: Option<HistogramData>,
}

/// RGB histogram data
#[derive(Debug, Clone)]
pub struct HistogramData {
    pub red: [u32; 256],
    pub green: [u32; 256],
    pub blue: [u32; 256],
    pub luminance: [u32; 256],
}

impl Default for HistogramData {
    fn default() -> Self {
        Self {
            red: [0; 256],
            green: [0; 256],
            blue: [0; 256],
            luminance: [0; 256],
        }
    }
}

/// Messages for the histogram
#[derive(Debug)]
pub enum HistogramMsg {
    SetData(HistogramData),
    Clear,
}

#[relm4::component(pub)]
impl SimpleComponent for Histogram {
    type Init = ();
    type Input = HistogramMsg;
    type Output = ();

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            add_css_class: "histogram-panel",
            
            gtk4::Label {
                set_label: "Histogram",
                add_css_class: "panel-title",
                set_halign: gtk4::Align::Start,
                set_margin_bottom: 8,
            },
            
            #[name = "drawing_area"]
            gtk4::DrawingArea {
                set_height_request: 100,
                set_hexpand: true,
                add_css_class: "histogram",
                
                set_draw_func: {
                    let data = model.data.clone();
                    move |_, cr, width, height| {
                        draw_histogram(cr, width as f64, height as f64, data.as_ref());
                    }
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Histogram { data: None };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            HistogramMsg::SetData(data) => {
                self.data = Some(data);
            }
            HistogramMsg::Clear => {
                self.data = None;
            }
        }
    }
}

/// Draw the histogram using Cairo
fn draw_histogram(
    cr: &gtk4::cairo::Context,
    width: f64,
    height: f64,
    data: Option<&HistogramData>,
) {
    // Background
    cr.set_source_rgb(0.1, 0.1, 0.1);
    let _ = cr.paint();
    
    let Some(data) = data else {
        return;
    };
    
    // Find max value for normalization
    let max_value = data.red.iter()
        .chain(data.green.iter())
        .chain(data.blue.iter())
        .copied()
        .max()
        .unwrap_or(1) as f64;
    
    let bar_width = width / 256.0;
    
    // Draw RGB channels with transparency
    for channel in [(&data.red, (1.0, 0.2, 0.2)), (&data.green, (0.2, 1.0, 0.2)), (&data.blue, (0.2, 0.2, 1.0))] {
        let (histogram, (r, g, b)) = channel;
        cr.set_source_rgba(r, g, b, 0.5);
        
        for (i, &value) in histogram.iter().enumerate() {
            let bar_height = (value as f64 / max_value) * height;
            let x = i as f64 * bar_width;
            let y = height - bar_height;
            
            cr.rectangle(x, y, bar_width, bar_height);
        }
        
        let _ = cr.fill();
    }
}
