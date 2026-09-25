//! O que o operador escolheu nas telas de atendimento, guardado ao lado do
//! catálogo (`atendimento.json`), como o `tema.json`.
//!
//! | Campo | No site |
//! |---|---|
//! | `avisos_do_sistema` | o sino "Ligar notificações do sistema" (`localStorage`) |
//! | `atendente` | o nome do "Quem está assumindo?" (`lexdesk_atendente`) |
//!
//! 🔑 **Os avisos nascem ligados**, ao contrário do site. Lá o sino pede
//! permissão ao navegador, e o padrão é desligado; aqui o app foi pedido para
//! o operador ficar antenado (dono, 2026-09-25), e um aviso que precisa ser
//! descoberto num botão não avisa ninguém.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferencias {
    pub avisos_do_sistema: bool,
    pub atendente: String,
}

impl Default for Preferencias {
    fn default() -> Self {
        Self {
            avisos_do_sistema: true,
            atendente: String::new(),
        }
    }
}

#[cfg(not(test))]
pub fn arquivo() -> PathBuf {
    infrastructure::paths::AppPaths::catalog_root().join("atendimento.json")
}

/// Nos testes, um arquivo temporário por chamada — o mesmo cuidado do
/// `tema.json`: teste não grava na preferência de quem roda a suíte.
#[cfg(test)]
pub fn arquivo() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROXIMO: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "vlb-atendimento-teste-{}-{}.json",
        std::process::id(),
        PROXIMO.fetch_add(1, Ordering::SeqCst)
    ))
}

/// Arquivo ausente ou estragado é o padrão.
pub fn ler(arquivo: &Path) -> Preferencias {
    std::fs::read(arquivo)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Falhar em guardar só faz a próxima abertura voltar ao padrão.
pub fn guardar(arquivo: &Path, preferencias: &Preferencias) {
    if let Some(pai) = arquivo.parent() {
        let _ = std::fs::create_dir_all(pai);
    }
    if let Ok(texto) = serde_json::to_vec_pretty(preferencias) {
        let _ = std::fs::write(arquivo, texto);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nasce_com_avisos_ligados_e_guarda_o_que_mudou() {
        let caminho = arquivo();
        assert_eq!(ler(&caminho), Preferencias::default());
        assert!(ler(&caminho).avisos_do_sistema);

        let mudada = Preferencias {
            avisos_do_sistema: false,
            atendente: "Ana".into(),
        };
        guardar(&caminho, &mudada);
        assert_eq!(ler(&caminho), mudada);

        std::fs::write(&caminho, b"{estragado").unwrap();
        assert_eq!(ler(&caminho), Preferencias::default());
        std::fs::write(&caminho, br#"{"atendente":"Bia"}"#).unwrap();
        assert_eq!(
            ler(&caminho),
            Preferencias {
                avisos_do_sistema: true,
                atendente: "Bia".into()
            },
            "campo que falta vem do padrão"
        );
        let _ = std::fs::remove_file(caminho);
    }
}
