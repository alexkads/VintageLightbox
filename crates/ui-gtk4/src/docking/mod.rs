//! Docking System for VintageLightbox
//!
//! Custom docking system using GTK4 Paned and Notebook widgets.
//! Provides flexible panel layouts for Library and Develop views.

pub mod dock_manager;

pub use dock_manager::{DockManager, PanelPosition};
