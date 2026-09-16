//! Para onde a janela pode ir (DESKTOP_TAURI §6, regra 4).
//!
//! A janela carrega conteúdo da internet **e** tem a ponte. Um link que a levasse
//! para fora do site colocaria uma página alheia no lugar em que a ponte mora —
//! a capacidade do Tauri ainda recusaria os comandos, mas a primeira trava é não
//! deixar a página estranha entrar.

use url::Url;

/// O site, que só aparece fora do app: autorização e links abertos no navegador.
pub const SITE: &str = "https://recordarfotos.com.br";

/// A tela que a janela principal abre.
pub const ROTA_INICIAL: &str = "/dashboard/sessoes-fotograficas";

#[derive(Debug, PartialEq, Eq)]
pub enum Destino {
    /// Fica na janela.
    NaJanela,
    /// Sai da janela e abre no navegador do sistema.
    NoNavegador,
    /// Não vai a lugar nenhum.
    Recusado,
}

/// Decide o destino de uma navegação.
///
/// `desenvolvimento` libera a pilha local (`localhost`), e só existe em build de
/// depuração: o binário do balcão nunca carrega outra origem com a ponte.
pub fn destino(url: &Url, desenvolvimento: bool) -> Destino {
    let host = url.host_str().unwrap_or_default();
    match url.scheme() {
        "https" if host == "recordarfotos.com.br" || host == "www.recordarfotos.com.br" => {
            Destino::NaJanela
        }
        // 🔑 O "Entrar com Google" sai para o Google e volta pelo
        // `/auth/google/callback`. Barrar aqui quebraria o login. A ponte não
        // corre risco: a capacidade `producao` só vale para `recordarfotos.com.br`.
        "https" if host == "accounts.google.com" => Destino::NaJanela,
        // As páginas que vêm dentro do próprio app (o diagnóstico da Fase 0):
        // `tauri://localhost` no macOS e no Linux, `http://tauri.localhost` no Windows.
        "tauri" => Destino::NaJanela,
        "http" | "https" if host == "tauri.localhost" => Destino::NaJanela,
        "http" if desenvolvimento && (host == "localhost" || host == "127.0.0.1") => {
            Destino::NaJanela
        }
        // Moldura vazia e download gerado pela página.
        "about" | "blob" => Destino::NaJanela,
        "http" | "https" | "mailto" | "tel" => Destino::NoNavegador,
        _ => Destino::Recusado,
    }
}

/// A pilha local só existe em build de depuração (§6, regra 1).
pub const DESENVOLVIMENTO: bool = cfg!(debug_assertions);

/// A tela empacotada, na rota pedida (DESKTOP_TAURI §0).
///
/// 🚫 **Nunca o site remoto.** Em depuração, `VLB_TELA_URL` aponta para o
/// servidor do Vite desta máquina (`pnpm --filter @recordarfotos/desktop dev`),
/// que serve o mesmo código da tela empacotada, para editar sem recompilar.
pub fn tela(rota: &str) -> tauri::WebviewUrl {
    if DESENVOLVIMENTO {
        if let Ok(base) = std::env::var("VLB_TELA_URL") {
            if let Ok(mut url) = Url::parse(&base) {
                url.set_path(rota);
                return tauri::WebviewUrl::External(url);
            }
        }
    }
    tauri::WebviewUrl::App(rota.trim_start_matches('/').into())
}

/// Aplica a regra de navegação, abrindo no navegador do sistema o que é de fora.
pub fn decidir(url: &Url) -> bool {
    let decisao = destino(url, DESENVOLVIMENTO);
    if DESENVOLVIMENTO && decisao != Destino::NaJanela {
        eprintln!("[navegação {decisao:?}] {url}");
    }
    match decisao {
        Destino::NaJanela => true,
        Destino::NoNavegador => {
            let _ = tauri_plugin_opener::open_url(url.as_str(), None::<&str>);
            false
        }
        Destino::Recusado => false,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn de(texto: &str, desenvolvimento: bool) -> Destino {
        destino(&Url::parse(texto).unwrap(), desenvolvimento)
    }

    #[test]
    fn o_site_fica_na_janela() {
        assert_eq!(
            de(
                "https://recordarfotos.com.br/dashboard/sessoes-fotograficas",
                false
            ),
            Destino::NaJanela
        );
        assert_eq!(
            de("https://www.recordarfotos.com.br/", false),
            Destino::NaJanela
        );
    }

    #[test]
    fn o_login_do_google_fica_na_janela() {
        assert_eq!(
            de("https://accounts.google.com/o/oauth2/v2/auth?x=1", false),
            Destino::NaJanela
        );
    }

    #[test]
    fn um_link_de_fora_vai_para_o_navegador() {
        assert_eq!(
            de("https://wa.me/5511999999999", false),
            Destino::NoNavegador
        );
        assert_eq!(
            de("mailto:contato@recordarfotos.com.br", false),
            Destino::NoNavegador
        );
    }

    #[test]
    fn um_dominio_parecido_nao_passa_por_site() {
        assert_eq!(
            de("https://recordarfotos.com.br.golpe.com/", false),
            Destino::NoNavegador
        );
        assert_eq!(
            de("https://golpe-recordarfotos.com.br/", false),
            Destino::NoNavegador
        );
        assert_eq!(
            de("http://recordarfotos.com.br/", false),
            Destino::NoNavegador
        );
    }

    #[test]
    fn a_pilha_local_so_entra_em_desenvolvimento() {
        assert_eq!(de("http://localhost:3001/", true), Destino::NaJanela);
        assert_eq!(de("http://localhost:3001/", false), Destino::NoNavegador);
    }

    #[test]
    fn as_paginas_do_app_ficam_na_janela() {
        assert_eq!(de("tauri://localhost/index.html", false), Destino::NaJanela);
        assert_eq!(
            de("http://tauri.localhost/index.html", false),
            Destino::NaJanela
        );
    }

    #[test]
    fn esquemas_perigosos_sao_recusados() {
        assert_eq!(de("file:///etc/passwd", false), Destino::Recusado);
        assert_eq!(de("data:text/html,oi", false), Destino::Recusado);
    }
}
