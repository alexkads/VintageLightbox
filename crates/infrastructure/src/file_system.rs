//! File System module
//!
//! Operações de file system para escanear diretórios e validar arquivos.

use domain::DomainResult;
use std::path::{Path, PathBuf};

/// Extensões de arquivo suportadas
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "tiff", "tif", "cr2", "nef", "arw", "dng", "raf", "orf", "rw2",
];

/// Scanner de arquivos de fotos
pub struct FileScanner {
    extensions: Vec<String>,
}

impl FileScanner {
    /// Cria um novo scanner com extensões padrão
    pub fn new() -> Self {
        Self {
            extensions: SUPPORTED_EXTENSIONS.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Cria um scanner com extensões customizadas
    pub fn with_extensions(extensions: Vec<String>) -> Self {
        Self { extensions }
    }

    /// Escaneia um diretório recursivamente buscando arquivos de foto
    pub fn scan_directory(&self, path: &Path) -> DomainResult<Vec<PathBuf>> {
        self.scan_directory_with_depth(path, true)
    }

    /// Escaneia um diretório, opcionalmente descendo em subpastas
    ///
    /// `recursive == false` é o "Include Subfolders" desmarcado da tela de importação:
    /// varre só o nível pedido, o que num cartão de 64GB é a diferença entre uma tela
    /// instantânea e uma espera de minutos.
    pub fn scan_directory_with_depth(
        &self,
        path: &Path,
        recursive: bool,
    ) -> DomainResult<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.scan_level(path, recursive, &mut files)?;
        files.sort();
        Ok(files)
    }

    /// Percorre um diretório, descendo em subpastas só quando `recursive`
    fn scan_level(
        &self,
        path: &Path,
        recursive: bool,
        files: &mut Vec<PathBuf>,
    ) -> DomainResult<()> {
        if !path.exists() {
            return Err(domain::DomainError::InvalidOperation(format!(
                "Path does not exist: {}",
                path.display()
            )));
        }

        if !path.is_dir() {
            return Err(domain::DomainError::InvalidOperation(format!(
                "Path is not a directory: {}",
                path.display()
            )));
        }

        let entries = std::fs::read_dir(path).map_err(|e| {
            domain::DomainError::InvalidOperation(format!("Failed to read directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                domain::DomainError::InvalidOperation(format!("Failed to read entry: {}", e))
            })?;

            let path = entry.path();

            // Ignorar arquivos/diretórios ocultos
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
            }

            if path.is_dir() {
                // Recursão em subdiretórios
                if recursive {
                    self.scan_level(&path, recursive, files)?;
                }
            } else if path.is_file() {
                // Verificar se a extensão é suportada
                if self.is_supported_file(&path) {
                    files.push(path);
                }
            }
        }

        Ok(())
    }

    /// Verifica se um arquivo tem extensão suportada
    fn is_supported_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            self.extensions.iter().any(|e| e == &ext_str)
        } else {
            false
        }
    }

    /// Retorna as extensões suportadas
    pub fn supported_extensions(&self) -> &[String] {
        &self.extensions
    }
}

impl Default for FileScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_directory() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Criar estrutura de diretórios
        fs::create_dir(base_path.join("subdir1")).unwrap();
        fs::create_dir(base_path.join("subdir2")).unwrap();
        fs::create_dir(base_path.join(".hidden")).unwrap();

        // Criar arquivos de teste
        fs::write(base_path.join("photo1.jpg"), b"fake jpg").unwrap();
        fs::write(base_path.join("photo2.JPG"), b"fake jpg").unwrap();
        fs::write(base_path.join("photo3.cr2"), b"fake raw").unwrap();
        fs::write(base_path.join("document.txt"), b"text file").unwrap();
        fs::write(base_path.join(".hidden_photo.jpg"), b"hidden").unwrap();

        fs::write(base_path.join("subdir1/photo4.nef"), b"fake nef").unwrap();
        fs::write(base_path.join("subdir1/photo5.png"), b"fake png").unwrap();

        fs::write(base_path.join("subdir2/photo6.arw"), b"fake arw").unwrap();
        fs::write(base_path.join(".hidden/photo7.jpg"), b"in hidden dir").unwrap();

        temp_dir
    }

    #[test]
    fn test_scan_directory_finds_photos() {
        let temp_dir = create_test_directory();
        let scanner = FileScanner::new();

        let files = scanner.scan_directory(temp_dir.path()).unwrap();

        // Deve encontrar: photo1.jpg, photo2.JPG, photo3.cr2, photo4.nef, photo5.png, photo6.arw
        // Não deve encontrar: document.txt, .hidden_photo.jpg, photo7.jpg (em .hidden)
        assert_eq!(files.len(), 6);
    }

    #[test]
    fn test_scan_directory_recursive() {
        let temp_dir = create_test_directory();
        let scanner = FileScanner::new();

        let files = scanner.scan_directory(temp_dir.path()).unwrap();

        // Verificar que encontrou arquivos em subdiretórios
        let has_subdir1 = files
            .iter()
            .any(|p| p.to_string_lossy().contains("subdir1"));
        let has_subdir2 = files
            .iter()
            .any(|p| p.to_string_lossy().contains("subdir2"));

        assert!(has_subdir1);
        assert!(has_subdir2);
    }

    #[test]
    fn test_scan_directory_ignores_hidden_files() {
        let temp_dir = create_test_directory();
        let scanner = FileScanner::new();

        let files = scanner.scan_directory(temp_dir.path()).unwrap();

        // Não deve encontrar .hidden_photo.jpg
        let has_hidden = files.iter().any(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
        });

        assert!(!has_hidden);
    }

    #[test]
    fn test_scan_directory_ignores_hidden_directories() {
        let temp_dir = create_test_directory();
        let scanner = FileScanner::new();

        let files = scanner.scan_directory(temp_dir.path()).unwrap();

        // Não deve encontrar photo7.jpg que está em .hidden/
        let has_hidden_dir = files
            .iter()
            .any(|p| p.to_string_lossy().contains(".hidden"));

        assert!(!has_hidden_dir);
    }

    #[test]
    fn test_scan_directory_case_insensitive_extensions() {
        let temp_dir = create_test_directory();
        let scanner = FileScanner::new();

        let files = scanner.scan_directory(temp_dir.path()).unwrap();

        // Deve encontrar tanto .jpg quanto .JPG
        let has_lowercase = files
            .iter()
            .any(|p| p.to_string_lossy().ends_with("photo1.jpg"));
        let has_uppercase = files
            .iter()
            .any(|p| p.to_string_lossy().ends_with("photo2.JPG"));

        assert!(has_lowercase);
        assert!(has_uppercase);
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        let scanner = FileScanner::new();
        let result = scanner.scan_directory(Path::new("/nonexistent/path"));

        assert!(result.is_err());
    }

    #[test]
    fn test_scan_file_instead_of_directory() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jpg");
        fs::write(&file_path, b"test").unwrap();

        let scanner = FileScanner::new();
        let result = scanner.scan_directory(&file_path);

        assert!(result.is_err());
    }

    #[test]
    fn test_custom_extensions() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("photo.jpg"), b"jpg").unwrap();
        fs::write(temp_dir.path().join("photo.png"), b"png").unwrap();
        fs::write(temp_dir.path().join("photo.cr2"), b"raw").unwrap();

        // Scanner que só aceita jpg
        let scanner = FileScanner::with_extensions(vec!["jpg".to_string()]);
        let files = scanner.scan_directory(temp_dir.path()).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().ends_with(".jpg"));
    }

    #[test]
    fn test_scan_sem_subpastas_fica_no_nivel_de_cima() {
        let temp_dir = create_test_directory();
        let scanner = FileScanner::new();

        let files = scanner
            .scan_directory_with_depth(temp_dir.path(), false)
            .unwrap();

        // Só photo1.jpg, photo2.JPG, photo3.cr2 — nada de subdir1/subdir2
        assert_eq!(files.len(), 3);
        assert!(!files.iter().any(|p| p.to_string_lossy().contains("subdir")));
    }

    #[test]
    fn test_scan_devolve_ordenado() {
        let temp_dir = create_test_directory();
        let scanner = FileScanner::new();

        let files = scanner.scan_directory(temp_dir.path()).unwrap();

        // A grade da importação mostra na ordem em que vem: leitura de diretório não
        // garante ordem nenhuma, então o scanner ordena.
        let mut ordenado = files.clone();
        ordenado.sort();
        assert_eq!(files, ordenado);
    }

    #[test]
    fn test_supported_extensions() {
        let scanner = FileScanner::new();
        let extensions = scanner.supported_extensions();

        assert!(extensions.contains(&"jpg".to_string()));
        assert!(extensions.contains(&"cr2".to_string()));
        assert!(extensions.contains(&"nef".to_string()));
    }
}
