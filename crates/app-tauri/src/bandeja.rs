//! A bandeja: onde o app mora enquanto a janela está minimizada (dono,
//! 2026-09-16), com o estado dos envios à vista.
//!
//! - **Minimizar tira a janela de vista** e põe o ícone na bandeja (a barra de
//!   menus no macOS, a área de notificação no Windows e no Linux). No macOS o
//!   ícone também sai do Dock: o app "vai para a bandeja".
//! - **O clique no ícone abre a janelinha**, com a conta, os envios (subindo,
//!   esperando nota, recusados), o último envio, o problema de rede se houver,
//!   e o espaço do catálogo. O laço dos envios (`sincronizacao.rs`) a atualiza
//!   a cada volta.
//! - **"Abrir o VintageLightbox" traz a janela de volta**, e a bandeja some.
//!
//! Fechar a janela com envio pendente (G9) também passa por aqui: a janela
//! some, e a bandeja fica mostrando o que ainda sobe.
//!
//! ⚠️ Um item de menu do Tauri não pode ser escondido: as linhas são fixas e
//! dizem "nenhuma" quando não há o que contar.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, Runtime};

use crate::catalogo::fila::Contagem;
use crate::catalogo::importacao::Situacao;

const ICONE: &[u8] = include_bytes!("../../../empacotamento/icones/32x32.png");

const ABRIR: &str = "bandeja:abrir";
const ENVIAR_AGORA: &str = "bandeja:enviar-agora";
const ABRIR_PASTA: &str = "bandeja:abrir-pasta";
const SAIR: &str = "bandeja:sair";

/// Tudo o que a janelinha mostra, juntado pelo laço dos envios.
#[derive(Debug, Clone, Default)]
pub struct Estado {
    pub fila: Contagem,
    /// Envios que a página ainda guarda (os que um worker está subindo).
    pub na_pagina: u32,
    pub area: Situacao,
    pub conta: Option<String>,
    pub pilha_local: bool,
    /// Instante (RFC 3339) do último envio que chegou ao servidor.
    pub ultimo_envio: Option<String>,
    /// O envio que falhou: quando tenta de novo (unix) e por quê.
    pub proxima_tentativa: Option<(i64, Option<String>)>,
    pub bytes_do_catalogo: Option<u64>,
    /// Agora, em segundos unix: as frases "há 2 min" são relativas a ele.
    pub agora: i64,
}

/// As linhas da janelinha, uma por item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linhas {
    pub cabecalho: String,
    pub conta: String,
    pub subindo: String,
    pub sem_nota: String,
    pub recusadas: String,
    pub ultimo: String,
    pub rede: String,
    pub catalogo: String,
    /// A dica do ícone: o resumo numa linha.
    pub dica: String,
}

fn plural(n: i64, um: &str, varios: &str) -> String {
    if n == 1 {
        format!("1 {um}")
    } else {
        format!("{n} {varios}")
    }
}

/// "agora há pouco", "há 5 min", "há 3 h", "há 2 dias".
fn ha_quanto(segundos: i64) -> String {
    match segundos.max(0) {
        0..=59 => "agora há pouco".into(),
        s @ 60..=3599 => format!("há {} min", s / 60),
        s @ 3600..=86_399 => format!("há {} h", s / 3600),
        s => format!("há {}", plural(s / 86_400, "dia", "dias")),
    }
}

/// "12 KB", "3,4 MB", "1,2 GB".
fn tamanho(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    let (valor, unidade) = if b < KB * KB {
        (b / KB, "KB")
    } else if b < KB * KB * KB {
        (b / (KB * KB), "MB")
    } else {
        (b / (KB * KB * KB), "GB")
    };
    let texto = if valor >= 100.0 || unidade == "KB" {
        format!("{valor:.0}")
    } else {
        format!("{valor:.1}")
    };
    format!("{} {unidade}", texto.replace('.', ","))
}

/// O que cada linha diz, pelo estado.
pub fn linhas(e: &Estado) -> Linhas {
    let subindo_n = e.fila.pendentes + i64::from(e.na_pagina) + e.area.a_subir;
    let versao = env!("CARGO_PKG_VERSION");
    let cabecalho = if e.pilha_local {
        format!("VintageLightbox {versao} · PILHA LOCAL")
    } else {
        format!("VintageLightbox {versao}")
    };
    let conta = match &e.conta {
        Some(email) => format!("Conta: {email}"),
        None => "Conta: sem sessão".into(),
    };
    let subindo = match subindo_n {
        0 => "Subindo: nada na fila".into(),
        n => format!("Subindo: {}", plural(n, "foto", "fotos")),
    };
    let sem_nota = match e.area.sem_nota {
        0 => "Esperando nota: nenhuma".into(),
        n => format!("Esperando nota: {}", plural(n, "foto", "fotos")),
    };
    let recusadas = match e.fila.recusados {
        0 => "Recusadas pelo servidor: nenhuma".into(),
        n => format!("Recusadas pelo servidor: {n} (veja no app)"),
    };
    let ultimo = match e
        .ultimo_envio
        .as_deref()
        .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
    {
        Some(d) => format!("Último envio: {}", ha_quanto(e.agora - d.timestamp())),
        None => "Último envio: nenhum ainda".into(),
    };
    let rede = match &e.proxima_tentativa {
        Some((quando, motivo)) => {
            let falta = (quando - e.agora).max(0);
            let porque = motivo
                .as_deref()
                .map(|m| m.chars().take(40).collect::<String>())
                .unwrap_or_else(|| "falha no envio".into());
            if falta == 0 {
                format!("Tentando de novo agora ({porque})")
            } else {
                format!("Nova tentativa em {falta} s ({porque})")
            }
        }
        None => "Conexão: sem falhas".into(),
    };
    let catalogo = match e.bytes_do_catalogo {
        Some(b) => format!("Catálogo neste computador: {}", tamanho(b)),
        None => "Catálogo neste computador: calculando…".into(),
    };

    let mut resumo = Vec::new();
    if subindo_n > 0 {
        resumo.push(format!("{} subindo", plural(subindo_n, "foto", "fotos")));
    }
    if e.fila.recusados > 0 {
        resumo.push(format!(
            "{} pelo servidor",
            plural(e.fila.recusados, "recusada", "recusadas")
        ));
    }
    if e.area.sem_nota > 0 {
        resumo.push(format!("{} esperando nota", e.area.sem_nota));
    }
    let dica = if resumo.is_empty() {
        "VintageLightbox — Tudo sincronizado".into()
    } else {
        format!("VintageLightbox — {}", resumo.join(" · "))
    };

    Linhas {
        cabecalho,
        conta,
        subindo,
        sem_nota,
        recusadas,
        ultimo,
        rede,
        catalogo,
        dica,
    }
}

/// Os itens da janelinha, para atualizar sem remontar.
struct Itens<R: Runtime> {
    cabecalho: MenuItem<R>,
    conta: MenuItem<R>,
    subindo: MenuItem<R>,
    sem_nota: MenuItem<R>,
    recusadas: MenuItem<R>,
    ultimo: MenuItem<R>,
    rede: MenuItem<R>,
    catalogo: MenuItem<R>,
    enviar_agora: MenuItem<R>,
}

pub struct Bandeja<R: Runtime> {
    icone: TrayIcon<R>,
    itens: Itens<R>,
    /// As últimas linhas mostradas: atualizar só quando algo mudou.
    ultimas: Mutex<Option<Linhas>>,
    /// Quando a janela foi trazida de volta: a vigia espera ela terminar de
    /// voltar antes de perguntar de novo.
    restaurada_em: Mutex<Option<Instant>>,
    /// O ícone está à vista: ir para a bandeja de novo não faz nada.
    icone_visivel: AtomicBool,
    /// A pasta do catálogo, para "Abrir a pasta".
    pasta: Option<PathBuf>,
}

/// Monta a bandeja, escondida até a janela ser minimizada.
pub fn criar<R: Runtime>(app: &AppHandle<R>, pasta: Option<PathBuf>) -> tauri::Result<()> {
    let linha = |id: &str, texto: &str| MenuItem::with_id(app, id, texto, false, None::<&str>);
    let itens = Itens {
        cabecalho: linha("bandeja:cabecalho", "VintageLightbox")?,
        conta: linha("bandeja:conta", "Conta: …")?,
        subindo: linha("bandeja:subindo", "Subindo: …")?,
        sem_nota: linha("bandeja:sem-nota", "Esperando nota: …")?,
        recusadas: linha("bandeja:recusadas", "Recusadas pelo servidor: …")?,
        ultimo: linha("bandeja:ultimo", "Último envio: …")?,
        rede: linha("bandeja:rede", "Conexão: …")?,
        catalogo: linha("bandeja:catalogo", "Catálogo neste computador: …")?,
        enviar_agora: MenuItem::with_id(app, ENVIAR_AGORA, "Enviar agora", false, None::<&str>)?,
    };
    let abrir_pasta = MenuItem::with_id(
        app,
        ABRIR_PASTA,
        "Abrir a pasta do catálogo",
        pasta.is_some(),
        None::<&str>,
    )?;
    let separador = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(
        app,
        &[
            &itens.cabecalho,
            &itens.conta,
            &separador()?,
            &itens.subindo,
            &itens.sem_nota,
            &itens.recusadas,
            &itens.ultimo,
            &itens.rede,
            &separador()?,
            &itens.catalogo,
            &abrir_pasta,
            &separador()?,
            &itens.enviar_agora,
            &MenuItem::with_id(app, ABRIR, "Abrir o VintageLightbox", true, None::<&str>)?,
            &separador()?,
            &MenuItem::with_id(app, SAIR, "Sair", true, None::<&str>)?,
        ],
    )?;
    let icone = TrayIconBuilder::with_id("vintagelightbox")
        .icon(Image::from_bytes(ICONE)?)
        .tooltip("VintageLightbox")
        .menu(&menu)
        // O clique abre a janelinha, com o estado dos envios (dono,
        // 2026-09-16); a janela volta pelo "Abrir".
        .show_menu_on_left_click(true)
        .on_menu_event(|app, evento| match evento.id().as_ref() {
            ABRIR => mostrar_janela(app),
            ENVIAR_AGORA => {
                if let Some(catalogo) = app.try_state::<Mutex<crate::catalogo::Catalogo>>() {
                    let _ = catalogo.lock().expect("catálogo").tentar_ja();
                }
                app.state::<crate::sincronizacao::Sincronizador>().acordar();
            }
            ABRIR_PASTA => {
                use tauri_plugin_opener::OpenerExt;
                let pasta = app.try_state::<Bandeja<R>>().and_then(|b| b.pasta.clone());
                if let Some(pasta) = pasta {
                    let _ = app
                        .opener()
                        .open_path(pasta.to_string_lossy(), None::<&str>);
                }
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
        itens,
        ultimas: Mutex::new(None),
        restaurada_em: Mutex::new(None),
        icone_visivel: AtomicBool::new(false),
        pasta,
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
            tokio::time::sleep(Duration::from_millis(700)).await;
            let Some(janela) = app.get_webview_window("principal") else {
                continue;
            };
            let voltando = app
                .try_state::<Bandeja<tauri::Wry>>()
                .and_then(|b| *b.restaurada_em.lock().expect("bandeja"))
                .is_some_and(|quando| quando.elapsed() < Duration::from_secs(2));
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
        if bandeja.icone_visivel.swap(true, Ordering::SeqCst) {
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
    // acessório, oculto.
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_dock_visibility(true);
        let _ = app.show();
    }
    if let Some(bandeja) = app.try_state::<Bandeja<R>>() {
        *bandeja.restaurada_em.lock().expect("bandeja") = Some(Instant::now());
    }
    if let Some(janela) = app.get_webview_window("principal") {
        app.state::<crate::sincronizacao::Sincronizador>()
            .manter_aberto();
        // Desminimizar antes de mostrar.
        let _ = janela.unminimize();
        let _ = janela.show();
        let _ = janela.set_focus();
    }
    if let Some(bandeja) = app.try_state::<Bandeja<R>>() {
        bandeja.icone_visivel.store(false, Ordering::SeqCst);
        let _ = bandeja.icone.set_visible(false);
    }
}

/// Atualiza a janelinha. Chamado a cada volta do laço dos envios.
pub fn atualizar<R: Runtime>(app: &AppHandle<R>, estado: &Estado) {
    let Some(bandeja) = app.try_state::<Bandeja<R>>() else {
        return;
    };
    let novas = linhas(estado);
    let mut ultimas = bandeja.ultimas.lock().expect("bandeja");
    if ultimas.as_ref() == Some(&novas) {
        return;
    }
    let i = &bandeja.itens;
    for (item, texto) in [
        (&i.cabecalho, &novas.cabecalho),
        (&i.conta, &novas.conta),
        (&i.subindo, &novas.subindo),
        (&i.sem_nota, &novas.sem_nota),
        (&i.recusadas, &novas.recusadas),
        (&i.ultimo, &novas.ultimo),
        (&i.rede, &novas.rede),
        (&i.catalogo, &novas.catalogo),
    ] {
        let _ = item.set_text(texto);
    }
    let _ = i
        .enviar_agora
        .set_enabled(estado.fila.pendentes > 0 || estado.fila.recusados > 0);
    let _ = bandeja.icone.set_tooltip(Some(&novas.dica));
    *ultimas = Some(novas);
}

/// O tamanho de uma pasta, somando os arquivos (sem seguir atalhos).
pub fn tamanho_da_pasta(pasta: &std::path::Path) -> u64 {
    let mut total = 0;
    let mut pendentes = vec![pasta.to_path_buf()];
    while let Some(atual) = pendentes.pop() {
        let Ok(entradas) = std::fs::read_dir(&atual) else {
            continue;
        };
        for entrada in entradas.flatten() {
            let Ok(tipo) = entrada.file_type() else {
                continue;
            };
            if tipo.is_dir() {
                pendentes.push(entrada.path());
            } else if tipo.is_file() {
                total += entrada.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

#[cfg(test)]
mod testes {
    use super::*;

    fn estado() -> Estado {
        Estado {
            agora: 1_000_000,
            ..Default::default()
        }
    }

    #[test]
    fn sem_nada_pendente_diz_que_esta_tudo_certo() {
        let l = linhas(&estado());
        assert_eq!(l.conta, "Conta: sem sessão");
        assert_eq!(l.subindo, "Subindo: nada na fila");
        assert_eq!(l.sem_nota, "Esperando nota: nenhuma");
        assert_eq!(l.recusadas, "Recusadas pelo servidor: nenhuma");
        assert_eq!(l.ultimo, "Último envio: nenhum ainda");
        assert_eq!(l.rede, "Conexão: sem falhas");
        assert_eq!(l.dica, "VintageLightbox — Tudo sincronizado");
        assert!(!l.cabecalho.contains("PILHA LOCAL"));
    }

    #[test]
    fn cada_linha_conta_a_sua_parte() {
        let e = Estado {
            fila: Contagem {
                pendentes: 2,
                recusados: 1,
            },
            na_pagina: 1,
            area: Situacao {
                a_subir: 2,
                sem_nota: 3,
            },
            conta: Some("dono@estudio".into()),
            pilha_local: true,
            ultimo_envio: Some(
                chrono::DateTime::from_timestamp(1_000_000 - 300, 0)
                    .unwrap()
                    .to_rfc3339(),
            ),
            proxima_tentativa: Some((1_000_040, Some("sem rede".into()))),
            bytes_do_catalogo: Some(3 * 1024 * 1024 + 512 * 1024),
            agora: 1_000_000,
        };
        let l = linhas(&e);
        assert!(l.cabecalho.ends_with("· PILHA LOCAL"), "{}", l.cabecalho);
        assert_eq!(l.conta, "Conta: dono@estudio");
        assert_eq!(l.subindo, "Subindo: 5 fotos");
        assert_eq!(l.sem_nota, "Esperando nota: 3 fotos");
        assert_eq!(l.recusadas, "Recusadas pelo servidor: 1 (veja no app)");
        assert_eq!(l.ultimo, "Último envio: há 5 min");
        assert_eq!(l.rede, "Nova tentativa em 40 s (sem rede)");
        assert_eq!(l.catalogo, "Catálogo neste computador: 3,5 MB");
        assert_eq!(
            l.dica,
            "VintageLightbox — 5 fotos subindo · 1 recusada pelo servidor · 3 esperando nota"
        );
    }

    #[test]
    fn o_tempo_e_o_tamanho_em_palavras() {
        assert_eq!(ha_quanto(10), "agora há pouco");
        assert_eq!(ha_quanto(7200), "há 2 h");
        assert_eq!(ha_quanto(86_400), "há 1 dia");
        assert_eq!(ha_quanto(3 * 86_400), "há 3 dias");
        assert_eq!(tamanho(2048), "2 KB");
        assert_eq!(tamanho(5 * 1024 * 1024 * 1024 / 2), "2,5 GB");
    }

    #[test]
    fn o_tamanho_da_pasta_soma_os_arquivos() {
        let pasta = tempfile::tempdir().unwrap();
        std::fs::write(pasta.path().join("a"), [0u8; 10]).unwrap();
        std::fs::create_dir(pasta.path().join("b")).unwrap();
        std::fs::write(pasta.path().join("b/c"), [0u8; 5]).unwrap();
        assert_eq!(tamanho_da_pasta(pasta.path()), 15);
    }
}
