//! A tela do cliente no segundo monitor (DESKTOP_TAURI §5).
//!
//! No Chrome, a página abre um pop-up e usa `getScreenDetails` para levá-lo ao
//! outro monitor. O WebKitGTK e o WKWebView não têm essa API: o pop-up abre, mas
//! não sabe em qual tela ficar. Aqui a janela nasce no Rust, que conhece os
//! monitores, e vai em tela cheia para o que não é o do operador.
//!
//! A página que ela mostra é a mesma `/tela-do-cliente` do site, e conversa com
//! a galeria pelo mesmo `BroadcastChannel`.

use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

use crate::erro::ErroDaPonte;
use crate::navegacao::{decidir, endereco};

pub const ROTULO: &str = "tela-do-cliente";
const ROTA: &str = "/tela-do-cliente";

/// Onde um monitor começa, em pixels físicos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Origem {
    pub x: i32,
    pub y: i32,
}

/// Escolhe o monitor do cliente: o primeiro que não é o do operador.
///
/// Devolve `None` com um monitor só, e aí a janela abre como janela comum,
/// que o operador arrasta.
pub fn monitor_do_cliente(monitores: &[Origem], do_operador: Option<Origem>) -> Option<usize> {
    if monitores.len() < 2 {
        return None;
    }
    match do_operador {
        Some(operador) => monitores.iter().position(|m| *m != operador),
        None => Some(1),
    }
}

pub async fn abrir(app: &AppHandle) -> Result<(), ErroDaPonte> {
    if let Some(janela) = app.get_webview_window(ROTULO) {
        let _ = janela.show();
        let _ = janela.set_focus();
        return Ok(());
    }

    let janela = WebviewWindowBuilder::new(app, ROTULO, WebviewUrl::External(endereco(ROTA)))
        .title("Tela do cliente")
        .inner_size(1280.0, 800.0)
        .visible(false)
        .on_navigation(decidir)
        .build()
        .map_err(|e| ErroDaPonte::Janela(e.to_string()))?;

    let monitores = janela.available_monitors().unwrap_or_default();
    let origens: Vec<Origem> = monitores
        .iter()
        .map(|m| Origem {
            x: m.position().x,
            y: m.position().y,
        })
        .collect();
    let do_operador = app
        .get_webview_window("principal")
        .and_then(|p| p.current_monitor().ok().flatten())
        .map(|m| Origem {
            x: m.position().x,
            y: m.position().y,
        });

    if let Some(indice) = monitor_do_cliente(&origens, do_operador) {
        let alvo = &monitores[indice];
        // ⚠️ No Wayland o app não escolhe posição de janela, e esta chamada não
        // tem efeito (DESKTOP_TAURI P9). A tela cheia continua valendo.
        let _ = janela.set_position(PhysicalPosition::new(alvo.position().x, alvo.position().y));
        let _ = janela.set_fullscreen(true);
    }
    janela
        .show()
        .map_err(|e| ErroDaPonte::Janela(e.to_string()))?;
    Ok(())
}

pub fn fechar(app: &AppHandle) {
    if let Some(janela) = app.get_webview_window(ROTULO) {
        let _ = janela.close();
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const ESQUERDA: Origem = Origem { x: 0, y: 0 };
    const DIREITA: Origem = Origem { x: 2560, y: 0 };

    #[test]
    fn um_monitor_so_nao_tem_tela_do_cliente() {
        assert_eq!(monitor_do_cliente(&[ESQUERDA], Some(ESQUERDA)), None);
        assert_eq!(monitor_do_cliente(&[], None), None);
    }

    #[test]
    fn vai_para_o_monitor_que_nao_e_do_operador() {
        assert_eq!(
            monitor_do_cliente(&[ESQUERDA, DIREITA], Some(ESQUERDA)),
            Some(1)
        );
        assert_eq!(
            monitor_do_cliente(&[ESQUERDA, DIREITA], Some(DIREITA)),
            Some(0)
        );
    }

    #[test]
    fn sem_saber_do_operador_usa_o_segundo() {
        assert_eq!(monitor_do_cliente(&[ESQUERDA, DIREITA], None), Some(1));
    }
}
