//! Folder Tree Component (Placeholder)
//!
//! Tree view for navigating photo folders.

use gtk4::prelude::*;
use relm4::prelude::*;

pub struct FolderTree;

#[derive(Debug)]
pub enum FolderTreeMsg {
    Refresh,
}

#[derive(Debug)]
pub enum FolderTreeOutput {
    FolderSelected(String),
}

#[relm4::component(pub)]
impl SimpleComponent for FolderTree {
    type Init = ();
    type Input = FolderTreeMsg;
    type Output = FolderTreeOutput;

    view! {
        gtk4::ScrolledWindow {
            set_hexpand: true,
            set_vexpand: true,
            set_policy: (gtk4::PolicyType::Automatic, gtk4::PolicyType::Automatic),

            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                set_margin_all: 8,

                gtk4::Label {
                    set_label: "Folder Tree",
                    set_halign: gtk4::Align::Start,
                    add_css_class: "caption",
                },

                gtk4::Label {
                    set_label: "📁 All Photos\n📁 2024\n📁 2023",
                    set_halign: gtk4::Align::Start,
                    set_margin_top: 8,
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = FolderTree;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {
        // Placeholder - no logic yet
    }
}
