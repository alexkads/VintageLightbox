//! A fila sem rede, de ponta a ponta, contra uma porta de mentira que deixa o
//! teste derrubar e devolver a internet.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use domain::services::pos_venda::{
    EstadoNoBalcao, FotoDaGaleria, Galeria, GaleriaAberta, GaleriaDoPainel, MudancaDaFoto,
    MudancaDaGaleria, NovaGaleria, ResumosDoAtendimento, Sessao,
};
use domain::value_objects::CropSettings;
use infrastructure::database::FilaDeSincronizacao;
use infrastructure::gpu_adjustments::Ajustes;
use infrastructure::pos_venda::EstadoDaRede;

use super::porta::PublicadorLocalPrimeiro;
use super::trabalhador::{AoTrocarGaleria, Trabalhador};
use super::Sincronizacao;
use crate::pos_venda::porta::{FotoClassificada, Publicador, Recado};

const SEM_REDE: &str = "sem resposta do site: error sending request";

/// O site de mentira: uma galeria com duas fotos, e a rede que o teste liga e
/// desliga.
#[derive(Default)]
struct SiteDeMentira {
    sem_rede: AtomicBool,
    /// O que chegou ao site, na ordem.
    chegou: Mutex<Vec<String>>,
    /// A recusa de regra da próxima mudança de foto.
    recusar: Mutex<Option<String>>,
    /// Quantas mudanças de foto bateram à porta, a rede caída incluída.
    mudancas_tentadas: AtomicUsize,
    /// A falha da próxima gravação pelo pedido cru (o caixa).
    recusar_json: Mutex<Option<String>>,
    /// As gravações do pedido cru que chegaram **com** a hora do balcão.
    com_hora: Mutex<Vec<String>>,
    /// O id que o site dá à sessão criada — `None` aceita o do app.
    id_do_site: Mutex<Option<String>>,
    /// As miniaturas pedidas — a pré-carga.
    miniaturas: Mutex<Vec<String>>,
    /// Os `GET` do pedido cru, na ordem.
    lidos: Mutex<Vec<String>>,
    /// As cópias de trabalho pedidas — a pré-carga.
    copias: Mutex<Vec<String>>,
    /// A hora de cada `PATCH` de foto que chegou (`None` = gesto de agora).
    horas: Mutex<Vec<Option<String>>>,
}

impl SiteDeMentira {
    fn responder(&self, canal: Sender<Recado>, recado: Recado, registro: String) {
        if self.sem_rede.load(Ordering::SeqCst) {
            let falha = match recado {
                Recado::Negociou { foto_id, .. } => Recado::Negociou {
                    foto_id,
                    erro: Some(SEM_REDE.into()),
                },
                Recado::ClassificadaSubiu { foto_id, .. } => Recado::EnvioFalhou {
                    alvo: foto_id,
                    frase: SEM_REDE.into(),
                },
                Recado::GaleriaAtualizada => Recado::GaleriaNaoAtualizada(SEM_REDE.into()),
                _ => Recado::Falhou(SEM_REDE.into()),
            };
            let _ = canal.send(falha);
            return;
        }
        self.chegou.lock().unwrap().push(registro);
        let _ = canal.send(recado);
    }

    fn aberta(&self) -> GaleriaAberta {
        GaleriaAberta {
            galeria: GaleriaDoPainel {
                id: "g1".into(),
                titulo: "Ana".into(),
                produto_id: "p1".into(),
                criada_em_iso: chrono::Local::now().format("%Y-%m-%d").to_string(),
                ..Default::default()
            },
            fotos: vec![
                FotoDaGaleria {
                    id: "f1".into(),
                    ..Default::default()
                },
                FotoDaGaleria {
                    id: "f2".into(),
                    estado: domain::services::pos_venda::EstadoDaFotoNoSite::LevadaNoBalcao,
                    ..Default::default()
                },
            ],
            vence_venda: None,
            vence_download: None,
            faixas: Vec::new(),
            avisos: Vec::new(),
            resumos: ResumosDoAtendimento::default(),
            conflitos: Vec::new(),
        }
    }
}

impl Publicador for SiteDeMentira {
    fn pedir_json(
        &self,
        _sessao: Sessao,
        pedido: crate::pos_venda::porta::PedidoJson,
        canal: Sender<Recado>,
    ) {
        let rotulo = pedido.rotulo;
        if self.sem_rede.load(Ordering::SeqCst) {
            let _ = canal.send(Recado::Json {
                rotulo,
                resultado: Err(SEM_REDE.into()),
            });
            return;
        }
        if pedido.metodo == "GET" {
            self.lidos.lock().unwrap().push(pedido.caminho.clone());
        } else if let Some(frase) = self.recusar_json.lock().unwrap().take() {
            let _ = canal.send(Recado::Json {
                rotulo,
                resultado: Err(frase),
            });
            return;
        }
        let resposta = match (
            pedido.metodo,
            pedido.caminho.split('?').next().unwrap_or(""),
        ) {
            ("GET", "/pos-venda/caixa") => serde_json::Value::Null,
            ("GET", _) => serde_json::json!([]),
            ("POST", "/pos-venda/galerias") => pedido.corpo.clone().unwrap(),
            _ => serde_json::json!({ "id": "do-site", "numero": 7, "total_centavos": 1 }),
        };
        if pedido.metodo != "GET" {
            let mut corpo = pedido.corpo.clone().unwrap_or_default();
            if let Some(m) = corpo.as_object_mut() {
                if m.remove("ocorrido_em").is_some() {
                    self.com_hora.lock().unwrap().push(pedido.caminho.clone());
                }
            }
            self.chegou
                .lock()
                .unwrap()
                .push(format!("{} {} {}", pedido.metodo, pedido.caminho, corpo));
        }
        let _ = canal.send(Recado::Json {
            rotulo,
            resultado: Ok(resposta),
        });
    }
    fn autorizar(&self, _canal: Sender<Recado>) {}
    fn retomar(&self, _canal: Sender<Recado>) {}
    fn sair(&self, _canal: Sender<Recado>) {}
    fn produtos(&self, _sessao: Sessao, _canal: Sender<Recado>) {}
    fn estudios(&self, _sessao: Sessao, _canal: Sender<Recado>) {}
    fn link(&self, _sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        self.responder(
            canal,
            Recado::Link(domain::services::pos_venda::LinkDeAcesso {
                url: format!("https://site/g/{galeria_id}"),
                validade_em_segundos: None,
            }),
            format!("link {galeria_id}"),
        );
    }
    fn atualizar_galeria(
        &self,
        _sessao: Sessao,
        galeria_id: String,
        _mudanca: MudancaDaGaleria,
        canal: Sender<Recado>,
    ) {
        self.responder(
            canal,
            Recado::GaleriaAtualizada,
            format!("editar {galeria_id}"),
        );
    }
    fn galerias(&self, _sessao: Sessao, canal: Sender<Recado>) {
        let lista = vec![self.aberta().galeria];
        self.responder(canal, Recado::Galerias(lista), "listar".into());
    }
    fn criar_galeria(&self, _sessao: Sessao, nova: NovaGaleria, canal: Sender<Recado>) {
        let id = self
            .id_do_site
            .lock()
            .unwrap()
            .clone()
            .or(nova.id.clone())
            .unwrap();
        self.responder(
            canal,
            Recado::Criada(Galeria {
                id: id.clone(),
                titulo: nova.titulo,
            }),
            format!("criar {id}"),
        );
    }
    fn subir_classificada(
        &self,
        _sessao: Sessao,
        galeria_id: String,
        foto: FotoClassificada,
        canal: Sender<Recado>,
    ) {
        self.responder(
            canal,
            Recado::ClassificadaSubiu {
                foto_id: foto.foto_id.clone(),
                parametros: None,
            },
            format!("subir {} em {galeria_id}", foto.foto_id),
        );
    }
    fn tirar_do_site(&self, _sessao: Sessao, _foto_id: String, _canal: Sender<Recado>) {}
    fn rejeitar_tirando_da_nuvem(&self, _sessao: Sessao, _foto_id: String, _canal: Sender<Recado>) {
    }
    fn remover_remoto(&self, _sessao: Sessao, _no_site: String, _canal: Sender<Recado>) {}
    fn excluir_da_sessao(
        &self,
        _sessao: Sessao,
        _no_site: Option<String>,
        _local: Option<String>,
        _canal: Sender<Recado>,
    ) {
    }
    fn negociar(
        &self,
        _sessao: Sessao,
        foto_id: String,
        mudanca: MudancaDaFoto,
        canal: Sender<Recado>,
    ) {
        self.mudancas_tentadas.fetch_add(1, Ordering::SeqCst);
        if !self.sem_rede.load(Ordering::SeqCst) {
            if let Some(frase) = self.recusar.lock().unwrap().take() {
                let _ = canal.send(Recado::Negociou {
                    foto_id,
                    erro: Some(frase),
                });
                return;
            }
        }
        self.responder(
            canal,
            Recado::Negociou {
                foto_id: foto_id.clone(),
                erro: None,
            },
            {
                // A hora vai à parte: o registro mostra o que mudou.
                let hora = mudanca.ocorrido_em.clone();
                if !self.sem_rede.load(Ordering::SeqCst) {
                    self.horas.lock().unwrap().push(hora);
                }
                let sem_hora = MudancaDaFoto {
                    ocorrido_em: None,
                    ..mudanca
                };
                format!("patch {foto_id} {}", sem_hora.corpo())
            },
        );
    }
    fn enviar_arquivo(
        &self,
        _sessao: Sessao,
        _galeria_id: String,
        _caminho: String,
        _ordem: u32,
        _estado: EstadoNoBalcao,
        _canal: Sender<Recado>,
    ) {
    }
    fn abrir_galeria(&self, _sessao: Sessao, _galeria_id: String, canal: Sender<Recado>) {
        // A leitura, quando o site não responde, vem do espelho — que aqui é
        // a mesma galeria de sempre.
        let _ = canal.send(Recado::Aberta(Box::new(self.aberta())));
    }
    fn miniatura(&self, _sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
        self.miniaturas.lock().unwrap().push(foto_id.clone());
        let _ = canal.send(Recado::Miniatura {
            foto_id,
            bytes: vec![1],
        });
    }
    fn salvar_revelacao(
        &self,
        _sessao: Sessao,
        foto_no_site: String,
        _ajustes: Ajustes,
        _corte: CropSettings,
        canal: Sender<Recado>,
    ) {
        self.responder(
            canal,
            Recado::RevelacaoSalva {
                foto_no_site: foto_no_site.clone(),
            },
            format!("revelar {foto_no_site}"),
        );
    }
    fn avisar(&self, _sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        self.responder(
            canal,
            Recado::Avisado(Vec::new()),
            format!("avisar {galeria_id}"),
        );
    }
    fn copia_de_trabalho(
        &self,
        _sessao: Sessao,
        foto_local: String,
        foto_no_site: String,
        canal: Sender<Recado>,
    ) {
        self.copias.lock().unwrap().push(foto_no_site);
        let _ = canal.send(Recado::Pixels {
            foto_id: foto_local,
            bytes: vec![1],
        });
    }
}

fn sessao() -> Sessao {
    Sessao {
        access_token: "tok".into(),
        refresh_token: "ref".into(),
        access_vence_em: i64::MAX / 2,
        refresh_vence_em: i64::MAX / 2,
    }
}

struct Balcao {
    site: Arc<SiteDeMentira>,
    sinc: Arc<Sincronizacao>,
    porta: PublicadorLocalPrimeiro,
    ecos: Receiver<Recado>,
    trocas: Arc<Mutex<Vec<(String, String)>>>,
    _pasta: tempfile::TempDir,
}

async fn montar() -> Balcao {
    let pasta = tempfile::tempdir().unwrap();
    let url = format!("sqlite:{}?mode=rwc", pasta.path().join("c.db").display());
    let pool = infrastructure::create_pool(&url).await.unwrap();
    infrastructure::run_migrations(&pool).await.unwrap();
    let rede = Arc::new(EstadoDaRede::default());
    let sinc = Sincronizacao::abrir(
        FilaDeSincronizacao::new(pool),
        rede,
        tokio::runtime::Handle::current(),
    )
    .await;
    let site = Arc::new(SiteDeMentira::default());
    let conta = Arc::new(Mutex::new(None));
    let (eco, ecos) = std::sync::mpsc::channel();
    sinc.mandar_ecos_para(eco);
    let trocas = Arc::new(Mutex::new(Vec::new()));
    let ao_trocar: AoTrocarGaleria = {
        let trocas = trocas.clone();
        Arc::new(move |de, para| trocas.lock().unwrap().push((de, para)))
    };
    Trabalhador {
        sinc: sinc.clone(),
        site: site.clone(),
        sessao: conta.clone(),
        ao_trocar_galeria: ao_trocar,
    }
    .ligar();
    let porta = PublicadorLocalPrimeiro::novo(site.clone(), sinc.clone(), conta);
    Balcao {
        site,
        sinc,
        porta,
        ecos,
        trocas,
        _pasta: pasta,
    }
}

/// Espera um recado no canal, sem prender o runtime.
async fn colher(canal: &Receiver<Recado>) -> Recado {
    for _ in 0..400 {
        if let Ok(r) = canal.try_recv() {
            return r;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("nenhum recado chegou");
}

async fn ate(condicao: impl Fn() -> bool, o_que: &str) {
    for _ in 0..600 {
        if condicao() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("não aconteceu: {o_que}");
}

fn nota(n: i16) -> MudancaDaFoto {
    MudancaDaFoto {
        nota: Some(Some(n)),
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn com_rede_e_fila_vazia_o_gesto_vai_direto_e_a_recusa_chega_na_hora() {
    let b = montar().await;
    let (canal, recados) = std::sync::mpsc::channel();
    b.porta
        .negociar(sessao(), "f1".into(), nota(5), canal.clone());
    assert_eq!(
        colher(&recados).await,
        Recado::Negociou {
            foto_id: "f1".into(),
            erro: None
        }
    );
    assert_eq!(b.site.chegou.lock().unwrap().len(), 1);

    *b.site.recusar.lock().unwrap() = Some("o site respondeu 409 Conflict: foto vendida".into());
    b.porta.negociar(sessao(), "f2".into(), nota(3), canal);
    let Recado::Negociou {
        erro: Some(frase), ..
    } = colher(&recados).await
    else {
        panic!("a recusa tinha de chegar");
    };
    assert!(frase.contains("foto vendida"));
    assert_eq!(
        b.sinc.situacao().pendentes,
        0,
        "recusa de regra não vai à fila"
    );
}

/// O dia de hoje: a internet cai, o operador dá notas, leva e cria uma
/// sessão; a tela mostra tudo; a internet volta e o site termina igual.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sem_rede_os_gestos_ficam_na_tela_e_sobem_na_ordem_quando_ela_volta() {
    let b = montar().await;
    b.site.sem_rede.store(true, Ordering::SeqCst);
    let (canal, recados) = std::sync::mpsc::channel();

    // A primeira falha marca a rede como caída e já guarda.
    b.porta
        .negociar(sessao(), "f1".into(), nota(5), canal.clone());
    assert_eq!(
        colher(&recados).await,
        Recado::Negociou {
            foto_id: "f1".into(),
            erro: None
        }
    );
    assert!(!b.sinc.rede().com_rede());
    // Mudou de ideia: a 3 vale.
    b.porta
        .negociar(sessao(), "f1".into(), nota(3), canal.clone());
    b.porta.negociar(
        sessao(),
        "f2".into(),
        MudancaDaFoto {
            estado: Some(EstadoNoBalcao::LevadaNoBalcao),
            ..Default::default()
        },
        canal.clone(),
    );
    let _ = colher(&recados).await;
    let _ = colher(&recados).await;

    // A sessão nova, com o id daqui.
    let id = "5e1d4c2b-0000-4000-8000-0000000000aa".to_string();
    b.porta.criar_galeria(
        sessao(),
        NovaGaleria {
            titulo: "Bruno".into(),
            email: None,
            whatsapp: None,
            produto_id: "p1".into(),
            estudio_id: Some("s1".into()),
            id: Some(id.clone()),
        },
        canal.clone(),
    );
    let Recado::Criada(criada) = colher(&recados).await else {
        panic!("a sessão tinha de nascer");
    };
    assert_eq!(criada.id, id);
    assert_eq!(b.sinc.situacao().pendentes, 4);

    // A tela relê: a grade tem a nota 3 e a levada; a lista tem a sessão nova.
    b.porta.abrir_galeria(sessao(), "g1".into(), canal.clone());
    let Recado::Aberta(aberta) = colher(&recados).await else {
        panic!("a sessão tinha de abrir do espelho");
    };
    assert_eq!(aberta.fotos[0].nota, Some(3));
    assert_eq!(
        aberta.fotos[1].estado,
        domain::services::pos_venda::EstadoDaFotoNoSite::LevadaNoBalcao
    );
    // Sem rede e sem espelho, a lista ainda traz a sessão criada aqui.
    b.porta.galerias(sessao(), canal.clone());
    let Recado::Galerias(lista) = colher(&recados).await else {
        panic!("a lista tinha de trazer a sessão criada sem rede");
    };
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].id, id);
    b.porta.abrir_galeria(sessao(), id.clone(), canal.clone());
    let Recado::Aberta(nova) = colher(&recados).await else {
        panic!("a sessão criada sem rede tinha de abrir vazia");
    };
    assert_eq!(nova.galeria.titulo, "Bruno");
    assert!(nova.fotos.is_empty());
    assert!(b.site.chegou.lock().unwrap().is_empty());

    // A internet volta: o trabalhador leva tudo, na ordem.
    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.acordar.notify_one();
    ate(|| b.sinc.situacao().pendentes == 0, "a fila esvaziar").await;
    assert_eq!(
        *b.site.chegou.lock().unwrap(),
        vec![
            r#"patch f1 {"nota":5}"#.to_string(),
            r#"patch f1 {"nota":3}"#.to_string(),
            r#"patch f2 {"estado":"levada_no_balcao"}"#.to_string(),
            format!("criar {id}"),
        ]
    );
    assert!(b.sinc.rede().com_rede());
    // E a raiz recebe os ecos, como se os gestos fossem de agora.
    let mut ecos = Vec::new();
    while ecos.len() < 4 {
        ecos.push(colher(&b.ecos).await);
    }
    assert!(matches!(ecos[3], Recado::Criada(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn com_fila_esperando_o_gesto_novo_entra_atras_mesmo_com_rede() {
    let b = montar().await;
    b.site.sem_rede.store(true, Ordering::SeqCst);
    let (canal, recados) = std::sync::mpsc::channel();
    b.porta
        .negociar(sessao(), "f1".into(), nota(5), canal.clone());
    let _ = colher(&recados).await;
    // A rede voltou para a tela, mas a fila ainda não andou.
    b.sinc.rede().marcar_com_rede();
    b.porta
        .negociar(sessao(), "f1".into(), nota(2), canal.clone());
    let _ = colher(&recados).await;
    assert!(
        b.site.chegou.lock().unwrap().is_empty(),
        "o novo não furou a vez"
    );
    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.acordar.notify_one();
    ate(|| b.sinc.situacao().pendentes == 0, "a fila esvaziar").await;
    assert_eq!(
        *b.site.chegou.lock().unwrap(),
        vec![
            r#"patch f1 {"nota":5}"#.to_string(),
            r#"patch f1 {"nota":2}"#.to_string()
        ]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_recusa_na_volta_vira_conflito_e_a_fila_segue() {
    let b = montar().await;
    b.site.sem_rede.store(true, Ordering::SeqCst);
    // A rede já caiu para a tela: os dois gestos vão direto à fila, na ordem.
    b.sinc.rede().marcar_sem_rede();
    let (canal, recados) = std::sync::mpsc::channel();
    b.porta
        .negociar(sessao(), "f1".into(), nota(5), canal.clone());
    b.porta
        .negociar(sessao(), "f2".into(), nota(4), canal.clone());
    let _ = colher(&recados).await;
    let _ = colher(&recados).await;

    *b.site.recusar.lock().unwrap() =
        Some("o site respondeu 409 Conflict: a foto já foi comprada".into());
    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.acordar.notify_one();
    ate(|| b.sinc.situacao().pendentes == 0, "a fila esvaziar").await;
    assert_eq!(b.sinc.situacao().conflitos, 1);
    assert_eq!(
        *b.site.chegou.lock().unwrap(),
        vec![r#"patch f2 {"nota":4}"#.to_string()]
    );
    let conflitos = b.sinc.fila().conflitos().await.unwrap();
    assert_eq!(conflitos[0].alvo, "f1");
    assert!(conflitos[0].erro.as_deref().unwrap().contains("comprada"));
    let na_tela = b.sinc.conflitos();
    assert!(na_tela[0].frase.contains("comprada"));

    // O operador manda de novo: o site agora aceita, e a fila esvazia.
    b.sinc.refazer(na_tela[0].id);
    assert_eq!(b.sinc.situacao().conflitos, 0);
    ate(
        || b.sinc.situacao().pendentes == 0 && b.site.chegou.lock().unwrap().len() == 2,
        "o refeito subir",
    )
    .await;
    assert_eq!(b.sinc.fila().contar().await.unwrap(), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_foto_e_a_revelacao_sem_rede_ficam_guardadas_e_sobem_depois() {
    let b = montar().await;
    b.site.sem_rede.store(true, Ordering::SeqCst);
    let (canal, recados) = std::sync::mpsc::channel();
    b.porta.subir_classificada(
        sessao(),
        "g1".into(),
        FotoClassificada {
            foto_id: "local-1".into(),
            ordem: 1,
            estado: None,
            nota: Some(5),
            produto_id: None,
        },
        canal.clone(),
    );
    assert_eq!(
        colher(&recados).await,
        Recado::GuardadoParaDepois {
            alvo: "local-1".into(),
            sem_internet: true,
        }
    );
    b.porta.salvar_revelacao(
        sessao(),
        "f1".into(),
        Ajustes::default(),
        CropSettings::default(),
        canal.clone(),
    );
    assert_eq!(
        colher(&recados).await,
        Recado::GuardadoParaDepois {
            alvo: "f1".into(),
            sem_internet: true,
        }
    );
    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.acordar.notify_one();
    ate(|| b.sinc.situacao().pendentes == 0, "a fila esvaziar").await;
    assert_eq!(
        *b.site.chegou.lock().unwrap(),
        vec!["subir local-1 em g1".to_string(), "revelar f1".to_string()]
    );
    assert!(matches!(
        colher(&b.ecos).await,
        Recado::ClassificadaSubiu { .. }
    ));
    assert!(matches!(
        colher(&b.ecos).await,
        Recado::RevelacaoSalva { .. }
    ));
}

/// Um backend anterior ao id do app escolhe outro: os gestos atrás da criação
/// e as fotos do catálogo seguem o id de lá.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sessao_que_ganhou_outro_id_leva_a_fila_e_as_fotos() {
    let b = montar().await;
    b.site.sem_rede.store(true, Ordering::SeqCst);
    let (canal, recados) = std::sync::mpsc::channel();
    let daqui = "0b3a9f00-0000-4000-8000-000000000001".to_string();
    b.porta.criar_galeria(
        sessao(),
        NovaGaleria {
            titulo: "Carla".into(),
            email: None,
            whatsapp: None,
            produto_id: "p1".into(),
            estudio_id: None,
            id: Some(daqui.clone()),
        },
        canal.clone(),
    );
    let _ = colher(&recados).await;
    b.porta.subir_classificada(
        sessao(),
        daqui.clone(),
        FotoClassificada {
            foto_id: "local-9".into(),
            ordem: 1,
            estado: None,
            nota: Some(4),
            produto_id: None,
        },
        canal.clone(),
    );
    let _ = colher(&recados).await;

    *b.site.id_do_site.lock().unwrap() = Some("site-77".into());
    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.acordar.notify_one();
    ate(|| b.sinc.situacao().pendentes == 0, "a fila esvaziar").await;
    assert_eq!(
        *b.site.chegou.lock().unwrap(),
        vec![
            "criar site-77".to_string(),
            "subir local-9 em site-77".to_string()
        ]
    );
    assert_eq!(
        *b.trocas.lock().unwrap(),
        vec![(daqui, "site-77".to_string())]
    );
}

/// 💰 O caixa sem rede: abrir, vender e a faixa em lote ficam guardados; a
/// leitura do caixa já os mostra; na volta sobem na ordem, com a chave.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn o_caixa_abre_vende_e_muda_a_faixa_sem_rede() {
    use crate::pos_venda::porta::PedidoJson;
    use serde_json::json;
    let b = montar().await;
    b.site.sem_rede.store(true, Ordering::SeqCst);
    b.sinc.rede().marcar_sem_rede();
    let (canal, recados) = std::sync::mpsc::channel();

    b.porta.pedir_json(
        sessao(),
        PedidoJson::gravar(
            "abrir",
            "POST",
            "/pos-venda/caixa",
            json!({ "estudio_id": "s1", "fundo_de_troco_centavos": 5000, "chave": "a1", "local": "ESTUDIO" }),
        ),
        canal.clone(),
    );
    let Recado::Json {
        rotulo: "abrir",
        resultado: Ok(aberto),
    } = colher(&recados).await
    else {
        panic!("abrir tinha de responder");
    };
    assert_eq!(aberto["guardada_sem_rede"], true);

    b.porta.pedir_json(
        sessao(),
        PedidoJson::gravar(
            "venda",
            "POST",
            "/pos-venda/caixa/vendas",
            json!({
                "galeria_id": "g1",
                "itens": [{ "foto_id": "f1", "arquivo": "a.jpg", "cobrado_centavos": 2500 }],
                "pagamentos": [{ "forma": "pix", "valor_centavos": 2500 }],
                "chave": "v1",
            }),
        ),
        canal.clone(),
    );
    let Recado::Json {
        resultado: Ok(venda),
        ..
    } = colher(&recados).await
    else {
        panic!("a venda tinha de responder");
    };
    assert_eq!(crate::caixa::dados::ler_venda(venda).unwrap().total, 2500);

    b.porta.pedir_json(
        sessao(),
        PedidoJson::gravar(
            "lote",
            "PATCH",
            "/pos-venda/fotos/f2",
            json!({ "produto_id": "p9" }),
        ),
        canal.clone(),
    );
    let Recado::Json {
        rotulo: "lote",
        resultado: Ok(_),
    } = colher(&recados).await
    else {
        panic!("o lote tinha de responder");
    };

    // O estorno precisa do site e não vai à fila: sem rede, a falha de rede
    // de sempre, que o caixa já sabe dizer.
    b.porta.pedir_json(
        sessao(),
        PedidoJson::gravar(
            "estorno",
            "POST",
            "/pos-venda/caixa/vendas/x/estornos",
            json!({}),
        ),
        canal.clone(),
    );
    let Recado::Json {
        resultado: Err(frase),
        ..
    } = colher(&recados).await
    else {
        panic!("o estorno tinha de ser recusado sem rede");
    };
    assert!(super::e_falta_de_rede(&frase), "{frase}");

    // A leitura do caixa (sem rede, sem espelho) já mostra o aberto e a venda.
    b.porta.pedir_json(
        sessao(),
        PedidoJson::ler("caixa", "/pos-venda/caixa?estudio_id=s1&local=ESTUDIO"),
        canal.clone(),
    );
    let Recado::Json {
        resultado: Ok(caixa),
        ..
    } = colher(&recados).await
    else {
        panic!("o caixa tinha de abrir do que ficou guardado");
    };
    let caixa = crate::caixa::dados::ler_caixa_aberto(caixa)
        .unwrap()
        .unwrap();
    assert_eq!(caixa.fundo_de_troco, 5000);
    assert_eq!(caixa.vendas.len(), 1);

    // A rede volta: abrir, vender e o PATCH, nessa ordem e com a chave.
    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.acordar.notify_one();
    ate(|| b.sinc.situacao().pendentes == 0, "a fila esvaziar").await;
    let chegou = b.site.chegou.lock().unwrap().clone();
    assert_eq!(chegou.len(), 3, "{chegou:?}");
    assert!(chegou[0].starts_with("POST /pos-venda/caixa {"));
    assert!(chegou[0].contains("\"chave\":\"a1\""));
    assert!(chegou[1].starts_with("POST /pos-venda/caixa/vendas"));
    assert!(chegou[1].contains("\"chave\":\"v1\""));
    assert_eq!(chegou[2], r#"patch f2 {"produto_id":"p9"}"#);
}

/// A sessão do assistente de nova sessão, criada sem rede: o id nasce aqui, a
/// lista e a abertura a mostram, e ela sobe com o atendimento junto.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sessao_do_assistente_nasce_sem_rede_com_o_id_daqui() {
    use crate::pos_venda::porta::PedidoJson;
    use serde_json::json;
    let b = montar().await;
    b.site.sem_rede.store(true, Ordering::SeqCst);
    b.sinc.rede().marcar_sem_rede();
    let (canal, recados) = std::sync::mpsc::channel();
    b.porta.pedir_json(
        sessao(),
        PedidoJson::gravar(
            "nova-criada",
            "POST",
            "/pos-venda/galerias",
            json!({ "titulo": "Dani", "produto_id": "p1", "estudio_id": "s1", "voucher_id": "v7" }),
        ),
        canal.clone(),
    );
    let Recado::Json {
        resultado: Ok(criada),
        ..
    } = colher(&recados).await
    else {
        panic!("a sessão tinha de nascer");
    };
    let id = criada["id"].as_str().unwrap().to_string();
    assert!(uuid::Uuid::parse_str(&id).is_ok());

    b.porta.abrir_galeria(sessao(), id.clone(), canal.clone());
    let Recado::Aberta(aberta) = colher(&recados).await else {
        panic!("a sessão tinha de abrir");
    };
    assert_eq!(aberta.galeria.titulo, "Dani");
    b.porta.pedir_json(
        sessao(),
        PedidoJson::ler("galeria", format!("/pos-venda/galerias/{id}")),
        canal.clone(),
    );
    let Recado::Json {
        resultado: Ok(json_da_galeria),
        ..
    } = colher(&recados).await
    else {
        panic!("o caixa tinha de ler a sessão criada aqui");
    };
    assert_eq!(json_da_galeria["galeria"]["titulo"], "Dani");
    b.porta.pedir_json(
        sessao(),
        PedidoJson::ler("galerias", "/pos-venda/galerias"),
        canal.clone(),
    );
    let Recado::Json {
        resultado: Ok(lista),
        ..
    } = colher(&recados).await
    else {
        panic!("a lista do caixa tinha de vir");
    };
    assert_eq!(lista[0]["id"], id.as_str());

    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.acordar.notify_one();
    ate(|| b.sinc.situacao().pendentes == 0, "a fila esvaziar").await;
    let chegou = b.site.chegou.lock().unwrap().clone();
    assert!(chegou[0].contains("\"voucher_id\":\"v7\""), "{chegou:?}");
    assert!(chegou[0].contains(&id));
}

/// 📥 Com rede, a lista de sessões leva as de hoje ao espelho — a galeria e as
/// miniaturas —, uma vez por abertura do app.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_lista_com_rede_pre_carrega_as_sessoes_de_hoje() {
    let b = montar().await;
    let (canal, recados) = std::sync::mpsc::channel();
    b.porta.galerias(sessao(), canal.clone());
    let Recado::Galerias(lista) = colher(&recados).await else {
        panic!("a lista tinha de vir");
    };
    assert_eq!(lista.len(), 1);
    ate(
        || b.site.miniaturas.lock().unwrap().len() == 2,
        "as duas miniaturas",
    )
    .await;
    assert!(
        b.site
            .lidos
            .lock()
            .unwrap()
            .contains(&"/pos-venda/galerias/g1/bot".to_string()),
        "o QR do bot vai ao espelho junto"
    );
    ate(
        || *b.site.copias.lock().unwrap() == vec!["f2".to_string()],
        "a cópia de trabalho da levada",
    )
    .await;
    // A segunda lista não pré-carrega de novo.
    b.porta.galerias(sessao(), canal);
    let _ = colher(&recados).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(b.site.miniaturas.lock().unwrap().len(), 2);
}

/// 📴 O `PATCH` que sobe pela fila leva a hora em que o gesto aconteceu — o
/// site deixa como está o campo que alguém mudou depois (vale o último gesto a
/// acontecer). O gesto de agora vai sem hora.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn o_patch_da_fila_leva_a_hora_do_gesto() {
    let b = montar().await;
    let (canal, recados) = std::sync::mpsc::channel();
    b.porta
        .negociar(sessao(), "f1".into(), nota(5), canal.clone());
    let _ = colher(&recados).await;
    b.site.sem_rede.store(true, Ordering::SeqCst);
    b.sinc.rede().marcar_sem_rede();
    b.porta
        .negociar(sessao(), "f2".into(), nota(3), canal.clone());
    let _ = colher(&recados).await;
    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.acordar.notify_one();
    ate(|| b.sinc.situacao().pendentes == 0, "a fila esvaziar").await;
    let horas = b.site.horas.lock().unwrap().clone();
    assert_eq!(horas.len(), 2);
    assert_eq!(horas[0], None, "o gesto de agora vai sem hora");
    let hora = horas[1].clone().expect("o gesto da fila leva a hora");
    assert!(
        chrono::DateTime::parse_from_rfc3339(&hora).is_ok(),
        "{hora}"
    );
}

/// ❌ O `X` numa foto da nuvem, sem rede: a grade já a mostra rejeitada; na
/// volta a fila a marca no site e pede à raiz o resgate. Se o operador
/// desfez o `X` enquanto esperava, a foto fica onde está.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rejeicao_da_nuvem_sem_rede_marca_e_pede_o_resgate_na_volta() {
    use serde_json::json;
    let b = montar().await;
    b.site.sem_rede.store(true, Ordering::SeqCst);
    b.sinc.rede().marcar_sem_rede();
    let (canal, recados) = std::sync::mpsc::channel();
    // A raiz guarda as duas rejeições (`app/resgate.rs`).
    for foto in ["f1", "f2"] {
        b.sinc.guardar(super::Gesto::RejeitarDaNuvem {
            foto_no_site: foto.into(),
            arquivo: format!("{foto}.jpg"),
            parametros: json!({ "exposure": 0.3 }),
            galeria_id: Some("g1".into()),
        });
    }
    // A f2 o operador desfez antes de a rede voltar.
    b.porta.negociar(
        sessao(),
        "f2".into(),
        MudancaDaFoto {
            rejeitada: Some(false),
            ..Default::default()
        },
        canal.clone(),
    );
    let _ = colher(&recados).await;

    b.porta.abrir_galeria(sessao(), "g1".into(), canal.clone());
    let Recado::Aberta(aberta) = colher(&recados).await else {
        panic!("a sessão tinha de abrir");
    };
    assert!(
        aberta.fotos[0].rejeitada,
        "a grade mostra a rejeitada sem rede"
    );
    assert!(!aberta.fotos[1].rejeitada, "e a desfeita, desfeita");

    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.acordar.notify_one();
    ate(|| b.sinc.situacao().pendentes == 0, "a fila esvaziar").await;
    assert_eq!(
        *b.site.chegou.lock().unwrap(),
        vec![
            r#"patch f1 {"rejeitada":true}"#.to_string(),
            r#"patch f2 {"rejeitada":true}"#.to_string(),
            r#"patch f2 {"rejeitada":false}"#.to_string(),
        ]
    );
    let mut resgates = Vec::new();
    for _ in 0..3 {
        if let Recado::ResgateDaRejeitada { foto_no_site, .. } = colher(&b.ecos).await {
            resgates.push(foto_no_site);
        }
    }
    assert_eq!(
        resgates,
        vec!["f1".to_string()],
        "só a que continua rejeitada volta"
    );
}

/// 📤 Depois de uma queda, as fotos guardadas sobem em lote, e o gesto que
/// vem atrás delas (a nota da foto do site) espera a vez.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn as_fotos_guardadas_sobem_em_lote_e_a_nota_espera_a_vez() {
    let b = montar().await;
    b.site.sem_rede.store(true, Ordering::SeqCst);
    b.sinc.rede().marcar_sem_rede();
    let (canal, recados) = std::sync::mpsc::channel();
    for n in 1..=4 {
        b.porta.subir_classificada(
            sessao(),
            "g1".into(),
            FotoClassificada {
                foto_id: format!("local-{n}"),
                ordem: n,
                estado: None,
                nota: Some(5),
                produto_id: None,
            },
            canal.clone(),
        );
        let _ = colher(&recados).await;
    }
    // A reabertura pede de novo a subida da primeira: ela não entra duas vezes.
    b.porta.subir_classificada(
        sessao(),
        "g1".into(),
        FotoClassificada {
            foto_id: "local-1".into(),
            ordem: 1,
            estado: None,
            nota: Some(5),
            produto_id: None,
        },
        canal.clone(),
    );
    let _ = colher(&recados).await;
    assert_eq!(b.sinc.situacao().pendentes, 4);
    b.porta
        .negociar(sessao(), "f1".into(), nota(4), canal.clone());
    let _ = colher(&recados).await;
    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.acordar.notify_one();
    ate(|| b.sinc.situacao().pendentes == 0, "a fila esvaziar").await;
    let chegou = b.site.chegou.lock().unwrap().clone();
    assert_eq!(chegou.len(), 5, "{chegou:?}");
    let mut subidas: Vec<String> = chegou[..4].to_vec();
    subidas.sort();
    assert_eq!(
        subidas,
        (1..=4)
            .map(|n| format!("subir local-{n} em g1"))
            .collect::<Vec<_>>()
    );
    assert_eq!(chegou[4], r#"patch f1 {"nota":4}"#);
}

/// 💰 A venda que vai **agora**, com rede, não leva a hora do balcão — o site
/// tem de poder recusar a foto já vendida antes de o operador cobrar. A que
/// ficou na fila leva: o dinheiro já está na gaveta, e ela entra marcada.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn so_a_venda_guardada_leva_a_hora_do_balcao() {
    use crate::pos_venda::porta::PedidoJson;
    use serde_json::json;
    let b = montar().await;
    let (canal, recados) = std::sync::mpsc::channel();
    let venda = |chave: &str| {
        PedidoJson::gravar(
            "venda",
            "POST",
            "/pos-venda/caixa/vendas",
            json!({ "galeria_id": "g1", "itens": [], "pagamentos": [], "chave": chave }),
        )
    };
    b.porta.pedir_json(sessao(), venda("agora"), canal.clone());
    let _ = colher(&recados).await;
    assert!(
        b.site.com_hora.lock().unwrap().is_empty(),
        "a venda de agora vai sem a hora"
    );

    b.site.sem_rede.store(true, Ordering::SeqCst);
    b.sinc.rede().marcar_sem_rede();
    b.porta
        .pedir_json(sessao(), venda("guardada"), canal.clone());
    let _ = colher(&recados).await;
    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.acordar.notify_one();
    ate(|| b.sinc.situacao().pendentes == 0, "a fila esvaziar").await;
    assert_eq!(
        *b.site.com_hora.lock().unwrap(),
        vec!["/pos-venda/caixa/vendas".to_string()],
        "a venda guardada leva a hora"
    );
}

/// 📴 O porquê vai na resposta guardada, decidido na hora de guardar: a venda
/// que a rede segurou diz "sem internet"; a que o site soluçou, não — mesmo que
/// a fila já a tenha levado quando a tela for ler (a corrida que o e2e da
/// resposta perdida pegou).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_venda_guardada_diz_por_que_ficou() {
    use crate::pos_venda::porta::PedidoJson;
    use serde_json::json;
    let b = montar().await;
    let (canal, recados) = std::sync::mpsc::channel();
    let venda = |chave: &str| {
        PedidoJson::gravar(
            "venda",
            "POST",
            "/pos-venda/caixa/vendas",
            json!({ "galeria_id": "g1", "itens": [], "pagamentos": [], "chave": chave }),
        )
    };
    let motivo = |recado: Recado| match recado {
        Recado::Json {
            resultado: Ok(v), ..
        } => (v["guardada_sem_rede"].clone(), v["sem_internet"].clone()),
        outro => panic!("a venda guardada responde com o JSON: {outro:?}"),
    };

    *b.site.recusar_json.lock().unwrap() =
        Some("o site respondeu 500 Internal Server Error: o banco não respondeu".into());
    b.porta.pedir_json(sessao(), venda("soluco"), canal.clone());
    assert_eq!(motivo(colher(&recados).await), (json!(true), json!(false)));
    ate(|| b.sinc.situacao().pendentes == 0, "a fila levar a venda").await;

    b.site.sem_rede.store(true, Ordering::SeqCst);
    b.porta.pedir_json(sessao(), venda("sem-rede"), canal);
    assert_eq!(motivo(colher(&recados).await), (json!(true), json!(true)));
}

/// 🌪️ O site respondeu, mas falhou (o `5xx` que a API explica): o gesto não
/// se perde — fica na fila —, e o "Sem internet" **não** acende: há rede.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_falha_que_o_site_explica_guarda_o_gesto_sem_acender_o_sem_internet() {
    let b = montar().await;
    *b.site.recusar.lock().unwrap() =
        Some("o site respondeu 500 Internal Server Error: o banco não respondeu".into());
    let (canal, recados) = std::sync::mpsc::channel();
    b.porta
        .negociar(sessao(), "f1".into(), nota(5), canal.clone());
    assert_eq!(
        colher(&recados).await,
        Recado::Negociou {
            foto_id: "f1".into(),
            erro: None
        },
        "a tela segue: o gesto ficou guardado"
    );
    assert!(b.sinc.rede().com_rede(), "o site respondeu: há rede");
    ate(|| b.sinc.situacao().pendentes == 0, "a fila levar o gesto").await;
    assert_eq!(
        *b.site.chegou.lock().unwrap(),
        vec![r#"patch f1 {"nota":5}"#.to_string()]
    );
}

/// 🌪️ O site que soluça ganha tentativas; persistindo, é conflito — o
/// operador precisa saber. A rede caída e a conta vencida esperam sem limite.
#[test]
fn o_soluco_do_site_tem_limite_e_a_rede_caida_nao() {
    use super::trabalhador::{desfecho, Desfecho, SOLUCOS_DO_SITE};
    let falha = |frase: &str| Recado::Falhou(frase.into());
    let explicada = "o site respondeu 500 Internal Server Error: o banco não respondeu";
    assert!(matches!(
        desfecho(falha(explicada), 0),
        Desfecho::Esperar(_)
    ));
    assert!(matches!(
        desfecho(falha(explicada), SOLUCOS_DO_SITE - 1),
        Desfecho::Recusado(_)
    ));
    let pausa = "o site respondeu 429 Too Many Requests: devagar";
    assert!(matches!(desfecho(falha(pausa), 3), Desfecho::Esperar(_)));
    let sem_rede = "sem resposta do site (o caminho até a API devolveu 502 Bad Gateway)";
    assert!(matches!(
        desfecho(falha(sem_rede), 500),
        Desfecho::Esperar(_)
    ));
    let conta = "o site recusou o acesso: confira e-mail e senha";
    assert!(matches!(desfecho(falha(conta), 500), Desfecho::Esperar(_)));
    let regra = "o site respondeu 409 Conflict: a foto já foi comprada";
    assert!(matches!(desfecho(falha(regra), 0), Desfecho::Recusado(_)));
}

/// 🌪️ A queda longa não gasta o fôlego do soluço: a espera pela rede não
/// conta tentativa, e o primeiro `5xx` explicado da volta ainda espera a vez
/// em vez de virar conflito.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_queda_longa_nao_gasta_o_folego_do_soluco() {
    use super::trabalhador::SOLUCOS_DO_SITE;
    let b = montar().await;
    b.site.sem_rede.store(true, Ordering::SeqCst);
    let (canal, recados) = std::sync::mpsc::channel();
    b.porta.negociar(sessao(), "f1".into(), nota(5), canal);
    let _ = colher(&recados).await;
    // A rede caída, mais vezes que o fôlego inteiro do soluço.
    for vez in 0..SOLUCOS_DO_SITE {
        let antes = b.site.mudancas_tentadas.load(Ordering::SeqCst);
        b.sinc.tentar_agora();
        ate(
            || b.site.mudancas_tentadas.load(Ordering::SeqCst) > antes,
            &format!("a tentativa {vez} sem rede"),
        )
        .await;
    }
    let guardado = b.sinc.fila().pendentes().await.unwrap().remove(0);
    assert_eq!(guardado.tentativas, 0, "a rede caída não gasta tentativa");

    // A rede volta com um soluço do site: o gesto espera a vez.
    *b.site.recusar.lock().unwrap() =
        Some("o site respondeu 500 Internal Server Error: o banco não respondeu".into());
    b.site.sem_rede.store(false, Ordering::SeqCst);
    b.sinc.tentar_agora();
    ate(
        || b.site.recusar.lock().unwrap().is_none(),
        "o soluço do site",
    )
    .await;
    b.sinc.tentar_agora();
    ate(
        || !b.site.chegou.lock().unwrap().is_empty() || b.sinc.situacao().conflitos > 0,
        "o gesto chegar (ou virar conflito)",
    )
    .await;
    assert_eq!(
        b.sinc.situacao().conflitos,
        0,
        "o soluço da volta virou conflito"
    );
    assert_eq!(
        *b.site.chegou.lock().unwrap(),
        vec![r#"patch f1 {"nota":5}"#.to_string()]
    );
}

/// 🔗 O link do cliente com a bandeira da rede atrasada (a rede voltou, e
/// nada perguntou ainda): o app tenta, e o link sai. Sem rede de verdade, a
/// frase diz que o link precisa de internet.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn o_link_tenta_mesmo_com_a_bandeira_atrasada() {
    let b = montar().await;
    b.sinc.rede().marcar_sem_rede();
    let (canal, recados) = std::sync::mpsc::channel();
    b.porta.link(sessao(), "g1".into(), canal.clone());
    assert!(matches!(colher(&recados).await, Recado::Link(_)));

    b.site.sem_rede.store(true, Ordering::SeqCst);
    b.porta.link(sessao(), "g1".into(), canal);
    let Recado::Falhou(frase) = colher(&recados).await else {
        panic!("sem rede o link falha com a frase");
    };
    assert!(frase.contains("precisa de internet"), "{frase}");
}
