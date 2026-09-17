//! VintageLightbox em Tauri (recordarfotos-e-commerce/docs/DESKTOP_TAURI.md).
//!
//! ```text
//! cargo run -p app-tauri                     # a tela empacotada (interface/)
//! cargo run -p app-tauri -- --diagnostico    # e a página que responde P1–P9
//! VLB_TELA_URL=http://localhost:5174 cargo run -p app-tauri   # a tela do Vite (só em debug)
//! ```

// Sem console no Windows em release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ambiente;
mod api;
mod bytes;
mod catalogo;
mod comandos;
mod depuracao;
mod erro;
#[cfg(target_os = "macos")]
mod menu;
mod navegacao;
mod origem;
mod pasta_de_saida;
mod protocolo;
mod raizes;
mod rotas_do_catalogo;
mod sincronizacao;
mod tela_do_cliente;

use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{App, Manager, WebviewUrl, WebviewWindowBuilder};

use api::ContaDoApp;

use navegacao::{decidir, tela, DESENVOLVIMENTO, ROTA_INICIAL};
use pasta_de_saida::PastaDeSaida;
use raizes::RaizesPermitidas;

fn main() {
    let diagnostico = std::env::args().any(|argumento| argumento == "--diagnostico");

    let construtor = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let construtor = construtor.menu(menu::do_app);
    construtor
        // G1: uma instância só. Ele vem primeiro, como o plugin pede: a segunda
        // abertura só traz a janela da primeira para frente, e termina.
        .plugin(tauri_plugin_single_instance::init(
            |app, _argumentos, _pasta| mostrar_principal(app),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(RaizesPermitidas::default())
        .manage(sincronizacao::Sincronizador::default())
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
            depuracao::fotografar_janela,
            api::ha_sessao,
            api::entrar,
            api::sair,
            api::chamar_api,
            sincronizacao::guardar_envio,
            sincronizacao::fila_de_envios,
            sincronizacao::tentar_envios_agora,
            sincronizacao::esquecer_envio,
            sincronizacao::envios_recusados,
            sincronizacao::pendentes_na_pagina,
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
            app.manage(ContaDoApp::nova(app.path().app_data_dir().ok()));
            if DESENVOLVIMENTO {
                if let Ok(pasta) = std::env::var("VLB_PASTA_DE_TESTE") {
                    let escolhida = app
                        .state::<PastaDeSaida>()
                        .escolher(std::path::Path::new(&pasta));
                    eprintln!("[depuração] pasta de saída de teste {pasta}: {escolhida:?}");
                }
            }

            // G2: o catálogo abre antes da janela. Se não abrir, o app diz por
            // quê e termina, em vez de trabalhar sem onde guardar.
            match abrir_catalogo(app) {
                Ok(catalogo) => {
                    if DESENVOLVIMENTO {
                        eprintln!("[catálogo] {:?}", catalogo.situacao());
                    }
                    app.manage(std::sync::Mutex::new(catalogo));
                    sincronizacao::iniciar(app.handle());
                }
                Err(erro) => {
                    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
                    eprintln!("[catálogo] {erro}");
                    let identificador = app.handle().clone();
                    app.dialog()
                        .message(erro.to_string())
                        .title("O VintageLightbox não pôde abrir o catálogo")
                        .kind(MessageDialogKind::Error)
                        .show(move |_| identificador.exit(1));
                    return Ok(());
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
        .on_window_event(|janela, evento| {
            // G9: fechar com envio na fila só esconde a janela. O laço dos
            // envios termina o app quando a fila esvazia.
            if let tauri::WindowEvent::CloseRequested { api, .. } = evento {
                if janela.label() == "principal"
                    && sincronizacao::ha_envio_pendente(janela.app_handle())
                {
                    api.prevent_close();
                    let _ = janela.hide();
                    janela
                        .app_handle()
                        .state::<sincronizacao::Sincronizador>()
                        .fechar_ao_esvaziar();
                    for (rotulo, outra) in janela.app_handle().webview_windows() {
                        if rotulo != "principal" {
                            let _ = outra.close();
                        }
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("o VintageLightbox (Tauri) não conseguiu abrir")
        .run(|_app, _evento| {
            // No macOS, clicar no ícone do Dock com a janela escondida a traz de volta.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _evento {
                mostrar_principal(_app);
            }
        });
}

/// Traz a janela principal de volta, e com ela o app deixa de terminar sozinho
/// quando a fila esvaziar.
fn mostrar_principal(app: &tauri::AppHandle) {
    if let Some(janela) = app.get_webview_window("principal") {
        app.state::<sincronizacao::Sincronizador>().manter_aberto();
        let _ = janela.unminimize();
        let _ = janela.show();
        let _ = janela.set_focus();
    }
}

/// A pasta do catálogo: `Imagens/VintageLightbox/Catalogo Tauri` (D13).
///
/// Fixa, para ser previsível. `VLB_CATALOGO_TAURI` só vale em depuração.
fn abrir_catalogo(app: &App) -> Result<catalogo::Catalogo, catalogo::ErroDoCatalogo> {
    // A pilha local tem o próprio catálogo: a fila dele leva bilhetes dela.
    let nome = if ambiente::na_pilha_local() {
        "Catalogo Tauri (pilha local)"
    } else {
        "Catalogo Tauri"
    };
    let raiz = DESENVOLVIMENTO
        .then(|| std::env::var_os("VLB_CATALOGO_TAURI"))
        .flatten()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            app.path()
                .picture_dir()
                .ok()
                .map(|imagens| imagens.join("VintageLightbox").join(nome))
        })
        .unwrap_or_else(|| std::path::PathBuf::from(nome));
    catalogo::Catalogo::abrir(&raiz)
}

fn abrir_principal(app: &App) -> tauri::Result<()> {
    // 🚫 A tela é a empacotada, nunca o site remoto (DESKTOP_TAURI §0).
    let construtor = WebviewWindowBuilder::new(app, "principal", tela(ROTA_INICIAL));
    let mut janela = ambiente::armazenamento(construtor, app.path().app_data_dir().ok())
        .title(ambiente::titulo("VintageLightbox"))
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
