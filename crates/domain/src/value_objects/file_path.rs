//! FilePath Value Object
//!
//! Representa um caminho de arquivo validado.
//! Implementado usando TDD.

use crate::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Caminho de arquivo validado
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FilePath(PathBuf);

impl FilePath {
    /// Cria um novo FilePath validado
    pub fn new<P: AsRef<Path>>(path: P) -> DomainResult<Self> {
        let path = path.as_ref();

        // Validações básicas
        if path.as_os_str().is_empty() {
            return Err(DomainError::InvalidFilePath(
                "Path cannot be empty".to_string(),
            ));
        }

        Ok(Self(path.to_path_buf()))
    }

    /// Cria FilePath sem validação (uso interno)
    #[cfg(test)]
    pub fn new_unchecked<P: AsRef<Path>>(path: P) -> Self {
        Self(path.as_ref().to_path_buf())
    }

    /// Retorna o path como &Path
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Retorna o path como PathBuf
    pub fn to_path_buf(&self) -> PathBuf {
        self.0.clone()
    }

    /// Retorna o nome do arquivo
    pub fn file_name(&self) -> Option<&str> {
        self.0.file_name().and_then(|os_str| os_str.to_str())
    }

    /// Retorna a extensão do arquivo
    pub fn extension(&self) -> Option<&str> {
        self.0.extension().and_then(|os_str| os_str.to_str())
    }

    /// Retorna o diretório pai
    pub fn parent(&self) -> Option<&Path> {
        self.0.parent()
    }

    /// Verifica se o arquivo existe
    pub fn exists(&self) -> bool {
        self.0.exists()
    }

    /// Retorna string do path
    pub fn as_str(&self) -> DomainResult<&str> {
        self.0
            .to_str()
            .ok_or_else(|| DomainError::InvalidFilePath("Invalid UTF-8 in path".to_string()))
    }

    /// Retorna string do path (lossy conversion)
    pub fn to_string_lossy(&self) -> String {
        self.0.to_string_lossy().to_string()
    }
}

impl AsRef<Path> for FilePath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl std::fmt::Display for FilePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl TryFrom<&str> for FilePath {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for FilePath {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<PathBuf> for FilePath {
    type Error = DomainError;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filepath_creation_with_valid_path() {
        // Arrange & Act
        let path1 = FilePath::new("/path/to/photo.jpg");
        let path2 = FilePath::new("relative/path/image.raw");
        let path3 = FilePath::new("C:\\Users\\photo.dng");

        // Assert
        assert!(path1.is_ok());
        assert!(path2.is_ok());
        assert!(path3.is_ok());
    }

    #[test]
    fn test_filepath_creation_with_empty_path() {
        // Act
        let result = FilePath::new("");

        // Assert
        assert!(result.is_err());
        match result {
            Err(DomainError::InvalidFilePath(msg)) => {
                assert!(msg.contains("empty"));
            }
            _ => panic!("Expected InvalidFilePath error"),
        }
    }

    #[test]
    fn test_filepath_as_path() {
        // Arrange
        let filepath = FilePath::new("/path/to/photo.jpg").unwrap();

        // Act
        let path = filepath.as_path();

        // Assert
        assert_eq!(path, Path::new("/path/to/photo.jpg"));
    }

    #[test]
    fn test_filepath_file_name() {
        // Arrange
        let filepath = FilePath::new("/path/to/photo.jpg").unwrap();

        // Act
        let filename = filepath.file_name();

        // Assert
        assert_eq!(filename, Some("photo.jpg"));
    }

    #[test]
    fn test_filepath_file_name_with_no_name() {
        // Arrange
        let filepath = FilePath::new("/").unwrap();

        // Act
        let filename = filepath.file_name();

        // Assert
        assert_eq!(filename, None);
    }

    #[test]
    fn test_filepath_extension() {
        // Arrange
        let filepath1 = FilePath::new("/path/photo.jpg").unwrap();
        let filepath2 = FilePath::new("/path/photo.RAW").unwrap();
        let filepath3 = FilePath::new("/path/noextension").unwrap();

        // Assert
        assert_eq!(filepath1.extension(), Some("jpg"));
        assert_eq!(filepath2.extension(), Some("RAW"));
        assert_eq!(filepath3.extension(), None);
    }

    #[test]
    fn test_filepath_parent() {
        // Arrange
        let filepath = FilePath::new("/path/to/photo.jpg").unwrap();

        // Act
        let parent = filepath.parent();

        // Assert
        assert_eq!(parent, Some(Path::new("/path/to")));
    }

    #[test]
    fn test_filepath_display() {
        // Arrange
        let filepath = FilePath::new("/path/to/photo.jpg").unwrap();

        // Act
        let display = format!("{}", filepath);

        // Assert
        assert!(display.contains("photo.jpg"));
    }

    #[test]
    fn test_filepath_try_from_str() {
        // Act
        let result: Result<FilePath, _> = "/path/photo.jpg".try_into();

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_filepath_try_from_string() {
        // Arrange
        let path_string = String::from("/path/photo.jpg");

        // Act
        let result: Result<FilePath, _> = path_string.try_into();

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_filepath_try_from_pathbuf() {
        // Arrange
        let path_buf = PathBuf::from("/path/photo.jpg");

        // Act
        let result: Result<FilePath, _> = path_buf.try_into();

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_filepath_equality() {
        // Arrange
        let path1 = FilePath::new("/path/photo.jpg").unwrap();
        let path2 = FilePath::new("/path/photo.jpg").unwrap();
        let path3 = FilePath::new("/other/photo.jpg").unwrap();

        // Assert
        assert_eq!(path1, path2);
        assert_ne!(path1, path3);
    }

    #[test]
    fn test_filepath_to_string_lossy() {
        // Arrange
        let filepath = FilePath::new("/path/to/photo.jpg").unwrap();

        // Act
        let string = filepath.to_string_lossy();

        // Assert
        assert!(string.contains("photo.jpg"));
    }

    #[test]
    fn test_filepath_as_ref() {
        // Arrange
        let filepath = FilePath::new("/path/photo.jpg").unwrap();

        // Act
        let path_ref: &Path = filepath.as_ref();

        // Assert
        assert_eq!(path_ref, Path::new("/path/photo.jpg"));
    }
}
