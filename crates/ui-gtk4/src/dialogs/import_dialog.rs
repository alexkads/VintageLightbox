//! Import Dialog
//!
//! Dialog for importing photos with preview and options.

use gtk4::prelude::*;
use relm4::prelude::*;

/// Import dialog component
pub struct ImportDialog {
    is_open: bool,
    selected_files: Vec<std::path::PathBuf>,
}

/// Messages for the import dialog
#[derive(Debug)]
pub enum ImportDialogMsg {
    Open,
    Close,
    SelectFiles,
    FilesSelected(Vec<std::path::PathBuf>),
    StartImport,
}

/// Output messages from the import dialog
#[derive(Debug)]
pub enum ImportDialogOutput {
    ImportFiles(Vec<std::path::PathBuf>),
    Closed,
}

#[relm4::component(pub)]
impl SimpleComponent for ImportDialog {
    type Init = ();
    type Input = ImportDialogMsg;
    type Output = ImportDialogOutput;

    view! {
        gtk4::Window {
            set_title: Some("Import Photos"),
            set_default_width: 600,
            set_default_height: 400,
            set_modal: true,
            #[watch]
            set_visible: model.is_open,
            
            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                set_margin_all: 16,
                set_spacing: 16,
                
                // Header
                gtk4::Label {
                    set_label: "Select photos to import",
                    add_css_class: "title-2",
                    set_halign: gtk4::Align::Start,
                },
                
                // File selection area
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Vertical,
                    set_vexpand: true,
                    add_css_class: "card",
                    set_margin_all: 8,
                    
                    gtk4::Label {
                        #[watch]
                        set_label: &format!("{} files selected", model.selected_files.len()),
                        set_halign: gtk4::Align::Center,
                        set_valign: gtk4::Align::Center,
                        set_vexpand: true,
                    },
                    
                    gtk4::Button {
                        set_label: "Choose Files...",
                        set_halign: gtk4::Align::Center,
                        connect_clicked => ImportDialogMsg::SelectFiles,
                    },
                },
                
                // Action buttons
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    set_halign: gtk4::Align::End,
                    set_spacing: 8,
                    
                    gtk4::Button {
                        set_label: "Cancel",
                        connect_clicked => ImportDialogMsg::Close,
                    },
                    
                    gtk4::Button {
                        set_label: "Import",
                        add_css_class: "suggested-action",
                        #[watch]
                        set_sensitive: !model.selected_files.is_empty(),
                        connect_clicked => ImportDialogMsg::StartImport,
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
        let model = ImportDialog {
            is_open: false,
            selected_files: Vec::new(),
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ImportDialogMsg::Open => {
                self.is_open = true;
            }
            ImportDialogMsg::Close => {
                self.is_open = false;
                self.selected_files.clear();
                let _ = sender.output(ImportDialogOutput::Closed);
            }
            ImportDialogMsg::SelectFiles => {
                // Would open native file chooser here
            }
            ImportDialogMsg::FilesSelected(files) => {
                self.selected_files = files;
            }
            ImportDialogMsg::StartImport => {
                let files = std::mem::take(&mut self.selected_files);
                self.is_open = false;
                let _ = sender.output(ImportDialogOutput::ImportFiles(files));
            }
        }
    }
}
