//! Collection ID Value Object
//!
//! Identificador único para coleções.
//! Implementado usando TDD.

use crate::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Collection ID - identificador único de coleção usando UUID v4
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CollectionId(Uuid);

impl CollectionId {
    /// Cria um novo CollectionId com UUID gerado randomicamente
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Cria CollectionId a partir de uma string UUID válida
    pub fn from_string(s: &str) -> DomainResult<Self> {
        let uuid = Uuid::parse_str(s).map_err(|_| DomainError::InvalidId {
            id: s.to_string(),
            reason: "Invalid UUID format".to_string(),
        })?;
        Ok(Self(uuid))
    }

    /// Retorna o UUID interno
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Converte para string
    pub fn as_string(&self) -> String {
        self.0.to_string()
    }
}

impl Default for CollectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for CollectionId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl fmt::Display for CollectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_id_creation() {
        let id1 = CollectionId::new();
        let id2 = CollectionId::new();

        // Cada ID deve ser único
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_collection_id_from_valid_string() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let id = CollectionId::from_string(uuid_str).unwrap();

        assert_eq!(id.as_string(), uuid_str);
    }

    #[test]
    fn test_collection_id_from_invalid_string() {
        let result = CollectionId::from_string("not-a-uuid");

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DomainError::InvalidId { .. }));
    }

    #[test]
    fn test_collection_id_display() {
        let uuid = Uuid::new_v4();
        let id = CollectionId::from(uuid);

        assert_eq!(id.to_string(), uuid.to_string());
    }

    #[test]
    fn test_collection_id_equality() {
        let uuid = Uuid::new_v4();
        let id1 = CollectionId::from(uuid);
        let id2 = CollectionId::from(uuid);

        assert_eq!(id1, id2);
    }

    #[test]
    fn test_collection_id_default() {
        let id = CollectionId::default();

        assert!(!id.as_uuid().is_nil());
    }

    #[test]
    fn test_collection_id_as_uuid() {
        let uuid = Uuid::new_v4();
        let id = CollectionId::from(uuid);

        assert_eq!(id.as_uuid(), &uuid);
    }

    #[test]
    fn test_collection_id_roundtrip_string() {
        let id1 = CollectionId::new();
        let id_str = id1.as_string();
        let id2 = CollectionId::from_string(&id_str).unwrap();

        assert_eq!(id1, id2);
    }

    #[test]
    fn test_collection_id_from_uuid() {
        let uuid = Uuid::new_v4();
        let id = CollectionId::from(uuid);

        assert_eq!(id.as_uuid(), &uuid);
    }

    #[test]
    fn test_collection_id_hash() {
        use std::collections::HashSet;

        let id1 = CollectionId::new();
        let id2 = id1;
        let id3 = CollectionId::new();

        let mut set = HashSet::new();
        set.insert(id1);
        set.insert(id2); // Deve ser o mesmo que id1
        set.insert(id3);

        assert_eq!(set.len(), 2); // Apenas 2 IDs únicos
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_collection_id_roundtrip_always_works(uuid_str in "[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}") {
            let id = CollectionId::from_string(&uuid_str).unwrap();
            let roundtrip = CollectionId::from_string(&id.as_string()).unwrap();
            prop_assert_eq!(id, roundtrip);
        }
    }
}
