//! Scan Source Use Case
//!
//! Lista as fotos de uma origem para a tela de importação — sem importar nada.
//!
//! Existe separado de `ImportWithOptions` porque a tela precisa mostrar a grade **antes**
//! de qualquer decisão: o usuário escolhe o que entra olhando as miniaturas, e até apertar
//! "Importar" nenhum arquivo foi lido além do necessário para listar.

use domain::{services::SourceScanner, value_objects::FilePath, DomainResult};
use std::sync::Arc;

/// Use case para varrer uma origem de importação
pub struct ScanSourceUseCase {
    scanner: Arc<dyn SourceScanner>,
}

impl ScanSourceUseCase {
    /// Cria uma nova instância
    pub fn new(scanner: Arc<dyn SourceScanner>) -> Self {
        Self { scanner }
    }

    /// Lista as fotos da origem
    pub async fn execute(
        &self,
        root: &str,
        include_subfolders: bool,
    ) -> DomainResult<Vec<FilePath>> {
        if root.trim().is_empty() {
            return Ok(Vec::new());
        }

        self.scanner.scan(root, include_subfolders).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use domain::DomainError;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct ScannerFalso {
        arquivos: Vec<String>,
        chamadas: AtomicUsize,
    }

    impl ScannerFalso {
        fn com(arquivos: &[&str]) -> Self {
            Self {
                arquivos: arquivos.iter().map(|s| s.to_string()).collect(),
                chamadas: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl SourceScanner for ScannerFalso {
        async fn scan(
            &self,
            _root: &str,
            _include_subfolders: bool,
        ) -> DomainResult<Vec<FilePath>> {
            self.chamadas.fetch_add(1, Ordering::Relaxed);
            Ok(self
                .arquivos
                .iter()
                .filter_map(|f| FilePath::new(f).ok())
                .collect())
        }
    }

    struct ScannerQueFalha;

    #[async_trait]
    impl SourceScanner for ScannerQueFalha {
        async fn scan(&self, _root: &str, _include: bool) -> DomainResult<Vec<FilePath>> {
            Err(DomainError::InvalidOperation("cartão sumiu".to_string()))
        }
    }

    #[tokio::test]
    async fn devolve_os_arquivos_da_origem() {
        let use_case = ScanSourceUseCase::new(Arc::new(ScannerFalso::com(&[
            "/cartao/a.cr2",
            "/cartao/b.jpg",
        ])));

        let files = use_case.execute("/cartao", true).await.unwrap();

        assert_eq!(files.len(), 2);
    }

    #[tokio::test]
    async fn origem_vazia_nao_chega_a_varrer_o_disco() {
        let scanner = Arc::new(ScannerFalso::com(&["/cartao/a.cr2"]));
        let use_case = ScanSourceUseCase::new(scanner.clone());

        let files = use_case.execute("   ", true).await.unwrap();

        assert!(files.is_empty());
        assert_eq!(scanner.chamadas.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn erro_do_scanner_sobe_para_a_tela() {
        let use_case = ScanSourceUseCase::new(Arc::new(ScannerQueFalha));

        assert!(use_case.execute("/cartao", true).await.is_err());
    }
}
