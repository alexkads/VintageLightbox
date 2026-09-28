//! 🧯 O vigia dos travamentos: uma thread à parte que percebe quando a
//! interface parou de responder e guarda em disco o que a captura tinha.
//!
//! # Por que uma thread, e não o botão "Salvar"
//!
//! 🚨 **Com a interface travada, nada dela roda** — nem botão, nem callback,
//! nem tarefa. Quem pode agir é outra thread. Enquanto a captura está ligada,
//! a thread da interface bate ([`bater`]) a cada quadro e a cada 100 ms (uma
//! tarefa dela, que também roda com a janela parada); o vigia olha a cada
//! 50 ms e, quando a batida some por mais de [`LIMIAR`]:
//!
//! 1. grava **na hora** a sessão até ali em
//!    `~/.vintagelightbox/desempenho/pendentes/<id>.json` (arquivo `.tmp`
//!    renomeado: o processo morto no meio não deixa JSON pela metade);
//! 2. quando a batida volta, anota o travamento com a duração na captura.
//!
//! Se o operador matar o app travado, o arquivo fica, e entra no banco na
//! próxima vez que a tela de Desempenho abrir ([`super::porta`]).
//!
//! ⚠️ A trava da captura é tentada por 200 ms: a interface só a segura por
//! microssegundos, mas se um dia travar segurando, o vigia desiste em vez de
//! travar junto.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// A interface parada por mais que isto está travada.
pub const LIMIAR: Duration = Duration::from_millis(1_000);
/// Quando a batida volta a vir dentro disto, o travamento acabou.
const VOLTOU: Duration = Duration::from_millis(200);

static LIGADO: AtomicBool = AtomicBool::new(false);
static GERACAO: AtomicU64 = AtomicU64::new(0);
static BATIDA_MS: AtomicU64 = AtomicU64::new(0);

fn epoca() -> Instant {
    static EPOCA: OnceLock<Instant> = OnceLock::new();
    *EPOCA.get_or_init(Instant::now)
}

fn agora_ms() -> u64 {
    epoca().elapsed().as_millis() as u64
}

/// A pasta dos travamentos que ainda não entraram no banco.
pub fn pasta_dos_pendentes() -> Option<PathBuf> {
    crate::atualizacao::compilar::casa().map(|c| c.join("desempenho").join("pendentes"))
}

/// A batida da thread da interface.
#[inline]
pub fn bater() {
    if LIGADO.load(Ordering::Relaxed) {
        BATIDA_MS.store(agora_ms(), Ordering::Relaxed);
    }
}

pub fn ligar() {
    BATIDA_MS.store(agora_ms(), Ordering::SeqCst);
    LIGADO.store(true, Ordering::SeqCst);
    let minha = GERACAO.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = std::thread::Builder::new()
        .name("desempenho: vigia".into())
        .spawn(move || vigiar(minha));
}

pub fn desligar() {
    LIGADO.store(false, Ordering::SeqCst);
    GERACAO.fetch_add(1, Ordering::SeqCst);
}

fn vigiar(minha: u64) {
    let mut travada_desde: Option<u64> = None;
    let mut gravou = false;
    while LIGADO.load(Ordering::SeqCst) && GERACAO.load(Ordering::SeqCst) == minha {
        std::thread::sleep(Duration::from_millis(50));
        let agora = agora_ms();
        let ultima = BATIDA_MS.load(Ordering::SeqCst);
        let parada = agora.saturating_sub(ultima);
        match travada_desde {
            None if parada > LIMIAR.as_millis() as u64 => {
                travada_desde = Some(ultima);
                gravou = gravar_fotografia();
            }
            Some(desde) if parada < VOLTOU.as_millis() as u64 => {
                let durou = Duration::from_millis(ultima.saturating_sub(desde));
                super::travamento(epoca() + Duration::from_millis(desde), durou);
                eprintln!(
                    "⏱️ [Desempenho] a interface travou por {} ms{}",
                    durou.as_millis(),
                    if gravou { " (guardado em disco)" } else { "" }
                );
                travada_desde = None;
            }
            _ => {}
        }
    }
}

/// Grava a sessão até aqui. Da thread do vigia, síncrono.
fn gravar_fotografia() -> bool {
    let (Some(pasta), Some(sessao)) = (pasta_dos_pendentes(), super::fotografia_para_o_vigia())
    else {
        return false;
    };
    gravar_em(&pasta, &sessao)
}

pub fn gravar_em(pasta: &std::path::Path, sessao: &domain::desempenho::SessaoDeDesempenho) -> bool {
    let Ok(bytes) = serde_json::to_vec(sessao) else {
        return false;
    };
    if std::fs::create_dir_all(pasta).is_err() {
        return false;
    }
    let id: String = sessao
        .cabecalho
        .id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let temporario = pasta.join(format!("{id}.tmp"));
    std::fs::write(&temporario, bytes).is_ok()
        && std::fs::rename(&temporario, pasta.join(format!("{id}.json"))).is_ok()
}

/// As sessões guardadas pelo vigia, com o arquivo de cada uma.
pub fn pendentes_em(
    pasta: &std::path::Path,
) -> Vec<(PathBuf, domain::desempenho::SessaoDeDesempenho)> {
    let Ok(lista) = std::fs::read_dir(pasta) else {
        return Vec::new();
    };
    lista
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| {
            let sessao = std::fs::read(&p)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
            match sessao {
                Some(s) => Some((p, s)),
                None => {
                    // Ilegível nunca vai entrar: sai para não se repetir.
                    let _ = std::fs::remove_file(&p);
                    None
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_fotografia_vai_e_volta_do_disco() {
        let pasta = tempfile::TempDir::new().unwrap();
        let mut s = domain::desempenho::SessaoDeDesempenho::default();
        s.cabecalho.id = "abc-123/../x".into();
        s.cabecalho.origem = "travamento".into();
        assert!(gravar_em(pasta.path(), &s));
        std::fs::write(pasta.path().join("torto.json"), b"{meio").unwrap();
        let lidas = pendentes_em(pasta.path());
        assert_eq!(lidas.len(), 1);
        assert_eq!(lidas[0].1, s);
        assert!(lidas[0].0.ends_with("abc-123x.json"), "{:?}", lidas[0].0);
        assert!(!pasta.path().join("torto.json").exists());
    }
}
