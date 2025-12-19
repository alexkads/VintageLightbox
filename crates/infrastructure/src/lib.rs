//! # Infrastructure Layer - VintageLightbox
//!
//! Esta camada contém implementações concretas de persistência,
//! file system, e outras integrações externas.

pub mod database;
pub mod file_system;
pub mod raw_processing;
pub mod scan_directory;
pub mod exif_reader;

pub use database::{PhotoRepositoryImpl, CollectionRepositoryImpl, create_pool, run_migrations};
pub use scan_directory::{ScanDirectoryUseCase, ScanDirectoryResult};
pub use exif_reader::ExifReader;
pub mod thumbnail_generator;
pub use thumbnail_generator::ThumbnailGeneratorImpl;
pub use raw_processing::RawDecoderImpl;
pub mod image_exporter;
pub use image_exporter::ImageExporterImpl;
