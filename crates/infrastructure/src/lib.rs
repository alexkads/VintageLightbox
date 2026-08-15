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
pub mod source_scanner;
pub mod thumbnail_generator;
pub mod content_hash;

// Re-exports for main.rs compatibility
pub use database::{create_pool, run_migrations, PhotoRepositoryImpl, SqlitePresetRepository, CollectionRepositoryImpl};
pub use exif_reader::ExifReader;
pub use thumbnail_generator::ThumbnailGeneratorImpl;
pub use image_exporter::ImageExporterImpl;
pub use file_organizer::FileOrganizerImpl;
pub use source_scanner::SourceScannerImpl;
// pub use fs::FileSystemImpl; // Commenting out until verified
