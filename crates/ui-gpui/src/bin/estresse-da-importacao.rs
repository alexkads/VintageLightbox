//! Estresse da importação com o balcão trabalhando em cima.
//!
//! # Por que isto existe
//!
//! *"40 fotos de 24 MB, 12 ficaram para trás"* (26/set/2026). A importação
//! sozinha passava limpa — 40 de 40 em 9 s —, e o defeito só aparecia com o
//! resto do balcão ligado: a receita padrão revelando cada foto que entra
//! (preset **e** corte), a segunda tela mostrando a foto grande ao cliente,
//! nota e bandeira sendo dadas, a releitura do catálogo e a cópia em segundo
//! plano passando as fotos do rascunho para a sessão. Foi aqui que apareceram
//! "151 trocas para 150 fotos" e "49 de 50 cortes": a troca regravava a foto
//! inteira com a leitura velha. O pedido do dono foi este teste: *"várias
//! importações de 50 fotos de 24 MB e nesse momento mostrar para o cliente no
//! segundo monitor e ir classificando e sinalizando"*.
//!
//! Tudo aqui são as peças de verdade do `main.rs` — SQLite, cache de prévias,
//! gerador, organizador, `ReceitaPadrao` na GPU —, num catálogo à parte.
//!
//! ```bash
//! VLB_CATALOG=/tmp/estresse cargo run --release -p ui-gpui \
//!     --bin estresse-da-importacao -- <pasta com as fotos> [levas] [fila|juntas] [sem-balcao]
//! ```
//!
//! O veredito está no fim: cada foto na sessão dela, nenhuma presa no
//! rascunho, uma troca por foto, e os números de erro de cada gesto.
//!
//! ⚠️ **Recusa um catálogo que já tem fotos**: o caminho vem de `VLB_CATALOG`,
//! e um engano apontaria para a biblioteca real.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use domain::value_objects::{ImportMode, ImportOptions, OrganizationStrategy, RenamePattern};
use infrastructure::cache::preview_manager::PreviewManager;
use infrastructure::paths::AppPaths;

#[derive(Default)]
struct Conta {
    ok: AtomicUsize,
    erro: AtomicUsize,
    ultimo: Mutex<Option<String>>,
}
impl Conta {
    fn anotar<T, E: std::fmt::Display>(&self, r: Result<T, E>) {
        match r {
            Ok(_) => self.ok.fetch_add(1, Ordering::Relaxed),
            Err(e) => {
                *self.ultimo.lock().unwrap() = Some(e.to_string());
                self.erro.fetch_add(1, Ordering::Relaxed)
            }
        };
    }
    fn mostrar(&self, nome: &str) {
        println!(
            "  {nome:<14} ok {:>6}  erro {:>4}  {}",
            self.ok.load(Ordering::Relaxed),
            self.erro.load(Ordering::Relaxed),
            self.ultimo.lock().unwrap().clone().unwrap_or_default()
        );
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let pasta = std::env::args().nth(1).expect("pasta das fotos");
    let levas: usize = std::env::args()
        .nth(2)
        .map(|n| n.parse().unwrap())
        .unwrap_or(3);
    let juntas = std::env::args().nth(3).as_deref() == Some("juntas");
    let balcao = std::env::args().nth(4).as_deref() != Some("sem-balcao");
    let mut arquivos: Vec<String> = std::fs::read_dir(&pasta)
        .unwrap()
        .flatten()
        .map(|e| e.path().to_string_lossy().to_string())
        .filter(|c| c.ends_with(".jpg"))
        .collect();
    arquivos.sort();
    println!(
        "{} arquivos × {levas} levas ({}), balcão {}",
        arquivos.len(),
        if juntas { "ao mesmo tempo" } else { "em fila" },
        if balcao { "trabalhando" } else { "parado" }
    );

    let catalogo = AppPaths::catalog_root();
    assert!(
        !AppPaths::main_db_path().exists(),
        "{} já tem um catálogo: use VLB_CATALOG com uma pasta nova",
        catalogo.display()
    );
    std::fs::create_dir_all(&catalogo).unwrap();
    let url = format!(
        "sqlite:{}?mode=rwc",
        AppPaths::main_db_path().to_string_lossy()
    );
    let pool = infrastructure::create_pool(&url).await.unwrap();
    infrastructure::run_migrations(&pool).await.unwrap();
    let fotos = Arc::new(infrastructure::PhotoRepositoryImpl::new(pool.clone()));
    let extrator = Arc::new(infrastructure::ExifReader);
    let miniaturas = Arc::new(infrastructure::ThumbnailGeneratorImpl::new());
    let organizador = Arc::new(infrastructure::FileOrganizerImpl::new(catalogo.clone()));
    let previews = Arc::new(PreviewManager::new());
    let dispositivos =
        Arc::new(infrastructure::devices::repository::InfrastructureDeviceRepository::new());
    let importacao = Arc::new(adapters::controllers::ImportController::new(
        Arc::new(use_cases::ImportPhotoUseCase::new(
            fotos.clone(),
            extrator.clone(),
            miniaturas.clone(),
            previews.clone(),
        )),
        Arc::new(use_cases::CheckDuplicatesUseCase::new(fotos.clone())),
        Arc::new(use_cases::ImportWithOptionsUseCase::new(
            fotos.clone(),
            extrator.clone(),
            miniaturas,
            previews.clone(),
            organizador,
        )),
        Arc::new(use_cases::GetImportSourcesUseCase::new(dispositivos)),
        Arc::new(use_cases::ScanSourceUseCase::new(Arc::new(
            infrastructure::SourceScannerImpl::new(),
        ))),
        Arc::new(use_cases::DescribeCandidatesUseCase::new(extrator)),
    ));
    let biblioteca = Arc::new(adapters::controllers::LibraryController::new(fotos.clone()));
    let marcador = Arc::new(adapters::controllers::PhotoController::new(
        Arc::new(use_cases::RatePhotoUseCase::new(fotos.clone())),
        Arc::new(use_cases::SetColorLabelUseCase::new(fotos.clone())),
        Arc::new(use_cases::SetFlagUseCase::new(fotos.clone())),
        Arc::new(use_cases::DeletePhotoUseCase::new(fotos.clone())),
        Arc::new(use_cases::MarcarCompradaUseCase::new(fotos.clone())),
    ));

    // 🎨 A receita padrão de verdade: preset + corte 3:2, revelada na GPU a
    // partir da prévia de 2560, gravada pelo mesmo Gravador do main.rs.
    let editor = Arc::new(adapters::controllers::EditorController::new(
        Arc::new(use_cases::SavePhotoEditsUseCase::new(fotos.clone())),
        Arc::new(use_cases::pos_venda::RevelacoesLocaisUseCase::new(
            Arc::new(infrastructure::SqliteRevelacoesDoSite::new(pool.clone())),
        )),
    ));
    use ui_gpui::revelacao::persistencia::Gravador;
    let gravador: Arc<dyn Gravador> =
        Arc::new(ui_gpui::revelacao::persistencia::GravadorDoBanco::novo(
            editor,
            tokio::runtime::Handle::current(),
            Vec::new(),
        ));
    let (avisos, reveladas) = std::sync::mpsc::channel::<String>();
    let servico = Arc::new(ui_gpui::sessoes::receita_padrao::ReceitaPadrao::nova(
        previews.clone(),
        gravador.clone(),
        avisos,
    ));
    let mut preset_ajustes = ui_gpui::sessoes::nova::receita::ajustes_da_receita(None);
    preset_ajustes.exposure = 0.4;
    preset_ajustes.contrast = 1.2;
    let usar_receita = balcao;

    // O que entrou, para o balcão ter em que mexer.
    let entradas: Arc<Mutex<Vec<String>>> = Arc::default();
    let acabou = Arc::new(AtomicBool::new(false));
    let (tela2, nota, bandeira, releitura, troca, importar) = (
        Arc::new(Conta::default()),
        Arc::new(Conta::default()),
        Arc::new(Conta::default()),
        Arc::new(Conta::default()),
        Arc::new(Conta::default()),
        Arc::new(Conta::default()),
    );
    let sorteio = Arc::new(AtomicUsize::new(7));
    let sortear = {
        let sorteio = sorteio.clone();
        move |n: usize| {
            let s = sorteio.fetch_add(0x9E37_79B9, Ordering::Relaxed);
            (s.wrapping_mul(2654435761) >> 7) % n.max(1)
        }
    };

    let mut trabalhos = Vec::new();
    let fim_da_copia = Arc::new(AtomicBool::new(false));
    let movidas = Arc::new(AtomicUsize::new(0));
    let trabalho_da_troca;
    let laco = |periodo_ms: u64,
                acabou: Arc<AtomicBool>,
                f: Box<
        dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync,
    >| {
        tokio::spawn(async move {
            while !acabou.load(Ordering::Relaxed) {
                f().await;
                tokio::time::sleep(Duration::from_millis(periodo_ms)).await;
            }
        })
    };

    if balcao {
        // 🖥️ Segunda tela: o cliente vendo a foto grande.
        {
            let (entradas, previews, conta, sortear) = (
                entradas.clone(),
                previews.clone(),
                tela2.clone(),
                sortear.clone(),
            );
            trabalhos.push(laco(
                250,
                acabou.clone(),
                Box::new(move || {
                    let (entradas, previews, conta, sortear) = (
                        entradas.clone(),
                        previews.clone(),
                        conta.clone(),
                        sortear.clone(),
                    );
                    Box::pin(async move {
                        let id = {
                            let e = entradas.lock().unwrap();
                            if e.is_empty() {
                                return;
                            }
                            e[sortear(e.len())].clone()
                        };
                        let r = tokio::task::spawn_blocking(move || {
                            previews.get_preview(&id).map(|_| ()).ok_or("sem prévia")
                        })
                        .await
                        .unwrap();
                        conta.anotar(r);
                    })
                }),
            ));
        }
        // ⭐ Classificar e 🚩 sinalizar.
        for (periodo, conta, e_nota) in
            [(120u64, nota.clone(), true), (170, bandeira.clone(), false)]
        {
            let (entradas, marcador, sortear) =
                (entradas.clone(), marcador.clone(), sortear.clone());
            trabalhos.push(laco(
                periodo,
                acabou.clone(),
                Box::new(move || {
                    let (entradas, marcador, conta, sortear) = (
                        entradas.clone(),
                        marcador.clone(),
                        conta.clone(),
                        sortear.clone(),
                    );
                    Box::pin(async move {
                        let id = {
                            let e = entradas.lock().unwrap();
                            if e.is_empty() {
                                return;
                            }
                            e[sortear(e.len())].clone()
                        };
                        let r = if e_nota {
                            marcador.rate_photo(&id, 1 + sortear(5) as i32).await
                        } else {
                            marcador.set_flag(&id, 1).await
                        };
                        conta.anotar(r);
                    })
                }),
            ));
        }
        // 🔁 A releitura do catálogo que a raiz pede a cada foto.
        {
            let (biblioteca, conta) = (biblioteca.clone(), releitura.clone());
            trabalhos.push(laco(
                200,
                acabou.clone(),
                Box::new(move || {
                    let (biblioteca, conta) = (biblioteca.clone(), conta.clone());
                    Box::pin(async move { conta.anotar(biblioteca.get_all_photos().await) })
                }),
            ));
        }
    }
    // 🚚 A cópia em segundo plano: o que entra no rascunho passa para a sessão.
    {
        let (biblioteca, conta, movidas) = (biblioteca.clone(), troca.clone(), movidas.clone());
        trabalho_da_troca = Some(laco(
            400,
            fim_da_copia.clone(),
            Box::new(move || {
                let (biblioteca, conta, movidas) =
                    (biblioteca.clone(), conta.clone(), movidas.clone());
                Box::pin(async move {
                    for n in 0..8 {
                        let r = biblioteca
                            .trocar_sessao(&format!("rascunho:{n}"), &format!("g{n}"))
                            .await;
                        if let Ok(k) = &r {
                            movidas.fetch_add(*k, Ordering::Relaxed);
                        }
                        conta.anotar(r);
                    }
                })
            }),
        ));
    }

    let inicio = Instant::now();
    let mut lotes = Vec::new();
    for n in 0..levas {
        let (gravador, servico) = (gravador.clone(), servico.clone());
        let (importacao, arquivos, entradas, conta, catalogo) = (
            importacao.clone(),
            arquivos.clone(),
            entradas.clone(),
            importar.clone(),
            catalogo.clone(),
        );
        let lote = async move {
            let id = format!("rascunho:{n}");
            let opcoes = ImportOptions {
                sessao_id: Some(id.clone()),
                mode: ImportMode::Copy,
                destination: Some(
                    catalogo
                        .join("Ensaios")
                        .join(id.replace(':', "-"))
                        .to_string_lossy()
                        .into_owned(),
                ),
                organization: OrganizationStrategy::IntoOneFolder,
                rename_pattern: RenamePattern::Uuid,
                ..Default::default()
            };
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let leitor = tokio::spawn(async move {
                use adapters::view_models::ImportProgressViewModel as Vm;
                while let Some(ev) = rx.recv().await {
                    match ev {
                        Vm::Completed { photo_id, .. } => {
                            conta.anotar(Ok::<_, String>(()));
                            if usar_receita {
                                // O `aplicar_receita` da nova sessão, foto a foto.
                                let corte = ui_gpui::sessoes::nova::receita::corte_centralizado(Some("3:2"), 0, 0);
                                gravador.gravar(photo_id.clone(), preset_ajustes, corte);
                                servico.pedir(photo_id.clone(), preset_ajustes, Some("3:2".into()), (preset_ajustes, corte));
                            }
                            entradas.lock().unwrap().push(photo_id)
                        }
                        Vm::Failed { path, error } => conta.anotar(Err::<(), _>(format!("{path}: {error}"))),
                        Vm::Finished { successful, failed, skipped } => println!(
                            "{:>6.1}s leva {n}: {successful} ok, {failed} falhas, {skipped} puladas",
                            inicio.elapsed().as_secs_f32()
                        ),
                        _ => {}
                    }
                }
            });
            let _ = importacao
                .import_with_options(arquivos, opcoes, tx, Arc::default(), Arc::default())
                .await;
            let _ = leitor.await;
        };
        if juntas {
            lotes.push(tokio::spawn(lote));
        } else {
            lote.await;
        }
    }
    for l in lotes {
        let _ = l.await;
    }
    // A cópia acabou: a última troca (o "de_novo" do assistente) e o
    // assistente larga. O balcão segue classificando por mais 5 s.
    fim_da_copia.store(true, Ordering::Relaxed);
    if let Some(t) = trabalho_da_troca {
        let _ = t.await;
    }
    for n in 0..levas {
        if let Ok(k) = biblioteca
            .trocar_sessao(&format!("rascunho:{n}"), &format!("g{n}"))
            .await
        {
            movidas.fetch_add(k, Ordering::Relaxed);
        }
    }
    while servico.progresso().andando() {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    println!(
        "{:>6.1}s receita padrão terminou: {:?}",
        inicio.elapsed().as_secs_f32(),
        servico.progresso()
    );
    tokio::time::sleep(Duration::from_secs(5)).await;
    acabou.store(true, Ordering::Relaxed);
    for t in trabalhos {
        let _ = t.await;
    }

    println!("\n⏱  {:.1}s", inicio.elapsed().as_secs_f32());
    importar.mostrar("importar");
    tela2.mostrar("segunda tela");
    nota.mostrar("nota");
    bandeira.mostrar("bandeira");
    println!(
        "  {:<14} {} avisos de revelada",
        "receita",
        reveladas.try_iter().count()
    );
    releitura.mostrar("releitura");
    troca.mostrar("troca");

    // O veredito: cada foto importada tem de estar na sessão dela.
    let todas = biblioteca.get_all_photos().await.unwrap();
    let mut por_sessao = std::collections::BTreeMap::<String, usize>::new();
    for f in &todas {
        *por_sessao
            .entry(f.sessao_id.clone().unwrap_or("—".into()))
            .or_default() += 1;
    }
    println!("\nfotos por sessão no fim: {por_sessao:?}");
    let presas = todas
        .iter()
        .filter(|f| {
            f.sessao_id
                .as_deref()
                .is_some_and(|s| s.starts_with("rascunho:"))
        })
        .count();
    println!("presas no rascunho depois da última troca: {presas}");
    println!(
        "trocas rascunho→sessão: {} para {} fotos importadas (a mais = foto que voltou ao rascunho)",
        movidas.load(Ordering::Relaxed),
        importar.ok.load(Ordering::Relaxed)
    );
}
