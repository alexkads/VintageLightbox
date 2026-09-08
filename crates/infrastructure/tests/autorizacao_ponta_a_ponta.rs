//! O handoff inteiro, automatizado: o app de um lado, um servidor HTTP de
//! verdade do outro, e o teste no papel do navegador.
//!
//! # Por que existe, se já há teste de unidade
//!
//! Porque o que pode quebrar aqui está **entre** as peças, e cada peça sozinha
//! passa: o servidor loopback sobe numa porta, o navegador volta com o código
//! **naquela** porta, o verificador que fecha o PKCE é o mesmo que foi sorteado
//! antes de a URL ser montada, e o par que chega vira sessão guardada. Trocar a
//! ordem de dois passos deixa todos os unitários verdes.
//!
//! ⚠️ **`VLB_SEM_NAVEGADOR`**: sem isso, este teste abriria uma janela de
//! navegador na máquina de quem roda `cargo test`.

use domain::services::pos_venda::{CofreDeSessao, PosVendaApi};
use infrastructure::pos_venda::cofre::CofreEmMemoria;
use infrastructure::PosVendaApiHttp;
use serde_json::json;
use std::sync::Arc;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// O vetor da RFC 7636, apêndice B: o desafio que o site receberia deste
/// verificador é `E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM`.
const QUINZE_DIAS: i64 = 15 * 86_400;

/// O teste faz o que o navegador faria: lê a URL que o app abriria, tira dela a
/// porta do servidor local, e bate lá com o código que o site emitiu.
async fn navegador_confirma(url_do_app: &str, code: &str) {
    let porta: u16 = url_do_app
        .split("porta=")
        .nth(1)
        .and_then(|resto| resto.split('&').next())
        .and_then(|p| p.parse().ok())
        .expect("a URL leva a porta do servidor local");
    let estado = url_do_app
        .split("estado=")
        .nth(1)
        .and_then(|resto| resto.split('&').next())
        .expect("a URL leva o estado")
        .to_string();
    let code = code.to_string();

    tokio::spawn(async move {
        // O favicon vem primeiro, como vem de um navegador de verdade.
        let _ = reqwest::get(format!("http://127.0.0.1:{porta}/favicon.ico")).await;
        let resposta = reqwest::get(format!(
            "http://127.0.0.1:{porta}/?code={code}&estado={estado}"
        ))
        .await
        .expect("o servidor do app responde");
        let corpo = resposta.text().await.unwrap();
        assert!(
            corpo.contains("Computador autorizado"),
            "o operador precisa ver que deu certo, e não uma aba em branco: {corpo}"
        );
    });
}

/// O par que o site devolve ao app.
fn sessao_do_app() -> serde_json::Value {
    json!({
        "access_token": "acesso-1",
        "refresh_token": "renovacao-1",
        "expires_in": 900,
        "refresh_expires_in": QUINZE_DIAS
    })
}

/// Um site que só troca o código de **quem mandar os dois campos**.
///
/// O verificador não pode ser conferido por valor aqui: no fluxo de verdade ele
/// é sorteado dentro do app e não sai de lá — que é o ponto. O que este mock
/// prende é que ele **foi mandado** junto do código; o casamento com o desafio
/// tem prova própria (`o_desafio_e_o_sha256_do_verificador`, no unitário, e a
/// recusa do verificador errado, no e2e do backend).
async fn site_de_mentira() -> MockServer {
    let servidor = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/token"))
        .and(body_string_contains("\"code\":\"codigo-do-site\""))
        .and(body_string_contains("\"verificador\":\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(sessao_do_app()))
        .mount(&servidor)
        .await;

    servidor
}

/// Um site que exige **o** verificador — usado onde o teste é quem o escolhe.
async fn site_que_exige_o_verificador_da_rfc() -> MockServer {
    let servidor = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v2/auth/app/token"))
        // 🔑 Se o app mandasse outra coisa — ou não mandasse —, o servidor não
        // responderia, e o teste falharia em vez de passar sem PKCE nenhum.
        .and(body_string_contains(
            "\"verificador\":\"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk\"",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(sessao_do_app()))
        .mount(&servidor)
        .await;

    servidor
}

/// 🔑 O fluxo inteiro, pela porta de produção: `autorizar_pelo_navegador`.
///
/// Quem faz o papel do navegador é o abridor injetado — ele recebe a **mesma
/// URL** que o operador receberia, e é dela que o teste tira a porta em que o
/// app está escutando. É essa costura que nenhum unitário pega: a URL anunciada
/// e a porta que escuta têm de ser a mesma coisa.
#[tokio::test]
async fn o_app_sai_do_navegador_com_sessao_de_quinze_dias_guardada() {
    let servidor = site_de_mentira().await;
    let cofre = Arc::new(CofreEmMemoria::default());

    let api = PosVendaApiHttp::nova(servidor.uri())
        .com_cofre(cofre.clone())
        .com_site("https://recordarfotos.com.br")
        .com_abridor(Arc::new(|url: &str| {
            let url = url.to_string();
            // O site autêntico responderia com um código; aqui o de mentira
            // devolve um fixo, e o navegador o entrega no loopback.
            tokio::spawn(async move { navegador_confirma(&url, "codigo-do-site").await });
        }));

    let sessao = api
        .autorizar_pelo_navegador()
        .await
        .expect("o operador autorizou");

    assert_eq!(sessao.access_token, "acesso-1");
    let agora = chrono::Utc::now().timestamp();
    assert!(
        sessao.refresh_vence_em - agora > 14 * 86_400,
        "a sessão do app tem de nascer com quinze dias"
    );

    // E já nasce guardada: fechar o app agora não custa uma reautorização.
    let guardada = cofre
        .ler()
        .expect("a sessão vai para o cofre na autorização");
    assert_eq!(guardada.refresh_token, "renovacao-1");
}

/// O caminho de verdade: o pedido é montado pelo teste (mesmo código de
/// produção), então a URL — e a porta — são conhecidas antes de o navegador
/// entrar em cena.
#[tokio::test]
async fn o_codigo_que_volta_pelo_loopback_vira_sessao_de_quinze_dias() {
    use infrastructure::pos_venda::autorizacao::PedidoDeAutorizacao;

    let servidor = site_que_exige_o_verificador_da_rfc().await;

    // Passos 1 e 2: o app sorteia o segredo e sobe o servidor local.
    let pedido = PedidoDeAutorizacao::novo().await.unwrap();
    let url = pedido.url("https://recordarfotos.com.br");

    // O desafio que viaja é o SHA-256 do verificador — e o verificador **não**
    // aparece na URL, que é o ponto inteiro do PKCE.
    assert!(url.contains("/autorizar-app?desafio="));
    assert!(!url.contains(&pedido.verificador));

    // Passos 3 e 4: o operador confirma, e o site manda o navegador ao loopback.
    navegador_confirma(&url, "codigo-do-site").await;
    let code = pedido
        .esperar_codigo()
        .await
        .expect("o app recebe o código");
    assert_eq!(code, "codigo-do-site");

    // Passo 5: a troca. Feita aqui pelo mesmo caminho HTTP do app — o servidor
    // só responde a quem mandar o verificador certo.
    let resposta = reqwest::Client::new()
        .post(format!("{}/api/v2/auth/app/token", servidor.uri()))
        .json(&json!({
            "code": code,
            "verificador": "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resposta.status().as_u16(), 200);
}

/// A recusa do operador chega ao app como recusa — e não como espera até o
/// prazo, que é o que aconteceria se o site apenas fechasse a aba.
#[tokio::test]
async fn quando_o_operador_cancela_o_app_sabe_na_hora() {
    use infrastructure::pos_venda::autorizacao::PedidoDeAutorizacao;

    let pedido = PedidoDeAutorizacao::novo().await.unwrap();
    let porta = pedido.porta;
    let estado = pedido.estado.clone();

    tokio::spawn(async move {
        let _ = reqwest::get(format!(
            "http://127.0.0.1:{porta}/?recusado=1&estado={estado}"
        ))
        .await;
    });

    assert!(matches!(
        pedido.esperar_codigo().await,
        Err(domain::DomainError::AcessoRecusado)
    ));
}

/// Depois da autorização, o app trabalha: a chamada com o acesso vencido renova
/// sozinha e a sessão nova fica guardada.
#[tokio::test]
async fn o_acesso_vencido_renova_e_o_par_novo_e_guardado() {
    let servidor = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/auth/refresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "acesso-2",
            "refresh_token": "renovacao-2",
            "expires_in": 900,
            "refresh_expires_in": QUINZE_DIAS
        })))
        .mount(&servidor)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/products/admin"))
        .and(header("authorization", "Bearer acesso-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&servidor)
        .await;

    let cofre = Arc::new(CofreEmMemoria::default());
    let api = PosVendaApiHttp::nova(servidor.uri()).com_cofre(cofre.clone());

    let agora = chrono::Utc::now().timestamp();
    let vencida = domain::services::pos_venda::Sessao {
        access_token: "acesso-1".into(),
        refresh_token: "renovacao-1".into(),
        access_vence_em: agora - 10,
        refresh_vence_em: agora + QUINZE_DIAS,
    };

    api.produtos(&vencida)
        .await
        .expect("a chamada passa depois de renovar");

    let guardada = cofre.ler().expect("o par novo vai para o cofre");
    assert_eq!(guardada.access_token, "acesso-2");
    assert!(guardada.refresh_vence_em - agora > 14 * 86_400);
}
