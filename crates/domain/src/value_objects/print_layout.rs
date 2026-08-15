//! PrintLayout Value Object
//!
//! Representa os diferentes layouts de impressão disponíveis.
//! Implementado usando TDD (Test-Driven Development).

use crate::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};

/// Layout de impressão
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum PrintLayout {
    /// Uma foto por página
    #[default]
    Single,
    /// Múltiplas fotos em grid (colunas x linhas)
    Multiple { columns: u8, rows: u8 },
    /// Contact sheet com número específico de fotos por página
    ContactSheet { photos_per_page: u8 },
}

impl PrintLayout {
    /// Número mínimo de fotos por página em contact sheet
    pub const MIN_PHOTOS_PER_PAGE: u8 = 1;
    /// Número máximo de fotos por página em contact sheet
    pub const MAX_PHOTOS_PER_PAGE: u8 = 100;

    /// Cria um layout Multiple com validação
    pub fn multiple(columns: u8, rows: u8) -> DomainResult<Self> {
        if columns == 0 || rows == 0 {
            return Err(DomainError::InvalidPrintSettings(
                "Grid dimensions must be greater than 0".to_string(),
            ));
        }

        if columns > 10 || rows > 10 {
            return Err(DomainError::InvalidPrintSettings(
                "Grid dimensions must be 10 or less".to_string(),
            ));
        }

        Ok(PrintLayout::Multiple { columns, rows })
    }

    /// Cria um layout ContactSheet com validação
    pub fn contact_sheet(photos_per_page: u8) -> DomainResult<Self> {
        if !(Self::MIN_PHOTOS_PER_PAGE..=Self::MAX_PHOTOS_PER_PAGE).contains(&photos_per_page)
        {
            return Err(DomainError::InvalidPrintSettings(format!(
                "Photos per page must be between {} and {}",
                Self::MIN_PHOTOS_PER_PAGE,
                Self::MAX_PHOTOS_PER_PAGE
            )));
        }

        Ok(PrintLayout::ContactSheet { photos_per_page })
    }

    /// Retorna o número total de fotos que cabem em uma página
    pub fn photos_per_page(&self) -> u8 {
        match self {
            PrintLayout::Single => 1,
            PrintLayout::Multiple { columns, rows } => columns * rows,
            PrintLayout::ContactSheet { photos_per_page } => *photos_per_page,
        }
    }

    /// Retorna as dimensões do grid (colunas, linhas) se aplicável
    pub fn grid_dimensions(&self) -> Option<(u8, u8)> {
        match self {
            PrintLayout::Single => Some((1, 1)),
            PrintLayout::Multiple { columns, rows } => Some((*columns, *rows)),
            PrintLayout::ContactSheet { .. } => None,
        }
    }

    /// Verifica se o layout requer metadata (contact sheet)
    pub fn requires_metadata(&self) -> bool {
        matches!(self, PrintLayout::ContactSheet { .. })
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    // 🔴 RED -> 🟢 GREEN -> 🔵 REFACTOR

    #[test]
    fn test_single_layout() {
        let layout = PrintLayout::Single;
        assert_eq!(layout.photos_per_page(), 1);
        assert_eq!(layout.grid_dimensions(), Some((1, 1)));
        assert!(!layout.requires_metadata());
    }

    #[test]
    fn test_multiple_layout_creation_with_valid_dimensions() {
        let layout = PrintLayout::multiple(2, 2);
        assert!(layout.is_ok());
        let layout = layout.unwrap();
        assert_eq!(layout.photos_per_page(), 4);
        assert_eq!(layout.grid_dimensions(), Some((2, 2)));
        assert!(!layout.requires_metadata());

        let layout = PrintLayout::multiple(3, 4).unwrap();
        assert_eq!(layout.photos_per_page(), 12);
        assert_eq!(layout.grid_dimensions(), Some((3, 4)));
    }

    #[test]
    fn test_multiple_layout_creation_with_zero_dimensions() {
        let layout = PrintLayout::multiple(0, 2);
        assert!(layout.is_err());

        let layout = PrintLayout::multiple(2, 0);
        assert!(layout.is_err());

        let layout = PrintLayout::multiple(0, 0);
        assert!(layout.is_err());
    }

    #[test]
    fn test_multiple_layout_creation_with_too_large_dimensions() {
        let layout = PrintLayout::multiple(11, 2);
        assert!(layout.is_err());

        let layout = PrintLayout::multiple(2, 11);
        assert!(layout.is_err());

        let layout = PrintLayout::multiple(15, 15);
        assert!(layout.is_err());
    }

    #[test]
    fn test_contact_sheet_creation_with_valid_values() {
        let layout = PrintLayout::contact_sheet(12);
        assert!(layout.is_ok());
        let layout = layout.unwrap();
        assert_eq!(layout.photos_per_page(), 12);
        assert_eq!(layout.grid_dimensions(), None);
        assert!(layout.requires_metadata());

        let layout = PrintLayout::contact_sheet(1).unwrap();
        assert_eq!(layout.photos_per_page(), 1);

        let layout = PrintLayout::contact_sheet(100).unwrap();
        assert_eq!(layout.photos_per_page(), 100);
    }

    #[test]
    fn test_contact_sheet_creation_with_invalid_values() {
        let layout = PrintLayout::contact_sheet(0);
        assert!(layout.is_err());

        let layout = PrintLayout::contact_sheet(101);
        assert!(layout.is_err());

        let layout = PrintLayout::contact_sheet(255);
        assert!(layout.is_err());
    }

    #[test]
    fn test_default_layout() {
        let layout = PrintLayout::default();
        assert_eq!(layout, PrintLayout::Single);
    }

    #[test]
    fn test_layout_constants() {
        assert_eq!(PrintLayout::MIN_PHOTOS_PER_PAGE, 1);
        assert_eq!(PrintLayout::MAX_PHOTOS_PER_PAGE, 100);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_valid_multiple_layout_always_succeeds(
            columns in 1u8..=10,
            rows in 1u8..=10
        ) {
            let layout = PrintLayout::multiple(columns, rows);
            prop_assert!(layout.is_ok());
            let layout = layout.unwrap();
            prop_assert_eq!(layout.photos_per_page(), columns * rows);
        }

        #[test]
        fn test_invalid_multiple_layout_always_fails(
            columns in 11u8..=20,
            rows in 11u8..=20
        ) {
            let layout = PrintLayout::multiple(columns, rows);
            prop_assert!(layout.is_err());
        }

        #[test]
        fn test_valid_contact_sheet_always_succeeds(photos in 1u8..=100) {
            let layout = PrintLayout::contact_sheet(photos);
            prop_assert!(layout.is_ok());
            let layout = layout.unwrap();
            prop_assert_eq!(layout.photos_per_page(), photos);
            prop_assert!(layout.requires_metadata());
        }

        #[test]
        fn test_invalid_contact_sheet_always_fails(photos in 101u8..=200) {
            let layout = PrintLayout::contact_sheet(photos);
            prop_assert!(layout.is_err());
        }

        #[test]
        fn test_photos_per_page_matches_grid(
            columns in 1u8..=10,
            rows in 1u8..=10
        ) {
            let layout = PrintLayout::multiple(columns, rows).unwrap();
            let expected = (columns as u16) * (rows as u16);
            prop_assert_eq!(layout.photos_per_page() as u16, expected);
        }
    }
}
