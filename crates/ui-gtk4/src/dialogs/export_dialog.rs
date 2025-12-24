//! Export Dialog
//!
//! Dialog for exporting photos with format and quality options.

use gtk4::prelude::*;
use relm4::prelude::*;

/// Export dialog component
pub struct ExportDialog {
    is_open: bool,
    output_path: Option<std::path::PathBuf>,
    quality: i32,
}

/// Messages for the export dialog
#[derive(Debug)]
pub enum ExportDialogMsg {
    Open,
    Close,
    SelectOutputPath,
    SetQuality(i32),
    StartExport,
}

/// Output messages from the export dialog
#[derive(Debug)]
pub enum ExportDialogOutput {
    Export { path: std::path::PathBuf, quality: i32 },
    Closed,
}

#[relm4::component(pub)]
impl SimpleComponent for ExportDialog {
    type Init = ();
    type Input = ExportDialogMsg;
    type Output = ExportDialogOutput;

    view! {
        gtk4::Window {
            set_title: Some("Export Photo"),
            set_default_width: 400,
            set_default_height: 300,
            set_modal: true,
            #[watch]
            set_visible: model.is_open,
            
            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                set_margin_all: 16,
                set_spacing: 16,
                
                gtk4::Label {
                    set_label: "Export Settings",
                    add_css_class: "title-2",
                    set_halign: gtk4::Align::Start,
                },
                
                // Quality slider
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_spacing: 8,
                    
                    gtk4::Label {
                        set_label: "Quality:",
                    },
                    
                    gtk4::Scale {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_hexpand: true,
                        set_range: (1.0, 100.0),
                        #[watch]
                        set_value: model.quality as f64,
                        set_draw_value: true,
                    },
                },
                
                // Output path
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_spacing: 8,
                    
                    gtk4::Entry {
                        set_hexpand: true,
                        set_placeholder_text: Some("Output folder..."),
                        #[watch]
                        set_text: model.output_path.as_ref()
                            .and_then(|p| p.to_str())
                            .unwrap_or(""),
                    },
                    
                    gtk4::Button {
                        set_icon_name: "folder-open-symbolic",
                        connect_clicked => ExportDialogMsg::SelectOutputPath,
                    },
                },
                
                // Spacer
                gtk4::Box {
                    set_vexpand: true,
                },
                
                // Action buttons
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_halign: gtk4::Align::End,
                    set_spacing: 8,
                    
                    gtk4::Button {
                        set_label: "Cancel",
                        connect_clicked => ExportDialogMsg::Close,
                    },
                    
                    gtk4::Button {
                        set_label: "Export",
                        add_css_class: "suggested-action",
                        #[watch]
                        set_sensitive: model.output_path.is_some(),
                        connect_clicked => ExportDialogMsg::StartExport,
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
        let model = ExportDialog {
            is_open: false,
            output_path: None,
            quality: 90,
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ExportDialogMsg::Open => {
                self.is_open = true;
            }
            ExportDialogMsg::Close => {
                self.is_open = false;
                let _ = sender.output(ExportDialogOutput::Closed);
            }
            ExportDialogMsg::SelectOutputPath => {
                // Would open folder chooser
            }
            ExportDialogMsg::SetQuality(quality) => {
                self.quality = quality;
            }
            ExportDialogMsg::StartExport => {
                if let Some(path) = self.output_path.take() {
                    self.is_open = false;
                    let _ = sender.output(ExportDialogOutput::Export { 
                        path, 
                        quality: self.quality 
                    });
                }
            }
        }
    }
}
