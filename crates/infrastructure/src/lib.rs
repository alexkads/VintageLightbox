//! # Infrastructure Layer - VintageLightbox
//!
//! Esta camada contém implementações concretas de persistência,
//! file system, e outras integrações externas.

pub mod database;
pub mod file_system;
pub mod raw_processing;

pub use database::{PhotoRepositoryImpl, CollectionRepositoryImpl, create_pool, run_migrations};
