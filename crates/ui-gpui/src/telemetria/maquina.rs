//! Quem é este computador, para o servidor.
//!
//! 🔑 **O id nasce aqui e fica em `~/.vintagelightbox/maquina.json`** — a mesma
//! casa do instalador. Reinstalar por cima, compilando ou pelo pacote, mantém o
//! arquivo e portanto o id: o painel continua vendo o mesmo balcão. Trocar de
//! conta não troca o id: o computador é o mesmo.

use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// O que o app diz de si ao abrir o fluxo e ao relatar uma ocorrência.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Computador {
    pub id: String,
    /// O nome da máquina na rede (hostname).
    pub nome: String,
    /// `windows`, `macos` ou `linux`.
    pub sistema: String,
    /// O que o sistema diz de si: "Fedora Linux 44", "Windows 11 Pro"…
    pub distribuicao: Option<String>,
    pub arquitetura: String,
    /// `compilado`, `pacote` ou `desenvolvimento`.
    pub jeito: String,
    pub versao: String,
}

#[derive(Serialize, Deserialize)]
struct Arquivo {
    id: String,
}

/// Lê o id guardado, ou cria um. Sem casa gravável, um id da sessão: o fluxo
/// abre igual, e o painel verá um computador novo a cada abertura — melhor que
/// não ver nenhum.
pub fn id_guardado_em(pasta: &Path) -> String {
    let arquivo = pasta.join("maquina.json");
    if let Some(id) = std::fs::read(&arquivo)
        .ok()
        .and_then(|b| serde_json::from_slice::<Arquivo>(&b).ok())
        .map(|a| a.id)
        .filter(|id| (8..=64).contains(&id.len()))
    {
        return id;
    }
    let id = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::create_dir_all(pasta);
    if let Ok(bytes) = serde_json::to_vec(&Arquivo { id: id.clone() }) {
        let _ = std::fs::write(&arquivo, bytes);
    }
    id
}

fn sistema() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// Este computador, lido uma vez.
pub fn deste() -> &'static Computador {
    static COMPUTADOR: OnceLock<Computador> = OnceLock::new();
    COMPUTADOR.get_or_init(|| {
        let pasta = crate::atualizacao::compilar::casa()
            .unwrap_or_else(|| std::env::temp_dir().join(".vintagelightbox"));
        let jeito = match crate::atualizacao::novidades::jeito_desta_instalacao() {
            crate::atualizacao::novidades::JeitoDaInstalacao::Compilado => "compilado",
            crate::atualizacao::novidades::JeitoDaInstalacao::Pacote => "pacote",
            crate::atualizacao::novidades::JeitoDaInstalacao::Desenvolvimento => "desenvolvimento",
        };
        Computador {
            id: id_guardado_em(&pasta),
            nome: sysinfo::System::host_name().unwrap_or_default(),
            sistema: sistema().into(),
            distribuicao: sysinfo::System::long_os_version(),
            arquitetura: std::env::consts::ARCH.into(),
            jeito: jeito.into(),
            versao: env!("CARGO_PKG_VERSION").into(),
        }
    })
}

/// Codifica para a query: só letra, número, `-` e `_` passam. O `.` também é
/// codificado — o cliente da API recusa caminho com `..`, e um hostname
/// estranho não pode derrubar o fluxo.
pub fn na_query(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    for byte in texto.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
            saida.push(byte as char);
        } else {
            saida.push_str(&format!("%{byte:02X}"));
        }
    }
    saida
}

/// O caminho do fluxo, com o que o servidor precisa para comparar a versão.
pub fn caminho_do_fluxo(c: &Computador) -> String {
    let mut caminho = format!(
        "/app-desktop/eventos?maquina={}&versao={}&sistema={}&arquitetura={}&jeito={}&nome={}",
        na_query(&c.id),
        na_query(&c.versao),
        na_query(&c.sistema),
        na_query(&c.arquitetura),
        na_query(&c.jeito),
        na_query(&c.nome),
    );
    if let Some(d) = &c.distribuicao {
        caminho.push_str("&distribuicao=");
        caminho.push_str(&na_query(d));
    }
    caminho
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_id_fica_guardado_e_volta_o_mesmo() {
        let pasta = tempfile::TempDir::new().unwrap();
        let primeiro = id_guardado_em(pasta.path());
        assert_eq!(
            id_guardado_em(pasta.path()),
            primeiro,
            "reabrir não troca o id"
        );
        std::fs::write(pasta.path().join("maquina.json"), b"torto").unwrap();
        assert_ne!(
            id_guardado_em(pasta.path()),
            primeiro,
            "arquivo torto vira id novo"
        );
    }

    #[test]
    fn a_query_nao_leva_ponto_nem_espaco() {
        assert_eq!(na_query("0.1.19"), "0%2E1%2E19");
        assert_eq!(na_query("Fedora Linux 44"), "Fedora%20Linux%2044");
        let c = Computador {
            id: "abc-123_x".into(),
            nome: "BALCÃO..01".into(),
            sistema: "linux".into(),
            distribuicao: None,
            arquitetura: "x86_64".into(),
            jeito: "compilado".into(),
            versao: "0.1.19".into(),
        };
        let caminho = caminho_do_fluxo(&c);
        assert!(!caminho.contains(".."), "{caminho}");
        assert!(caminho.starts_with("/app-desktop/eventos?maquina=abc-123_x&versao=0%2E1%2E19"));
    }
}
