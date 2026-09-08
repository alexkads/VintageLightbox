pub mod add_photo_to_collection;
pub mod check_duplicates;
pub mod configure_print_job;
pub mod create_collection;
pub mod delete_photo;
pub mod edit;
pub mod export;
pub mod export_photo;
pub mod import;
pub mod import_photo;
pub mod import_photos; // Likely legacy but better include
pub mod import_with_options;
pub mod marcar_comprada;
pub mod organize;
pub mod pos_venda;
pub mod presets;
pub mod preview_before_import;
pub mod rate_photo;
pub mod remove_photo_from_collection;
pub mod save_photo_edits;
pub mod set_color_label;
pub mod set_flag;

// Re-export common types
pub use check_duplicates::CheckDuplicatesUseCase;
pub use export_photo::ExportPhotoUseCase;
pub use import::describe_candidates::{DescribeCandidatesUseCase, ImportCandidate};
pub use import::get_import_sources::GetImportSourcesUseCase;
pub use import::scan_source::ScanSourceUseCase;
pub use import_photo::ImportPhotoUseCase;
pub use import_with_options::{ImportProgress, ImportRequest, ImportWithOptionsUseCase};
pub use preview_before_import::PreviewBeforeImportUseCase;
pub use save_photo_edits::SavePhotoEditsUseCase;

// Exports for PhotoController
pub use delete_photo::DeletePhotoUseCase;
pub use marcar_comprada::MarcarCompradaUseCase;
pub use rate_photo::RatePhotoUseCase;
pub use set_color_label::SetColorLabelUseCase;
pub use set_flag::SetFlagUseCase;

// Exports for LibraryController?
pub use add_photo_to_collection::AddPhotoToCollectionUseCase;
pub use create_collection::CreateCollectionUseCase;
pub use remove_photo_from_collection::RemovePhotoFromCollectionUseCase;

// Exports for PrintController
pub use configure_print_job::ConfigurePrintJobUseCase;
