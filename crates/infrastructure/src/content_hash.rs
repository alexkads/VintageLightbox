//! Content Hash Utility
//!
//! Provides SHA-256 hash calculation for image files.
//! Used for duplicate detection during import.

use sha2::{Sha256, Digest};
use std::path::Path;
use std::fs::File;
use std::io::{BufReader, Read};
use domain::{DomainError, DomainResult};

/// Calculate SHA-256 hash of a file's content (synchronous version)
/// Returns a hex-encoded string of the hash
pub fn calculate_file_hash(path: &Path) -> DomainResult<String> {
    let file = File::open(path)
        .map_err(|e| DomainError::InfrastructureError(format!("Failed to open file for hashing: {}", e)))?;

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to read file for hashing: {}", e)))?;

        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

/// Calculate SHA-256 hash of a file's content (async version)
/// Returns a hex-encoded string of the hash
/// Uses tokio::fs for non-blocking I/O
pub async fn calculate_file_hash_async(path: &Path) -> DomainResult<String> {
    use tokio::io::AsyncReadExt;

    let file = tokio::fs::File::open(path).await
        .map_err(|e| DomainError::InfrastructureError(format!("Failed to open file for hashing: {}", e)))?;

    let mut reader = tokio::io::BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer).await
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to read file for hashing: {}", e)))?;

        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;
    
    #[test]
    fn test_calculate_file_hash() {
        // Create a temp file with known content
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"test content for hashing").unwrap();
        file.flush().unwrap();
        
        let hash = calculate_file_hash(file.path()).unwrap();
        
        // Hash should be 64 hex characters (256 bits)
        assert_eq!(hash.len(), 64);
        // Same content should produce same hash
        let hash2 = calculate_file_hash(file.path()).unwrap();
        assert_eq!(hash, hash2);
    }
    
    #[test]
    fn test_hash_different_content() {
        let mut file1 = NamedTempFile::new().unwrap();
        file1.write_all(b"content 1").unwrap();
        file1.flush().unwrap();
        
        let mut file2 = NamedTempFile::new().unwrap();
        file2.write_all(b"content 2").unwrap();
        file2.flush().unwrap();
        
        let hash1 = calculate_file_hash(file1.path()).unwrap();
        let hash2 = calculate_file_hash(file2.path()).unwrap();
        
        assert_ne!(hash1, hash2);
    }
    
    #[test]
    fn test_hash_nonexistent_file() {
        let result = calculate_file_hash(Path::new("/nonexistent/file.jpg"));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_calculate_file_hash_async() {
        // Create a temp file with known content
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"test content for async hashing").unwrap();
        file.flush().unwrap();

        let hash = calculate_file_hash_async(file.path()).await.unwrap();

        // Hash should be 64 hex characters (256 bits)
        assert_eq!(hash.len(), 64);
        // Same content should produce same hash
        let hash2 = calculate_file_hash_async(file.path()).await.unwrap();
        assert_eq!(hash, hash2);
    }

    #[tokio::test]
    async fn test_async_hash_matches_sync() {
        // Verify that async and sync versions produce the same hash
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"consistent hashing test").unwrap();
        file.flush().unwrap();

        let sync_hash = calculate_file_hash(file.path()).unwrap();
        let async_hash = calculate_file_hash_async(file.path()).await.unwrap();

        assert_eq!(sync_hash, async_hash, "Async and sync hashes should match");
    }
}
