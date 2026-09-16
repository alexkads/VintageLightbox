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
use infrastructure::{CofreDoSistema, PosVendaApiHttp};
use serde::Serialize;
use tauri::State;

use crate::bytes::Bytes;
use crate::erro::ErroDaPonte;
use crate::navegacao::{DESENVOLVIMENTO, SITE};

/// A API de produção.
const API: &str = "https://api.recordarfotos.com.br";

pub struct ContaDoApp {
    api: PosVendaApiHttp,
    cofre: Arc<CofreDoSistema>,
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
}

impl ContaDoApp {
    pub fn nova() -> Self {
        // `VLB_POS_VENDA_URL` e `VLB_SITE_URL` só valem em depuração, como no
        // resto do app: o binário do balcão fala com produção, sempre.
        let (api, site) = if DESENVOLVIMENTO {
            (
                std::env::var("VLB_POS_VENDA_URL").unwrap_or_else(|_| API.to_string()),
                std::env::var("VLB_SITE_URL").unwrap_or_else(|_| SITE.to_string()),
            )
        } else {
            (API.to_string(), SITE.to_string())
        };
        let cofre = Arc::new(CofreDoSistema::novo());
        ContaDoApp {
            api: PosVendaApiHttp::nova(api)
                .com_site(site)
                .com_cofre(cofre.clone()),
            cofre,
            sessao: tokio::sync::Mutex::new(None),
        }
    }

    async fn sessao(&self) -> Option<Sessao> {
        let mut guardada = self.sessao.lock().await;
        if guardada.is_none() {
            let cofre = self.cofre.clone();
            *guardada = tauri::async_runtime::spawn_blocking(move || cofre.ler())
                .await
                .ok()
                .flatten();
        }
        guardada.clone()
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
    metodo: String,
    caminho: String,
    corpo: Option<String>,
) -> Result<RespostaDaApi, ErroDaPonte> {
    let sessao = conta.sessao().await.ok_or(ErroDaPonte::SemSessao)?;
    let resposta = conta
        .api
        .chamar(&sessao, &metodo, &caminho, corpo)
        .await
        .map_err(da_api)?;
    Ok(RespostaDaApi {
        status: resposta.status,
        tipo: resposta.tipo,
        corpo: resposta.bytes.into(),
    })
}
