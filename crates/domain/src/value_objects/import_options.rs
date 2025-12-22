//! Import Options Value Objects
//!
//! Configurações para importação avançada de fotos.

use serde::{Deserialize, Serialize};

/// Opções de importação
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportOptions {
    /// Estratégia de organização de arquivos
    pub organization: OrganizationStrategy,
    /// Padrão de renomeação de arquivos
    pub rename_pattern: RenamePattern,
    /// Se deve pular duplicatas automaticamente
    pub skip_duplicates: bool,
}

impl ImportOptions {
    /// Cria novas opções de importação com valores padrão
    pub fn new() -> Self {
        Self::default()
    }

    /// Cria opções de importação customizadas
    pub fn with(
        organization: OrganizationStrategy,
        rename_pattern: RenamePattern,
        skip_duplicates: bool,
    ) -> Self {
        Self {
            organization,
            rename_pattern,
            skip_duplicates,
        }
    }
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            organization: OrganizationStrategy::ByDate,
            rename_pattern: RenamePattern::Standard,
            skip_duplicates: true,
        }
    }
}

/// Estratégia de organização de arquivos
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationStrategy {
    /// Organizar por data (YYYY/MM/DD)
    ByDate,
    /// Preservar estrutura de diretórios original
    PreserveStructure,
}

impl Default for OrganizationStrategy {
    fn default() -> Self {
        Self::ByDate
    }
}

/// Padrão de renomeação de arquivos
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenamePattern {
    /// Padrão: photo-YYYY-MM-DD-NNN.ext
    Standard,
    /// Manter nome original do arquivo
    KeepOriginal,
    /// Padrão customizado (v2 feature)
    Custom(String),
}

impl Default for RenamePattern {
    fn default() -> Self {
        Self::Standard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_options_default() {
        let options = ImportOptions::default();
        assert_eq!(options.organization, OrganizationStrategy::ByDate);
        assert_eq!(options.rename_pattern, RenamePattern::Standard);
        assert!(options.skip_duplicates);
    }

    #[test]
    fn test_import_options_new() {
        let options = ImportOptions::new();
        assert_eq!(options, ImportOptions::default());
    }

    #[test]
    fn test_import_options_with() {
        let options = ImportOptions::with(
            OrganizationStrategy::PreserveStructure,
            RenamePattern::KeepOriginal,
            false,
        );
        assert_eq!(options.organization, OrganizationStrategy::PreserveStructure);
        assert_eq!(options.rename_pattern, RenamePattern::KeepOriginal);
        assert!(!options.skip_duplicates);
    }

    #[test]
    fn test_organization_strategy_equality() {
        assert_eq!(OrganizationStrategy::ByDate, OrganizationStrategy::ByDate);
        assert_ne!(
            OrganizationStrategy::ByDate,
            OrganizationStrategy::PreserveStructure
        );
    }

    #[test]
    fn test_rename_pattern_equality() {
        assert_eq!(RenamePattern::Standard, RenamePattern::Standard);
        assert_eq!(RenamePattern::KeepOriginal, RenamePattern::KeepOriginal);
        assert_ne!(RenamePattern::Standard, RenamePattern::KeepOriginal);

        let custom1 = RenamePattern::Custom("pattern1".to_string());
        let custom2 = RenamePattern::Custom("pattern1".to_string());
        let custom3 = RenamePattern::Custom("pattern2".to_string());
        assert_eq!(custom1, custom2);
        assert_ne!(custom1, custom3);
    }
}
