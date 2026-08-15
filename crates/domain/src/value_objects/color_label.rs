//! ColorLabel Value Object
//!
//! Representa etiquetas de cor para classificação visual de fotos.
//! Similar ao sistema usado em Lightroom.

use crate::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};

/// Etiqueta de cor para classificação de fotos
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColorLabel {
    /// Vermelho
    Red,
    /// Amarelo
    Yellow,
    /// Verde
    Green,
    /// Azul
    Blue,
    /// Roxo
    Purple,
}

impl ColorLabel {
    /// Retorna todas as cores disponíveis
    pub fn all() -> &'static [ColorLabel] {
        &[
            ColorLabel::Red,
            ColorLabel::Yellow,
            ColorLabel::Green,
            ColorLabel::Blue,
            ColorLabel::Purple,
        ]
    }

    /// Retorna o nome da cor
    pub fn name(&self) -> &'static str {
        match self {
            ColorLabel::Red => "red",
            ColorLabel::Yellow => "yellow",
            ColorLabel::Green => "green",
            ColorLabel::Blue => "blue",
            ColorLabel::Purple => "purple",
        }
    }

    /// Cria ColorLabel a partir do nome
    pub fn from_name(name: &str) -> DomainResult<Self> {
        match name.to_lowercase().as_str() {
            "red" => Ok(ColorLabel::Red),
            "yellow" => Ok(ColorLabel::Yellow),
            "green" => Ok(ColorLabel::Green),
            "blue" => Ok(ColorLabel::Blue),
            "purple" => Ok(ColorLabel::Purple),
            _ => Err(DomainError::InvalidColorLabel),
        }
    }

    /// Retorna o código numérico da cor (para persistência)
    pub fn as_code(&self) -> u8 {
        match self {
            ColorLabel::Red => 1,
            ColorLabel::Yellow => 2,
            ColorLabel::Green => 3,
            ColorLabel::Blue => 4,
            ColorLabel::Purple => 5,
        }
    }

    /// Cria ColorLabel a partir do código numérico
    pub fn from_code(code: u8) -> DomainResult<Self> {
        match code {
            1 => Ok(ColorLabel::Red),
            2 => Ok(ColorLabel::Yellow),
            3 => Ok(ColorLabel::Green),
            4 => Ok(ColorLabel::Blue),
            5 => Ok(ColorLabel::Purple),
            _ => Err(DomainError::InvalidColorLabel),
        }
    }
}

impl std::fmt::Display for ColorLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl TryFrom<u8> for ColorLabel {
    type Error = DomainError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::from_code(value)
    }
}

impl From<ColorLabel> for u8 {
    fn from(label: ColorLabel) -> Self {
        label.as_code()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_label_all() {
        // Act
        let all = ColorLabel::all();

        // Assert
        assert_eq!(all.len(), 5);
        assert!(all.contains(&ColorLabel::Red));
        assert!(all.contains(&ColorLabel::Yellow));
        assert!(all.contains(&ColorLabel::Green));
        assert!(all.contains(&ColorLabel::Blue));
        assert!(all.contains(&ColorLabel::Purple));
    }

    #[test]
    fn test_color_label_name() {
        assert_eq!(ColorLabel::Red.name(), "red");
        assert_eq!(ColorLabel::Yellow.name(), "yellow");
        assert_eq!(ColorLabel::Green.name(), "green");
        assert_eq!(ColorLabel::Blue.name(), "blue");
        assert_eq!(ColorLabel::Purple.name(), "purple");
    }

    #[test]
    fn test_color_label_from_valid_name() {
        assert_eq!(ColorLabel::from_name("red").unwrap(), ColorLabel::Red);
        assert_eq!(ColorLabel::from_name("RED").unwrap(), ColorLabel::Red);
        assert_eq!(ColorLabel::from_name("Yellow").unwrap(), ColorLabel::Yellow);
        assert_eq!(ColorLabel::from_name("green").unwrap(), ColorLabel::Green);
    }

    #[test]
    fn test_color_label_from_invalid_name() {
        assert!(ColorLabel::from_name("pink").is_err());
        assert!(ColorLabel::from_name("").is_err());
        assert!(ColorLabel::from_name("notacolor").is_err());

        let result = ColorLabel::from_name("invalid");
        assert_eq!(result.unwrap_err(), DomainError::InvalidColorLabel);
    }

    #[test]
    fn test_color_label_as_code() {
        assert_eq!(ColorLabel::Red.as_code(), 1);
        assert_eq!(ColorLabel::Yellow.as_code(), 2);
        assert_eq!(ColorLabel::Green.as_code(), 3);
        assert_eq!(ColorLabel::Blue.as_code(), 4);
        assert_eq!(ColorLabel::Purple.as_code(), 5);
    }

    #[test]
    fn test_color_label_from_valid_code() {
        assert_eq!(ColorLabel::from_code(1).unwrap(), ColorLabel::Red);
        assert_eq!(ColorLabel::from_code(2).unwrap(), ColorLabel::Yellow);
        assert_eq!(ColorLabel::from_code(3).unwrap(), ColorLabel::Green);
        assert_eq!(ColorLabel::from_code(4).unwrap(), ColorLabel::Blue);
        assert_eq!(ColorLabel::from_code(5).unwrap(), ColorLabel::Purple);
    }

    #[test]
    fn test_color_label_from_invalid_code() {
        assert!(ColorLabel::from_code(0).is_err());
        assert!(ColorLabel::from_code(6).is_err());
        assert!(ColorLabel::from_code(255).is_err());
    }

    #[test]
    fn test_color_label_display() {
        assert_eq!(format!("{}", ColorLabel::Red), "red");
        assert_eq!(format!("{}", ColorLabel::Blue), "blue");
    }

    #[test]
    fn test_color_label_try_from_u8() {
        let label: Result<ColorLabel, _> = 3u8.try_into();
        assert!(label.is_ok());
        assert_eq!(label.unwrap(), ColorLabel::Green);

        let invalid: Result<ColorLabel, _> = 10u8.try_into();
        assert!(invalid.is_err());
    }

    #[test]
    fn test_color_label_into_u8() {
        let code: u8 = ColorLabel::Purple.into();
        assert_eq!(code, 5);
    }

    #[test]
    fn test_color_label_roundtrip_code() {
        for label in ColorLabel::all() {
            let code = label.as_code();
            let recovered = ColorLabel::from_code(code).unwrap();
            assert_eq!(*label, recovered);
        }
    }

    #[test]
    fn test_color_label_roundtrip_name() {
        for label in ColorLabel::all() {
            let name = label.name();
            let recovered = ColorLabel::from_name(name).unwrap();
            assert_eq!(*label, recovered);
        }
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_valid_codes_always_succeed(code in 1u8..=5u8) {
            let label = ColorLabel::from_code(code);
            prop_assert!(label.is_ok());
        }

        #[test]
        fn test_invalid_codes_always_fail(code in 6u8..=255u8) {
            let label = ColorLabel::from_code(code);
            prop_assert!(label.is_err());
        }
    }
}
