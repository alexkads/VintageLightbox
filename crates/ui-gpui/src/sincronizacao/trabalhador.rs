//! 📤 O trabalhador da fila: leva ao site, na ordem, o que o balcão fez sem
//! rede.
//!
//! 🔑 **Um gesto por vez, sempre o mais velho.** É a ordem em que o operador
//! trabalhou, e é ela que dá ao site o mesmo final que a tela mostra: a nota 5
//! e depois a 3 chegam nessa ordem.
//!
//! 🔑 **A rede caída nunca gasta o gesto.** Sem resposta (ou o `5xx` do
//! caminho até a API), ele fica onde está e o trabalhador espera — de 2 s a
//! 30 s, dobrando. O site que respondeu e soluçou (o `5xx` que ele explica, o
//! `429`) ganha [`SOLUCOS_DO_SITE`] tentativas. A recusa de regra (`4xx`) tira
//! o gesto da vez na hora: ele vira **conflito**, com a frase do site, e espera
//! o operador na tela "Sincronização". Uma fila travada atrás de uma recusa
//! seria a internet de volta e nada subindo.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use domain::services::pos_venda::{MudancaDaFoto, Sessao};

use super::{e_falta_de_rede, e_passageira, Gesto, Sincronizacao};
use crate::pos_venda::porta::{FotoClassificada, Publicador, Recado};

const ESPERA_INICIAL: Duration = Duration::from_secs(2);
const ESPERA_MAXIMA: Duration = Duration::from_secs(30);
/// Sem nada na fila, ele dorme até ser acordado — ou até isto, por garantia.
const COCHILO: Duration = Duration::from_secs(60);
/// Sem rede e sem fila, de quanto em quanto ele pergunta se ela voltou.
const SONDA: Duration = Duration::from_secs(15);
/// Quantas fotos sobem ao mesmo tempo — o número da esteira.
const EM_VOO: usize = crate::envios::EM_VOO;
/// Quantas vezes um gesto espera o **site** se recompor (o `5xx` que a API
/// explica, o `429` da pausa) antes de virar conflito. A rede caída não conta:
/// por ela o gesto espera o que for preciso.
pub const SOLUCOS_DO_SITE: i64 = 8;
/// O teto de um gesto: a foto grande subindo numa rede lenta.
const TETO_DO_GESTO: Duration = Duration::from_secs(15 * 60);

/// O que o site fez com o gesto.
#[derive(Debug, Clone, PartialEq)]
pub enum Desfecho {
    Chegou(Recado),
    /// Sem rede, ou sem a conta: o gesto espera.
    Esperar(String),
    /// Recusa de regra: vira conflito.
    Recusado(String),
}

/// Lê a resposta do site a um gesto que já foi tentado `tentativas` vezes.
pub fn desfecho(recado: Recado, tentativas: i64) -> Desfecho {
    let frase = match &recado {
        Recado::Falhou(f)
        | Recado::GaleriaNaoAtualizada(f)
        | Recado::EnvioFalhou { frase: f, .. }
        | Recado::Negociou { erro: Some(f), .. } => f.clone(),
        Recado::Json {
            resultado: Err(f), ..
        } => f.clone(),
        Recado::FaltaEmail { frase, .. } => {
            return Desfecho::Recusado(format!("a sessão não tem e-mail: {frase}"))
        }
        _ => return Desfecho::Chegou(recado),
    };
    // A rede caída e a conta vencida não são recusa do gesto: a rede volta,
    // entrar de novo resolve, e o gesto continua valendo — sem limite.
    if e_falta_de_rede(&frase) || frase.contains("recusou o acesso") {
        return Desfecho::Esperar(frase);
    }
    // 🌪️ O site respondeu, mas soluçou (o `5xx` que ele explica) ou pediu uma
    // pausa (`429`, a fila despejando centenas de gestos depois de uma queda):
    // vale esperar e repetir — até um ponto. Persistindo, é defeito, e o
    // operador precisa saber: vira conflito com a frase do site.
    let pausa = frase.contains("respondeu 429");
    if (e_passageira(&frase) || pausa) && tentativas + 1 < SOLUCOS_DO_SITE {
        return Desfecho::Esperar(frase);
    }
    Desfecho::Recusado(frase)
}

/// Manda o gesto pela porta do site e espera a resposta.
fn executar(
    publicador: &dyn Publicador,
    sessao: Sessao,
    gesto: Gesto,
    ocorrido_em: &str,
    canal: Sender<Recado>,
) {
    match gesto {
        Gesto::CriarGaleria(nova) => publicador.criar_galeria(sessao, nova, canal),
        Gesto::AtualizarGaleria {
            galeria_id,
            mudanca,
        } => publicador.atualizar_galeria(sessao, galeria_id, mudanca, canal),
        // 📴 A hora do gesto vai junto: o site deixa como está o campo que
        // alguém mudou depois dela (vale o último gesto a acontecer).
        Gesto::MudarFoto { foto_id, mudanca } => publicador.negociar(
            sessao,
            foto_id,
            MudancaDaFoto {
                ocorrido_em: Some(ocorrido_em.to_string()),
                ..mudanca
            },
            canal,
        ),
        Gesto::RejeitarDaNuvem { foto_no_site, .. } => publicador.negociar(
            sessao,
            foto_no_site,
            MudancaDaFoto {
                rejeitada: Some(true),
                ocorrido_em: Some(ocorrido_em.to_string()),
                ..Default::default()
            },
            canal,
        ),
        // 🔑 **A nota vai do catálogo, e não da fila.** A da fila é a do
        // instante em que o gesto foi guardado; o operador pode ter dado outra
        // depois, sem rede, e a subida que só sai agora tem de levar a última.
        // Sem nota explícita, o caso de uso lê a do catálogo (`publicar::subir`).
        Gesto::SubirClassificada { galeria_id, foto } => publicador.subir_classificada(
            sessao,
            galeria_id,
            FotoClassificada { nota: None, ..foto },
            canal,
        ),
        Gesto::SalvarRevelacao {
            foto_no_site,
            parametros,
        } => {
            let (ajustes, corte) = crate::revelacao::persistencia::de_json(&parametros);
            let corte = crate::revelacao::persistencia::para_crop_settings(&corte);
            publicador.salvar_revelacao(sessao, foto_no_site, ajustes, corte, canal)
        }
        Gesto::Avisar { galeria_id } => publicador.avisar(sessao, galeria_id, canal),
        Gesto::PedidoJson {
            metodo,
            caminho,
            corpo,
        } => publicador.pedir_json(
            sessao,
            crate::pos_venda::porta::PedidoJson {
                rotulo: "fila",
                metodo: super::caixa::metodo_estatico(&metodo),
                caminho,
                corpo: Some(corpo),
            },
            canal,
        ),
    }
}

/// Quem mais precisa saber de uma sessão que trocou de id: as fotos do
/// catálogo que apontam para ela (`photos.sessao_id`).
pub type AoTrocarGaleria = Arc<dyn Fn(String, String) + Send + Sync>;

/// O trabalhador montado.
pub struct Trabalhador {
    pub sinc: Arc<Sincronizacao>,
    /// A porta que fala com o site — **a de dentro**, nunca a local-primeiro:
    /// a falha de rede aqui é para o trabalhador, e não para voltar à fila.
    pub site: Arc<dyn Publicador>,
    pub sessao: Arc<Mutex<Option<Sessao>>>,
    pub ao_trocar_galeria: AoTrocarGaleria,
}

impl Trabalhador {
    /// Liga o laço no tokio. Vive enquanto o app viver.
    pub fn ligar(self) {
        let tokio = self.sinc.tokio().clone();
        tokio.spawn(async move { self.laco().await });
    }

    async fn laco(self) {
        let mut espera = ESPERA_INICIAL;
        loop {
            match self.uma_volta().await {
                Volta::Andou => espera = ESPERA_INICIAL,
                Volta::Vazia if !self.sinc.rede().com_rede() => {
                    espera = ESPERA_INICIAL;
                    let _ = tokio::time::timeout(SONDA, self.sinc.acordar.notified()).await;
                    self.sondar().await;
                }
                Volta::Vazia => {
                    espera = ESPERA_INICIAL;
                    let _ = tokio::time::timeout(COCHILO, self.sinc.acordar.notified()).await;
                }
                Volta::Esperar => {
                    let _ = tokio::time::timeout(espera, self.sinc.acordar.notified()).await;
                    espera = (espera * 2).min(ESPERA_MAXIMA);
                }
            }
        }
    }

    /// 📶 A sonda: a lista de estúdios, que é pequena e passa pelo espelho
    /// (`chamar`) — a resposta que vier marca a rede como de volta.
    async fn sondar(&self) {
        let Some(sessao) = self.sessao.lock().ok().and_then(|s| s.clone()) else {
            return;
        };
        let (canal, resposta) = std::sync::mpsc::channel();
        self.site.estudios(sessao, canal);
        let _ = tokio::task::spawn_blocking(move || resposta.recv_timeout(Duration::from_secs(30)))
            .await;
    }

    async fn uma_volta(&self) -> Volta {
        let proximas = match self.sinc.fila().proximas(EM_VOO as u32).await {
            Ok(p) => p,
            Err(erro) => {
                crate::telemetria::avisar!("⚠️ [Sincronização] a fila não leu: {erro}");
                return Volta::Esperar;
            }
        };
        if proximas.is_empty() {
            return Volta::Vazia;
        }
        let Some(sessao) = self.sessao.lock().ok().and_then(|s| s.clone()) else {
            // Ninguém entrou ainda nesta abertura do app: a conta vem com o
            // primeiro pedido da tela.
            return Volta::Esperar;
        };
        // 📤 **As fotos sobem três por vez, como na esteira.** Só as que estão
        // no começo da fila e uma atrás da outra: o gesto que vem depois delas
        // (a nota da foto que acabou de subir) continua esperando a vez.
        let mut lote: Vec<infrastructure::database::GestoGuardado> = Vec::new();
        for guardado in proximas {
            let cabe = match lote.first() {
                None => true,
                Some(primeiro) => {
                    sobe_arquivo(primeiro)
                        && sobe_arquivo(&guardado)
                        && lote.iter().all(|g| g.alvo != guardado.alvo)
                }
            };
            if !cabe {
                break;
            }
            lote.push(guardado);
        }
        let voltas = futures::future::join_all(
            lote.into_iter()
                .map(|guardado| self.levar(guardado, sessao.clone())),
        )
        .await;
        if voltas.iter().any(|v| matches!(v, Volta::Esperar)) {
            Volta::Esperar
        } else {
            Volta::Andou
        }
    }

    /// Leva um gesto ao site e cuida do que voltou.
    async fn levar(
        &self,
        guardado: infrastructure::database::GestoGuardado,
        sessao: Sessao,
    ) -> Volta {
        let fila = self.sinc.fila();
        let Some(gesto) = Gesto::do_guardado(&guardado) else {
            let _ = fila
                .conflito(
                    guardado.id,
                    "gesto gravado por outra versão do app — atualize o VintageLightbox",
                )
                .await;
            self.sinc.esquecer(&guardado.chave);
            return Volta::Andou;
        };

        let (canal, resposta) = std::sync::mpsc::channel();
        executar(
            self.site.as_ref(),
            sessao,
            gesto.clone(),
            &guardado.ocorrido_em,
            canal,
        );
        let recado = tokio::task::spawn_blocking(move || resposta.recv_timeout(TETO_DO_GESTO))
            .await
            .ok()
            .and_then(Result::ok);
        let Some(recado) = recado else {
            let _ = fila.tentou(guardado.id).await;
            return Volta::Esperar;
        };

        match desfecho(recado, guardado.tentativas) {
            Desfecho::Chegou(recado) => {
                self.sinc.rede().marcar_com_rede();
                self.sinc.conta_recusada(false);
                // 🔁 A sessão criada aqui ganhou outro id no site (um backend
                // anterior ao id do app): os gestos e as fotos seguem o de lá.
                let do_site = match &recado {
                    Recado::Criada(criada) => Some(criada.id.clone()),
                    Recado::Json {
                        resultado: Ok(valor),
                        ..
                    } => valor
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                    _ => None,
                };
                if let (Some(nova), Some(do_site)) = (super::nova_do_gesto(&gesto), do_site) {
                    if let Some(daqui) = nova.id.as_deref().filter(|d| *d != do_site) {
                        let _ = fila.trocar_galeria(daqui, &do_site).await;
                        self.sinc.trocar_galeria_na_memoria(daqui, &do_site);
                        (self.ao_trocar_galeria)(daqui.to_string(), do_site);
                    }
                }
                let _ = fila.concluir(guardado.id).await;
                self.sinc.esquecer(&guardado.chave);
                // ❌ A rejeição da nuvem marcou a foto no site: agora o resgate
                // (o bruto volta para cá, e só então sai da nuvem) — a menos
                // que o operador a tenha desfeito enquanto esperava a rede.
                if let Gesto::RejeitarDaNuvem {
                    foto_no_site,
                    arquivo,
                    parametros,
                    galeria_id,
                } = &gesto
                {
                    if !self.sinc.rejeicao_desfeita(foto_no_site) {
                        self.sinc.ecoar(Recado::ResgateDaRejeitada {
                            foto_no_site: foto_no_site.clone(),
                            arquivo: arquivo.clone(),
                            parametros: parametros.clone(),
                            galeria_id: galeria_id.clone(),
                        });
                    }
                }
                self.sinc.ecoar(recado);
                Volta::Andou
            }
            Desfecho::Esperar(frase) => {
                if e_falta_de_rede(&frase) {
                    self.sinc.rede().marcar_sem_rede();
                } else if frase.contains("recusou o acesso") {
                    // A conta venceu: o selo pede para entrar de novo.
                    self.sinc.conta_recusada(true);
                } else {
                    // 🌪️ Só o soluço do site gasta tentativa. Contando a rede
                    // caída, uma queda de uma hora deixava o gesto sem fôlego, e
                    // o primeiro `5xx` da volta o mandava aos conflitos.
                    let _ = fila.tentou(guardado.id).await;
                }
                Volta::Esperar
            }
            Desfecho::Recusado(frase) => {
                self.sinc.rede().marcar_com_rede();
                crate::telemetria::avisar!(
                    "⚠️ [Sincronização] o site recusou \"{}\": {frase}",
                    gesto.descricao()
                );
                let _ = fila.conflito(guardado.id, &frase).await;
                self.sinc.esquecer(&guardado.chave);
                self.sinc.registrar_conflito(super::Conflito {
                    id: guardado.id,
                    chave: guardado.chave.clone(),
                    gesto,
                    frase,
                    ocorrido_em: guardado.ocorrido_em.clone(),
                });
                Volta::Andou
            }
        }
    }
}

/// O gesto sobe arquivo (a foto classificada, a revelação) — o que vai em lote.
fn sobe_arquivo(guardado: &infrastructure::database::GestoGuardado) -> bool {
    matches!(
        guardado.tipo.as_str(),
        "subir_classificada" | "salvar_revelacao"
    )
}

enum Volta {
    Andou,
    Vazia,
    Esperar,
}
