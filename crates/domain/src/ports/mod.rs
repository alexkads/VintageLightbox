pub mod system_gateway;
pub mod editing_orchestrator;
pub mod unit_of_work;

// Re-exports
pub use editing_orchestrator::{
    EditingOrchestrator,
    EditObserver,
    EditEvent,
    EditResult,
};
pub use unit_of_work::{
    UnitOfWork,
    TransactionScope,
    TransactionState,
    TransactionResult,
    TransactionalRepository,
    execute_in_transaction,
};
