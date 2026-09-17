//! Os envios em segundo plano, no lugar do service worker (DESKTOP_TAURI §0,
//! etapa D).
//!
//! # O que o service worker fazia, e por que aqui é diferente
//!
//! No site, os *workers* da importação e da revelação guardam cada envio no
//! IndexedDB (`recordarfotos:envios`), e o service worker os sobe quando pode,
//! até com a aba fechada. No app não há service worker. Por isso:
//!
//! 1. a página entrega cada envio guardado ao Rust (`guardar_envio`), e só então
//!    o apaga do IndexedDB. O envio passa a morar no catálogo, fora da limpeza
//!    do WebKit (G12);
//! 2. este laço sobe um de cada vez, pela fila do catálogo (G6);
//! 3. uma recusa (`4xx`) **fica registrada com o motivo** (G7). O service worker
//!    a descartava;
//! 4. falha de rede, `5xx` e `429` voltam à fila, esperando cada vez mais;
//! 5. um bilhete vencido é trocado por um novo antes de mandar, porque um envio
//!    pode esperar a rede por horas;
//! 6. fechar a janela com envio pendente só a esconde. O app termina quando a
//!    fila esvazia (G9).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use infrastructure::CorpoCru;
use serde::Deserialize;
use tauri::ipc::Request;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Notify;

use crate::api::ContaDoApp;
use crate::catalogo::fila::{Contagem, Desfecho, EnvioNaFila, Guardado, Recusado};
use crate::catalogo::Catalogo;
use crate::erro::ErroDaPonte;
use crate::protocolo;

/// Sem nada a fazer, o laço confere a fila a cada tanto.
const INTERVALO: Duration = Duration::from_secs(15);
/// Um bilhete que vence em menos que isto é trocado antes de mandar.
const MARGEM_DO_BILHETE: i64 = 60;

/// O sinal para o laço acordar antes do intervalo.
#[derive(Default)]
pub struct Sincronizador {
    acordar: Notify,
    /// O operador fechou a janela com envio na fila: o app termina quando ela
    /// esvaziar.
    fechar_ao_esvaziar: AtomicBool,
    /// Os envios que a página ainda guarda no IndexedDB (os que um worker está
    /// subindo por conta própria).
    na_pagina: AtomicU32,
    /// O tamanho do catálogo, e quando foi medido: somar a pasta inteira a
    /// cada volta seria ler o disco à toa.
    tamanho_do_catalogo: Mutex<(Option<std::time::Instant>, Option<u64>)>,
}

impl Sincronizador {
    pub fn acordar(&self) {
        self.acordar.notify_one();
    }

    /// O operador abriu o app de novo antes de a fila esvaziar.
    pub fn manter_aberto(&self) {
        self.fechar_ao_esvaziar.store(false, Ordering::SeqCst);
    }

    pub fn fechar_ao_esvaziar(&self) {
        self.fechar_ao_esvaziar.store(true, Ordering::SeqCst);
        self.acordar();
    }
}

fn agora() -> i64 {
    chrono::Utc::now().timestamp()
}

// ── O bilhete ────────────────────────────────────────────────────────────────

/// De que é um bilhete, lido do próprio bilhete (sem conferir a assinatura: isso
/// é com o servidor).
#[derive(Debug, PartialEq, Eq)]
pub struct Bilhete {
    /// A rota da API que emite um novo, com o mesmo alvo.
    pub renovar_em: String,
    pub vence_em: i64,
}

/// O caminho da API (relativo a `/api/v2`) de um endereço de envio do app.
pub fn caminho_do_envio(url: &str) -> Option<String> {
    let url = tauri::Url::parse(url).ok()?;
    let caminho = url
        .path()
        .strip_prefix(protocolo::ENVIO)?
        .strip_prefix("/api/v2")?;
    Some(match url.query() {
        Some(q) => format!("{caminho}?{q}"),
        None => caminho.to_string(),
    })
}

/// Lê `{"p": "bilhete-de-envio/<id>", "e": <vencimento>}` de dentro do bilhete.
pub fn ler_bilhete(caminho: &str) -> Option<Bilhete> {
    #[derive(Deserialize)]
    struct Carga {
        p: String,
        e: i64,
    }
    let token = caminho.split('?').next()?.rsplit('/').next()?;
    let carga = token.split('.').next()?;
    let json = URL_SAFE_NO_PAD.decode(carga.trim_end_matches('=')).ok()?;
    let carga: Carga = serde_json::from_slice(&json).ok()?;
    let (tipo, id) = carga.p.split_once('/')?;
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let renovar_em = match tipo {
        "bilhete-de-envio" => format!("/pos-venda/galerias/{id}/bilhete"),
        "bilhete-de-revelacao" => format!("/pos-venda/fotos/{id}/bilhete-de-revelacao"),
        "bilhete-de-bruto" => format!("/pos-venda/fotos/{id}/bilhete-de-bruto"),
        _ => return None,
    };
    Some(Bilhete {
        renovar_em,
        vence_em: carga.e,
    })
}

// ── O corpo ──────────────────────────────────────────────────────────────────

/// Monta o `multipart/form-data` do envio, como o navegador montaria.
pub fn montar_multipart(
    campos: &BTreeMap<String, String>,
    arquivos: &[(String, String, String, Vec<u8>)],
    fronteira: &str,
) -> CorpoCru {
    // Aspas e quebras de linha num nome quebrariam o cabeçalho. O navegador
    // as troca pelo código, e o servidor as desfaz.
    let seguro = |s: &str| {
        s.replace('"', "%22")
            .replace('\r', "%0D")
            .replace('\n', "%0A")
    };
    let mut corpo = Vec::new();
    for (nome, valor) in campos {
        corpo.extend_from_slice(
            format!(
                "--{fronteira}\r\nContent-Disposition: form-data; name=\"{}\"\r\n\r\n",
                seguro(nome)
            )
            .as_bytes(),
        );
        corpo.extend_from_slice(valor.as_bytes());
        corpo.extend_from_slice(b"\r\n");
    }
    for (campo, nome, tipo, bytes) in arquivos {
        corpo.extend_from_slice(
            format!(
                "--{fronteira}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n",
                seguro(campo),
                seguro(nome),
                seguro(tipo)
            )
            .as_bytes(),
        );
        corpo.extend_from_slice(bytes);
        corpo.extend_from_slice(b"\r\n");
    }
    corpo.extend_from_slice(format!("--{fronteira}--\r\n").as_bytes());
    CorpoCru {
        tipo: format!("multipart/form-data; boundary={fronteira}"),
        bytes: corpo,
    }
}

/// O desfecho pela resposta do servidor, com a regra do service worker, exceto
/// que o `4xx` é guardado em vez de descartado.
pub fn desfecho_da_resposta(status: u16, corpo: &[u8]) -> Desfecho {
    if (200..300).contains(&status) || status == 409 {
        return Desfecho::Feito;
    }
    let mensagem = mensagem_do_corpo(corpo);
    if status == 429 || status >= 500 {
        Desfecho::TentarDepois(format!("{status}: {mensagem}"))
    } else {
        Desfecho::Recusado(format!("{status}: {mensagem}"))
    }
}

fn mensagem_do_corpo(corpo: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(corpo)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.get("message"))
                .and_then(|m| m.as_str().map(str::to_string))
        })
        .unwrap_or_else(|| String::from_utf8_lossy(corpo).chars().take(200).collect())
}

// ── O laço ───────────────────────────────────────────────────────────────────

/// Começa o laço. Antes, o que estava `enviando` quando o app caiu volta à fila.
pub fn iniciar(app: &AppHandle) {
    if let Some(catalogo) = app.try_state::<Mutex<Catalogo>>() {
        let _ = catalogo.lock().expect("catálogo").retomar_envios();
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let trabalhou = rodar_uma(&app).await;
            avisar_contagem(&app);
            if !trabalhou {
                let sincronizador = app.state::<Sincronizador>();
                let _ = tokio::time::timeout(INTERVALO, sincronizador.acordar.notified()).await;
            }
        }
    });
}

/// Uma tentativa. `false` quando não havia nada para agora.
async fn rodar_uma(app: &AppHandle) -> bool {
    let Some(catalogo) = app.try_state::<Mutex<Catalogo>>() else {
        return false;
    };
    let proximo = catalogo.lock().expect("catálogo").proximo_envio(agora());
    let mut envio = match proximo {
        Ok(Some(envio)) => envio,
        Ok(None) => return false,
        Err(erro) => {
            eprintln!("[envios] a fila não abriu: {erro}");
            return false;
        }
    };
    let desfecho = tentar(app, &catalogo, &mut envio).await;
    if crate::navegacao::DESENVOLVIMENTO {
        eprintln!("[envios] {} → {:?}", envio.chave, desfecho);
    }
    let concluido = catalogo
        .lock()
        .expect("catálogo")
        .concluir_envio(&envio, &desfecho, agora());
    if let Err(erro) = concluido {
        eprintln!(
            "[envios] não registrei o desfecho de {}: {erro}",
            envio.chave
        );
    }
    if desfecho == Desfecho::Feito {
        let _ = app.emit("envio-chegou", &envio.chave);
    }
    true
}

async fn tentar(
    app: &AppHandle,
    catalogo: &State<'_, Mutex<Catalogo>>,
    envio: &mut EnvioNaFila,
) -> Desfecho {
    let Some(mut caminho) = caminho_do_envio(&envio.dados.url) else {
        return Desfecho::Recusado(format!(
            "endereço de envio desconhecido: {}",
            envio.dados.url
        ));
    };
    let conta = app.state::<ContaDoApp>();

    // Um bilhete prestes a vencer é trocado antes: o envio pode ter esperado a
    // rede por horas.
    if let Some(bilhete) = ler_bilhete(&caminho) {
        if bilhete.vence_em <= agora() + MARGEM_DO_BILHETE {
            match renovar(&conta, &bilhete).await {
                Ok(novo) => {
                    let url = format!(
                        "{}{}/api/v2{novo}",
                        protocolo::endereco("").as_str().trim_end_matches('/'),
                        protocolo::ENVIO
                    );
                    if let Err(erro) = catalogo
                        .lock()
                        .expect("catálogo")
                        .trocar_bilhete(envio, &url)
                    {
                        return Desfecho::TentarDepois(format!(
                            "não guardei o bilhete novo: {erro}"
                        ));
                    }
                    caminho = novo;
                }
                Err(desfecho) => return desfecho,
            }
        }
    }

    let arquivos = {
        let catalogo = catalogo.lock().expect("catálogo");
        let mut lidos = Vec::with_capacity(envio.dados.arquivos.len());
        for arquivo in &envio.dados.arquivos {
            match catalogo.ler_arquivo_do_envio(arquivo) {
                Ok(bytes) => lidos.push((
                    arquivo.campo.clone(),
                    arquivo.nome.clone(),
                    arquivo.tipo.clone(),
                    bytes,
                )),
                Err(erro) => {
                    return Desfecho::Recusado(format!("o arquivo do envio sumiu: {erro}"))
                }
            }
        }
        lidos
    };
    let fronteira = format!("vlb-{}", envio.chave);
    let corpo = montar_multipart(&envio.dados.campos, &arquivos, &fronteira);

    match conta.api.chamar(None, "POST", &caminho, Some(corpo)).await {
        Ok(resposta) => desfecho_da_resposta(resposta.status, &resposta.bytes),
        Err(erro) => Desfecho::TentarDepois(erro.to_string()),
    }
}

/// Um bilhete novo para o mesmo alvo. Devolve o caminho dele (relativo a `/api/v2`).
async fn renovar(conta: &ContaDoApp, bilhete: &Bilhete) -> Result<String, Desfecho> {
    #[derive(Deserialize)]
    struct Novo {
        caminho: String,
    }
    let Some(sessao) = conta.sessao().await else {
        return Err(Desfecho::TentarDepois(
            "o bilhete venceu, e não há sessão para renovar".into(),
        ));
    };
    let corpo = CorpoCru {
        tipo: "application/json".into(),
        bytes: b"{}".to_vec(),
    };
    let resposta = conta
        .api
        .chamar(Some(&sessao), "POST", &bilhete.renovar_em, Some(corpo))
        .await
        .map_err(|e| Desfecho::TentarDepois(format!("renovar o bilhete: {e}")))?;
    match desfecho_da_resposta(resposta.status, &resposta.bytes) {
        Desfecho::Feito => {}
        Desfecho::Recusado(motivo) => {
            return Err(Desfecho::Recusado(format!("renovar o bilhete: {motivo}")))
        }
        Desfecho::TentarDepois(motivo) => {
            return Err(Desfecho::TentarDepois(format!(
                "renovar o bilhete: {motivo}"
            )))
        }
    }
    let novo: Novo = serde_json::from_slice(&resposta.bytes)
        .map_err(|_| Desfecho::TentarDepois("o bilhete novo veio ilegível".into()))?;
    novo.caminho
        .strip_prefix("/api/v2")
        .map(str::to_string)
        .ok_or_else(|| Desfecho::Recusado(format!("bilhete novo fora da API: {}", novo.caminho)))
}

pub fn contagem(app: &AppHandle) -> Contagem {
    app.try_state::<Mutex<Catalogo>>()
        .and_then(|c| c.lock().expect("catálogo").contagem_da_fila().ok())
        .unwrap_or_default()
}

/// O que a área temporária da importação ainda tem a fazer.
pub fn area_temporaria(app: &AppHandle) -> crate::catalogo::importacao::Situacao {
    app.try_state::<Mutex<Catalogo>>()
        .and_then(|c| c.lock().expect("catálogo").importacao_situacao().ok())
        .unwrap_or_default()
}

/// Há trabalho que fechar a janela interromperia: na fila ou ainda na página.
pub fn ha_envio_pendente(app: &AppHandle) -> bool {
    contagem(app).pendentes > 0
        || area_temporaria(app).a_subir > 0
        || app
            .state::<Sincronizador>()
            .na_pagina
            .load(Ordering::SeqCst)
            > 0
}

/// Avisa as janelas e, se o operador já fechou a janela, termina o app quando a
/// fila esvazia.
pub fn avisar_contagem(app: &AppHandle) {
    let atual = contagem(app);
    let _ = app.emit("fila-de-envios", &atual);
    crate::bandeja::atualizar(app, &estado_da_bandeja(app, atual));
    let fechar = app
        .state::<Sincronizador>()
        .fechar_ao_esvaziar
        .load(Ordering::SeqCst);
    if fechar && !ha_envio_pendente(app) {
        app.exit(0);
    }
}

/// Tudo o que a janelinha da bandeja mostra.
fn estado_da_bandeja(app: &AppHandle, fila: Contagem) -> crate::bandeja::Estado {
    let sincronizador = app.state::<Sincronizador>();
    let (ultimo_envio, proxima_tentativa, raiz) = app
        .try_state::<Mutex<Catalogo>>()
        .map(|c| {
            let c = c.lock().expect("catálogo");
            (
                c.ultimo_envio_concluido().ok().flatten(),
                c.proxima_tentativa().ok().flatten(),
                Some(c.raiz().to_path_buf()),
            )
        })
        .unwrap_or((None, None, None));
    crate::bandeja::Estado {
        fila,
        na_pagina: sincronizador.na_pagina.load(Ordering::SeqCst),
        area: area_temporaria(app),
        conta: app
            .try_state::<ContaDoApp>()
            .and_then(|c| c.email.lock().expect("e-mail").clone()),
        pilha_local: crate::ambiente::na_pilha_local(),
        ultimo_envio,
        proxima_tentativa,
        bytes_do_catalogo: tamanho_do_catalogo(&sincronizador, raiz),
        agora: agora(),
    }
}

/// O último tamanho medido; mede de novo no máximo uma vez por minuto.
fn tamanho_do_catalogo(
    sincronizador: &Sincronizador,
    raiz: Option<std::path::PathBuf>,
) -> Option<u64> {
    const VALIDADE: Duration = Duration::from_secs(60);
    let mut guardado = sincronizador.tamanho_do_catalogo.lock().expect("tamanho");
    let velho = guardado.0.is_none_or(|quando| quando.elapsed() > VALIDADE);
    if let (true, Some(raiz)) = (velho, raiz) {
        // Marca antes de medir: a próxima volta não dispara outra medição.
        guardado.0 = Some(std::time::Instant::now());
        let bytes = crate::bandeja::tamanho_da_pasta(&raiz);
        guardado.1 = Some(bytes);
    }
    guardado.1
}

// ── Os comandos ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct EnvioDaPagina {
    chave: String,
    url: String,
    campos: BTreeMap<String, String>,
    arquivos: Vec<ArquivoDaPagina>,
}

#[derive(Deserialize)]
struct ArquivoDaPagina {
    campo: String,
    nome: String,
    tipo: String,
    base64: String,
}

/// Recebe um envio que a página guardou no IndexedDB. Só com `true` a página
/// apaga a cópia de lá: `false` quer dizer que a mesma chave está subindo agora,
/// e a página entrega de novo depois.
#[tauri::command]
pub async fn guardar_envio(
    request: Request<'_>,
    catalogo: State<'_, Mutex<Catalogo>>,
    sincronizador: State<'_, Sincronizador>,
) -> Result<bool, ErroDaPonte> {
    let tauri::ipc::InvokeBody::Json(valor) = request.body() else {
        return Err(ErroDaPonte::PedidoIncompleto);
    };
    let envio: EnvioDaPagina =
        serde_json::from_value(valor.clone()).map_err(|_| ErroDaPonte::PedidoIncompleto)?;
    if caminho_do_envio(&envio.url).is_none() {
        return Err(ErroDaPonte::Api(format!(
            "endereço de envio desconhecido: {}",
            envio.url
        )));
    }
    let mut arquivos = Vec::with_capacity(envio.arquivos.len());
    for a in envio.arquivos {
        let bytes = STANDARD
            .decode(a.base64)
            .map_err(|_| ErroDaPonte::PedidoIncompleto)?;
        arquivos.push((a.campo, a.nome, a.tipo, bytes));
    }
    let guardado = catalogo
        .lock()
        .expect("catálogo")
        .guardar_envio(&envio.chave, &envio.url, envio.campos, arquivos, agora())
        .map_err(|e| ErroDaPonte::Gravacao(e.to_string()))?;
    sincronizador.acordar();
    Ok(guardado != Guardado::Ocupado)
}

/// A página descartou o que ia subir (a edição da foto, por exemplo).
#[tauri::command]
pub fn esquecer_envio(
    chave: String,
    catalogo: State<'_, Mutex<Catalogo>>,
    app: AppHandle,
) -> Result<bool, ErroDaPonte> {
    let esquecido = catalogo
        .lock()
        .expect("catálogo")
        .esquecer_envio(&chave)
        .map_err(|e| ErroDaPonte::Gravacao(e.to_string()))?;
    let _ = app.emit("fila-de-envios", contagem(&app));
    Ok(esquecido)
}

/// Os envios que o servidor recusou, com o motivo (G7).
#[tauri::command]
pub fn envios_recusados(
    catalogo: State<'_, Mutex<Catalogo>>,
) -> Result<Vec<Recusado>, ErroDaPonte> {
    catalogo
        .lock()
        .expect("catálogo")
        .envios_recusados()
        .map_err(|e| ErroDaPonte::Leitura(e.to_string()))
}

/// A página conta o que ainda está no IndexedDB, para fechar a janela não
/// terminar o app com envio que ainda não passou para cá.
#[tauri::command]
pub fn pendentes_na_pagina(quantos: u32, sincronizador: State<'_, Sincronizador>) {
    sincronizador.na_pagina.store(quantos, Ordering::SeqCst);
}

#[tauri::command]
pub fn fila_de_envios(app: AppHandle) -> Contagem {
    contagem(&app)
}

/// O operador pediu, ou a rede voltou: tenta já o que estava esperando.
#[tauri::command]
pub fn tentar_envios_agora(
    catalogo: State<'_, Mutex<Catalogo>>,
    sincronizador: State<'_, Sincronizador>,
) {
    let _ = catalogo.lock().expect("catálogo").tentar_ja();
    sincronizador.acordar();
}

#[cfg(test)]
mod testes {
    use super::*;

    fn token(p: &str, e: i64) -> String {
        let carga = URL_SAFE_NO_PAD.encode(format!(r#"{{"p":"{p}","e":{e}}}"#));
        format!("{carga}.assinatura")
    }

    #[test]
    fn o_caminho_sai_do_endereco_do_app() {
        assert_eq!(
            caminho_do_envio("vlb://localhost/envio-publico/api/v2/public/pos-venda/envio/abc.def")
                .as_deref(),
            Some("/public/pos-venda/envio/abc.def")
        );
        assert_eq!(
            caminho_do_envio("http://vlb.localhost/envio-publico/api/v2/public/pos-venda/bruto/x")
                .as_deref(),
            Some("/public/pos-venda/bruto/x")
        );
        assert_eq!(
            caminho_do_envio("https://api.recordarfotos.com.br/api/v2/x"),
            None
        );
    }

    #[test]
    fn o_bilhete_diz_como_renovar_e_quando_vence() {
        let caminho = format!(
            "/public/pos-venda/envio/{}",
            token("bilhete-de-envio/9e17341e-cc81", 1_789_609_016)
        );
        assert_eq!(
            ler_bilhete(&caminho),
            Some(Bilhete {
                renovar_em: "/pos-venda/galerias/9e17341e-cc81/bilhete".into(),
                vence_em: 1_789_609_016
            })
        );
        let revelacao = format!("/x/{}", token("bilhete-de-revelacao/f1", 5));
        assert_eq!(
            ler_bilhete(&revelacao).unwrap().renovar_em,
            "/pos-venda/fotos/f1/bilhete-de-revelacao"
        );
        let bruto = format!("/x/{}", token("bilhete-de-bruto/f2", 5));
        assert_eq!(
            ler_bilhete(&bruto).unwrap().renovar_em,
            "/pos-venda/fotos/f2/bilhete-de-bruto"
        );
    }

    #[test]
    fn um_bilhete_estranho_nao_vira_rota() {
        assert_eq!(
            ler_bilhete(&format!("/x/{}", token("outra-coisa/f1", 5))),
            None
        );
        assert_eq!(
            ler_bilhete(&format!("/x/{}", token("bilhete-de-envio/../x", 5))),
            None
        );
        assert_eq!(ler_bilhete("/x/nao-e-bilhete"), None);
    }

    #[test]
    fn o_multipart_e_o_do_navegador() {
        let mut campos = BTreeMap::new();
        campos.insert("nota".to_string(), "5".to_string());
        let corpo = montar_multipart(
            &campos,
            &[(
                "file".into(),
                "Sessão \"1\".jpg".into(),
                "image/jpeg".into(),
                b"JPG".to_vec(),
            )],
            "F",
        );
        assert_eq!(corpo.tipo, "multipart/form-data; boundary=F");
        assert_eq!(
            String::from_utf8(corpo.bytes).unwrap(),
            "--F\r\nContent-Disposition: form-data; name=\"nota\"\r\n\r\n5\r\n\
             --F\r\nContent-Disposition: form-data; name=\"file\"; filename=\"Sessão %221%22.jpg\"\r\nContent-Type: image/jpeg\r\n\r\nJPG\r\n\
             --F--\r\n"
        );
    }

    #[test]
    fn o_desfecho_segue_a_regra_e_guarda_o_4xx() {
        assert_eq!(desfecho_da_resposta(201, b""), Desfecho::Feito);
        assert_eq!(desfecho_da_resposta(409, b""), Desfecho::Feito);
        assert_eq!(
            desfecho_da_resposta(
                410,
                br#"{"error":{"code":"GONE","message":"foto apagada"}}"#
            ),
            Desfecho::Recusado("410: foto apagada".into())
        );
        assert_eq!(
            desfecho_da_resposta(503, b"fora"),
            Desfecho::TentarDepois("503: fora".into())
        );
        assert!(matches!(
            desfecho_da_resposta(429, b""),
            Desfecho::TentarDepois(_)
        ));
    }
}
