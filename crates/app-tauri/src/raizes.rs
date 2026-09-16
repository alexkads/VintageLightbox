//! Os arquivos que a página pode pedir (DESKTOP_TAURI §6, regra 3).
//!
//! A página nunca inventa um caminho. Ela só pode pedir de volta o que o
//! operador escolheu no seletor nativo, e a conferência acontece aqui, no Rust.
//! Um XSS no site que chamasse `ler_raw("/home/operador/.ssh/id_ed25519")`
//! recebe uma recusa, e não um arquivo.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::erro::ErroDaPonte;

#[derive(Default)]
pub struct RaizesPermitidas {
    arquivos: Mutex<HashSet<PathBuf>>,
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

    /// Devolve o caminho só se ele foi escolhido antes.
    ///
    /// 🔑 A comparação é entre caminhos **canônicos**: `../` e links simbólicos
    /// não servem para chegar a um arquivo que não foi escolhido.
    pub fn conferir(&self, caminho: &str) -> Result<PathBuf, ErroDaPonte> {
        let canonico = Path::new(caminho)
            .canonicalize()
            .map_err(|_| ErroDaPonte::ForaDasRaizes)?;
        let arquivos = self.arquivos.lock().expect("a trava das raízes envenenou");
        if arquivos.contains(&canonico) {
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
    fn caminho_inexistente_e_recusado_sem_dizer_se_existe() {
        let raizes = RaizesPermitidas::default();
        assert_eq!(
            raizes.conferir("/nao/existe/foto.cr2"),
            Err(ErroDaPonte::ForaDasRaizes)
        );
    }
}
