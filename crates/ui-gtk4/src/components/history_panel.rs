//! History Panel Component
//!
//! Helper component to manage edit history.

use gtk4::prelude::*;
use relm4::prelude::*;

pub struct HistoryPanel;

#[derive(Debug)]
pub enum HistoryPanelMsg {}

#[relm4::component(pub)]
impl SimpleComponent for HistoryPanel {
    type Init = ();
    type Input = HistoryPanelMsg;
    type Output = ();

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            set_margin_all: 8,
            
            gtk4::Label {
                set_label: "History (Coming Soon)",
                add_css_class: "heading",
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = HistoryPanel;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {}
}
