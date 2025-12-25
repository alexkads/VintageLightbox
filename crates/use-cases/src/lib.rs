pub mod import_photo;
pub mod preview_before_import;
pub mod check_duplicates;
pub mod import_with_options;
pub mod import;
pub mod export_photo;
pub mod save_photo_edits;
pub mod presets;
pub mod rate_photo;
pub mod set_color_label;
pub mod set_flag;
pub mod delete_photo;
pub mod add_photo_to_collection;
pub mod remove_photo_from_collection;
pub mod create_collection;
pub mod organize;
pub mod edit;
pub mod export;
pub mod import_photos; // Likely legacy but better include

// Re-export common types
pub use import_photo::ImportPhotoUseCase;
pub use preview_before_import::PreviewBeforeImportUseCase;
pub use check_duplicates::CheckDuplicatesUseCase;
pub use import_with_options::{ImportWithOptionsUseCase, ImportRequest, ImportProgress};
pub use export_photo::ExportPhotoUseCase;
pub use import::get_import_sources::GetImportSourcesUseCase;
pub use save_photo_edits::SavePhotoEditsUseCase;

// Exports for PhotoController
pub use rate_photo::RatePhotoUseCase;
pub use set_color_label::SetColorLabelUseCase;
pub use set_flag::SetFlagUseCase;
pub use delete_photo::DeletePhotoUseCase;

// Exports for LibraryController?
pub use create_collection::CreateCollectionUseCase;
pub use add_photo_to_collection::AddPhotoToCollectionUseCase;
pub use remove_photo_from_collection::RemovePhotoFromCollectionUseCase;
