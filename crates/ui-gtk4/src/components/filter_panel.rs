//! Filter Panel Component (Placeholder)
//!
//! Panel for filtering photos by rating, color labels, flags, etc.

use gtk4::prelude::*;
use relm4::prelude::*;

pub struct FilterPanel;

#[derive(Debug)]
pub enum FilterPanelMsg {
    Reset,
}

#[derive(Debug)]
pub enum FilterPanelOutput {
    FilterChanged,
}

#[relm4::component(pub)]
impl SimpleComponent for FilterPanel {
    type Init = ();
    type Input = FilterPanelMsg;
    type Output = FilterPanelOutput;

    view! {
        gtk4::ScrolledWindow {
            set_hexpand: true,
            set_vexpand: true,
            set_policy: (gtk4::PolicyType::Never, gtk4::PolicyType::Automatic),

            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                set_margin_all: 12,
                set_spacing: 12,

                gtk4::Label {
                    set_label: "Filters",
                    set_halign: gtk4::Align::Start,
                    add_css_class: "heading",
                },

                // Rating filter
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Vertical,
                    set_spacing: 4,

                    gtk4::Label {
                        set_label: "Rating",
                        set_halign: gtk4::Align::Start,
                        add_css_class: "caption",
                    },

                    gtk4::Label {
                        set_label: "★★★★★",
                        set_halign: gtk4::Align::Start,
                    },
                },

                // Color labels
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Vertical,
                    set_spacing: 4,

                    gtk4::Label {
                        set_label: "Color Labels",
                        set_halign: gtk4::Align::Start,
                        add_css_class: "caption",
                    },

                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 4,

                        gtk4::Button {
                            set_label: "🔴",
                        },
                        gtk4::Button {
                            set_label: "🟡",
                        },
                        gtk4::Button {
                            set_label: "🟢",
                        },
                        gtk4::Button {
                            set_label: "🔵",
                        },
                        gtk4::Button {
                            set_label: "🟣",
                        },
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = FilterPanel;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {
        // Placeholder - no logic yet
    }
}
