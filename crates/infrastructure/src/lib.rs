//! # Infrastructure Layer - VintageLightbox
//!
//! Esta camada contém implementações concretas de persistência,
//! file system, e outras integrações externas.

pub mod paths;
pub mod database;
pub mod file_system;
pub mod cache;
pub mod raw_processing;
pub mod scan_directory;
pub mod exif_reader;

pub use database::{PhotoRepositoryImpl, CollectionRepositoryImpl, create_pool, run_migrations};
pub use scan_directory::{ScanDirectoryUseCase, ScanDirectoryResult};
pub use exif_reader::ExifReader;
pub mod thumbnail_generator;
pub use thumbnail_generator::ThumbnailGeneratorImpl;
pub use raw_processing::{RawDecoderImpl, is_raw_file, load_raw_as_dynamic_image};
pub mod image_exporter;
pub use image_exporter::ImageExporterImpl;
pub mod content_hash;
pub use content_hash::calculate_file_hash;
pub mod file_organizer;
pub use file_organizer::FileOrganizerImpl;
