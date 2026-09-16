//! VintageLightbox em Tauri (recordarfotos-e-commerce/docs/DESKTOP_TAURI.md).
//!
//! ```text
//! cargo run -p app-tauri                     # a tela empacotada (interface/)
//! cargo run -p app-tauri -- --diagnostico    # e a página que responde P1–P9
//! VLB_TELA_URL=http://localhost:5174 cargo run -p app-tauri   # a tela do Vite (só em debug)
//! ```

// Sem console no Windows em release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod bytes;
mod comandos;
mod depuracao;
mod erro;
mod navegacao;
mod origem;
mod pasta_de_saida;
mod protocolo;
mod raizes;
mod tela_do_cliente;

use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{App, Manager, WebviewUrl, WebviewWindowBuilder};

use api::ContaDoApp;

use navegacao::{decidir, tela, DESENVOLVIMENTO, ROTA_INICIAL};
use pasta_de_saida::PastaDeSaida;
use raizes::RaizesPermitidas;

fn main() {
    let diagnostico = std::env::args().any(|argumento| argumento == "--diagnostico");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(RaizesPermitidas::default())
        .manage(ContaDoApp::nova())
        // A tela empacotada e as rotas internas dela (`protocolo.rs`).
        .register_asynchronous_uri_scheme_protocol(
            protocolo::ESQUEMA,
            |contexto, pedido, responder| {
                let app = contexto.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    responder.respond(protocolo::tratar(&app, pedido).await);
                });
            },
        )
        .invoke_handler(tauri::generate_handler![
            comandos::escolher_raw,
            comandos::ler_raw,
            comandos::converter_raw,
            comandos::cartoes_montados,
            comandos::escolher_origem,
            comandos::listar_origem,
            comandos::ler_da_origem,
            comandos::pasta_de_saida,
            comandos::escolher_pasta,
            comandos::esquecer_pasta,
            comandos::nomes_na_pasta,
            comandos::gravar_na_pasta,
            comandos::abrir_tela_do_cliente,
            comandos::fechar_tela_do_cliente,
            depuracao::registrar_no_terminal,
            api::ha_sessao,
            api::entrar,
            api::sair,
            api::chamar_api,
        ])
        .setup(move |app| {
            if DESENVOLVIMENTO {
                app.add_capability(include_str!("../capacidades-dev/local.json"))?;
                app.add_capability(include_str!("../capacidades-dev/depuracao-local.json"))?;
                // Um roteiro não abre seletor: a pasta de teste já nasce permitida.
                if let Ok(pasta) = std::env::var("VLB_ORIGEM_DE_TESTE") {
                    let permitida = app
                        .state::<RaizesPermitidas>()
                        .permitir_pasta(std::path::Path::new(&pasta));
                    eprintln!("[depuração] origem de teste {pasta}: {permitida:?}");
                }
            }
            let registro = app
                .path()
                .app_config_dir()
                .ok()
                .map(|pasta| pasta.join("pasta-de-saida.txt"));
            app.manage(PastaDeSaida::carregar(registro));
            if DESENVOLVIMENTO {
                if let Ok(pasta) = std::env::var("VLB_PASTA_DE_TESTE") {
                    let escolhida = app
                        .state::<PastaDeSaida>()
                        .escolher(std::path::Path::new(&pasta));
                    eprintln!("[depuração] pasta de saída de teste {pasta}: {escolhida:?}");
                }
            }

            abrir_principal(app)?;
            if diagnostico {
                let mut janela = WebviewWindowBuilder::new(
                    app,
                    "diagnostico",
                    WebviewUrl::App("diagnostico.html".into()),
                )
                .title("VintageLightbox — diagnóstico da Fase 0")
                .inner_size(760.0, 900.0);
                if DESENVOLVIMENTO {
                    janela = janela.initialization_script(depuracao::CONSOLE_NO_TERMINAL);
                }
                janela.build()?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("o VintageLightbox (Tauri) não conseguiu abrir");
}

fn abrir_principal(app: &App) -> tauri::Result<()> {
    // 🚫 A tela é a empacotada, nunca o site remoto (DESKTOP_TAURI §0).
    let mut janela = WebviewWindowBuilder::new(app, "principal", tela(ROTA_INICIAL))
        .title("VintageLightbox")
        .inner_size(1440.0, 900.0)
        .maximized(true)
        .on_navigation(decidir)
        // 🚨 **O arrastar e soltar é da página.** Com o tratador do Tauri ligado (o
        // padrão), o webview não entrega o `drop` ao HTML, e soltar fotos na
        // galeria deixaria de funcionar.
        .disable_drag_drop_handler()
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
        });
    if DESENVOLVIMENTO {
        janela = janela
            .initialization_script(depuracao::CONSOLE_NO_TERMINAL)
            .on_page_load(|janela, carga| {
                depuracao::rodar_roteiro(&janela, carga.event(), carga.url().as_str())
            });
    }
    janela.build()?;
    Ok(())
}
