//! VintageLightbox em Tauri (recordarfotos-e-commerce/docs/DESKTOP_TAURI.md).
//!
//! ```text
//! cargo run -p app-tauri                     # a janela no site de produção
//! cargo run -p app-tauri -- --diagnostico    # e a página que responde P1–P9
//! VLB_SITE_URL=http://localhost:3001 cargo run -p app-tauri   # pilha local (só em debug)
//! ```

// Sem console no Windows em release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod comandos;
mod erro;
mod navegacao;
mod pasta_de_saida;
mod raizes;
mod tela_do_cliente;

use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{App, Manager, WebviewUrl, WebviewWindowBuilder};

use navegacao::{decidir, endereco, DESENVOLVIMENTO, ROTA_INICIAL};
use pasta_de_saida::PastaDeSaida;
use raizes::RaizesPermitidas;

fn main() {
    let diagnostico = std::env::args().any(|argumento| argumento == "--diagnostico");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(RaizesPermitidas::default())
        .invoke_handler(tauri::generate_handler![
            comandos::escolher_raw,
            comandos::ler_raw,
            comandos::pasta_de_saida,
            comandos::escolher_pasta,
            comandos::esquecer_pasta,
            comandos::nomes_na_pasta,
            comandos::gravar_na_pasta,
            comandos::abrir_tela_do_cliente,
            comandos::fechar_tela_do_cliente,
        ])
        .setup(move |app| {
            if DESENVOLVIMENTO {
                app.add_capability(include_str!("../capacidades-dev/local.json"))?;
            }
            let registro = app
                .path()
                .app_config_dir()
                .ok()
                .map(|pasta| pasta.join("pasta-de-saida.txt"));
            app.manage(PastaDeSaida::carregar(registro));

            abrir_principal(app)?;
            if diagnostico {
                WebviewWindowBuilder::new(app, "diagnostico", WebviewUrl::App("index.html".into()))
                    .title("VintageLightbox — diagnóstico da Fase 0")
                    .inner_size(760.0, 900.0)
                    .build()?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("o VintageLightbox (Tauri) não conseguiu abrir");
}

fn abrir_principal(app: &App) -> tauri::Result<()> {
    WebviewWindowBuilder::new(
        app,
        "principal",
        WebviewUrl::External(endereco(ROTA_INICIAL)),
    )
    .title("VintageLightbox")
    .inner_size(1440.0, 900.0)
    .maximized(true)
    .on_navigation(decidir)
    .on_new_window(|url, _recursos| {
        // A tela do cliente não passa por aqui: no desktop ela nasce em
        // `abrir_tela_do_cliente`. O que sobra é pop-up da própria página.
        if decidir(&url) {
            NewWindowResponse::Allow
        } else {
            NewWindowResponse::Deny
        }
    })
    .on_download(|webview, evento| {
        // Sem destino, o webview de alguns sistemas descarta o download em
        // silêncio. A reserva da web é a pasta de downloads, e aqui também.
        if let DownloadEvent::Requested { url, destination } = evento {
            if let Ok(pasta) = webview.app_handle().path().download_dir() {
                let nome = destination
                    .file_name()
                    .map(|n| n.to_os_string())
                    .or_else(|| {
                        url.path_segments()
                            .and_then(|mut s| s.next_back().map(Into::into))
                    })
                    .unwrap_or_else(|| "download".into());
                *destination = pasta.join(nome);
            }
        }
        true
    })
    .build()?;
    Ok(())
}
