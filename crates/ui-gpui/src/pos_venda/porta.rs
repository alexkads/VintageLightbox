//! A ponte entre a tela e o `PosVendaController` — a mesma forma das outras
//! portas: o controller é `async` do tokio, o GPUI não roda futuros dele, e o
//! `Handle` é capturado no `main` antes de `Application::run` tomar a thread.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use adapters::controllers::PosVendaController;
use domain::services::pos_venda::{NovaGaleria, Produto, Sessao};
use use_cases::pos_venda::Progresso;

/// O que a tela pede para publicar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pedido {
    pub sessao: Sessao,
    pub galeria: NovaGaleria,
    /// Os ids da grade, na ordem em que vão aparecer no site.
    pub fotos: Vec<String>,
}

/// O que volta pelo canal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recado {
    Entrou(Sessao),
    Produtos(Vec<Produto>),
    Andamento(Progresso),
    /// Uma frase para a tela — login recusado, rede caída, galeria recusada.
    Falhou(String),
}

pub trait Publicador: Send + Sync + 'static {
    /// Todas devolvem na hora; a resposta vem pelo canal.
    fn entrar(&self, email: String, senha: String, canal: Sender<Recado>);
    fn produtos(&self, sessao: Sessao, canal: Sender<Recado>);
    fn publicar(&self, pedido: Pedido, canal: Sender<Recado>);
}

pub struct PublicadorDaApi {
    controlador: Arc<PosVendaController>,
    tokio: tokio::runtime::Handle,
}

impl PublicadorDaApi {
    pub fn novo(controlador: Arc<PosVendaController>, tokio: tokio::runtime::Handle) -> Self {
        Self { controlador, tokio }
    }
}

impl Publicador for PublicadorDaApi {
    fn entrar(&self, email: String, senha: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.entrar(&email, &senha).await {
                Ok(sessao) => Recado::Entrou(sessao),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn produtos(&self, sessao: Sessao, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.produtos(&sessao).await {
                Ok(produtos) => Recado::Produtos(produtos),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn publicar(&self, pedido: Pedido, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            // O andamento do caso de uso chega por um canal próprio e é
            // repassado embrulhado: a tela tem um receptor só.
            let (tx, rx) = std::sync::mpsc::channel();
            let repasse = canal.clone();
            let ponte = std::thread::spawn(move || {
                for progresso in rx {
                    let _ = repasse.send(Recado::Andamento(progresso));
                }
            });

            let resultado = controlador
                .publicar(pedido.sessao, pedido.galeria, pedido.fotos, tx)
                .await;
            let _ = ponte.join();

            if let Err(erro) = resultado {
                let _ = canal.send(Recado::Falhou(erro));
            }
        });
    }
}

#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    /// Entra com qualquer senha, lista os produtos que recebeu, e "publica"
    /// respondendo o andamento inteiro na hora.
    #[derive(Default)]
    pub struct PublicadorDeMentira {
        pub produtos: Vec<Produto>,
        pub pedidos: Mutex<Vec<Pedido>>,
        /// A senha que entra; qualquer outra é recusada — para a tela poder ser
        /// testada com login errado.
        pub senha_certa: Option<String>,
    }

    impl PublicadorDeMentira {
        pub fn pedidos(&self) -> Vec<Pedido> {
            self.pedidos.lock().expect("os pedidos").clone()
        }
    }

    impl Publicador for PublicadorDeMentira {
        fn entrar(&self, _email: String, senha: String, canal: Sender<Recado>) {
            let aceita = self
                .senha_certa
                .as_deref()
                .is_none_or(|certa| certa == senha);
            let _ = canal.send(if aceita {
                Recado::Entrou(Sessao {
                    access_token: "tok-de-mentira".into(),
                })
            } else {
                Recado::Falhou("e-mail ou senha recusados pelo site".into())
            });
        }

        fn produtos(&self, _sessao: Sessao, canal: Sender<Recado>) {
            let _ = canal.send(Recado::Produtos(self.produtos.clone()));
        }

        fn publicar(&self, pedido: Pedido, canal: Sender<Recado>) {
            self.pedidos
                .lock()
                .expect("os pedidos")
                .push(pedido.clone());
            let total = pedido.fotos.len();
            let _ = canal.send(Recado::Andamento(Progresso::Comecou { total }));
            let _ = canal.send(Recado::Andamento(Progresso::GaleriaCriada {
                id: "g-de-mentira".into(),
            }));
            for id in &pedido.fotos {
                let _ = canal.send(Recado::Andamento(Progresso::Enviada {
                    nome: format!("{id}.jpg"),
                    estado: domain::services::pos_venda::EstadoNoBalcao::Disponivel,
                }));
            }
            let _ = canal.send(Recado::Andamento(Progresso::Terminou {
                galeria_id: "g-de-mentira".into(),
                sucesso: total,
                falhas: 0,
            }));
        }
    }
}
