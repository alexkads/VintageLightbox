//! O handoff pelo navegador: como este app recebe uma sessão sem ver senha.
//!
//! # Os cinco passos
//!
//! 1. Sorteia um **verificador** — 32 bytes de aleatório que nunca saem desta
//!    máquina — e calcula o **desafio**, que é o SHA-256 dele.
//! 2. Sobe um servidor numa porta livre de `127.0.0.1`.
//! 3. Abre no navegador `/{site}/autorizar-app?desafio=…&porta=…&estado=…`.
//! 4. O operador entra no site (senha, Google, o que ele já usa) e confirma. O
//!    site redireciona para `http://127.0.0.1:porta/?code=…`, que é o servidor
//!    do passo 2.
//! 5. Quem chama troca o código pelos tokens, mandando o **verificador** junto.
//!
//! # 🚨 Por que o código sozinho não basta (e por que isso não é paranoia)
//!
//! O passo 4 é a parte pública do caminho: o código passa pela barra de
//! endereços, entra no histórico e chega por HTTP puro no `localhost` — onde
//! qualquer processo da máquina pode estar escutando. Se ele **fosse** a sessão,
//! interceptá-lo daria quinze dias de acesso ao estúdio.
//!
//! Com o PKCE, o que o interceptador ganha é um papel de dois minutos que não
//! abre nada: o backend só troca o código por tokens para quem apresentar o
//! verificador, e o verificador nunca esteve na rede.
//!
//! # O `estado` é outra coisa, e resolve outro problema
//!
//! Ele não protege segredo nenhum: serve para o app reconhecer **a própria**
//! resposta. Sem ele, uma aba antiga de uma autorização abandonada, recarregada
//! por acaso, entregaria um código de outro pedido ao servidor que está de pé
//! agora.

use std::time::Duration;

use base64::Engine;
use domain::{DomainError, DomainResult};
use rand::RngCore;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Quanto tempo o app espera o operador decidir na outra janela.
///
/// Cinco minutos: dá para entrar no site, digitar a senha, resolver o segundo
/// fator do Google e ler a tela — e não deixa um servidor pendurado a tarde toda
/// se o operador fechou o navegador e foi almoçar.
const PRAZO_PARA_AUTORIZAR: Duration = Duration::from_secs(300);

/// O que o app leva para o navegador e guarda para a troca.
pub struct PedidoDeAutorizacao {
    /// O segredo que prova, na troca, que quem chega é quem pediu.
    pub verificador: String,
    /// A porta que o servidor local abriu — vai na URL.
    pub porta: u16,
    /// O que o app manda para reconhecer a própria resposta.
    pub estado: String,
    desafio: String,
    ouvinte: TcpListener,
}

impl PedidoDeAutorizacao {
    /// Sorteia o segredo e sobe o servidor. Falha só se não houver porta livre.
    pub async fn novo() -> DomainResult<Self> {
        let verificador = sortear();
        let desafio = desafio_de(&verificador);
        let estado = sortear();

        // Porta 0: o sistema escolhe uma livre. Cravar um número seria disputar
        // com o que já estivesse ali e, pior, tornar previsível onde o código vai
        // cair — um processo local poderia tomar a porta antes.
        let ouvinte = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| DomainError::InfrastructureError(format!("sem porta livre: {e}")))?;

        let porta = ouvinte
            .local_addr()
            .map_err(|e| DomainError::InfrastructureError(format!("porta desconhecida: {e}")))?
            .port();

        Ok(Self {
            verificador,
            desafio,
            porta,
            estado,
            ouvinte,
        })
    }

    /// O endereço a abrir no navegador.
    pub fn url(&self, base_do_site: &str) -> String {
        format!(
            "{}/autorizar-app?desafio={}&porta={}&estado={}&app=VintageLightbox",
            base_do_site.trim_end_matches('/'),
            self.desafio,
            self.porta,
            self.estado
        )
    }

    /// Espera o navegador voltar com o código.
    ///
    /// ⚠️ **Ignora o que não for a resposta.** O navegador pede `/favicon.ico`
    /// sozinho, e uma aba velha pode chegar com outro `estado`: nos dois casos o
    /// servidor responde e **continua esperando**, em vez de tratar aquilo como
    /// o desfecho.
    pub async fn esperar_codigo(&self) -> DomainResult<String> {
        tokio::time::timeout(PRAZO_PARA_AUTORIZAR, self.aceitar_ate_o_codigo())
            .await
            .map_err(|_| {
                DomainError::InfrastructureError(
                    "a autorização não foi concluída no navegador".into(),
                )
            })?
    }

    async fn aceitar_ate_o_codigo(&self) -> DomainResult<String> {
        loop {
            let (mut conexao, _) = self.ouvinte.accept().await.map_err(|e| {
                DomainError::InfrastructureError(format!("o navegador não chegou: {e}"))
            })?;

            let Some(alvo) = primeira_linha(&mut conexao).await else {
                responder(&mut conexao, PAGINA_DE_ERRO).await;
                continue;
            };

            match ler_resposta(&alvo, &self.estado) {
                Resposta::Codigo(code) => {
                    responder(&mut conexao, PAGINA_DE_SUCESSO).await;
                    return Ok(code);
                }
                Resposta::Recusado => {
                    responder(&mut conexao, PAGINA_DE_RECUSA).await;
                    return Err(DomainError::AcessoRecusado);
                }
                // Favicon, aba velha, alguém curioso batendo na porta: responde e
                // segue esperando o que interessa.
                Resposta::Nada => responder(&mut conexao, PAGINA_DE_ERRO).await,
            }
        }
    }
}

/// O que veio na volta do navegador.
#[derive(Debug, PartialEq, Eq)]
enum Resposta {
    Codigo(String),
    Recusado,
    Nada,
}

/// Lê `GET /?code=…&estado=…` — só isso, sem biblioteca de HTTP.
///
/// Um servidor completo aqui seria uma dependência a mais para atender **uma**
/// requisição, de um cliente conhecido, numa porta que só o navegador desta
/// máquina alcança.
fn ler_resposta(alvo: &str, estado_esperado: &str) -> Resposta {
    let Some(query) = alvo.split_once('?').map(|(_, q)| q) else {
        return Resposta::Nada;
    };

    let mut code = None;
    let mut recusado = false;
    let mut estado = None;

    for par in query.split('&') {
        match par.split_once('=') {
            Some(("code", v)) => code = Some(v.to_string()),
            Some(("estado", v)) => estado = Some(v.to_string()),
            Some(("recusado", "1")) => recusado = true,
            _ => {}
        }
    }

    // 🔑 O estado confere **antes** de qualquer conclusão: resposta de outro
    // pedido não é resposta deste, nem para autorizar nem para recusar.
    if estado.as_deref() != Some(estado_esperado) {
        return Resposta::Nada;
    }

    if recusado {
        return Resposta::Recusado;
    }
    match code {
        Some(c) if !c.is_empty() => Resposta::Codigo(c),
        _ => Resposta::Nada,
    }
}

/// O alvo da primeira linha (`GET <alvo> HTTP/1.1`), se houver.
async fn primeira_linha(conexao: &mut TcpStream) -> Option<String> {
    // Um pedido de navegador cabe folgado; o teto existe para uma conexão que
    // despeje dados não crescer a memória do app.
    let mut buffer = vec![0u8; 8 * 1024];
    let lidos = conexao.read(&mut buffer).await.ok()?;
    let texto = String::from_utf8_lossy(&buffer[..lidos]);
    let linha = texto.lines().next()?;
    let mut partes = linha.split_whitespace();
    partes.next()?; // o método
    partes.next().map(str::to_string)
}

async fn responder(conexao: &mut TcpStream, corpo: &str) {
    let resposta = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
        corpo.len()
    );
    let _ = conexao.write_all(resposta.as_bytes()).await;
    let _ = conexao.shutdown().await;
}

/// Abre a URL no navegador padrão.
///
/// Falhar aqui **não é fim de fluxo**: quem chama mostra o endereço na tela para
/// o operador abrir à mão. Numa máquina de estúdio isso é raro, mas o servidor
/// já está de pé esperando de qualquer jeito.
pub fn abrir_no_navegador(url: &str) -> bool {
    let programa = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };

    std::process::Command::new(programa)
        .arg(url)
        .spawn()
        .is_ok()
}

/// 32 bytes de aleatório em base64url — 43 caracteres, o piso da RFC 7636.
fn sortear() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn desafio_de(verificador: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verificador.as_bytes()))
}

const PAGINA_DE_SUCESSO: &str = "<!doctype html><meta charset=utf-8><title>Autorizado</title><body style=\"font-family:system-ui;background:#0a0a0a;color:#e5e5e5;display:grid;place-items:center;height:100vh;margin:0\"><div style=\"text-align:center\"><h1>Computador autorizado</h1><p>Pode fechar esta aba e voltar ao VintageLightbox.</p></div>";
const PAGINA_DE_RECUSA: &str = "<!doctype html><meta charset=utf-8><title>Cancelado</title><body style=\"font-family:system-ui;background:#0a0a0a;color:#e5e5e5;display:grid;place-items:center;height:100vh;margin:0\"><div style=\"text-align:center\"><h1>Autorização cancelada</h1><p>Nada foi liberado. Pode fechar esta aba.</p></div>";
const PAGINA_DE_ERRO: &str = "<!doctype html><meta charset=utf-8><title>VintageLightbox</title><body style=\"font-family:system-ui;background:#0a0a0a;color:#e5e5e5;display:grid;place-items:center;height:100vh;margin:0\"><p>Esta janela é do VintageLightbox. Volte ao aplicativo para autorizar.</p>";

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_desafio_e_o_sha256_do_verificador_em_base64url() {
        // O vetor da RFC 7636, apêndice B — se a conta mudar aqui, o backend
        // recusa toda autorização e o sintoma seria "código inválido" sem pista.
        let verificador = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            desafio_de(verificador),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn o_verificador_tem_o_tamanho_que_a_rfc_exige() {
        let v = sortear();
        assert_eq!(v.len(), 43);
        assert!(v
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
        assert_ne!(v, sortear(), "dois sorteios iguais seriam gerador quebrado");
    }

    #[test]
    fn le_o_codigo_da_volta_do_navegador() {
        assert_eq!(
            ler_resposta("/?code=abc123&estado=st", "st"),
            Resposta::Codigo("abc123".into())
        );
    }

    /// 🚨 Resposta de outro pedido não é resposta deste — nem para autorizar,
    /// nem para recusar.
    #[test]
    fn ignora_a_volta_com_estado_de_outro_pedido() {
        assert_eq!(
            ler_resposta("/?code=abc&estado=outro", "st"),
            Resposta::Nada
        );
        assert_eq!(
            ler_resposta("/?recusado=1&estado=outro", "st"),
            Resposta::Nada
        );
    }

    #[test]
    fn o_favicon_do_navegador_nao_encerra_a_espera() {
        assert_eq!(ler_resposta("/favicon.ico", "st"), Resposta::Nada);
        assert_eq!(ler_resposta("/", "st"), Resposta::Nada);
    }

    #[test]
    fn a_recusa_do_operador_chega_como_recusa() {
        assert_eq!(
            ler_resposta("/?recusado=1&estado=st", "st"),
            Resposta::Recusado
        );
    }

    #[tokio::test]
    async fn a_url_leva_desafio_porta_e_estado() {
        let pedido = PedidoDeAutorizacao::novo().await.unwrap();
        let url = pedido.url("https://recordarfotos.com.br/");

        assert!(url.starts_with("https://recordarfotos.com.br/autorizar-app?desafio="));
        assert!(url.contains(&format!("porta={}", pedido.porta)));
        assert!(url.contains(&format!("estado={}", pedido.estado)));
        // O verificador **não** vai na URL — é o ponto inteiro do PKCE.
        assert!(!url.contains(&pedido.verificador));
    }

    /// O caminho inteiro do servidor local, com um cliente TCP de verdade.
    #[tokio::test]
    async fn o_servidor_local_devolve_o_codigo_que_o_navegador_trouxe() {
        let pedido = PedidoDeAutorizacao::novo().await.unwrap();
        let porta = pedido.porta;
        let estado = pedido.estado.clone();

        let navegador = tokio::spawn(async move {
            // Primeiro o favicon, que o navegador pede sozinho: se ele encerrasse
            // a espera, a autorização morreria antes do código.
            for alvo in [
                "/favicon.ico".to_string(),
                format!("/?code=cod-1&estado={estado}"),
            ] {
                let mut c = TcpStream::connect(("127.0.0.1", porta)).await.unwrap();
                c.write_all(format!("GET {alvo} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
                    .await
                    .unwrap();
                let mut resposta = String::new();
                c.read_to_string(&mut resposta).await.unwrap();
            }
        });

        assert_eq!(pedido.esperar_codigo().await.unwrap(), "cod-1");
        navegador.await.unwrap();
    }
}
