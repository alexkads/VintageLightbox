//! Library View with DockManager
//!
//! Complete library view using the DockManager system with proper component integration.

use gtk4::prelude::*;
use relm4::prelude::*;
use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use infrastructure::cache::preview_manager::PreviewManager;

use crate::docking::{DockManager, PanelPosition};
use crate::components::photo_grid::{PhotoGrid, PhotoGridMsg, PhotoGridOutput};
use crate::components::folder_tree::{FolderTree, FolderTreeOutput};
use crate::components::filter_panel::FilterPanel;
use crate::components::metadata_panel::MetadataPanel;
use crate::components::filmstrip::{Filmstrip, FilmstripMsg};

#[allow(dead_code)]
pub struct LibraryView {
    dock_manager: DockManager,
    photo_grid: Controller<PhotoGrid>,
    folder_tree: Controller<FolderTree>,
    filter_panel: Controller<FilterPanel>,
    metadata_panel: Controller<MetadataPanel>,
    filmstrip: Controller<Filmstrip>,
}

#[derive(Debug)]
pub enum LibraryViewMsg {
    SetPhotos(Vec<PhotoViewModel>),
}

#[derive(Debug)]
pub enum LibraryViewOutput {
    SelectPhoto(String),
    OpenPhoto(String),
}

#[relm4::component(pub)]
impl SimpleComponent for LibraryView {
    type Init = Arc<PreviewManager>;
    type Input = LibraryViewMsg;
    type Output = LibraryViewOutput;

    view! {
        #[root]
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            set_spacing: 0,
            set_hexpand: true,
            set_vexpand: true,

            // Header
            gtk4::Box {
                set_orientation: gtk4::Orientation::Horizontal,
                set_margin_all: 12,
                set_spacing: 8,

                gtk4::Label {
                    set_markup: "<span size='large' weight='bold'>Library</span>",
                    set_hexpand: true,
                    set_halign: gtk4::Align::Start,
                },

                gtk4::Button {
                    set_label: "Import",
                    set_tooltip_text: Some("Import photos"),
                },
            },

            // DockManager root
            #[local_ref]
            dock_root -> gtk4::Box {},
        }
    }

    fn init(
        preview_manager: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Initialize child components
        let photo_grid = PhotoGrid::builder()
            .launch(preview_manager.clone())
            .forward(sender.input_sender(), |output| match output {
                PhotoGridOutput::PhotoSelected(_id) => LibraryViewMsg::SetPhotos(vec![]), // Placeholder
                PhotoGridOutput::PhotoActivated(_id) => LibraryViewMsg::SetPhotos(vec![]), // Placeholder
            });

        let folder_tree = FolderTree::builder()
            .launch(())
            .forward(sender.input_sender(), |output| match output {
                FolderTreeOutput::FolderSelected(_) => LibraryViewMsg::SetPhotos(vec![]),
            });

        let filter_panel = FilterPanel::builder()
            .launch(())
            .forward(sender.input_sender(), |_output| {
                LibraryViewMsg::SetPhotos(vec![])
            });

        let metadata_panel = MetadataPanel::builder()
            .launch(())
            .detach();

        let filmstrip = Filmstrip::builder()
            .launch(())
            .detach();

        // Create DockManager with Library layout
        let mut dock_manager = DockManager::new_library();

        // Add panels to DockManager
        dock_manager.add_panel(
            PanelPosition::Center,
            photo_grid.widget().upcast_ref::<gtk4::Widget>(),
            None,
        );

        dock_manager.add_panel(
            PanelPosition::Left,
            folder_tree.widget().upcast_ref::<gtk4::Widget>(),
            Some("Folders"),
        );

        dock_manager.add_panel(
            PanelPosition::Left,
            filter_panel.widget().upcast_ref::<gtk4::Widget>(),
            Some("Filters"),
        );

        dock_manager.add_panel(
            PanelPosition::Right,
            metadata_panel.widget().upcast_ref::<gtk4::Widget>(),
            Some("Metadata"),
        );

        dock_manager.add_panel(
            PanelPosition::Bottom,
            filmstrip.widget().upcast_ref::<gtk4::Widget>(),
            None,
        );

        let model = LibraryView {
            dock_manager,
            photo_grid,
            folder_tree,
            filter_panel,
            metadata_panel,
            filmstrip,
        };

        let dock_root = model.dock_manager.get_root();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            LibraryViewMsg::SetPhotos(photos) => {
                println!("LibraryView: Setting {} photos", photos.len());

                // Forward to PhotoGrid
                self.photo_grid.emit(PhotoGridMsg::SetPhotos(photos.clone()));

                // Forward to Filmstrip
                self.filmstrip.emit(FilmstripMsg::SetPhotos(photos));
            }
        }
    }
}
