use domain::{
    value_objects::FilePath,
    services::{RawDecoder, RawImage},
    DomainResult,
    DomainError,
};
use async_trait::async_trait;


/// Implementação do RawDecoder usando a crate `rawloader`
pub struct RawDecoderImpl;

impl RawDecoderImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RawDecoderImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RawDecoder for RawDecoderImpl {
    fn decode(&self, path: &FilePath) -> DomainResult<RawImage> {
        let path_str = path.as_ref().to_str()
            .ok_or_else(|| DomainError::InvalidOperation("Invalid path encoding".to_string()))?;

        // rawloader::decode_file returns Result<RawImage, RAWError>
        let raw_image = rawloader::decode_file(path_str)
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to decode RAW file: {}", e)))?;

        // Convert rawloader::RawImage to domain::RawImage
        let data = match raw_image.data {
            rawloader::RawImageData::Integer(v) => v,
            _ => return Err(DomainError::InfrastructureError("Unsupported RAW data format".to_string())),
        };

        Ok(RawImage {
            width: raw_image.width,
            height: raw_image.height,
            data,
            cpp: raw_image.cpp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_decode_invalid_file() {
        let decoder = RawDecoderImpl::new();
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Not a RAW file").unwrap();
        
        let path = FilePath::new(file.path().to_str().unwrap()).unwrap();
        let result = decoder.decode(&path);
        
        assert!(result.is_err());
        match result {
            Err(DomainError::InfrastructureError(_)) => assert!(true),
            _ => assert!(false, "Expected InfrastructureError"),
        }
    }
    
    // Note: Testing successful decoding requires a real RAW file, 
    // which we don't have in the repo. This would be covered by integration tests
    // with test assets in a real environment.
}
