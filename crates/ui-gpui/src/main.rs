//! O app em GPUI, ao lado do de egui.
//!
//! `cargo run -p ui-gpui` — e `cargo run -p ui` continua abrindo o de sempre.
//! Os dois leem o mesmo catálogo, ou catálogos separados via `VLB_CATALOG`
//! enquanto este aqui está em obras.
//!
//! Nesta primeira subida ele prova a pilha, e não a Biblioteca: janela aberta,
//! catálogo resolvido, banco migrado e a ponte de imagem desenhando na tela.
//! É o degrau que responde "o GPUI sobe neste projeto?" antes de qualquer
//! decisão de layout.

use gpui::{
    div, img, prelude::*, px, rgb, size, App, Application, Bounds, Context, SharedString, Window,
    WindowBounds, WindowOptions,
};
use image::{Rgba, RgbaImage};
use std::sync::Arc;

use ui_gpui::imagem::para_gpui;

struct PrimeiraLuz {
    catalogo: SharedString,
    migrations: SharedString,
    /// Desenhada pela ponte da §3.1 — é a prova visual de que ela funciona.
    amostra: Arc<gpui::RenderImage>,
}

impl Render for PrimeiraLuz {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(24.))
            .size_full()
            .bg(rgb(0x1b1b1b))
            .text_color(rgb(0xe6e6e6))
            .child(div().text_xl().child("VintageLightbox — GPUI"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .text_sm()
                    .text_color(rgb(0x9a9a9a))
                    .child(format!("catálogo: {}", self.catalogo))
                    .child(format!("banco: {}", self.migrations)),
            )
            .child(
                // A faixa vermelho / verde / azul, nesta ordem. Se a ponte
                // trocasse os canais, ela apareceria azul / verde / vermelho —
                // e é por isso que a amostra é colorida e não cinza.
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x9a9a9a))
                            .child("ponte de imagem — tem de ler vermelho, verde, azul:"),
                    )
                    .child(img(self.amostra.clone()).w(px(240.)).h(px(80.))),
            )
    }
}

/// Três faixas verticais em RGBA, para a conversão ter o que provar.
fn amostra_rgb() -> RgbaImage {
    let (largura, altura) = (240, 80);
    let mut img = RgbaImage::new(largura, altura);

    for y in 0..altura {
        for x in 0..largura {
            let cor = match x * 3 / largura {
                0 => Rgba([220, 40, 40, 255]),
                1 => Rgba([40, 200, 90, 255]),
                _ => Rgba([50, 90, 230, 255]),
            };
            img.put_pixel(x, y, cor);
        }
    }

    img
}

#[tokio::main]
async fn main() {
    // O mesmo preâmbulo do `crates/ui`, e de propósito: os dois apps abrem o
    // mesmo catálogo e rodam as mesmas migrations. Se este divergisse daquele,
    // a comparação de paridade da fase 5 mediria dois bancos diferentes.
    let catalogo = infrastructure::paths::AppPaths::catalog_root();
    if !catalogo.exists() {
        std::fs::create_dir_all(&catalogo).expect("Failed to create catalog directory");
    }

    let db_path = infrastructure::paths::AppPaths::main_db_path();
    let database_url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());

    let pool = infrastructure::create_pool(&database_url)
        .await
        .expect("Failed to create database pool");
    infrastructure::run_migrations(&pool)
        .await
        .expect("Failed to run database migrations");

    let versao: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    let catalogo_texto: SharedString = catalogo.to_string_lossy().to_string().into();
    let migrations_texto: SharedString = format!("migrations aplicadas até a v{versao}").into();

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(720.), px(420.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| PrimeiraLuz {
                    catalogo: catalogo_texto.clone(),
                    migrations: migrations_texto.clone(),
                    amostra: para_gpui(image::DynamicImage::ImageRgba8(amostra_rgb())),
                })
            },
        )
        .expect("abrir a janela");
        cx.activate(true);
    });
}
