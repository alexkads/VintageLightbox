//! O menu do app no macOS: um só, com o nome do app, como o do app Tauri
//! (`app-tauri/src/menu.rs`, dono, 2026-09-16: sem "Editar" e sem "Janela").
//!
//! Até 2026-09-17 este app não tinha menu nenhum (ver `encerramento`): no
//! macOS sobrava o nome do binário na barra, sem "Sobre" e sem "Sair".
//!
//! ⚠️ **Sem Copiar, Colar e Desfazer**, ao contrário do Tauri. Lá eles moram
//! no menu porque o webview só recebe `Cmd+C` por um item de menu; aqui o
//! campo de texto do `gpui-component` tem as próprias teclas, e um item de
//! menu com `Cmd+Z` disputaria a tecla com o desfazer da revelação.
//!
//! No Windows e no Linux o GPUI não desenha menu de app, e nada aqui aparece.

use gpui::{actions, App, Menu, MenuItem};

actions!(
    vintagelightbox_menu,
    [Sobre, Ocultar, OcultarOsOutros, MostrarTodos, Sair]
);

/// O nome que o menu, a caixa "Sobre" e o título da janela mostram.
///
/// 🔑 **Com o "(Zed GPUI)"**, como o outro app se chama "VintageLightbox
/// (Tauri)": os dois convivem na mesma máquina, e o nome no topo é o que diz
/// qual está aberto (dono, 2026-09-17).
pub const NOME: &str = "VintageLightbox (Zed GPUI)";

/// O ícone da caixa "Sobre". Sem ele, o build de depuração (que não é um
/// pacote `.app`) mostra o ícone genérico do macOS.
#[cfg(target_os = "macos")]
const ICONE: &[u8] = include_bytes!("../../../empacotamento/icones/512x512.png");

/// Liga as ações e instala o menu.
pub fn instalar(cx: &mut App) {
    cx.on_action(|_: &Sobre, _cx| sobre());
    cx.on_action(|_: &Ocultar, cx| cx.hide());
    cx.on_action(|_: &OcultarOsOutros, cx| cx.hide_other_apps());
    cx.on_action(|_: &MostrarTodos, cx| cx.unhide_other_apps());
    cx.on_action(|_: &Sair, cx| cx.quit());
    cx.bind_keys([
        gpui::KeyBinding::new("cmd-h", Ocultar, None),
        gpui::KeyBinding::new("alt-cmd-h", OcultarOsOutros, None),
        gpui::KeyBinding::new("cmd-q", Sair, None),
    ]);
    cx.set_menus(vec![Menu {
        name: NOME.into(),
        items: vec![
            MenuItem::action(format!("Sobre o {NOME}"), Sobre),
            MenuItem::separator(),
            MenuItem::action(format!("Ocultar o {NOME}"), Ocultar),
            MenuItem::action("Ocultar os outros", OcultarOsOutros),
            MenuItem::action("Mostrar todos", MostrarTodos),
            MenuItem::separator(),
            MenuItem::action(format!("Sair do {NOME}"), Sair),
        ],
    }]);
}

/// A caixa "Sobre" do sistema, com o nome, a versão e o ícone do app.
#[cfg(target_os = "macos")]
fn sobre() {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};

    fn texto(s: &str) -> *mut AnyObject {
        let classe = AnyClass::get(c"NSString").expect("NSString");
        let c = std::ffi::CString::new(s).unwrap_or_default();
        unsafe { msg_send![classe, stringWithUTF8String: c.as_ptr()] }
    }

    unsafe {
        let Some(app_classe) = AnyClass::get(c"NSApplication") else {
            return;
        };
        let app: *mut AnyObject = msg_send![app_classe, sharedApplication];

        let dados_classe = AnyClass::get(c"NSData").expect("NSData");
        let dados: *mut AnyObject = msg_send![
            dados_classe,
            dataWithBytes: ICONE.as_ptr(),
            length: ICONE.len()
        ];
        let imagem_classe = AnyClass::get(c"NSImage").expect("NSImage");
        let imagem: *mut AnyObject = msg_send![imagem_classe, alloc];
        let imagem: *mut AnyObject = msg_send![imagem, initWithData: dados];

        let chaves = [
            texto("ApplicationName"),
            texto("ApplicationVersion"),
            texto("Copyright"),
            texto("ApplicationIcon"),
        ];
        let valores = [
            texto(NOME),
            texto(env!("CARGO_PKG_VERSION")),
            texto("RecordarFotos"),
            imagem,
        ];
        let quantos = if imagem.is_null() { 3 } else { 4 };
        let dicionario_classe = AnyClass::get(c"NSDictionary").expect("NSDictionary");
        let opcoes: *mut AnyObject = msg_send![
            dicionario_classe,
            dictionaryWithObjects: valores.as_ptr(),
            forKeys: chaves.as_ptr(),
            count: quantos as usize
        ];
        let _: () = msg_send![app, activateIgnoringOtherApps: true];
        let _: () = msg_send![app, orderFrontStandardAboutPanelWithOptions: opcoes];
    }
}

#[cfg(not(target_os = "macos"))]
fn sobre() {}
