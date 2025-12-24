//! Settings Dialog
//!
//! Application settings and cache management.

use gtk4::prelude::*;
use relm4::prelude::*;

/// Settings dialog component
pub struct SettingsDialog {
    is_open: bool,
    cache_stats: CacheStats,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub thumbnail_count: usize,
    pub preview_count: usize,
    pub total_size_mb: f64,
    pub cache_path: String,
}

/// Messages for the settings dialog
#[derive(Debug)]
pub enum SettingsDialogMsg {
    Open,
    Close,
    ClearThumbnails,
    ClearPreviews,
    ClearAllCache,
    UpdateStats(CacheStats),
}

/// Output messages from the settings dialog
#[derive(Debug)]
pub enum SettingsDialogOutput {
    ClearThumbnails,
    ClearPreviews,
    ClearAllCache,
    Closed,
}

#[relm4::component(pub)]
impl SimpleComponent for SettingsDialog {
    type Init = ();
    type Input = SettingsDialogMsg;
    type Output = SettingsDialogOutput;

    view! {
        gtk4::Window {
            set_title: Some("Settings"),
            set_default_width: 500,
            set_default_height: 400,
            set_modal: true,
            #[watch]
            set_visible: model.is_open,
            
            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                set_margin_all: 16,
                set_spacing: 16,
                
                gtk4::Label {
                    set_label: "Settings",
                    add_css_class: "title-1",
                    set_halign: gtk4::Align::Start,
                },
                
                // Cache section
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Vertical,
                    add_css_class: "card",
                    set_margin_all: 8,
                    set_spacing: 12,
                    
                    gtk4::Label {
                        set_label: "Cache Management",
                        add_css_class: "heading",
                        set_halign: gtk4::Align::Start,
                    },
                    
                    // Stats
                    gtk4::Grid {
                        set_row_spacing: 4,
                        set_column_spacing: 16,
                        
                        attach[0, 0, 1, 1] = &gtk4::Label {
                            set_label: "Thumbnails:",
                            set_halign: gtk4::Align::Start,
                            add_css_class: "dim-label",
                        },
                        attach[1, 0, 1, 1] = &gtk4::Label {
                            #[watch]
                            set_label: &format!("{}", model.cache_stats.thumbnail_count),
                            set_halign: gtk4::Align::Start,
                        },
                        
                        attach[0, 1, 1, 1] = &gtk4::Label {
                            set_label: "Previews:",
                            set_halign: gtk4::Align::Start,
                            add_css_class: "dim-label",
                        },
                        attach[1, 1, 1, 1] = &gtk4::Label {
                            #[watch]
                            set_label: &format!("{}", model.cache_stats.preview_count),
                            set_halign: gtk4::Align::Start,
                        },
                        
                        attach[0, 2, 1, 1] = &gtk4::Label {
                            set_label: "Total Size:",
                            set_halign: gtk4::Align::Start,
                            add_css_class: "dim-label",
                        },
                        attach[1, 2, 1, 1] = &gtk4::Label {
                            #[watch]
                            set_label: &format!("{:.1} MB", model.cache_stats.total_size_mb),
                            set_halign: gtk4::Align::Start,
                        },
                        
                        attach[0, 3, 1, 1] = &gtk4::Label {
                            set_label: "Location:",
                            set_halign: gtk4::Align::Start,
                            add_css_class: "dim-label",
                        },
                        attach[1, 3, 1, 1] = &gtk4::Label {
                            #[watch]
                            set_label: &model.cache_stats.cache_path,
                            set_halign: gtk4::Align::Start,
                            set_ellipsize: gtk4::pango::EllipsizeMode::Middle,
                        },
                    },
                    
                    // Clear buttons
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 8,
                        set_halign: gtk4::Align::Start,
                        
                        gtk4::Button {
                            set_label: "Clear Thumbnails",
                            connect_clicked => SettingsDialogMsg::ClearThumbnails,
                        },
                        
                        gtk4::Button {
                            set_label: "Clear Previews",
                            connect_clicked => SettingsDialogMsg::ClearPreviews,
                        },
                        
                        gtk4::Button {
                            set_label: "Clear All",
                            add_css_class: "destructive-action",
                            connect_clicked => SettingsDialogMsg::ClearAllCache,
                        },
                    },
                },
                
                // Spacer
                gtk4::Box {
                    set_vexpand: true,
                },
                
                // Close button
                gtk4::Box {
                    set_halign: gtk4::Align::End,
                    
                    gtk4::Button {
                        set_label: "Close",
                        connect_clicked => SettingsDialogMsg::Close,
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
        let model = SettingsDialog {
            is_open: false,
            cache_stats: CacheStats::default(),
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SettingsDialogMsg::Open => {
                self.is_open = true;
            }
            SettingsDialogMsg::Close => {
                self.is_open = false;
                let _ = sender.output(SettingsDialogOutput::Closed);
            }
            SettingsDialogMsg::ClearThumbnails => {
                let _ = sender.output(SettingsDialogOutput::ClearThumbnails);
            }
            SettingsDialogMsg::ClearPreviews => {
                let _ = sender.output(SettingsDialogOutput::ClearPreviews);
            }
            SettingsDialogMsg::ClearAllCache => {
                let _ = sender.output(SettingsDialogOutput::ClearAllCache);
            }
            SettingsDialogMsg::UpdateStats(stats) => {
                self.cache_stats = stats;
            }
        }
    }
}
