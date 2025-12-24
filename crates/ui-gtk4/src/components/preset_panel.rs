//! Preset Panel Component
//!
//! Helper component to manage presets.

use gtk4::prelude::*;
use relm4::prelude::*;

pub struct PresetPanel;

#[derive(Debug)]
pub enum PresetPanelMsg {}

#[relm4::component(pub)]
impl SimpleComponent for PresetPanel {
    type Init = ();
    type Input = PresetPanelMsg;
    type Output = ();

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            set_margin_all: 8,
            
            gtk4::Label {
                set_label: "Presets (Coming Soon)",
                add_css_class: "heading",
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = PresetPanel;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {}
}
