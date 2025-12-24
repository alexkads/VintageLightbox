//! Toolbar Component
//!
//! Top toolbar with import, export, view switching, and settings buttons.

use gtk4::prelude::*;
use relm4::prelude::*;

use crate::model::CurrentView;

/// Toolbar component
pub struct Toolbar {
    current_view: CurrentView,
}

/// Messages that can be sent to the toolbar
#[derive(Debug)]
pub enum ToolbarMsg {
    Import,
    Export,
    SwitchView(CurrentView),
    OpenSettings,
}

/// Messages that the toolbar sends to its parent
#[derive(Debug)]
pub enum ToolbarOutput {
    Import,
    Export,
    SwitchToLibrary,
    SwitchToDevelop,
    OpenSettings,
}

#[relm4::component(pub)]
impl SimpleComponent for Toolbar {
    type Init = ();
    type Input = ToolbarMsg;
    type Output = ToolbarOutput;

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Horizontal,
            set_spacing: 8,
            set_margin_all: 8,
            add_css_class: "toolbar",
            
            // Import button
            gtk4::Button {
                set_icon_name: "folder-open-symbolic",
                set_tooltip_text: Some("Import Photos (Cmd+I)"),
                add_css_class: "flat",
                connect_clicked => ToolbarMsg::Import,
            },
            
            // Separator
            gtk4::Separator {
                set_orientation: gtk4::Orientation::Vertical,
                set_margin_start: 4,
                set_margin_end: 4,
            },
            
            // View toggle buttons
            gtk4::ToggleButton {
                set_label: "Library",
                set_tooltip_text: Some("Library View (Cmd+G)"),
                #[watch]
                set_active: model.current_view == CurrentView::Library,
                connect_toggled[sender] => move |btn| {
                    if btn.is_active() {
                        sender.input(ToolbarMsg::SwitchView(CurrentView::Library));
                    }
                },
            },
            
            gtk4::ToggleButton {
                set_label: "Develop",
                set_tooltip_text: Some("Develop View (Cmd+D)"),
                #[watch]
                set_active: model.current_view == CurrentView::Develop,
                connect_toggled[sender] => move |btn| {
                    if btn.is_active() {
                        sender.input(ToolbarMsg::SwitchView(CurrentView::Develop));
                    }
                },
            },
            
            // Spacer
            gtk4::Box {
                set_hexpand: true,
            },
            
            // Export button
            gtk4::Button {
                set_icon_name: "document-save-as-symbolic",
                set_tooltip_text: Some("Export Photo"),
                add_css_class: "flat",
                connect_clicked => ToolbarMsg::Export,
            },
            
            // Settings button
            gtk4::Button {
                set_icon_name: "emblem-system-symbolic",
                set_tooltip_text: Some("Settings"),
                add_css_class: "flat",
                connect_clicked => ToolbarMsg::OpenSettings,
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Toolbar {
            current_view: CurrentView::Library,
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ToolbarMsg::Import => {
                let _ = sender.output(ToolbarOutput::Import);
            }
            ToolbarMsg::Export => {
                let _ = sender.output(ToolbarOutput::Export);
            }
            ToolbarMsg::SwitchView(view) => {
                self.current_view = view;
                match view {
                    CurrentView::Library => {
                        let _ = sender.output(ToolbarOutput::SwitchToLibrary);
                    }
                    CurrentView::Develop => {
                        let _ = sender.output(ToolbarOutput::SwitchToDevelop);
                    }
                }
            }
            ToolbarMsg::OpenSettings => {
                let _ = sender.output(ToolbarOutput::OpenSettings);
            }
        }
    }
}
