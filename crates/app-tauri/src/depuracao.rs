//! Ferramentas de depuração, só em build de depuração (`cargo run`).
//!
//! O app mostra o site dentro do WKWebView, e o que dá errado lá dentro não
//! aparece em lugar nenhum: não há DevTools aberto nem terminal. Aqui:
//!
//! - **o console da página vai para o terminal**: `console.*`, erros e promessas
//!   rejeitadas, com o nome da janela;
//! - **`VLB_ROTEIRO=arquivo.js`** roda um roteiro dentro da janela principal a
//!   cada página carregada. É como se testa um fluxo no WebKit de verdade sem
//!   olhar a tela.
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
  window.addEventListener("error", (e) => enviar("erro", [e.error || e.message, `${e.filename}:${e.lineno}`]));
  window.addEventListener("unhandledrejection", (e) => enviar("promessa", [e.reason]));
})();
"#;

/// Escreve no terminal o que a página mandou.
#[tauri::command]
pub fn registrar_no_terminal<R: Runtime>(window: WebviewWindow<R>, nivel: String, texto: String) {
    eprintln!("[{} {}] {}", window.label(), nivel, texto);
}

/// Roda o roteiro de `VLB_ROTEIRO`, se houver, quando uma página termina de carregar.
pub fn rodar_roteiro<R: Runtime>(janela: &WebviewWindow<R>, evento: PageLoadEvent, url: &str) {
    if evento != PageLoadEvent::Finished {
        return;
    }
    eprintln!("[{} carregou] {}", janela.label(), url);
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
