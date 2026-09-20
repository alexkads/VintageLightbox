//! Source Scanner Implementation
//!
//! Implementa [`domain::services::SourceScanner`] sobre o [`FileScanner`] já existente.
//!
//! A varredura é síncrona e pode demorar (cartão cheio, disco de rede), então roda em
//! `spawn_blocking` — a tela de importação continua desenhando enquanto ela acontece.

use async_trait::async_trait;
use domain::{services::SourceScanner, value_objects::FilePath, DomainError, DomainResult};
use std::path::PathBuf;

use crate::file_system::FileScanner;

/// Implementação do SourceScanner
pub struct SourceScannerImpl;

impl SourceScannerImpl {
    /// Cria uma nova instância
    pub fn new() -> Self {
        Self
    }
}

impl Default for SourceScannerImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SourceScanner for SourceScannerImpl {
    async fn scan(&self, root: &str, include_subfolders: bool) -> DomainResult<Vec<FilePath>> {
        let root = PathBuf::from(root);

        let paths = tokio::task::spawn_blocking(move || {
            FileScanner::new().scan_directory_with_depth(&root, include_subfolders)
        })
        .await
        .map_err(|e| DomainError::InfrastructureError(format!("Scan task falhou: {}", e)))??;

        // Um caminho inválido não pode derrubar a varredura inteira: o arquivo some da lista
        // e os outros continuam aparecendo na grade.
        Ok(paths
            .iter()
            .filter_map(|p| p.to_str())
            .filter_map(|p| FilePath::new(p).ok())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn origem_de_teste() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("DCIM")).unwrap();
        fs::write(dir.path().join("raiz.jpg"), b"jpg").unwrap();
        fs::write(dir.path().join("DCIM/dentro.cr2"), b"raw").unwrap();
        fs::write(dir.path().join("leiame.txt"), b"texto").unwrap();
        dir
    }

    #[tokio::test]
    async fn scan_com_subpastas_encontra_tudo() {
        let dir = origem_de_teste();
        let scanner = SourceScannerImpl::new();

        let files = scanner
            .scan(dir.path().to_str().unwrap(), true)
            .await
            .unwrap();

        assert_eq!(files.len(), 2);
    }

    #[tokio::test]
    async fn scan_sem_subpastas_ignora_dcim() {
        let dir = origem_de_teste();
        let scanner = SourceScannerImpl::new();

        let files = scanner
            .scan(dir.path().to_str().unwrap(), false)
            .await
            .unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].to_string().ends_with("raiz.jpg"));
    }

    #[tokio::test]
    async fn scan_de_origem_inexistente_devolve_erro() {
        let scanner = SourceScannerImpl::new();
        let resultado = scanner.scan("/caminho/que/nao/existe", true).await;
        assert!(resultado.is_err());
    }
}
