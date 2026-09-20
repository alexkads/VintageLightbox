//! A ponte entre a tela e o `ExportController`.
//!
//! Mesma forma das outras portas da casa (`Gravador`, `Marcador`, `Acervo`, as
//! quatro da importação): o controller é `async` do tokio, o GPUI não roda
//! futuros dele, e o `Handle` é capturado no `main` antes de `Application::run`
//! tomar a thread.
//!
//! 🚨 **Esta é a porta que faltava para o app entregar alguma coisa.** O
//! `ExportPhotoUseCase`, o `ExportController` e o `ImageExporterImpl` existiam,
//! testados, desde antes da migração — e **nunca foram construídos no
//! `main.rs`**. Camada pronta não é funcionalidade entregue.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use adapters::controllers::ExportController;
use domain::value_objects::ExportOptions;

/// Uma foto do lote: o id que o banco conhece e o arquivo que vai ser criado.
#[derive(Debug, Clone, PartialEq)]
pub struct Saida {
    pub id: String,
    pub destino: PathBuf,
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
    Terminou {
        sucesso: usize,
        falhas: usize,
    },
}

pub trait Exportador: Send + Sync + 'static {
    /// Enfileira o lote e **devolve na hora**. O andamento chega pelo canal.
    ///
    /// 🔑 **As opções valem para o lote inteiro, e não por foto.** É o que
    /// separa "entrega final" de "prévia da galeria": as duas são o mesmo lote
    /// com uma decisão diferente na frente, e deixar a decisão por item abriria
    /// a porta para metade sair marcada e metade não.
    fn exportar(&self, saidas: Vec<Saida>, opcoes: ExportOptions, canal: Sender<Andamento>);
}

pub struct ExportadorDoBanco {
    controlador: Arc<ExportController>,
    tokio: tokio::runtime::Handle,
}

impl ExportadorDoBanco {
    pub fn novo(controlador: Arc<ExportController>, tokio: tokio::runtime::Handle) -> Self {
        Self { controlador, tokio }
    }
}

impl Exportador for ExportadorDoBanco {
    fn exportar(&self, saidas: Vec<Saida>, opcoes: ExportOptions, canal: Sender<Andamento>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let _ = canal.send(Andamento::Comecou {
                total: saidas.len(),
            });

            let (mut sucesso, mut falhas) = (0usize, 0usize);
            for saida in saidas {
                // 🚨 A pasta é criada aqui, e não pelo exportador: ele recebe um
                // caminho de arquivo e `File::create` não cria pasta. Sem isto,
                // escolher uma pasta que não existe falharia foto a foto com um
                // erro de sistema que não diz o que fazer.
                if let Some(pasta) = saida.destino.parent() {
                    let _ = tokio::fs::create_dir_all(pasta).await;
                }

                let destino = saida.destino.to_string_lossy().to_string();
                match controlador.export_photo(saida.id, destino, &opcoes).await {
                    Ok(()) => {
                        sucesso += 1;
                        let _ = canal.send(Andamento::Feita {
                            destino: saida.destino,
                        });
                    }
                    Err(erro) => {
                        falhas += 1;
                        let _ = canal.send(Andamento::Falhou {
                            destino: saida.destino,
                            erro,
                        });
                    }
                }
            }

            let _ = canal.send(Andamento::Terminou { sucesso, falhas });
        });
    }
}

/// O exportador dos testes: registra o que foi pedido e responde na hora.
#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct ExportadorDeMentira {
        pub pedidos: Mutex<Vec<Vec<Saida>>>,
        /// As opções do último lote — é como o teste confere que a marca d'água
        /// pedida na tela chegou até a porta.
        pub opcoes: Mutex<Option<ExportOptions>>,
        /// Quantas fotos do início do lote devem falhar — para a tela poder ser
        /// testada com falha no meio, que é o caso que ninguém reproduz à mão.
        pub falham: Mutex<usize>,
    }

    impl ExportadorDeMentira {
        pub fn pedidos(&self) -> Vec<Vec<Saida>> {
            self.pedidos.lock().expect("os pedidos").clone()
        }
    }

    impl Exportador for ExportadorDeMentira {
        fn exportar(&self, saidas: Vec<Saida>, opcoes: ExportOptions, canal: Sender<Andamento>) {
            *self.opcoes.lock().expect("as opções") = Some(opcoes);
            self.pedidos
                .lock()
                .expect("os pedidos")
                .push(saidas.clone());

            let quantas_falham = *self.falham.lock().expect("as falhas");
            let _ = canal.send(Andamento::Comecou {
                total: saidas.len(),
            });

            let (mut sucesso, mut falhas) = (0usize, 0usize);
            for (i, saida) in saidas.into_iter().enumerate() {
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

            let _ = canal.send(Andamento::Terminou { sucesso, falhas });
        }
    }
}
