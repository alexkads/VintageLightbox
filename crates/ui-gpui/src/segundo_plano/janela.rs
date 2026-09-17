//! O que o GPUI não sabe fazer com a janela: perguntar se está minimizada,
//! escondê-la sem fechar e tirar o app do Dock.
//!
//! 🚨 **O GPUI 0.2 não tem evento nem pergunta de "minimizou"** — como o Tauri,
//! que também não tinha (`app-tauri/src/bandeja.rs`, `vigiar_minimizar`). A
//! saída é a mesma de lá: perguntar ao sistema a cada volta do laço, pela alça
//! nativa da janela.
//!
//! 🚨 **Mudar a janela é sempre depois (`Adiado`), nunca dentro de um `update`.**
//! `orderOut:`, `deminiaturize:` e `ShowWindow` chamam os callbacks da janela
//! na hora, e eles pedem o `App` — que o `update` já tem emprestado: "RefCell
//! already borrowed", e o app cai (conferido pelo roteiro, 2026-09-17). É pelo
//! mesmo motivo que o `activate_window` do próprio GPUI adia o que faz.
//! Perguntar (`isMiniaturized`, `IsIconic`) não dispara nada e pode ser na hora.
//!
//! | | macOS | Windows | Linux |
//! |---|---|---|---|
//! | minimizada? | `isMiniaturized` | `IsIconic` | sem pergunta: `false` |
//! | esconder (G9) | `orderOut:` | `SW_HIDE` | minimiza |
//! | trazer de volta | `deminiaturize:` + `makeKeyAndOrderFront:` | `SW_RESTORE` + foco | `activate_window` |
//! | sair do Dock | `setActivationPolicy:` | — | — |
//!
//! ⚠️ **No Linux o X11 e o Wayland não respondem "minimizada" de um jeito só**,
//! e o GPUI não expõe nem um nem outro. Lá a bandeja aparece ao fechar com
//! envio na fila (G9); minimizar deixa a janela na barra do sistema, como
//! qualquer programa.

use gpui::Window;

/// Um gesto na janela para rodar fora do `update` em que foi preparado.
pub type Adiado = Box<dyn FnOnce()>;

/// A janela principal está minimizada agora.
pub fn minimizada(window: &Window) -> bool {
    plataforma::minimizada(window)
}

/// Prepara tirar a janela de vista sem fechá-la (G9: fechar com envio na fila).
pub fn esconder(window: &Window) -> Adiado {
    plataforma::esconder(window)
}

/// Prepara desminimizar, mostrar e dar o foco.
pub fn mostrar(window: &Window) -> Adiado {
    plataforma::mostrar(window)
}

/// Prepara o botão de fechar da janela, de verdade: passa pelo "posso
/// fechar?" (G9). Só o roteiro de depuração usa.
pub fn pedir_para_fechar(window: &Window) -> Adiado {
    plataforma::pedir_para_fechar(window)
}

/// No macOS, o app sai do Dock enquanto mora na bandeja, e volta com a janela.
/// Chamar só fora de `update`, como os `Adiado`.
///
/// 🚨 **O app volta antes da janela** (a mesma lição do Tauri): sem o Dock ele
/// é um acessório, e uma janela de acessório não vem para a frente.
pub fn no_dock(visivel: bool) {
    plataforma::no_dock(visivel);
}

fn nada() -> Adiado {
    Box::new(|| {})
}

#[cfg(target_os = "macos")]
mod plataforma {
    use gpui::Window;
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use super::{nada, Adiado};

    /// `NSApplicationActivationPolicyRegular` e `…Accessory`.
    const REGULAR: isize = 0;
    const ACESSORIO: isize = 1;

    fn ns_window(window: &Window) -> Option<*mut AnyObject> {
        let alca = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::AppKit(appkit) = alca.as_raw() else {
            return None;
        };
        let vista = appkit.ns_view.as_ptr() as *mut AnyObject;
        let janela: *mut AnyObject = unsafe { msg_send![vista, window] };
        (!janela.is_null()).then_some(janela)
    }

    fn ns_app() -> Option<*mut AnyObject> {
        let classe = AnyClass::get(c"NSApplication")?;
        let app: *mut AnyObject = unsafe { msg_send![classe, sharedApplication] };
        (!app.is_null()).then_some(app)
    }

    /// Um gesto na `NSWindow`, para depois. A janela principal vive tanto
    /// quanto o app, e o gesto roda no mesmo laço, logo em seguida.
    fn na_janela(window: &Window, gesto: fn(*mut AnyObject)) -> Adiado {
        match ns_window(window) {
            Some(j) => {
                let alca = j as usize;
                Box::new(move || gesto(alca as *mut AnyObject))
            }
            None => nada(),
        }
    }

    pub fn minimizada(window: &Window) -> bool {
        ns_window(window).is_some_and(|j| unsafe { msg_send![j, isMiniaturized] })
    }

    pub fn esconder(window: &Window) -> Adiado {
        na_janela(window, |j| {
            let nulo: *const AnyObject = std::ptr::null();
            let _: () = unsafe { msg_send![j, orderOut: nulo] };
        })
    }

    pub fn mostrar(window: &Window) -> Adiado {
        na_janela(window, |j| {
            let nulo: *const AnyObject = std::ptr::null();
            unsafe {
                if let Some(app) = ns_app() {
                    let _: () = msg_send![app, activateIgnoringOtherApps: true];
                }
                // Desminimizar antes de mostrar: escondida e minimizada ao
                // mesmo tempo, o macOS não a devolvia (Tauri, 2026-09-16).
                let minimizada: bool = msg_send![j, isMiniaturized];
                if minimizada {
                    let _: () = msg_send![j, deminiaturize: nulo];
                }
                let _: () = msg_send![j, makeKeyAndOrderFront: nulo];
            }
        })
    }

    pub fn pedir_para_fechar(window: &Window) -> Adiado {
        na_janela(window, |j| {
            let nulo: *const AnyObject = std::ptr::null();
            let _: () = unsafe { msg_send![j, performClose: nulo] };
        })
    }

    pub fn no_dock(visivel: bool) {
        let Some(app) = ns_app() else {
            return;
        };
        let politica = if visivel { REGULAR } else { ACESSORIO };
        let _: bool = unsafe { msg_send![app, setActivationPolicy: politica] };
    }
}

#[cfg(target_os = "windows")]
mod plataforma {
    use gpui::Window;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, IsWindowVisible, PostMessageW, SetForegroundWindow, ShowWindow, SW_HIDE,
        SW_RESTORE, SW_SHOW, WM_CLOSE,
    };

    use super::{nada, Adiado};

    fn hwnd(window: &Window) -> Option<HWND> {
        let alca = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::Win32(win32) = alca.as_raw() else {
            return None;
        };
        Some(win32.hwnd.get() as HWND)
    }

    fn na_janela(window: &Window, gesto: fn(HWND)) -> Adiado {
        match hwnd(window) {
            Some(h) => {
                let alca = h as usize;
                Box::new(move || gesto(alca as HWND))
            }
            None => nada(),
        }
    }

    pub fn minimizada(window: &Window) -> bool {
        hwnd(window).is_some_and(|h| unsafe { IsIconic(h) } != 0)
    }

    pub fn esconder(window: &Window) -> Adiado {
        na_janela(window, |h| unsafe {
            ShowWindow(h, SW_HIDE);
        })
    }

    pub fn mostrar(window: &Window) -> Adiado {
        na_janela(window, |h| unsafe {
            if IsWindowVisible(h) == 0 {
                ShowWindow(h, SW_SHOW);
            }
            if IsIconic(h) != 0 {
                ShowWindow(h, SW_RESTORE);
            }
            SetForegroundWindow(h);
        })
    }

    pub fn pedir_para_fechar(window: &Window) -> Adiado {
        na_janela(window, |h| unsafe {
            PostMessageW(h, WM_CLOSE, 0, 0);
        })
    }

    /// No Windows a janela minimizada continua na barra de tarefas, como no
    /// Tauri (lá `set_dock_visibility` só existe no macOS).
    pub fn no_dock(_visivel: bool) {}
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod plataforma {
    use gpui::Window;

    use super::{nada, Adiado};

    pub fn minimizada(_window: &Window) -> bool {
        false
    }

    // No Linux quem mexe na janela é o próprio GPUI, que já adia o que faz.
    pub fn esconder(window: &Window) -> Adiado {
        window.minimize_window();
        nada()
    }

    pub fn mostrar(window: &Window) -> Adiado {
        window.activate_window();
        nada()
    }

    pub fn pedir_para_fechar(_window: &Window) -> Adiado {
        nada()
    }

    pub fn no_dock(_visivel: bool) {}
}
