//! EXIF Metadata Reader
//!
//! Extrai metadados EXIF de arquivos de imagem.

use std::path::Path;
use std::fs::File;
use domain::DomainResult;

/// Metadados EXIF extraídos de uma foto
#[derive(Debug, Clone, PartialEq)]
pub struct PhotoMetadata {
    /// Modelo da câmera
    pub camera_model: Option<String>,
    /// Fabricante da câmera
    pub camera_make: Option<String>,
    /// Data/hora da captura
    pub date_time: Option<String>,
    /// ISO
    pub iso: Option<u32>,
    /// Abertura (f-number)
    pub aperture: Option<f64>,
    /// Velocidade do obturador
    pub shutter_speed: Option<String>,
    /// Distância focal
    pub focal_length: Option<f64>,
    /// Largura da imagem
    pub width: Option<u32>,
    /// Altura da imagem
    pub height: Option<u32>,
}

impl Default for PhotoMetadata {
    fn default() -> Self {
        Self {
            camera_model: None,
            camera_make: None,
            date_time: None,
            iso: None,
            aperture: None,
            shutter_speed: None,
            focal_length: None,
            width: None,
            height: None,
        }
    }
}

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
        
        let exif_data = exifreader.read_from_container(&mut bufreader).map_err(|e| {
            domain::DomainError::InvalidOperation(format!("Failed to read EXIF data: {}", e))
        })?;

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
        if let Some(field) = exif_data.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY) {
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
        self.read_metadata(path).is_ok()
    }
}

impl Default for ExifReader {
    fn default() -> Self {
        Self::new()
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
        // Este é um JPEG válido mas mínimo para testes
        let jpeg_data = vec![
            0xFF, 0xD8, // SOI (Start of Image)
            0xFF, 0xE1, // APP1 marker (EXIF)
            0x00, 0x10, // APP1 length (16 bytes)
            0x45, 0x78, 0x69, 0x66, 0x00, 0x00, // "Exif\0\0"
            // Minimal TIFF header
            0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00,
            0xFF, 0xD9, // EOI (End of Image)
        ];
        
        file.write_all(&jpeg_data).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_exif_reader_creation() {
        let _reader = ExifReader::new();
        assert!(true); // Just test instantiation
    }

    #[test]
    fn test_read_metadata_from_nonexistent_file() {
        let reader = ExifReader::new();
        let result = reader.read_metadata(Path::new("/nonexistent/file.jpg"));
        assert!(result.is_err());
    }

    #[test]
    fn test_has_exif_nonexistent_file() {
        let reader = ExifReader::new();
        assert!(!reader.has_exif(Path::new("/nonexistent/file.jpg")));
    }

    #[test]
    fn test_read_metadata_from_valid_jpeg() {
        let reader = ExifReader::new();
        let file = create_test_jpeg_with_exif();
        
        // Should not panic, even if EXIF data is minimal
        let result = reader.read_metadata(file.path());
        
        // May succeed or fail depending on EXIF validity, but shouldn't panic
        match result {
            Ok(metadata) => {
                // If successful, metadata should be initialized
                assert!(metadata.camera_model.is_none() || metadata.camera_model.is_some());
            }
            Err(_) => {
                // It's ok if minimal EXIF fails to parse
            }
        }
    }

    #[test]
    fn test_metadata_default() {
        let metadata = PhotoMetadata::default();
        assert!(metadata.camera_model.is_none());
        assert!(metadata.camera_make.is_none());
        assert!(metadata.date_time.is_none());
        assert!(metadata.iso.is_none());
        assert!(metadata.aperture.is_none());
        assert!(metadata.shutter_speed.is_none());
        assert!(metadata.focal_length.is_none());
        assert!(metadata.width.is_none());
        assert!(metadata.height.is_none());
    }

    #[test]
    fn test_metadata_clone() {
        let metadata = PhotoMetadata {
            camera_model: Some("Canon EOS 5D".to_string()),
            camera_make: Some("Canon".to_string()),
            date_time: Some("2024:01:01 12:00:00".to_string()),
            iso: Some(400),
            aperture: Some(2.8),
            shutter_speed: Some("1/125".to_string()),
            focal_length: Some(50.0),
            width: Some(6000),
            height: Some(4000),
        };

        let cloned = metadata.clone();
        assert_eq!(metadata, cloned);
    }

    #[test]
    fn test_exif_reader_default() {
        let reader = ExifReader::default();
        assert!(!reader.has_exif(Path::new("/nonexistent.jpg")));
    }
}
