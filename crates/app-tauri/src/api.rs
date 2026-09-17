//! A conta e a API, para a tela empacotada (DESKTOP_TAURI §0, etapa A).
//!
//! O site guarda a sessão em cookie e chama a API do servidor. O app faz as duas
//! coisas aqui, com o mesmo cliente do app GPUI (`PosVendaApiHttp`):
//!
//! - **entrar** é o handoff pelo navegador do sistema (PKCE): o app nunca vê a
//!   senha, e a sessão vai para o chaveiro;
//! - **cada pedido** sai daqui com o token de agora, renovado quando vence. Sair
//!   pelo Rust também evita o CORS do backend, que aceita uma origem só.

use std::sync::Arc;

use domain::services::pos_venda::{CofreDeSessao, PosVendaApi, Sessao};
use domain::DomainError;
use infrastructure::{CofreDoSistema, CorpoCru, PosVendaApiHttp};
use serde::Deserialize;
use serde::Serialize;
use tauri::State;

use crate::bytes::Bytes;
use crate::erro::ErroDaPonte;
use crate::navegacao::{DESENVOLVIMENTO, SITE};

/// A API de produção.
const API: &str = "https://api.recordarfotos.com.br";

pub struct ContaDoApp {
    pub(crate) api: PosVendaApiHttp,
    cofre: Arc<dyn CofreDeSessao>,
    /// A sessão lida do chaveiro, guardada depois da primeira leitura.
    ///
    /// 🚨 **Ler o chaveiro pode demorar o quanto o operador quiser.** Na
    /// primeira vez que um binário novo o lê, o macOS abre um diálogo pedindo
    /// permissão. Um comando síncrono roda na thread principal, e a janela
    /// inteira congelou esperando essa resposta (2026-09-16). Por isso a
    /// leitura sai da thread principal e acontece uma vez só.
    ///
    /// Esta é só a semente: quem sabe o token de agora é o `PosVendaApiHttp`,
    /// que renova por baixo.
    sessao: tokio::sync::Mutex<Option<Sessao>>,
    /// O e-mail da conta, lido da resposta de `/auth/me` que a tela já pede.
    /// É o que a bandeja mostra.
    pub(crate) email: std::sync::Mutex<Option<String>>,
}

/// O item do app Tauri no chaveiro, separado do app GPUI.
const SERVICO_NO_CHAVEIRO: &str = "br.com.recordarfotos.vintagelightbox.tauri";

impl ContaDoApp {
    /// `pasta` é a pasta de dados do app, onde o build de depuração guarda a
    /// sessão (ver [`CofreEmArquivo`]).
    pub fn nova(pasta: Option<std::path::PathBuf>) -> Self {
        // `VLB_POS_VENDA_URL` e `VLB_SITE_URL` só valem em depuração, como no
        // resto do app: o binário do balcão fala com produção, sempre.
        let local = crate::ambiente::api_local();
        let (api, site) = match &local {
            Some(api) => (
                api.clone(),
                std::env::var("VLB_SITE_URL").unwrap_or_else(|_| "http://localhost:8001".into()),
            ),
            None => (API.to_string(), SITE.to_string()),
        };
        // 🔧 Em depuração, cada recompilação é outro binário para o macOS, que
        // pergunta de novo pelo chaveiro. A sessão fica num arquivo do app.
        let cofre: Arc<dyn CofreDeSessao> = match pasta {
            // A pilha local tem a própria sessão, e não herda a do GPUI, que é
            // de produção.
            Some(pasta) if local.is_some() => Arc::new(CofreEmArquivo {
                arquivo: pasta.join("sessao-local.json"),
                herdar_do_gpui: false,
            }),
            Some(pasta) if DESENVOLVIMENTO => Arc::new(CofreEmArquivo {
                arquivo: pasta.join("sessao-dev.json"),
                herdar_do_gpui: true,
            }),
            _ => Arc::new(CofreDoSistema::com_servico(SERVICO_NO_CHAVEIRO)),
        };
        ContaDoApp {
            api: PosVendaApiHttp::nova(api)
                .com_site(site)
                .com_cofre(cofre.clone()),
            cofre,
            sessao: tokio::sync::Mutex::new(None),
            email: std::sync::Mutex::new(None),
        }
    }

    pub(crate) async fn sessao(&self) -> Option<Sessao> {
        let mut guardada = self.sessao.lock().await;
        if guardada.is_none() {
            let cofre = self.cofre.clone();
            let inicio = std::time::Instant::now();
            *guardada = tauri::async_runtime::spawn_blocking(move || cofre.ler())
                .await
                .ok()
                .flatten();
            if DESENVOLVIMENTO {
                eprintln!("[tempo] chaveiro: {:?}", inicio.elapsed());
            }
        }
        guardada.clone()
    }
}

/// A sessão num arquivo, só em build de depuração.
///
/// 🔒 O arquivo nasce com permissão só do usuário. Ele existe para o
/// desenvolvimento não parar a cada recompilação; o binário do balcão usa o
/// chaveiro do sistema.
struct CofreEmArquivo {
    arquivo: std::path::PathBuf,
    /// Sem arquivo ainda, traz a sessão do app GPUI (só em produção).
    herdar_do_gpui: bool,
}

#[derive(Serialize, Deserialize)]
struct SessaoEmArquivo {
    access_token: String,
    refresh_token: String,
    access_vence_em: i64,
    refresh_vence_em: i64,
}

impl CofreDeSessao for CofreEmArquivo {
    fn guardar(&self, sessao: &Sessao) {
        let _ = std::fs::remove_file(self.marca_de_saida());
        let guardada = SessaoEmArquivo {
            access_token: sessao.access_token.clone(),
            refresh_token: sessao.refresh_token.clone(),
            access_vence_em: sessao.access_vence_em,
            refresh_vence_em: sessao.refresh_vence_em,
        };
        let Ok(texto) = serde_json::to_string(&guardada) else {
            return;
        };
        if let Some(pai) = self.arquivo.parent() {
            let _ = std::fs::create_dir_all(pai);
        }
        let _ = std::fs::write(&self.arquivo, texto);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.arquivo, std::fs::Permissions::from_mode(0o600));
        }
    }

    fn ler(&self) -> Option<Sessao> {
        let Ok(bytes) = std::fs::read(&self.arquivo) else {
            // Sem arquivo ainda: traz, uma vez, a sessão do app GPUI no chaveiro.
            // O macOS pergunta essa vez, e o arquivo evita todas as seguintes.
            // 🚨 **Quem saiu não volta sozinho** (dono, 2026-09-16: "não estou
            // conseguindo deslogar"). Sem esta marca, sair apagava o arquivo e
            // a leitura seguinte trazia de novo a sessão do GPUI.
            if !self.herdar_do_gpui || self.marca_de_saida().exists() {
                return None;
            }
            let do_gpui = CofreDoSistema::novo().ler()?;
            self.guardar(&do_gpui);
            return Some(do_gpui);
        };
        let g: SessaoEmArquivo = serde_json::from_slice(&bytes).ok()?;
        Some(Sessao {
            access_token: g.access_token,
            refresh_token: g.refresh_token,
            access_vence_em: g.access_vence_em,
            refresh_vence_em: g.refresh_vence_em,
        })
    }

    fn esquecer(&self) {
        let _ = std::fs::remove_file(&self.arquivo);
        let _ = std::fs::write(self.marca_de_saida(), b"");
    }
}

impl CofreEmArquivo {
    /// Existe depois de "Sair", e some na próxima entrada.
    fn marca_de_saida(&self) -> std::path::PathBuf {
        self.arquivo.with_extension("saiu")
    }
}

fn da_api(erro: DomainError) -> ErroDaPonte {
    match erro {
        DomainError::AcessoRecusado => ErroDaPonte::SemSessao,
        outro => ErroDaPonte::Api(outro.to_string()),
    }
}

/// Há uma sessão guardada? Se ela venceu de vez, o primeiro pedido descobre, e a
/// tela volta para a entrada.
#[tauri::command]
pub async fn ha_sessao(conta: State<'_, ContaDoApp>) -> Result<bool, ErroDaPonte> {
    Ok(conta.sessao().await.is_some())
}

/// Abre o navegador do sistema para autorizar este computador e espera a volta.
#[tauri::command]
pub async fn entrar(conta: State<'_, ContaDoApp>) -> Result<(), ErroDaPonte> {
    let sessao = conta.api.autorizar_pelo_navegador().await.map_err(da_api)?;
    *conta.sessao.lock().await = Some(sessao);
    Ok(())
}

#[tauri::command]
pub async fn sair(conta: State<'_, ContaDoApp>) -> Result<(), ErroDaPonte> {
    *conta.sessao.lock().await = None;
    *conta.email.lock().expect("e-mail") = None;
    let cofre = conta.cofre.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || cofre.esquecer()).await;
    Ok(())
}

#[derive(Serialize)]
pub struct RespostaDaApi {
    status: u16,
    tipo: Option<String>,
    #[serde(flatten)]
    corpo: Bytes,
}

/// Um pedido à API em nome da tela. `caminho` é relativo a `/api/v2`.
#[tauri::command]
pub async fn chamar_api(
    conta: State<'_, ContaDoApp>,
    sincronizador: State<'_, crate::sincronizacao::Sincronizador>,
    metodo: String,
    caminho: String,
    corpo: Option<String>,
) -> Result<RespostaDaApi, ErroDaPonte> {
    let sessao = conta.sessao().await.ok_or(ErroDaPonte::SemSessao)?;
    let inicio = std::time::Instant::now();
    let resposta = conta
        .api
        .chamar(
            Some(&sessao),
            &metodo,
            &caminho,
            corpo.map(|texto| CorpoCru {
                tipo: "application/json".into(),
                bytes: texto.into_bytes(),
            }),
        )
        .await
        .map_err(da_api)?;
    if DESENVOLVIMENTO {
        eprintln!(
            "[tempo] {metodo} {caminho}: {:?} ({})",
            inicio.elapsed(),
            resposta.status
        );
    }
    if caminho == "/auth/me" && (200..300).contains(&resposta.status) {
        if let Some(email) = email_da_resposta(&resposta.bytes) {
            *conta.email.lock().expect("e-mail") = Some(email);
            // A bandeja mostra a conta já, e não na próxima volta dos envios.
            sincronizador.acordar();
        }
    }
    Ok(RespostaDaApi {
        status: resposta.status,
        tipo: resposta.tipo,
        corpo: resposta.bytes.into(),
    })
}

/// O e-mail na resposta de `/auth/me`, esteja ele no topo ou dentro de `user`.
fn email_da_resposta(bytes: &[u8]) -> Option<String> {
    let valor: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    ["/email", "/user/email", "/data/email", "/data/user/email"]
        .iter()
        .find_map(|p| {
            valor
                .pointer(p)
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn quem_saiu_nao_herda_a_sessao_do_gpui_ate_entrar_de_novo() {
        let pasta = tempfile::tempdir().unwrap();
        // `herdar_do_gpui` ligado: sem a marca, a leitura iria ao chaveiro.
        let cofre = CofreEmArquivo {
            arquivo: pasta.path().join("sessao-dev.json"),
            herdar_do_gpui: true,
        };
        let sessao = Sessao {
            access_token: "a".into(),
            refresh_token: "r".into(),
            access_vence_em: 10,
            refresh_vence_em: 20,
        };
        cofre.guardar(&sessao);
        cofre.esquecer();
        assert!(cofre.ler().is_none(), "saiu, e a sessão do GPUI não volta");
        cofre.guardar(&sessao);
        assert!(!cofre.marca_de_saida().exists());
        assert_eq!(cofre.ler().unwrap().access_token, "a");
    }

    #[test]
    fn o_email_sai_da_resposta_do_auth_me() {
        assert_eq!(
            email_da_resposta(br#"{"email":"a@b.c","role":"ADMIN"}"#).as_deref(),
            Some("a@b.c")
        );
        assert_eq!(
            email_da_resposta(br#"{"user":{"email":"x@y.z"}}"#).as_deref(),
            Some("x@y.z")
        );
        assert_eq!(email_da_resposta(b"nada"), None);
    }

    #[test]
    fn o_cofre_de_arquivo_guarda_le_e_esquece() {
        let pasta = tempfile::tempdir().unwrap();
        let cofre = CofreEmArquivo {
            arquivo: pasta.path().join("sub").join("sessao-local.json"),
            herdar_do_gpui: false,
        };
        let sessao = Sessao {
            access_token: "a".into(),
            refresh_token: "r".into(),
            access_vence_em: 10,
            refresh_vence_em: 20,
        };
        cofre.guardar(&sessao);
        let lida = cofre.ler().unwrap();
        assert_eq!(lida.access_token, "a");
        assert_eq!(lida.refresh_vence_em, 20);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(&cofre.arquivo)
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(modo & 0o777, 0o600);
        }
        cofre.esquecer();
        assert!(!cofre.arquivo.exists());
        assert!(cofre.marca_de_saida().exists());
        // A pilha local não traz a sessão de produção do app GPUI.
        assert!(cofre.ler().is_none());
    }
}
