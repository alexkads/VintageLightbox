//! O menu do app no macOS: um só, com o nome do app (dono, 2026-09-16: sem
//! "Editar" e sem "Janela").
//!
//! Sem isto, o Tauri monta o menu padrão em inglês (File, Edit, View, Window,
//! Help).
//!
//! 🚨 **Copiar e colar moram aqui dentro.** No macOS, Cmd+C, Cmd+V, Cmd+X,
//! Cmd+A e Cmd+Z só chegam aos campos da página por um item de menu. Sem um
//! menu "Editar", eles ficam no menu do app; tirá-los tiraria os atalhos.
//!
//! Só no macOS (`main.rs`): no Windows e no Linux o menu iria para dentro da
//! janela, e lá a tela do site já tem tudo o que o operador usa.

use objc2_foundation::{NSString, NSUserDefaults};
use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu};

/// Os itens que o macOS põe sozinho no menu que tem "Copiar" e "Colar", e o
/// valor que os tira. Achados no AppKit do macOS 26 e conferidos pelo System
/// Events (2026-09-16).
const SEM_ITENS_DO_SISTEMA: &[(&str, bool)] = &[
    ("NSDisabledDictationMenuItem", true),
    ("NSDisabledCharacterPaletteMenuItem", true),
    ("NSAutoFillSystemInsertMenuEnabled", false),
    ("NSAutoFillOrSystemInsertMenuEnabled", false),
    ("NSAllowsWritingTools", false),
];

/// Tira do menu o Ditado, os Emojis, o AutoFill e o Writing Tools. Precisa
/// rodar antes de o app terminar de abrir, que é quando o AppKit os põe.
pub fn sem_itens_do_sistema() {
    let preferencias = NSUserDefaults::standardUserDefaults();
    for (chave, valor) in SEM_ITENS_DO_SISTEMA {
        preferencias.setBool_forKey(*valor, &NSString::from_str(chave));
    }
}
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
            &PredefinedMenuItem::undo(app, Some("Desfazer"))?,
            &PredefinedMenuItem::redo(app, Some("Refazer"))?,
            &PredefinedMenuItem::cut(app, Some("Recortar"))?,
            &PredefinedMenuItem::copy(app, Some("Copiar"))?,
            &PredefinedMenuItem::paste(app, Some("Colar"))?,
            &PredefinedMenuItem::select_all(app, Some("Selecionar tudo"))?,
            &separador()?,
            &PredefinedMenuItem::hide(app, Some(&format!("Ocultar o {nome}")))?,
            &PredefinedMenuItem::hide_others(app, Some("Ocultar os outros"))?,
            &PredefinedMenuItem::show_all(app, Some("Mostrar todos"))?,
            &separador()?,
            &PredefinedMenuItem::quit(app, Some(&format!("Sair do {nome}")))?,
        ],
    )?;
    Menu::with_items(app, &[&aplicativo])
}
