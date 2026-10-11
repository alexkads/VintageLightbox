//! 📡 O app falando de si com o servidor: o aviso de versão nova por SSE, o
//! que o operador fez com ele, e os panics e erros operacionais.
//!
//! # O pedido (dono, 2026-09-26)
//!
//! *"Eu preciso saber se o usuário está recebendo a mensagem de atualização e
//! minha API precisa descobrir que existe uma versão e notificar via SSE para
//! as aplicações comparando a versão dele com a que está disponível."* E: *"um
//! registro de bug panic no VintageLightbox com envio do erro para API, assim
//! poderemos coletar problemas operacionais na Aplicação."*
//!
//! Até a 0.1.18 o balcão era uma caixa preta: um `eprintln!` no Windows não vai
//! a lugar nenhum, e o defeito das "12 fotos que ficaram para trás" foi
//! investigado sem uma linha do computador onde aconteceu.
//!
//! # As peças
//!
//! - [`maquina`]: quem é este computador (id guardado, sistema, versão);
//! - [`deposito`]: o que ainda não chegou ao servidor, em disco;
//! - o gancho de panic ([`instalar_o_gancho_de_panico`]), que escreve no
//!   depósito na hora;
//! - o canal ([`ligar`], [`conta_entrou`]): o fluxo `/app-desktop/eventos` e o
//!   envio do depósito, enquanto a conta está dentro.
//!
//! # 🔑 Tudo aqui é acessório
//!
//! Nenhuma função devolve erro, nenhuma espera a rede, e sem [`ligar`] (os
//! testes, o app antes de montar a API) todas viram nada. Relatar um problema
//! não pode ser a causa de outro.

pub mod deposito;
pub mod maquina;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use domain::services::pos_venda::Sessao;
use domain::DomainError;
use infrastructure::pos_venda::{CorpoCru, PosVendaApiHttp};

use crate::tempo_real::porta::{espera_antes_da_tentativa, SILENCIO_MAXIMO};
use crate::tempo_real::sse::LeitorSse;
use deposito::{Ocorrencia, Reacao, Registro};

/// De quanto em quanto tempo o depósito é revisto, mesmo sem nada novo.
const REVISAO_DO_DEPOSITO: Duration = Duration::from_secs(5 * 60);
/// O mesmo erro, da mesma origem, só é relatado uma vez nesta janela.
const JANELA_DO_REPETIDO: Duration = Duration::from_secs(10 * 60);
/// Erros operacionais por hora, no máximo — o panic não conta.
const ERROS_POR_HORA: u32 = 60;
/// Relatos por envio: o teto do servidor (`RELATOS_POR_ENVIO`).
const RELATOS_POR_ENVIO: usize = 50;

struct Telemetria {
    api: Arc<PosVendaApiHttp>,
    tokio: tokio::runtime::Handle,
    pasta: PathBuf,
    /// A conta aberta — sem ela nada sobe, e o depósito espera.
    sessao: Mutex<Option<Sessao>>,
    /// O fluxo e o envio, vivos enquanto a conta está dentro.
    tarefas: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    /// A versão que o servidor anunciou e a tela ainda não recolheu.
    anuncio: Mutex<Option<crate::atualizacao::novidades::Novidades>>,
    /// O servidor encerrou a sessão desta conta e a tela ainda não saiu: o
    /// motivo, como veio (`sessao_encerrada`), ou vazio quando o app só
    /// descobriu ao reabrir o fluxo.
    encerramento: Mutex<Option<String>>,
    /// Acorda o envio quando entra relato novo.
    novo_relato: tokio::sync::Notify,
    repetidos: Mutex<HashMap<(String, String), Instant>>,
    erros_na_hora: AtomicU32,
    hora: Mutex<Instant>,
}

static TELEMETRIA: OnceLock<Telemetria> = OnceLock::new();

/// A pasta do depósito — a mesma casa do instalador.
fn pasta_do_deposito() -> Option<PathBuf> {
    crate::atualizacao::compilar::casa().map(|c| deposito::pasta_em(&c))
}

/// Liga a telemetria. Chamado **uma vez**, no `main`, com o mesmo cliente (e o
/// mesmo token) do resto do app.
pub fn ligar(api: Arc<PosVendaApiHttp>, tokio: tokio::runtime::Handle) {
    let Some(pasta) = pasta_do_deposito() else {
        return;
    };
    let _ = TELEMETRIA.set(Telemetria {
        api,
        tokio,
        pasta,
        sessao: Mutex::new(None),
        tarefas: Mutex::new(Vec::new()),
        anuncio: Mutex::new(None),
        encerramento: Mutex::new(None),
        novo_relato: tokio::sync::Notify::new(),
        repetidos: Mutex::new(HashMap::new()),
        erros_na_hora: AtomicU32::new(0),
        hora: Mutex::new(Instant::now()),
    });
}

// ── O panic ────────────────────────────────────────────────────────────────

/// O gancho: grava o panic no depósito **antes** de qualquer outra coisa, e
/// depois chama o gancho de sempre (a mensagem no terminal continua saindo).
///
/// Instalado no começo do `main`, antes de janela e rede: um panic na abertura
/// também fica registrado, e sobe na próxima vez que a conta entrar.
pub fn instalar_o_gancho_de_panico() {
    let Some(pasta) = pasta_do_deposito() else {
        return;
    };
    let anterior = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mensagem = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic sem mensagem".into());
        let origem = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "desconhecida".into());
        let thread = std::thread::current()
            .name()
            .unwrap_or("sem nome")
            .to_string();
        let rastro = std::backtrace::Backtrace::force_capture().to_string();
        let c = maquina::deste();
        deposito::guardar(
            &pasta,
            &Registro::Ocorrencia(Ocorrencia {
                id: uuid::Uuid::new_v4().to_string(),
                maquina_id: c.id.clone(),
                tipo: "panico".into(),
                versao: c.versao.clone(),
                sistema: c.sistema.clone(),
                origem,
                mensagem,
                rastro: Some(rastro),
                contexto: serde_json::json!({
                    "thread": thread,
                    "distribuicao": c.distribuicao,
                    "jeito": c.jeito,
                }),
                ocorrida_em: chrono::Utc::now().to_rfc3339(),
            }),
        );
        anterior(info);
    }));
}

// ── Erros operacionais ─────────────────────────────────────────────────────

/// Um erro que o operador não deveria ver acontecer — a importação que
/// falhou, o envio recusado, a atualização que não compilou.
///
/// Repetido (mesma origem, mesma mensagem) só conta uma vez a cada
/// [`JANELA_DO_REPETIDO`], e o total por hora tem teto: um erro que se repete
/// a cada quadro não pode encher o disco do balcão nem o banco.
pub fn erro(origem: &str, mensagem: &str) {
    let Some(t) = TELEMETRIA.get() else {
        return;
    };
    let agora = Instant::now();
    {
        let mut hora = t.hora.lock().unwrap_or_else(|e| e.into_inner());
        if agora.duration_since(*hora) > Duration::from_secs(3600) {
            *hora = agora;
            t.erros_na_hora.store(0, Ordering::Relaxed);
        }
    }
    {
        let mut repetidos = t.repetidos.lock().unwrap_or_else(|e| e.into_inner());
        repetidos.retain(|_, quando| agora.duration_since(*quando) < JANELA_DO_REPETIDO);
        let chave = (origem.to_string(), mensagem.to_string());
        if repetidos.contains_key(&chave) {
            return;
        }
        repetidos.insert(chave, agora);
    }
    if t.erros_na_hora.fetch_add(1, Ordering::Relaxed) >= ERROS_POR_HORA {
        return;
    }
    let c = maquina::deste();
    deposito::guardar(
        &t.pasta,
        &Registro::Ocorrencia(Ocorrencia {
            id: uuid::Uuid::new_v4().to_string(),
            maquina_id: c.id.clone(),
            tipo: "erro".into(),
            versao: c.versao.clone(),
            sistema: c.sistema.clone(),
            origem: origem.to_string(),
            mensagem: mensagem.to_string(),
            rastro: None,
            contexto: serde_json::json!({ "distribuicao": c.distribuicao, "jeito": c.jeito }),
            ocorrida_em: chrono::Utc::now().to_rfc3339(),
        }),
    );
    t.novo_relato.notify_one();
}

/// O `eprintln!` de aviso, que também vira ocorrência.
///
/// 🔑 Os `⚠️` do app são exatamente os "problemas operacionais" do pedido: o
/// erro que o código já percebia e só contava ao terminal — que no balcão
/// ninguém vê.
macro_rules! avisar {
    ($($arg:tt)*) => {{
        let texto = format!($($arg)*);
        eprintln!("{texto}");
        $crate::telemetria::erro(module_path!(), &texto);
    }};
}
pub(crate) use avisar;

// ── O aviso de versão ──────────────────────────────────────────────────────

/// O que se fez com o aviso de versão: `exibida`, `atualizar`, `depois`,
/// `instalada` ou `falhou`.
pub fn reagir(versao: &str, reacao: &str, detalhe: Option<String>) {
    let Some(t) = TELEMETRIA.get() else {
        return;
    };
    deposito::guardar(
        &t.pasta,
        &Registro::Reacao(Reacao {
            maquina: maquina::deste().id.clone(),
            versao: versao.to_string(),
            reacao: reacao.to_string(),
            detalhe,
        }),
    );
    t.novo_relato.notify_one();
}

/// Uma chamada à API com a conta aberta, para os outros acessórios do app (a
/// ferramenta de desempenho manda e lê as sessões por aqui). `None` sem
/// telemetria ligada, sem conta ou sem rede — quem chama guarda para depois.
pub async fn chamar(metodo: &str, caminho: &str, corpo: Option<Vec<u8>>) -> Option<(u16, Vec<u8>)> {
    let t = TELEMETRIA.get()?;
    let sessao = t.sessao.lock().unwrap_or_else(|e| e.into_inner()).clone()?;
    t.api
        .chamar(
            Some(&sessao),
            metodo,
            caminho,
            corpo.map(|bytes| CorpoCru {
                tipo: "application/json".into(),
                bytes,
            }),
        )
        .await
        .ok()
        .map(|r| (r.status, r.bytes))
}

/// Se a telemetria foi ligada — o app de verdade, e não um teste.
pub fn ligada() -> bool {
    TELEMETRIA.get().is_some()
}

/// A versão que o servidor anunciou desde a última vez que a tela olhou.
pub fn tomar_anuncio() -> Option<crate::atualizacao::novidades::Novidades> {
    TELEMETRIA
        .get()?
        .anuncio
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
}

/// O servidor encerrou a sessão desta conta desde a última vez que a tela
/// olhou — o motivo, para a frase da entrada ([`frase_do_encerramento`]).
pub fn tomar_encerramento() -> Option<String> {
    TELEMETRIA
        .get()?
        .encerramento
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
}

/// O motivo, se o evento é o `sessao_encerrada` do servidor
/// (`handlers/app_desktop.rs`: `data: {"tipo": "sessao_encerrada", "motivo": …}`).
/// Evento sem motivo legível ainda encerra — com o motivo vazio.
pub fn motivo_do_encerramento(evento: &crate::tempo_real::sse::EventoSse) -> Option<String> {
    if evento.nome != "sessao_encerrada" {
        return None;
    }
    Some(
        serde_json::from_str::<serde_json::Value>(&evento.dados)
            .ok()
            .and_then(|dados| dados.get("motivo")?.as_str().map(str::to_string))
            .unwrap_or_default(),
    )
}

/// O que a entrada diz a quem foi posto para fora.
pub fn frase_do_encerramento(motivo: &str) -> &'static str {
    match motivo {
        "perfil_de_acesso" => "O perfil de acesso desta conta mudou. Entre de novo para continuar.",
        "papel" => "O papel desta conta mudou. Entre de novo para continuar.",
        "senha" => "A senha desta conta foi trocada. Entre de novo para continuar.",
        "administrador" => "Um administrador encerrou esta sessão. Entre de novo para continuar.",
        "seguranca_do_google" => {
            "O Google avisou de um problema na sua conta Google. Entre de novo para continuar."
        }
        "reuso_de_token" => {
            "Esta sessão foi encerrada por segurança. Entre de novo para continuar."
        }
        _ => "Sua sessão neste computador terminou. Entre de novo para continuar.",
    }
}

// ── A conta ────────────────────────────────────────────────────────────────

/// A conta entrou: abre o fluxo e começa a esvaziar o depósito.
pub fn conta_entrou(sessao: Sessao) {
    let Some(t) = TELEMETRIA.get() else {
        return;
    };
    conta_saiu();
    // O encerramento que sobrou era da conta de antes.
    *t.encerramento.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *t.sessao.lock().unwrap_or_else(|e| e.into_inner()) = Some(sessao.clone());
    let fluxo = t.tokio.spawn(manter_o_fluxo(sessao));
    let envio = t.tokio.spawn(esvaziar_sempre());
    *t.tarefas.lock().unwrap_or_else(|e| e.into_inner()) = vec![fluxo, envio];
}

/// A conta saiu: fecha o fluxo. O depósito fica, para a próxima conta.
pub fn conta_saiu() {
    let Some(t) = TELEMETRIA.get() else {
        return;
    };
    for tarefa in t
        .tarefas
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
    {
        tarefa.abort();
    }
    *t.sessao.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// O fluxo do servidor: abre, lê até cair, espera e abre de novo — a mesma
/// política dos fluxos do chatbot (`tempo_real::porta`).
async fn manter_o_fluxo(sessao: Sessao) {
    let Some(t) = TELEMETRIA.get() else {
        return;
    };
    let caminho = maquina::caminho_do_fluxo(maquina::deste());
    let mut tentativa = 0u32;
    loop {
        let mut leitor = LeitorSse::default();
        let mut abriu = false;
        let mut encerrada: Option<String> = None;
        let resultado = t
            .api
            .escutar(&sessao, &caminho, SILENCIO_MAXIMO, |pedaco| {
                for evento in leitor.ler(pedaco) {
                    match evento.nome.as_str() {
                        "pronto" => {
                            abriu = true;
                            // Conectou: é a hora de entregar o que esperava.
                            t.novo_relato.notify_one();
                        }
                        _ => {
                            if let Some(versao) = anuncio_do_evento(&evento) {
                                *t.anuncio.lock().unwrap_or_else(|e| e.into_inner()) = Some(versao);
                            }
                            if let Some(motivo) = motivo_do_encerramento(&evento) {
                                encerrada = Some(motivo);
                            }
                        }
                    }
                }
            })
            .await;
        if abriu {
            tentativa = 0;
        }
        // 🔒 O servidor encerrou a sessão desta conta (mexeram no perfil de
        // acesso, no papel ou na senha): a tela sai e pede login de novo. O
        // fluxo não reabre — o token daqui já não vale.
        if let Some(motivo) = encerrada {
            *t.encerramento.lock().unwrap_or_else(|e| e.into_inner()) = Some(motivo);
            return;
        }
        match resultado {
            // O servidor recusou o token. Pode ser o aviso que se perdeu (o app
            // estava sem rede quando a sessão foi encerrada): uma chamada
            // qualquer tira a dúvida, porque ela renova antes de desistir.
            Err(DomainError::AcessoRecusado) => {
                match t.api.chamar(Some(&sessao), "GET", "/auth/me", None).await {
                    // Renovou e passou: foi piscada, o fluxo reabre.
                    Ok(resposta) if resposta.status != 401 => {}
                    // Nem renovando: a sessão acabou.
                    Ok(_) | Err(DomainError::AcessoRecusado) => {
                        *t.encerramento.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(String::new());
                        return;
                    }
                    // Sem rede não se conclui nada: tenta de novo adiante.
                    Err(_) => {}
                }
            }
            Err(erro) if erro.to_string().contains("respondeu 403") => return,
            Err(erro) => {
                eprintln!("⚠️ [Telemetria] o fluxo do servidor caiu: {erro}")
            }
            Ok(()) => {}
        }
        tokio::time::sleep(espera_antes_da_tentativa(tentativa)).await;
        tentativa = tentativa.saturating_add(1);
    }
}

/// A versão anunciada, se o evento é o `versao_nova` do servidor
/// (`handlers/app_desktop.rs`: `event: versao_nova`,
/// `data: {"versao": …, "titulo": …, "importante": …}`).
pub fn anuncio_do_evento(
    evento: &crate::tempo_real::sse::EventoSse,
) -> Option<crate::atualizacao::novidades::Novidades> {
    if evento.nome != "versao_nova" {
        return None;
    }
    let dados = serde_json::from_str::<serde_json::Value>(&evento.dados).ok()?;
    let versao = dados.get("versao")?.as_str()?.to_string();
    crate::atualizacao::novidades::versao_em_numeros(&versao)?;
    let titulo = dados
        .get("titulo")
        .and_then(|t| t.as_str())
        .filter(|t| !t.trim().is_empty())
        .map_or_else(|| format!("Versão {versao}"), str::to_string);
    let importante = dados
        .get("importante")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    Some(crate::atualizacao::novidades::Novidades::do_anuncio(
        versao, titulo, importante,
    ))
}

/// Esvazia o depósito quando entra relato novo, quando o fluxo abre e a cada
/// [`REVISAO_DO_DEPOSITO`].
async fn esvaziar_sempre() {
    let Some(t) = TELEMETRIA.get() else {
        return;
    };
    loop {
        esvaziar(t).await;
        let _ = tokio::time::timeout(REVISAO_DO_DEPOSITO, t.novo_relato.notified()).await;
        // Um respiro: dez erros seguidos sobem num envio, e não em dez.
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn esvaziar(t: &Telemetria) {
    let Some(sessao) = t.sessao.lock().unwrap_or_else(|e| e.into_inner()).clone() else {
        return;
    };
    let pendentes = deposito::pendentes(&t.pasta);
    let (ocorrencias, reacoes): (Vec<_>, Vec<_>) = pendentes
        .into_iter()
        .partition(|(_, r)| matches!(r, Registro::Ocorrencia(_)));

    for lote in ocorrencias.chunks(RELATOS_POR_ENVIO) {
        let corpo = serde_json::json!({
            "ocorrencias": lote
                .iter()
                .filter_map(|(_, r)| match r {
                    Registro::Ocorrencia(o) => Some(o),
                    Registro::Reacao(_) => None,
                })
                .collect::<Vec<_>>()
        });
        match enviar(t, &sessao, "/app-desktop/ocorrencias", &corpo).await {
            // 400 é relato que o servidor nunca vai aceitar: sai do depósito
            // para não travar os outros atrás dele.
            Some(status) if (200..300).contains(&status) || status == 400 => {
                deposito::esquecer(&lote.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>());
            }
            _ => return,
        }
    }
    for (caminho, registro) in reacoes {
        let Registro::Reacao(r) = registro else {
            continue;
        };
        match enviar(
            t,
            &sessao,
            "/app-desktop/reacoes",
            &serde_json::to_value(&r).unwrap_or_default(),
        )
        .await
        {
            Some(status) if (200..300).contains(&status) || status == 400 => {
                deposito::esquecer(&[caminho]);
            }
            _ => return,
        }
    }
}

/// O status da resposta, ou `None` sem rede.
async fn enviar(
    t: &Telemetria,
    sessao: &Sessao,
    caminho: &str,
    corpo: &serde_json::Value,
) -> Option<u16> {
    let bytes = serde_json::to_vec(corpo).ok()?;
    t.api
        .chamar(
            Some(sessao),
            "POST",
            caminho,
            Some(CorpoCru {
                tipo: "application/json".into(),
                bytes,
            }),
        )
        .await
        .ok()
        .map(|r| r.status)
}

#[cfg(test)]
mod testes {
    use crate::tempo_real::sse::LeitorSse;

    /// O que o servidor escreve, lido pelo mesmo leitor dos outros fluxos.
    #[test]
    fn o_anuncio_do_servidor_e_lido() {
        let mut leitor = LeitorSse::default();
        let eventos = leitor.ler(
            b"event: pronto\ndata: 1\n\n:keep-alive\n\nevent: versao_nova\ndata: {\"tipo\":\"versao_nova\",\"versao\":\"0.1.19\",\"titulo\":\"t\",\"importante\":true}\n\n",
        );
        let anuncios: Vec<_> = eventos
            .iter()
            .filter_map(super::anuncio_do_evento)
            .collect();
        assert_eq!(anuncios.len(), 1);
        assert_eq!(anuncios[0].versao, "0.1.19");
        assert_eq!(anuncios[0].titulo, "t");
        assert!(anuncios[0].importante);
    }

    #[test]
    fn sem_ligar_tudo_vira_nada() {
        // O teste roda sem `ligar`: nada pode tocar o disco de quem desenvolve.
        super::erro("teste", "nada");
        super::reagir("0.1.19", "exibida", None);
        assert_eq!(super::tomar_anuncio(), None);
        super::conta_saiu();
    }

    #[test]
    fn o_evento_de_sessao_encerrada_traz_o_motivo() {
        let mut leitor = LeitorSse::default();
        let eventos = leitor.ler(
            b"event: pronto\ndata: 1\n\nevent: sessao_encerrada\ndata: {\"tipo\":\"sessao_encerrada\",\"motivo\":\"perfil_de_acesso\"}\n\n",
        );
        assert_eq!(super::motivo_do_encerramento(&eventos[0]), None);
        assert_eq!(
            super::motivo_do_encerramento(&eventos[1]).as_deref(),
            Some("perfil_de_acesso")
        );
        assert!(super::frase_do_encerramento("perfil_de_acesso").contains("perfil de acesso"));

        // Sem motivo legível o evento ainda encerra, com a frase genérica.
        let mut leitor = LeitorSse::default();
        let cru = leitor.ler(b"event: sessao_encerrada\ndata: {}\n\n");
        assert_eq!(super::motivo_do_encerramento(&cru[0]).as_deref(), Some(""));
        assert!(super::frase_do_encerramento("").contains("terminou"));
        assert!(super::frase_do_encerramento("motivo_do_futuro").contains("terminou"));
    }
}
