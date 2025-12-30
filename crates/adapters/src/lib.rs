//! # Adapters Layer - VintageLightbox
//!
//! Controllers, Presenters, View Models, State e Services
//!
//! Esta camada adapta dados entre Use Cases e a camada de UI,
//! mantendo a lógica de aplicação independente do framework de UI.

pub mod controllers;
pub mod presenters;
pub mod view_models;
pub mod state;
pub mod services;

pub use view_models::PhotoViewModel;
pub use state::{ApplicationState, EditingSession, EditHistory, PhotoFilters};
pub use services::NavigationService;
