//! 🖌️ O editor em camadas **sozinho**: a janela do editor para um arquivo, sem
//! conta do site, sem sessão e sem Revelação — para testar o editor sem montar
//! o app inteiro (pedido do dono, 27/set/2026).
//!
//! ```bash
//! cargo run --release -p ui-gpui --bin editor -- foto.jpg
//! cargo run --release -p ui-gpui --bin editor -- foto.NEF --catalogo /tmp/edicoes
//! ```
//!
//! - A base é a mesma do app: `base_neutra` (LibRaw para RAW, C28).
//! - O projeto e a imagem editada vão para um catálogo de edições **próprio**:
//!   `--catalogo <pasta>`, ou `VLB_CATALOG`, ou `<pasta da foto>/.editor-avulso`.
//!   Nunca o catálogo do balcão, a menos que se aponte para ele.
//! - Salvar imprime no stdout a versão salva (`salva revisão N: <arquivo>`), para
//!   um script conferir. Reabrir o mesmo arquivo traz o projeto intacto.
//!
//! **Roteiro** (como o do app, `VLB_ROTEIRO=<arquivo>` e `VLB_FOTOS=<pasta>`),
//! uma linha por passo, começando quando a foto termina de abrir:
//!
//! ```text
//! esperar 500
//! mouse apertar 0.2 0.5      # fração da foto, evento real do AppKit
//! mouse arrastar 0.8 0.5
//! mouse soltar 0.8 0.5
//! tecla 1 cmd                # ⌘S (keyCode 1)
//! foto 01-pintado
//! estado
//! fim
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::Root;
use gpui_kit::{px, size, App, AppContext, Bounds, WindowBounds, WindowOptions};
use ui_gpui::editor::porta::{Edicoes, EdicoesDoCatalogo, FotoDoEditor};
use ui_gpui::editor::{EditorDeFoto, EventoDoEditor};
use ui_gpui::tema;

/// O respiro entre dois passos do roteiro — o mesmo do app.
const RESPIRO: Duration = Duration::from_millis(120);

fn uso() -> ! {
    eprintln!("uso: editor <foto> [--catalogo <pasta>]");
    std::process::exit(2);
}

#[tokio::main]
async fn main() {
    let mut argumentos = std::env::args().skip(1);
    let Some(foto) = argumentos.next().map(PathBuf::from) else {
        uso()
    };
    let mut catalogo: Option<PathBuf> = std::env::var_os("VLB_CATALOG")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    while let Some(a) = argumentos.next() {
        match a.as_str() {
            "--catalogo" => {
                catalogo = Some(
                    argumentos
                        .next()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| uso()),
                )
            }
            _ => uso(),
        }
    }
    let foto = std::fs::canonicalize(&foto).unwrap_or_else(|e| {
        eprintln!("a foto {} não abriu: {e}", foto.display());
        std::process::exit(1);
    });
    let raiz = catalogo.unwrap_or_else(|| {
        foto.parent()
            .unwrap_or(Path::new("."))
            .join(".editor-avulso")
    });
    std::fs::create_dir_all(&raiz).expect("criar a pasta do catálogo de edições");

    let banco = raiz.join("vintage_lightbox.db");
    let pool = infrastructure::create_pool(&format!("sqlite:{}?mode=rwc", banco.display()))
        .await
        .expect("abrir o catálogo de edições");
    infrastructure::run_migrations(&pool)
        .await
        .expect("as migrations do catálogo");
    let edicoes: Arc<dyn Edicoes> = Arc::new(
        EdicoesDoCatalogo::carregar(
            raiz.clone(),
            infrastructure::database::CatalogoDeEdicoes::new(pool),
            tokio::runtime::Handle::current(),
        )
        .await,
    );
    eprintln!(
        "🖌️  editor avulso: {} — catálogo em {}",
        foto.display(),
        raiz.display()
    );

    // 🔑 Um id que nunca se confunde com uma foto do catálogo do balcão.
    let alvo = FotoDoEditor {
        id: format!("avulsa:{}", foto.display()),
        pos_venda_foto_id: None,
        nome: foto
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        caminho: foto.to_string_lossy().into_owned(),
    };
    let roteiro = std::env::var_os("VLB_ROTEIRO").map(|caminho| {
        std::fs::read_to_string(&caminho).unwrap_or_else(|e| {
            eprintln!("o roteiro {} não abriu: {e}", Path::new(&caminho).display());
            std::process::exit(1);
        })
    });
    let pasta_das_fotos = std::env::var_os("VLB_FOTOS").map(PathBuf::from);

    gpui_kit::application()
        .with_assets(ui_gpui::recursos::Recursos)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            ui_gpui::imagem::coleta::ligar(cx);
            tema::aplicar(
                tema::escolha_guardada(&tema::arquivo_da_escolha()),
                None,
                cx,
            );
            ui_gpui::editor::init(cx);

            let bounds = Bounds::centered(None, size(px(1280.), px(860.)), cx);
            let caminho = foto.clone();
            let mut editor = None;
            let janela = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        titlebar: Some(gpui_kit::TitlebarOptions {
                            title: Some(format!("Editar — {}", alvo.nome).into()),
                            ..Default::default()
                        }),
                        is_resizable: true,
                        window_decorations: ui_gpui::janela::decoracoes_ao_abrir(),
                        ..Default::default()
                    },
                    |window, cx| {
                        let ed = cx.new(|cx| {
                            EditorDeFoto::novo(
                                alvo,
                                edicoes,
                                move || infrastructure::base_neutra::base_neutra(&caminho),
                                window,
                                cx,
                            )
                        });
                        editor = Some(ed.clone());
                        cx.new(|cx| Root::new(ed, window, cx))
                    },
                )
                .expect("abrir a janela do editor");
            let editor = editor.expect("o editor");

            // O que foi salvo vai para o stdout — é por onde um script confere.
            cx.subscribe(&editor, |_ed, evento: &EventoDoEditor, _cx| {
                let EventoDoEditor::Salva { versao, .. } = evento;
                match versao {
                    Some(v) => println!("salva revisão {}: {}", v.revisao, v.arquivo.display()),
                    None => println!("salva sem efeito: a foto fica como o bruto"),
                }
            })
            .detach();
            // Fechar a janela encerra.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            if let Some(texto) = roteiro {
                let janela: gpui_kit::AnyWindowHandle = janela.into();
                let pasta = pasta_das_fotos.clone();
                cx.spawn(async move |cx| {
                    // Começa quando a foto abriu (ou falhou).
                    loop {
                        cx.background_executor().timer(RESPIRO).await;
                        let pronto = cx.update(|cx| {
                            let ed = editor.read(cx);
                            ed.pronta() || ed.falha().is_some()
                        });
                        if pronto {
                            break;
                        }
                    }
                    // Um quadro para o palco ser medido.
                    cx.background_executor()
                        .timer(Duration::from_millis(400))
                        .await;
                    for linha in texto.lines() {
                        let linha = linha.split('#').next().unwrap_or_default().trim();
                        if linha.is_empty() {
                            continue;
                        }
                        let mut partes = linha.splitn(2, ' ');
                        match (partes.next().unwrap_or_default(), partes.next()) {
                            ("esperar", Some(ms)) => {
                                let ms = ms.trim().parse().unwrap_or(0);
                                cx.background_executor()
                                    .timer(Duration::from_millis(ms))
                                    .await;
                                continue;
                            }
                            ("fim", _) => {
                                cx.update(|cx| cx.quit());
                                return;
                            }
                            _ => {}
                        }
                        let gesto = linha.to_string();
                        let pasta = pasta.clone();
                        let editor = editor.clone();
                        cx.update(|cx| {
                            let _ = janela.update(cx, |_, window, cx| {
                                editor.update(cx, |ed, cx| {
                                    ed.seguir_o_roteiro(&gesto, pasta.as_deref(), window, cx)
                                })
                            });
                        });
                        cx.background_executor().timer(RESPIRO).await;
                    }
                })
                .detach();
            }
            cx.activate(true);
        });
}
