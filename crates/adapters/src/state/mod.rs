pub mod application_state;
pub mod editing_session;
pub mod photo_filters;

pub use application_state::{ApplicationState, CurrentView};
pub use editing_session::EditingSession;
pub use photo_filters::PhotoFilters;

pub type EditHistory = Vec<EditingSession>; // Placeholder type
