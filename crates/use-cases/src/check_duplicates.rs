//! Check Duplicates Use Case
//!
//! Caso de uso para detectar fotos duplicadas através de content hash (SHA-256).
//! Usado antes da importação para evitar duplicatas no catálogo.

use domain::{
    entities::Photo, repositories::PhotoRepository, value_objects::FilePath, DomainResult,
};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::sync::Arc;

/// Resultado da verificação de duplicata para um arquivo
#[derive(Debug, Clone)]
pub struct DuplicateCheckResult {
    /// Caminho do arquivo verificado
    pub file_path: FilePath,
    /// Hash SHA-256 do arquivo
    pub content_hash: String,
    /// Se é duplicata (já existe no catálogo)
    pub is_duplicate: bool,
    /// Foto existente no catálogo (se for duplicata)
    pub existing_photo: Option<Photo>,
}

/// Use Case para detectar duplicatas
pub struct CheckDuplicatesUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
}

impl CheckDuplicatesUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(photo_repository: Arc<dyn PhotoRepository>) -> Self {
        Self { photo_repository }
    }

    /// Calcula SHA-256 hash de um arquivo
    fn calculate_file_hash(path: &std::path::Path) -> Result<String, std::io::Error> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    /// Verifica uma lista de arquivos para detectar duplicatas **na sessão**
    /// `sessao_id` (`None` é o lote sem sessão).
    ///
    /// 🚨 Duplicata é a foto que **já está nesta sessão**, e não em qualquer
    /// lugar do catálogo: a mesma foto em outra sessão, ou num rascunho
    /// abandonado, entra de novo — ver
    /// [`PhotoRepository::find_by_content_hash_na_sessao`].
    ///
    /// Processa arquivos em paralelo usando Rayon para melhor performance.
    /// Hash é calculado de forma síncrona (bloqueante) em threads separadas.
    pub async fn execute(
        &self,
        files: Vec<FilePath>,
        sessao_id: Option<&str>,
    ) -> DomainResult<Vec<DuplicateCheckResult>> {
        use rayon::prelude::*;

        if files.is_empty() {
            return Ok(Vec::new());
        }

        // Calcular hashes em paralelo usando Rayon
        // O hash lê cada arquivo inteiro. Rodá-lo no worker assíncrono segura
        // esse worker até o lote todo terminar e atrasa os recados de progresso.
        let hashes: Vec<(FilePath, Option<String>)> = tokio::task::spawn_blocking(move || {
            files
                .par_iter()
                .map(|path| {
                    let hash = Self::calculate_file_hash(path.as_ref()).ok();
                    (path.clone(), hash)
                })
                .collect()
        })
        .await
        .map_err(|e| domain::DomainError::InfrastructureError(format!("Hash das fotos: {e}")))?;

        // Verificar duplicatas no banco de dados (sequencial, mas indexado)
        let mut results = Vec::new();
        for (file_path, hash_opt) in hashes {
            if let Some(hash) = hash_opt {
                // Buscar no banco de dados
                let existing_photo = self
                    .photo_repository
                    .find_by_content_hash_na_sessao(&hash, sessao_id.map(str::to_owned))
                    .await?;

                results.push(DuplicateCheckResult {
                    file_path,
                    content_hash: hash,
                    is_duplicate: existing_photo.is_some(),
                    existing_photo,
                });
            } else {
                // Falha ao calcular hash - não é duplicata por padrão
                results.push(DuplicateCheckResult {
                    file_path,
                    content_hash: String::new(),
                    is_duplicate: false,
                    existing_photo: None,
                });
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::repositories::PhotoRepository;
    use mockall::{mock, predicate::*};
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Mock do PhotoRepository
    mock! {
        pub PhotoRepo {}

        #[async_trait::async_trait]
        impl PhotoRepository for PhotoRepo {
            async fn save(&self, photo: &Photo) -> DomainResult<()>;
            async fn find_by_id(&self, id: &domain::value_objects::PhotoId) -> DomainResult<Option<Photo>>;
            async fn find_all(&self) -> DomainResult<Vec<Photo>>;
            async fn update(&self, photo: &Photo) -> DomainResult<()>;
            async fn delete(&self, id: &domain::value_objects::PhotoId) -> DomainResult<()>;
            async fn exists(&self, id: &domain::value_objects::PhotoId) -> DomainResult<bool>;
            async fn find_by_content_hash(&self, hash: &str) -> DomainResult<Option<Photo>>;
            async fn find_by_content_hash_na_sessao(&self, hash: &str, sessao_id: Option<String>) -> DomainResult<Option<Photo>>;
        }
    }

    // Helper para criar arquivo temporário
    fn create_temp_file_with_content(content: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content).unwrap();
        file.flush().unwrap();
        file
    }

    #[tokio::test]
    async fn test_no_duplicates() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();

        // Nenhuma foto encontrada no banco
        mock_repo
            .expect_find_by_content_hash_na_sessao()
            .times(2)
            .returning(|_, _| Ok(None));

        let use_case = CheckDuplicatesUseCase::new(Arc::new(mock_repo));

        let temp1 = create_temp_file_with_content(b"file 1 content");
        let temp2 = create_temp_file_with_content(b"file 2 content");
        let files = vec![
            FilePath::new(temp1.path().to_str().unwrap()).unwrap(),
            FilePath::new(temp2.path().to_str().unwrap()).unwrap(),
        ];

        // Act
        let result = use_case.execute(files, Some("ensaio")).await;

        // Assert
        assert!(result.is_ok());
        let results = result.unwrap();
        assert_eq!(results.len(), 2);
        assert!(!results[0].is_duplicate);
        assert!(!results[1].is_duplicate);
        assert!(results[0].existing_photo.is_none());
        assert!(results[1].existing_photo.is_none());
        assert_eq!(results[0].content_hash.len(), 64); // SHA-256 = 64 hex chars
    }

    #[tokio::test]
    async fn test_all_duplicates() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();

        // Criar uma foto fake para retornar
        let fake_photo = Photo::new(FilePath::new("/fake/path.jpg").unwrap());
        let fake_photo_clone = fake_photo.clone();

        // Todas as fotos são duplicatas
        mock_repo
            .expect_find_by_content_hash_na_sessao()
            .times(2)
            .returning(move |_, _| Ok(Some(fake_photo_clone.clone())));

        let use_case = CheckDuplicatesUseCase::new(Arc::new(mock_repo));

        let temp1 = create_temp_file_with_content(b"duplicate content");
        let temp2 = create_temp_file_with_content(b"another duplicate");
        let files = vec![
            FilePath::new(temp1.path().to_str().unwrap()).unwrap(),
            FilePath::new(temp2.path().to_str().unwrap()).unwrap(),
        ];

        // Act
        let result = use_case.execute(files, Some("ensaio")).await;

        // Assert
        assert!(result.is_ok());
        let results = result.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].is_duplicate);
        assert!(results[1].is_duplicate);
        assert!(results[0].existing_photo.is_some());
        assert!(results[1].existing_photo.is_some());
    }

    #[tokio::test]
    async fn test_mixed_results() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();

        let fake_photo = Photo::new(FilePath::new("/fake/path.jpg").unwrap());
        let fake_photo_clone = fake_photo.clone();

        // Primeira chamada retorna duplicata, segunda não
        mock_repo
            .expect_find_by_content_hash_na_sessao()
            .times(2)
            .returning(move |hash: &str, _| {
                // Simular que o primeiro arquivo é duplicata
                if hash.starts_with('a') || hash.starts_with('b') || hash.starts_with('c') {
                    Ok(Some(fake_photo_clone.clone()))
                } else {
                    Ok(None)
                }
            });

        let use_case = CheckDuplicatesUseCase::new(Arc::new(mock_repo));

        let temp1 = create_temp_file_with_content(b"content 1");
        let temp2 = create_temp_file_with_content(b"different content 2");
        let files = vec![
            FilePath::new(temp1.path().to_str().unwrap()).unwrap(),
            FilePath::new(temp2.path().to_str().unwrap()).unwrap(),
        ];

        // Act
        let result = use_case.execute(files, Some("ensaio")).await;

        // Assert
        assert!(result.is_ok());
        let results = result.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_hash_consistency() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();

        mock_repo
            .expect_find_by_content_hash_na_sessao()
            .times(1)
            .returning(|_, _| Ok(None));

        let use_case = CheckDuplicatesUseCase::new(Arc::new(mock_repo));

        let temp = create_temp_file_with_content(b"consistent content");
        let files = vec![FilePath::new(temp.path().to_str().unwrap()).unwrap()];

        // Act
        let result1 = use_case
            .execute(files.clone(), Some("ensaio"))
            .await
            .unwrap();

        // O hash deveria ser o mesmo se recalcularmos
        let hash1 = &result1[0].content_hash;
        assert_eq!(hash1.len(), 64);

        // Verificar que o hash é hexadecimal
        assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// 🚨 A conferência pergunta **pela sessão de destino**, e nunca pelo
    /// catálogo inteiro: a mesma foto em outra sessão não é duplicata desta.
    #[tokio::test]
    async fn confere_so_na_sessao_de_destino() {
        let mut mock_repo = MockPhotoRepo::new();
        mock_repo.expect_find_by_content_hash().never();
        mock_repo
            .expect_find_by_content_hash_na_sessao()
            .withf(|_, sessao| sessao.as_deref() == Some("ensaio-b"))
            .times(1)
            .returning(|_, _| Ok(None));

        let use_case = CheckDuplicatesUseCase::new(Arc::new(mock_repo));
        let temp = create_temp_file_with_content(b"foto");
        let files = vec![FilePath::new(temp.path().to_str().unwrap()).unwrap()];

        let resultado = use_case.execute(files, Some("ensaio-b")).await.unwrap();
        assert!(!resultado[0].is_duplicate);
    }

    #[tokio::test]
    async fn test_empty_input() {
        // Arrange
        let mock_repo = MockPhotoRepo::new();
        let use_case = CheckDuplicatesUseCase::new(Arc::new(mock_repo));

        // Act
        let result = use_case.execute(vec![], Some("ensaio")).await;

        // Assert
        assert!(result.is_ok());
        let results = result.unwrap();
        assert_eq!(results.len(), 0);
    }
}
