//! PhotoId Value Object
//!
//! Identificador único para fotos usando UUID v4.
//! Implementado usando TDD.

use crate::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identificador único de uma foto
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PhotoId(Uuid);

impl PhotoId {
    /// Cria um novo PhotoId com UUID aleatório
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Cria um PhotoId a partir de um UUID existente
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Cria um PhotoId a partir de uma string UUID
    pub fn from_string(s: &str) -> DomainResult<Self> {
        Uuid::parse_str(s)
            .map(Self)
            .map_err(|_| DomainError::InvalidPhotoId)
    }

    /// Retorna o UUID interno
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Retorna a representação em string
    pub fn as_string(&self) -> String {
        self.0.to_string()
    }
}

impl Default for PhotoId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PhotoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for PhotoId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<PhotoId> for Uuid {
    fn from(id: PhotoId) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_photo_id_creation() {
        // Act
        let id1 = PhotoId::new();
        let id2 = PhotoId::new();

        // Assert - IDs devem ser únicos
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_photo_id_from_uuid() {
        // Arrange
        let uuid = Uuid::new_v4();

        // Act
        let id = PhotoId::from_uuid(uuid);

        // Assert
        assert_eq!(id.as_uuid(), uuid);
    }

    #[test]
    fn test_photo_id_from_valid_string() {
        // Arrange
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";

        // Act
        let result = PhotoId::from_string(uuid_str);

        // Assert
        assert!(result.is_ok());
        let id = result.unwrap();
        assert_eq!(id.as_string(), uuid_str);
    }

    #[test]
    fn test_photo_id_from_invalid_string() {
        // Arrange
        let invalid_strings = vec![
            "not-a-uuid",
            "12345",
            "",
            "550e8400-e29b-41d4-a716", // UUID incompleto
        ];

        // Act & Assert
        for invalid in invalid_strings {
            let result = PhotoId::from_string(invalid);
            assert!(result.is_err());
            assert_eq!(result.unwrap_err(), DomainError::InvalidPhotoId);
        }
    }

    #[test]
    fn test_photo_id_as_string() {
        // Arrange
        let id = PhotoId::new();

        // Act
        let string = id.as_string();

        // Assert - deve ser um UUID válido
        assert!(Uuid::parse_str(&string).is_ok());
    }

    #[test]
    fn test_photo_id_display() {
        // Arrange
        let id = PhotoId::new();

        // Act
        let display = format!("{}", id);

        // Assert
        assert_eq!(display, id.as_string());
    }

    #[test]
    fn test_photo_id_equality() {
        // Arrange
        let uuid = Uuid::new_v4();
        let id1 = PhotoId::from_uuid(uuid);
        let id2 = PhotoId::from_uuid(uuid);
        let id3 = PhotoId::new();

        // Assert
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_photo_id_hash() {
        use std::collections::HashMap;

        // Arrange
        let id = PhotoId::new();
        let mut map = HashMap::new();

        // Act
        map.insert(id, "test_photo");

        // Assert
        assert_eq!(map.get(&id), Some(&"test_photo"));
    }

    #[test]
    fn test_photo_id_default() {
        // Act
        let id1 = PhotoId::default();
        let id2 = PhotoId::default();

        // Assert - defaults devem ser únicos
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_photo_id_roundtrip_string() {
        // Arrange
        let id1 = PhotoId::new();

        // Act
        let string = id1.as_string();
        let id2 = PhotoId::from_string(&string).unwrap();

        // Assert
        assert_eq!(id1, id2);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;

    #[test]
    fn test_photo_id_roundtrip_always_works() {
        // Arrange
        for _ in 0..100 {
            let id1 = PhotoId::new();

            // Act
            let string = id1.as_string();
            let id2 = PhotoId::from_string(&string).unwrap();

            // Assert
            assert_eq!(id1, id2);
        }
    }
}
