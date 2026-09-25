//! O ícone na bandeja — a barra de menus no macOS, a área de notificação no
//! Windows e no Linux —, com a janelinha que mostra a conta, os envios, o
//! espaço do catálogo e os botões "Abrir a pasta do catálogo", "Abrir o
//! VintageLightbox" e "Sair".
//!
//! 🔑 **O `tray-icon` na mesma versão do lockfile**: não entra dependência
//! nova na árvore, só uma aresta.
//!
//! 🚨 **Cada sistema quer o ícone numa thread diferente.**
//! - macOS: na thread principal, com o laço do AppKit já rodando — é a do GPUI.
//! - Windows: na thread que roda o laço de mensagens Win32 — também a do GPUI.
//! - Linux: numa thread com o laço do GTK, que o GPUI não roda. Lá o ícone mora
//!   numa thread própria, e as ordens chegam por canal.
//!
//! O clique nos itens chega pelo canal global do `muda` em qualquer sistema, e
//! o laço de `segundo_plano` o esvazia a cada volta.
//!
//! ⚠️ Nasce só na primeira vez que o app vai para a bandeja: quem nunca
//! minimiza não carrega o ícone, e o macOS não pisca um ícone escondido na
//! abertura.

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::segundo_plano::frases::Linhas;

/// No macOS a barra de menus é desenhada a 18 pt em tela Retina: a imagem
/// maior sai nítida. Nos outros, o tamanho da bandeja do sistema.
#[cfg(target_os = "macos")]
const PNG: &[u8] = include_bytes!("../../../empacotamento/icones/128x128.png");
#[cfg(not(target_os = "macos"))]
const PNG: &[u8] = include_bytes!("../../../empacotamento/icones/32x32.png");

const ABRIR: &str = "bandeja:abrir";
const ABRIR_PASTA: &str = "bandeja:abrir-pasta";
const SAIR: &str = "bandeja:sair";

/// Se o sistema **mostra** ícone de bandeja — e, portanto, se o app pode ir
/// para lá sem sumir.
///
/// # 🚨 O GNOME puro não tem bandeja
///
/// *"Eu não consigo fechar o sistema, pois o mesmo não vai pra bandeja no
/// linux, agora mesmo não estou conseguindo nem matar o processo"* (dono,
/// 25/set/2026). No Linux o ícone é um `StatusNotifierItem`, e só aparece se
/// alguém o hospeda: o KDE, o XFCE, ou o GNOME **com a extensão AppIndicator**.
/// Sem ela, o `tray-icon` cria o ícone sem erro nenhum e ninguém o desenha —
/// fechar minimizava, o "Sair" da bandeja não existia na tela, e o app não
/// tinha como ser encerrado.
///
/// Quem hospeda o ícone registra o nome `org.kde.StatusNotifierWatcher` no
/// barramento da sessão; a pergunta é essa. Sem `gdbus` para perguntar, a
/// resposta é "não": o pior caso vira um app que fecha ao fechar, e não um
/// que não fecha nunca.
#[cfg(not(test))]
pub fn existe_no_sistema() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    std::process::Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.DBus",
            "--object-path",
            "/org/freedesktop/DBus",
            "--method",
            "org.freedesktop.DBus.NameHasOwner",
            "org.kde.StatusNotifierWatcher",
        ])
        .output()
        .map(|saida| {
            saida.status.success() && String::from_utf8_lossy(&saida.stdout).contains("true")
        })
        .unwrap_or(false)
}

/// 🧪 Na suíte a bandeja existe, a não ser que o cenário finja o GNOME puro.
#[cfg(test)]
pub fn existe_no_sistema() -> bool {
    !teste::SEM_BANDEJA.with(std::cell::Cell::get)
}

/// 🧪 O GNOME sem a extensão, para o teste: por padrão a suíte tem bandeja.
#[cfg(test)]
pub mod teste {
    use std::cell::Cell;

    thread_local! {
        pub(super) static SEM_BANDEJA: Cell<bool> = const { Cell::new(false) };
    }

    pub fn fingir_sem_bandeja() {
        SEM_BANDEJA.with(|s| s.set(true));
    }
}

/// O que o operador escolheu na janelinha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clique {
    Abrir,
    AbrirPasta,
    Sair,
}

/// Os cliques desde a última volta. Esvazia também os eventos do ícone
/// (passar o mouse, clicar): ninguém os lê, e o canal é sem limite.
pub fn cliques() -> Vec<Clique> {
    while TrayIconEvent::receiver().try_recv().is_ok() {}
    let mut saida = Vec::new();
    while let Ok(evento) = MenuEvent::receiver().try_recv() {
        match evento.id.as_ref() {
            ABRIR => saida.push(Clique::Abrir),
            ABRIR_PASTA => saida.push(Clique::AbrirPasta),
            SAIR => saida.push(Clique::Sair),
            _ => {}
        }
    }
    saida
}

/// Uma ordem para o ícone.
#[derive(Debug, Clone)]
enum Ordem {
    Visivel(bool),
    Linhas(Box<Linhas>),
}

/// Os itens da janelinha, para trocar o texto sem remontar o menu.
///
/// ⚠️ Um item de menu não se esconde: as linhas são fixas e dizem "nenhuma"
/// quando não há o que contar.
struct Itens {
    cabecalho: MenuItem,
    conta: MenuItem,
    subindo: MenuItem,
    rejeitadas: MenuItem,
    recusadas: MenuItem,
    segundo_plano: MenuItem,
    ultimo: MenuItem,
    catalogo: MenuItem,
}

struct Montado {
    icone: TrayIcon,
    itens: Itens,
}

fn montar() -> Result<Montado, String> {
    let linha = |id: &str, texto: &str| MenuItem::with_id(id, texto, false, None);
    let itens = Itens {
        cabecalho: linha("bandeja:cabecalho", crate::menu::NOME),
        conta: linha("bandeja:conta", "Conta: …"),
        subindo: linha("bandeja:subindo", "Subindo: …"),
        rejeitadas: linha("bandeja:rejeitadas", "Rejeitadas (não sobem): …"),
        recusadas: linha("bandeja:recusadas", "Recusadas pelo servidor: …"),
        segundo_plano: linha("bandeja:segundo-plano", "Em segundo plano: …"),
        ultimo: linha("bandeja:ultimo", "Último envio: …"),
        catalogo: linha("bandeja:catalogo", "Catálogo neste computador: …"),
    };
    let abrir_pasta = MenuItem::with_id(ABRIR_PASTA, "Abrir a pasta do catálogo", true, None);
    let abrir = MenuItem::with_id(ABRIR, "Abrir o VintageLightbox", true, None);
    let sair = MenuItem::with_id(SAIR, "Sair", true, None);
    let (s1, s2, s3, s4) = (
        PredefinedMenuItem::separator(),
        PredefinedMenuItem::separator(),
        PredefinedMenuItem::separator(),
        PredefinedMenuItem::separator(),
    );
    let menu = Menu::with_items(&[
        &itens.cabecalho,
        &itens.conta,
        &s1,
        &itens.subindo,
        &itens.rejeitadas,
        &itens.recusadas,
        &itens.segundo_plano,
        &itens.ultimo,
        &s2,
        &itens.catalogo,
        &abrir_pasta,
        &s3,
        &abrir,
        &s4,
        &sair,
    ])
    .map_err(|e| e.to_string())?;

    let rgba = image::load_from_memory(PNG)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (largura, altura) = rgba.dimensions();
    let imagem = Icon::from_rgba(rgba.into_raw(), largura, altura).map_err(|e| e.to_string())?;
    let icone = TrayIconBuilder::new()
        .with_id("vintagelightbox-gpui")
        .with_icon(imagem)
        .with_tooltip(crate::menu::NOME)
        .with_menu(Box::new(menu))
        // O clique abre a janelinha com o estado dos envios (dono,
        // 2026-09-16); a janela volta pelo "Abrir".
        .with_menu_on_left_click(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(Montado { icone, itens })
}

fn aplicar(montado: &Montado, ordem: Ordem) {
    match ordem {
        Ordem::Visivel(visivel) => {
            let _ = montado.icone.set_visible(visivel);
        }
        Ordem::Linhas(novas) => {
            let i = &montado.itens;
            for (item, texto) in [
                (&i.cabecalho, &novas.cabecalho),
                (&i.conta, &novas.conta),
                (&i.subindo, &novas.subindo),
                (&i.rejeitadas, &novas.rejeitadas),
                (&i.recusadas, &novas.recusadas),
                (&i.segundo_plano, &novas.segundo_plano),
                (&i.ultimo, &novas.ultimo),
                (&i.catalogo, &novas.catalogo),
            ] {
                item.set_text(texto);
            }
            let _ = montado.icone.set_tooltip(Some(&novas.dica));
        }
    }
}

/// O ícone, pronto para receber ordens.
pub struct Icone {
    #[cfg(not(target_os = "linux"))]
    montado: Montado,
    #[cfg(target_os = "linux")]
    ordens: std::sync::mpsc::Sender<Ordem>,
}

impl Icone {
    /// Monta o ícone, escondido.
    #[cfg(not(target_os = "linux"))]
    pub fn criar() -> Result<Self, String> {
        let montado = montar()?;
        let _ = montado.icone.set_visible(false);
        Ok(Self { montado })
    }

    /// Monta o ícone numa thread com o laço do GTK, escondido.
    #[cfg(target_os = "linux")]
    pub fn criar() -> Result<Self, String> {
        use gtk::glib;

        let (ordens, recebidas) = std::sync::mpsc::channel::<Ordem>();
        let (pronto, resposta) = std::sync::mpsc::channel::<Result<(), String>>();
        std::thread::Builder::new()
            .name("bandeja-gtk".into())
            .spawn(move || {
                if let Err(erro) = gtk::init() {
                    let _ = pronto.send(Err(erro.to_string()));
                    return;
                }
                let montado = match montar() {
                    Ok(m) => m,
                    Err(erro) => {
                        let _ = pronto.send(Err(erro));
                        return;
                    }
                };
                let _ = montado.icone.set_visible(false);
                let _ = pronto.send(Ok(()));
                glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
                    loop {
                        match recebidas.try_recv() {
                            Ok(ordem) => aplicar(&montado, ordem),
                            Err(std::sync::mpsc::TryRecvError::Empty) => break,
                            // O app acabou: o ícone vai junto.
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                gtk::main_quit();
                                return glib::ControlFlow::Break;
                            }
                        }
                    }
                    glib::ControlFlow::Continue
                });
                gtk::main();
            })
            .map_err(|e| e.to_string())?;
        resposta
            .recv()
            .map_err(|_| "a thread da bandeja morreu antes de responder".to_string())??;
        Ok(Self { ordens })
    }

    fn mandar(&self, ordem: Ordem) {
        #[cfg(not(target_os = "linux"))]
        aplicar(&self.montado, ordem);
        #[cfg(target_os = "linux")]
        let _ = self.ordens.send(ordem);
    }

    pub fn mostrar(&self, visivel: bool) {
        self.mandar(Ordem::Visivel(visivel));
    }

    pub fn atualizar(&self, linhas: &Linhas) {
        self.mandar(Ordem::Linhas(Box::new(linhas.clone())));
    }
}

/// Abre a pasta no gerenciador de arquivos do sistema.
pub fn abrir_pasta(pasta: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let programa = "open";
    #[cfg(target_os = "windows")]
    let programa = "explorer";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let programa = "xdg-open";
    if let Err(erro) = std::process::Command::new(programa).arg(pasta).spawn() {
        eprintln!("⚠️ [Bandeja] não abri {}: {erro}", pasta.display());
    }
}
