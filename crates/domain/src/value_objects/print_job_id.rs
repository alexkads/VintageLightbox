//! PrintJobId Value Object
//!
//! Identificador único para um trabalho de impressão.
//! Implementado usando TDD (Test-Driven Development).

use crate::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// ID único de um trabalho de impressão
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrintJobId(Uuid);

impl PrintJobId {
    /// Cria um novo PrintJobId aleatório
    pub fn new() -> Self {
        PrintJobId(Uuid::new_v4())
    }

    /// Cria um PrintJobId a partir de uma string UUID
    pub fn from_string(s: &str) -> DomainResult<Self> {
        Uuid::parse_str(s)
            .map(PrintJobId)
            .map_err(|_| DomainError::InvalidId {
                id: s.to_string(),
                reason: "Invalid UUID format".to_string(),
            })
    }

    /// Retorna o valor interno do UUID
    pub fn value(&self) -> Uuid {
        self.0
    }


}

impl Default for PrintJobId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PrintJobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for PrintJobId {
    fn from(uuid: Uuid) -> Self {
        PrintJobId(uuid)
    }
}

impl From<PrintJobId> for Uuid {
    fn from(id: PrintJobId) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 🔴 RED -> 🟢 GREEN -> 🔵 REFACTOR

    #[test]
    fn test_print_job_id_creation() {
        let id1 = PrintJobId::new();
        let id2 = PrintJobId::new();

        // IDs devem ser diferentes
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_print_job_id_from_valid_string() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let id = PrintJobId::from_string(uuid_str);
        assert!(id.is_ok());
        assert_eq!(id.unwrap().to_string(), uuid_str);
    }

    #[test]
    fn test_print_job_id_from_invalid_string() {
        let invalid_str = "not-a-uuid";
        let id = PrintJobId::from_string(invalid_str);
        assert!(id.is_err());
    }

    #[test]
    fn test_print_job_id_to_string() {
        let id = PrintJobId::new();
        let s = id.to_string();
        assert!(!s.is_empty());
        assert_eq!(s.len(), 36); // UUID string length
    }

    #[test]
    fn test_print_job_id_display() {
        let id = PrintJobId::new();
        let display = format!("{}", id);
        assert_eq!(display, id.to_string());
    }

    #[test]
    fn test_print_job_id_from_uuid() {
        let uuid = Uuid::new_v4();
        let id: PrintJobId = uuid.into();
        assert_eq!(id.value(), uuid);
    }

    #[test]
    fn test_print_job_id_into_uuid() {
        let id = PrintJobId::new();
        let uuid: Uuid = id.into();
        assert_eq!(uuid, id.value());
    }

    #[test]
    fn test_print_job_id_equality() {
        let uuid = Uuid::new_v4();
        let id1 = PrintJobId::from(uuid);
        let id2 = PrintJobId::from(uuid);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_print_job_id_default() {
        let id = PrintJobId::default();
        assert!(!id.to_string().is_empty());
    }
}
