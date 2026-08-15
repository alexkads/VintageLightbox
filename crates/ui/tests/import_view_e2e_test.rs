//! E2E da tela de importação
//!
//! Cobre o que os testes de unidade da view não alcançam: que a grade **desenha**, que os
//! botões existem com o rótulo que o usuário lê, e que apertar "Importar" devolve os
//! caminhos marcados — o passo que antes era um `// TODO: call controller` e mandava o
//! usuário de volta para a biblioteca sem importar nada.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

use adapters::controllers::ImportController;
use domain::import_source::{DeviceRepository, ImportSource};
use domain::services::{
    FileOrganizer, MetadataExtractor, PreviewStorage, PreviewType, SourceScanner,
    ThumbnailGenerator,
};
use domain::value_objects::{
    FilePath, ImportMode, OrganizationStrategy, PhotoId, PhotoMetadata, RenamePattern,
};
use domain::DomainResult;
use infrastructure::cache::preview_manager::PreviewManager;
use ui::async_loader::AsyncThumbnailLoader;
use ui::state::{AppState, ImportCandidate};
use ui::views::import_view::{ImportAction, ImportView};

// ============================================================
// Dublês — a tela não toca disco nem banco nestes testes
// ============================================================

struct MetadataFalso;
impl MetadataExtractor for MetadataFalso {
    fn extract(&self, _path: &FilePath) -> DomainResult<PhotoMetadata> {
        Ok(PhotoMetadata::default())
    }
}

struct ThumbFalso;
#[async_trait::async_trait]
impl ThumbnailGenerator for ThumbFalso {
    async fn generate(&self, _path: &FilePath, _max: u32) -> DomainResult<Vec<u8>> {
        Ok(vec![])
    }
}

struct PreviewFalso;
impl PreviewStorage for PreviewFalso {
    fn save(&self, _id: &PhotoId, _tipo: PreviewType, _bytes: &[u8]) -> DomainResult<()> {
        Ok(())
    }
    fn get(&self, _id: &PhotoId, _tipo: PreviewType) -> DomainResult<Option<Vec<u8>>> {
        Ok(None)
    }
    fn has(&self, _id: &PhotoId, _tipo: PreviewType) -> DomainResult<bool> {
        Ok(false)
    }
    fn delete(&self, _id: &PhotoId) -> DomainResult<()> {
        Ok(())
    }
}

struct OrganizadorFalso;
#[async_trait::async_trait]
impl FileOrganizer for OrganizadorFalso {
    async fn organize_file(
        &self,
        source: &FilePath,
        _metadata: Option<&PhotoMetadata>,
        _strategy: OrganizationStrategy,
        _rename: RenamePattern,
    ) -> DomainResult<FilePath> {
        Ok(source.clone())
    }
}

struct DispositivosFalsos;
#[async_trait::async_trait]
impl DeviceRepository for DispositivosFalsos {
    async fn get_mounted_devices(&self) -> Vec<ImportSource> {
        vec![]
    }
    async fn get_history(&self) -> Vec<ImportSource> {
        vec![]
    }
    async fn add_to_history(&self, _path: std::path::PathBuf) {}
}

struct ScannerFalso;
#[async_trait::async_trait]
impl SourceScanner for ScannerFalso {
    async fn scan(&self, _root: &str, _include: bool) -> DomainResult<Vec<FilePath>> {
        Ok(vec![])
    }
}

mod repo {
    use super::*;
    use domain::entities::Photo;
    use domain::repositories::PhotoRepository;

    pub struct PhotoRepoFalso;

    #[async_trait::async_trait]
    impl PhotoRepository for PhotoRepoFalso {
        async fn save(&self, _photo: &Photo) -> DomainResult<()> {
            Ok(())
        }
        async fn find_by_id(&self, _id: &PhotoId) -> DomainResult<Option<Photo>> {
            Ok(None)
        }
        async fn find_all(&self) -> DomainResult<Vec<Photo>> {
            Ok(vec![])
        }
        async fn update(&self, _photo: &Photo) -> DomainResult<()> {
            Ok(())
        }
        async fn delete(&self, _id: &PhotoId) -> DomainResult<()> {
            Ok(())
        }
        async fn exists(&self, _id: &PhotoId) -> DomainResult<bool> {
            Ok(false)
        }
        async fn find_by_content_hash(&self, _hash: &str) -> DomainResult<Option<Photo>> {
            Ok(None)
        }
    }
}

fn controlador() -> Arc<ImportController> {
    let photo_repo = Arc::new(repo::PhotoRepoFalso);
    let metadata = Arc::new(MetadataFalso);
    let thumb = Arc::new(ThumbFalso);
    let preview = Arc::new(PreviewFalso);
    let organizador = Arc::new(OrganizadorFalso);

    Arc::new(ImportController::new(
        Arc::new(use_cases::ImportPhotoUseCase::new(
            photo_repo.clone(),
            metadata.clone(),
            thumb.clone(),
            preview.clone(),
        )),
        Arc::new(use_cases::CheckDuplicatesUseCase::new(photo_repo.clone())),
        Arc::new(use_cases::ImportWithOptionsUseCase::new(
            photo_repo.clone(),
            metadata.clone(),
            thumb.clone(),
            preview.clone(),
            organizador,
        )),
        Arc::new(use_cases::GetImportSourcesUseCase::new(Arc::new(
            DispositivosFalsos,
        ))),
        Arc::new(use_cases::ScanSourceUseCase::new(Arc::new(ScannerFalso))),
        Arc::new(use_cases::DescribeCandidatesUseCase::new(metadata)),
    ))
}

/// Estado com uma origem escolhida e alguns arquivos já listados
fn estado_com_grade() -> AppState {
    let mut state = AppState::new();
    state.import_view_state.open = true;

    // Origens já pedidas: sem isso a tela dispara um `tokio::spawn`, e o harness do
    // kittest não roda dentro de um runtime.
    state.import_view_state.sources_requested = true;
    state.import_view_state.selected_source_path = Some("/cartao".to_string());

    state.import_view_state.candidates = vec![
        ImportCandidate::from_path("/cartao/IMG_0001.CR2".to_string()),
        ImportCandidate::from_path("/cartao/IMG_0002.CR2".to_string()),
        ImportCandidate::from_path("/cartao/IMG_0003.JPG".to_string()),
    ];

    state
}

struct Cenario {
    state: Rc<RefCell<AppState>>,
    acao: Rc<RefCell<Option<ImportAction>>>,
}

fn montar(state: AppState) -> (Harness<'static>, Cenario) {
    #[allow(unused_mut)]
    let state = Rc::new(RefCell::new(state));
    let acao: Rc<RefCell<Option<ImportAction>>> = Rc::new(RefCell::new(None));

    let controller = controlador();
    let (sender, _receiver) = tokio::sync::mpsc::channel(32);

    let temp = tempfile::TempDir::new().unwrap();
    let preview_manager = Arc::new(PreviewManager::new_with_path(temp.path().to_path_buf()));
    let mut thumbnails = AsyncThumbnailLoader::new(preview_manager);

    let state_ui = state.clone();
    let acao_ui = acao.clone();

    // Janela do tamanho de uma real: com 800px os dois painéis laterais comem o meio, e o
    // retrato mostraria uma grade de uma coluna que ninguém vai ver na prática.
    let harness = Harness::builder()
        .with_size(egui::Vec2::new(1440.0, 900.0))
        .build(move |ctx| {
        // `temp` fica vivo enquanto o harness existir — o cache de previews aponta para ele
        let _ = &temp;

        let mut state = state_ui.borrow_mut();
        if let Some(a) = ImportView::show(ctx, &mut state, &controller, &sender, &mut thumbnails) {
            *acao_ui.borrow_mut() = Some(a);
        }
        });

    // A fonte de ícones é instalada pelo app em `VintageLightboxApp::new`; sem ela aqui,
    // todo ícone do Phosphor vira quadradinho e o retrato mentiria sobre a tela.
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    harness.ctx.set_fonts(fonts);

    (harness, Cenario { state, acao })
}

#[test]
fn grade_mostra_o_total_marcado_e_o_botao_de_importar() {
    let (mut harness, _cenario) = montar(estado_com_grade());
    harness.run();

    // Tudo entra marcado, como no Lightroom
    harness.get_by_label("Importar 3 fotos");
    harness.get_by_label("Marcar todas");
    harness.get_by_label("Desmarcar todas");
}

#[test]
fn importar_devolve_os_caminhos_marcados() {
    let (mut harness, cenario) = montar(estado_com_grade());
    harness.run();

    harness.get_by_label("Importar 3 fotos").click();
    harness.run();

    let acao = cenario.acao.borrow();
    match acao.as_ref() {
        Some(ImportAction::Import { files, options }) => {
            assert_eq!(files.len(), 3);
            assert!(files.contains(&"/cartao/IMG_0001.CR2".to_string()));
            // A raiz da origem viaja junto — é o que `PreserveStructure` precisa
            assert_eq!(options.source_root.as_deref(), Some("/cartao"));
        }
        outro => panic!("esperava uma ação de importar, veio {:?}", outro.is_some()),
    }
}

#[test]
fn desmarcar_todas_desabilita_a_importacao() {
    let (mut harness, cenario) = montar(estado_com_grade());
    harness.run();

    harness.get_by_label("Desmarcar todas").click();
    harness.run();

    assert_eq!(cenario.state.borrow().import_view_state.checked_count(), 0);

    // O botão continua na tela, agora dizendo zero — e desabilitado
    harness.get_by_label("Importar 0 fotos").click();
    harness.run();

    assert!(
        cenario.acao.borrow().is_none(),
        "clicar no botão desabilitado não pode disparar importação nenhuma"
    );
}

#[test]
fn cancelar_devolve_a_acao_de_cancelar() {
    let (mut harness, cenario) = montar(estado_com_grade());
    harness.run();

    harness.get_by_label("Cancelar").click();
    harness.run();

    assert!(matches!(
        cenario.acao.borrow().as_ref(),
        Some(ImportAction::Cancel)
    ));
}

#[test]
fn modo_move_avisa_que_vai_apagar_os_originais() {
    let (mut harness, cenario) = montar(estado_com_grade());
    harness.run();

    harness.get_by_label("Move").click();
    harness.run();

    assert_eq!(
        cenario.state.borrow().import_view_state.options.mode,
        ImportMode::Move
    );
    harness.get_by_label("Os originais serão apagados da origem");
}

#[test]
fn modo_add_esconde_o_destino() {
    let (mut harness, cenario) = montar(estado_com_grade());
    harness.run();

    harness.get_by_label("Add").click();
    harness.run();

    assert_eq!(
        cenario.state.borrow().import_view_state.options.mode,
        ImportMode::Add
    );

    // Sem cópia não há para onde copiar: o painel troca de conteúdo em vez de mostrar
    // um destino que não seria usado.
    harness.get_by_label("As fotos serão catalogadas onde estão. Nada é copiado, nada é movido.");
}

#[test]
fn sem_origem_escolhida_a_grade_orienta_em_vez_de_ficar_vazia() {
    let mut state = AppState::new();
    state.import_view_state.open = true;
    state.import_view_state.sources_requested = true;

    let (mut harness, _cenario) = montar(state);
    harness.run();

    harness.get_by_label("Escolha um cartão ou uma pasta à esquerda");
}

/// Retrato da tela — não é asserção, é o que permite olhar o layout sem abrir o app
///
/// Roda só quando pedido (`VLB_SNAPSHOT=1`), porque renderização de fonte varia entre
/// máquinas e um snapshot obrigatório viraria falha intermitente no CI.
#[test]
fn retrato_da_tela() {
    if std::env::var("VLB_SNAPSHOT").is_err() {
        return;
    }

    let (mut harness, _cenario) = montar(estado_com_grade());
    harness.run();
    harness.snapshot("import_view");
}
