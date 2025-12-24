//! Color Labels Component
//!
//! Color label selector (Red, Yellow, Green, Blue, Purple).

use gtk4::prelude::*;
use relm4::prelude::*;

use domain::value_objects::ColorLabel;

/// Color labels component
pub struct ColorLabels {
    selected: Option<ColorLabel>,
}

/// Messages for color labels
#[derive(Debug)]
pub enum ColorLabelsMsg {
    SetSelected(Option<ColorLabel>),
    Select(ColorLabel),
    Clear,
}

/// Output messages from color labels
#[derive(Debug)]
pub enum ColorLabelsOutput {
    ColorLabelChanged(Option<ColorLabel>),
}

#[relm4::component(pub)]
impl SimpleComponent for ColorLabels {
    type Init = Option<ColorLabel>;
    type Input = ColorLabelsMsg;
    type Output = ColorLabelsOutput;

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Horizontal,
            set_spacing: 4,
            add_css_class: "color-labels",
            
            // Red
            gtk4::Button {
                add_css_class: "circular",
                add_css_class: "color-button",
                add_css_class: "color-red",
                #[watch]
                add_css_class: if model.selected == Some(ColorLabel::Red) { "selected" } else { "" },
                set_tooltip_text: Some("Red (6)"),
                connect_clicked => ColorLabelsMsg::Select(ColorLabel::Red),
            },
            
            // Yellow
            gtk4::Button {
                add_css_class: "circular",
                add_css_class: "color-button",
                add_css_class: "color-yellow",
                #[watch]
                add_css_class: if model.selected == Some(ColorLabel::Yellow) { "selected" } else { "" },
                set_tooltip_text: Some("Yellow (7)"),
                connect_clicked => ColorLabelsMsg::Select(ColorLabel::Yellow),
            },
            
            // Green
            gtk4::Button {
                add_css_class: "circular",
                add_css_class: "color-button",
                add_css_class: "color-green",
                #[watch]
                add_css_class: if model.selected == Some(ColorLabel::Green) { "selected" } else { "" },
                set_tooltip_text: Some("Green (8)"),
                connect_clicked => ColorLabelsMsg::Select(ColorLabel::Green),
            },
            
            // Blue
            gtk4::Button {
                add_css_class: "circular",
                add_css_class: "color-button",
                add_css_class: "color-blue",
                #[watch]
                add_css_class: if model.selected == Some(ColorLabel::Blue) { "selected" } else { "" },
                set_tooltip_text: Some("Blue (9)"),
                connect_clicked => ColorLabelsMsg::Select(ColorLabel::Blue),
            },
            
            // Purple
            gtk4::Button {
                add_css_class: "circular",
                add_css_class: "color-button",
                add_css_class: "color-purple",
                #[watch]
                add_css_class: if model.selected == Some(ColorLabel::Purple) { "selected" } else { "" },
                set_tooltip_text: Some("Purple"),
                connect_clicked => ColorLabelsMsg::Select(ColorLabel::Purple),
            },
            
            // Clear
            gtk4::Button {
                set_icon_name: "edit-clear-symbolic",
                set_tooltip_text: Some("Clear color label"),
                add_css_class: "flat",
                add_css_class: "circular",
                set_margin_start: 4,
                connect_clicked => ColorLabelsMsg::Clear,
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = ColorLabels { selected: init };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ColorLabelsMsg::SetSelected(label) => {
                self.selected = label;
            }
            ColorLabelsMsg::Select(label) => {
                // Toggle: clicking the selected label clears it
                let new_label = if self.selected == Some(label.clone()) {
                    None
                } else {
                    Some(label)
                };
                self.selected = new_label.clone();
                let _ = sender.output(ColorLabelsOutput::ColorLabelChanged(new_label));
            }
            ColorLabelsMsg::Clear => {
                self.selected = None;
                let _ = sender.output(ColorLabelsOutput::ColorLabelChanged(None));
            }
        }
    }
}
