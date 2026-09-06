//! A ponte entre a tela e o `PosVendaController` — a mesma forma das outras
//! portas: o controller é `async` do tokio, o GPUI não roda futuros dele, e o
//! `Handle` é capturado no `main` antes de `Application::run` tomar a thread.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use adapters::controllers::PosVendaController;
use domain::services::pos_venda::{
    Galeria, GaleriaDoPainel, LinkDeAcesso, MudancaDaFoto, NovaGaleria, Produto, Sessao,
};
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
    /// O link que entra sem senha, pronto para ir ao cliente.
    Link(LinkDeAcesso),
    /// As sessões fotográficas que já existem.
    Galerias(Vec<GaleriaDoPainel>),
    /// Uma sessão recém-aberta, ainda sem foto nenhuma.
    Criada(Galeria),
    /// Os pixels de uma foto do site — o passo 11.
    ///
    /// Leva o id da foto junto: um download que volta depois de a seta ter
    /// andado não pode pintar a foto errada, e quem confere isso é a tela.
    Pixels {
        foto_id: String,
        bytes: Vec<u8>,
    },
    /// O passo 3 terminou para uma foto: ela subiu, ou saiu do storage.
    ///
    /// 🔑 **Notifica, não descreve.** Quem escuta só precisa saber que o
    /// catálogo mudou, para reler — os números estão no banco.
    Sincronizou,
    /// Uma frase para a tela — login recusado, rede caída, galeria recusada.
    Falhou(String),
}

pub trait Publicador: Send + Sync + 'static {
    /// Todas devolvem na hora; a resposta vem pelo canal.
    fn entrar(&self, email: String, senha: String, canal: Sender<Recado>);
    fn produtos(&self, sessao: Sessao, canal: Sender<Recado>);
    fn publicar(&self, pedido: Pedido, canal: Sender<Recado>);
    /// O passo 7 do fluxo: o link do cliente.
    ///
    /// ⚠️ **Pedido ao site, nunca montado aqui.** O endereço da galeria exige
    /// sessão e o cliente não tem conta — ele saiu do estúdio, não do site.
    fn link(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>);
    /// A lista de sessões fotográficas.
    fn galerias(&self, sessao: Sessao, canal: Sender<Recado>);
    /// Abre uma sessão vazia — as fotos vêm depois.
    fn criar_galeria(&self, sessao: Sessao, nova: NovaGaleria, canal: Sender<Recado>);
    /// O passo 3: a foto foi classificada e sobe para a galeria aberta.
    fn subir_classificada(
        &self,
        sessao: Sessao,
        galeria_id: String,
        foto_id: String,
        ordem: u32,
        canal: Sender<Recado>,
    );
    /// O passo 3 ao contrário: a classificação foi zerada, a foto sai do storage.
    fn tirar_do_site(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>);
    /// O passo 6: o que o cliente acertou no balcão, gravado na foto do site.
    fn negociar(
        &self,
        sessao: Sessao,
        foto_id: String,
        mudanca: MudancaDaFoto,
        canal: Sender<Recado>,
    );
    /// O passo 11: os pixels da foto que só existe no storage.
    ///
    /// `foto_local` é o id **do catálogo**, e volta no recado: é por ele que a
    /// tela confere se a foto na frente ainda é a mesma.
    fn copia_de_trabalho(
        &self,
        sessao: Sessao,
        foto_local: String,
        foto_no_site: String,
        canal: Sender<Recado>,
    );
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

    fn link(&self, sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.link_da_galeria(&sessao, &galeria_id).await {
                Ok(link) => Recado::Link(link),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn galerias(&self, sessao: Sessao, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.galerias(&sessao).await {
                Ok(lista) => Recado::Galerias(lista),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn criar_galeria(&self, sessao: Sessao, nova: NovaGaleria, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.criar_galeria(&sessao, &nova).await {
                Ok(galeria) => Recado::Criada(galeria),
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn subir_classificada(
        &self,
        sessao: Sessao,
        galeria_id: String,
        foto_id: String,
        ordem: u32,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador
                .enviar_uma(&sessao, &galeria_id, &foto_id, ordem)
                .await
            {
                Ok(_) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn tirar_do_site(&self, sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.remover_do_site(&sessao, &foto_id).await {
                Ok(()) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn negociar(
        &self,
        sessao: Sessao,
        foto_id: String,
        mudanca: MudancaDaFoto,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.mudar_foto(&sessao, &foto_id, &mudanca).await {
                Ok(()) => Recado::Sincronizou,
                Err(erro) => Recado::Falhou(erro),
            };
            let _ = canal.send(recado);
        });
    }

    fn copia_de_trabalho(
        &self,
        sessao: Sessao,
        foto_local: String,
        foto_no_site: String,
        canal: Sender<Recado>,
    ) {
        let controlador = self.controlador.clone();
        self.tokio.spawn(async move {
            let recado = match controlador.copia_de_trabalho(&sessao, &foto_no_site).await {
                Ok(bytes) => Recado::Pixels {
                    foto_id: foto_local,
                    bytes,
                },
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
        /// De quais galerias o link foi pedido.
        pub links: Mutex<Vec<String>>,
        /// As sessões que a listagem vai encontrar.
        pub galerias: Mutex<Vec<GaleriaDoPainel>>,
        /// As que foram abertas por esta tela.
        pub criadas: Mutex<Vec<NovaGaleria>>,
        /// `(galeria, foto, ordem)` de cada classificada que subiu.
        pub subidas: Mutex<Vec<(String, String, u32)>>,
        /// As fotos tiradas do storage.
        pub tiradas: Mutex<Vec<String>>,
        /// O que foi negociado, por foto.
        pub negociadas: Mutex<Vec<(String, MudancaDaFoto)>>,
        /// Os ids no site cujos pixels foram pedidos.
        pub baixadas: Mutex<Vec<String>>,
    }

    /// Um JPEG 1×1 cinza, codificado de verdade.
    fn jpeg_de_um_pixel() -> Vec<u8> {
        let mut bytes = Vec::new();
        let imagem = image::RgbImage::from_pixel(1, 1, image::Rgb([128, 128, 128]));
        image::DynamicImage::ImageRgb8(imagem)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Jpeg,
            )
            .expect("codificar um pixel");
        bytes
    }

    impl PublicadorDeMentira {
        pub fn pedidos(&self) -> Vec<Pedido> {
            self.pedidos.lock().expect("os pedidos").clone()
        }

        pub fn links(&self) -> Vec<String> {
            self.links.lock().expect("os links").clone()
        }

        pub fn criadas(&self) -> Vec<NovaGaleria> {
            self.criadas.lock().expect("as criadas").clone()
        }

        pub fn subidas(&self) -> Vec<(String, String, u32)> {
            self.subidas.lock().expect("as subidas").clone()
        }

        pub fn tiradas(&self) -> Vec<String> {
            self.tiradas.lock().expect("as tiradas").clone()
        }

        pub fn negociadas(&self) -> Vec<(String, MudancaDaFoto)> {
            self.negociadas.lock().expect("as negociadas").clone()
        }

        pub fn baixadas(&self) -> Vec<String> {
            self.baixadas.lock().expect("as baixadas").clone()
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

        fn link(&self, _sessao: Sessao, galeria_id: String, canal: Sender<Recado>) {
            self.links
                .lock()
                .expect("os links")
                .push(galeria_id.clone());
            let _ = canal.send(Recado::Link(LinkDeAcesso {
                url: format!("https://recordarfotos.com.br/entrar?t={galeria_id}"),
                validade_em_segundos: 604_800,
            }));
        }

        fn subir_classificada(
            &self,
            _sessao: Sessao,
            galeria_id: String,
            foto_id: String,
            ordem: u32,
            canal: Sender<Recado>,
        ) {
            self.subidas
                .lock()
                .expect("as subidas")
                .push((galeria_id, foto_id, ordem));
            let _ = canal.send(Recado::Sincronizou);
        }

        fn tirar_do_site(&self, _sessao: Sessao, foto_id: String, canal: Sender<Recado>) {
            self.tiradas.lock().expect("as tiradas").push(foto_id);
            let _ = canal.send(Recado::Sincronizou);
        }

        fn copia_de_trabalho(
            &self,
            _sessao: Sessao,
            foto_local: String,
            foto_no_site: String,
            canal: Sender<Recado>,
        ) {
            self.baixadas
                .lock()
                .expect("as baixadas")
                .push(foto_no_site);
            // Um JPEG 1×1 de verdade: a tela decodifica o que chega, e um vetor
            // de lixo faria o teste passar por um caminho que a produção não tem.
            let _ = canal.send(Recado::Pixels {
                foto_id: foto_local,
                bytes: jpeg_de_um_pixel(),
            });
        }

        fn negociar(
            &self,
            _sessao: Sessao,
            foto_id: String,
            mudanca: MudancaDaFoto,
            canal: Sender<Recado>,
        ) {
            self.negociadas
                .lock()
                .expect("as negociadas")
                .push((foto_id, mudanca));
            let _ = canal.send(Recado::Sincronizou);
        }

        fn galerias(&self, _sessao: Sessao, canal: Sender<Recado>) {
            let _ = canal.send(Recado::Galerias(
                self.galerias.lock().expect("as galerias").clone(),
            ));
        }

        fn criar_galeria(&self, _sessao: Sessao, nova: NovaGaleria, canal: Sender<Recado>) {
            let id = format!("g{}", self.criadas.lock().expect("as criadas").len() + 1);
            self.criadas.lock().expect("as criadas").push(nova.clone());
            // A criada entra na lista, como entraria no site.
            self.galerias
                .lock()
                .expect("as galerias")
                .push(GaleriaDoPainel {
                    id: id.clone(),
                    titulo: nova.titulo.clone(),
                    email: nova.email.clone(),
                    whatsapp: nova.whatsapp.clone(),
                    produto_id: nova.produto_id.clone(),
                    user_id: None,
                    criada_em_iso: "2026-09-06".into(),
                    expira_em: None,
                    fotos: domain::services::pos_venda::ContagemDeFotos::default(),
                    totais: None,
                });
            let _ = canal.send(Recado::Criada(Galeria {
                id,
                titulo: nova.titulo,
            }));
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
            let _ = canal.send(Recado::Andamento(Progresso::ClienteAvisado { falha: None }));
            let _ = canal.send(Recado::Andamento(Progresso::Terminou {
                galeria_id: "g-de-mentira".into(),
                sucesso: total,
                falhas: 0,
            }));
        }
    }
}
