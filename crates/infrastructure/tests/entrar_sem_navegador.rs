//! As entradas sem navegador (2026-10-18): e-mail e senha, Google
//! Authenticator e link no e-mail — o cliente HTTP do app contra um site de
//! mentira, conferindo o caminho, o corpo, a frase de cada recusa e que a
//! sessão que volta vai para o cofre, como a do navegador.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use domain::services::pos_venda::{CofreDeSessao, PosVendaApi};
use domain::DomainError;
use infrastructure::pos_venda::cofre::CofreEmMemoria;
use infrastructure::PosVendaApiHttp;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sessao_do_app() -> serde_json::Value {
    json!({
        "access_token": "acesso-1",
        "refresh_token": "renovacao-1",
        "expires_in": 900,
        "refresh_expires_in": 15 * 86_400
    })
}

fn recusado(erro: DomainError) -> String {
    match erro {
        DomainError::Recusado(frase) => frase,
        outro => panic!("esperava Recusado, veio {outro:?}"),
    }
}

#[tokio::test]
async fn a_senha_certa_vira_sessao_guardada_e_a_errada_diz_o_que_errou() {
    let site = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/login"))
        .and(body_partial_json(
            json!({ "email": "balcao@x.com", "senha": "certa" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(sessao_do_app()))
        .mount(&site)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/login"))
        .and(body_partial_json(json!({ "senha": "errada" })))
        .respond_with(ResponseTemplate::new(401))
        .mount(&site)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/login"))
        .and(body_partial_json(json!({ "email": "cliente@x.com" })))
        .respond_with(ResponseTemplate::new(403))
        .mount(&site)
        .await;

    let cofre = Arc::new(CofreEmMemoria::default());
    let api = PosVendaApiHttp::nova(site.uri()).com_cofre(cofre.clone());

    let sessao = api
        .entrar_com_senha(" balcao@x.com ", "certa")
        .await
        .expect("a senha certa entra");
    assert_eq!(sessao.access_token, "acesso-1");
    assert_eq!(
        cofre.ler().unwrap().refresh_token,
        "renovacao-1",
        "vai para o chaveiro"
    );

    let errada = api
        .entrar_com_senha("balcao@x.com", "errada")
        .await
        .unwrap_err();
    assert_eq!(recusado(errada), "e-mail ou senha incorretos.");

    let cliente = api
        .entrar_com_senha("cliente@x.com", "certa")
        .await
        .unwrap_err();
    assert!(recusado(cliente).contains("não opera o pós-venda"));
}

#[tokio::test]
async fn o_codigo_do_authenticator_vai_ao_site_e_volta_sessao() {
    let site = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/autenticador"))
        .and(body_partial_json(
            json!({ "email": "balcao@x.com", "codigo": "123456" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(sessao_do_app()))
        .mount(&site)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/autenticador"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&site)
        .await;

    let api = PosVendaApiHttp::nova(site.uri());
    assert!(api
        .entrar_com_autenticador("balcao@x.com", "123456")
        .await
        .is_ok());
    let pausa = api
        .entrar_com_autenticador("balcao@x.com", "000000")
        .await
        .unwrap_err();
    assert!(recusado(pausa).contains("Espere um minuto"));
}

/// O pedido leva o desafio PKCE e o nome do computador; a espera pergunta com o
/// verificador até o site confirmar, e só então a sessão entra no cofre.
#[tokio::test]
async fn o_link_pede_com_o_desafio_e_espera_ate_o_toque() {
    let site = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/link"))
        .and(body_partial_json(
            json!({ "email": "balcao@x.com", "aparelho": "BALCAO-1" }),
        ))
        .respond_with(
            ResponseTemplate::new(202).set_body_json(json!({ "pedido": "p-1", "expira_em": 900 })),
        )
        .mount(&site)
        .await;
    // Duas perguntas "ainda não" e depois a sessão.
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/link/aguardar"))
        .and(body_partial_json(json!({ "pedido": "p-1" })))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({ "pendente": true })))
        .up_to_n_times(2)
        .mount(&site)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/link/aguardar"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sessao_do_app()))
        .mount(&site)
        .await;

    let cofre = Arc::new(CofreEmMemoria::default());
    let api = PosVendaApiHttp::nova(site.uri())
        .com_cofre(cofre.clone())
        .com_aparelho("BALCAO-1")
        .com_intervalo_do_link(Duration::from_millis(10));

    let pedido = api.pedir_link("balcao@x.com").await.expect("o link sai");
    assert_eq!(pedido.pedido, "p-1");
    assert_eq!(pedido.verificador.len(), 43, "o verificador da RFC 7636");

    let pedidos = site.received_requests().await.unwrap();
    let corpo: serde_json::Value = serde_json::from_slice(&pedidos[0].body).unwrap();
    assert!(
        !corpo.to_string().contains(&pedido.verificador),
        "o verificador nunca sai daqui no pedido"
    );
    assert_eq!(corpo["desafio"].as_str().unwrap().len(), 43);

    let sessao = api
        .aguardar_link(&pedido, Arc::new(AtomicBool::new(false)))
        .await
        .expect("o toque no e-mail entra");
    assert_eq!(sessao.access_token, "acesso-1");
    assert!(cofre.ler().is_some());

    let perguntas = site
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.url.path() == "/api/v2/auth/app/link/aguardar")
        .count();
    assert_eq!(perguntas, 3, "duas esperas e a que trouxe a sessão");
}

#[tokio::test]
async fn cancelar_para_a_espera_e_link_vencido_diz_para_pedir_outro() {
    let site = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/link/aguardar"))
        .and(body_partial_json(json!({ "pedido": "vencido" })))
        .respond_with(ResponseTemplate::new(400))
        .mount(&site)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/link/aguardar"))
        .respond_with(ResponseTemplate::new(202))
        .mount(&site)
        .await;

    let api = PosVendaApiHttp::nova(site.uri()).com_intervalo_do_link(Duration::from_millis(10));
    let pedido = |p: &str| domain::services::pos_venda::PedidoDeLink {
        pedido: p.into(),
        verificador: "v".repeat(43),
        expira_em: 900,
    };

    let vencido = api
        .aguardar_link(&pedido("vencido"), Arc::new(AtomicBool::new(false)))
        .await
        .unwrap_err();
    assert!(recusado(vencido).contains("Peça outro"));

    let cancelar = Arc::new(AtomicBool::new(false));
    let espera = {
        let cancelar = cancelar.clone();
        let api = Arc::new(api);
        tokio::spawn(async move { api.aguardar_link(&pedido("pendente"), cancelar).await })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    cancelar.store(true, std::sync::atomic::Ordering::Relaxed);
    let desfecho = tokio::time::timeout(Duration::from_secs(2), espera)
        .await
        .expect("cancelar para a espera na hora")
        .unwrap();
    assert!(recusado(desfecho.unwrap_err()).contains("cancelada"));
}
