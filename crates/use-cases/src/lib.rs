//! # Use Cases Layer - VintageLightbox
//!
//! Esta camada contém a lógica de negócio da aplicação,
//! orquestrando as operações entre o domain e a infraestrutura.

pub mod import_photo;
pub mod import_photos;
pub mod preview_before_import;
pub mod check_duplicates;
pub mod import_with_options;
pub mod rate_photo;
pub mod set_color_label;
pub mod create_collection;
pub mod add_photo_to_collection;
pub mod remove_photo_from_collection;
pub mod delete_photo;

pub use import_photo::ImportPhotoUseCase;
pub use import_photos::{ImportPhotosUseCase, BatchImportResult};
pub use preview_before_import::{PreviewBeforeImportUseCase, ImportPreviewItem};
pub use check_duplicates::{CheckDuplicatesUseCase, DuplicateCheckResult};
pub use import_with_options::{ImportWithOptionsUseCase, ImportRequest, ImportProgress, ImportResult};
pub use rate_photo::RatePhotoUseCase;
pub use set_color_label::SetColorLabelUseCase;
pub use create_collection::CreateCollectionUseCase;
pub use add_photo_to_collection::AddPhotoToCollectionUseCase;
pub use remove_photo_from_collection::RemovePhotoFromCollectionUseCase;
pub use delete_photo::DeletePhotoUseCase;
pub mod save_photo_edits;
pub use save_photo_edits::SavePhotoEditsUseCase;

pub mod export_photo;
pub use export_photo::ExportPhotoUseCase;

pub mod import;
pub mod edit;
pub mod organize;
pub mod export;

// Re-exports
