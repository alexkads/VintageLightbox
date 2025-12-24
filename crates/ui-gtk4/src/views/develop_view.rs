//! Develop View
//!
//! Photo editing view with image viewer and adjustment panels via DockManager.

use gtk4::prelude::*;
use relm4::prelude::*;

use adapters::view_models::PhotoViewModel;
use crate::components::image_viewer::{ImageViewer, ImageViewerMsg};
use crate::components::histogram::Histogram;
use crate::components::adjustment_panel::{AdjustmentPanel, AdjustmentPanelMsg, AdjustmentPanelOutput, AdjustmentType};
use crate::components::preset_panel::PresetPanel;
use crate::components::history_panel::HistoryPanel;
use crate::components::filmstrip::Filmstrip;
use crate::docking::dock_manager::{DockManager, PanelPosition};

/// Develop view component
#[allow(dead_code)]
pub struct DevelopView {
    photos: Vec<PhotoViewModel>,
    
    dock_manager: DockManager,
    
    image_viewer: Controller<ImageViewer>,
    histogram: Controller<Histogram>,
    adjustment_panel: Controller<AdjustmentPanel>,
    preset_panel: Controller<PresetPanel>,
    history_panel: Controller<HistoryPanel>,
    filmstrip: Controller<Filmstrip>,
}

/// Messages for the develop view
#[derive(Debug)]
pub enum DevelopViewMsg {
    SetPhotos(Vec<PhotoViewModel>),

    // Forwarded from AdjustmentPanel
    AdjustmentChanged(AdjustmentType, f32),
    
    // Actions
    NavigateNext,
    NavigatePrevious,
    PhotoSelected(String),
    ResetAdjustments,
}

/// Output messages from the develop view
#[derive(Debug)]
pub enum DevelopViewOutput {
    NavigateNext,
    NavigatePrevious,
    ResetAdjustments,
    ExposureChanged(f32),
    ContrastChanged(f32),
    TemperatureChanged(f32),
    TintChanged(f32),
    HighlightsChanged(f32),
    ShadowsChanged(f32),
    WhitesChanged(f32),
    BlacksChanged(f32),
    ClarityChanged(f32),
    VibranceChanged(f32),
    SaturationChanged(f32),
}

#[relm4::component(pub)]
impl SimpleComponent for DevelopView {
    type Init = ();
    type Input = DevelopViewMsg;
    type Output = DevelopViewOutput;

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            set_hexpand: true,
            set_vexpand: true,
            append: model.dock_manager.get_root(),
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut dock_manager = DockManager::new_develop();

        // 1. Image Viewer (Center)
        let image_viewer = ImageViewer::builder()
            .launch(())
            .detach();
        dock_manager.add_panel(PanelPosition::Center, image_viewer.widget().upcast_ref::<gtk4::Widget>(), Some("Image Viewer"));

        // 2. Histogram (Right - Tab 1)
        let histogram = Histogram::builder()
            .launch(())
            .detach();
        dock_manager.add_panel(PanelPosition::Right, histogram.widget().upcast_ref::<gtk4::Widget>(), Some("Histogram"));

        // 3. Adjustment Panel (Right - Tab 2)
        let adjustment_panel = AdjustmentPanel::builder()
            .launch(())
            .forward(sender.input_sender(), |msg| {
                match msg {
                     AdjustmentPanelOutput::AdjustmentChanged(t, v) => DevelopViewMsg::AdjustmentChanged(t, v),
                     AdjustmentPanelOutput::AdjustmentConfirmed(t, v) => DevelopViewMsg::AdjustmentChanged(t, v),
                }
            });
        dock_manager.add_panel(PanelPosition::Right, adjustment_panel.widget().upcast_ref::<gtk4::Widget>(), Some("Adjustments"));

        // 4. Presets (Left - Tab 1)
        let preset_panel = PresetPanel::builder()
            .launch(())
            .detach();
        dock_manager.add_panel(PanelPosition::Left, preset_panel.widget().upcast_ref::<gtk4::Widget>(), Some("Presets"));

        // 5. History (Left - Tab 2)
        let history_panel = HistoryPanel::builder()
            .launch(())
            .detach();
        dock_manager.add_panel(PanelPosition::Left, history_panel.widget().upcast_ref::<gtk4::Widget>(), Some("History"));

        // 6. Filmstrip (Bottom)
        let filmstrip = Filmstrip::builder()
            .launch(())
            .detach();
        dock_manager.add_panel(PanelPosition::Bottom, filmstrip.widget().upcast_ref::<gtk4::Widget>(), Some("Filmstrip"));

        let model = DevelopView {
            photos: Vec::new(),
            dock_manager,
            image_viewer,
            histogram,
            adjustment_panel,
            preset_panel,
            history_panel,
            filmstrip,
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            DevelopViewMsg::SetPhotos(photos) => {
                self.photos = photos.clone();
                if let Some(photo) = self.photos.first() {
                     let path = photo.path.clone();
                     self.image_viewer.emit(ImageViewerMsg::LoadImage(path));
                }
            }
            DevelopViewMsg::AdjustmentChanged(adj_type, value) => {
                let output = match adj_type {
                    AdjustmentType::Exposure => DevelopViewOutput::ExposureChanged(value),
                    AdjustmentType::Contrast => DevelopViewOutput::ContrastChanged(value),
                    AdjustmentType::Temperature => DevelopViewOutput::TemperatureChanged(value),
                    AdjustmentType::Tint => DevelopViewOutput::TintChanged(value),
                    AdjustmentType::Highlights => DevelopViewOutput::HighlightsChanged(value),
                    AdjustmentType::Shadows => DevelopViewOutput::ShadowsChanged(value),
                    AdjustmentType::Whites => DevelopViewOutput::WhitesChanged(value),
                    AdjustmentType::Blacks => DevelopViewOutput::BlacksChanged(value),
                    AdjustmentType::Clarity => DevelopViewOutput::ClarityChanged(value),
                    AdjustmentType::Vibrance => DevelopViewOutput::VibranceChanged(value),
                    AdjustmentType::Saturation => DevelopViewOutput::SaturationChanged(value),
                };
                let _ = sender.output(output);
            }
            DevelopViewMsg::NavigateNext => {
                let _ = sender.output(DevelopViewOutput::NavigateNext);
            }
            DevelopViewMsg::NavigatePrevious => {
                let _ = sender.output(DevelopViewOutput::NavigatePrevious);
            }
            DevelopViewMsg::PhotoSelected(_id) => {
                // TODO: Sync to other components?
            }
            DevelopViewMsg::ResetAdjustments => {
                 self.adjustment_panel.emit(AdjustmentPanelMsg::ResetAll);
                 let _ = sender.output(DevelopViewOutput::ResetAdjustments);
            }
        }
    }
}
