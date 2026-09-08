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

/// Onde o **site** mora — quem autentica é ele, não a API.
///
/// Ver [`site_para`] para o que acontece quando ele não está escrito.
pub const SITE_PADRAO: &str = "https://recordarfotos.com.br";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configuracao {
    #[serde(default = "base_padrao")]
    pub base_url: String,
    /// O site que abre a tela de autorização. `None` = deduzido da API.
    ///
    /// 🚨 **Existe porque misturar os dois lados falha de um jeito que não se
    /// explica sozinho** (achado do dono, 6/set/2026): com a API em
    /// `localhost:8080` e o site em produção, o operador autoriza, tudo parece
    /// certo na tela do navegador, e o app responde "autorização recusada" — o
    /// código foi assinado com o segredo de um servidor e apresentado a outro.
    #[serde(default)]
    pub site_url: Option<String>,
}

// 📌 Havia um `produto_id` aqui: o modal de publicação lembrava o produto da
// última galeria para não perguntar de novo. O modal saiu em 7/set/2026, e o
// produto passou a ser escolhido onde a web o escolhe — ao criar a sessão. Um
// `produto_id` sobrando num `pos-venda.json` antigo é ignorado na leitura.

/// O site que combina com esta API.
///
/// Deduzir é melhor do que exigir a linha no JSON: os dois casos que existem na
/// prática são a produção e a pilha local, e em nenhum deles o operador deveria
/// precisar saber que há dois endereços. O que estiver escrito em `site_url`
/// (ou em `VLB_SITE_URL`) vence esta dedução.
pub fn site_para(base_da_api: &str) -> String {
    let api = base_da_api.trim_end_matches('/');

    // A pilha local do e-commerce: API em 8080, site em 8001 (`make up`).
    if let Some(resto) = api
        .strip_prefix("http://localhost:")
        .or_else(|| api.strip_prefix("http://127.0.0.1:"))
    {
        let hospedeiro = if api.contains("127.0.0.1") {
            "127.0.0.1"
        } else {
            "localhost"
        };
        let _ = resto;
        return format!("http://{hospedeiro}:8001");
    }

    // Produção, e qualquer coisa parecida com ela: o site é a API sem o `api.`.
    match api.split_once("://") {
        Some((esquema, host)) if host.starts_with("api.") => {
            format!("{esquema}://{}", &host[4..])
        }
        _ => SITE_PADRAO.to_string(),
    }
}

impl Configuracao {
    /// O site desta configuração — escrito, ou deduzido da API.
    pub fn site(&self) -> String {
        self.site_url
            .as_deref()
            .map(|s| s.trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| site_para(&self.base_url))
    }
}

fn base_padrao() -> String {
    BASE_PADRAO.to_string()
}

impl Default for Configuracao {
    fn default() -> Self {
        Self {
            base_url: base_padrao(),
            site_url: None,
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
    if let Ok(site) = std::env::var("VLB_SITE_URL") {
        if !site.trim().is_empty() {
            config.site_url = Some(site.trim().to_string());
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
            site_url: None,
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

    /// 🚨 O defeito de 6/set/2026: API local com site de produção. O código
    /// nasce assinado por um servidor e é apresentado a outro, e a única pista
    /// que o operador tem é "autorização recusada".
    #[test]
    fn o_site_acompanha_a_api_em_vez_de_apontar_para_producao() {
        assert_eq!(
            site_para("http://localhost:8080"),
            "http://localhost:8001",
            "com a API local, autorizar em producao nunca daria certo"
        );
        assert_eq!(site_para("http://127.0.0.1:8080"), "http://127.0.0.1:8001");
        assert_eq!(
            site_para("https://api.recordarfotos.com.br"),
            "https://recordarfotos.com.br"
        );
        // Endereço que não se parece com nenhum dos dois: produção, que é o que
        // o app fazia antes de haver dedução nenhuma.
        assert_eq!(site_para("https://homologacao.exemplo"), SITE_PADRAO);
    }

    #[test]
    fn o_site_escrito_vence_a_deducao() {
        let config = Configuracao {
            base_url: "http://localhost:8080".into(),
            site_url: Some("http://localhost:3000/".into()),
        };
        assert_eq!(config.site(), "http://localhost:3000");
    }
}
