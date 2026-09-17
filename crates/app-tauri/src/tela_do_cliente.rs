//! A tela do cliente no segundo monitor (DESKTOP_TAURI §5).
//!
//! No Chrome, a página abre um pop-up e usa `getScreenDetails` para levá-lo ao
//! outro monitor. O WebKitGTK e o WKWebView não têm essa API: o pop-up abre, mas
//! não sabe em qual tela ficar. Aqui a janela nasce no Rust, que conhece os
//! monitores, e vai em tela cheia para o que não é o do operador.
//!
//! A página que ela mostra é a `/tela-do-cliente` da tela empacotada, com o
//! código da mesma rota do site, e conversa com a galeria pelo mesmo
//! `BroadcastChannel`.

use tauri::{AppHandle, Manager, WebviewWindowBuilder};

use crate::erro::ErroDaPonte;
use crate::navegacao::{decidir, tela, DESENVOLVIMENTO};

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

    let construtor = WebviewWindowBuilder::new(app, ROTULO, tela(ROTA));
    let mut construtor = crate::ambiente::armazenamento(construtor, app.path().app_data_dir().ok())
        .title(crate::ambiente::titulo("Tela do cliente"))
        .background_color(tauri::window::Color(0, 0, 0, 255))
        .inner_size(1280.0, 800.0)
        .visible(false)
        .on_navigation(decidir);
    if DESENVOLVIMENTO {
        construtor = construtor
            .initialization_script(crate::depuracao::CONSOLE_NO_TERMINAL)
            .on_page_load(|janela, carga| {
                crate::depuracao::rodar_roteiro(&janela, carga.event(), carga.url().as_str())
            });
    }
    let janela = construtor
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
        //
        // 🚨 **No macOS a posição vai em pontos, com a escala do monitor de
        //    destino.** Em pixels físicos, o Tauri a converte pela escala da
        //    *janela*, que nasce no monitor do operador: com o Retina (2x) ao lado
        //    de um Full HD (1x), a origem do Full HD caía pela metade, dentro do
        //    Retina, e a tela cheia cobria o operador (2026-09-16). No Windows e
        //    no Linux a área de trabalho é em pixels físicos, e a física é a certa.
        #[cfg(target_os = "macos")]
        let destino: tauri::Position = alvo
            .position()
            .to_logical::<f64>(alvo.scale_factor())
            .into();
        #[cfg(not(target_os = "macos"))]
        let destino: tauri::Position =
            tauri::PhysicalPosition::new(alvo.position().x, alvo.position().y).into();
        let _ = janela.set_position(destino);
        let _ = janela.set_fullscreen(true);
    }
    janela
        .show()
        .map_err(|e| ErroDaPonte::Janela(e.to_string()))?;
    if DESENVOLVIMENTO {
        contar_onde_ficou(janela, origens, do_operador);
    }
    Ok(())
}

/// Em depuração, diz no terminal onde a janela foi parar. É como se confere o
/// segundo monitor sem olhar para ele.
fn contar_onde_ficou(
    janela: tauri::WebviewWindow,
    origens: Vec<Origem>,
    do_operador: Option<Origem>,
) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let onde = janela.current_monitor().ok().flatten().map(|m| {
            format!(
                "{:?} em {:?} (escala {})",
                m.name(),
                m.position(),
                m.scale_factor()
            )
        });
        eprintln!(
            "[tela do cliente] monitores {origens:?}, operador {do_operador:?}; \
             a janela está em {:?}, tela cheia {:?}, no monitor {onde:?}",
            janela.outer_position().ok(),
            janela.is_fullscreen().ok(),
        );
    });
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
