//! Describe Import Candidates Use Case
//!
//! Lê os metadados dos arquivos que a grade de importação está mostrando — **sem** gerar
//! miniatura.
//!
//! A separação importa: `PreviewBeforeImport` decodifica cada imagem para produzir um
//! thumbnail, o que num cartão com 2.000 RAWs leva minutos. A grade precisa de muito menos
//! para ficar útil (nome, data de captura, tamanho, se é RAW), e as miniaturas chegam
//! depois, só para as células visíveis.

use domain::{
    services::MetadataExtractor,
    value_objects::{FilePath, PhotoMetadata},
    DomainResult,
};
use std::sync::Arc;

/// Descrição de um arquivo candidato à importação
#[derive(Debug, Clone)]
pub struct ImportCandidate {
    /// Caminho do arquivo na origem
    pub file_path: FilePath,
    /// Metadados EXIF (vazios quando o arquivo não tem ou não pôde ser lido)
    pub metadata: PhotoMetadata,
    /// Tamanho em bytes
    pub file_size: u64,
    /// Se é um arquivo RAW
    pub is_raw: bool,
}

/// Extensões tratadas como RAW
const RAW_EXTENSIONS: &[&str] = &["cr2", "cr3", "nef", "arw", "dng", "raf", "orf", "rw2"];

/// Use case para descrever candidatos à importação
pub struct DescribeCandidatesUseCase {
    metadata_extractor: Arc<dyn MetadataExtractor>,
}

impl DescribeCandidatesUseCase {
    /// Cria uma nova instância
    pub fn new(metadata_extractor: Arc<dyn MetadataExtractor>) -> Self {
        Self { metadata_extractor }
    }

    /// Descreve os arquivos informados, em paralelo
    ///
    /// Um arquivo ilegível não some da lista: ele volta com metadados vazios, porque
    /// esconder da grade um arquivo que está no cartão é pior do que mostrá-lo sem data.
    pub async fn execute(&self, files: Vec<FilePath>) -> DomainResult<Vec<ImportCandidate>> {
        if files.is_empty() {
            return Ok(Vec::new());
        }

        let tasks: Vec<_> = files
            .into_iter()
            .map(|path| {
                let metadata_extractor = self.metadata_extractor.clone();

                tokio::spawn(async move {
                    let metadata = metadata_extractor.extract(&path).unwrap_or_default();

                    let file_size = std::fs::metadata(path.as_ref() as &std::path::Path)
                        .map(|m| m.len())
                        .unwrap_or(0);

                    let is_raw = Self::is_raw_file(&path);

                    ImportCandidate {
                        file_path: path,
                        metadata,
                        file_size,
                        is_raw,
                    }
                })
            })
            .collect();

        let results = futures::future::join_all(tasks).await;

        Ok(results.into_iter().filter_map(|r| r.ok()).collect())
    }

    /// Verifica se o arquivo é RAW pela extensão
    fn is_raw_file(path: &FilePath) -> bool {
        std::path::Path::new(path.as_ref() as &std::path::Path)
            .extension()
            .map(|ext| {
                let ext = ext.to_string_lossy().to_lowercase();
                RAW_EXTENSIONS.contains(&ext.as_str())
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use domain::DomainError;
    use std::io::Write;
    use tempfile::TempDir;

    struct ExtratorFalso {
        camera: Option<String>,
        falha: bool,
    }

    #[async_trait]
    impl MetadataExtractor for ExtratorFalso {
        fn extract(&self, _path: &FilePath) -> DomainResult<PhotoMetadata> {
            if self.falha {
                return Err(DomainError::InvalidOperation("sem exif".to_string()));
            }
            Ok(PhotoMetadata {
                camera_model: self.camera.clone(),
                ..Default::default()
            })
        }
    }

    fn arquivo_com(dir: &TempDir, nome: &str, bytes: &[u8]) -> FilePath {
        let path = dir.path().join(nome);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        FilePath::new(path.to_str().unwrap()).unwrap()
    }

    #[tokio::test]
    async fn descreve_tamanho_e_raw() {
        let dir = TempDir::new().unwrap();
        let raw = arquivo_com(&dir, "foto.cr2", b"1234567890");
        let jpg = arquivo_com(&dir, "foto.jpg", b"12345");

        let use_case = DescribeCandidatesUseCase::new(Arc::new(ExtratorFalso {
            camera: Some("Canon R6".to_string()),
            falha: false,
        }));

        let mut candidatos = use_case.execute(vec![raw, jpg]).await.unwrap();
        candidatos.sort_by_key(|c| c.file_size);

        assert_eq!(candidatos[0].file_size, 5);
        assert!(!candidatos[0].is_raw);
        assert_eq!(candidatos[1].file_size, 10);
        assert!(candidatos[1].is_raw);
    }

    #[tokio::test]
    async fn arquivo_sem_exif_continua_na_lista() {
        let dir = TempDir::new().unwrap();
        let jpg = arquivo_com(&dir, "foto.jpg", b"12345");

        let use_case = DescribeCandidatesUseCase::new(Arc::new(ExtratorFalso {
            camera: None,
            falha: true,
        }));

        let candidatos = use_case.execute(vec![jpg]).await.unwrap();

        assert_eq!(candidatos.len(), 1);
        assert!(candidatos[0].metadata.camera_model.is_none());
    }

    #[tokio::test]
    async fn lista_vazia_nao_faz_nada() {
        let use_case = DescribeCandidatesUseCase::new(Arc::new(ExtratorFalso {
            camera: None,
            falha: false,
        }));

        assert!(use_case.execute(vec![]).await.unwrap().is_empty());
    }
}
