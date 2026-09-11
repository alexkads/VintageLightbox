//! Import Photo Use Case
//!
//! Caso de uso responsável por importar uma foto para o catálogo.
//! Implementado com TDD e usando mocks para testes.

use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    services::{MetadataExtractor, PreviewStorage, PreviewType, ThumbnailGenerator},
    value_objects::FilePath,
    DomainResult,
};
use std::path::Path;
use std::sync::Arc;

/// Use Case para importar fotos
pub struct ImportPhotoUseCase {
    photo_repository: Arc<dyn PhotoRepository>,
    metadata_extractor: Arc<dyn MetadataExtractor>,
    thumbnail_generator: Arc<dyn ThumbnailGenerator>,
    preview_storage: Arc<dyn PreviewStorage>,
}

impl ImportPhotoUseCase {
    /// Cria uma nova instância do Use Case
    pub fn new(
        photo_repository: Arc<dyn PhotoRepository>,
        metadata_extractor: Arc<dyn MetadataExtractor>,
        thumbnail_generator: Arc<dyn ThumbnailGenerator>,
        preview_storage: Arc<dyn PreviewStorage>,
    ) -> Self {
        Self {
            photo_repository,
            metadata_extractor,
            thumbnail_generator,
            preview_storage,
        }
    }

    /// Importa uma foto do sistema de arquivos, copiando para diretório organizado
    pub async fn execute(&self, file_path: FilePath) -> DomainResult<Photo> {
        // Extrair metadados primeiro para obter a data
        let metadata_result = self.metadata_extractor.extract(&file_path);

        // Determinar a data para organização das pastas
        let (year, month, day) = if let Ok(ref metadata) = metadata_result {
            if let Some(ref dt) = metadata.date_time {
                // Tentar parsear EXIF date format "YYYY:MM:DD HH:MM:SS"
                let parts: Vec<&str> = dt.split(' ').next().unwrap_or("").split(':').collect();
                if parts.len() >= 3 {
                    (
                        parts[0].to_string(),
                        parts[1].to_string(),
                        parts[2].to_string(),
                    )
                } else {
                    Self::current_date()
                }
            } else {
                Self::current_date()
            }
        } else {
            Self::current_date()
        };

        // Obter diretório base: ~/Pictures/VintageLightbox
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let base_dir = std::path::PathBuf::from(&home)
            .join("Pictures")
            .join("VintageLightbox");

        // Criar subdiretório baseado na data: YYYY/MM/DD
        let dest_dir = base_dir.join(&year).join(&month).join(&day);

        // Criar diretórios se não existirem
        if !dest_dir.exists() {
            tokio::fs::create_dir_all(&dest_dir).await.map_err(|e| {
                domain::DomainError::InfrastructureError(format!(
                    "Failed to create directory {}: {}",
                    dest_dir.display(),
                    e
                ))
            })?;
        }

        // Obter extensão do arquivo original
        let source_path: &Path = file_path.as_ref();
        let extension = source_path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "jpg".to_string());

        // Nome padronizado `photo-YYYY-MM-DD-NNN`, com o número reservado.
        //
        // 🚨 **`create_new`, e não `exists()`.** Perguntar ao disco "este nome
        // está livre?" e copiar depois deixa a janela em que outro arquivo do
        // mesmo lote escolhe o mesmo nome: os dois copiam para lá e um
        // sobrescreve o outro. É a armadilha nº 67 na versão do sistema de
        // arquivos, e o `file_organizer` — o caminho por onde a importação
        // **realmente** passa hoje — já a resolve assim. Este aqui é o caminho
        // legado (`ImportController::import_files`, que nenhuma tela chama):
        // ficar inseguro só porque está adormecido seria deixar a armadilha
        // armada para quem o religar.
        //
        // ⚠️ O arquivo nasce com zero byte e só então recebe a cópia. Uma falha
        // no meio deixa um arquivo vazio no destino — o menor dos dois preços,
        // porque o outro é a foto de alguém por cima da foto de outro.
        let mut sequential = 1;
        let dest_path = loop {
            let tentativa = dest_dir.join(format!(
                "photo-{}-{}-{}-{:03}.{}",
                year, month, day, sequential, extension
            ));
            let livre = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tentativa)
                .await
                .is_ok();
            if livre {
                break tentativa;
            }
            sequential += 1;
            if sequential > 9_999 {
                return Err(domain::DomainError::InfrastructureError(format!(
                    "não há nome livre para {} em {}",
                    source_path.display(),
                    dest_dir.display()
                )));
            }
        };

        tokio::fs::copy(source_path, &dest_path)
            .await
            .map_err(|e| {
                domain::DomainError::InfrastructureError(format!(
                    "Failed to copy file to {}: {}",
                    dest_path.display(),
                    e
                ))
            })?;
        let final_path = dest_path;

        // Criar FilePath com o novo caminho
        let new_file_path = FilePath::new(final_path.to_string_lossy().as_ref())?;

        // Criar nova entidade Photo com o caminho copiado
        let mut photo = Photo::new(new_file_path.clone());

        // Definir metadados se disponíveis
        if let Ok(metadata) = metadata_result {
            photo.set_metadata(metadata);
        }

        // Gerar Thumbnails (Batched)
        // Solicita geração de thumbnails de 300px (Thumbnail) e 2560px (Large Preview) em um único passo
        match self
            .thumbnail_generator
            .generate_set(&new_file_path, &[300, 2560])
            .await
        {
            Ok(results) => {
                if results.len() >= 2 {
                    if let Err(e) =
                        self.preview_storage
                            .save(&photo.id(), PreviewType::Thumbnail, &results[0])
                    {
                        eprintln!("Failed to save thumbnail: {}", e);
                    }
                    if let Err(e) =
                        self.preview_storage
                            .save(&photo.id(), PreviewType::Large, &results[1])
                    {
                        eprintln!("Failed to save preview: {}", e);
                    }
                }
            }
            Err(e) => eprintln!("Failed to generate thumbnails: {}", e),
        }

        // Persistir no repositório
        self.photo_repository.save(&photo).await?;

        Ok(photo)
    }

    /// Retorna data atual formatada como (YYYY, MM, DD)
    fn current_date() -> (String, String, String) {
        let now = chrono::Local::now();
        (
            now.format("%Y").to_string(),
            now.format("%m").to_string(),
            now.format("%d").to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{repositories::PhotoRepository, value_objects::PhotoMetadata, DomainError};
    use mockall::mock;
    use mockall::predicate::*;
    use tempfile::NamedTempFile;

    // Mock do MetadataExtractor
    mock! {
        pub MetadataExtractor {}

        impl MetadataExtractor for MetadataExtractor {
            fn extract(&self, path: &FilePath) -> DomainResult<PhotoMetadata>;
        }
    }

    // Mock do ThumbnailGenerator
    mock! {
        pub ThumbnailGenerator {}
        #[async_trait::async_trait]
        impl ThumbnailGenerator for ThumbnailGenerator {
            async fn generate(&self, path: &FilePath, max_dimension: u32) -> DomainResult<Vec<u8>>;
            async fn generate_set(&self, path: &FilePath, max_sizes: &[u32]) -> DomainResult<Vec<Vec<u8>>>;
        }
    }

    // Mock do PreviewStorage
    mock! {
        pub PreviewStorage {}
        impl PreviewStorage for PreviewStorage {
            fn save(&self, id: &domain::value_objects::PhotoId, preview_type: PreviewType, data: &[u8]) -> DomainResult<()>;
            fn get(&self, id: &domain::value_objects::PhotoId, preview_type: PreviewType) -> DomainResult<Option<Vec<u8>>>;
            fn has(&self, id: &domain::value_objects::PhotoId, preview_type: PreviewType) -> DomainResult<bool>;
            fn delete(&self, id: &domain::value_objects::PhotoId) -> DomainResult<()>;
        }
    }

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
        }
    }

    #[tokio::test]
    async fn test_import_photo_success() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();

        // Espera-se que save seja chamado uma vez e retorne Ok
        mock_repo.expect_save().times(1).returning(|_| Ok(()));

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));

        let mut mock_generator = MockThumbnailGenerator::new();
        // Expect generate_set call for 300 and 2560 sizes
        mock_generator
            .expect_generate_set()
            .with(always(), eq(vec![300, 2560]))
            .times(1)
            .returning(|_, _| Ok(vec![vec![1, 2, 3], vec![4, 5, 6]])); // Return fake image data

        let mut mock_preview_storage = MockPreviewStorage::new();
        mock_preview_storage
            .expect_save()
            .times(2)
            .returning(|_, _, _| Ok(()));

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
            Arc::new(mock_preview_storage),
        );
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();
        let file_path = FilePath::new(path).unwrap();

        // Act
        let result = use_case.execute(file_path.clone()).await;

        // Assert
        assert!(result.is_ok());
        let photo = result.unwrap();
        let path_str = photo.file_path().to_string();
        assert!(path_str.contains("VintageLightbox"));
        assert_ne!(photo.file_path(), &file_path);
    }

    /// 🚨 **Duas fotos do mesmo lote não podem receber o mesmo destino.**
    ///
    /// O nome de saída é `photo-AAAA-MM-DD-NNN`, e o número vinha de perguntar
    /// ao disco "já existe?" — copiando **depois**. Entre a pergunta e a cópia
    /// cabe outro arquivo do mesmo lote: os dois acham o mesmo número livre,
    /// os dois copiam para lá, e um apaga o outro. É a armadilha nº 67 na
    /// versão do sistema de arquivos.
    ///
    /// ⚠️ Era pior do que sobrescrever: quando o destino já existia, este
    /// caminho **não copiava** e gravava no catálogo o caminho do arquivo do
    /// outro — a foto importada apontava para uma imagem que não é a dela.
    ///
    /// # 🔑 Por que o teste é concorrente, e tinha de ser
    ///
    /// A primeira versão deste caso importava duas fotos **em sequência** e
    /// passava com o código defeituoso: a primeira já tinha criado o arquivo,
    /// então a segunda via "existe" e ia para o número seguinte. O defeito só
    /// aparece quando as duas escolhem **ao mesmo tempo** — que é como uma
    /// importação de lote roda. Um teste que não reprova o defeito não vale
    /// nada, e este foi reescrito depois de ser pego passando.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn fotos_importadas_ao_mesmo_tempo_nao_disputam_o_mesmo_arquivo() {
        use std::io::Write;

        fn caso() -> ImportPhotoUseCase {
            let mut repo = MockPhotoRepo::new();
            repo.expect_save().returning(|_| Ok(()));
            let mut extrator = MockMetadataExtractor::new();
            extrator
                .expect_extract()
                .returning(|_| Ok(PhotoMetadata::default()));
            let mut miniaturas = MockThumbnailGenerator::new();
            miniaturas
                .expect_generate_set()
                .returning(|_, _| Ok(vec![vec![1], vec![2]]));
            let mut previas = MockPreviewStorage::new();
            previas.expect_save().returning(|_, _, _| Ok(()));
            ImportPhotoUseCase::new(
                Arc::new(repo),
                Arc::new(extrator),
                Arc::new(miniaturas),
                Arc::new(previas),
            )
        }

        // Oito origens com conteúdos distintos: é o conteúdo que denuncia a
        // troca, e não o nome.
        const QUANTAS: usize = 8;
        let mut origens = Vec::new();
        for i in 0..QUANTAS {
            let mut arquivo = NamedTempFile::with_suffix(".jpg").unwrap();
            write!(arquivo, "foto numero {i}").unwrap();
            origens.push(arquivo);
        }

        let mut tarefas = Vec::new();
        for (i, origem) in origens.iter().enumerate() {
            let caminho = FilePath::new(origem.path().to_str().unwrap()).unwrap();
            tarefas.push(tokio::spawn(
                async move { (i, caso().execute(caminho).await) },
            ));
        }

        let mut destinos = Vec::new();
        for tarefa in tarefas {
            let (i, resultado) = tarefa.await.expect("a tarefa não entra em pânico");
            let foto = resultado.expect("a importação não falha");
            destinos.push((i, foto.file_path().to_string()));
        }

        let unicos: std::collections::HashSet<&String> =
            destinos.iter().map(|(_, caminho)| caminho).collect();
        assert_eq!(
            unicos.len(),
            QUANTAS,
            "duas fotos caíram no mesmo arquivo: {destinos:?}"
        );

        for (i, caminho) in &destinos {
            let conteudo = std::fs::read_to_string(caminho).unwrap();
            assert_eq!(
                conteudo,
                format!("foto numero {i}"),
                "a foto {i} aponta para os bytes de outra"
            );
            let _ = std::fs::remove_file(caminho);
        }
    }

    #[tokio::test]
    async fn test_import_photo_repository_error() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();

        // Simular erro no repositório
        mock_repo
            .expect_save()
            .times(1)
            .returning(|_| Err(DomainError::InvalidFilePath("DB error".to_string())));

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));

        let mut mock_generator = MockThumbnailGenerator::new();
        mock_generator
            .expect_generate_set()
            .returning(|_, _| Ok(vec![vec![], vec![]]));

        let mut mock_preview_storage = MockPreviewStorage::new();
        mock_preview_storage
            .expect_save()
            .returning(|_, _, _| Ok(()));

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
            Arc::new(mock_preview_storage),
        );
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();
        let file_path = FilePath::new(path).unwrap();

        // Act
        let result = use_case.execute(file_path).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_import_photo_creates_new_id() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();

        mock_repo.expect_save().times(2).returning(|_| Ok(()));

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));

        let mut mock_generator = MockThumbnailGenerator::new();
        mock_generator
            .expect_generate_set()
            .returning(|_, _| Ok(vec![vec![], vec![]]));

        let mut mock_preview_storage = MockPreviewStorage::new();
        mock_preview_storage
            .expect_save()
            .returning(|_, _, _| Ok(()));

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
            Arc::new(mock_preview_storage),
        );

        let temp_file1 = NamedTempFile::new().unwrap();
        let file_path1 = FilePath::new(temp_file1.path().to_str().unwrap()).unwrap();

        let temp_file2 = NamedTempFile::new().unwrap();
        let file_path2 = FilePath::new(temp_file2.path().to_str().unwrap()).unwrap();

        // Act
        let photo1 = use_case.execute(file_path1).await.unwrap();
        let photo2 = use_case.execute(file_path2).await.unwrap();

        // Assert
        // Cada foto deve ter um ID único
        assert_ne!(photo1.id(), photo2.id());
    }

    #[tokio::test]
    async fn test_import_photo_with_different_extensions() {
        // Arrange
        let mut mock_repo = MockPhotoRepo::new();

        mock_repo.expect_save().times(3).returning(|_| Ok(()));

        let mut mock_extractor = MockMetadataExtractor::new();
        mock_extractor
            .expect_extract()
            .returning(|_| Ok(PhotoMetadata::default()));

        let mut mock_generator = MockThumbnailGenerator::new();
        mock_generator
            .expect_generate_set()
            .returning(|_, _| Ok(vec![vec![], vec![]]));

        let mut mock_preview_storage = MockPreviewStorage::new();
        mock_preview_storage
            .expect_save()
            .returning(|_, _, _| Ok(()));

        let use_case = ImportPhotoUseCase::new(
            Arc::new(mock_repo),
            Arc::new(mock_extractor),
            Arc::new(mock_generator),
            Arc::new(mock_preview_storage),
        );

        // Act & Assert - diferentes extensões devem funcionar
        let extensions = vec!["jpg", "png", "raw"];

        for ext in extensions {
            let mut builder = tempfile::Builder::new();
            let suffix = format!(".{}", ext);
            builder.suffix(&suffix);
            let temp_file = builder.tempfile().unwrap();
            let path_str = temp_file.path().to_str().unwrap();
            let file_path = FilePath::new(path_str).unwrap();

            let result = use_case.execute(file_path).await;

            assert!(result.is_ok());
        }
    }
}
