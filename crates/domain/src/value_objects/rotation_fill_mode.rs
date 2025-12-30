//! RotationFillMode Value Object
//!
//! Representa como as áreas vazias são preenchidas quando a imagem é rotacionada
//! por um ângulo não múltiplo de 90°.

use crate::{DomainError, DomainResult};

/// Modo de preenchimento para áreas vazias na rotação
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum RotationFillMode {
    /// Preenche com preto (padrão)
    #[default]
    Black,
    /// Preenche com branco
    White,
    /// Preenche com transparência (para exportação PNG)
    Transparent,
    /// Content-aware fill inteligente (futuro)
    Intelligent,
    /// Reduz o crop para caber na área rotacionada sem cantos vazios
    ShrinkToFit,
}

impl RotationFillMode {
    /// Retorna todas as opções disponíveis
    pub fn all() -> &'static [RotationFillMode] {
        &[
            RotationFillMode::Black,
            RotationFillMode::White,
            RotationFillMode::Transparent,
            RotationFillMode::Intelligent,
            RotationFillMode::ShrinkToFit,
        ]
    }

    /// Retorna o nome do modo
    pub fn name(&self) -> &'static str {
        match self {
            RotationFillMode::Black => "black",
            RotationFillMode::White => "white",
            RotationFillMode::Transparent => "transparent",
            RotationFillMode::Intelligent => "intelligent",
            RotationFillMode::ShrinkToFit => "shrink_to_fit",
        }
    }

    /// Retorna o nome de exibição (para UI)
    pub fn display_name(&self) -> &'static str {
        match self {
            RotationFillMode::Black => "Black",
            RotationFillMode::White => "White",
            RotationFillMode::Transparent => "Transparent",
            RotationFillMode::Intelligent => "Intelligent",
            RotationFillMode::ShrinkToFit => "Shrink to Fit",
        }
    }

    /// Cria RotationFillMode a partir do nome
    pub fn from_name(name: &str) -> DomainResult<Self> {
        match name.to_lowercase().as_str() {
            "black" => Ok(RotationFillMode::Black),
            "white" => Ok(RotationFillMode::White),
            "transparent" => Ok(RotationFillMode::Transparent),
            "intelligent" => Ok(RotationFillMode::Intelligent),
            "shrink_to_fit" | "shrinktofit" => Ok(RotationFillMode::ShrinkToFit),
            _ => Err(DomainError::InvalidOperation(format!("Invalid fill mode: {}", name))),
        }
    }

    /// Retorna o código numérico (para persistência)
    pub fn as_code(&self) -> u8 {
        match self {
            RotationFillMode::Black => 0,
            RotationFillMode::White => 1,
            RotationFillMode::Transparent => 2,
            RotationFillMode::Intelligent => 3,
            RotationFillMode::ShrinkToFit => 4,
        }
    }

    /// Cria RotationFillMode a partir do código numérico
    pub fn from_code(code: u8) -> DomainResult<Self> {
        match code {
            0 => Ok(RotationFillMode::Black),
            1 => Ok(RotationFillMode::White),
            2 => Ok(RotationFillMode::Transparent),
            3 => Ok(RotationFillMode::Intelligent),
            4 => Ok(RotationFillMode::ShrinkToFit),
            _ => Err(DomainError::InvalidOperation(format!("Invalid fill mode code: {}", code))),
        }
    }
}

impl std::fmt::Display for RotationFillMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl TryFrom<u8> for RotationFillMode {
    type Error = DomainError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::from_code(value)
    }
}

impl From<RotationFillMode> for u8 {
    fn from(mode: RotationFillMode) -> Self {
        mode.as_code()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotation_fill_mode_all() {
        let all = RotationFillMode::all();
        assert_eq!(all.len(), 5);
        assert!(all.contains(&RotationFillMode::Black));
        assert!(all.contains(&RotationFillMode::White));
        assert!(all.contains(&RotationFillMode::Transparent));
        assert!(all.contains(&RotationFillMode::Intelligent));
        assert!(all.contains(&RotationFillMode::ShrinkToFit));
    }

    #[test]
    fn test_rotation_fill_mode_default() {
        let mode = RotationFillMode::default();
        assert_eq!(mode, RotationFillMode::Black);
    }

    #[test]
    fn test_rotation_fill_mode_name() {
        assert_eq!(RotationFillMode::Black.name(), "black");
        assert_eq!(RotationFillMode::White.name(), "white");
        assert_eq!(RotationFillMode::Transparent.name(), "transparent");
        assert_eq!(RotationFillMode::Intelligent.name(), "intelligent");
        assert_eq!(RotationFillMode::ShrinkToFit.name(), "shrink_to_fit");
    }

    #[test]
    fn test_rotation_fill_mode_display_name() {
        assert_eq!(RotationFillMode::Black.display_name(), "Black");
        assert_eq!(RotationFillMode::ShrinkToFit.display_name(), "Shrink to Fit");
    }

    #[test]
    fn test_rotation_fill_mode_from_valid_name() {
        assert_eq!(RotationFillMode::from_name("black").unwrap(), RotationFillMode::Black);
        assert_eq!(RotationFillMode::from_name("WHITE").unwrap(), RotationFillMode::White);
        assert_eq!(RotationFillMode::from_name("shrink_to_fit").unwrap(), RotationFillMode::ShrinkToFit);
        assert_eq!(RotationFillMode::from_name("shrinktofit").unwrap(), RotationFillMode::ShrinkToFit);
    }

    #[test]
    fn test_rotation_fill_mode_from_invalid_name() {
        assert!(RotationFillMode::from_name("invalid").is_err());
        assert!(RotationFillMode::from_name("").is_err());
    }

    #[test]
    fn test_rotation_fill_mode_as_code() {
        assert_eq!(RotationFillMode::Black.as_code(), 0);
        assert_eq!(RotationFillMode::White.as_code(), 1);
        assert_eq!(RotationFillMode::Transparent.as_code(), 2);
        assert_eq!(RotationFillMode::Intelligent.as_code(), 3);
        assert_eq!(RotationFillMode::ShrinkToFit.as_code(), 4);
    }

    #[test]
    fn test_rotation_fill_mode_from_valid_code() {
        assert_eq!(RotationFillMode::from_code(0).unwrap(), RotationFillMode::Black);
        assert_eq!(RotationFillMode::from_code(1).unwrap(), RotationFillMode::White);
        assert_eq!(RotationFillMode::from_code(2).unwrap(), RotationFillMode::Transparent);
        assert_eq!(RotationFillMode::from_code(3).unwrap(), RotationFillMode::Intelligent);
        assert_eq!(RotationFillMode::from_code(4).unwrap(), RotationFillMode::ShrinkToFit);
    }

    #[test]
    fn test_rotation_fill_mode_from_invalid_code() {
        assert!(RotationFillMode::from_code(5).is_err());
        assert!(RotationFillMode::from_code(255).is_err());
    }

    #[test]
    fn test_rotation_fill_mode_display() {
        assert_eq!(format!("{}", RotationFillMode::Black), "Black");
        assert_eq!(format!("{}", RotationFillMode::ShrinkToFit), "Shrink to Fit");
    }

    #[test]
    fn test_rotation_fill_mode_roundtrip_code() {
        for mode in RotationFillMode::all() {
            let code = mode.as_code();
            let recovered = RotationFillMode::from_code(code).unwrap();
            assert_eq!(*mode, recovered);
        }
    }

    #[test]
    fn test_rotation_fill_mode_roundtrip_name() {
        for mode in RotationFillMode::all() {
            let name = mode.name();
            let recovered = RotationFillMode::from_name(name).unwrap();
            assert_eq!(*mode, recovered);
        }
    }
}
