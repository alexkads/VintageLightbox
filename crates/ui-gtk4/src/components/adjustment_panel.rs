//! Adjustment Panel Component
//!
//! Groups all editing sliders (Exposure, Contrast, etc.) into a cohesive panel.

use gtk4::prelude::*;
use relm4::prelude::*;

use crate::components::slider_control::{SliderControl, SliderInit, SliderOutput};

/// Adjustment panel component
pub struct AdjustmentPanel {
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AdjustmentType {
    Exposure,
    Contrast,
    Temperature,
    Tint,
    Highlights,
    Shadows,
    Whites,
    Blacks,
    Clarity,
    Vibrance,
    Saturation,
}

#[derive(Debug)]
pub enum AdjustmentPanelMsg {
    SetAdjustment(AdjustmentType, f64),
    ResetAll,
    InternalSliderUpdate(AdjustmentType, f64, bool), // val, is_debounced
}

#[derive(Debug)]
pub enum AdjustmentPanelOutput {
    AdjustmentChanged(AdjustmentType, f32), // Live update
    AdjustmentConfirmed(AdjustmentType, f32), // Debounced/Saved update
}

#[relm4::component(pub)]
impl SimpleComponent for AdjustmentPanel {
    type Init = ();
    type Input = AdjustmentPanelMsg;
    type Output = AdjustmentPanelOutput;

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            set_spacing: 16,
            set_margin_all: 8,
            
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
                    
                },
                
                model.exposure_slider.widget() {},
                model.contrast_slider.widget() {},
            },
            
            gtk4::Separator { set_orientation: gtk4::Orientation::Horizontal },
            
            // White Balance
            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                
                gtk4::Label {
                    set_label: "White Balance",
                    add_css_class: "heading",
                    set_halign: gtk4::Align::Start,
                    set_margin_bottom: 8,
                },
                
                model.temperature_slider.widget() {},
                model.tint_slider.widget() {},
            },
            
            gtk4::Separator { set_orientation: gtk4::Orientation::Horizontal },
            
            // Tone
            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                
                gtk4::Label {
                    set_label: "Tone",
                    add_css_class: "heading",
                    set_halign: gtk4::Align::Start,
                    set_margin_bottom: 8,
                },
                
                model.highlights_slider.widget() {},
                model.shadows_slider.widget() {},
                model.whites_slider.widget() {},
                model.blacks_slider.widget() {},
            },
            
            gtk4::Separator { set_orientation: gtk4::Orientation::Horizontal },
            
            // Presence
            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                
                gtk4::Label {
                    set_label: "Presence",
                    add_css_class: "heading",
                    set_halign: gtk4::Align::Start,
                    set_margin_bottom: 8,
                },
                
                model.clarity_slider.widget() {},
                model.vibrance_slider.widget() {},
                model.saturation_slider.widget() {},
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let exposure_slider = create_slider("Exposure", -5.0, 5.0, AdjustmentType::Exposure, &sender);
        let contrast_slider = create_slider("Contrast", -100.0, 100.0, AdjustmentType::Contrast, &sender);
        let temperature_slider = create_slider("Temp", -10.0, 10.0, AdjustmentType::Temperature, &sender);
        let tint_slider = create_slider("Tint", -10.0, 10.0, AdjustmentType::Tint, &sender);
        let highlights_slider = create_slider("Highlights", -100.0, 100.0, AdjustmentType::Highlights, &sender);
        let shadows_slider = create_slider("Shadows", -100.0, 100.0, AdjustmentType::Shadows, &sender);
        let whites_slider = create_slider("Whites", -100.0, 100.0, AdjustmentType::Whites, &sender);
        let blacks_slider = create_slider("Blacks", -100.0, 100.0, AdjustmentType::Blacks, &sender);
        let clarity_slider = create_slider("Clarity", -1.0, 1.0, AdjustmentType::Clarity, &sender);
        let vibrance_slider = create_slider("Vibrance", -1.0, 1.0, AdjustmentType::Vibrance, &sender);
        let saturation_slider = create_slider("Saturation", -1.0, 1.0, AdjustmentType::Saturation, &sender);

        let model = AdjustmentPanel {
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
            AdjustmentPanelMsg::SetAdjustment(adj_type, value) => {
                let slider_msg = crate::components::slider_control::SliderMsg::SetValue(value);
                match adj_type {
                    AdjustmentType::Exposure => self.exposure_slider.emit(slider_msg),
                    AdjustmentType::Contrast => self.contrast_slider.emit(slider_msg),
                    AdjustmentType::Temperature => self.temperature_slider.emit(slider_msg),
                    AdjustmentType::Tint => self.tint_slider.emit(slider_msg),
                    AdjustmentType::Highlights => self.highlights_slider.emit(slider_msg),
                    AdjustmentType::Shadows => self.shadows_slider.emit(slider_msg),
                    AdjustmentType::Whites => self.whites_slider.emit(slider_msg),
                    AdjustmentType::Blacks => self.blacks_slider.emit(slider_msg),
                    AdjustmentType::Clarity => self.clarity_slider.emit(slider_msg),
                    AdjustmentType::Vibrance => self.vibrance_slider.emit(slider_msg),
                    AdjustmentType::Saturation => self.saturation_slider.emit(slider_msg),
                }
            }
            AdjustmentPanelMsg::ResetAll => {
                let reset_msg = crate::components::slider_control::SliderMsg::Reset;
                self.exposure_slider.emit(reset_msg.clone());
                self.contrast_slider.emit(reset_msg.clone());
                self.temperature_slider.emit(reset_msg.clone());
                self.tint_slider.emit(reset_msg.clone());
                self.highlights_slider.emit(reset_msg.clone());
                self.shadows_slider.emit(reset_msg.clone());
                self.whites_slider.emit(reset_msg.clone());
                self.blacks_slider.emit(reset_msg.clone());
                self.clarity_slider.emit(reset_msg.clone());
                self.vibrance_slider.emit(reset_msg.clone());
                self.saturation_slider.emit(reset_msg);
            }
            AdjustmentPanelMsg::InternalSliderUpdate(adj_type, value, is_debounced) => {
                if is_debounced {
                    let _ = sender.output(AdjustmentPanelOutput::AdjustmentConfirmed(adj_type, value as f32));
                } else {
                    let _ = sender.output(AdjustmentPanelOutput::AdjustmentChanged(adj_type, value as f32));
                }
            }
        }
    }
}

fn create_slider(
    label: &str, 
    min: f64, 
    max: f64, 
    adj_type: AdjustmentType, 
    sender: &ComponentSender<AdjustmentPanel>
) -> Controller<SliderControl> {
    SliderControl::builder()
        .launch(SliderInit { 
            label: label.to_string(), 
            min, 
            max, 
            default: 0.0 
        })
        .forward(sender.input_sender(), move |msg| {
            match msg {
                SliderOutput::ValueChanged(v) => AdjustmentPanelMsg::InternalSliderUpdate(adj_type, v, false),
                SliderOutput::DebouncedValue(v) => AdjustmentPanelMsg::InternalSliderUpdate(adj_type, v, true),
            }
        })
}
