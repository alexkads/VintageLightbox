//! 🔒 **Uma cópia só do app por catálogo** (dono, 03/out/2026: *"proteja a
//! aplicação para não abrir várias instâncias do mesmo executável, a não ser
//! que seja por algum motivo específico como teste de carga em
//! desenvolvimento"*).
//!
//! Duas cópias abertas no mesmo catálogo são dois donos do mesmo SQLite, do
//! mesmo cache de prévias e da mesma fila de envios: uma grava a revelação que
//! a outra sobrescreve com o que tinha em memória, os dois laços sobem a mesma
//! foto, e aparecem dois ícones na bandeja. No balcão isso acontece sem querer
//! — o app vai para a bandeja ao fechar, e o operador o abre de novo pelo
//! atalho achando que tinha saído.
//!
//! # Como
//!
//! - **A trava é do sistema** (`File::try_lock`: `flock` no Unix, `LockFileEx`
//!   no Windows) sobre `vintagelightbox.trava`, no catálogo. Quem a solta é o
//!   sistema quando o processo termina — inclusive num panic ou num `kill -9` —,
//!   então nunca sobra trava de um app que caiu.
//! - **A segunda cópia acorda a primeira e sai.** Ela escreve um carimbo em
//!   `vintagelightbox.acordar`; a primeira confere o arquivo a cada
//!   [`PASSO`] e traz a janela para a frente (da bandeja, inclusive). Arquivo
//!   e não porta TCP de propósito: nada de firewall perguntando no Windows.
//! - **A reabertura depois de atualizar espera.** O app antigo abre o novo
//!   (`--reabertura`) e só então sai; sem esperar, o novo acharia a trava ainda
//!   presa, acordaria quem está saindo e o balcão ficaria sem app nenhum.
//!
//! # Exceções
//!
//! - **Outro catálogo é outra cópia.** A trava mora no catálogo, então o
//!   roteiro com `VLB_CATALOG` próprio roda ao lado do app aberto para o dono.
//! - **`VLB_VARIAS_COPIAS=1`** desliga a trava, e só com as ferramentas de
//!   desenvolvimento ligadas (`depuracao::ferramentas_ligadas`): o binário do
//!   balcão nem olha a variável.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A variável que libera várias cópias no mesmo catálogo, em desenvolvimento.
pub const VAR_VARIAS_COPIAS: &str = "VLB_VARIAS_COPIAS";

/// O argumento com que o app se reabre depois de atualizar.
pub const ARG_REABERTURA: &str = "--reabertura";

/// De quanto em quanto tempo a primeira cópia confere se pediram a janela.
pub const PASSO: Duration = Duration::from_millis(400);

/// Quanto a reabertura espera o app antigo largar a trava.
const ESPERA_DA_REABERTURA: Duration = Duration::from_secs(20);

const ARQUIVO_DA_TRAVA: &str = "vintagelightbox.trava";
const ARQUIVO_DE_ACORDAR: &str = "vintagelightbox.acordar";

/// A trava desta cópia. Enquanto ela vive, o catálogo é deste processo.
pub struct Trava {
    _arquivo: File,
    acordar: PathBuf,
    visto: Option<String>,
}

/// O que a abertura encontrou.
pub enum Abertura {
    /// A trava é nossa: segue abrindo.
    Dona(Trava),
    /// Já há uma cópia aberta neste catálogo; ela foi acordada.
    JaAberta,
    /// Sem trava, de propósito (`VLB_VARIAS_COPIAS`) ou porque o arquivo não
    /// abriu — um disco somente leitura não pode impedir o balcão de trabalhar.
    SemTrava,
}

/// A decisão inteira da abertura, lida do ambiente do processo.
pub fn abrir(catalogo: &Path) -> Abertura {
    let liberado = crate::depuracao::ferramentas_ligadas()
        && std::env::var_os(VAR_VARIAS_COPIAS).is_some_and(|v| !v.is_empty() && v != "0");
    if liberado {
        eprintln!("⚠️ [Cópia única] {VAR_VARIAS_COPIAS} ligada: várias cópias no mesmo catálogo");
        return Abertura::SemTrava;
    }
    let reabertura = std::env::args().skip(1).any(|a| a == ARG_REABERTURA);
    let espera = if reabertura {
        ESPERA_DA_REABERTURA
    } else {
        Duration::ZERO
    };
    match tomar(catalogo, espera) {
        Ok(Some(trava)) => Abertura::Dona(trava),
        Ok(None) => {
            acordar(catalogo);
            Abertura::JaAberta
        }
        Err(erro) => {
            eprintln!("⚠️ [Cópia única] a trava do catálogo não abriu: {erro}");
            Abertura::SemTrava
        }
    }
}

/// Tenta tomar a trava do catálogo, insistindo por até `espera`.
///
/// `Ok(None)` é "outra cópia está com ela".
pub fn tomar(catalogo: &Path, espera: Duration) -> std::io::Result<Option<Trava>> {
    std::fs::create_dir_all(catalogo)?;
    let mut arquivo = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(catalogo.join(ARQUIVO_DA_TRAVA))?;
    let limite = Instant::now() + espera;
    loop {
        match arquivo.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < limite => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(std::fs::TryLockError::WouldBlock) => return Ok(None),
            Err(std::fs::TryLockError::Error(erro)) => return Err(erro),
        }
    }
    // O PID só para quem for olhar o arquivo; a trava é o `lock`, não isto.
    let _ = arquivo.set_len(0);
    let _ = writeln!(arquivo, "{}", std::process::id());
    let acordar = catalogo.join(ARQUIVO_DE_ACORDAR);
    let visto = std::fs::read_to_string(&acordar).ok();
    Ok(Some(Trava {
        _arquivo: arquivo,
        acordar,
        visto,
    }))
}

/// O pedido da segunda cópia: "mostre a janela".
pub fn acordar(catalogo: &Path) {
    let carimbo = format!(
        "{} {}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    );
    if let Err(erro) = std::fs::write(catalogo.join(ARQUIVO_DE_ACORDAR), carimbo) {
        eprintln!("⚠️ [Cópia única] não deu para acordar a cópia aberta: {erro}");
    }
}

impl Trava {
    /// Se outra cópia pediu a janela desde a última conferência.
    pub fn pediram_a_janela(&mut self) -> bool {
        let agora = std::fs::read_to_string(&self.acordar).ok();
        if agora.is_some() && agora != self.visto {
            self.visto = agora;
            return true;
        }
        false
    }
}

/// Liga a conferência na aplicação: o pedido da segunda cópia traz a janela
/// para a frente, da bandeja ou de trás de outra janela.
pub fn ligar(mut trava: Trava, cx: &mut gpui_kit::App) {
    cx.spawn(async move |cx| loop {
        cx.background_executor().timer(PASSO).await;
        if trava.pediram_a_janela() {
            eprintln!("🔒 [Cópia única] abriram de novo: a janela vem para a frente");
            cx.update(crate::segundo_plano::trazer_para_a_frente);
        }
    })
    .detach();
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_segunda_copia_nao_toma_a_trava_e_acorda_a_primeira() {
        let pasta = tempfile::tempdir().unwrap();
        let mut primeira = tomar(pasta.path(), Duration::ZERO).unwrap().unwrap();
        assert!(!primeira.pediram_a_janela());

        assert!(tomar(pasta.path(), Duration::ZERO).unwrap().is_none());
        acordar(pasta.path());
        assert!(primeira.pediram_a_janela());
        // Um pedido acorda uma vez só.
        assert!(!primeira.pediram_a_janela());
    }

    #[test]
    fn a_trava_volta_quando_a_primeira_termina() {
        let pasta = tempfile::tempdir().unwrap();
        let primeira = tomar(pasta.path(), Duration::ZERO).unwrap().unwrap();
        drop(primeira);
        assert!(tomar(pasta.path(), Duration::ZERO).unwrap().is_some());
    }

    /// 🔄 A reabertura depois de atualizar: o app antigo ainda está saindo.
    #[test]
    fn a_reabertura_espera_a_antiga_largar() {
        let pasta = tempfile::tempdir().unwrap();
        let antiga = tomar(pasta.path(), Duration::ZERO).unwrap().unwrap();
        let saindo = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            drop(antiga);
        });
        let nova = tomar(pasta.path(), Duration::from_secs(5)).unwrap();
        saindo.join().unwrap();
        assert!(nova.is_some());
    }

    #[test]
    fn outro_catalogo_e_outra_copia() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let _primeira = tomar(a.path(), Duration::ZERO).unwrap().unwrap();
        assert!(tomar(b.path(), Duration::ZERO).unwrap().is_some());
    }
}
