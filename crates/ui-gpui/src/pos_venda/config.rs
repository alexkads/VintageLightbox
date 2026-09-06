//! O que o app lembra entre sessões sobre o site: a base da API e o produto
//! escolhido da última vez.
//!
//! 🚨 **A senha não entra aqui, e o token também não.** O arquivo mora ao lado
//! do catálogo, em JSON legível; guardar a credencial do estúdio nele seria
//! deixá-la em todo backup do catálogo. Onde a sessão dorme é o chaveiro do
//! sistema — ver `infrastructure::pos_venda::cofre`.
//!
//! ⚠️ **O `email` saiu daqui em 2026-09-06.** Ele existia para preencher o campo
//! da tela de login, e essa tela deixou de ter campo: quem autentica agora é o
//! navegador, e o e-mail de quem entrou o site já sabe.
//!
//! Mesmo molde de `biblioteca/arranjo.rs`: ler nunca derruba (toda falha é o
//! padrão), gravar falha só imprime.

use std::path::{Path, PathBuf};

use infrastructure::paths::AppPaths;
use serde::{Deserialize, Serialize};

/// Onde a API de produção mora. Sobrescrita pelo arquivo, e pelo arquivo
/// sobrescrita por `VLB_POS_VENDA_URL` — para apontar a homologação sem editar
/// JSON à mão.
pub const BASE_PADRAO: &str = "https://api.recordarfotos.com.br";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configuracao {
    #[serde(default = "base_padrao")]
    pub base_url: String,
    #[serde(default)]
    pub produto_id: Option<String>,
}

fn base_padrao() -> String {
    BASE_PADRAO.to_string()
}

impl Default for Configuracao {
    fn default() -> Self {
        Self {
            base_url: base_padrao(),
            produto_id: None,
        }
    }
}

pub fn caminho() -> PathBuf {
    AppPaths::catalog_root().join("pos-venda.json")
}

/// A configuração, com a variável de ambiente por cima do arquivo.
pub fn ler() -> Configuracao {
    let mut config = ler_de(&caminho()).unwrap_or_default();
    if let Ok(base) = std::env::var("VLB_POS_VENDA_URL") {
        if !base.trim().is_empty() {
            config.base_url = base.trim().to_string();
        }
    }
    config
}

pub fn ler_de(caminho: &Path) -> Option<Configuracao> {
    let texto = std::fs::read_to_string(caminho).ok()?;
    serde_json::from_str(&texto).ok()
}

pub fn gravar(config: &Configuracao) {
    gravar_em(&caminho(), config);
}

pub fn gravar_em(caminho: &Path, config: &Configuracao) {
    let Ok(texto) = serde_json::to_string_pretty(config) else {
        return;
    };
    if let Some(pasta) = caminho.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    if let Err(erro) = std::fs::write(caminho, texto) {
        eprintln!("⚠️  Não foi possível gravar a configuração do pós-venda: {erro}");
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn a_configuracao_gravada_volta_igual_e_arquivo_ruim_vira_padrao() {
        let dir = TempDir::new().unwrap();
        let arquivo = dir.path().join("pos-venda.json");

        let config = Configuracao {
            base_url: "http://localhost:8080".into(),
            produto_id: Some("p1".into()),
        };
        gravar_em(&arquivo, &config);
        assert_eq!(ler_de(&arquivo), Some(config));

        std::fs::write(&arquivo, "{ isto não é json").unwrap();
        assert_eq!(ler_de(&arquivo), None);
        assert_eq!(ler_de(&dir.path().join("nao-existe.json")), None);
    }

    /// 🚨 O arquivo nunca carrega senha nem token — o teste é a regra escrita.
    #[test]
    fn o_arquivo_nao_tem_onde_guardar_senha() {
        let texto = serde_json::to_string(&Configuracao::default()).unwrap();
        assert!(!texto.contains("senha") && !texto.contains("token"));
    }
}
