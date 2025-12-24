//! Slider Control Component
//!
//! Customizable slider for editing adjustments.

use gtk4::prelude::*;
use relm4::prelude::*;

/// Slider control component
pub struct SliderControl {
    label: String,
    value: f64,
    min: f64,
    max: f64,
    default: f64,
}

/// Initialization for slider
pub struct SliderInit {
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
}

/// Messages for the slider
#[derive(Debug)]
pub enum SliderMsg {
    SetValue(f64),
    Reset,
}

/// Output messages from the slider
#[derive(Debug)]
pub enum SliderOutput {
    ValueChanged(f64),
}

#[relm4::component(pub)]
impl SimpleComponent for SliderControl {
    type Init = SliderInit;
    type Input = SliderMsg;
    type Output = SliderOutput;

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Horizontal,
            set_spacing: 8,
            set_margin_start: 8,
            set_margin_end: 8,
            set_margin_top: 4,
            set_margin_bottom: 4,
            add_css_class: "slider-control",
            
            // Label
            gtk4::Label {
                set_label: &model.label,
                set_width_chars: 12,
                set_xalign: 0.0,
                add_css_class: "slider-label",
            },
            
            // Slider
            #[name = "scale"]
            gtk4::Scale {
                set_orientation: gtk4::Orientation::Horizontal,
                set_hexpand: true,
                set_draw_value: false,
                set_range: (model.min, model.max),
                #[watch]
                set_value: model.value,
                
                connect_value_changed[sender] => move |scale| {
                    sender.input(SliderMsg::SetValue(scale.value()));
                },
            },
            
            // Value display
            gtk4::Label {
                #[watch]
                set_label: &format!("{:+.1}", model.value),
                set_width_chars: 6,
                add_css_class: "slider-value",
            },
            
            // Reset button
            gtk4::Button {
                set_icon_name: "edit-undo-symbolic",
                set_tooltip_text: Some("Reset to default"),
                add_css_class: "flat",
                add_css_class: "circular",
                connect_clicked => SliderMsg::Reset,
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = SliderControl {
            label: init.label,
            value: init.default,
            min: init.min,
            max: init.max,
            default: init.default,
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SliderMsg::SetValue(value) => {
                if (self.value - value).abs() > 0.001 {
                    self.value = value;
                    let _ = sender.output(SliderOutput::ValueChanged(value));
                }
            }
            SliderMsg::Reset => {
                self.value = self.default;
                let _ = sender.output(SliderOutput::ValueChanged(self.default));
            }
        }
    }
}
