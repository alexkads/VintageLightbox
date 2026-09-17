//! Ferramentas de depuração, só em build de depuração (`cargo run`).
//!
//! O app mostra o site dentro do WKWebView, e o que dá errado lá dentro não
//! aparece em lugar nenhum: não há DevTools aberto nem terminal. Aqui:
//!
//! - **o console da página vai para o terminal**: `console.*`, erros e promessas
//!   rejeitadas, com o nome da janela;
//! - **`VLB_ROTEIRO=arquivo.js`** roda um roteiro dentro das janelas do app (a
//!   principal e a tela do cliente; ele se orienta por `location.pathname`) a
//!   cada página carregada. É como se testa um fluxo no WebKit de verdade sem
//!   olhar a tela;
//! - **`VLB_FOTOS=pasta`** deixa o roteiro fotografar a janela
//!   (`invoke("fotografar_janela", { nome })`), com o que o WebKit compôs de
//!   verdade, WebGPU incluído. O `screencapture` não tem permissão nesta máquina.
//!
//! 🔒 O comando existe no binário de release, mas nenhuma capacidade de release
//! o libera: as duas que o permitem ficam em `capacidades-dev/` e só entram em
//! depuração.

use tauri::webview::PageLoadEvent;
use tauri::{Runtime, WebviewWindow};

/// Liga o console ao terminal. Roda antes de qualquer script da página.
pub const CONSOLE_NO_TERMINAL: &str = r#"
(() => {
  if (window.__vlbConsole) return;
  window.__vlbConsole = true;
  const enviar = (nivel, partes) => {
    const invoke = window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke;
    if (!invoke) return;
    const texto = partes
      .map((p) => {
        if (p instanceof Error) return `${p.name}: ${p.message}\n${p.stack || ""}`;
        if (typeof p === "string") return p;
        try { return JSON.stringify(p); } catch { return String(p); }
      })
      .join(" ");
    invoke("registrar_no_terminal", { nivel, texto: texto.slice(0, 4000) }).catch(() => {});
  };
  for (const nivel of ["log", "info", "warn", "error"]) {
    const original = console[nivel].bind(console);
    console[nivel] = (...partes) => { original(...partes); enviar(nivel, partes); };
  }
  window.__vlbErros = [];
  const guardar = (texto) => { if (window.__vlbErros.length < 20) window.__vlbErros.push(String(texto).slice(0, 500)); };
  window.addEventListener("error", (e) => { guardar(`${e.message} ${e.filename}:${e.lineno}`); enviar("erro", [e.error || e.message, `${e.filename}:${e.lineno}`]); });
  window.addEventListener("unhandledrejection", (e) => { guardar(e.reason && e.reason.stack || e.reason); enviar("promessa", [e.reason]); });
})();
"#;

/// Escreve no terminal o que a página mandou.
#[tauri::command]
pub fn registrar_no_terminal<R: Runtime>(window: WebviewWindow<R>, nivel: String, texto: String) {
    eprintln!("[{} {}] {}", window.label(), nivel, texto);
}

/// Grava `<VLB_FOTOS>/<nome>.png` com o que a janela mostra agora.
#[tauri::command]
pub fn fotografar_janela<R: Runtime>(window: WebviewWindow<R>, nome: String) -> Result<(), String> {
    let pasta = std::env::var_os("VLB_FOTOS").ok_or("VLB_FOTOS não definida")?;
    if nome.is_empty()
        || !nome
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("nome inválido: {nome}"));
    }
    let destino = std::path::PathBuf::from(pasta).join(format!("{nome}.png"));
    fotografar(&window, destino)
}

#[cfg(target_os = "macos")]
fn fotografar<R: Runtime>(
    window: &WebviewWindow<R>,
    destino: std::path::PathBuf,
) -> Result<(), String> {
    use block2::RcBlock;
    use objc2_app_kit::NSImage;
    use objc2_foundation::NSError;
    use objc2_web_kit::WKWebView;

    window
        .with_webview(move |webview| {
            let bloco = RcBlock::new(move |imagem: *mut NSImage, _erro: *mut NSError| {
                // SAFETY: o WebKit entrega uma NSImage válida durante o bloco, ou nula.
                let Some(imagem) = (unsafe { imagem.as_ref() }) else {
                    eprintln!("[foto] o WebKit não fotografou");
                    return;
                };
                let Some(tiff) = imagem.TIFFRepresentation() else {
                    eprintln!("[foto] sem TIFF");
                    return;
                };
                match image::load_from_memory(&tiff.to_vec()).and_then(|i| i.save(&destino)) {
                    Ok(()) => eprintln!("[foto] {}", destino.display()),
                    Err(erro) => eprintln!("[foto] {}: {erro}", destino.display()),
                }
            });
            // SAFETY: `inner()` é o WKWebView desta janela, e o Tauri chama isto
            // na thread principal.
            unsafe {
                let wk = &*(webview.inner() as *const WKWebView);
                wk.takeSnapshotWithConfiguration_completionHandler(None, &bloco);
            }
        })
        .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
fn fotografar<R: Runtime>(
    _window: &WebviewWindow<R>,
    _destino: std::path::PathBuf,
) -> Result<(), String> {
    Err("a foto da janela só existe no macOS".into())
}

/// Roda o roteiro de `VLB_ROTEIRO`, se houver, quando uma página termina de carregar.
pub fn rodar_roteiro<R: Runtime>(janela: &WebviewWindow<R>, evento: PageLoadEvent, url: &str) {
    if evento != PageLoadEvent::Finished {
        return;
    }
    eprintln!("[{} carregou] {}", janela.label(), url);
    // `VLB_SONDA=1`: o que a página mostra, lido pelo próprio webview, sem
    // depender da ponte. É o que responde quando o console fica mudo.
    if std::env::var("VLB_SONDA").is_ok() {
        let janela = janela.clone();
        std::thread::spawn(move || {
            for espera in [6, 14] {
                std::thread::sleep(std::time::Duration::from_secs(espera));
                let rotulo = janela.label().to_string();
                let _ = janela.as_ref().eval_with_callback(
                "JSON.stringify({url: location.href, ponte: !!window.__TAURI__, erros: window.__vlbErros || null, texto: (document.body && document.body.innerText || '').replace(/\\s+/g, ' ').slice(0, 500)})",
                move |resposta| eprintln!("[{rotulo} sonda] {resposta}"),
            );
            }
        });
    }
    let Ok(caminho) = std::env::var("VLB_ROTEIRO") else {
        return;
    };
    match std::fs::read_to_string(&caminho) {
        Ok(roteiro) => {
            let _ = janela.eval(roteiro);
        }
        Err(erro) => eprintln!("[roteiro] não li {caminho}: {erro}"),
    }
}
