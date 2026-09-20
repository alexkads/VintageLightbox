//! Quanto custa pôr uma miniatura na tela.
//!
//! # Por que isto existe
//!
//! "Não está fluido" é uma observação de quem usa, e o critério da fase 1 é
//! **2.000 fotos a 60fps**. 60fps são **16,7 ms por quadro** — e todo o
//! trabalho de revelar uma linha nova durante a rolagem cabe nesse orçamento,
//! ou não cabe.
//!
//! Este binário mede o caminho inteiro de uma miniatura: consulta ao SQLite,
//! decodificação do JPEG e conversão para BGRA. Com o número em mãos dá para
//! responder se o problema é o perfil de compilação, o volume de trabalho por
//! quadro, ou o desenho.
//!
//! ```bash
//! VLB_CATALOG=/tmp/catalogo-de-medicao cargo run -p ui-gpui --bin medir-miniaturas
//! VLB_CATALOG=/tmp/catalogo-de-medicao cargo run --release -p ui-gpui --bin medir-miniaturas
//! ```

use std::num::NonZeroUsize;
use std::time::Instant;

use infrastructure::cache::preview_manager::PreviewManager;
use infrastructure::paths::AppPaths;
use ui_gpui::biblioteca::miniaturas::CacheDeMiniaturas;

/// O orçamento de um quadro a 60fps.
const ORCAMENTO_MS: f64 = 16.7;

#[tokio::main]
async fn main() {
    let quantas: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(120);

    let db_path = AppPaths::main_db_path();
    let url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
    let pool = infrastructure::create_pool(&url)
        .await
        .expect("abrir o banco");

    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM photos LIMIT ?1")
        .bind(quantas as i64)
        .fetch_all(&pool)
        .await
        .expect("ler os ids");

    if ids.is_empty() {
        eprintln!("❌ catálogo vazio — use o semear-catalogo antes.");
        std::process::exit(1);
    }

    println!(
        "perfil: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    println!("miniaturas medidas: {}", ids.len());

    let previews = PreviewManager::new();
    // Capacidade folgada: aqui se mede o custo de **carregar**, e um descarte no
    // meio da medição mediria o cache, não a decodificação.
    let mut cache = CacheDeMiniaturas::nova(NonZeroUsize::new(ids.len().max(1)).unwrap());

    let comeco = Instant::now();
    for id in &ids {
        cache.obter(&previews, id);
    }
    let total = comeco.elapsed();

    let por_miniatura = total.as_secs_f64() * 1000.0 / ids.len() as f64;
    println!("total: {:.1} ms", total.as_secs_f64() * 1000.0);
    println!("por miniatura: {por_miniatura:.2} ms");

    // Quantas cabem num quadro é a pergunta que decide a rolagem: revelar uma
    // linha nova custa uma miniatura por coluna.
    let cabem = ORCAMENTO_MS / por_miniatura;
    println!("cabem em 16,7 ms: {cabem:.1} miniaturas");

    if cabem < 6.0 {
        println!(
            "🚨 uma linha de 6 colunas NÃO cabe num quadro — a rolagem engasga \
             ao revelar linha nova"
        );
    } else {
        println!("✅ uma linha de 6 colunas cabe num quadro");
    }

    // A segunda passada mede o cache quente: é o custo de rolar de volta por
    // onde já se passou, e tem de ser praticamente zero.
    let comeco = Instant::now();
    for id in &ids {
        cache.obter(&previews, id);
    }
    let quente = comeco.elapsed().as_secs_f64() * 1000.0 / ids.len() as f64;
    println!("por miniatura, já em cache: {quente:.4} ms");
}
