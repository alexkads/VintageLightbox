//! A importação pelo caminho de verdade: disco, banco e os controllers.
//!
//! # Por que isto existe
//!
//! Os 24 testes da importação usam **dublês** (`ExploradorDeMentira`,
//! `ImportadorDeMentira`): eles conferem a máquina de estados da tela — as
//! corridas, a ordem dos pedidos, o que cada tecla faz — e **nenhum deles toca
//! no disco**. Quer dizer que a fase 3 podia estar inteira e a importação, na
//! máquina de quem usa, não importar nada.
//!
//! Aqui não há dublê: os arquivos existem, o banco é um SQLite de verdade, e
//! quem responde são o `ExploradorDoDisco` e o `ImportadorDoDisco` que o
//! `main.rs` monta.
//!
//! ⚠️ **O que continua fora**: o seletor nativo de pasta, que é janela do
//! sistema e não pode aparecer na máquina de quem roda a suíte.
//!
//! 🚨 **E os testes são `multi_thread` de propósito.** O `#[tokio::test]` padrão
//! roda numa thread só; como aqui se espera o recado com um `recv_timeout`
//! **bloqueante**, essa thread fica presa e a tarefa que produziria o recado
//! nunca roda. O primeiro rascunho falhou nos três por isso — e o susto foi
//! justo: era o teste, não o app. O `main.rs` usa `#[tokio::main]`, que é
//! multi-thread.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use image::{DynamicImage, Rgba, RgbaImage};
use tempfile::TempDir;

use ui_gpui::importacao::estado::Recado;
use ui_gpui::importacao::explorador::{
    Andamento, Explorador, ExploradorDoDisco, Importador, ImportadorDoDisco,
};

/// Uma foto de verdade em disco — JPEG, como sai de uma câmera.
fn gravar_jpeg(pasta: &std::path::Path, nome: &str) -> String {
    // ⚠️ **O conteúdo varia com o nome**, e não é firula: a importação pula
    // duplicatas por **hash do arquivo**, então duas fotos idênticas entrariam
    // como uma só — e o teste acusaria a duplicata como se fosse defeito da
    // importação. Foi o que aconteceu no primeiro rascunho: 1 de 2.
    let semente = nome.bytes().map(u32::from).sum::<u32>();
    let mut imagem = RgbaImage::new(64, 48);
    for (x, y, pixel) in imagem.enumerate_pixels_mut() {
        *pixel = Rgba([
            ((x * 4 + semente) % 256) as u8,
            ((y * 5 + semente) % 256) as u8,
            120,
            255,
        ]);
    }

    let caminho = pasta.join(nome);
    DynamicImage::ImageRgba8(imagem)
        .to_rgb8()
        .save(&caminho)
        .expect("gravar o JPEG");

    caminho.to_string_lossy().to_string()
}

/// O que o `main.rs` monta, com o banco e o destino num diretório descartável.
async fn montar() -> (
    Arc<ExploradorDoDisco>,
    Arc<ImportadorDoDisco>,
    Arc<infrastructure::PhotoRepositoryImpl>,
    TempDir,
) {
    let dir = TempDir::new().expect("diretório temporário");
    let url = format!(
        "sqlite:{}?mode=rwc",
        dir.path().join("catalogo.db").to_string_lossy()
    );

    let pool = infrastructure::create_pool(&url)
        .await
        .expect("abrir o banco");
    infrastructure::run_migrations(&pool)
        .await
        .expect("rodar as migrations");

    let repositorio = Arc::new(infrastructure::PhotoRepositoryImpl::new(pool.clone()));
    let extrator = Arc::new(infrastructure::ExifReader);
    let miniaturas = Arc::new(infrastructure::ThumbnailGeneratorImpl::new());
    let organizador = Arc::new(infrastructure::FileOrganizerImpl::new(
        dir.path().join("destino"),
    ));
    let dispositivos =
        Arc::new(infrastructure::devices::repository::InfrastructureDeviceRepository::new());
    let previews = Arc::new(
        infrastructure::cache::preview_manager::PreviewManager::new_with_path(
            dir.path().join("previews"),
        ),
    );

    let importacao = Arc::new(adapters::controllers::ImportController::new(
        Arc::new(use_cases::ImportPhotoUseCase::new(
            repositorio.clone(),
            extrator.clone(),
            miniaturas.clone(),
            previews.clone(),
        )),
        Arc::new(use_cases::CheckDuplicatesUseCase::new(repositorio.clone())),
        Arc::new(use_cases::ImportWithOptionsUseCase::new(
            repositorio.clone(),
            extrator.clone(),
            miniaturas,
            previews,
            organizador,
        )),
        Arc::new(use_cases::GetImportSourcesUseCase::new(dispositivos)),
        Arc::new(use_cases::ScanSourceUseCase::new(Arc::new(
            infrastructure::SourceScannerImpl::new(),
        ))),
        Arc::new(use_cases::DescribeCandidatesUseCase::new(extrator)),
    ));

    let handle = tokio::runtime::Handle::current();
    (
        Arc::new(ExploradorDoDisco::novo(importacao.clone(), handle.clone())),
        Arc::new(ImportadorDoDisco::novo(importacao, handle)),
        repositorio,
        dir,
    )
}

/// Espera um recado chegar, com teto — a tela real espera acordando a cada
/// 100 ms, e aqui a espera é a mesma coisa sem o laço da interface.
fn esperar(canal: &Receiver<Recado>) -> Recado {
    canal
        .recv_timeout(Duration::from_secs(20))
        .expect("o recado tinha de chegar — 20 s é muito mais do que o caminho leva")
}

fn canal_de_recados() -> (Sender<Recado>, Receiver<Recado>) {
    channel()
}

/// 🚨 Varrer uma pasta de verdade responde com os arquivos dela.
#[tokio::test(flavor = "multi_thread")]
async fn varrer_uma_pasta_lista_as_fotos() {
    let (explorador, _importador, _repositorio, dir) = montar().await;

    let origem = dir.path().join("cartao");
    std::fs::create_dir_all(&origem).expect("criar a origem");
    gravar_jpeg(&origem, "DSC_0001.jpg");
    gravar_jpeg(&origem, "DSC_0002.jpg");

    let (envio, recepcao) = canal_de_recados();
    explorador.varrer(origem.to_string_lossy().to_string(), false, envio);

    match esperar(&recepcao) {
        Recado::Varrido { arquivos, .. } => {
            assert_eq!(arquivos.len(), 2, "as duas fotos da pasta");
        }
        outro => panic!("esperava a varredura, veio {outro:?}"),
    }
}

/// 🚨 Descrever lê o arquivo e devolve tamanho e tipo.
#[tokio::test(flavor = "multi_thread")]
async fn detalhar_traz_o_que_a_grade_mostra() {
    let (explorador, _importador, _repositorio, dir) = montar().await;

    let origem = dir.path().join("cartao");
    std::fs::create_dir_all(&origem).expect("criar a origem");
    let foto = gravar_jpeg(&origem, "DSC_0001.jpg");

    let (envio, recepcao) = canal_de_recados();
    explorador.detalhar(vec![foto.clone()], envio);

    match esperar(&recepcao) {
        Recado::Descritos(itens) => {
            assert_eq!(itens.len(), 1);
            assert_eq!(itens[0].caminho, foto);
            assert!(itens[0].tamanho > 0, "o tamanho vem do disco");
            assert!(!itens[0].e_raw, "um JPEG não é RAW");
        }
        outro => panic!("esperava as descrições, veio {outro:?}"),
    }
}

/// 🚨 **Importar de verdade põe *todas* as fotos no catálogo.**
///
/// É o teste que faltava, e ele achou um defeito no primeiro dia: com **doze**
/// fotos, entravam onze — e a que sobrava vinha com
/// `Not enough bytes, expected 2 but found 0` no lugar da miniatura.
///
/// A causa estava no `FileOrganizerImpl`: o nome do destino era escolhido por
/// *"não existe? então é meu"*, e a importação roda **oito arquivos em
/// paralelo**. Dois perguntavam junto, os dois ouviam "não existe", e os dois
/// copiavam para o mesmo caminho — uma foto por cima da outra, e a que estava
/// sendo lida no meio da cópia virava arquivo pela metade.
///
/// 🔑 **Doze, e não duas**: com duas o encontro é raro e o teste passaria quase
/// sempre. Doze acima do teto de oito garante disputa em toda execução.
#[tokio::test(flavor = "multi_thread")]
async fn importar_poe_todas_as_fotos_no_catalogo() {
    use domain::repositories::PhotoRepository;

    const QUANTAS: usize = 12;

    let (_explorador, importador, repositorio, dir) = montar().await;

    let origem = dir.path().join("cartao");
    std::fs::create_dir_all(&origem).expect("criar a origem");
    let arquivos: Vec<String> = (1..=QUANTAS)
        .map(|i| gravar_jpeg(&origem, &format!("DSC_{i:04}.jpg")))
        .collect();

    let antes = repositorio.find_all().await.expect("ler o catálogo");
    assert!(antes.is_empty(), "o catálogo começa vazio");

    let (envio, recepcao) = channel::<Andamento>();
    let opcoes = domain::value_objects::ImportOptions {
        source_root: Some(origem.to_string_lossy().to_string()),
        ..Default::default()
    };
    importador.importar(arquivos, opcoes, envio);

    // A importação vai mandando andamento; o que interessa é o fim.
    let mut falhas = Vec::new();
    let mut terminou = false;
    for _ in 0..(QUANTAS * 4) {
        let andamento = recepcao
            .recv_timeout(Duration::from_secs(30))
            .expect("o andamento tinha de chegar");
        match andamento {
            Andamento::Falhou { caminho, erro } => falhas.push(format!("{caminho}: {erro}")),
            Andamento::Terminou { .. } => {
                terminou = true;
                break;
            }
            _ => {}
        }
    }
    assert!(terminou, "a importação tinha de terminar");
    assert!(falhas.is_empty(), "nenhuma foto podia falhar: {falhas:#?}");

    let depois = repositorio.find_all().await.expect("ler o catálogo");
    assert_eq!(
        depois.len(),
        QUANTAS,
        "as {QUANTAS} fotos tinham de estar no catálogo depois de importar"
    );

    // E cada uma foi para um arquivo **próprio** no destino.
    let caminhos: std::collections::HashSet<String> = depois
        .iter()
        .map(|foto| foto.file_path().as_ref().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        caminhos.len(),
        QUANTAS,
        "duas fotos apontando para o mesmo arquivo é uma sobrescrevendo a outra"
    );
}
