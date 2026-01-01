pub mod system_gateway;
pub mod editing_orchestrator;

// Re-exports
pub use editing_orchestrator::{
    EditingOrchestrator,
    EditObserver,
    EditEvent,
    EditResult,
};
