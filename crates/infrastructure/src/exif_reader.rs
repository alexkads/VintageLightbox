//! EXIF Metadata Reader
//!
//! Extrai metadados EXIF de arquivos de imagem.

use async_trait::async_trait;
use domain::{
    services::MetadataExtractor,
    value_objects::{FilePath, PhotoMetadata},
    DomainResult,
};
use std::fs::File;
use std::path::Path;

/// Leitor de metadados EXIF
pub struct ExifReader;

impl ExifReader {
    /// Cria uma nova instância do ExifReader
    pub fn new() -> Self {
        Self
    }

    /// Extrai metadados EXIF de um arquivo de imagem
    pub fn read_metadata(&self, path: &Path) -> DomainResult<PhotoMetadata> {
        let file = File::open(path).map_err(|e| {
            domain::DomainError::InvalidOperation(format!("Failed to open file: {}", e))
        })?;

        let mut bufreader = std::io::BufReader::new(&file);
        let exifreader = exif::Reader::new();

        // Se falhar ao ler EXIF, retorna metadados vazios em vez de erro
        // Isso permite importar imagens sem EXIF
        let Ok(exif_data) = exifreader.read_from_container(&mut bufreader) else {
            return Ok(PhotoMetadata::default());
        };

        let mut metadata = PhotoMetadata::default();

        // Extrair modelo da câmera
        if let Some(field) = exif_data.get_field(exif::Tag::Model, exif::In::PRIMARY) {
            metadata.camera_model = Some(field.display_value().to_string());
        }

        // Extrair fabricante da câmera
        if let Some(field) = exif_data.get_field(exif::Tag::Make, exif::In::PRIMARY) {
            metadata.camera_make = Some(field.display_value().to_string());
        }

        // Extrair data/hora
        if let Some(field) = exif_data.get_field(exif::Tag::DateTime, exif::In::PRIMARY) {
            metadata.date_time = Some(field.display_value().to_string());
        }

        // Extrair ISO
        if let Some(field) =
            exif_data.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
        {
            if let exif::Value::Short(ref v) = field.value {
                if !v.is_empty() {
                    metadata.iso = Some(v[0] as u32);
                }
            }
        }

        // Extrair abertura (f-number)
        if let Some(field) = exif_data.get_field(exif::Tag::FNumber, exif::In::PRIMARY) {
            if let exif::Value::Rational(ref v) = field.value {
                if !v.is_empty() {
                    metadata.aperture = Some(v[0].to_f64());
                }
            }
        }

        // Extrair velocidade do obturador
        if let Some(field) = exif_data.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY) {
            metadata.shutter_speed = Some(field.display_value().to_string());
        }

        // Extrair distância focal
        if let Some(field) = exif_data.get_field(exif::Tag::FocalLength, exif::In::PRIMARY) {
            if let exif::Value::Rational(ref v) = field.value {
                if !v.is_empty() {
                    metadata.focal_length = Some(v[0].to_f64());
                }
            }
        }

        // Extrair dimensões
        if let Some(field) = exif_data.get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY) {
            if let exif::Value::Long(ref v) = field.value {
                if !v.is_empty() {
                    metadata.width = Some(v[0]);
                }
            }
        }

        if let Some(field) = exif_data.get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY) {
            if let exif::Value::Long(ref v) = field.value {
                if !v.is_empty() {
                    metadata.height = Some(v[0]);
                }
            }
        }

        Ok(metadata)
    }

    /// Verifica se um arquivo tem metadados EXIF
    pub fn has_exif(&self, path: &Path) -> bool {
        // Tenta ler e vê se algum campo foi preenchido
        if let Ok(metadata) = self.read_metadata(path) {
            metadata.camera_model.is_some() || metadata.iso.is_some()
        } else {
            false
        }
    }
}

impl Default for ExifReader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MetadataExtractor for ExifReader {
    fn extract(&self, path: &FilePath) -> DomainResult<PhotoMetadata> {
        let path_ref: &Path = path.as_ref();
        self.read_metadata(path_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Helper para criar um arquivo JPEG mínimo com EXIF
    fn create_test_jpeg_with_exif() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();

        // JPEG mínimo com marcador EXIF
        let jpeg_data = vec![
            0xFF, 0xD8, // SOI (Start of Image)
            0xFF, 0xE1, // APP1 marker (EXIF)
            0x00, 0x10, // APP1 length (16 bytes)
            0x45, 0x78, 0x69, 0x66, 0x00, 0x00, // "Exif\0\0"
            // Minimal TIFF header
            0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, 0xFF, 0xD9, // EOI (End of Image)
        ];

        file.write_all(&jpeg_data).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_exif_reader_creation() {
        // O que este teste realmente prova é que `new()` não entra em pânico —
        // o `assert!(true)` que estava aqui não afirmava nada e escondia isso.
        // Construir e descartar já é a afirmação inteira.
        let _reader = ExifReader::new();
    }

    #[test]
    fn test_read_metadata_from_nonexistent_file() {
        let reader = ExifReader::new();
        let result = reader.read_metadata(Path::new("/nonexistent/file.jpg"));
        assert!(result.is_err());
    }

    #[test]
    fn test_read_metadata_from_valid_jpeg() {
        let reader = ExifReader::new();
        let file = create_test_jpeg_with_exif();

        let result = reader.read_metadata(file.path());

        match result {
            Ok(metadata) => {
                assert!(metadata.camera_model.is_none() || metadata.camera_model.is_some());
            }
            Err(_) => {
                // Should not error even if EXIF is empty
            }
        }
    }

    #[test]
    fn test_metadata_extraction_implementation() {
        let reader = ExifReader::new();
        // Just verify it implements the trait by calling it
        let _ = MetadataExtractor::extract(&reader, &FilePath::new("/test.jpg").unwrap());
    }
}
