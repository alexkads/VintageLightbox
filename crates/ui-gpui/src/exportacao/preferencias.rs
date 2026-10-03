//! O que a exportação lembra de uma vez para a outra: pasta, uso, formato,
//! qualidade e marca d'água.
//!
//! 🔑 **Escolher a mesma pasta e o mesmo logotipo a cada lote é atrito puro.**
//! Até aqui tudo vivia na memória da tela e morria ao fechar o app: o operador
//! abria o modal no dia seguinte e encontrava "escolha uma pasta" de novo.
//!
//! Mora ao lado do catálogo, como o `arranjo-*.json` (`biblioteca/arranjo.rs`),
//! e pelo mesmo motivo: o catálogo de medição não mexe nas escolhas de quem
//! trabalha. E, como lá, **ler daqui nunca derruba nada** — arquivo ruim é o
//! padrão.

use std::path::{Path, PathBuf};

use domain::value_objects::FormatoDeSaida;
use infrastructure::paths::AppPaths;
use serde::{Deserialize, Serialize};

use super::tela::Modo;

/// A qualidade do JPEG quando nada foi escolhido — **92, a do site**
/// (`exportar-dialogo.tsx`): o arquivo não pode depender da tela que o fez.
pub const QUALIDADE_PADRAO: u8 = 92;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferencias {
    pub pasta: Option<PathBuf>,
    pub marca: Option<PathBuf>,
    pub previa: bool,
    pub formato: String,
    pub qualidade: u8,
}

impl Default for Preferencias {
    fn default() -> Self {
        Self {
            pasta: None,
            marca: None,
            previa: false,
            formato: "jpg".into(),
            qualidade: QUALIDADE_PADRAO,
        }
    }
}

impl Preferencias {
    pub fn modo(&self) -> Modo {
        if self.previa {
            Modo::Previa
        } else {
            Modo::Entrega
        }
    }

    pub fn formato(&self) -> FormatoDeSaida {
        FormatoDeSaida::TODOS
            .into_iter()
            .find(|f| f.extensao() == self.formato)
            .unwrap_or_default()
    }

    /// A pasta lembrada, **se ela ainda existe**. Um HD externo desconectado
    /// não pode virar o destino de um lote que vai falhar foto a foto.
    pub fn pasta_que_existe(&self) -> Option<PathBuf> {
        self.pasta.clone().filter(|p| p.is_dir())
    }
}

/// Onde as preferências moram. Nos testes, um arquivo por thread (como o
/// arranjo): a suíte não pode gravar por cima das escolhas de quem a roda.
pub fn caminho() -> PathBuf {
    if cfg!(test) {
        let thread = format!("{:?}", std::thread::current().id()).replace(['(', ')'], "");
        return std::env::temp_dir()
            .join(format!("vlb-testes-{}", std::process::id()))
            .join(format!("{thread}-exportacao.json"));
    }
    AppPaths::catalog_root().join("exportacao.json")
}

pub fn ler_de(caminho: &Path) -> Preferencias {
    std::fs::read_to_string(caminho)
        .ok()
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default()
}

/// Grava. Falhar aqui não interrompe nada: o pior desfecho é a escolha não
/// sobreviver ao fechamento.
pub fn gravar_em(caminho: &Path, preferencias: &Preferencias) {
    let Ok(texto) = serde_json::to_string_pretty(preferencias) else {
        return;
    };
    if let Some(pasta) = caminho.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    let _ = std::fs::write(caminho, texto);
}

/// A pasta Downloads do sistema — o destino de quem nunca escolheu outro,
/// como o "Baixar" do site.
pub fn pasta_dos_downloads() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|d| d.download_dir().map(Path::to_path_buf))
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ida_e_volta() {
        let dir = tempfile::tempdir().unwrap();
        let arquivo = dir.path().join("exportacao.json");
        let escolhidas = Preferencias {
            pasta: Some(dir.path().to_path_buf()),
            marca: Some("/logos/estudio.png".into()),
            previa: true,
            formato: "tif".into(),
            qualidade: 80,
        };
        gravar_em(&arquivo, &escolhidas);
        let lidas = ler_de(&arquivo);
        assert_eq!(lidas, escolhidas);
        assert_eq!(lidas.formato(), FormatoDeSaida::Tiff);
        assert_eq!(lidas.modo(), Modo::Previa);
        assert_eq!(lidas.pasta_que_existe(), Some(dir.path().to_path_buf()));
    }

    /// Arquivo estragado é o padrão, e não um erro.
    #[test]
    fn arquivo_ruim_e_o_padrao() {
        let dir = tempfile::tempdir().unwrap();
        let arquivo = dir.path().join("exportacao.json");
        std::fs::write(&arquivo, "{não é json").unwrap();
        assert_eq!(ler_de(&arquivo), Preferencias::default());
        assert_eq!(ler_de(&dir.path().join("nenhum.json")).qualidade, 92);
    }

    /// A pasta que sumiu (HD desconectado) não é destino.
    #[test]
    fn pasta_que_sumiu_nao_vale() {
        let p = Preferencias {
            pasta: Some("/nao/existe/mais".into()),
            ..Default::default()
        };
        assert_eq!(p.pasta_que_existe(), None);
    }
}
