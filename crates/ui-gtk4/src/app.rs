//! Main Application Component (relm4)
//!
//! This is the root component of the application following the Elm Architecture.
//! It manages the top-level view and coordinates all child components.

use gtk4::prelude::*;
use relm4::prelude::*;
use relm4::ComponentParts;

use crate::model::{AppInit, AppModel, CurrentView};
use crate::messages::{AppMsg, CommandOutput};
use crate::views::library_view::{LibraryView, LibraryViewMsg};
use crate::views::develop_view::{DevelopView, DevelopViewMsg};
use crate::components::toolbar::Toolbar;

/// Main application component
/// In relm4, `model` in the view! macro refers to `&Self`, so we name our fields accordingly
#[allow(dead_code)]
pub struct VintageLightboxApp {
    /// Application state
    state: AppModel,
    /// Toolbar component
    toolbar: Controller<Toolbar>,
    /// Library view component
    library_view: Controller<LibraryView>,
    /// Develop view component  
    develop_view: Controller<DevelopView>,
}

#[relm4::component(pub)]
impl Component for VintageLightboxApp {
    type Init = AppInit;
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = CommandOutput;

    view! {
        gtk4::ApplicationWindow {
            set_title: Some("VintageLightbox"),
            set_default_width: 1400,
            set_default_height: 900,
            
            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,
                
                // Toolbar at the top
                #[local_ref]
                toolbar_widget -> gtk4::Box {},
                
                // Main content area
                gtk4::Stack {
                    set_vexpand: true,
                    set_hexpand: true,
                    set_transition_type: gtk4::StackTransitionType::SlideLeftRight,
                    
                    #[name = "library_page"]
                    add_child = &gtk4::Box {
                        set_orientation: gtk4::Orientation::Vertical,
                        #[local_ref]
                        library_widget -> gtk4::Box {},
                    } -> {
                        set_name: "library_page",
                        set_title: "Library",
                    },
                    
                    #[name = "develop_page"]
                    add_child = &gtk4::Box {
                        set_orientation: gtk4::Orientation::Vertical,
                        #[local_ref]
                        develop_widget -> gtk4::Box {},
                    } -> {
                        set_name: "develop_page",
                        set_title: "Develop",
                    },
                    
                    #[watch]
                    set_visible_child_name: match model.state.current_view {
                        CurrentView::Library => "library_page",
                        CurrentView::Develop => "develop_page",
                    },
                },
                
                // Status bar at the bottom
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Horizontal,
                    add_css_class: "statusbar",
                    set_margin_all: 4,
                    
                    gtk4::Label {
                        #[watch]
                        set_label: &format!(
                            "{} photos | {} selected",
                            model.state.photos.len(),
                            model.state.selection_count()
                        ),
                    },
                    
                    // Loading indicator
                    #[name = "loading_spinner"]
                    gtk4::Spinner {
                        set_margin_start: 8,
                        #[watch]
                        set_spinning: model.state.is_loading,
                        #[watch]
                        set_visible: model.state.is_loading,
                    },
                    
                    // Loading message
                    gtk4::Label {
                        set_margin_start: 4,
                        #[watch]
                        set_label: model.state.loading_message.as_deref().unwrap_or(""),
                        #[watch]
                        set_visible: model.state.loading_message.is_some(),
                    },
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Extract what we need from init before moving it
        let preview_manager = init.preview_manager.clone();

        // Create the state
        let state = AppModel::new(init);

        // Create child components
        let toolbar = Toolbar::builder()
            .launch(())
            .forward(sender.input_sender(), |msg| {
                use crate::components::toolbar::ToolbarOutput;
                match msg {
                    ToolbarOutput::Import => AppMsg::OpenImportDialog,
                    ToolbarOutput::Export => AppMsg::OpenExportDialog,
                    ToolbarOutput::SwitchToLibrary => AppMsg::SwitchToLibrary,
                    ToolbarOutput::SwitchToDevelop => AppMsg::SwitchToDevelop,
                    ToolbarOutput::OpenSettings => AppMsg::OpenSettings,
                }
            });

        let library_view = LibraryView::builder()
            .launch(preview_manager)
            .forward(sender.input_sender(), |msg| {
                use crate::views::library_view::LibraryViewOutput;
                match msg {
                    LibraryViewOutput::SelectPhoto(id) => AppMsg::SelectPhoto(id),
                    LibraryViewOutput::OpenPhoto(id) => AppMsg::OpenPhotoInDevelop(id),
                }
            });
        
        let develop_view = DevelopView::builder()
            .launch(())
            .forward(sender.input_sender(), |msg| {
                use crate::views::develop_view::DevelopViewOutput;
                match msg {
                    DevelopViewOutput::ExposureChanged(v) => AppMsg::SetExposure(v),
                    DevelopViewOutput::ContrastChanged(v) => AppMsg::SetContrast(v),
                    DevelopViewOutput::TemperatureChanged(v) => AppMsg::SetTemperature(v),
                    DevelopViewOutput::TintChanged(v) => AppMsg::SetTint(v),
                    DevelopViewOutput::HighlightsChanged(v) => AppMsg::SetHighlights(v),
                    DevelopViewOutput::ShadowsChanged(v) => AppMsg::SetShadows(v),
                    DevelopViewOutput::WhitesChanged(v) => AppMsg::SetWhites(v),
                    DevelopViewOutput::BlacksChanged(v) => AppMsg::SetBlacks(v),
                    DevelopViewOutput::ClarityChanged(v) => AppMsg::SetClarity(v),
                    DevelopViewOutput::VibranceChanged(v) => AppMsg::SetVibrance(v),
                    DevelopViewOutput::SaturationChanged(v) => AppMsg::SetSaturation(v),
                    DevelopViewOutput::NavigateNext => AppMsg::NavigateNext,
                    DevelopViewOutput::NavigatePrevious => AppMsg::NavigatePrevious,
                    DevelopViewOutput::ResetAdjustments => AppMsg::ResetAdjustments,
                }
            });
        
        // Get widget references for the view
        let toolbar_widget = toolbar.widget().clone();
        let library_widget = library_view.widget().clone();
        let develop_widget = develop_view.widget().clone();
        
        let model = VintageLightboxApp {
            state,
            toolbar,
            library_view,
            develop_view,
        };
        
        let widgets = view_output!();
        
        // Setup keyboard shortcuts
        setup_shortcuts(&root, &sender);
        
        // Load initial data
        sender.input(AppMsg::LoadPhotos);
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            // Navigation
            AppMsg::SwitchToLibrary => {
                self.state.current_view = CurrentView::Library;
            }
            AppMsg::SwitchToDevelop => {
                self.state.current_view = CurrentView::Develop;
            }
            
            // Loading
            AppMsg::LoadPhotos => {
                self.state.is_loading = true;
                self.state.loading_message = Some("Loading photos...".to_string());
                
                let library_controller = self.state.library_controller.clone();
                sender.oneshot_command(async move {
                    let result = library_controller.get_all_photos().await;
                    CommandOutput::PhotosLoaded(result.map_err(|e| e.to_string()))
                });
            }
            AppMsg::PhotosLoaded(photos) => {
                self.state.photos = photos;
                self.state.is_loading = false;
                self.state.loading_message = None;
            }
            
            AppMsg::LoadPresets => {
                // Presets loading - will be implemented when PresetController API is ready
                self.state.presets = Vec::new();
            }
            AppMsg::PresetsLoaded(presets) => {
                self.state.presets = presets;
            }
            
            // Selection
            AppMsg::SelectPhoto(id) => {
                self.state.selected_photo_id = Some(id.clone());
                self.state.selected_photo_ids.clear();
                self.state.selected_photo_ids.insert(id);
            }
            AppMsg::TogglePhotoSelection(id) => {
                if self.state.selected_photo_ids.contains(&id) {
                    self.state.selected_photo_ids.remove(&id);
                } else {
                    self.state.selected_photo_ids.insert(id);
                }
            }
            AppMsg::SelectAll => {
                self.state.select_all();
            }
            AppMsg::ClearSelection => {
                self.state.clear_selection();
            }
            AppMsg::OpenPhotoInDevelop(id) => {
                self.state.develop_photo_id = Some(id);
                self.state.current_view = CurrentView::Develop;
            }
            
            // Navigation
            AppMsg::NavigateNext => {
                self.state.navigate_next();
            }
            AppMsg::NavigatePrevious => {
                self.state.navigate_previous();
            }
            
            // Editing
            AppMsg::SetExposure(v) => {
                self.state.push_to_history();
                self.state.exposure = v;
            }
            AppMsg::SetContrast(v) => {
                self.state.push_to_history();
                self.state.contrast = v;
            }
            AppMsg::SetTemperature(v) => {
                self.state.push_to_history();
                self.state.temperature = v;
            }
            AppMsg::SetTint(v) => {
                self.state.push_to_history();
                self.state.tint = v;
            }
            AppMsg::SetHighlights(v) => {
                self.state.push_to_history();
                self.state.highlights = v;
            }
            AppMsg::SetShadows(v) => {
                self.state.push_to_history();
                self.state.shadows = v;
            }
            AppMsg::SetWhites(v) => {
                self.state.push_to_history();
                self.state.whites = v;
            }
            AppMsg::SetBlacks(v) => {
                self.state.push_to_history();
                self.state.blacks = v;
            }
            AppMsg::SetClarity(v) => {
                self.state.push_to_history();
                self.state.clarity = v;
            }
            AppMsg::SetVibrance(v) => {
                self.state.push_to_history();
                self.state.vibrance = v;
            }
            AppMsg::SetSaturation(v) => {
                self.state.push_to_history();
                self.state.saturation = v;
            }
            AppMsg::ResetAdjustments => {
                self.state.reset_adjustments();
            }
            AppMsg::Undo => {
                self.state.undo();
            }
            AppMsg::Redo => {
                self.state.redo();
            }
            AppMsg::ToggleBeforeAfter => {
                self.state.show_before = !self.state.show_before;
            }
            
            // Rating & Labels - use PhotoController
            AppMsg::SetRating(rating) => {
                if let Some(ref photo_id) = self.state.selected_photo_id.clone() {
                    let photo_controller = self.state.photo_controller.clone();
                    let id = photo_id.clone();
                    sender.oneshot_command(async move {
                        let _ = photo_controller.rate_photo(&id, rating).await;
                        CommandOutput::EditsSaved(Ok(()))
                    });
                }
            }
            AppMsg::SetColorLabel(label) => {
                if let Some(ref photo_id) = self.state.selected_photo_id.clone() {
                    let photo_controller = self.state.photo_controller.clone();
                    let id = photo_id.clone();
                    let label_str = label.map(|l| l.to_string()).unwrap_or_default();
                    sender.oneshot_command(async move {
                        let _ = photo_controller.set_color_label(&id, &label_str).await;
                        CommandOutput::EditsSaved(Ok(()))
                    });
                }
            }
            AppMsg::SetFlag(flag) => {
                if let Some(ref photo_id) = self.state.selected_photo_id.clone() {
                    let photo_controller = self.state.photo_controller.clone();
                    let id = photo_id.clone();
                    let flag_code = flag.unwrap_or(0);
                    sender.oneshot_command(async move {
                        let _ = photo_controller.set_flag(&id, flag_code).await;
                        CommandOutput::EditsSaved(Ok(()))
                    });
                }
            }
            
            // Filtering
            AppMsg::FilterByRating(rating) => {
                self.state.filter.min_rating = rating;
            }
            AppMsg::FilterByColorLabel(label) => {
                self.state.filter.color_label = label;
            }
            AppMsg::FilterByFlag(flag) => {
                self.state.filter.flag = flag;
            }
            AppMsg::FilterByFolder(folder) => {
                self.state.filter.folder_path = folder;
            }
            AppMsg::ClearFilters => {
                self.state.filter = Default::default();
            }
            
            // Dialogs
            AppMsg::OpenImportDialog => {
                self.state.show_import_dialog = true;
            }
            AppMsg::OpenExportDialog => {
                self.state.show_export_dialog = true;
            }
            AppMsg::OpenSettings => {
                self.state.show_settings_dialog = true;
            }
            AppMsg::CloseDialog => {
                self.state.show_import_dialog = false;
                self.state.show_export_dialog = false;
                self.state.show_settings_dialog = false;
            }
            
            // Toast notifications
            AppMsg::ShowToast { message, is_error } => {
                self.state.toast_message = Some(message);
                self.state.toast_is_error = is_error;
            }
            AppMsg::Error(error) => {
                self.state.toast_message = Some(error);
                self.state.toast_is_error = true;
            }
            
            // Handle remaining messages
            _ => {}
        }
    }

    fn update_cmd(
        &mut self,
        cmd: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match cmd {
            CommandOutput::PhotosLoaded(result) => {
                match result {
                    Ok(photos) => {
                        // Store photos in state
                        self.state.photos = photos.clone();
                        // Forward to library view
                        self.library_view.emit(LibraryViewMsg::SetPhotos(photos.clone()));
                        // Forward to develop view for filmstrip
                        self.develop_view.emit(DevelopViewMsg::SetPhotos(photos));
                    }
                    Err(e) => {
                        self.state.toast_message = Some(format!("Failed to load photos: {}", e));
                        self.state.toast_is_error = true;
                    }
                }
                self.state.is_loading = false;
                self.state.loading_message = None;
            }
            CommandOutput::PresetsLoaded(result) => {
                if let Ok(presets) = result {
                    self.state.presets = presets;
                }
            }
            CommandOutput::ImportComplete(result) => {
                self.state.is_loading = false;
                match result {
                    Ok(photos) => {
                        self.state.photos.extend(photos);
                        self.state.toast_message = Some("Import completed!".to_string());
                        self.state.toast_is_error = false;
                    }
                    Err(e) => {
                        self.state.toast_message = Some(format!("Import failed: {}", e));
                        self.state.toast_is_error = true;
                    }
                }
            }
            CommandOutput::EditsSaved(result) => {
                if let Err(e) = result {
                    self.state.toast_message = Some(format!("Failed to save: {}", e));
                    self.state.toast_is_error = true;
                }
            }
            CommandOutput::ExportComplete(result) => {
                match result {
                    Ok(()) => {
                        self.state.toast_message = Some("Export completed!".to_string());
                        self.state.toast_is_error = false;
                    }
                    Err(e) => {
                        self.state.toast_message = Some(format!("Export failed: {}", e));
                        self.state.toast_is_error = true;
                    }
                }
            }
            CommandOutput::ThumbnailReady { photo_id, data } => {
                // Update thumbnail in library view
                let _ = (photo_id, data);
            }
            CommandOutput::PreviewReady { photo_id, data } => {
                // Update preview in develop view
                let _ = (photo_id, data);
            }
        }
    }
}

/// Setup keyboard shortcuts
fn setup_shortcuts(window: &gtk4::ApplicationWindow, sender: &ComponentSender<VintageLightboxApp>) {
    let controller = gtk4::EventControllerKey::new();
    
    let sender_clone = sender.clone();
    controller.connect_key_pressed(move |_, key, _, modifier| {
        use gtk4::gdk::Key;
        
        let cmd = modifier.contains(gtk4::gdk::ModifierType::META_MASK) 
            || modifier.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
        let shift = modifier.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
        
        match key {
            // Rating shortcuts (1-5)
            Key::_1 => { sender_clone.input(AppMsg::SetRating(1)); }
            Key::_2 => { sender_clone.input(AppMsg::SetRating(2)); }
            Key::_3 => { sender_clone.input(AppMsg::SetRating(3)); }
            Key::_4 => { sender_clone.input(AppMsg::SetRating(4)); }
            Key::_5 => { sender_clone.input(AppMsg::SetRating(5)); }
            Key::_0 => { sender_clone.input(AppMsg::SetRating(0)); }
            
            // Navigation
            Key::Left => { sender_clone.input(AppMsg::NavigatePrevious); }
            Key::Right => { sender_clone.input(AppMsg::NavigateNext); }
            
            // View switching
            Key::g if cmd => { sender_clone.input(AppMsg::SwitchToLibrary); }
            Key::d if cmd => { sender_clone.input(AppMsg::SwitchToDevelop); }
            
            // Undo/Redo
            Key::z if cmd && shift => { sender_clone.input(AppMsg::Redo); }
            Key::z if cmd => { sender_clone.input(AppMsg::Undo); }
            
            // Selection
            Key::a if cmd => { sender_clone.input(AppMsg::SelectAll); }
            Key::Escape => { sender_clone.input(AppMsg::ClearSelection); }
            
            // Flags
            Key::p => { sender_clone.input(AppMsg::SetFlag(Some(1))); } // Pick
            Key::x => { sender_clone.input(AppMsg::SetFlag(Some(-1))); } // Reject
            Key::u => { sender_clone.input(AppMsg::SetFlag(None)); } // Unflag
            
            // Before/After
            Key::backslash => { sender_clone.input(AppMsg::ToggleBeforeAfter); }
            
            _ => return gtk4::glib::Propagation::Proceed,
        }
        
        gtk4::glib::Propagation::Stop
    });
    
    window.add_controller(controller);
}
