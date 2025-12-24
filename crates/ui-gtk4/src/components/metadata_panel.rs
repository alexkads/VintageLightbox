//! Metadata Panel Component (Placeholder)
//!
//! Displays EXIF and photo metadata.

use gtk4::prelude::*;
use relm4::prelude::*;

pub struct MetadataPanel;

#[derive(Debug)]
pub enum MetadataPanelMsg {
    SetPhotoId(Option<String>),
}

#[relm4::component(pub)]
impl SimpleComponent for MetadataPanel {
    type Init = ();
    type Input = MetadataPanelMsg;
    type Output = ();

    view! {
        gtk4::ScrolledWindow {
            set_hexpand: true,
            set_vexpand: true,
            set_policy: (gtk4::PolicyType::Never, gtk4::PolicyType::Automatic),

            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                set_margin_all: 12,
                set_spacing: 8,

                gtk4::Label {
                    set_label: "Metadata",
                    set_halign: gtk4::Align::Start,
                    add_css_class: "heading",
                },

                // Camera info
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Vertical,
                    set_spacing: 2,

                    create_metadata_row("Camera", "Canon EOS R5"),
                    create_metadata_row("Lens", "RF 24-70mm f/2.8"),
                    create_metadata_row("ISO", "400"),
                    create_metadata_row("Aperture", "f/2.8"),
                    create_metadata_row("Shutter", "1/250s"),
                    create_metadata_row("Focal Length", "35mm"),
                },

                gtk4::Separator {
                    set_margin_top: 8,
                    set_margin_bottom: 8,
                },

                // File info
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Vertical,
                    set_spacing: 2,

                    create_metadata_row("Dimensions", "6000 × 4000"),
                    create_metadata_row("File Size", "25.3 MB"),
                    create_metadata_row("Format", "CR3 (RAW)"),
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = MetadataPanel;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {
        // Placeholder - no logic yet
    }
}

fn create_metadata_row(label: &str, value: &str) -> gtk4::Box {
    let row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .build();

    let label_widget = gtk4::Label::builder()
        .label(label)
        .halign(gtk4::Align::Start)
        .width_chars(12)
        .build();
    label_widget.add_css_class("caption");

    let value_widget = gtk4::Label::builder()
        .label(value)
        .halign(gtk4::Align::Start)
        .hexpand(true)
        .build();

    row.append(&label_widget);
    row.append(&value_widget);
    row
}
