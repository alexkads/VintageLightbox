//! VintageLightbox em Tauri — Fase 0 (recordarfotos-e-commerce/docs/DESKTOP_TAURI.md).
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
mod raizes;

use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{App, Manager, Url, WebviewUrl, WebviewWindowBuilder};

use navegacao::{destino, Destino, ROTA_INICIAL, SITE};
use raizes::RaizesPermitidas;

/// A pilha local só existe em build de depuração (§6, regra 1).
const DESENVOLVIMENTO: bool = cfg!(debug_assertions);

fn main() {
    let diagnostico = std::env::args().any(|argumento| argumento == "--diagnostico");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(RaizesPermitidas::default())
        .invoke_handler(tauri::generate_handler![
            comandos::escolher_raw,
            comandos::ler_raw
        ])
        .setup(move |app| {
            if DESENVOLVIMENTO {
                app.add_capability(include_str!("../capacidades-dev/local.json"))?;
            }
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

/// O endereço que a janela principal abre.
///
/// `VLB_SITE_URL` só vale em depuração: o binário do balcão abre produção, sempre.
fn endereco_inicial() -> Url {
    let site = if DESENVOLVIMENTO {
        std::env::var("VLB_SITE_URL").unwrap_or_else(|_| SITE.to_string())
    } else {
        SITE.to_string()
    };
    let mut url = Url::parse(&site).expect("VLB_SITE_URL não é um endereço");
    url.set_path(ROTA_INICIAL);
    url
}

fn abrir_principal(app: &App) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "principal", WebviewUrl::External(endereco_inicial()))
        .title("VintageLightbox")
        .inner_size(1440.0, 900.0)
        .maximized(true)
        .on_navigation(decidir)
        .on_new_window(|url, _recursos| {
            // 🧪 Fase 0: a tela do cliente ainda abre por `window.open`, pelo
            // caminho padrão do webview, e P9 mede se ele serve. A Fase 1 troca
            // por `abrir_tela_do_cliente`, com a janela criada aqui.
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

/// Aplica a regra de navegação, abrindo no navegador do sistema o que é de fora.
fn decidir(url: &Url) -> bool {
    match destino(url, DESENVOLVIMENTO) {
        Destino::NaJanela => true,
        Destino::NoNavegador => {
            let _ = tauri_plugin_opener::open_url(url.as_str(), None::<&str>);
            false
        }
        Destino::Recusado => false,
    }
}
