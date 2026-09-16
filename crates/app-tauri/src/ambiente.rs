//! Produção ou a pilha local desta máquina (`make up` no e-commerce).
//!
//! 🔒 **Só em build de depuração.** O binário do balcão fala com produção,
//! sempre. Em depuração, `VLB_POS_VENDA_URL` troca a API, e com ela tudo o que
//! guarda estado passa a ser **outro**, para produção e pilha local nunca se
//! misturarem:
//!
//! - a sessão (`sessao-local.json`, sem herdar a do app GPUI, que é de produção);
//! - o catálogo (`Catalogo Tauri (pilha local)`), com a fila de envios dele;
//! - o armazenamento do webview (IndexedDB e caches), onde moram as edições
//!   locais e os envios que ainda não passaram para o Rust. Um envio de produção
//!   que subisse para a pilha local seria recusado, e ficaria lá.

use tauri::{Manager, Runtime, WebviewWindowBuilder};

use crate::navegacao::DESENVOLVIMENTO;

/// A API da pilha local, quando o app foi aberto para ela.
pub fn api_local() -> Option<String> {
    if !DESENVOLVIMENTO {
        return None;
    }
    std::env::var("VLB_POS_VENDA_URL").ok()
}

pub fn na_pilha_local() -> bool {
    api_local().is_some()
}

/// O título das janelas: a pilha local não pode ser confundida com produção.
pub fn titulo(base: &str) -> String {
    if na_pilha_local() {
        format!("{base} — PILHA LOCAL")
    } else {
        base.to_string()
    }
}

/// Dá à janela o armazenamento da pilha local, separado do de produção.
///
/// As janelas do app compartilham o mesmo (o canal da tela do cliente depende
/// disso), então toda janela passa por aqui.
pub fn armazenamento<'a, R: Runtime, M: Manager<R>>(
    construtor: WebviewWindowBuilder<'a, R, M>,
    _pasta_de_dados: Option<std::path::PathBuf>,
) -> WebviewWindowBuilder<'a, R, M> {
    if !na_pilha_local() {
        return construtor;
    }
    #[cfg(target_os = "macos")]
    {
        construtor.data_store_identifier(*b"vlb-pilha-local!")
    }
    #[cfg(not(target_os = "macos"))]
    {
        match _pasta_de_dados {
            Some(pasta) => construtor.data_directory(pasta.join("webview-pilha-local")),
            None => construtor,
        }
    }
}
