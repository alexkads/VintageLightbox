pub mod cache;
pub mod database;
pub mod devices;
// pub mod fs; // Removing fs as it seems problematic and might not be used directly or empty.
pub mod content_hash;
/// Por que um DNG não abre — quando a resposta é "não é o arquivo".
pub mod dng;
pub mod exif_reader;
pub mod file_organizer;
pub mod file_system;
/// O motor de revelação: wgpu, o WGSL e os 46 ajustes.
///
/// 🔑 Mora aqui porque a **tela** e o **arquivo exportado** precisam atravessar
/// o mesmo shader. Enquanto ele vivia no crate de interface, a exportação tinha
/// a própria implementação da mesma matemática, na CPU, com 15 dos 46 ajustes.
pub mod gpu_adjustments;
pub mod image_exporter;
pub mod paths;
pub mod raw_processing;
pub mod scan_directory;
pub mod source_scanner;
pub mod thumbnail_generator;
/// Do pixel revelado ao pixel exibido: corte, giro, espelho e endireitamento.
///
/// 🔑 Veio do `ui-gpui` pela mesma razão que o motor: a exportação tem de
/// entregar o arquivo com o mesmo enquadramento que a tela mostra.
pub mod transformacao;

// Re-exports for main.rs compatibility
pub use database::{
    create_pool, run_migrations, CollectionRepositoryImpl, PhotoRepositoryImpl,
    SqlitePresetRepository,
};
pub use exif_reader::ExifReader;
pub use file_organizer::FileOrganizerImpl;
pub use image_exporter::ImageExporterImpl;
pub use source_scanner::SourceScannerImpl;
pub use thumbnail_generator::ThumbnailGeneratorImpl;
// pub use fs::FileSystemImpl; // Commenting out until verified
