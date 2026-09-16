//! Os arquivos que a página pode pedir (DESKTOP_TAURI §6, regra 3).
//!
//! A página nunca inventa um caminho. Ela só pode pedir de volta o que o
//! operador escolheu no seletor nativo, e a conferência acontece aqui, no Rust.
//! Um XSS no site que chamasse `ler_raw("/home/operador/.ssh/id_ed25519")`
//! recebe uma recusa, e não um arquivo.
//!
//! Há dois tipos de raiz: o **arquivo** escolhido no seletor, que vale só para
//! ele, e a **pasta de origem** (o cartão da câmera ou a pasta escolhida para
//! importar), que vale para o que estiver dentro dela.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::erro::ErroDaPonte;

#[derive(Default)]
pub struct RaizesPermitidas {
    arquivos: Mutex<HashSet<PathBuf>>,
    pastas: Mutex<HashSet<PathBuf>>,
}

impl RaizesPermitidas {
    /// Registra um arquivo escolhido pelo operador.
    pub fn permitir(&self, arquivo: &Path) -> Result<PathBuf, ErroDaPonte> {
        let canonico = arquivo
            .canonicalize()
            .map_err(|_| ErroDaPonte::ArquivoInexistente)?;
        self.arquivos
            .lock()
            .expect("a trava das raízes envenenou")
            .insert(canonico.clone());
        Ok(canonico)
    }

    /// Registra uma pasta de origem: tudo o que estiver dentro dela passa a
    /// poder ser lido.
    pub fn permitir_pasta(&self, pasta: &Path) -> Result<PathBuf, ErroDaPonte> {
        let canonica = pasta
            .canonicalize()
            .map_err(|_| ErroDaPonte::ArquivoInexistente)?;
        if !canonica.is_dir() {
            return Err(ErroDaPonte::ArquivoInexistente);
        }
        self.pastas
            .lock()
            .expect("a trava das raízes envenenou")
            .insert(canonica.clone());
        Ok(canonica)
    }

    /// A pasta, só se ela foi registrada como origem.
    pub fn conferir_pasta(&self, caminho: &str) -> Result<PathBuf, ErroDaPonte> {
        let canonica = Path::new(caminho)
            .canonicalize()
            .map_err(|_| ErroDaPonte::ForaDasRaizes)?;
        let pastas = self.pastas.lock().expect("a trava das raízes envenenou");
        if pastas.contains(&canonica) {
            Ok(canonica)
        } else {
            Err(ErroDaPonte::ForaDasRaizes)
        }
    }

    /// Devolve o caminho só se ele foi escolhido, ou se está dentro de uma
    /// pasta de origem.
    ///
    /// 🔑 A comparação é entre caminhos **canônicos**: `../` e links simbólicos
    /// não servem para chegar a um arquivo que não foi escolhido.
    pub fn conferir(&self, caminho: &str) -> Result<PathBuf, ErroDaPonte> {
        let canonico = Path::new(caminho)
            .canonicalize()
            .map_err(|_| ErroDaPonte::ForaDasRaizes)?;
        if !canonico.is_file() {
            return Err(ErroDaPonte::ForaDasRaizes);
        }
        let escolhido = self
            .arquivos
            .lock()
            .expect("a trava das raízes envenenou")
            .contains(&canonico);
        let dentro = self
            .pastas
            .lock()
            .expect("a trava das raízes envenenou")
            .iter()
            .any(|pasta| canonico.starts_with(pasta));
        if escolhido || dentro {
            Ok(canonico)
        } else {
            Err(ErroDaPonte::ForaDasRaizes)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn so_devolve_o_que_foi_escolhido() {
        let pasta = tempfile::tempdir().unwrap();
        let escolhido = pasta.path().join("foto.cr2");
        let outro = pasta.path().join("segredo.txt");
        std::fs::write(&escolhido, b"x").unwrap();
        std::fs::write(&outro, b"x").unwrap();

        let raizes = RaizesPermitidas::default();
        raizes.permitir(&escolhido).unwrap();

        assert!(raizes.conferir(escolhido.to_str().unwrap()).is_ok());
        assert_eq!(
            raizes.conferir(outro.to_str().unwrap()),
            Err(ErroDaPonte::ForaDasRaizes)
        );
    }

    #[test]
    fn um_desvio_por_pasta_pai_nao_engana() {
        let pasta = tempfile::tempdir().unwrap();
        std::fs::create_dir(pasta.path().join("sub")).unwrap();
        let escolhido = pasta.path().join("foto.cr2");
        std::fs::write(&escolhido, b"x").unwrap();

        let raizes = RaizesPermitidas::default();
        raizes.permitir(&escolhido).unwrap();

        let desvio = pasta.path().join("sub").join("..").join("foto.cr2");
        assert!(raizes.conferir(desvio.to_str().unwrap()).is_ok());

        let fora = pasta.path().join("sub").join("..").join("..");
        assert_eq!(
            raizes.conferir(fora.to_str().unwrap()),
            Err(ErroDaPonte::ForaDasRaizes)
        );
    }

    #[test]
    fn a_pasta_de_origem_libera_o_que_esta_dentro_e_so_isso() {
        let raiz = tempfile::tempdir().unwrap();
        let cartao = raiz.path().join("CARTAO");
        std::fs::create_dir_all(cartao.join("DCIM/100CANON")).unwrap();
        let dentro = cartao.join("DCIM/100CANON/IMG_0001.CR2");
        let vizinho = raiz.path().join("CARTAO-falso.txt");
        std::fs::write(&dentro, b"x").unwrap();
        std::fs::write(&vizinho, b"x").unwrap();

        let raizes = RaizesPermitidas::default();
        raizes.permitir_pasta(&cartao).unwrap();

        assert!(raizes.conferir(dentro.to_str().unwrap()).is_ok());
        assert_eq!(
            raizes.conferir(vizinho.to_str().unwrap()),
            Err(ErroDaPonte::ForaDasRaizes)
        );
        let por_fora = cartao.join("..").join("CARTAO-falso.txt");
        assert_eq!(
            raizes.conferir(por_fora.to_str().unwrap()),
            Err(ErroDaPonte::ForaDasRaizes)
        );
        // A pasta em si não é um arquivo a ler.
        assert_eq!(
            raizes.conferir(cartao.to_str().unwrap()),
            Err(ErroDaPonte::ForaDasRaizes)
        );
    }

    #[cfg(unix)]
    #[test]
    fn um_link_simbolico_dentro_da_origem_nao_leva_para_fora() {
        let raiz = tempfile::tempdir().unwrap();
        let cartao = raiz.path().join("CARTAO");
        std::fs::create_dir(&cartao).unwrap();
        let segredo = raiz.path().join("segredo.txt");
        std::fs::write(&segredo, b"x").unwrap();
        std::os::unix::fs::symlink(&segredo, cartao.join("foto.jpg")).unwrap();

        let raizes = RaizesPermitidas::default();
        raizes.permitir_pasta(&cartao).unwrap();
        assert_eq!(
            raizes.conferir(cartao.join("foto.jpg").to_str().unwrap()),
            Err(ErroDaPonte::ForaDasRaizes)
        );
    }

    #[test]
    fn so_a_pasta_registrada_pode_ser_listada() {
        let raiz = tempfile::tempdir().unwrap();
        let cartao = raiz.path().join("CARTAO");
        std::fs::create_dir(&cartao).unwrap();
        let raizes = RaizesPermitidas::default();
        assert_eq!(
            raizes.conferir_pasta(cartao.to_str().unwrap()),
            Err(ErroDaPonte::ForaDasRaizes)
        );
        raizes.permitir_pasta(&cartao).unwrap();
        assert!(raizes.conferir_pasta(cartao.to_str().unwrap()).is_ok());
        assert_eq!(
            raizes.conferir_pasta(raiz.path().to_str().unwrap()),
            Err(ErroDaPonte::ForaDasRaizes)
        );
    }

    #[test]
    fn caminho_inexistente_e_recusado_sem_dizer_se_existe() {
        let raizes = RaizesPermitidas::default();
        assert_eq!(
            raizes.conferir("/nao/existe/foto.cr2"),
            Err(ErroDaPonte::ForaDasRaizes)
        );
    }
}
