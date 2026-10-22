//! 📴 **O balcão sem internet** — a sincronização que o `CLAUDE.md` prometia
//! desde 6/set/2026 (*"o app vai guardar o que foi feito sem internet e
//! conciliar quando ela voltar"*).
//!
//! # Por que existe (dono, 20/10/2026)
//!
//! > *"Hoje ficamos sem internet no estúdio, com isso descobrimos o que já era
//! > certo, o sistema não trabalha offline. Precisamos preparar o sistema e
//! > deixá-lo offline first."*
//!
//! # As três peças
//!
//! | Peça | Onde | O que faz |
//! |---|---|---|
//! | o espelho | `infrastructure::pos_venda::espelho` | a última leitura de cada rota, devolvida quando o site não responde |
//! | a fila | tabela `fila_de_sincronizacao` + [`Sincronizacao`] | o gesto que não chegou, na ordem, com a hora do balcão |
//! | a porta | [`porta::PublicadorLocalPrimeiro`] | a tela fala com ela como falava com a API; ela decide se vai já ou se guarda |
//!
//! 🔑 **A tela não espera a rede em gesto nenhum do balcão.** A nota, o levar,
//! a negociação, a sessão nova, a foto que sobe: sem rede, o gesto vai para a
//! fila e a tela segue. A leitura (a lista de sessões, a sessão aberta) vem do
//! espelho **com a fila somada por cima** — a nota dada sem internet continua
//! na grade ao reabrir.
//!
//! 🔑 **A ordem é uma só.** Enquanto houver gesto na fila, o gesto novo entra
//! atrás dele, mesmo com a rede de volta: o `PATCH` de agora não pode chegar ao
//! site antes do de uma hora atrás.
//!
//! ⚠️ **O que a fila não faz** — e diz na tela: tirar da nuvem a foto que só
//! existe lá (o resgate baixa o arquivo), gerar o link do cliente (é o site
//! que assina) e o caixa (etapa própria).

pub mod caixa;
pub mod gesto;
pub mod porta;
pub mod trabalhador;

use std::sync::{Arc, Mutex};

use domain::services::pos_venda::{GaleriaAberta, GaleriaDoPainel, ResumosDoAtendimento};
use infrastructure::database::{FilaDeSincronizacao, GestoNovo};
use infrastructure::pos_venda::EstadoDaRede;

pub use gesto::Gesto;

/// Um gesto que a tela já soma por cima do espelho.
#[derive(Debug, Clone)]
struct NaMemoria {
    chave: String,
    gesto: Gesto,
}

/// Um gesto que o site recusou, à espera do operador.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflito {
    pub id: i64,
    pub chave: String,
    pub gesto: Gesto,
    /// A frase do site.
    pub frase: String,
    /// A hora do balcão em que o gesto aconteceu (RFC 3339).
    pub ocorrido_em: String,
}

/// O que a fila tem, para o selo do rodapé.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Situacao {
    /// Gestos esperando a vez.
    pub pendentes: usize,
    /// Gestos que o site recusou e esperam o operador.
    pub conflitos: usize,
    /// Desde quando o site não responde (segundos desde a época).
    pub sem_rede_desde: Option<i64>,
    /// A conta deste computador venceu (mais de 15 dias, ou foi encerrada):
    /// a fila espera alguém entrar de novo.
    pub precisa_entrar: bool,
    /// 💾 O disco recusou guardar um gesto (cheio, quase sempre): ele está só
    /// na memória e se perde se o app fechar. A gravação insiste sozinha.
    pub sem_disco: bool,
}

/// A fila de sincronização deste balcão — uma só, dividida pela porta, pelo
/// trabalhador e pelo selo.
pub struct Sincronizacao {
    fila: FilaDeSincronizacao,
    rede: Arc<EstadoDaRede>,
    tokio: tokio::runtime::Handle,
    /// Os pendentes, na ordem — a cópia que a tela soma por cima do espelho
    /// sem ir ao banco no meio de um quadro.
    memoria: Mutex<Vec<NaMemoria>>,
    conflitos: Mutex<Vec<Conflito>>,
    /// Quem grava na tabela: **uma tarefa só**, para a ordem da tabela ser a
    /// ordem dos gestos.
    escrita: tokio::sync::mpsc::UnboundedSender<GestoNovo>,
    /// Acorda o trabalhador — um gesto novo, ou o operador pedindo "tentar
    /// agora".
    pub(crate) acordar: Arc<tokio::sync::Notify>,
    /// Para onde vão as respostas dos gestos que chegaram — a raiz as colhe
    /// como as de um envio feito agora (a revelação sai do depósito, a grade
    /// relê).
    ecos: Mutex<Option<std::sync::mpsc::Sender<crate::pos_venda::porta::Recado>>>,
    /// A conta venceu e a fila espera — ver [`Situacao::precisa_entrar`].
    conta_recusada: std::sync::atomic::AtomicBool,
    /// 💾 Ver [`Situacao::sem_disco`].
    sem_disco: Arc<std::sync::atomic::AtomicBool>,
}

/// A fila deste app — uma só, montada na abertura (`montagem.rs`).
///
/// ⚠️ **A montagem nova substitui a anterior**, e não o contrário: o e2e do
/// ciclo de vida monta um balcão por cenário no mesmo processo (em série), e
/// cada cenário tem de ver a fila do próprio balcão — a do anterior morreu com
/// o tokio dele.
static DO_APP: Mutex<Option<Arc<Sincronizacao>>> = Mutex::new(None);

/// Registra a fila do app.
pub fn definir_a_do_app(sinc: Arc<Sincronizacao>) {
    if let Ok(mut do_app) = DO_APP.lock() {
        *do_app = Some(sinc);
    }
}

/// A fila do app, para o selo e a lista do rodapé. `None` nos testes que não
/// montam o balcão.
pub fn a_do_app() -> Option<Arc<Sincronizacao>> {
    DO_APP.lock().ok().and_then(|f| f.clone())
}

/// 📴 O balcão está sem internet agora? A frase do gesto guardado só diz "sem
/// internet" quando é isso: o `5xx` que o site explica e a fila que ainda
/// esvazia com a rede de volta também guardam o gesto.
pub fn sem_internet_agora() -> bool {
    a_do_app().is_some_and(|fila| !fila.rede().com_rede())
}

/// O primeiro intervalo entre as tentativas de guardar no disco.
const ESPERA_DO_DISCO: std::time::Duration = std::time::Duration::from_secs(1);
/// O intervalo mais longo — o operador que liberou espaço não espera mais que
/// isso para ver o aviso sumir.
const ESPERA_MAXIMA_DO_DISCO: std::time::Duration = std::time::Duration::from_secs(15);

/// 💾 **Guarda o gesto na tabela, insistindo até conseguir.**
///
/// 🚨 Antes, a falha (o disco cheio) só ia ao relato: o gesto ficava na
/// memória, a tela o dava por guardado, ele **nunca subia** (o trabalhador lê
/// da tabela), prendia na fila os que vinham atrás e sumia ao fechar o app
/// (verificação de 22/10/2026). Agora a tarefa repete o mesmo gesto — os de
/// trás esperam no canal, e a ordem da tabela continua a dos gestos — e
/// `sem_disco` acende o aviso do rodapé até o disco aceitar.
async fn gravar_ate_conseguir(
    fila: &FilaDeSincronizacao,
    gesto: &GestoNovo,
    sem_disco: &std::sync::atomic::AtomicBool,
) {
    use std::sync::atomic::Ordering;
    let mut espera = ESPERA_DO_DISCO;
    loop {
        match fila.enfileirar(gesto).await {
            Ok(_) => {
                sem_disco.store(false, Ordering::Relaxed);
                return;
            }
            Err(erro) => {
                if !sem_disco.swap(true, Ordering::Relaxed) {
                    crate::telemetria::avisar!(
                        "⚠️ [Sincronização] o disco não guardou o gesto {}: {erro}",
                        gesto.tipo
                    );
                }
                tokio::time::sleep(espera).await;
                espera = (espera * 2).min(ESPERA_MAXIMA_DO_DISCO);
            }
        }
    }
}

impl Sincronizacao {
    /// Abre a fila do catálogo: lê o que ficou de antes (o app fechado sem
    /// rede) e liga a tarefa que grava.
    pub async fn abrir(
        fila: FilaDeSincronizacao,
        rede: Arc<EstadoDaRede>,
        tokio: tokio::runtime::Handle,
    ) -> Arc<Self> {
        let pendentes = fila.pendentes().await.unwrap_or_else(|erro| {
            crate::telemetria::avisar!("⚠️ [Sincronização] a fila não abriu: {erro}");
            Vec::new()
        });
        let memoria = pendentes
            .iter()
            .filter_map(|g| {
                Gesto::do_guardado(g).map(|gesto| NaMemoria {
                    chave: g.chave.clone(),
                    gesto,
                })
            })
            .collect();
        let conflitos = fila
            .conflitos()
            .await
            .unwrap_or_default()
            .iter()
            .filter_map(|g| {
                Some(Conflito {
                    id: g.id,
                    chave: g.chave.clone(),
                    gesto: Gesto::do_guardado(g)?,
                    frase: g.erro.clone().unwrap_or_default(),
                    ocorrido_em: g.ocorrido_em.clone(),
                })
            })
            .collect();
        if !pendentes.is_empty() {
            eprintln!(
                "📴 [Sincronização] {} gesto(s) de antes esperam a rede",
                pendentes.len()
            );
        }

        let (escrita, mut a_gravar) = tokio::sync::mpsc::unbounded_channel::<GestoNovo>();
        let acordar = Arc::new(tokio::sync::Notify::new());
        let gravadora = fila.clone();
        let acorda = acordar.clone();
        let sem_disco = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sem_disco_da_tarefa = sem_disco.clone();
        tokio.spawn(async move {
            while let Some(gesto) = a_gravar.recv().await {
                gravar_ate_conseguir(&gravadora, &gesto, &sem_disco_da_tarefa).await;
                acorda.notify_one();
            }
        });

        Arc::new(Self {
            fila,
            rede,
            tokio,
            memoria: Mutex::new(memoria),
            conflitos: Mutex::new(conflitos),
            escrita,
            acordar,
            ecos: Mutex::new(None),
            conta_recusada: Default::default(),
            sem_disco,
        })
    }

    pub(crate) fn conta_recusada(&self, recusada: bool) {
        self.conta_recusada
            .store(recusada, std::sync::atomic::Ordering::Relaxed);
    }

    /// ❌ O operador desfez, sem rede, a rejeição desta foto da nuvem — a
    /// fila espera um `X` de volta (`rejeitada: false`) para ela. Aí o
    /// resgate não roda: a foto fica onde está.
    pub(crate) fn rejeicao_desfeita(&self, foto_no_site: &str) -> bool {
        self.memoria.lock().is_ok_and(|m| {
            m.iter().any(|item| {
                matches!(&item.gesto, Gesto::MudarFoto { foto_id, mudanca }
                    if foto_id == foto_no_site && mudanca.rejeitada == Some(false))
            })
        })
    }

    /// A raiz passa a receber as respostas dos gestos que a fila levou.
    pub fn mandar_ecos_para(
        &self,
        canal: std::sync::mpsc::Sender<crate::pos_venda::porta::Recado>,
    ) {
        if let Ok(mut ecos) = self.ecos.lock() {
            *ecos = Some(canal);
        }
    }

    pub(crate) fn ecoar(&self, recado: crate::pos_venda::porta::Recado) {
        if let Some(canal) = self.ecos.lock().ok().and_then(|e| e.clone()) {
            let _ = canal.send(recado);
        }
    }

    /// "Tentar agora": o trabalhador não espera o fim da pausa.
    pub fn tentar_agora(&self) {
        self.acordar.notify_one();
    }

    pub fn rede(&self) -> &Arc<EstadoDaRede> {
        &self.rede
    }

    pub(crate) fn fila(&self) -> &FilaDeSincronizacao {
        &self.fila
    }

    pub(crate) fn tokio(&self) -> &tokio::runtime::Handle {
        &self.tokio
    }

    /// Guarda o gesto no fim da fila, com a hora de agora. Volta na hora:
    /// quem grava é a tarefa.
    pub fn guardar(&self, gesto: Gesto) {
        self.guardar_feito_em(gesto, chrono::Local::now().to_rfc3339());
    }

    /// 🕐 Guarda o gesto com a hora **em que o operador o fez**, e não a da
    /// falha que o mandou para a fila. Dois gestos seguidos que tentam o site
    /// ao mesmo tempo podem cair na fila em qualquer ordem; com a hora do gesto,
    /// o "vale o último" do site (`marcas_em`) acerta o final mesmo assim — a
    /// nota 5 e depois a 3 terminam em 3 (verificação de 22/10/2026).
    pub fn guardar_feito_em(&self, gesto: Gesto, feito_em: String) {
        // 📤 A mesma foto a subir já espera na fila (a reabertura do app pede
        // de novo a subida do que falta): a segunda não entra. A subida lê a
        // nota e o estado do catálogo na hora de ir, então nada se perde.
        if let Gesto::SubirClassificada { foto, .. } = &gesto {
            let ja = self.memoria.lock().is_ok_and(|m| {
                m.iter().any(|item| {
                    matches!(&item.gesto, Gesto::SubirClassificada { foto: f, .. }
                        if f.foto_id == foto.foto_id)
                })
            });
            if ja {
                return;
            }
        }
        let chave = uuid::Uuid::new_v4().to_string();
        let novo = gesto.para_guardar(&chave, &feito_em);
        if let Ok(mut memoria) = self.memoria.lock() {
            memoria.push(NaMemoria { chave, gesto });
        }
        let _ = self.escrita.send(novo);
    }

    /// Há gesto esperando? Enquanto houver, o novo entra atrás.
    pub fn tem_pendente(&self) -> bool {
        self.memoria.lock().map(|m| !m.is_empty()).unwrap_or(false)
    }

    /// O gesto chegou (ou foi recusado): sai da memória da tela.
    pub(crate) fn esquecer(&self, chave: &str) {
        if let Ok(mut memoria) = self.memoria.lock() {
            memoria.retain(|m| m.chave != chave);
        }
    }

    pub(crate) fn registrar_conflito(&self, conflito: Conflito) {
        if let Ok(mut conflitos) = self.conflitos.lock() {
            conflitos.push(conflito);
        }
    }

    /// Os gestos que o site recusou, na ordem.
    pub fn conflitos(&self) -> Vec<Conflito> {
        self.conflitos.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// "Tentar de novo" o gesto recusado: ele volta para a fila — o operador
    /// corrigiu a causa no site, ou a recusa era de um instante.
    pub fn refazer(&self, id: i64) {
        let Some(conflito) = self.tirar_conflito(id) else {
            return;
        };
        if let Ok(mut memoria) = self.memoria.lock() {
            memoria.push(NaMemoria {
                chave: conflito.chave,
                gesto: conflito.gesto,
            });
        }
        let fila = self.fila.clone();
        let acordar = self.acordar.clone();
        self.tokio.spawn(async move {
            let _ = fila.refazer(id).await;
            acordar.notify_one();
        });
    }

    /// "Descartar" o gesto recusado: o operador desistiu dele.
    pub fn descartar(&self, id: i64) {
        if self.tirar_conflito(id).is_none() {
            return;
        }
        let fila = self.fila.clone();
        self.tokio.spawn(async move {
            let _ = fila.descartar(id).await;
        });
    }

    fn tirar_conflito(&self, id: i64) -> Option<Conflito> {
        let mut conflitos = self.conflitos.lock().ok()?;
        let posicao = conflitos.iter().position(|c| c.id == id)?;
        Some(conflitos.remove(posicao))
    }

    /// 🔁 A sessão criada sem rede ganhou outro id no site: os gestos que
    /// esperam passam a citar o de lá.
    pub(crate) fn trocar_galeria_na_memoria(&self, de: &str, para: &str) {
        let Ok(mut memoria) = self.memoria.lock() else {
            return;
        };
        for item in memoria.iter_mut() {
            match &mut item.gesto {
                Gesto::CriarGaleria(nova) if nova.id.as_deref() == Some(de) => {
                    nova.id = Some(para.to_string())
                }
                Gesto::AtualizarGaleria { galeria_id, .. }
                | Gesto::Avisar { galeria_id }
                | Gesto::SubirClassificada { galeria_id, .. }
                    if galeria_id == de =>
                {
                    *galeria_id = para.to_string()
                }
                Gesto::RejeitarDaNuvem { galeria_id, .. } if galeria_id.as_deref() == Some(de) => {
                    *galeria_id = Some(para.to_string())
                }
                Gesto::PedidoJson { caminho, corpo, .. } => {
                    *caminho = caminho.replace(de, para);
                    if let Ok(trocado) = serde_json::from_str(&corpo.to_string().replace(de, para))
                    {
                        *corpo = trocado;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn situacao(&self) -> Situacao {
        Situacao {
            pendentes: self.memoria.lock().map(|m| m.len()).unwrap_or(0),
            conflitos: self.conflitos.lock().map(|c| c.len()).unwrap_or(0),
            precisa_entrar: self
                .conta_recusada
                .load(std::sync::atomic::Ordering::Relaxed),
            sem_rede_desde: self.rede.sem_rede_desde(),
            sem_disco: self.sem_disco.load(std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// Os gestos pendentes, na ordem — para a tela "Sincronização".
    pub fn pendentes(&self) -> Vec<Gesto> {
        self.memoria
            .lock()
            .map(|m| m.iter().map(|n| n.gesto.clone()).collect())
            .unwrap_or_default()
    }

    /// 📴 A sessão aberta como o site a terá quando a fila chegar lá: as
    /// mudanças de foto e da sessão que ainda esperam, por cima.
    pub fn projetar_aberta(&self, aberta: &mut GaleriaAberta) {
        let Ok(memoria) = self.memoria.lock() else {
            return;
        };
        for item in memoria.iter() {
            match &item.gesto {
                Gesto::MudarFoto { foto_id, mudanca } => aberta.aplicar(foto_id, mudanca),
                Gesto::RejeitarDaNuvem { foto_no_site, .. } => aberta.aplicar(
                    foto_no_site,
                    &domain::services::pos_venda::MudancaDaFoto {
                        rejeitada: Some(true),
                        ..Default::default()
                    },
                ),
                Gesto::AtualizarGaleria {
                    galeria_id,
                    mudanca,
                } if *galeria_id == aberta.galeria.id => aberta.aplicar_na_galeria(mudanca),
                _ => {}
            }
        }
    }

    /// 📴 A lista de sessões com as criadas sem rede no topo e as edições por
    /// cima.
    pub fn projetar_lista(&self, lista: &mut Vec<GaleriaDoPainel>) {
        let Ok(memoria) = self.memoria.lock() else {
            return;
        };
        for item in memoria.iter() {
            if let Some(nova) = nova_do_gesto(&item.gesto) {
                let Some(id) = nova.id.as_deref() else {
                    continue;
                };
                if !lista.iter().any(|g| g.id == id) {
                    lista.insert(0, galeria_da_nova(&nova));
                }
                continue;
            }
            if let Gesto::AtualizarGaleria {
                galeria_id,
                mudanca,
            } = &item.gesto
            {
                if let Some(g) = lista.iter_mut().find(|g| g.id == *galeria_id) {
                    g.aplicar(mudanca);
                }
            }
        }
    }

    /// 📴 A leitura JSON com o que espera a rede por cima — o caixa. `valor`
    /// é o que o site (ou o espelho) mandou; `None` quando nada veio.
    pub fn projetar_json(
        &self,
        caminho: &str,
        valor: Option<serde_json::Value>,
    ) -> Option<serde_json::Value> {
        let Ok(memoria) = self.memoria.lock() else {
            return valor;
        };
        // A lista de sessões pelo pedido cru (o caixa): as criadas aqui entram.
        if caminho.split('?').next() == Some("/pos-venda/galerias") {
            let mut lista = match valor {
                Some(serde_json::Value::Array(lista)) => lista,
                Some(outro) => return Some(outro),
                None => Vec::new(),
            };
            for nova in memoria.iter().filter_map(|m| nova_do_gesto(&m.gesto)) {
                let ja = lista
                    .iter()
                    .any(|g| g.get("id").and_then(serde_json::Value::as_str) == nova.id.as_deref());
                if !ja {
                    lista.insert(0, galeria_em_json(&nova));
                }
            }
            return Some(serde_json::Value::Array(lista));
        }
        let pendentes: Vec<caixa::Pendente<'_>> = memoria
            .iter()
            .filter_map(|m| match &m.gesto {
                Gesto::PedidoJson { caminho, corpo, .. } => {
                    Some((caminho.split('?').next().unwrap_or(caminho), corpo))
                }
                _ => None,
            })
            .collect();
        caixa::projetar(caminho, valor, &pendentes)
    }

    /// 📴 A galeria em JSON (`GET /pos-venda/galerias/{id}` pelo pedido cru,
    /// a do caixa) com as mudanças de foto que esperam a rede.
    pub fn projetar_galeria_json(&self, valor: &mut serde_json::Value) {
        let Ok(memoria) = self.memoria.lock() else {
            return;
        };
        let Some(fotos) = valor
            .get_mut("fotos")
            .and_then(serde_json::Value::as_array_mut)
        else {
            return;
        };
        let rejeitar = domain::services::pos_venda::MudancaDaFoto {
            rejeitada: Some(true),
            ..Default::default()
        };
        for item in memoria.iter() {
            let (foto_id, mudanca) = match &item.gesto {
                Gesto::MudarFoto { foto_id, mudanca } => (foto_id, mudanca),
                Gesto::RejeitarDaNuvem { foto_no_site, .. } => (foto_no_site, &rejeitar),
                _ => continue,
            };
            if let Some(foto) = fotos
                .iter_mut()
                .find(|f| f.get("id").and_then(serde_json::Value::as_str) == Some(foto_id))
            {
                aplicar_na_foto_json(foto, mudanca);
            }
        }
    }

    /// 📴 A sessão criada sem rede, ainda sem linha no site: a tela entra nela
    /// vazia, como entraria numa recém-criada.
    pub fn aberta_criada_aqui(&self, galeria_id: &str) -> Option<GaleriaAberta> {
        let nova = self.nova_criada_aqui(galeria_id)?;
        let mut aberta = GaleriaAberta {
            galeria: galeria_da_nova(&nova),
            fotos: Vec::new(),
            vence_venda: None,
            vence_download: None,
            faixas: Vec::new(),
            avisos: Vec::new(),
            resumos: ResumosDoAtendimento::default(),
            conflitos: Vec::new(),
        };
        self.projetar_aberta(&mut aberta);
        Some(aberta)
    }

    /// A sessão criada sem rede, como foi pedida.
    pub fn nova_criada_aqui(
        &self,
        galeria_id: &str,
    ) -> Option<domain::services::pos_venda::NovaGaleria> {
        let memoria = self.memoria.lock().ok()?;
        memoria
            .iter()
            .filter_map(|m| nova_do_gesto(&m.gesto))
            .find(|nova| nova.id.as_deref() == Some(galeria_id))
    }

    /// A sessão criada sem rede no JSON do site (`GET /pos-venda/galerias/{id}`
    /// pelo pedido cru, o do caixa): vazia, como a recém-criada.
    pub fn galeria_criada_aqui_em_json(&self, galeria_id: &str) -> Option<serde_json::Value> {
        let nova = self.nova_criada_aqui(galeria_id)?;
        Some(serde_json::json!({
            "galeria": galeria_em_json(&nova),
            "fotos": [],
        }))
    }
}

/// Uma mudança de foto sobre a foto em JSON, como o site a gravaria.
fn aplicar_na_foto_json(
    foto: &mut serde_json::Value,
    mudanca: &domain::services::pos_venda::MudancaDaFoto,
) {
    use serde_json::Value;
    let Some(mapa) = foto.as_object_mut() else {
        return;
    };
    if let Some(estado) = mudanca.estado {
        if mapa.get("estado").and_then(Value::as_str) != Some("comprada") {
            mapa.insert("estado".into(), Value::from(estado.como_texto()));
        }
    }
    if let Some(rejeitada) = mudanca.rejeitada {
        mapa.insert(
            "rejeitada_em".into(),
            if rejeitada {
                Value::from(chrono::Utc::now().to_rfc3339())
            } else {
                Value::Null
            },
        );
    }
    // O resto tem o mesmo nome e o mesmo jeito no corpo do PATCH e na foto.
    if let Value::Object(corpo) = mudanca.corpo() {
        for (campo, valor) in corpo {
            if campo != "estado" && campo != "rejeitada" {
                mapa.insert(campo, valor);
            }
        }
    }
}

/// A sessão que o gesto cria, quando ele cria uma: a porta tipada
/// (`criar_galeria`) ou o assistente de nova sessão (`POST /pos-venda/galerias`
/// pelo pedido cru, com o atendimento junto).
pub fn nova_do_gesto(gesto: &Gesto) -> Option<domain::services::pos_venda::NovaGaleria> {
    match gesto {
        Gesto::CriarGaleria(nova) => Some(nova.clone()),
        Gesto::PedidoJson {
            metodo,
            caminho,
            corpo,
        } if metodo.eq_ignore_ascii_case("POST") && caminho == "/pos-venda/galerias" => {
            domain::services::pos_venda::NovaGaleria::do_corpo(corpo)
        }
        _ => None,
    }
}

/// A linha da sessão criada aqui, no JSON do painel do site.
fn galeria_em_json(nova: &domain::services::pos_venda::NovaGaleria) -> serde_json::Value {
    serde_json::json!({
        "id": nova.id,
        "titulo": nova.titulo,
        "email": nova.email,
        "whatsapp": nova.whatsapp,
        "produto_id": nova.produto_id,
        "estudio_id": nova.estudio_id,
        "criada_em": chrono::Utc::now().to_rfc3339(),
        "guardada_sem_rede": true,
    })
}

/// A linha da lista de uma sessão que só existe aqui.
fn galeria_da_nova(nova: &domain::services::pos_venda::NovaGaleria) -> GaleriaDoPainel {
    GaleriaDoPainel {
        id: nova.id.clone().unwrap_or_default(),
        titulo: nova.titulo.clone(),
        email: nova.email.clone(),
        whatsapp: nova.whatsapp.clone(),
        produto_id: nova.produto_id.clone(),
        estudio_id: nova.estudio_id.clone(),
        criada_em_iso: chrono::Local::now().format("%Y-%m-%d").to_string(),
        ..Default::default()
    }
}

/// O nome da pasta do espelho para um endereço da API: só letras, números e
/// `-` (`https://api.recordarfotos.com.br` → `api-recordarfotos-com-br`).
pub fn pasta_do_endereco(base: &str) -> String {
    let sem_esquema = base.split("://").nth(1).unwrap_or(base);
    let nome: String = sem_esquema
        .trim_end_matches('/')
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    if nome.is_empty() {
        "sem-endereco".into()
    } else {
        nome
    }
}

/// A falha que é **falta de rede** — ninguém respondeu, ou só o caminho até a
/// API: vira fila e acende o "Sem internet".
pub fn e_falta_de_rede(frase: &str) -> bool {
    infrastructure::pos_venda::http::erro_de_rede(frase)
}

/// A falha que **passa sozinha**: a rede, ou o `5xx` que a própria API
/// explica (o armazenamento que soluçou). Vira fila — o gesto do operador não
/// se perde por um soluço do servidor —, mas só a primeira acende o "Sem
/// internet": na segunda o site respondeu.
pub fn e_passageira(frase: &str) -> bool {
    infrastructure::pos_venda::http::erro_passageiro(frase)
}

#[cfg(test)]
mod testes;
