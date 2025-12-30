pub mod cache;
pub mod database;
pub mod devices;
// pub mod fs; // Removing fs as it seems problematic and might not be used directly or empty.
pub mod exif_reader;
pub mod file_organizer;
pub mod file_system;
pub mod image_exporter;
pub mod paths;
pub mod raw_processing;
pub mod scan_directory;
pub mod thumbnail_generator;
pub mod content_hash;
pub mod intelligent_fill;
pub mod image_processing;
pub mod services;

// Re-exports for main.rs compatibility
pub use database::{create_pool, run_migrations, PhotoRepositoryImpl, SqlitePresetRepository, CollectionRepositoryImpl};
pub use exif_reader::ExifReader;
pub use thumbnail_generator::ThumbnailGeneratorImpl;
pub use image_exporter::ImageExporterImpl;
pub use file_organizer::FileOrganizerImpl;
pub use intelligent_fill::OnnxInpainterImpl;
// pub use fs::FileSystemImpl; // Commenting out until verified
