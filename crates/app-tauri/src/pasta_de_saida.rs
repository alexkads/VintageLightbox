//! A pasta para onde a exportação grava (DESKTOP_TAURI §5).
//!
//! No Chrome, `showDirectoryPicker` resolve isso, mas a escolha dura só a aba.
//! No WebKitGTK e no WKWebView a API nem existe, e o arquivo cai na pasta de
//! downloads. Aqui o operador escolhe **uma vez**, e a escolha fica guardada
//! entre aberturas do app.
//!
//! 🔒 A pasta escolhida é a única raiz de escrita (§6, regra 3). A página manda
//! só o **nome** do arquivo, nunca um caminho, e o nome é conferido aqui.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

use crate::erro::ErroDaPonte;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pasta {
    /// O que a tela mostra em "Salvar em:".
    pub nome: String,
    pub caminho: String,
}

impl Pasta {
    fn de(caminho: &Path) -> Self {
        Pasta {
            nome: caminho
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| caminho.to_string_lossy().into_owned()),
            caminho: caminho.to_string_lossy().into_owned(),
        }
    }
}

/// A escolha em vigor, e onde ela é guardada entre aberturas.
pub struct PastaDeSaida {
    atual: Mutex<Option<PathBuf>>,
    registro: Option<PathBuf>,
}

impl PastaDeSaida {
    /// Lê a escolha guardada em `registro`, se ela ainda existir no disco.
    pub fn carregar(registro: Option<PathBuf>) -> Self {
        let atual = registro
            .as_deref()
            .and_then(|r| std::fs::read_to_string(r).ok())
            .map(|texto| PathBuf::from(texto.trim()))
            .filter(|p| p.is_dir());
        PastaDeSaida {
            atual: Mutex::new(atual),
            registro,
        }
    }

    pub fn atual(&self) -> Option<Pasta> {
        self.trava().as_deref().map(Pasta::de)
    }

    pub fn escolher(&self, caminho: &Path) -> Result<Pasta, ErroDaPonte> {
        let canonico = caminho
            .canonicalize()
            .map_err(|_| ErroDaPonte::ArquivoInexistente)?;
        if !canonico.is_dir() {
            return Err(ErroDaPonte::SemPasta);
        }
        if let Some(registro) = &self.registro {
            // Falhar ao guardar não impede a escolha: ela vale até fechar o app.
            if let Some(pai) = registro.parent() {
                let _ = std::fs::create_dir_all(pai);
            }
            let _ = std::fs::write(registro, canonico.to_string_lossy().as_bytes());
        }
        let pasta = Pasta::de(&canonico);
        *self.trava() = Some(canonico);
        Ok(pasta)
    }

    pub fn esquecer(&self) {
        *self.trava() = None;
        if let Some(registro) = &self.registro {
            let _ = std::fs::remove_file(registro);
        }
    }

    /// Os nomes que já estão na pasta. A exportação os usa para não repetir.
    pub fn nomes(&self) -> Result<Vec<String>, ErroDaPonte> {
        let pasta = self.exigir()?;
        let mut nomes: Vec<String> = std::fs::read_dir(&pasta)
            .map_err(|e| ErroDaPonte::Gravacao(e.to_string()))?
            .filter_map(Result::ok)
            .map(|entrada| entrada.file_name().to_string_lossy().into_owned())
            .collect();
        nomes.sort();
        Ok(nomes)
    }

    /// Grava um arquivo novo na pasta.
    ///
    /// 🚨 **Nunca por cima.** No navegador, `getFileHandle(nome, { create: true })`
    /// esvaziava o arquivo existente, e exportar a mesma sessão duas vezes
    /// apagava a primeira. Aqui o arquivo é criado com `create_new`: se o nome
    /// já existe, a gravação é recusada, e a página escolhe outro nome
    /// (`Destinos`).
    pub fn gravar(&self, nome: &str, bytes: &[u8]) -> Result<(), ErroDaPonte> {
        let nome = nome_seguro(nome)?;
        let destino = self.exigir()?.join(nome);
        let mut arquivo = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destino)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::AlreadyExists => ErroDaPonte::JaExiste,
                _ => ErroDaPonte::Gravacao(e.to_string()),
            })?;
        if let Err(e) = arquivo.write_all(bytes).and_then(|_| arquivo.sync_all()) {
            // Um arquivo pela metade parece foto e não é.
            drop(arquivo);
            let _ = std::fs::remove_file(&destino);
            return Err(ErroDaPonte::Gravacao(e.to_string()));
        }
        Ok(())
    }

    fn exigir(&self) -> Result<PathBuf, ErroDaPonte> {
        self.trava().clone().ok_or(ErroDaPonte::SemPasta)
    }

    fn trava(&self) -> std::sync::MutexGuard<'_, Option<PathBuf>> {
        self.atual
            .lock()
            .expect("a trava da pasta de saída envenenou")
    }
}

/// Aceita só um nome de arquivo, sem nada que leve para fora da pasta.
pub fn nome_seguro(nome: &str) -> Result<&str, ErroDaPonte> {
    let invalido = nome.is_empty()
        || nome.len() > 255
        || nome == "."
        || nome == ".."
        || nome.ends_with('.')
        || nome.ends_with(' ')
        || nome.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        });
    if invalido {
        Err(ErroDaPonte::NomeInvalido)
    } else {
        Ok(nome)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com_pasta() -> (tempfile::TempDir, PastaDeSaida) {
        let raiz = tempfile::tempdir().unwrap();
        let destino = raiz.path().join("saida");
        std::fs::create_dir(&destino).unwrap();
        let pasta = PastaDeSaida::carregar(Some(raiz.path().join("config").join("pasta.txt")));
        pasta.escolher(&destino).unwrap();
        (raiz, pasta)
    }

    #[test]
    fn sem_escolha_nao_grava() {
        let pasta = PastaDeSaida::carregar(None);
        assert_eq!(pasta.gravar("a.jpg", b"x"), Err(ErroDaPonte::SemPasta));
    }

    #[test]
    fn grava_e_lista() {
        let (_raiz, pasta) = com_pasta();
        pasta.gravar("b.jpg", b"1").unwrap();
        pasta.gravar("a.jpg", b"2").unwrap();
        assert_eq!(pasta.nomes().unwrap(), vec!["a.jpg", "b.jpg"]);
    }

    #[test]
    fn nunca_grava_por_cima() {
        let (_raiz, pasta) = com_pasta();
        pasta.gravar("a.jpg", b"primeira").unwrap();
        assert_eq!(
            pasta.gravar("a.jpg", b"segunda"),
            Err(ErroDaPonte::JaExiste)
        );
        let caminho = PathBuf::from(pasta.atual().unwrap().caminho).join("a.jpg");
        assert_eq!(std::fs::read(caminho).unwrap(), b"primeira");
    }

    #[test]
    fn um_nome_nao_sai_da_pasta() {
        let (raiz, pasta) = com_pasta();
        for nome in [
            "../fora.jpg",
            "..",
            "sub/a.jpg",
            "sub\\a.jpg",
            "",
            "c:a.jpg",
            "a.jpg.",
            "a\u{0}.jpg",
        ] {
            assert_eq!(
                pasta.gravar(nome, b"x"),
                Err(ErroDaPonte::NomeInvalido),
                "{nome:?}"
            );
        }
        assert!(!raiz.path().join("fora.jpg").exists());
    }

    #[test]
    fn nomes_comuns_passam() {
        for nome in ["DSC_0001.jpg", "Sessão da Maria (2).tif", ".oculto.png"] {
            assert_eq!(nome_seguro(nome), Ok(nome));
        }
    }

    #[test]
    fn a_escolha_sobrevive_a_reabrir_o_app() {
        let (raiz, pasta) = com_pasta();
        let escolhida = pasta.atual().unwrap();
        let reaberta = PastaDeSaida::carregar(Some(raiz.path().join("config").join("pasta.txt")));
        assert_eq!(reaberta.atual(), Some(escolhida));

        reaberta.esquecer();
        let de_novo = PastaDeSaida::carregar(Some(raiz.path().join("config").join("pasta.txt")));
        assert_eq!(de_novo.atual(), None);
    }

    #[test]
    fn pasta_guardada_que_sumiu_e_ignorada() {
        let (raiz, pasta) = com_pasta();
        std::fs::remove_dir(PathBuf::from(pasta.atual().unwrap().caminho)).unwrap();
        let reaberta = PastaDeSaida::carregar(Some(raiz.path().join("config").join("pasta.txt")));
        assert_eq!(reaberta.atual(), None);
    }
}
