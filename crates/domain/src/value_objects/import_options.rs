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
    /// O que fazer com o arquivo original: catalogar onde está, copiar ou mover
    #[serde(default)]
    pub mode: ImportMode,
    /// Raiz de destino para `Copy`/`Move`. `None` usa o catálogo padrão.
    #[serde(default)]
    pub destination: Option<String>,
    /// Pasta de origem do lote — usada por `PreserveStructure` para saber o que preservar
    #[serde(default)]
    pub source_root: Option<String>,
    /// Se o scan da origem desce em subpastas
    #[serde(default = "default_true")]
    pub include_subfolders: bool,
}

fn default_true() -> bool {
    true
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
            ..Self::default()
        }
    }

    /// Define o modo de importação (Add/Copy/Move)
    pub fn with_mode(mut self, mode: ImportMode) -> Self {
        self.mode = mode;
        self
    }

    /// Define a raiz de destino para cópia/movimentação
    pub fn with_destination(mut self, destination: Option<String>) -> Self {
        self.destination = destination;
        self
    }

    /// Define a raiz de origem do lote (base para `PreserveStructure`)
    pub fn with_source_root(mut self, source_root: Option<String>) -> Self {
        self.source_root = source_root;
        self
    }

    /// `Add` cataloga o arquivo onde ele está — nada é copiado nem movido
    pub fn moves_files(&self) -> bool {
        !matches!(self.mode, ImportMode::Add)
    }
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            organization: OrganizationStrategy::ByDate,
            rename_pattern: RenamePattern::Standard,
            skip_duplicates: true,
            mode: ImportMode::Copy,
            destination: None,
            source_root: None,
            include_subfolders: true,
        }
    }
}

/// O que a importação faz com o arquivo original
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ImportMode {
    /// Cataloga o arquivo onde ele está, sem tocar no disco
    Add,
    /// Copia para o destino e mantém o original
    #[default]
    Copy,
    /// Copia para o destino e apaga o original
    Move,
}

impl ImportMode {
    /// Rótulo curto para exibição
    pub fn label(&self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Copy => "Copy",
            Self::Move => "Move",
        }
    }

    /// Explicação de uma linha, exibida abaixo do seletor
    pub fn description(&self) -> &'static str {
        match self {
            Self::Add => "Cataloga as fotos onde elas estão, sem copiar",
            Self::Copy => "Copia para o destino e mantém o original",
            Self::Move => "Copia para o destino e apaga o original",
        }
    }
}

/// Estratégia de organização de arquivos
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OrganizationStrategy {
    /// Organizar por data (YYYY/MM/DD)
    #[default]
    ByDate,
    /// Preservar a hierarquia de subpastas da origem
    PreserveStructure,
    /// Jogar todos os arquivos numa pasta só
    IntoOneFolder,
}

impl OrganizationStrategy {
    /// Rótulo curto para exibição
    pub fn label(&self) -> &'static str {
        match self {
            Self::ByDate => "Por data (YYYY/MM/DD)",
            Self::PreserveStructure => "Preservar subpastas",
            Self::IntoOneFolder => "Numa pasta só",
        }
    }
}

/// Padrão de renomeação de arquivos
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RenamePattern {
    /// Padrão: photo-YYYY-MM-DD-NNN.ext
    #[default]
    Standard,
    /// Manter nome original do arquivo
    KeepOriginal,
    /// Padrão customizado (v2 feature)
    Custom(String),
}

impl RenamePattern {
    /// Rótulo curto para exibição
    pub fn label(&self) -> &'static str {
        match self {
            Self::Standard => "photo-YYYY-MM-DD-001",
            Self::KeepOriginal => "Manter nome original",
            Self::Custom(_) => "Customizado",
        }
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
        assert_eq!(options.mode, ImportMode::Copy);
        assert!(options.destination.is_none());
        assert!(options.include_subfolders);
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

    #[test]
    fn test_import_mode_add_nao_move_arquivo() {
        let add = ImportOptions::default().with_mode(ImportMode::Add);
        assert!(!add.moves_files());

        let copy = ImportOptions::default().with_mode(ImportMode::Copy);
        assert!(copy.moves_files());

        let mover = ImportOptions::default().with_mode(ImportMode::Move);
        assert!(mover.moves_files());
    }

    #[test]
    fn test_builders_encadeiam() {
        let options = ImportOptions::default()
            .with_mode(ImportMode::Move)
            .with_destination(Some("/tmp/destino".to_string()))
            .with_source_root(Some("/Volumes/CARTAO".to_string()));

        assert_eq!(options.mode, ImportMode::Move);
        assert_eq!(options.destination.as_deref(), Some("/tmp/destino"));
        assert_eq!(options.source_root.as_deref(), Some("/Volumes/CARTAO"));
    }

    #[test]
    fn test_serde_aceita_json_antigo_sem_campos_novos() {
        // Catálogos gravados antes de `mode`/`destination` existirem têm de continuar lendo.
        let json = r#"{"organization":"ByDate","rename_pattern":"Standard","skip_duplicates":true}"#;
        let options: ImportOptions = serde_json::from_str(json).unwrap();

        assert_eq!(options.mode, ImportMode::Copy);
        assert!(options.destination.is_none());
        assert!(options.include_subfolders);
    }
}
