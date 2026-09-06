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
/// O motor de revelação (`Ajustes`, `Motor`) re-exportado do `revelacao-core`,
/// mais a leitura da entidade.
///
/// 🔑 A tela e o arquivo exportado atravessam o mesmo shader — e, desde
/// 2026-09-04, o navegador também: o motor mora num crate sem banco nem rede,
/// que compila para wasm32.
pub mod gpu_adjustments;
pub mod image_exporter;
pub mod paths;
pub mod pos_venda;
pub mod raw_processing;
pub mod scan_directory;
pub mod source_scanner;
pub mod thumbnail_generator;
/// Do pixel revelado ao pixel exibido: a ponte entre `CropSettings` e o
/// `Corte` do `revelacao-core`, onde a geometria mora desde 2026-09-04.
pub mod transformacao;

// Re-exports for main.rs compatibility
pub use database::{
    create_pool, run_migrations, CollectionRepositoryImpl, PhotoRepositoryImpl,
    SqlitePresetRepository,
};
pub use exif_reader::ExifReader;
pub use file_organizer::FileOrganizerImpl;
pub use image_exporter::ImageExporterImpl;
pub use pos_venda::{CofreDoSistema, CofreEmMemoria, PosVendaApiHttp};
pub use source_scanner::SourceScannerImpl;
pub use thumbnail_generator::ThumbnailGeneratorImpl;
// pub use fs::FileSystemImpl; // Commenting out until verified
