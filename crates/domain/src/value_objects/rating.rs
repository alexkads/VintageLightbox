//! Rating Value Object
//!
//! Representa a classificação de uma foto de 0 a 5 estrelas.
//! Implementado usando TDD (Test-Driven Development).

use crate::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};

/// Rating de uma foto (0-5 estrelas)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Rating(u8);

impl Rating {
    /// Valor mínimo para rating
    pub const MIN: u8 = 0;
    /// Valor máximo para rating
    pub const MAX: u8 = 5;

    /// Rating sem estrelas
    pub const ZERO: Rating = Rating(0);
    /// Rating com 1 estrela
    pub const ONE: Rating = Rating(1);
    /// Rating com 2 estrelas
    pub const TWO: Rating = Rating(2);
    /// Rating com 3 estrelas
    pub const THREE: Rating = Rating(3);
    /// Rating com 4 estrelas
    pub const FOUR: Rating = Rating(4);
    /// Rating com 5 estrelas
    pub const FIVE: Rating = Rating(5);

    /// Cria um novo Rating se o valor for válido (0-5)
    pub fn new(value: u8) -> DomainResult<Self> {
        if value <= Self::MAX {
            Ok(Rating(value))
        } else {
            Err(DomainError::InvalidRating)
        }
    }

    /// Retorna o valor do rating
    pub fn value(&self) -> u8 {
        self.0
    }

    /// Verifica se o rating é válido
    pub fn is_valid(&self) -> bool {
        self.0 <= Self::MAX
    }
}

impl TryFrom<u8> for Rating {
    type Error = DomainError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Rating> for u8 {
    fn from(rating: Rating) -> Self {
        rating.0
    }
}

impl Default for Rating {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 🔴 RED -> 🟢 GREEN -> 🔵 REFACTOR

    #[test]
    fn test_rating_creation_with_valid_values() {
        // Arrange & Act
        let rating_0 = Rating::new(0);
        let rating_3 = Rating::new(3);
        let rating_5 = Rating::new(5);

        // Assert
        assert!(rating_0.is_ok());
        assert!(rating_3.is_ok());
        assert!(rating_5.is_ok());
        assert_eq!(rating_0.unwrap().value(), 0);
        assert_eq!(rating_3.unwrap().value(), 3);
        assert_eq!(rating_5.unwrap().value(), 5);
    }

    #[test]
    fn test_rating_creation_with_invalid_values() {
        // Arrange & Act
        let rating_6 = Rating::new(6);
        let rating_10 = Rating::new(10);
        let rating_255 = Rating::new(255);

        // Assert
        assert!(rating_6.is_err());
        assert!(rating_10.is_err());
        assert!(rating_255.is_err());
        assert_eq!(rating_6.unwrap_err(), DomainError::InvalidRating);
    }

    #[test]
    fn test_rating_constants() {
        assert_eq!(Rating::ZERO.value(), 0);
        assert_eq!(Rating::ONE.value(), 1);
        assert_eq!(Rating::TWO.value(), 2);
        assert_eq!(Rating::THREE.value(), 3);
        assert_eq!(Rating::FOUR.value(), 4);
        assert_eq!(Rating::FIVE.value(), 5);
    }

    #[test]
    fn test_rating_try_from_u8() {
        let rating: Result<Rating, _> = 3u8.try_into();
        assert!(rating.is_ok());
        assert_eq!(rating.unwrap().value(), 3);

        let invalid_rating: Result<Rating, _> = 6u8.try_into();
        assert!(invalid_rating.is_err());
    }

    #[test]
    fn test_rating_into_u8() {
        let rating = Rating::FOUR;
        let value: u8 = rating.into();
        assert_eq!(value, 4);
    }

    #[test]
    fn test_rating_default() {
        let rating = Rating::default();
        assert_eq!(rating.value(), 0);
    }

    #[test]
    fn test_rating_comparison() {
        assert!(Rating::ONE < Rating::THREE);
        assert!(Rating::FIVE > Rating::TWO);
        assert_eq!(Rating::THREE, Rating::new(3).unwrap());
    }

    #[test]
    fn test_rating_is_valid() {
        assert!(Rating::ZERO.is_valid());
        assert!(Rating::FIVE.is_valid());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_valid_ratings_always_succeed(value in 0u8..=5) {
            let rating = Rating::new(value);
            prop_assert!(rating.is_ok());
        }

        #[test]
        fn test_invalid_ratings_always_fail(value in 6u8..=255) {
            let rating = Rating::new(value);
            prop_assert!(rating.is_err());
        }

        #[test]
        fn test_rating_roundtrip(value in 0u8..=5) {
            let rating = Rating::new(value).unwrap();
            let extracted: u8 = rating.into();
            prop_assert_eq!(extracted, value);
        }

        #[test]
        fn test_rating_comparison_is_consistent(a in 0u8..=5, b in 0u8..=5) {
            let rating_a = Rating::new(a).unwrap();
            let rating_b = Rating::new(b).unwrap();
            
            prop_assert_eq!(rating_a < rating_b, a < b);
            prop_assert_eq!(rating_a > rating_b, a > b);
            prop_assert_eq!(rating_a == rating_b, a == b);
        }
    }
}
