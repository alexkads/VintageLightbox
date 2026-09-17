//! A bandeja: onde o app mora enquanto a janela está minimizada (dono,
//! 2026-09-16), com o estado dos envios à vista.
//!
//! - **Minimizar esconde a janela** e põe o ícone na bandeja (a barra de menus
//!   no macOS, a área de notificação no Windows e no Linux). No macOS o ícone
//!   também sai do Dock: o app "vai para a bandeja".
//! - **O ícone diz como estão os envios**: quantos faltam subir, quantos o
//!   servidor recusou, ou que está tudo sincronizado. O laço dos envios
//!   (`sincronizacao.rs`) atualiza a cada volta.
//! - **Clicar no ícone abre o menu**, com o estado; "Abrir o VintageLightbox"
//!   traz a janela de volta, e a bandeja some.
//!
//! Fechar a janela com envio pendente (G9) também passa por aqui: a janela
//! some, e a bandeja fica mostrando o que ainda sobe.

use std::sync::Mutex;

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, Runtime};

use crate::catalogo::fila::Contagem;
use crate::catalogo::importacao::Situacao;

const ICONE: &[u8] = include_bytes!("../../../empacotamento/icones/32x32.png");

const ABRIR: &str = "bandeja:abrir";
const ENVIAR_AGORA: &str = "bandeja:enviar-agora";
const SAIR: &str = "bandeja:sair";

/// O ícone e a linha de estado do menu, para atualizar sem remontar.
pub struct Bandeja<R: Runtime> {
    icone: TrayIcon<R>,
    /// Quando a janela foi trazida de volta: a vigia espera ela terminar de
    /// voltar antes de perguntar de novo.
    restaurada_em: Mutex<Option<std::time::Instant>>,
    /// O ícone está à vista: ir para a bandeja de novo não faz nada.
    icone_visivel: std::sync::atomic::AtomicBool,
    estado: MenuItem<R>,
    enviar_agora: MenuItem<R>,
    /// O último texto mostrado: atualizar só quando muda.
    ultimo: Mutex<String>,
}

/// O que a bandeja diz, pelo estado da fila.
pub fn texto_do_estado(contagem: &Contagem, na_pagina: u32, area: &Situacao) -> String {
    let pendentes = contagem.pendentes + i64::from(na_pagina) + area.a_subir;
    let mut partes = Vec::new();
    match pendentes {
        0 => {}
        1 => partes.push("1 foto subindo".to_string()),
        n => partes.push(format!("{n} fotos subindo")),
    }
    match contagem.recusados {
        0 => {}
        1 => partes.push("1 recusada pelo servidor".to_string()),
        n => partes.push(format!("{n} recusadas pelo servidor")),
    }
    match area.sem_nota {
        0 => {}
        1 => partes.push("1 esperando nota".to_string()),
        n => partes.push(format!("{n} esperando nota")),
    }
    if partes.is_empty() {
        "Tudo sincronizado".to_string()
    } else {
        partes.join(" · ")
    }
}

/// Monta a bandeja, escondida até a janela ser minimizada.
pub fn criar<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let estado = MenuItem::with_id(
        app,
        "bandeja:estado",
        "Tudo sincronizado",
        false,
        None::<&str>,
    )?;
    let enviar_agora = MenuItem::with_id(app, ENVIAR_AGORA, "Enviar agora", false, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &estado,
            &enviar_agora,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, ABRIR, "Abrir o VintageLightbox", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, SAIR, "Sair", true, None::<&str>)?,
        ],
    )?;
    let icone = TrayIconBuilder::with_id("vintagelightbox")
        .icon(Image::from_bytes(ICONE)?)
        .tooltip("VintageLightbox — Tudo sincronizado")
        .menu(&menu)
        // O clique abre o menu, com o estado dos envios (dono, 2026-09-16); a
        // janela volta pelo "Abrir".
        .show_menu_on_left_click(true)
        .on_menu_event(|app, evento| match evento.id().as_ref() {
            ABRIR => mostrar_janela(app),
            ENVIAR_AGORA => {
                if let Some(catalogo) = app.try_state::<Mutex<crate::catalogo::Catalogo>>() {
                    let _ = catalogo.lock().expect("catálogo").tentar_ja();
                }
                app.state::<crate::sincronizacao::Sincronizador>().acordar();
            }
            // Pela bandeja, sair é escolha explícita: não espera a fila, como o
            // "Sair" do menu do app. O que não subiu continua no catálogo.
            SAIR => app.exit(0),
            _ => {}
        })
        .build(app)?;
    icone.set_visible(false)?;
    app.manage(Bandeja {
        icone,
        restaurada_em: Mutex::new(None),
        icone_visivel: std::sync::atomic::AtomicBool::new(false),
        estado,
        enviar_agora,
        ultimo: Mutex::new(String::new()),
    });
    Ok(())
}

/// Vigia a janela principal: minimizada, ela vai para a bandeja.
///
/// 🚨 **O Tauri não avisa que a janela foi minimizada.** Os eventos de tamanho
/// e de foco às vezes chegam (e `main.rs` os usa), mas não quando a janela já
/// estava sem foco: minimizada assim, ela ficava no Dock (conferido pelo System
/// Events, 2026-09-16). Uma pergunta a cada 0,7 s custa nada e não falha.
pub fn vigiar_minimizar(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;
            let Some(janela) = app.get_webview_window("principal") else {
                continue;
            };
            let voltando = app
                .try_state::<Bandeja<tauri::Wry>>()
                .and_then(|b| *b.restaurada_em.lock().expect("bandeja"))
                .is_some_and(|quando| quando.elapsed() < std::time::Duration::from_secs(2));
            if voltando {
                continue;
            }
            if janela.is_minimized().unwrap_or(false) {
                para_a_bandeja(&app);
            }
        }
    });
}

/// Tira o app de vista e mostra a bandeja.
///
/// Minimizada, a janela **fica minimizada**, e não escondida: sem o Dock ela
/// já não aparece em lugar nenhum, e voltar é desminimizar. Escondida e
/// minimizada ao mesmo tempo, o macOS não a desminimizava mais (conferido pelo
/// System Events, 2026-09-16). Fechada (G9), ela é escondida.
pub fn para_a_bandeja<R: Runtime>(app: &AppHandle<R>) {
    if let Some(bandeja) = app.try_state::<Bandeja<R>>() {
        if bandeja
            .icone_visivel
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return;
        }
        let _ = bandeja.icone.set_visible(true);
    }
    if let Some(janela) = app.get_webview_window("principal") {
        if !janela.is_minimized().unwrap_or(false) {
            let _ = janela.hide();
        }
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_dock_visibility(false);
}

/// Traz a janela de volta e tira a bandeja. O app deixa de terminar sozinho
/// quando a fila esvaziar (quem o fechou está de volta).
pub fn mostrar_janela<R: Runtime>(app: &AppHandle<R>) {
    // 🚨 **No macOS, o app volta antes da janela.** Sem o Dock ele fica como
    // acessório, oculto: mostrar e desminimizar a janela nesse estado a
    // deixava listada, mas minimizada e fora da tela (conferido pelo System
    // Events, 2026-09-16).
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_dock_visibility(true);
        let _ = app.show();
    }
    if let Some(bandeja) = app.try_state::<Bandeja<R>>() {
        *bandeja.restaurada_em.lock().expect("bandeja") = Some(std::time::Instant::now());
    }
    if let Some(janela) = app.get_webview_window("principal") {
        app.state::<crate::sincronizacao::Sincronizador>()
            .manter_aberto();
        // Desminimizar antes de mostrar: mostrada ainda minimizada, a janela
        // voltava para o Dock.
        let _ = janela.unminimize();
        let _ = janela.show();
        let _ = janela.set_focus();
    }
    if let Some(bandeja) = app.try_state::<Bandeja<R>>() {
        bandeja
            .icone_visivel
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = bandeja.icone.set_visible(false);
    }
}

/// Atualiza o que a bandeja diz. Chamado a cada volta do laço dos envios.
pub fn atualizar<R: Runtime>(
    app: &AppHandle<R>,
    contagem: &Contagem,
    na_pagina: u32,
    area: &Situacao,
) {
    let Some(bandeja) = app.try_state::<Bandeja<R>>() else {
        return;
    };
    let texto = texto_do_estado(contagem, na_pagina, area);
    let mut ultimo = bandeja.ultimo.lock().expect("bandeja");
    if *ultimo == texto {
        return;
    }
    let _ = bandeja.estado.set_text(&texto);
    let _ = bandeja
        .enviar_agora
        .set_enabled(contagem.pendentes > 0 || contagem.recusados > 0);
    let _ = bandeja
        .icone
        .set_tooltip(Some(format!("VintageLightbox — {texto}")));
    *ultimo = texto;
}

#[cfg(test)]
mod testes {
    use super::*;

    fn contagem(pendentes: i64, recusados: i64) -> Contagem {
        Contagem {
            pendentes,
            recusados,
        }
    }

    #[test]
    fn o_texto_diz_o_que_falta_e_o_que_foi_recusado() {
        let nada = Situacao::default();
        assert_eq!(
            texto_do_estado(&contagem(0, 0), 0, &nada),
            "Tudo sincronizado"
        );
        assert_eq!(texto_do_estado(&contagem(1, 0), 0, &nada), "1 foto subindo");
        // As que a página ainda guarda e as classificadas da importação também
        // estão subindo.
        let importacao = Situacao {
            a_subir: 2,
            sem_nota: 0,
        };
        assert_eq!(
            texto_do_estado(&contagem(2, 0), 1, &importacao),
            "5 fotos subindo"
        );
        assert_eq!(
            texto_do_estado(&contagem(0, 2), 0, &nada),
            "2 recusadas pelo servidor"
        );
        let sem_nota = Situacao {
            a_subir: 0,
            sem_nota: 3,
        };
        assert_eq!(
            texto_do_estado(&contagem(1, 1), 0, &sem_nota),
            "1 foto subindo · 1 recusada pelo servidor · 3 esperando nota"
        );
    }
}
