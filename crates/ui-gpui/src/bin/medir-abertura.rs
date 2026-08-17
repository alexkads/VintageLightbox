//! Quanto tempo o app fica sem janela ao abrir.
//!
//! # Por que isto existe
//!
//! A fase 1 deixou um item em aberto — *"carregamento assíncrono das fotos (hoje
//! bloqueia a abertura)"* — e ele atravessou as fases 2, 3 e 4 sem que ninguém
//! medisse **quanto** ele bloqueia. Dívida sem número é dívida que não dá para
//! priorizar: 30 ms e 3 s pedem decisões opostas, e as duas se parecem na
//! descrição.
//!
//! O `main.rs` faz quatro coisas antes de `open_window`, e este binário mede as
//! quatro na mesma ordem:
//!
//! 1. abrir o banco (`create_pool`);
//! 2. rodar as migrations;
//! 3. ler **todas** as fotos (`get_all_photos`);
//! 4. ler os presets.
//!
//! ```bash
//! VLB_CATALOG=/tmp/catalogo-de-medicao cargo run --release -p ui-gpui --bin medir-abertura
//! ```
//!
//! ⚠️ **Só vale em `--release`**, como toda medida deste projeto: a fase 1 quase
//! condenou o GPUI medindo fluidez em `debug`, onde decodificar JPEG custa 56×
//! mais.

use std::sync::Arc;
use std::time::Instant;

use infrastructure::paths::AppPaths;

/// O tempo em que uma janela que ainda não apareceu passa a ser notada.
///
/// Não é um número inventado: é a faixa clássica de resposta de interface — até
/// ~100 ms a ação parece instantânea; a partir daí aparece a espera. Serve para
/// o binário dar um veredito em vez de só um número.
const NOTAVEL_MS: f64 = 100.0;

#[tokio::main]
async fn main() {
    let catalogo = AppPaths::catalog_root();
    let db_path = AppPaths::main_db_path();
    let url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());

    println!(
        "perfil: {}",
        if cfg!(debug_assertions) {
            "debug ⚠️  — o número abaixo não vale para decidir nada"
        } else {
            "release"
        }
    );
    println!("catálogo: {}", catalogo.display());

    let tudo = Instant::now();

    let marca = Instant::now();
    let pool = infrastructure::create_pool(&url)
        .await
        .expect("abrir o banco");
    let abrir_banco = marca.elapsed();

    let marca = Instant::now();
    infrastructure::run_migrations(&pool)
        .await
        .expect("rodar as migrations");
    let migrations = marca.elapsed();

    let repositorio = Arc::new(infrastructure::PhotoRepositoryImpl::new(pool.clone()));
    let biblioteca = adapters::controllers::LibraryController::new(repositorio);

    let marca = Instant::now();
    let fotos = biblioteca
        .get_all_photos()
        .await
        .expect("ler as fotos do catálogo");
    let ler_fotos = marca.elapsed();

    let presets_repo = Arc::new(infrastructure::SqlitePresetRepository::new(pool.clone()));
    let controlador = adapters::controllers::PresetController::new(
        Arc::new(use_cases::presets::ListPresetsUseCase::new(
            presets_repo.clone(),
        )),
        Arc::new(use_cases::presets::SavePresetUseCase::new(
            presets_repo.clone(),
        )),
        Arc::new(use_cases::presets::DeletePresetUseCase::new(presets_repo)),
    );

    let marca = Instant::now();
    let presets = controlador.list_presets().await.unwrap_or_default();
    let ler_presets = marca.elapsed();

    let total = tudo.elapsed();

    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;

    println!("\n{} fotos, {} presets", fotos.len(), presets.len());
    println!("  abrir o banco   {:>8.1} ms", ms(abrir_banco));
    println!("  migrations      {:>8.1} ms", ms(migrations));
    println!("  ler as fotos    {:>8.1} ms", ms(ler_fotos));
    println!("  ler os presets  {:>8.1} ms", ms(ler_presets));
    println!("  ───────────────────────────");
    println!("  antes da janela {:>8.1} ms", ms(total));

    if fotos.is_empty() {
        println!("\n⚠️  Catálogo vazio — semeie antes, senão o número mede o nada.");
        return;
    }

    println!(
        "\n  por foto        {:>8.3} ms",
        ms(ler_fotos) / fotos.len() as f64
    );

    if ms(total) > NOTAVEL_MS {
        println!(
            "\n🚨 A janela demora {:.0} ms para aparecer — acima dos {NOTAVEL_MS:.0} ms em que a \
             espera passa a ser notada. O carregamento assíncrono deixa de ser dívida teórica.",
            ms(total)
        );
    } else {
        println!(
            "\n✅ {:.0} ms até a janela, abaixo dos {NOTAVEL_MS:.0} ms em que a espera aparece. \
             Carregar de forma assíncrona **não** é o que falta aqui.",
            ms(total)
        );
    }
}
