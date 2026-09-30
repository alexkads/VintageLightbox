//! A importação de verdade de um DNG revelado no Lightroom: o bruto é o sensor,
//! e a revelação vira os parâmetros da foto.
//!
//! `#[ignore]`: o DNG é do acervo do dono e não está no repositório. Rodar com
//! `cargo test -p infrastructure --test revelacao_do_lightroom -- --ignored`.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use domain::repositories::PhotoRepository;
use domain::services::PreviewType;
use domain::value_objects::{
    FilePath, ImportMode, ImportOptions, OrganizationStrategy, RenamePattern,
};
use infrastructure::{
    cache::preview_manager::PreviewManager, create_pool, run_migrations, ExifReader,
    FileOrganizerImpl, PhotoRepositoryImpl, ThumbnailGeneratorImpl,
};
use use_cases::import_with_options::ImportProgress;
use use_cases::{ImportRequest, ImportWithOptionsUseCase};

const DNG_DO_LIGHTROOM: &str = "/Users/alexkads/Documents/FotosParaSite/_CSF7953.dng";

#[tokio::test]
#[ignore]
async fn o_dng_do_lightroom_entra_com_a_revelacao_nos_parametros() {
    if !std::path::Path::new(DNG_DO_LIGHTROOM).exists() {
        return;
    }
    let temp = tempfile::TempDir::new().unwrap();
    let origem = temp.path().join("cartao");
    let catalogo = temp.path().join("catalogo");
    let cache = temp.path().join("cache");
    for d in [&origem, &catalogo, &cache] {
        std::fs::create_dir_all(d).unwrap();
    }
    let dng = origem.join("_CSF7953.dng");
    std::fs::copy(DNG_DO_LIGHTROOM, &dng).unwrap();

    let pool = create_pool(&format!(
        "sqlite:{}?mode=rwc",
        temp.path().join("t.db").to_string_lossy()
    ))
    .await
    .unwrap();
    run_migrations(&pool).await.unwrap();
    let repositorio = Arc::new(PhotoRepositoryImpl::new(pool));
    let previews = Arc::new(PreviewManager::new_with_path(cache));
    let importacao = ImportWithOptionsUseCase::new(
        repositorio.clone(),
        Arc::new(ExifReader),
        Arc::new(ThumbnailGeneratorImpl::new()),
        previews.clone(),
        Arc::new(FileOrganizerImpl::new(catalogo)),
    )
    .com_revelacao_do_arquivo(Arc::new(
        infrastructure::revelacao_do_arquivo::LeitorDoLightroom,
    ));

    let (progresso, mut eventos) = tokio::sync::mpsc::unbounded_channel();
    let resultado = importacao
        .execute(ImportRequest {
            files: vec![FilePath::new(dng.to_str().unwrap()).unwrap()],
            options: ImportOptions {
                mode: ImportMode::Copy,
                organization: OrganizationStrategy::IntoOneFolder,
                rename_pattern: RenamePattern::KeepOriginal,
                ..Default::default()
            },
            progress_sender: progresso,
            pause_flag: Arc::new(AtomicBool::new(false)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
        })
        .await
        .unwrap();
    assert_eq!(resultado.successful, 1, "o DNG com perdas tem de entrar");

    let mut revelacao = None;
    while let Ok(evento) = eventos.try_recv() {
        if let ImportProgress::Completed {
            revelacao_do_arquivo,
            ..
        } = evento
        {
            revelacao = revelacao_do_arquivo;
        }
    }
    assert_eq!(
        revelacao,
        Some(
            [
                "aspereza do grão",
                "forma da vinheta",
                "suavidade da vinheta"
            ]
            .map(String::from)
            .to_vec()
        )
    );

    let foto = repositorio
        .find_by_id(&resultado.imported_photos[0].id())
        .await
        .unwrap()
        .unwrap();
    let ajustes = infrastructure::gpu_adjustments::ajustes_da_entidade(&foto);
    assert!(
        (ajustes.exposure - 0.46).abs() < 1e-3,
        "{}",
        ajustes.exposure
    );
    assert_eq!(ajustes.split_shadow_hue, 50.0);
    assert_eq!(ajustes.grain_amount, 34.0);
    assert_eq!(
        foto.edit_exposure().map(|e| (e * 100.0).round()),
        Some(46.0)
    );

    // 🚨 O bruto é o sensor, de pé e sem a revelação: a prévia grande é
    // retrato, e colorida (o P&B é do Lightroom, e está nos parâmetros).
    let previa = domain::services::PreviewStorage::get(&*previews, &foto.id(), PreviewType::Large)
        .unwrap()
        .expect("a prévia grande");
    let img = image::load_from_memory(&previa).unwrap().to_rgb8();
    assert!(
        img.height() > img.width(),
        "{}x{}",
        img.width(),
        img.height()
    );
    let (mut r, mut b) = (0f64, 0f64);
    for p in img.pixels() {
        r += p[0] as f64;
        b += p[2] as f64;
    }
    let n = (img.width() * img.height()) as f64;
    assert!(
        r / n - b / n > 8.0,
        "o bruto saiu cinza: a revelação foi aplicada no pixel?"
    );
}
