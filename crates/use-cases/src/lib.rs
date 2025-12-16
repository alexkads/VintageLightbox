//! # Use Cases Layer - VintageLightbox
//!
//! Esta camada contém as regras de negócio da aplicação,
//! orquestrando o uso das entidades de domínio.

pub mod import_photo;
pub mod import_photos;
pub mod rate_photo;
pub mod create_collection;

pub use import_photo::ImportPhotoUseCase;
pub use import_photos::{ImportPhotosUseCase, BatchImportResult};
pub use rate_photo::RatePhotoUseCase;
pub use create_collection::CreateCollectionUseCase;

pub mod import;
pub mod edit;
pub mod organize;
pub mod export;

// Re-exports
