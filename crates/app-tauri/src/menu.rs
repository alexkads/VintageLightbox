//! O menu do app, em português e só com o que o app usa.
//!
//! Sem isto, o Tauri monta no macOS o menu padrão em inglês (File, Edit,
//! View, Window, Help), que o dono chamou de "visual sem acabamento"
//! (2026-09-16).
//!
//! 🚨 **O menu Editar não é enfeite.** No macOS, Cmd+C, Cmd+V, Cmd+X, Cmd+A e
//! Cmd+Z chegam aos campos da página pelos itens dele: sem o menu, copiar e
//! colar param de funcionar.
//!
//! Só no macOS (`main.rs`): no Windows e no Linux o menu iria para dentro da
//! janela, e lá a tela do site já tem tudo o que o operador usa.

use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Runtime};

pub fn do_app<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let nome = "VintageLightbox";
    let sobre = AboutMetadata {
        name: Some(nome.into()),
        version: Some(app.package_info().version.to_string()),
        copyright: Some("RecordarFotos".into()),
        ..Default::default()
    };
    let separador = || PredefinedMenuItem::separator(app);

    let aplicativo = Submenu::with_items(
        app,
        nome,
        true,
        &[
            &PredefinedMenuItem::about(app, Some(&format!("Sobre o {nome}")), Some(sobre))?,
            &separador()?,
            &PredefinedMenuItem::hide(app, Some(&format!("Ocultar o {nome}")))?,
            &PredefinedMenuItem::hide_others(app, Some("Ocultar os outros"))?,
            &PredefinedMenuItem::show_all(app, Some("Mostrar todos"))?,
            &separador()?,
            &PredefinedMenuItem::quit(app, Some(&format!("Sair do {nome}")))?,
        ],
    )?;
    let editar = Submenu::with_items(
        app,
        "Editar",
        true,
        &[
            &PredefinedMenuItem::undo(app, Some("Desfazer"))?,
            &PredefinedMenuItem::redo(app, Some("Refazer"))?,
            &separador()?,
            &PredefinedMenuItem::cut(app, Some("Recortar"))?,
            &PredefinedMenuItem::copy(app, Some("Copiar"))?,
            &PredefinedMenuItem::paste(app, Some("Colar"))?,
            &PredefinedMenuItem::select_all(app, Some("Selecionar tudo"))?,
        ],
    )?;
    let janela = Submenu::with_items(
        app,
        "Janela",
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some("Minimizar"))?,
            &PredefinedMenuItem::maximize(app, Some("Zoom"))?,
            &PredefinedMenuItem::fullscreen(app, Some("Tela cheia"))?,
            &separador()?,
            &PredefinedMenuItem::close_window(app, Some("Fechar janela"))?,
        ],
    )?;
    Menu::with_items(app, &[&aplicativo, &editar, &janela])
}
