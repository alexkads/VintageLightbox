//! O app em GPUI, ao lado do de egui.
//!
//! `cargo run -p ui-gpui` — e `cargo run -p ui` continua abrindo o de sempre.
//! Os dois leem o mesmo catálogo, ou catálogos separados via `VLB_CATALOG`
//! enquanto este aqui está em obras.

use std::sync::Arc;

use gpui::{px, size, App, AppContext, Application, Bounds, WindowBounds, WindowOptions};
use infrastructure::cache::preview_manager::PreviewManager;
use infrastructure::paths::AppPaths;

use ui_gpui::biblioteca::tela::Biblioteca;

#[tokio::main]
async fn main() {
    // O mesmo preâmbulo do `crates/ui`, e de propósito: os dois apps abrem o
    // mesmo catálogo e rodam as mesmas migrations. Se este divergisse daquele,
    // a comparação de paridade da fase 5 mediria dois bancos diferentes.
    let catalogo = AppPaths::catalog_root();
    if !catalogo.exists() {
        std::fs::create_dir_all(&catalogo).expect("Failed to create catalog directory");
    }

    let db_path = AppPaths::main_db_path();
    let database_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());

    let pool = infrastructure::create_pool(&database_url)
        .await
        .expect("Failed to create database pool");
    infrastructure::run_migrations(&pool)
        .await
        .expect("Failed to run database migrations");

    // As quatro camadas internas, intactas — este app fala com elas do mesmo
    // jeito que o de egui fala. É o que tornou o GPUI mais barato que o Tauri:
    // nada precisou virar comando serializável.
    let repositorio = Arc::new(infrastructure::PhotoRepositoryImpl::new(pool.clone()));
    let biblioteca = adapters::controllers::LibraryController::new(repositorio);

    // As fotos são carregadas **antes** da janela, e isso é provisório: num
    // acervo grande a abertura fica esperando o banco. A fase 1 termina com
    // isso assíncrono — mas o carregamento síncrono é o que permite medir os
    // 60fps da rolagem sem confundir com o tempo de abertura.
    let fotos = biblioteca
        .get_all_photos()
        .await
        .expect("ler as fotos do catálogo");

    let previews = Arc::new(PreviewManager::new());

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Biblioteca::nova(fotos.clone(), previews.clone())),
        )
        .expect("abrir a janela");
        cx.activate(true);
    });
}
