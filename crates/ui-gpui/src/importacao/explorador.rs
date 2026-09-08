//! Quem vai ao disco: varrer a origem, ler metadados, conferir duplicatas.
//!
//! A mesma forma das portas da Revelação ([`crate::revelacao::persistencia::Gravador`],
//! [`crate::revelacao::presets::GuardaDePresets`]) e pelas mesmas razões: os
//! controllers são `async` do tokio, o GPUI não roda futuros dele, e os testes de
//! tela precisam responder o que quiserem sem cartão de memória plugado.
//!
//! ## 🔑 A resposta volta por canal, e não por `Future`
//!
//! Uma varredura de cartão pode demorar minutos e a tela não pode ficar presa
//! nela. Cada chamada leva um [`Sender`] e devolve na hora; a tela drena o que
//! chegou a cada quadro, do mesmo jeito que a Revelação colhe o resultado da GPU.
//!
//! É também o que permite **descartar resposta atrasada**: o recado carrega de
//! qual origem ele é, e o estado decide se ainda interessa
//! ([`super::estado::aplicar`]).

use std::sync::mpsc::Sender;
use std::sync::Arc;

use adapters::controllers::ImportController;
use domain::value_objects::ImportOptions;

use super::estado::{Descricao, Origem, Recado};

/// O que a tela pede ao mundo de fora.
pub trait Explorador: Send + Sync + 'static {
    /// Lista os arquivos de uma origem. Responde `Varrido` ou `Falhou`.
    fn varrer(&self, raiz: String, subpastas: bool, canal: Sender<Recado>);

    /// Lista os cartões montados e as origens recentes. Responde `Origens`.
    fn origens(&self, canal: Sender<Recado>);

    /// Lê metadados e confere duplicatas. Responde `Descritos` **e**
    /// `Duplicados` — dois recados, porque as duas leituras terminam em tempos
    /// diferentes e a grade se completa aos poucos.
    fn detalhar(&self, arquivos: Vec<String>, canal: Sender<Recado>);
}

/// O explorador de verdade, falando com o `ImportController`.
pub struct ExploradorDoDisco {
    importacao: Arc<ImportController>,
    tokio: tokio::runtime::Handle,
}

impl ExploradorDoDisco {
    pub fn novo(importacao: Arc<ImportController>, tokio: tokio::runtime::Handle) -> Self {
        Self { importacao, tokio }
    }
}

impl Explorador for ExploradorDoDisco {
    fn origens(&self, canal: Sender<Recado>) {
        let importacao = self.importacao.clone();

        self.tokio.spawn(async move {
            let (dispositivos, historico) = importacao.get_sources().await;
            let converter = |fontes: Vec<domain::import_source::ImportSource>| {
                fontes
                    .into_iter()
                    .map(|fonte| Origem {
                        nome: fonte.name,
                        caminho: fonte.path.to_string_lossy().to_string(),
                    })
                    .collect()
            };

            let _ = canal.send(Recado::Origens {
                cartoes: converter(dispositivos),
                recentes: converter(historico),
            });
        });
    }

    fn varrer(&self, raiz: String, subpastas: bool, canal: Sender<Recado>) {
        let importacao = self.importacao.clone();

        self.tokio.spawn(async move {
            let recado = match importacao.scan_source(raiz.clone(), subpastas).await {
                Ok(arquivos) => Recado::Varrido { raiz, arquivos },
                Err(erro) => Recado::Falhou(erro),
            };
            // O erro de envio é a tela ter fechado no meio da varredura. Não há a
            // quem contar, e não é falha: é o modal que sumiu.
            let _ = canal.send(recado);
        });
    }

    fn detalhar(&self, arquivos: Vec<String>, canal: Sender<Recado>) {
        let importacao = self.importacao.clone();

        self.tokio.spawn(async move {
            // 🔑 **Metadados primeiro, duplicatas depois.** Ler EXIF é abrir o
            // cabeçalho de cada arquivo; conferir duplicata é ler o arquivo
            // **inteiro** para calcular o hash. A grade se completa com o que
            // custa pouco, e o que custa caro chega quando chegar.
            let descricoes = importacao.describe_candidates(arquivos.clone()).await;
            let recado = match descricoes {
                Ok(itens) => Recado::Descritos(
                    itens
                        .into_iter()
                        .map(|item| Descricao {
                            caminho: item.file_path,
                            tamanho: item.file_size,
                            e_raw: item.is_raw,
                            camera: item.camera,
                            data: item.date_time,
                            dimensoes: item.dimensions,
                        })
                        .collect(),
                ),
                Err(erro) => Recado::Falhou(erro),
            };
            if canal.send(recado).is_err() {
                return;
            }

            let recado = match importacao.check_duplicates(arquivos).await {
                Ok(resultados) => Recado::Duplicados(
                    resultados
                        .into_iter()
                        .filter(|r| r.is_duplicate)
                        .map(|r| r.file_path)
                        .collect(),
                ),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }
}

/// Quem abre o seletor de pasta do sistema.
///
/// 🔑 **Porta própria, e não um método do [`Explorador`].** Abrir diálogo é
/// interação com o sistema operacional, não leitura de disco — e os testes de
/// tela precisam escolher pasta sem que nenhuma janela apareça na máquina de quem
/// roda a suíte.
pub trait SeletorDePasta: Send + Sync + 'static {
    /// Responde **sempre**: `OrigemEscolhida` ou `SemEscolha`. Silêncio deixaria
    /// a tela esperando uma pasta que nunca vem.
    fn escolher(&self, canal: Sender<Recado>);

    /// O mesmo, para a pasta de destino. Responde `DestinoEscolhido` ou
    /// `SemEscolha`.
    fn escolher_destino(&self, canal: Sender<Recado>);
}

/// O seletor do sistema, via `rfd`.
///
/// ⚠️ **Janela do sistema, e não do framework.** É a mesma escolha que o legado
/// fez ao trocar o `egui_file` por `rfd`: um seletor desenhado pelo framework
/// disputa camada com o modal e aparece escurecido e sem responder ao clique.
pub struct SeletorNativo {
    tokio: tokio::runtime::Handle,
}

impl SeletorNativo {
    pub fn novo(tokio: tokio::runtime::Handle) -> Self {
        Self { tokio }
    }

    /// Abre o seletor e responde com o recado que a chamada pedir.
    ///
    /// 🔑 Um caminho só para as duas pastas: origem e destino diferem no título e
    /// no recado, e nada mais. Duas cópias divergiriam no primeiro ajuste — e o
    /// jeito de descobrir seria um dos dois parar de responder ao desistir.
    fn pedir(&self, titulo: &'static str, canal: Sender<Recado>, como: fn(String) -> Recado) {
        self.tokio.spawn(async move {
            let recado = match rfd::AsyncFileDialog::new()
                .set_title(titulo)
                .pick_folder()
                .await
            {
                Some(pasta) => como(pasta.path().to_string_lossy().to_string()),
                None => Recado::SemEscolha,
            };
            let _ = canal.send(recado);
        });
    }
}

impl SeletorDePasta for SeletorNativo {
    fn escolher(&self, canal: Sender<Recado>) {
        self.pedir("Escolher a origem", canal, Recado::OrigemEscolhida);
    }

    fn escolher_destino(&self, canal: Sender<Recado>) {
        self.pedir("Escolher o destino", canal, Recado::DestinoEscolhido);
    }
}

/// A chave da miniatura de um candidato no cache de previews.
///
/// 🚨 **O prefixo separa estas entradas das fotos catalogadas.** A mesma tabela
/// guarda as duas, e um caminho de cartão sem prefixo poderia colidir com o id de
/// uma foto — que é um UUID, mas o cache não valida formato. É a mesma decisão do
/// legado (`thumb_key`).
pub fn chave_de_miniatura(caminho: &str) -> String {
    format!("import::{caminho}")
}

/// Quem gera as miniaturas dos arquivos que ainda não estão no catálogo.
///
/// ⚠️ **Elas não existem em lugar nenhum ainda.** As fotos da grade estão no
/// cartão, fora do catálogo: não há preview gravado, e cada miniatura custa abrir
/// o arquivo e redimensionar. Por isso só se gera o que a grade está mostrando.
pub trait GeradorDeMiniaturas: Send + Sync + 'static {
    fn gerar(&self, caminhos: Vec<String>, canal: Sender<Recado>);
}

pub struct GeradorDoDisco {
    miniaturas: Arc<dyn domain::services::ThumbnailGenerator>,
    previews: Arc<infrastructure::cache::preview_manager::PreviewManager>,
    tokio: tokio::runtime::Handle,
}

impl GeradorDoDisco {
    /// O lado da miniatura da grade de importação.
    ///
    /// Menor que a da Biblioteca (320): aqui a célula é uma linha de lista, e
    /// gerar em 320 custaria o dobro do decode para desenhar em 48.
    const LADO: u32 = 128;

    pub fn novo(
        miniaturas: Arc<dyn domain::services::ThumbnailGenerator>,
        previews: Arc<infrastructure::cache::preview_manager::PreviewManager>,
        tokio: tokio::runtime::Handle,
    ) -> Self {
        Self {
            miniaturas,
            previews,
            tokio,
        }
    }
}

impl GeradorDeMiniaturas for GeradorDoDisco {
    fn gerar(&self, caminhos: Vec<String>, canal: Sender<Recado>) {
        let miniaturas = self.miniaturas.clone();
        let previews = self.previews.clone();

        self.tokio.spawn(async move {
            let mut prontas = Vec::new();

            for caminho in caminhos {
                let Ok(arquivo) = domain::value_objects::FilePath::new(&caminho) else {
                    continue;
                };
                let Ok(bytes) = miniaturas.generate(&arquivo, GeradorDoDisco::LADO).await else {
                    // ⚠️ Arquivo ilegível não interrompe o lote nem vira aviso: a
                    // célula fica com o retângulo vazio, que é o que a grade já
                    // mostra para quem ainda não chegou. Um cartão com um arquivo
                    // corrompido não pode encher a tela de erro.
                    continue;
                };
                let Ok(imagem) = image::load_from_memory(&bytes) else {
                    continue;
                };
                if previews
                    .save_thumbnail(&chave_de_miniatura(&caminho), &imagem)
                    .is_ok()
                {
                    prontas.push(caminho);
                }
            }

            if !prontas.is_empty() {
                let _ = canal.send(Recado::MiniaturasProntas(prontas));
            }
        });
    }
}

/// Quem sabe importar de verdade.
///
/// Separado do [`Explorador`] porque é outra decisão: explorar é grátis e
/// reversível, importar copia ou **move** arquivo. Um `trait` só faria o
/// explorador de mentira dos testes precisar saber importar.
pub trait Importador: Send + Sync + 'static {
    fn importar(
        &self,
        arquivos: Vec<String>,
        opcoes: ImportOptions,
        freios: Freios,
        canal: Sender<Andamento>,
    );
}

/// Os dois freios do lote, que a tela levanta e o importador obedece.
///
/// 🔑 **A tela é dona deles, e não o importador.** Um lote que corre numa thread
/// do tokio não tem como devolver um controle à interface depois de começar; o
/// arranjo aqui é o contrário — a tela cria as bandeiras, guarda a sua cópia e
/// entrega a outra. Clicar "Pausar" é escrever num `AtomicBool`, e por isso
/// nunca falha nem espera.
///
/// ⚠️ **Pausar não interrompe o arquivo em curso**: as até 8 tarefas paralelas
/// param antes do próximo, e a foto que já estava sendo lida termina. É o
/// mesmo do Lightroom, e é o que evita meia foto no catálogo.
#[derive(Clone, Default)]
pub struct Freios {
    pausa: Arc<std::sync::atomic::AtomicBool>,
    cancelar: Arc<std::sync::atomic::AtomicBool>,
}

impl Freios {
    pub fn pausada(&self) -> bool {
        self.pausa.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn pausar(&self, pausar: bool) {
        self.pausa
            .store(pausar, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn cancelada(&self) -> bool {
        self.cancelar.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Não tem par que desfaz: um lote cancelado não volta a correr, ele acaba.
    /// Solta a pausa junto — quem cancelou pausado espera que **acabe**, e as
    /// tarefas paradas precisam acordar para contar que desistiram.
    pub fn cancelar(&self) {
        self.cancelar
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.pausar(false);
    }
}

/// O que a importação vai contando enquanto trabalha.
#[derive(Debug, Clone, PartialEq)]
pub enum Andamento {
    Comecou {
        total: usize,
    },
    Feito {
        indice: usize,
        caminho: String,
    },
    Pulado {
        caminho: String,
    },
    Falhou {
        caminho: String,
        erro: String,
    },
    Terminou {
        sucesso: usize,
        falhas: usize,
        pulados: usize,
    },
}

pub struct ImportadorDoDisco {
    importacao: Arc<ImportController>,
    tokio: tokio::runtime::Handle,
}

impl ImportadorDoDisco {
    pub fn novo(importacao: Arc<ImportController>, tokio: tokio::runtime::Handle) -> Self {
        Self { importacao, tokio }
    }
}

impl Importador for ImportadorDoDisco {
    fn importar(
        &self,
        arquivos: Vec<String>,
        opcoes: ImportOptions,
        freios: Freios,
        canal: Sender<Andamento>,
    ) {
        let importacao = self.importacao.clone();

        self.tokio.spawn(async move {
            let (progresso, mut recebe) = tokio::sync::mpsc::unbounded_channel();

            let Freios { pausa, cancelar } = freios;

            let repassar = tokio::spawn(async move {
                use adapters::view_models::ImportProgressViewModel as Vm;
                while let Some(evento) = recebe.recv().await {
                    let andamento = match evento {
                        Vm::Starting { total } => Andamento::Comecou { total },
                        Vm::Completed { path, .. } => Andamento::Feito {
                            indice: 0,
                            caminho: path,
                        },
                        Vm::DuplicateSkipped { path, .. } => Andamento::Pulado { caminho: path },
                        Vm::Failed { path, error } => Andamento::Falhou {
                            caminho: path,
                            erro: error,
                        },
                        Vm::Finished {
                            successful,
                            failed,
                            skipped,
                        } => Andamento::Terminou {
                            sucesso: successful,
                            falhas: failed,
                            pulados: skipped,
                        },
                        // "Processing" e "Paused" não mudam nada que a tela
                        // mostre hoje: o contador anda no `Completed`.
                        _ => continue,
                    };
                    if canal.send(andamento).is_err() {
                        return;
                    }
                }
            });

            let _ = importacao
                .import_with_options(arquivos, opcoes, progresso, pausa, cancelar)
                .await;
            let _ = repassar.await;
        });
    }
}

/// Um explorador e um importador que respondem o que o teste mandar.
#[cfg(test)]
pub mod mentira {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    pub struct ExploradorDeMentira {
        /// O que responder à próxima varredura, por raiz.
        pub varreduras: Mutex<Vec<(String, Vec<String>)>>,
        pub pedidos: Mutex<Vec<String>>,
        pub cartoes: Mutex<Vec<Origem>>,
    }

    impl ExploradorDeMentira {
        pub fn responde(raiz: &str, arquivos: &[&str]) -> Self {
            Self {
                varreduras: Mutex::new(vec![(
                    raiz.to_string(),
                    arquivos.iter().map(|a| a.to_string()).collect(),
                )]),
                ..Default::default()
            }
        }

        pub fn pedidos(&self) -> Vec<String> {
            self.pedidos.lock().expect("os pedidos").clone()
        }
    }

    impl Explorador for ExploradorDeMentira {
        fn origens(&self, canal: Sender<Recado>) {
            self.pedidos
                .lock()
                .expect("os pedidos")
                .push("origens".into());
            let _ = canal.send(Recado::Origens {
                cartoes: self.cartoes.lock().expect("os cartões").clone(),
                recentes: Vec::new(),
            });
        }

        fn varrer(&self, raiz: String, _subpastas: bool, canal: Sender<Recado>) {
            self.pedidos
                .lock()
                .expect("os pedidos")
                .push(format!("varrer:{raiz}"));

            let resposta = self
                .varreduras
                .lock()
                .expect("as varreduras")
                .iter()
                .find(|(r, _)| *r == raiz)
                .map(|(_, arquivos)| arquivos.clone())
                .unwrap_or_default();

            let _ = canal.send(Recado::Varrido {
                raiz,
                arquivos: resposta,
            });
        }

        fn detalhar(&self, arquivos: Vec<String>, canal: Sender<Recado>) {
            self.pedidos
                .lock()
                .expect("os pedidos")
                .push(format!("detalhar:{}", arquivos.len()));

            let _ = canal.send(Recado::Descritos(
                arquivos
                    .iter()
                    .map(|caminho| Descricao {
                        caminho: caminho.clone(),
                        tamanho: 1024,
                        e_raw: caminho.ends_with(".NEF"),
                        camera: "Nikon Z6".into(),
                        data: "2026:08:16 10:00:00".into(),
                        dimensoes: Some("6000x4000".into()),
                    })
                    .collect(),
            ));
            let _ = canal.send(Recado::Duplicados(Vec::new()));
        }
    }

    /// Um seletor que devolve a pasta que o teste mandar, sem abrir janela.
    #[derive(Default)]
    pub struct SeletorDeMentira {
        pub escolha: Mutex<Option<String>>,
    }

    impl SeletorDeMentira {
        pub fn escolhe(caminho: &str) -> Self {
            Self {
                escolha: Mutex::new(Some(caminho.to_string())),
            }
        }
    }

    impl SeletorDePasta for SeletorDeMentira {
        fn escolher(&self, canal: Sender<Recado>) {
            let recado = match self.escolha.lock().expect("a escolha").clone() {
                Some(caminho) => Recado::OrigemEscolhida(caminho),
                None => Recado::SemEscolha,
            };
            let _ = canal.send(recado);
        }

        fn escolher_destino(&self, canal: Sender<Recado>) {
            let recado = match self.escolha.lock().expect("a escolha").clone() {
                Some(caminho) => Recado::DestinoEscolhido(caminho),
                None => Recado::SemEscolha,
            };
            let _ = canal.send(recado);
        }
    }

    /// Um gerador que diz "pronto" sem tocar em disco.
    #[derive(Default)]
    pub struct GeradorDeMentira {
        pub pedidos: Mutex<Vec<Vec<String>>>,
    }

    impl GeradorDeMentira {
        pub fn pedidos(&self) -> Vec<Vec<String>> {
            self.pedidos.lock().expect("os pedidos").clone()
        }
    }

    impl GeradorDeMiniaturas for GeradorDeMentira {
        fn gerar(&self, caminhos: Vec<String>, canal: Sender<Recado>) {
            self.pedidos
                .lock()
                .expect("os pedidos")
                .push(caminhos.clone());
            let _ = canal.send(Recado::MiniaturasProntas(caminhos));
        }
    }

    #[derive(Default)]
    pub struct ImportadorDeMentira {
        pub importados: Mutex<Vec<(Vec<String>, ImportOptions)>>,
        /// Os freios do último lote — é por aqui que o teste confere que o
        /// clique chegou até quem obedece.
        pub freios: Mutex<Option<Freios>>,
        /// Segura o andamento em vez de contá-lo na hora — o disco trabalhando.
        ///
        /// 🚨 **O `Default` responde no mesmo instante, e isso apaga o meio do
        /// lote.** Importar 500 fotos leva minutos, e é *durante* eles que o
        /// operador classifica e negocia. Sem isto, o lote nasce e morre dentro
        /// da mesma linha do teste, e esse estado não existe para ninguém.
        pub demorado: bool,
        pub guardados: Mutex<Vec<(Sender<Andamento>, Andamento)>>,
    }

    impl ImportadorDeMentira {
        /// O mesmo, mas contando o lote só quando o teste mandar.
        pub fn demorado() -> Self {
            Self {
                demorado: true,
                ..Default::default()
            }
        }

        pub fn importados(&self) -> Vec<(Vec<String>, ImportOptions)> {
            self.importados.lock().expect("os importados").clone()
        }

        /// Solta **um** passo do lote — o arquivo seguinte terminou.
        pub fn responder_uma(&self) {
            let mut guardados = self.guardados.lock().expect("os guardados");
            if guardados.is_empty() {
                return;
            }
            let (canal, andamento) = guardados.remove(0);
            let _ = canal.send(andamento);
        }

        /// Solta o lote inteiro.
        pub fn responder(&self) {
            for (canal, andamento) in self.guardados.lock().expect("os guardados").drain(..) {
                let _ = canal.send(andamento);
            }
        }

        fn contar(&self, canal: &Sender<Andamento>, andamento: Andamento) {
            if self.demorado {
                self.guardados
                    .lock()
                    .expect("os guardados")
                    .push((canal.clone(), andamento));
            } else {
                let _ = canal.send(andamento);
            }
        }

        pub fn freios(&self) -> Freios {
            self.freios
                .lock()
                .expect("os freios")
                .clone()
                .expect("nenhum lote foi pedido")
        }
    }

    impl Importador for ImportadorDeMentira {
        fn importar(
            &self,
            arquivos: Vec<String>,
            opcoes: ImportOptions,
            freios: Freios,
            canal: Sender<Andamento>,
        ) {
            *self.freios.lock().expect("os freios") = Some(freios);
            let total = arquivos.len();
            self.importados
                .lock()
                .expect("os importados")
                .push((arquivos.clone(), opcoes));

            self.contar(&canal, Andamento::Comecou { total });
            for (indice, caminho) in arquivos.into_iter().enumerate() {
                self.contar(&canal, Andamento::Feito { indice, caminho });
            }
            self.contar(
                &canal,
                Andamento::Terminou {
                    sucesso: total,
                    falhas: 0,
                    pulados: 0,
                },
            );
        }
    }
}
