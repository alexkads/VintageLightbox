//! A ponte entre a tela e quem produz os arquivos.
//!
//! Mesma forma das outras portas da casa (`Gravador`, `Marcador`, `Acervo`, as
//! quatro da importação): o trabalho é `async` do tokio, o GPUI não roda
//! futuros dele, e o `Handle` é capturado no `main` antes de `Application::run`
//! tomar a thread.
//!
//! # 🚨 A foto da sessão não exportava (02/10/2026)
//!
//! Até aqui todo item ia ao `ExportController`, que só conhece o **catálogo
//! desta máquina** — id UUID, arquivo no disco. Numa sessão, a grade é feita das
//! fotos do pós-venda, com id `site:<uuid>` e sem caminho: o dono exportou 16 e
//! recebeu *"16 falharam — Photo ID inválido"*. Exportar só funcionava para a
//! foto solta do catálogo, e não para o que o balcão realmente usa.
//!
//! 🔑 Agora cada [`Saida`] diz de onde a foto vem ([`Origem`]): a do catálogo
//! continua no controller; a do site é revelada a partir do bruto (o daqui,
//! quando a foto subiu deste computador, ou o baixado) pelo **mesmo** motor do
//! "Baixar JPEG" — [`RevelaDoSite`].
//!
//! # 💧 A marca d'água é da foto, e não do lote (09/10/2026)
//!
//! *"A marca d'água fica automática baseado no se a foto foi sinalizada com
//! LEVADA"* (dono). A levada sai revelada e inteira; a que não foi levada sai
//! como a **prévia marcada do site** — a mesma que o cliente vê à venda, com a
//! marca resistente que o servidor grava (C27). Não há logotipo a escolher, e
//! não há caminho em que a não levada saia limpa: a porta nem revela a foto.
//!
//! E o lote pode virar um **fotolivro** em PDF ([`Exportador::fotolivro`]) —
//! o crate `fotolivro`, o mesmo que a galeria do cliente usa.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use adapters::controllers::ExportController;
use domain::services::pos_venda::Sessao;
use domain::value_objects::{CropSettings, ExportOptions};
use infrastructure::gpu_adjustments::Ajustes;

/// Quantas fotos andam ao mesmo tempo.
///
/// 🔑 **Três, e não uma nem trinta.** A foto do site gasta a maior parte do
/// tempo baixando o bruto, e com uma por vez a rede ficava parada enquanto a GPU
/// revelava. A GPU continua uma só (o `Mutex` do `Motor`), então mais que isso
/// só empilharia brutos de 30 MB na memória esperando a vez.
const AO_MESMO_TEMPO: usize = 3;

/// Uma foto do lote: de onde ela vem e o arquivo que vai ser criado.
#[derive(Debug, Clone, PartialEq)]
pub struct Saida {
    pub origem: Origem,
    pub destino: PathBuf,
}

/// De onde sai a foto.
///
/// A variante do site carrega a revelação inteira (centenas de `f32`) e a do
/// catálogo só um id: a diferença de tamanho é real e não importa — um lote é
/// um `Vec` de algumas centenas, montado uma vez por clique.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Origem {
    /// Do catálogo desta máquina: o id UUID que o banco conhece.
    Catalogo { id: String },
    /// Do pós-venda: o id de lá, com a revelação que a tela tem para ela.
    Site {
        foto_no_site: String,
        ajustes: Ajustes,
        corte: CropSettings,
        /// O bruto **neste disco**, quando a foto subiu deste computador —
        /// poupa o download. `None` = baixar do site.
        original_local: Option<PathBuf>,
        /// 🔑 Levada (ou comprada): sai revelada e limpa. Senão, sai a prévia
        /// marcada do site, e a revelação nem acontece.
        levada: bool,
    },
}

/// Uma foto do fotolivro: de onde vem, o nome e para onde o toque leva.
#[derive(Debug, Clone, PartialEq)]
pub struct FotoDoLivro {
    pub origem: Origem,
    pub nome: String,
    pub link: Option<String>,
}

impl FotoDoLivro {
    pub fn levada(&self) -> bool {
        match &self.origem {
            Origem::Site { levada, .. } => *levada,
            // A do catálogo não está à venda: é do estúdio.
            Origem::Catalogo { .. } => true,
        }
    }
}

/// 💬 A entrega pelo WhatsApp: o lote sai para uma pasta temporária e cada
/// arquivo pronto sobe para a conversa — a foto como foto, o livro como
/// documento (dono, 10/out/2026: *"utilize o próprio botão de exportar pra
/// fazer isso com um combo para escolher o contato"*).
///
/// 🔑 **É o arquivo que a exportação gravaria**: a levada limpa, a à venda com
/// a marca d'água. A entrega não decide nada sobre a foto.
#[derive(Debug, Clone, PartialEq)]
pub struct Entrega {
    /// O número da conversa (`contact_id`).
    pub contato: String,
    pub legenda: String,
    /// O arquivo que leva a legenda — um só por lote, o primeiro. Guardado
    /// pelo destino para o "tentar de novo" não a repetir nem a perder.
    pub com_legenda: Option<PathBuf>,
}

/// Um contato do combo "Enviar para": conversa do WhatsApp com a janela de
/// 24 horas aberta.
#[derive(Debug, Clone, PartialEq)]
pub struct Contato {
    pub id: String,
    pub nome: String,
    /// Quando ele escreveu pela última vez — de onde sai o "resta 3h 20min".
    pub ultima_entrada: Option<chrono::DateTime<chrono::Utc>>,
}

/// O lado maior da foto que vai pelo WhatsApp: a Meta recomprime o que
/// recebe, e o teto dela para imagem é 5 MB.
pub const LADO_NO_WHATSAPP: u32 = 2560;

/// As opções da foto que vai pelo WhatsApp: JPEG, reduzida. Formato e
/// qualidade da tela não valem neste destino.
pub fn opcoes_do_whatsapp() -> ExportOptions {
    ExportOptions::default()
        .with_quality(85)
        .with_longest_edge(LADO_NO_WHATSAPP)
}

/// O que a tela precisa saber enquanto o lote corre.
#[derive(Debug, Clone, PartialEq)]
pub enum Andamento {
    Comecou {
        total: usize,
    },
    /// Uma foto saiu. Leva o destino para a tela poder mostrar o último nome.
    Feita {
        destino: PathBuf,
    },
    /// ⚠️ **A falha de uma foto não interrompe o lote.** Exportar 400 e parar na
    /// 7ª porque um arquivo sumiu do disco desperdiça as 393 que sairiam — mas
    /// ela também não pode sumir, senão o rodapé diz 400 e a pasta tem 399.
    Falhou {
        destino: PathBuf,
        erro: String,
    },
    /// O fotolivro foi gravado — é ele que o "Mostrar na pasta" aponta.
    Livro {
        destino: PathBuf,
    },
    Terminou {
        sucesso: usize,
        falhas: usize,
        /// As que nem começaram porque o operador parou o lote.
        canceladas: usize,
    },
}

/// O futuro que devolve os bytes de uma foto pronta.
pub type Pronta = Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + Send>>;

/// Quem revela a foto que só existe no site — com tamanho, marca e formato.
///
/// Uma porta, e não o `PublicadorDaApi` direto, para o teste do lote poder
/// afirmar o que foi pedido sem rede nem GPU.
pub trait RevelaDoSite: Send + Sync + 'static {
    fn revelar(
        &self,
        sessao: Sessao,
        foto_no_site: String,
        ajustes: Ajustes,
        corte: CropSettings,
        original_local: Option<PathBuf>,
        opcoes: ExportOptions,
    ) -> Pronta;

    /// 💧 A prévia com a marca d'água do sistema — a da foto não levada.
    fn previa_marcada(&self, sessao: Sessao, foto_no_site: String) -> Pronta;

    /// 🔗 O link que abre a galeria **sem senha**, assinado pelo site — o
    /// mesmo do "Copiar link". É dele que saem os links do fotolivro.
    fn link_da_galeria(&self, sessao: Sessao, galeria_id: String) -> LinkPronto;

    /// 💬 As conversas do WhatsApp com a janela de 24 horas aberta — o combo
    /// "Enviar para".
    fn contatos_do_whatsapp(&self, _sessao: Sessao) -> ContatosProntos {
        Box::pin(async { Err("esta montagem não fala com o WhatsApp".to_string()) })
    }

    /// 💬 Um arquivo pronto para a conversa (`send-media`). O servidor confere
    /// a janela antes de subir o arquivo para a Meta.
    fn enviar_ao_whatsapp(
        &self,
        _sessao: Sessao,
        _contato: String,
        _legenda: String,
        _nome: String,
        _bytes: Vec<u8>,
    ) -> Entregue {
        Box::pin(async { Err("esta montagem não fala com o WhatsApp".to_string()) })
    }
}

/// O link assinado da galeria, ou a frase de por que ele não saiu.
pub type LinkPronto = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;

/// Os contatos com a janela aberta, ou a frase de por que a lista não veio.
pub type ContatosProntos = Pin<Box<dyn Future<Output = Result<Vec<Contato>, String>> + Send>>;

/// O arquivo entregue na conversa, ou a frase do servidor (a janela que
/// fechou, o arquivo grande demais).
pub type Entregue = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

/// Quem exporta a foto do catálogo para um arquivo.
pub trait ExportaDoCatalogo: Send + Sync + 'static {
    fn exportar(
        &self,
        id: String,
        destino: PathBuf,
        opcoes: ExportOptions,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
}

/// O catálogo pelo `ExportController`, com dono.
pub struct CatalogoDoBanco(pub Arc<ExportController>);

impl ExportaDoCatalogo for CatalogoDoBanco {
    fn exportar(
        &self,
        id: String,
        destino: PathBuf,
        opcoes: ExportOptions,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        let controlador = self.0.clone();
        Box::pin(async move {
            let destino = destino.to_string_lossy().to_string();
            controlador.export_photo(id, destino, &opcoes).await
        })
    }
}

pub trait Exportador: Send + Sync + 'static {
    /// Enfileira o lote e **devolve na hora**. O andamento chega pelo canal.
    ///
    /// 🔑 **As opções valem para o lote inteiro, e não por foto.** É o que
    /// separa "entrega final" de "prévia da galeria": as duas são o mesmo lote
    /// com uma decisão diferente na frente, e deixar a decisão por item abriria
    /// a porta para metade sair marcada e metade não.
    ///
    /// `cancelar` ligado faz o lote parar **depois** das fotos em curso: um
    /// arquivo pela metade nunca fica com o nome final (ver [`gravar_por_cima`]).
    fn exportar(
        &self,
        saidas: Vec<Saida>,
        opcoes: ExportOptions,
        sessao: Option<Sessao>,
        entrega: Option<Entrega>,
        cancelar: Arc<AtomicBool>,
        canal: Sender<Andamento>,
    );

    /// 💬 Quem pode receber pelo WhatsApp agora. Devolve na hora; a lista
    /// chega pelo canal.
    fn contatos(&self, sessao: Sessao, canal: Sender<Result<Vec<Contato>, String>>) {
        let _ = sessao;
        let _ = canal.send(Err("esta montagem não fala com o WhatsApp".to_string()));
    }

    /// 📖 O fotolivro: as fotos, na ordem, num PDF só em `destino`. O
    /// andamento conta as fotos (uma `Feita` por foto pronta, com o nome dela)
    /// e termina com [`Andamento::Livro`].
    ///
    /// 🔗 `galeria` é a sessão no site: com ela, os links do livro saem
    /// assinados (ver [`assinar_os_links`]).
    #[allow(clippy::too_many_arguments)]
    fn fotolivro(
        &self,
        fotos: Vec<FotoDoLivro>,
        capa: fotolivro::Capa,
        galeria: Option<String>,
        destino: PathBuf,
        sessao: Option<Sessao>,
        entrega: Option<Entrega>,
        cancelar: Arc<AtomicBool>,
        canal: Sender<Andamento>,
    );
}

pub struct ExportadorDoBanco {
    catalogo: Arc<dyn ExportaDoCatalogo>,
    site: Arc<dyn RevelaDoSite>,
    tokio: tokio::runtime::Handle,
}

impl ExportadorDoBanco {
    pub fn novo(
        catalogo: Arc<dyn ExportaDoCatalogo>,
        site: Arc<dyn RevelaDoSite>,
        tokio: tokio::runtime::Handle,
    ) -> Self {
        Self {
            catalogo,
            site,
            tokio,
        }
    }
}

impl Exportador for ExportadorDoBanco {
    fn exportar(
        &self,
        saidas: Vec<Saida>,
        opcoes: ExportOptions,
        sessao: Option<Sessao>,
        entrega: Option<Entrega>,
        cancelar: Arc<AtomicBool>,
        canal: Sender<Andamento>,
    ) {
        let catalogo = self.catalogo.clone();
        let site = self.site.clone();
        self.tokio.spawn(exportar_lote(
            saidas, opcoes, sessao, entrega, cancelar, canal, catalogo, site,
        ));
    }

    fn contatos(&self, sessao: Sessao, canal: Sender<Result<Vec<Contato>, String>>) {
        let site = self.site.clone();
        self.tokio.spawn(async move {
            let _ = canal.send(site.contatos_do_whatsapp(sessao).await);
        });
    }

    fn fotolivro(
        &self,
        fotos: Vec<FotoDoLivro>,
        capa: fotolivro::Capa,
        galeria: Option<String>,
        destino: PathBuf,
        sessao: Option<Sessao>,
        entrega: Option<Entrega>,
        cancelar: Arc<AtomicBool>,
        canal: Sender<Andamento>,
    ) {
        let catalogo = self.catalogo.clone();
        let site = self.site.clone();
        self.tokio.spawn(montar_fotolivro(
            fotos, capa, galeria, destino, sessao, entrega, cancelar, canal, catalogo, site,
        ));
    }
}

/// 💬 Sobe um arquivo pronto para a conversa.
///
/// 🔑 **O andamento da foto só fecha depois daqui**: "feita" no destino
/// WhatsApp quer dizer "chegou à conversa", e é por isso que progresso, "Parar"
/// e "Tentar de novo" valem sem nada próprio.
async fn entregar(
    arquivo: &Path,
    entrega: &Entrega,
    sessao: Option<Sessao>,
    site: &dyn RevelaDoSite,
) -> Result<(), String> {
    let sessao =
        sessao.ok_or_else(|| "entre na conta do site para enviar pelo WhatsApp".to_string())?;
    let bytes = tokio::fs::read(arquivo)
        .await
        .map_err(|e| format!("o arquivo não pôde ser lido: {e}"))?;
    let nome = arquivo
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "foto.jpg".into());
    let legenda = if entrega.com_legenda.as_deref() == Some(arquivo) {
        entrega.legenda.clone()
    } else {
        String::new()
    };
    site.enviar_ao_whatsapp(sessao, entrega.contato.clone(), legenda, nome, bytes)
        .await
        .map_err(|erro| {
            crate::chatbot::modelo::explicar(&erro, "O WhatsApp não recebeu o arquivo.")
        })
}

/// O lado maior das fotos levadas no fotolivro: o mesmo que o livro usa
/// ([`fotolivro::LADO_NA_FOLHA`]) — revelar maior seria trabalho jogado fora.
const LADO_NO_LIVRO: u32 = fotolivro::LADO_NA_FOLHA;

/// Os bytes de uma foto do lote, já com a decisão da marca tomada.
async fn bytes_da_foto(
    origem: &Origem,
    opcoes: ExportOptions,
    sessao: Option<Sessao>,
    catalogo: &dyn ExportaDoCatalogo,
    site: &dyn RevelaDoSite,
) -> Result<Vec<u8>, String> {
    match origem {
        Origem::Catalogo { id } => {
            // O catálogo só sabe gravar arquivo: um temporário, lido de volta.
            let pasta = tempfile::tempdir().map_err(|e| format!("sem pasta temporária: {e}"))?;
            let arquivo = pasta
                .path()
                .join(format!("foto.{}", opcoes.formato().extensao()));
            catalogo
                .exportar(id.clone(), arquivo.clone(), opcoes)
                .await?;
            tokio::fs::read(&arquivo)
                .await
                .map_err(|e| format!("o arquivo não pôde ser lido: {e}"))
        }
        Origem::Site {
            foto_no_site,
            ajustes,
            corte,
            original_local,
            levada,
        } => {
            let sessao = sessao.ok_or_else(|| {
                "entre na conta do site para exportar as fotos da sessão".to_string()
            })?;
            if *levada {
                site.revelar(
                    sessao,
                    foto_no_site.clone(),
                    *ajustes,
                    corte.clone(),
                    original_local.clone(),
                    opcoes,
                )
                .await
            } else {
                site.previa_marcada(sessao, foto_no_site.clone()).await
            }
        }
    }
}

/// 🔗 Troca os links do livro pelo **link assinado** da galeria.
///
/// `/meus-ensaios/{id}` cru exige sessão, e o cliente do balcão não tem conta:
/// ele tocava na foto ou em "Baixar todas as minhas fotos" e caía no login
/// (dono, 10/out/2026: *"gerei um PDF manual e o link não abriu"*). O link
/// assinado é o do "Copiar link" — entra sem senha, não expira —, e o gesto vai
/// ao lado do token: `&foto=<id>` aqui, `&acao=…` nos botões do fim, que o
/// livro monta a partir do link da capa ([`fotolivro::com_acao`]). O site leva
/// o gesto até a galeria depois de entrar.
pub fn assinar_os_links(fotos: &mut [FotoDoLivro], capa: &mut fotolivro::Capa, link: &str) {
    let separador = if link.contains('?') { '&' } else { '?' };
    for foto in fotos.iter_mut() {
        if let (Origem::Site { foto_no_site, .. }, Some(_)) = (&foto.origem, &foto.link) {
            foto.link = Some(format!("{link}{separador}foto={foto_no_site}"));
        }
    }
    capa.galeria = Some(link.to_string());
}

/// O fotolivro inteiro: as fotos com até [`AO_MESMO_TEMPO`] andando juntas, o
/// livro diagramado e o PDF gravado de uma vez (o nome final só aparece com o
/// arquivo inteiro, como na exportação).
#[allow(clippy::too_many_arguments)]
pub async fn montar_fotolivro(
    mut fotos: Vec<FotoDoLivro>,
    mut capa: fotolivro::Capa,
    galeria: Option<String>,
    destino: PathBuf,
    sessao: Option<Sessao>,
    entrega: Option<Entrega>,
    cancelar: Arc<AtomicBool>,
    canal: Sender<Andamento>,
    catalogo: Arc<dyn ExportaDoCatalogo>,
    site: Arc<dyn RevelaDoSite>,
) {
    let total = fotos.len();
    let _ = canal.send(Andamento::Comecou { total });
    let sessao_da_entrega = sessao.clone();
    // 🔗 Os links entram sem senha quando o site assina; sessão sem e-mail
    // leva o link que pede o e-mail ao cliente. Sem assinatura (e-mail de
    // conta administradora, site fora do ar) ficam os da rota da galeria, que
    // pede o login: o livro sai do mesmo jeito.
    if let (Some(galeria), Some(sessao)) = (galeria, sessao.clone()) {
        if let Ok(link) = site.link_da_galeria(sessao, galeria).await {
            assinar_os_links(&mut fotos, &mut capa, &link);
        }
    }
    let opcoes = ExportOptions::default()
        .with_quality(92)
        .with_longest_edge(LADO_NO_LIVRO);

    let vagas = Arc::new(tokio::sync::Semaphore::new(AO_MESMO_TEMPO));
    let mut tarefas = tokio::task::JoinSet::new();
    let mut iniciadas = 0usize;
    for (posicao, foto) in fotos.into_iter().enumerate() {
        let Ok(vaga) = vagas.clone().acquire_owned().await else {
            break;
        };
        if cancelar.load(Ordering::SeqCst) {
            break;
        }
        iniciadas += 1;
        let (catalogo, site, opcoes, sessao) = (
            catalogo.clone(),
            site.clone(),
            opcoes.clone(),
            sessao.clone(),
        );
        tarefas.spawn(async move {
            let _vaga = vaga;
            let bytes = bytes_da_foto(&foto.origem, opcoes, sessao, &*catalogo, &*site).await;
            let imagem = bytes.and_then(|b| {
                foto_codec::orientacao::decodificar_de_pe(&b)
                    .map_err(|e| format!("a foto não abriu: {e}"))
            });
            (posicao, foto, imagem)
        });
    }

    let mut prontas = Vec::new();
    let mut falhas = 0usize;
    while let Some(fim) = tarefas.join_next().await {
        match fim {
            Ok((posicao, foto, Ok(imagem))) => {
                let _ = canal.send(Andamento::Feita {
                    destino: PathBuf::from(&foto.nome),
                });
                let levada = foto.levada();
                prontas.push((
                    posicao,
                    fotolivro::FotoDaFolha {
                        imagem,
                        nome: foto.nome,
                        levada,
                        link: foto.link,
                    },
                ));
            }
            Ok((_, foto, Err(erro))) => {
                falhas += 1;
                let _ = canal.send(Andamento::Falhou {
                    destino: PathBuf::from(&foto.nome),
                    erro: frase_do_erro(&erro),
                });
            }
            Err(erro) => {
                falhas += 1;
                let _ = canal.send(Andamento::Falhou {
                    destino: PathBuf::new(),
                    erro: format!("a foto parou no meio: {erro}"),
                });
            }
        }
    }
    // A ordem do livro é a do ensaio, e não a de quem terminou primeiro.
    prontas.sort_by_key(|(posicao, _)| *posicao);
    let sucesso = prontas.len();
    let parado = cancelar.load(Ordering::SeqCst);

    if !prontas.is_empty() && !parado {
        let folhas: Vec<_> = prontas.into_iter().map(|(_, f)| f).collect();
        let gravado = async {
            let pdf = tokio::task::spawn_blocking(move || fotolivro::gerar(&capa, folhas))
                .await
                .map_err(|e| format!("o livro não terminou: {e}"))??;
            if let Some(pasta) = destino.parent() {
                tokio::fs::create_dir_all(pasta)
                    .await
                    .map_err(|e| format!("a pasta de destino não pôde ser criada: {e}"))?;
            }
            let parcial = parcial_de(&destino);
            tokio::fs::write(&parcial, pdf)
                .await
                .map_err(|e| format!("o arquivo não pôde ser gravado: {e}"))?;
            gravar_por_cima(&parcial, &destino).await
        }
        .await;
        match gravado {
            Ok(()) => {
                let _ = canal.send(Andamento::Livro {
                    destino: destino.clone(),
                });
                // 💬 O livro gravado segue para a conversa. Ele fica no disco
                // mesmo se a entrega falhar: o "Ver o fotolivro" o abre.
                if let Some(entrega) = &entrega {
                    if let Err(erro) = entregar(&destino, entrega, sessao_da_entrega, &*site).await
                    {
                        falhas += 1;
                        let _ = canal.send(Andamento::Falhou {
                            destino: destino.clone(),
                            erro,
                        });
                    }
                }
            }
            Err(erro) => {
                falhas += 1;
                let _ = canal.send(Andamento::Falhou {
                    destino: destino.clone(),
                    erro,
                });
            }
        }
    }

    let _ = canal.send(Andamento::Terminou {
        sucesso,
        falhas,
        canceladas: total - iniciadas,
    });
}

/// O lote inteiro, com até [`AO_MESMO_TEMPO`] fotos andando juntas.
///
/// 💬 Com `entrega`, cada arquivo gravado sobe para a conversa e sai do disco:
/// a pasta do lote é temporária, e some no fim.
#[allow(clippy::too_many_arguments)]
pub async fn exportar_lote(
    saidas: Vec<Saida>,
    opcoes: ExportOptions,
    sessao: Option<Sessao>,
    entrega: Option<Entrega>,
    cancelar: Arc<AtomicBool>,
    canal: Sender<Andamento>,
    catalogo: Arc<dyn ExportaDoCatalogo>,
    site: Arc<dyn RevelaDoSite>,
) {
    let total = saidas.len();
    let _ = canal.send(Andamento::Comecou { total });
    let pasta_temporaria = entrega
        .as_ref()
        .and_then(|_| saidas.first())
        .and_then(|s| s.destino.parent().map(Path::to_path_buf));

    let vagas = Arc::new(tokio::sync::Semaphore::new(AO_MESMO_TEMPO));
    let mut tarefas = tokio::task::JoinSet::new();
    let mut iniciadas = 0usize;

    for saida in saidas {
        let Ok(vaga) = vagas.clone().acquire_owned().await else {
            break;
        };
        if cancelar.load(Ordering::SeqCst) {
            break;
        }
        iniciadas += 1;
        let (catalogo, site, opcoes, sessao, entrega) = (
            catalogo.clone(),
            site.clone(),
            opcoes.clone(),
            sessao.clone(),
            entrega.clone(),
        );
        tarefas.spawn(async move {
            let _vaga = vaga;
            let mut resultado = uma(&saida, opcoes, sessao.clone(), &*catalogo, &*site).await;
            if let Some(entrega) = &entrega {
                if resultado.is_ok() {
                    resultado = entregar(&saida.destino, entrega, sessao, &*site).await;
                }
                // Entregue ou não, o arquivo era só o caminho até a conversa.
                let _ = tokio::fs::remove_file(&saida.destino).await;
            }
            (saida.destino, resultado)
        });
    }

    let (mut sucesso, mut falhas) = (0usize, 0usize);
    while let Some(fim) = tarefas.join_next().await {
        match fim {
            Ok((destino, Ok(()))) => {
                sucesso += 1;
                let _ = canal.send(Andamento::Feita { destino });
            }
            Ok((destino, Err(erro))) => {
                falhas += 1;
                let _ = canal.send(Andamento::Falhou {
                    destino,
                    erro: frase_do_erro(&erro),
                });
            }
            // Uma tarefa que entrou em pânico ainda é uma foto que não saiu.
            Err(erro) => {
                falhas += 1;
                let _ = canal.send(Andamento::Falhou {
                    destino: PathBuf::new(),
                    erro: format!("a exportação parou no meio: {erro}"),
                });
            }
        }
    }

    // Vazia, a pasta temporária sai; com algo dentro (não deveria), fica.
    if let Some(pasta) = pasta_temporaria {
        let _ = tokio::fs::remove_dir(&pasta).await;
    }

    let _ = canal.send(Andamento::Terminou {
        sucesso,
        falhas,
        canceladas: total - iniciadas,
    });
}

/// Uma foto, do começo ao arquivo no disco.
async fn uma(
    saida: &Saida,
    opcoes: ExportOptions,
    sessao: Option<Sessao>,
    catalogo: &dyn ExportaDoCatalogo,
    site: &dyn RevelaDoSite,
) -> Result<(), String> {
    // 🚨 A pasta é criada aqui, e não pelo exportador: ele recebe um caminho
    // de arquivo e `File::create` não cria pasta. Sem isto, escolher uma pasta
    // que não existe falharia foto a foto com um erro de sistema que não diz o
    // que fazer.
    if let Some(pasta) = saida.destino.parent() {
        tokio::fs::create_dir_all(pasta)
            .await
            .map_err(|e| format!("a pasta de destino não pôde ser criada: {e}"))?;
    }
    let parcial = parcial_de(&saida.destino);

    match &saida.origem {
        Origem::Catalogo { id } => {
            catalogo
                .exportar(id.clone(), parcial.clone(), opcoes)
                .await?;
        }
        Origem::Site { .. } => {
            let bytes = bytes_da_foto(&saida.origem, opcoes, sessao, catalogo, site).await?;
            tokio::fs::write(&parcial, bytes)
                .await
                .map_err(|e| format!("o arquivo não pôde ser gravado: {e}"))?;
        }
    }
    gravar_por_cima(&parcial, &saida.destino).await
}

/// O arquivo de trabalho ao lado do destino: `DSC_1.jpg` → `DSC_1.jpg.part`.
///
/// 🔑 **O nome final só aparece com o arquivo inteiro.** Parar o lote, faltar
/// luz ou a GPU falhar no meio deixava um `DSC_1.jpg` truncado com cara de
/// pronto — e é o tipo de arquivo que vai para o cliente sem ninguém abrir.
fn parcial_de(destino: &Path) -> PathBuf {
    let mut nome = destino.as_os_str().to_owned();
    nome.push(".part");
    PathBuf::from(nome)
}

async fn gravar_por_cima(parcial: &Path, destino: &Path) -> Result<(), String> {
    match tokio::fs::rename(parcial, destino).await {
        Ok(()) => Ok(()),
        Err(erro) => {
            let _ = tokio::fs::remove_file(parcial).await;
            Err(format!("o arquivo não pôde ser gravado: {erro}"))
        }
    }
}

/// O erro na língua de quem está no balcão.
///
/// ⚠️ **O erro chega das camadas de dentro em texto de programador** — foi
/// assim que o dono leu "Photo ID inválido" dezesseis vezes. Aqui ele vira o
/// que aconteceu e, quando dá, o que fazer.
pub fn frase_do_erro(erro: &str) -> String {
    let minusculo = erro.to_lowercase();
    if minusculo.contains("photo id inválido") || minusculo.contains("foto não encontrada") {
        "a foto não está no catálogo deste computador".into()
    } else if minusculo.contains("adaptador de gpu") {
        "sem placa de vídeo disponível: o arquivo não pode sair diferente da tela".into()
    } else if minusculo.contains("marca d'água") {
        "a marca d'água não abriu — escolha o arquivo de novo".into()
    } else if minusculo.contains("failed to open source image") {
        "o arquivo da foto não abriu (foi movido ou apagado?)".into()
    } else {
        erro.to_string()
    }
}

/// O exportador dos testes: registra o que foi pedido e responde na hora.
#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    /// O pedido, o "Parar" e o canal de um lote que ainda não respondeu.
    pub type LoteSegurado = (Vec<Saida>, Arc<AtomicBool>, Sender<Andamento>);

    #[derive(Default)]
    pub struct ExportadorDeMentira {
        pub pedidos: Mutex<Vec<Vec<Saida>>>,
        /// As opções do último lote — é como o teste confere que a marca d'água
        /// pedida na tela chegou até a porta.
        pub opcoes: Mutex<Option<ExportOptions>>,
        /// A sessão que o último lote levou.
        pub sessoes: Mutex<Vec<Option<Sessao>>>,
        /// Quantas fotos do início do lote devem falhar — para a tela poder ser
        /// testada com falha no meio, que é o caso que ninguém reproduz à mão.
        pub falham: Mutex<usize>,
        /// Segura o lote: nada responde até o teste pedir. É como a tela é
        /// testada **durante** o lote (barra, Parar).
        pub segurar: Mutex<bool>,
        /// O canal e o pedido do lote segurado.
        pub segurado: Mutex<Option<LoteSegurado>>,
        /// Os fotolivros pedidos: as fotos, a capa e o arquivo.
        pub livros: Mutex<Vec<(Vec<FotoDoLivro>, fotolivro::Capa, PathBuf)>>,
        /// A galeria que cada fotolivro levou — é dela que o link é assinado.
        pub galerias: Mutex<Vec<Option<String>>>,
        /// 💬 A entrega de cada lote e de cada fotolivro, na ordem dos pedidos.
        pub entregas: Mutex<Vec<Option<Entrega>>>,
        /// 💬 O que o combo "Enviar para" recebe.
        pub contatos: Mutex<Vec<Contato>>,
        /// 💬 A lista de contatos falha com esta frase.
        pub contatos_falham: Mutex<Option<String>>,
    }

    impl ExportadorDeMentira {
        pub fn pedidos(&self) -> Vec<Vec<Saida>> {
            self.pedidos.lock().expect("os pedidos").clone()
        }

        /// Solta o lote segurado: as que não foram canceladas saem.
        pub fn soltar(&self) {
            let Some((saidas, cancelar, canal)) = self.segurado.lock().expect("o lote").take()
            else {
                return;
            };
            self.responder(saidas, &cancelar, &canal);
        }

        fn responder(&self, saidas: Vec<Saida>, cancelar: &AtomicBool, canal: &Sender<Andamento>) {
            let quantas_falham = *self.falham.lock().expect("as falhas");
            let total = saidas.len();
            let (mut sucesso, mut falhas, mut feitas) = (0usize, 0usize, 0usize);
            for (i, saida) in saidas.into_iter().enumerate() {
                if cancelar.load(Ordering::SeqCst) {
                    break;
                }
                feitas += 1;
                if i < quantas_falham {
                    falhas += 1;
                    let _ = canal.send(Andamento::Falhou {
                        destino: saida.destino,
                        erro: "falha de mentira".into(),
                    });
                } else {
                    sucesso += 1;
                    let _ = canal.send(Andamento::Feita {
                        destino: saida.destino,
                    });
                }
            }
            let _ = canal.send(Andamento::Terminou {
                sucesso,
                falhas,
                canceladas: total - feitas,
            });
        }
    }

    impl Exportador for ExportadorDeMentira {
        fn exportar(
            &self,
            saidas: Vec<Saida>,
            opcoes: ExportOptions,
            sessao: Option<Sessao>,
            entrega: Option<Entrega>,
            cancelar: Arc<AtomicBool>,
            canal: Sender<Andamento>,
        ) {
            *self.opcoes.lock().expect("as opções") = Some(opcoes);
            self.sessoes.lock().expect("as sessões").push(sessao);
            self.entregas.lock().expect("as entregas").push(entrega);
            self.pedidos
                .lock()
                .expect("os pedidos")
                .push(saidas.clone());

            let _ = canal.send(Andamento::Comecou {
                total: saidas.len(),
            });
            if *self.segurar.lock().expect("segurar") {
                *self.segurado.lock().expect("o lote") = Some((saidas, cancelar, canal));
                return;
            }
            self.responder(saidas, &cancelar, &canal);
        }

        fn fotolivro(
            &self,
            fotos: Vec<FotoDoLivro>,
            capa: fotolivro::Capa,
            galeria: Option<String>,
            destino: PathBuf,
            sessao: Option<Sessao>,
            entrega: Option<Entrega>,
            _cancelar: Arc<AtomicBool>,
            canal: Sender<Andamento>,
        ) {
            self.sessoes.lock().expect("as sessões").push(sessao);
            self.entregas.lock().expect("as entregas").push(entrega);
            self.galerias.lock().expect("as galerias").push(galeria);
            let total = fotos.len();
            let _ = canal.send(Andamento::Comecou { total });
            for foto in &fotos {
                let _ = canal.send(Andamento::Feita {
                    destino: PathBuf::from(&foto.nome),
                });
            }
            let _ = canal.send(Andamento::Livro {
                destino: destino.clone(),
            });
            let _ = canal.send(Andamento::Terminou {
                sucesso: total,
                falhas: 0,
                canceladas: 0,
            });
            self.livros
                .lock()
                .expect("os livros")
                .push((fotos, capa, destino));
        }

        fn contatos(&self, _sessao: Sessao, canal: Sender<Result<Vec<Contato>, String>>) {
            let _ = canal.send(
                match self.contatos_falham.lock().expect("a falha").clone() {
                    Some(frase) => Err(frase),
                    None => Ok(self.contatos.lock().expect("os contatos").clone()),
                },
            );
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::Mutex;

    /// O que subiu para a conversa: contato, legenda, nome e bytes.
    type Enviado = (String, String, String, Vec<u8>);

    #[derive(Default)]
    struct SiteDeMentira {
        pedidos: Mutex<Vec<(String, Option<PathBuf>, ExportOptions)>>,
        marcadas: Mutex<Vec<String>>,
        /// 💬 O que subiu para a conversa: contato, legenda, nome e bytes.
        enviados: Mutex<Vec<Enviado>>,
        /// 💬 O servidor recusa o envio com este erro (a janela que fechou).
        recusa: Mutex<Option<String>>,
    }

    impl RevelaDoSite for SiteDeMentira {
        fn revelar(
            &self,
            _sessao: Sessao,
            foto_no_site: String,
            _ajustes: Ajustes,
            _corte: CropSettings,
            original_local: Option<PathBuf>,
            opcoes: ExportOptions,
        ) -> Pronta {
            self.pedidos
                .lock()
                .unwrap()
                .push((foto_no_site.clone(), original_local, opcoes));
            Box::pin(async move { Ok(format!("bytes de {foto_no_site}").into_bytes()) })
        }

        fn previa_marcada(&self, _sessao: Sessao, foto_no_site: String) -> Pronta {
            self.marcadas.lock().unwrap().push(foto_no_site.clone());
            Box::pin(async move { Ok(format!("marcada de {foto_no_site}").into_bytes()) })
        }

        fn link_da_galeria(&self, _sessao: Sessao, _galeria_id: String) -> LinkPronto {
            Box::pin(async { Err("sem link no teste".to_string()) })
        }

        fn enviar_ao_whatsapp(
            &self,
            _sessao: Sessao,
            contato: String,
            legenda: String,
            nome: String,
            bytes: Vec<u8>,
        ) -> Entregue {
            let recusa = self.recusa.lock().unwrap().clone();
            if recusa.is_none() {
                self.enviados
                    .lock()
                    .unwrap()
                    .push((contato, legenda, nome, bytes));
            }
            Box::pin(async move { recusa.map_or(Ok(()), Err) })
        }
    }

    struct CatalogoDeMentira;

    impl ExportaDoCatalogo for CatalogoDeMentira {
        fn exportar(
            &self,
            id: String,
            destino: PathBuf,
            _opcoes: ExportOptions,
        ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
            Box::pin(async move {
                if id == "quebrada" {
                    return Err("Photo ID inválido".into());
                }
                tokio::fs::write(destino, b"do catalogo")
                    .await
                    .map_err(|e| e.to_string())
            })
        }
    }

    fn sessao() -> Sessao {
        Sessao {
            access_token: "tok".into(),
            refresh_token: "r".into(),
            access_vence_em: 4_102_444_800,
            refresh_vence_em: 4_102_444_800,
        }
    }

    fn do_site(id: &str, pasta: &Path) -> Saida {
        Saida {
            origem: Origem::Site {
                foto_no_site: id.into(),
                ajustes: Ajustes::default(),
                corte: CropSettings::default(),
                original_local: None,
                levada: true,
            },
            destino: pasta.join(format!("{id}.jpg")),
        }
    }

    fn rodar(
        saidas: Vec<Saida>,
        sessao: Option<Sessao>,
        cancelar: bool,
        site: Arc<SiteDeMentira>,
    ) -> Vec<Andamento> {
        rodar_para(saidas, sessao, None, cancelar, site)
    }

    fn rodar_para(
        saidas: Vec<Saida>,
        sessao: Option<Sessao>,
        entrega: Option<Entrega>,
        cancelar: bool,
        site: Arc<SiteDeMentira>,
    ) -> Vec<Andamento> {
        let (canal, recebe) = std::sync::mpsc::channel();
        let tokio = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio.block_on(exportar_lote(
            saidas,
            ExportOptions::default(),
            sessao,
            entrega,
            Arc::new(AtomicBool::new(cancelar)),
            canal,
            Arc::new(CatalogoDeMentira),
            site,
        ));
        recebe.try_iter().collect()
    }

    fn terminou(andamentos: &[Andamento]) -> (usize, usize, usize) {
        match andamentos.last() {
            Some(Andamento::Terminou {
                sucesso,
                falhas,
                canceladas,
            }) => (*sucesso, *falhas, *canceladas),
            outro => panic!("o lote não terminou: {outro:?}"),
        }
    }

    /// 🚨 **O defeito do print**: a foto da sessão (`site:<id>`) ia ao
    /// controller do catálogo e voltava "Photo ID inválido". Agora ela é
    /// revelada pelo site e o arquivo aparece na pasta.
    #[test]
    fn a_foto_do_site_sai_pelo_site_e_chega_ao_disco() {
        let pasta = tempfile::tempdir().unwrap();
        let site = Arc::new(SiteDeMentira::default());
        let andamentos = rodar(
            vec![do_site("a", pasta.path()), do_site("b", pasta.path())],
            Some(sessao()),
            false,
            site.clone(),
        );

        assert_eq!(terminou(&andamentos), (2, 0, 0));
        assert_eq!(
            std::fs::read(pasta.path().join("a.jpg")).unwrap(),
            b"bytes de a"
        );
        assert_eq!(site.pedidos.lock().unwrap().len(), 2);
        assert!(
            !pasta.path().join("a.jpg.part").exists(),
            "o arquivo de trabalho ficou para trás"
        );
    }

    /// 💧 **A não levada sai marcada, e nem é revelada** (dono, 09/10): a
    /// levada vem do motor, limpa; a outra é a prévia marcada do site.
    #[test]
    fn a_marca_dagua_segue_a_levada() {
        let pasta = tempfile::tempdir().unwrap();
        let site = Arc::new(SiteDeMentira::default());
        let mut a_venda = do_site("b", pasta.path());
        if let Origem::Site { levada, .. } = &mut a_venda.origem {
            *levada = false;
        }
        let andamentos = rodar(
            vec![do_site("a", pasta.path()), a_venda],
            Some(sessao()),
            false,
            site.clone(),
        );
        assert_eq!(terminou(&andamentos), (2, 0, 0));
        assert_eq!(
            std::fs::read(pasta.path().join("a.jpg")).unwrap(),
            b"bytes de a"
        );
        assert_eq!(
            std::fs::read(pasta.path().join("b.jpg")).unwrap(),
            b"marcada de b"
        );
        let reveladas: Vec<_> = site
            .pedidos
            .lock()
            .unwrap()
            .iter()
            .map(|p| p.0.clone())
            .collect();
        assert_eq!(reveladas, ["a"], "a não levada não pode passar pelo motor");
        assert_eq!(*site.marcadas.lock().unwrap(), ["b"]);
    }

    /// 📖 O fotolivro sai num PDF só, com as fotos na ordem do ensaio, e o
    /// último recado aponta o arquivo.
    #[test]
    fn o_fotolivro_vira_um_pdf() {
        /// `Some` = o site assina o link da galeria; `None` = recusa. O
        /// segundo campo guarda o que subiu para a conversa.
        struct SiteDeFotos(Option<&'static str>, Enviados);
        type Enviados = Arc<Mutex<Vec<(String, String, String, bool)>>>;
        impl RevelaDoSite for SiteDeFotos {
            fn revelar(
                &self,
                _: Sessao,
                _: String,
                _: Ajustes,
                _: CropSettings,
                _: Option<PathBuf>,
                _: ExportOptions,
            ) -> Pronta {
                Box::pin(async { Ok(jpeg(40, 30)) })
            }
            fn previa_marcada(&self, _: Sessao, _: String) -> Pronta {
                Box::pin(async { Ok(jpeg(30, 40)) })
            }
            fn link_da_galeria(&self, _: Sessao, galeria_id: String) -> LinkPronto {
                assert_eq!(galeria_id, "g", "o link é o desta galeria");
                let link = self.0.map(str::to_string);
                Box::pin(async move { link.ok_or_else(|| "sem e-mail".to_string()) })
            }
            fn enviar_ao_whatsapp(
                &self,
                _: Sessao,
                contato: String,
                legenda: String,
                nome: String,
                bytes: Vec<u8>,
            ) -> Entregue {
                self.1
                    .lock()
                    .unwrap()
                    .push((contato, legenda, nome, bytes.starts_with(b"%PDF")));
                Box::pin(async { Ok(()) })
            }
        }
        fn jpeg(l: u32, a: u32) -> Vec<u8> {
            let mut bytes = Vec::new();
            image::DynamicImage::ImageRgb8(image::RgbImage::new(l, a))
                .write_to(
                    &mut std::io::Cursor::new(&mut bytes),
                    image::ImageFormat::Jpeg,
                )
                .unwrap();
            bytes
        }
        let fotos = || -> Vec<FotoDoLivro> {
            ["a", "b"]
                .iter()
                .enumerate()
                .map(|(i, id)| FotoDoLivro {
                    origem: Origem::Site {
                        foto_no_site: id.to_string(),
                        ajustes: Ajustes::default(),
                        corte: CropSettings::default(),
                        original_local: None,
                        levada: i == 0,
                    },
                    nome: format!("{id}.jpg"),
                    link: Some(format!("https://site/meus-ensaios/g?foto={id}")),
                })
                .collect()
        };
        let montar = |site: SiteDeFotos, destino: &Path, entrega: Option<Entrega>| {
            let (canal, recebe) = std::sync::mpsc::channel();
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(montar_fotolivro(
                    fotos(),
                    fotolivro::Capa {
                        titulo: "Ensaio".into(),
                        galeria: Some("https://site/meus-ensaios/g".into()),
                        ..Default::default()
                    },
                    Some("g".into()),
                    destino.to_path_buf(),
                    Some(sessao()),
                    entrega,
                    Arc::new(AtomicBool::new(false)),
                    canal,
                    Arc::new(CatalogoDeMentira),
                    Arc::new(site),
                ));
            recebe.try_iter().collect::<Vec<_>>()
        };
        let tem = |pdf: &[u8], trecho: &str| {
            pdf.windows(trecho.len())
                .any(|janela| janela == trecho.as_bytes())
        };

        let pasta = tempfile::tempdir().unwrap();
        let destino = pasta.path().join("Ensaio.pdf");
        let assinado = "https://site/entrar/fast-link?token=abc.def.ghi";
        let andamentos = montar(
            SiteDeFotos(Some(assinado), Enviados::default()),
            &destino,
            None,
        );
        assert_eq!(terminou(&andamentos), (2, 0, 0));
        assert!(andamentos
            .iter()
            .any(|a| matches!(a, Andamento::Livro { destino: d } if *d == destino)));
        let pdf = std::fs::read(&destino).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert!(!pasta.path().join("Ensaio.pdf.part").exists());

        // 🔗 Todo link do livro entra sem senha: a foto, os dois botões do fim
        // e "Abrir minha galeria" — e nenhum sobra na rota que pede o login.
        for gesto in [
            "&foto=a",
            "&foto=b",
            "&acao=baixar-todas",
            "&acao=comprar-todas",
        ] {
            assert!(
                tem(&pdf, &format!("{assinado}{gesto}")),
                "falta {gesto} no link assinado"
            );
        }
        assert!(tem(&pdf, &format!("{assinado})")), "a galeria, sem gesto");
        assert!(!tem(&pdf, "/meus-ensaios/"), "sobrou link que pede o login");

        // Sem assinatura (sessão sem e-mail) o livro sai, com a rota da galeria.
        let sem_email = pasta.path().join("Sem e-mail.pdf");
        let andamentos = montar(SiteDeFotos(None, Enviados::default()), &sem_email, None);
        assert_eq!(terminou(&andamentos), (2, 0, 0));
        let pdf = std::fs::read(&sem_email).unwrap();
        assert!(tem(&pdf, "https://site/meus-ensaios/g?foto=a"));
        assert!(tem(&pdf, "https://site/meus-ensaios/g?acao=baixar-todas"));
        assert!(!tem(&pdf, "fast-link"));

        // 💬 Para o WhatsApp, o livro gravado sobe como um arquivo só, com a
        // legenda, e continua no disco para o "Ver o fotolivro".
        let pelo_whatsapp = pasta.path().join("Pelo WhatsApp.pdf");
        let enviados = Enviados::default();
        let andamentos = montar(
            SiteDeFotos(Some(assinado), enviados.clone()),
            &pelo_whatsapp,
            Some(Entrega {
                contato: "5547999998888".into(),
                legenda: "Seu fotolivro".into(),
                com_legenda: Some(pelo_whatsapp.clone()),
            }),
        );
        assert_eq!(terminou(&andamentos), (2, 0, 0));
        assert_eq!(
            *enviados.lock().unwrap(),
            [(
                "5547999998888".to_string(),
                "Seu fotolivro".to_string(),
                "Pelo WhatsApp.pdf".to_string(),
                true
            )]
        );
        assert!(pelo_whatsapp.exists());
    }

    /// 💬 Para o WhatsApp sobe **o arquivo que a exportação gravaria** — a
    /// levada limpa, a à venda marcada —, a legenda vai num só, e a pasta
    /// temporária some.
    #[test]
    fn o_lote_para_o_whatsapp_sobe_cada_arquivo_e_limpa_a_pasta() {
        let raiz = tempfile::tempdir().unwrap();
        let pasta = raiz.path().join("lote");
        let site = Arc::new(SiteDeMentira::default());
        let mut a_venda = do_site("b", &pasta);
        if let Origem::Site { levada, .. } = &mut a_venda.origem {
            *levada = false;
        }
        let andamentos = rodar_para(
            vec![do_site("a", &pasta), a_venda],
            Some(sessao()),
            Some(Entrega {
                contato: "5547999998888".into(),
                legenda: "Suas fotos".into(),
                com_legenda: Some(pasta.join("a.jpg")),
            }),
            false,
            site.clone(),
        );

        assert_eq!(terminou(&andamentos), (2, 0, 0));
        let mut enviados = site.enviados.lock().unwrap().clone();
        enviados.sort_by(|x, y| x.2.cmp(&y.2));
        assert_eq!(
            enviados,
            [
                (
                    "5547999998888".to_string(),
                    "Suas fotos".to_string(),
                    "a.jpg".to_string(),
                    b"bytes de a".to_vec()
                ),
                (
                    "5547999998888".to_string(),
                    String::new(),
                    "b.jpg".to_string(),
                    b"marcada de b".to_vec()
                ),
            ]
        );
        assert!(!pasta.exists(), "a pasta temporária tinha de sumir");
    }

    /// 💬 A janela que fechou no meio do lote: cada foto vira falha, com a
    /// frase do servidor, e nada fica no disco.
    #[test]
    fn a_recusa_do_whatsapp_vira_falha_com_a_frase_do_servidor() {
        let raiz = tempfile::tempdir().unwrap();
        let pasta = raiz.path().join("lote");
        let site = Arc::new(SiteDeMentira::default());
        *site.recusa.lock().unwrap() =
            Some("o site respondeu 409: A janela de 24 horas fechou.".into());
        let entrega = Entrega {
            contato: "5547999998888".into(),
            legenda: String::new(),
            com_legenda: None,
        };
        let andamentos = rodar_para(
            vec![do_site("a", &pasta)],
            Some(sessao()),
            Some(entrega.clone()),
            false,
            site.clone(),
        );
        assert_eq!(terminou(&andamentos), (0, 1, 0));
        assert!(andamentos.iter().any(|a| matches!(
            a,
            Andamento::Falhou { erro, .. } if erro == "A janela de 24 horas fechou."
        )));
        assert!(!pasta.join("a.jpg").exists());

        // Sem conta, a frase diz o que fazer — e nada é pedido ao site.
        let andamentos = rodar_para(
            vec![Saida {
                origem: Origem::Catalogo { id: "boa".into() },
                destino: pasta.join("boa.jpg"),
            }],
            None,
            Some(entrega),
            false,
            site,
        );
        assert!(andamentos.iter().any(|a| matches!(
            a,
            Andamento::Falhou { erro, .. }
                if erro == "entre na conta do site para enviar pelo WhatsApp"
        )));
    }

    /// A do catálogo continua pelo controller, no mesmo lote.
    #[test]
    fn catalogo_e_site_no_mesmo_lote() {
        let pasta = tempfile::tempdir().unwrap();
        let andamentos = rodar(
            vec![
                Saida {
                    origem: Origem::Catalogo { id: "uuid".into() },
                    destino: pasta.path().join("local.jpg"),
                },
                do_site("a", pasta.path()),
            ],
            Some(sessao()),
            false,
            Arc::new(SiteDeMentira::default()),
        );
        assert_eq!(terminou(&andamentos), (2, 0, 0));
        assert_eq!(
            std::fs::read(pasta.path().join("local.jpg")).unwrap(),
            b"do catalogo"
        );
    }

    /// Sem conta, a foto do site falha com o que fazer — e não com um id.
    #[test]
    fn sem_sessao_a_falha_diz_o_que_fazer() {
        let pasta = tempfile::tempdir().unwrap();
        let andamentos = rodar(
            vec![do_site("a", pasta.path())],
            None,
            false,
            Arc::new(SiteDeMentira::default()),
        );
        assert!(andamentos.iter().any(|a| matches!(a,
            Andamento::Falhou { erro, .. } if erro.contains("entre na conta"))));
        assert_eq!(terminou(&andamentos), (0, 1, 0));
    }

    /// O erro de programador vira frase de balcão.
    #[test]
    fn photo_id_invalido_vira_frase() {
        let pasta = tempfile::tempdir().unwrap();
        let andamentos = rodar(
            vec![Saida {
                origem: Origem::Catalogo {
                    id: "quebrada".into(),
                },
                destino: pasta.path().join("x.jpg"),
            }],
            None,
            false,
            Arc::new(SiteDeMentira::default()),
        );
        let erro = andamentos
            .iter()
            .find_map(|a| match a {
                Andamento::Falhou { erro, .. } => Some(erro.clone()),
                _ => None,
            })
            .unwrap();
        assert!(!erro.contains("Photo ID"), "{erro}");
        assert!(!pasta.path().join("x.jpg").exists());
    }

    /// Parado antes de começar, nada sai e tudo conta como cancelado.
    #[test]
    fn parar_nao_comeca_as_que_faltam() {
        let pasta = tempfile::tempdir().unwrap();
        let site = Arc::new(SiteDeMentira::default());
        let andamentos = rodar(
            vec![do_site("a", pasta.path()), do_site("b", pasta.path())],
            Some(sessao()),
            true,
            site.clone(),
        );
        assert_eq!(terminou(&andamentos), (0, 0, 2));
        assert!(site.pedidos.lock().unwrap().is_empty());
    }
}
