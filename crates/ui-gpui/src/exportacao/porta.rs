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
}

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
        cancelar: Arc<AtomicBool>,
        canal: Sender<Andamento>,
    );

    /// 📖 O fotolivro: as fotos, na ordem, num PDF só em `destino`. O
    /// andamento conta as fotos (uma `Feita` por foto pronta, com o nome dela)
    /// e termina com [`Andamento::Livro`].
    fn fotolivro(
        &self,
        fotos: Vec<FotoDoLivro>,
        capa: fotolivro::Capa,
        destino: PathBuf,
        sessao: Option<Sessao>,
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
        cancelar: Arc<AtomicBool>,
        canal: Sender<Andamento>,
    ) {
        let catalogo = self.catalogo.clone();
        let site = self.site.clone();
        self.tokio.spawn(exportar_lote(
            saidas, opcoes, sessao, cancelar, canal, catalogo, site,
        ));
    }

    fn fotolivro(
        &self,
        fotos: Vec<FotoDoLivro>,
        capa: fotolivro::Capa,
        destino: PathBuf,
        sessao: Option<Sessao>,
        cancelar: Arc<AtomicBool>,
        canal: Sender<Andamento>,
    ) {
        let catalogo = self.catalogo.clone();
        let site = self.site.clone();
        self.tokio.spawn(montar_fotolivro(
            fotos, capa, destino, sessao, cancelar, canal, catalogo, site,
        ));
    }
}

/// O lado maior das fotos levadas no fotolivro: o mesmo que o livro usa
/// (`fotolivro::gerar` reduz a 1800 px) — revelar maior seria trabalho jogado
/// fora.
const LADO_NO_LIVRO: u32 = 1800;

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

/// O fotolivro inteiro: as fotos com até [`AO_MESMO_TEMPO`] andando juntas, o
/// livro diagramado e o PDF gravado de uma vez (o nome final só aparece com o
/// arquivo inteiro, como na exportação).
#[allow(clippy::too_many_arguments)]
pub async fn montar_fotolivro(
    fotos: Vec<FotoDoLivro>,
    capa: fotolivro::Capa,
    destino: PathBuf,
    sessao: Option<Sessao>,
    cancelar: Arc<AtomicBool>,
    canal: Sender<Andamento>,
    catalogo: Arc<dyn ExportaDoCatalogo>,
    site: Arc<dyn RevelaDoSite>,
) {
    let total = fotos.len();
    let _ = canal.send(Andamento::Comecou { total });
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
            let pdf = tokio::task::spawn_blocking(move || fotolivro::gerar(&capa, &folhas))
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
pub async fn exportar_lote(
    saidas: Vec<Saida>,
    opcoes: ExportOptions,
    sessao: Option<Sessao>,
    cancelar: Arc<AtomicBool>,
    canal: Sender<Andamento>,
    catalogo: Arc<dyn ExportaDoCatalogo>,
    site: Arc<dyn RevelaDoSite>,
) {
    let total = saidas.len();
    let _ = canal.send(Andamento::Comecou { total });

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
        let (catalogo, site, opcoes, sessao) = (
            catalogo.clone(),
            site.clone(),
            opcoes.clone(),
            sessao.clone(),
        );
        tarefas.spawn(async move {
            let _vaga = vaga;
            let resultado = uma(&saida, opcoes, sessao, &*catalogo, &*site).await;
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
            cancelar: Arc<AtomicBool>,
            canal: Sender<Andamento>,
        ) {
            *self.opcoes.lock().expect("as opções") = Some(opcoes);
            self.sessoes.lock().expect("as sessões").push(sessao);
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
            destino: PathBuf,
            sessao: Option<Sessao>,
            _cancelar: Arc<AtomicBool>,
            canal: Sender<Andamento>,
        ) {
            self.sessoes.lock().expect("as sessões").push(sessao);
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
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct SiteDeMentira {
        pedidos: Mutex<Vec<(String, Option<PathBuf>, ExportOptions)>>,
        marcadas: Mutex<Vec<String>>,
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
        let (canal, recebe) = std::sync::mpsc::channel();
        let tokio = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio.block_on(exportar_lote(
            saidas,
            ExportOptions::default(),
            sessao,
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
        struct SiteDeFotos;
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
        let pasta = tempfile::tempdir().unwrap();
        let destino = pasta.path().join("Ensaio.pdf");
        let fotos: Vec<_> = ["a", "b"]
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
            .collect();
        let (canal, recebe) = std::sync::mpsc::channel();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(montar_fotolivro(
                fotos,
                fotolivro::Capa {
                    titulo: "Ensaio".into(),
                    ..Default::default()
                },
                destino.clone(),
                Some(sessao()),
                Arc::new(AtomicBool::new(false)),
                canal,
                Arc::new(CatalogoDeMentira),
                Arc::new(SiteDeFotos),
            ));
        let andamentos: Vec<_> = recebe.try_iter().collect();
        assert_eq!(terminou(&andamentos), (2, 0, 0));
        assert!(andamentos
            .iter()
            .any(|a| matches!(a, Andamento::Livro { destino: d } if *d == destino)));
        let pdf = std::fs::read(&destino).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert!(!pasta.path().join("Ensaio.pdf.part").exists());
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
