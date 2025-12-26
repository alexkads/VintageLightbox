//! PrintSettings Value Object
//!
//! Representa as configurações de impressão para uma foto.
//! Implementado usando TDD (Test-Driven Development).

use crate::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};

/// Tamanho do papel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperSize {
    /// A4 (210mm x 297mm)
    A4,
    /// Letter (8.5" x 11")
    Letter,
    /// Custom size (width_mm, height_mm)
    Custom { width_mm: u16, height_mm: u16 },
}

impl PaperSize {
    /// Retorna as dimensões em milímetros (largura, altura)
    pub fn dimensions_mm(&self) -> (u16, u16) {
        match self {
            PaperSize::A4 => (210, 297),
            PaperSize::Letter => (216, 279), // 8.5" x 11" = 215.9mm x 279.4mm
            PaperSize::Custom { width_mm, height_mm } => (*width_mm, *height_mm),
        }
    }
}

/// Orientação da página
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// Modo de cor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMode {
    Color,
    Grayscale,
}

/// Margens da página em milímetros
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Margins {
    pub top: u16,
    pub bottom: u16,
    pub left: u16,
    pub right: u16,
}

impl Margins {
    /// Cria novas margens com validação
    pub fn new(top: u16, bottom: u16, left: u16, right: u16) -> DomainResult<Self> {
        // Margens devem ser razoáveis (< 100mm)
        if top > 100 || bottom > 100 || left > 100 || right > 100 {
            return Err(DomainError::InvalidPrintSettings(
                "Margins must be less than 100mm".to_string(),
            ));
        }
        
        Ok(Margins {
            top,
            bottom,
            left,
            right,
        })
    }

    /// Margens padrão (10mm em todos os lados)
    pub fn default_margins() -> Self {
        Margins {
            top: 10,
            bottom: 10,
            left: 10,
            right: 10,
        }
    }
}

/// Configurações de impressão
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintSettings {
    paper_size: PaperSize,
    orientation: Orientation,
    margins: Margins,
    dpi: u16,
    color_mode: ColorMode,
}

impl PrintSettings {
    /// DPI mínimo permitido
    pub const MIN_DPI: u16 = 72;
    /// DPI máximo permitido
    pub const MAX_DPI: u16 = 1200;
    /// DPI padrão
    pub const DEFAULT_DPI: u16 = 300;

    /// Cria novas configurações de impressão com validação
    pub fn new(
        paper_size: PaperSize,
        orientation: Orientation,
        margins: Margins,
        dpi: u16,
        color_mode: ColorMode,
    ) -> DomainResult<Self> {
        // Valida DPI
        if dpi < Self::MIN_DPI || dpi > Self::MAX_DPI {
            return Err(DomainError::InvalidPrintSettings(format!(
                "DPI must be between {} and {}",
                Self::MIN_DPI,
                Self::MAX_DPI
            )));
        }

        Ok(PrintSettings {
            paper_size,
            orientation,
            margins,
            dpi,
            color_mode,
        })
    }

    /// Cria configurações padrão
    pub fn default_settings() -> Self {
        PrintSettings {
            paper_size: PaperSize::A4,
            orientation: Orientation::Portrait,
            margins: Margins::default_margins(),
            dpi: Self::DEFAULT_DPI,
            color_mode: ColorMode::Color,
        }
    }

    // Getters
    pub fn paper_size(&self) -> PaperSize {
        self.paper_size
    }

    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    pub fn margins(&self) -> Margins {
        self.margins
    }

    pub fn dpi(&self) -> u16 {
        self.dpi
    }

    pub fn color_mode(&self) -> ColorMode {
        self.color_mode
    }

    /// Retorna as dimensões da área imprimível em milímetros
    pub fn printable_area_mm(&self) -> (u16, u16) {
        let (width, height) = self.paper_size.dimensions_mm();
        let (width, height) = match self.orientation {
            Orientation::Portrait => (width, height),
            Orientation::Landscape => (height, width),
        };

        let printable_width = width.saturating_sub(self.margins.left + self.margins.right);
        let printable_height = height.saturating_sub(self.margins.top + self.margins.bottom);

        (printable_width, printable_height)
    }
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self::default_settings()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 🔴 RED -> 🟢 GREEN -> 🔵 REFACTOR

    #[test]
    fn test_paper_size_dimensions() {
        assert_eq!(PaperSize::A4.dimensions_mm(), (210, 297));
        assert_eq!(PaperSize::Letter.dimensions_mm(), (216, 279));
        assert_eq!(
            PaperSize::Custom {
                width_mm: 100,
                height_mm: 150
            }
            .dimensions_mm(),
            (100, 150)
        );
    }

    #[test]
    fn test_margins_creation_with_valid_values() {
        let margins = Margins::new(10, 10, 10, 10);
        assert!(margins.is_ok());
        let margins = margins.unwrap();
        assert_eq!(margins.top, 10);
        assert_eq!(margins.bottom, 10);
        assert_eq!(margins.left, 10);
        assert_eq!(margins.right, 10);
    }

    #[test]
    fn test_margins_creation_with_invalid_values() {
        // Margens muito grandes
        let margins = Margins::new(150, 10, 10, 10);
        assert!(margins.is_err());

        let margins = Margins::new(10, 150, 10, 10);
        assert!(margins.is_err());

        let margins = Margins::new(10, 10, 150, 10);
        assert!(margins.is_err());

        let margins = Margins::new(10, 10, 10, 150);
        assert!(margins.is_err());
    }

    #[test]
    fn test_margins_default() {
        let margins = Margins::default_margins();
        assert_eq!(margins.top, 10);
        assert_eq!(margins.bottom, 10);
        assert_eq!(margins.left, 10);
        assert_eq!(margins.right, 10);
    }

    #[test]
    fn test_print_settings_creation_with_valid_values() {
        let settings = PrintSettings::new(
            PaperSize::A4,
            Orientation::Portrait,
            Margins::default_margins(),
            300,
            ColorMode::Color,
        );
        assert!(settings.is_ok());
        let settings = settings.unwrap();
        assert_eq!(settings.dpi(), 300);
        assert_eq!(settings.paper_size(), PaperSize::A4);
    }

    #[test]
    fn test_print_settings_creation_with_invalid_dpi() {
        // DPI muito baixo
        let settings = PrintSettings::new(
            PaperSize::A4,
            Orientation::Portrait,
            Margins::default_margins(),
            50,
            ColorMode::Color,
        );
        assert!(settings.is_err());

        // DPI muito alto
        let settings = PrintSettings::new(
            PaperSize::A4,
            Orientation::Portrait,
            Margins::default_margins(),
            2000,
            ColorMode::Color,
        );
        assert!(settings.is_err());
    }

    #[test]
    fn test_print_settings_default() {
        let settings = PrintSettings::default();
        assert_eq!(settings.paper_size(), PaperSize::A4);
        assert_eq!(settings.orientation(), Orientation::Portrait);
        assert_eq!(settings.dpi(), 300);
        assert_eq!(settings.color_mode(), ColorMode::Color);
    }

    #[test]
    fn test_printable_area_portrait() {
        let settings = PrintSettings::new(
            PaperSize::A4,
            Orientation::Portrait,
            Margins::new(10, 10, 10, 10).unwrap(),
            300,
            ColorMode::Color,
        )
        .unwrap();

        let (width, height) = settings.printable_area_mm();
        // A4 = 210x297, margins = 10mm each side
        assert_eq!(width, 190); // 210 - 20
        assert_eq!(height, 277); // 297 - 20
    }

    #[test]
    fn test_printable_area_landscape() {
        let settings = PrintSettings::new(
            PaperSize::A4,
            Orientation::Landscape,
            Margins::new(10, 10, 10, 10).unwrap(),
            300,
            ColorMode::Color,
        )
        .unwrap();

        let (width, height) = settings.printable_area_mm();
        // A4 landscape = 297x210, margins = 10mm each side
        assert_eq!(width, 277); // 297 - 20
        assert_eq!(height, 190); // 210 - 20
    }

    #[test]
    fn test_dpi_constants() {
        assert_eq!(PrintSettings::MIN_DPI, 72);
        assert_eq!(PrintSettings::MAX_DPI, 1200);
        assert_eq!(PrintSettings::DEFAULT_DPI, 300);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_valid_dpi_always_succeeds(dpi in 72u16..=1200) {
            let settings = PrintSettings::new(
                PaperSize::A4,
                Orientation::Portrait,
                Margins::default_margins(),
                dpi,
                ColorMode::Color,
            );
            prop_assert!(settings.is_ok());
        }

        #[test]
        fn test_invalid_dpi_always_fails(dpi in 1201u16..=2000) {
            let settings = PrintSettings::new(
                PaperSize::A4,
                Orientation::Portrait,
                Margins::default_margins(),
                dpi,
                ColorMode::Color,
            );
            prop_assert!(settings.is_err());
        }

        #[test]
        fn test_valid_margins_always_succeed(
            top in 0u16..=100,
            bottom in 0u16..=100,
            left in 0u16..=100,
            right in 0u16..=100
        ) {
            let margins = Margins::new(top, bottom, left, right);
            prop_assert!(margins.is_ok());
        }

        #[test]
        fn test_printable_area_never_exceeds_paper_size(
            dpi in 72u16..=1200,
            top in 0u16..=50,
            bottom in 0u16..=50,
            left in 0u16..=50,
            right in 0u16..=50
        ) {
            let margins = Margins::new(top, bottom, left, right).unwrap();
            let settings = PrintSettings::new(
                PaperSize::A4,
                Orientation::Portrait,
                margins,
                dpi,
                ColorMode::Color,
            ).unwrap();

            let (width, height) = settings.printable_area_mm();
            let (paper_width, paper_height) = PaperSize::A4.dimensions_mm();
            
            prop_assert!(width <= paper_width);
            prop_assert!(height <= paper_height);
        }
    }
}
