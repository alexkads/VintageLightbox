//! E2E dos modos de importação, com arquivos de verdade no disco
//!
//! `Add`, `Copy` e `Move` decidem o que acontece com o original — e `Move` **apaga** o
//! arquivo na origem. Erro aqui não dá tela feia, dá foto perdida. Por isso estes testes
//! escrevem JPEGs reais num diretório temporário e conferem o disco depois, em vez de
//! confiar em mock de organizador.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use domain::value_objects::{FilePath, ImportMode, ImportOptions, OrganizationStrategy, RenamePattern};
use image::{ImageBuffer, Rgb};
use infrastructure::{
    cache::preview_manager::PreviewManager, create_pool, run_migrations, ExifReader,
    FileOrganizerImpl, PhotoRepositoryImpl, ThumbnailGeneratorImpl,
};
use tempfile::TempDir;
use tokio::sync::mpsc;
use use_cases::{ImportRequest, ImportWithOptionsUseCase};

/// Monta a pilha real de importação sobre um catálogo temporário
struct Bancada {
    _temp: TempDir,
    origem: std::path::PathBuf,
    catalogo: std::path::PathBuf,
    use_case: ImportWithOptionsUseCase,
}

async fn bancada() -> Bancada {
    let temp = TempDir::new().unwrap();

    let origem = temp.path().join("cartao");
    let catalogo = temp.path().join("catalogo");
    std::fs::create_dir_all(&origem).unwrap();
    std::fs::create_dir_all(&catalogo).unwrap();

    let db_path = temp.path().join("test.db");
    let pool = create_pool(&format!("sqlite:{}?mode=rwc", db_path.to_string_lossy()))
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();

    let cache = temp.path().join("cache");
    std::fs::create_dir_all(&cache).unwrap();

    let use_case = ImportWithOptionsUseCase::new(
        Arc::new(PhotoRepositoryImpl::new(pool)),
        Arc::new(ExifReader),
        Arc::new(ThumbnailGeneratorImpl::new()),
        Arc::new(PreviewManager::new_with_path(cache)),
        Arc::new(FileOrganizerImpl::new(catalogo.clone())),
    );

    Bancada {
        _temp: temp,
        origem,
        catalogo,
        use_case,
    }
}

/// Escreve um JPEG de verdade — cor única, para o hash de conteúdo diferir
fn jpeg(dir: &std::path::Path, nome: &str, semente: u8) -> FilePath {
    use std::io::{Cursor, Write};

    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(nome);

    let img = ImageBuffer::from_pixel(64, 64, Rgb([semente, 128, 200]));
    let dynamic = image::DynamicImage::ImageRgb8(img);

    let mut bytes = Vec::new();
    dynamic
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .unwrap();

    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();

    FilePath::new(path.to_str().unwrap()).unwrap()
}

async fn importar(bancada: &Bancada, files: Vec<FilePath>, options: ImportOptions) -> usize {
    let (progress_sender, mut rx) = mpsc::unbounded_channel();

    let request = ImportRequest {
        files,
        options,
        progress_sender,
        pause_flag: Arc::new(AtomicBool::new(false)),
        cancel_flag: Arc::new(AtomicBool::new(false)),
    };

    let resultado = bancada.use_case.execute(request).await.unwrap();

    // Drenar o canal para não deixar a task de progresso pendurada
    while rx.try_recv().is_ok() {}

    resultado.successful
}

#[tokio::test]
async fn modo_add_cataloga_sem_copiar_nada() {
    let bancada = bancada().await;
    let foto = jpeg(&bancada.origem, "IMG_0001.jpg", 10);

    let sucesso = importar(
        &bancada,
        vec![foto.clone()],
        ImportOptions::default().with_mode(ImportMode::Add),
    )
    .await;

    assert_eq!(sucesso, 1);

    let original: &std::path::Path = foto.as_ref();
    assert!(original.exists(), "Add não pode mexer no original");

    // Nada foi criado no catálogo — o arquivo é referenciado onde está
    let no_catalogo: Vec<_> = walk(&bancada.catalogo);
    assert!(
        no_catalogo.is_empty(),
        "Add não copia: encontrado {:?}",
        no_catalogo
    );
}

#[tokio::test]
async fn modo_copy_duplica_e_preserva_o_original() {
    let bancada = bancada().await;
    let foto = jpeg(&bancada.origem, "IMG_0002.jpg", 20);

    let sucesso = importar(
        &bancada,
        vec![foto.clone()],
        ImportOptions::default().with_mode(ImportMode::Copy),
    )
    .await;

    assert_eq!(sucesso, 1);

    let original: &std::path::Path = foto.as_ref();
    assert!(original.exists(), "Copy mantém o original");
    assert_eq!(walk(&bancada.catalogo).len(), 1, "e cria uma cópia");
}

#[tokio::test]
async fn modo_move_apaga_o_original_so_depois_de_copiar() {
    let bancada = bancada().await;
    let foto = jpeg(&bancada.origem, "IMG_0003.jpg", 30);

    let sucesso = importar(
        &bancada,
        vec![foto.clone()],
        ImportOptions::default().with_mode(ImportMode::Move),
    )
    .await;

    assert_eq!(sucesso, 1);

    let original: &std::path::Path = foto.as_ref();
    assert!(!original.exists(), "Move apaga o original");

    let copias = walk(&bancada.catalogo);
    assert_eq!(copias.len(), 1, "e a foto tem de estar no catálogo");
    assert!(copias[0].exists());
}

#[tokio::test]
async fn destino_escolhido_vence_o_catalogo_padrao() {
    let bancada = bancada().await;
    let foto = jpeg(&bancada.origem, "IMG_0004.jpg", 40);

    let destino = bancada._temp.path().join("hd_externo");

    let sucesso = importar(
        &bancada,
        vec![foto],
        ImportOptions::default()
            .with_mode(ImportMode::Copy)
            .with_destination(Some(destino.to_string_lossy().to_string())),
    )
    .await;

    assert_eq!(sucesso, 1);
    assert_eq!(walk(&destino).len(), 1, "a foto foi para o destino escolhido");
    assert!(
        walk(&bancada.catalogo).is_empty(),
        "e o catálogo padrão não foi tocado"
    );
}

#[tokio::test]
async fn preservar_subpastas_recria_a_hierarquia_a_partir_da_raiz_da_origem() {
    let bancada = bancada().await;
    let foto = jpeg(&bancada.origem.join("DCIM/100CANON"), "IMG_0005.jpg", 50);

    let sucesso = importar(
        &bancada,
        vec![foto],
        ImportOptions {
            organization: OrganizationStrategy::PreserveStructure,
            rename_pattern: RenamePattern::KeepOriginal,
            ..ImportOptions::default()
        }
        .with_source_root(Some(bancada.origem.to_string_lossy().to_string())),
    )
    .await;

    assert_eq!(sucesso, 1);

    let esperado = bancada.catalogo.join("DCIM/100CANON/IMG_0005.jpg");
    assert!(
        esperado.exists(),
        "esperava {:?}, achei {:?}",
        esperado,
        walk(&bancada.catalogo)
    );
}

#[tokio::test]
async fn sem_raiz_de_origem_preservar_subpastas_cai_em_pasta_unica() {
    let bancada = bancada().await;
    let foto = jpeg(&bancada.origem.join("DCIM/100CANON"), "IMG_0006.jpg", 60);

    // Sem `source_root` não há como saber que pedaço do caminho absoluto preservar —
    // recriar `/private/var/folders/.../cartao/DCIM/...` dentro do catálogo seria pior
    // do que achatar.
    let sucesso = importar(
        &bancada,
        vec![foto],
        ImportOptions {
            organization: OrganizationStrategy::PreserveStructure,
            rename_pattern: RenamePattern::KeepOriginal,
            ..ImportOptions::default()
        },
    )
    .await;

    assert_eq!(sucesso, 1);
    assert!(bancada.catalogo.join("IMG_0006.jpg").exists());
}

#[tokio::test]
async fn numa_pasta_so_ignora_a_data_e_a_hierarquia() {
    let bancada = bancada().await;
    let a = jpeg(&bancada.origem.join("dia1"), "IMG_0007.jpg", 70);
    let b = jpeg(&bancada.origem.join("dia2"), "IMG_0008.jpg", 80);

    let sucesso = importar(
        &bancada,
        vec![a, b],
        ImportOptions {
            organization: OrganizationStrategy::IntoOneFolder,
            rename_pattern: RenamePattern::KeepOriginal,
            ..ImportOptions::default()
        }
        .with_source_root(Some(bancada.origem.to_string_lossy().to_string())),
    )
    .await;

    assert_eq!(sucesso, 2);
    assert!(bancada.catalogo.join("IMG_0007.jpg").exists());
    assert!(bancada.catalogo.join("IMG_0008.jpg").exists());
}

/// Lista recursivamente os arquivos de um diretório (vazio se ele nem existe)
fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut encontrados = Vec::new();
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return encontrados;
    };

    for entrada in entradas.flatten() {
        let path = entrada.path();
        if path.is_dir() {
            encontrados.extend(walk(&path));
        } else {
            encontrados.push(path);
        }
    }

    encontrados
}
