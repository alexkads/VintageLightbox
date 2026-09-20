//! Coleções: o ensaio de um cliente, e o que a grade mostra dele.
//!
//! ## 🔑 Por que isto não é "mais uma forma de organizar"
//!
//! No fluxo do estúdio, **o ensaio de um cliente é uma coleção** — e é dela que a
//! galeria do `recordarfotos.com.br` sai. Pasta não serve para isso: a foto mora
//! numa pasta só, e o mesmo arquivo pode estar no ensaio do cliente, na seleção
//! do portfólio e no que ficou para trás. A coleção é a única estrutura do
//! Lightroom em que a mesma foto pertence a vários lugares sem ser copiada.
//!
//! ## O que este módulo é, e o que ele não é
//!
//! Aqui ficam **a porta e a regra**; a tela fica em [`super::tela`] e em
//! [`super::paineis`]. O corte é o mesmo do resto da casa — o que decide o que a
//! grade mostra tem de poder ser testado sem janela.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use adapters::controllers::{CollectionController, CollectionViewModel};

/// O que volta da camada de baixo.
#[derive(Debug, Clone, PartialEq)]
pub enum Recado {
    /// A lista lateral inteira.
    Listadas(Vec<CollectionViewModel>),
    /// Os ids de uma coleção — o que a grade precisa para filtrar.
    Fotos {
        colecao: String,
        ids: Vec<String>,
    },
    /// Uma coleção nova nasceu. Leva o id para poder ser aberta na hora.
    Criada(CollectionViewModel),
    /// O lote de acrescentar/remover terminou.
    Mudou {
        colecao: String,
        quantas: usize,
    },
    Falhou(String),
}

pub trait Colecoes: Send + Sync + 'static {
    fn listar(&self, canal: Sender<Recado>);
    fn fotos(&self, colecao: String, canal: Sender<Recado>);
    fn criar(&self, nome: String, canal: Sender<Recado>);
    fn acrescentar(&self, colecao: String, fotos: Vec<String>, canal: Sender<Recado>);
    fn remover(&self, colecao: String, fotos: Vec<String>, canal: Sender<Recado>);
}

pub struct ColecoesDoBanco {
    controlador: Arc<CollectionController>,
    tokio: tokio::runtime::Handle,
}

impl ColecoesDoBanco {
    pub fn novo(controlador: Arc<CollectionController>, tokio: tokio::runtime::Handle) -> Self {
        Self { controlador, tokio }
    }
}

impl Colecoes for ColecoesDoBanco {
    fn listar(&self, canal: Sender<Recado>) {
        let c = self.controlador.clone();
        self.tokio.spawn(async move {
            let _ = canal.send(match c.list().await {
                Ok(lista) => Recado::Listadas(lista),
                Err(erro) => Recado::Falhou(erro),
            });
        });
    }

    fn fotos(&self, colecao: String, canal: Sender<Recado>) {
        let c = self.controlador.clone();
        self.tokio.spawn(async move {
            let _ = canal.send(match c.photo_ids(colecao.clone()).await {
                Ok(ids) => Recado::Fotos { colecao, ids },
                Err(erro) => Recado::Falhou(erro),
            });
        });
    }

    fn criar(&self, nome: String, canal: Sender<Recado>) {
        let c = self.controlador.clone();
        self.tokio.spawn(async move {
            let _ = canal.send(match c.create(nome).await {
                Ok(nova) => Recado::Criada(nova),
                Err(erro) => Recado::Falhou(erro),
            });
        });
    }

    fn acrescentar(&self, colecao: String, fotos: Vec<String>, canal: Sender<Recado>) {
        let c = self.controlador.clone();
        self.tokio.spawn(async move {
            let _ = canal.send(match c.add_photos(colecao.clone(), fotos).await {
                Ok(quantas) => Recado::Mudou { colecao, quantas },
                Err(erro) => Recado::Falhou(erro),
            });
        });
    }

    fn remover(&self, colecao: String, fotos: Vec<String>, canal: Sender<Recado>) {
        let c = self.controlador.clone();
        self.tokio.spawn(async move {
            let _ = canal.send(match c.remove_photos(colecao.clone(), fotos).await {
                Ok(quantas) => Recado::Mudou { colecao, quantas },
                Err(erro) => Recado::Falhou(erro),
            });
        });
    }
}

/// O que a Biblioteca sabe sobre coleções — sem tela.
#[derive(Default)]
pub struct Estado {
    pub lista: Vec<CollectionViewModel>,
    /// A coleção aberta. `None` é "todas as fotos", que é o estado normal.
    aberta: Option<String>,
    /// Os ids da coleção aberta.
    ///
    /// 🚨 **Guardados como `HashSet`, e a ordem da grade continua sendo a do
    /// acervo.** Filtrar por pertencimento é a pergunta; usar a ordem do
    /// conjunto seria deixar a grade se reembaralhar entre execuções, porque a
    /// coleção guarda os ids num `HashSet` do domínio.
    ids: std::collections::HashSet<String>,
    pub aviso: Option<String>,
}

impl Estado {
    pub fn aberta(&self) -> Option<&str> {
        self.aberta.as_deref()
    }

    /// O nome da coleção aberta, para a tela poder dizer o que está mostrando.
    pub fn nome_da_aberta(&self) -> Option<&str> {
        let id = self.aberta.as_deref()?;
        self.lista
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.as_str())
    }

    /// Esta foto passa pelo filtro de coleção?
    ///
    /// ⚠️ **Sem coleção aberta, tudo passa** — e é isso que faz o filtro de
    /// coleção conviver com os de nota, cor e busca em vez de substituí-los.
    pub fn aceita(&self, id_da_foto: &str) -> bool {
        match self.aberta {
            None => true,
            Some(_) => self.ids.contains(id_da_foto),
        }
    }

    /// Abre uma coleção. Devolve `true` quando os ids precisam ser buscados.
    ///
    /// 🔑 **Abrir a mesma que já está aberta fecha.** É como o filtro de cor e o
    /// de sinalizador funcionam na barra: a mesma tecla liga e desliga, e sem
    /// isso "voltar a ver tudo" precisaria de um segundo controle chamado
    /// "todas" — que é o que o legado tinha e ninguém achava.
    pub fn abrir(&mut self, colecao: &str) -> bool {
        if self.aberta.as_deref() == Some(colecao) {
            self.aberta = None;
            self.ids.clear();
            return false;
        }
        self.aberta = Some(colecao.to_string());
        self.ids.clear();
        true
    }

    pub fn fechar(&mut self) {
        self.aberta = None;
        self.ids.clear();
    }

    /// Recebe os ids de uma coleção.
    ///
    /// 🚨 **Descarta a resposta que não é da coleção aberta.** Clicar em duas
    /// coleções em sequência dispara duas buscas, e elas voltam na ordem que o
    /// banco quiser: sem esta guarda, a grade mostraria o conteúdo da primeira
    /// com o nome da segunda no cabeçalho. É a mesma corrida que a importação
    /// tem entre origens.
    pub fn receber_fotos(&mut self, colecao: &str, ids: Vec<String>) -> bool {
        if self.aberta.as_deref() != Some(colecao) {
            return false;
        }
        self.ids = ids.into_iter().collect();
        true
    }

    pub fn quantas_na_aberta(&self) -> usize {
        self.ids.len()
    }
}

/// A porta de mentira: responde na hora e registra o que foi pedido.
#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct ColecoesDeMentira {
        pub lista: Mutex<Vec<CollectionViewModel>>,
        pub fotos: Mutex<Vec<(String, Vec<String>)>>,
        pub acrescentados: Mutex<Vec<(String, Vec<String>)>>,
        pub removidos: Mutex<Vec<(String, Vec<String>)>>,
        pub criadas: Mutex<Vec<String>>,
    }

    impl ColecoesDeMentira {
        pub fn com(lista: Vec<CollectionViewModel>) -> Self {
            Self {
                lista: Mutex::new(lista),
                ..Default::default()
            }
        }

        pub fn responde_fotos(&self, colecao: &str, ids: &[&str]) {
            self.fotos.lock().expect("as fotos").push((
                colecao.to_string(),
                ids.iter().map(|s| s.to_string()).collect(),
            ));
        }
    }

    impl Colecoes for ColecoesDeMentira {
        fn listar(&self, canal: Sender<Recado>) {
            let _ = canal.send(Recado::Listadas(
                self.lista.lock().expect("a lista").clone(),
            ));
        }

        fn fotos(&self, colecao: String, canal: Sender<Recado>) {
            let ids = self
                .fotos
                .lock()
                .expect("as fotos")
                .iter()
                .find(|(c, _)| *c == colecao)
                .map(|(_, ids)| ids.clone())
                .unwrap_or_default();
            let _ = canal.send(Recado::Fotos { colecao, ids });
        }

        fn criar(&self, nome: String, canal: Sender<Recado>) {
            self.criadas.lock().expect("as criadas").push(nome.clone());
            let nova = CollectionViewModel {
                id: format!("id-{nome}"),
                name: nome,
                photo_count: 0,
            };
            self.lista.lock().expect("a lista").push(nova.clone());
            let _ = canal.send(Recado::Criada(nova));
        }

        fn acrescentar(&self, colecao: String, fotos: Vec<String>, canal: Sender<Recado>) {
            let quantas = fotos.len();
            self.acrescentados
                .lock()
                .expect("os acrescentados")
                .push((colecao.clone(), fotos));
            let _ = canal.send(Recado::Mudou { colecao, quantas });
        }

        fn remover(&self, colecao: String, fotos: Vec<String>, canal: Sender<Recado>) {
            let quantas = fotos.len();
            self.removidos
                .lock()
                .expect("os removidos")
                .push((colecao.clone(), fotos));
            let _ = canal.send(Recado::Mudou { colecao, quantas });
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn colecao(id: &str, nome: &str) -> CollectionViewModel {
        CollectionViewModel {
            id: id.to_string(),
            name: nome.to_string(),
            photo_count: 0,
        }
    }

    /// ⚠️ Sem coleção aberta, a grade mostra tudo.
    #[test]
    fn sem_colecao_aberta_toda_foto_passa() {
        let estado = Estado::default();
        assert!(estado.aceita("qualquer-uma"));
        assert!(estado.aberta().is_none());
    }

    /// 🔑 Clicar na coleção aberta fecha, e a grade volta a mostrar tudo.
    ///
    /// Sem isso, "ver o acervo de novo" exigiria um segundo controle — e o
    /// caminho de volta é o que mais falta numa tela de filtro.
    #[test]
    fn clicar_na_aberta_fecha() {
        let mut estado = Estado::default();

        assert!(estado.abrir("ensaio-1"), "abrir pede os ids");
        estado.receber_fotos("ensaio-1", vec!["a".into()]);
        assert!(estado.aceita("a"));
        assert!(!estado.aceita("b"));

        assert!(!estado.abrir("ensaio-1"), "fechar não pede ids");
        assert!(estado.aberta().is_none());
        assert!(estado.aceita("b"), "fechada, tudo passa de novo");
    }

    /// 🚨 **A resposta de uma coleção que não está mais aberta é descartada.**
    ///
    /// Clicar em duas coleções em sequência dispara duas buscas, e elas voltam na
    /// ordem que o banco quiser. Sem esta guarda a grade mostraria o conteúdo da
    /// primeira com o nome da segunda no cabeçalho — e nada falharia.
    #[test]
    fn resposta_atrasada_de_outra_colecao_e_ignorada() {
        let mut estado = Estado::default();
        estado.abrir("ensaio-1");
        estado.abrir("ensaio-2");

        assert!(
            !estado.receber_fotos("ensaio-1", vec!["a".into(), "b".into()]),
            "a resposta da primeira chegou depois da troca"
        );
        assert_eq!(estado.quantas_na_aberta(), 0);

        assert!(estado.receber_fotos("ensaio-2", vec!["c".into()]));
        assert!(estado.aceita("c"));
        assert!(!estado.aceita("a"));
    }

    /// Trocar de coleção não deixa os ids da anterior para trás.
    #[test]
    fn trocar_de_colecao_limpa_os_ids() {
        let mut estado = Estado::default();
        estado.abrir("ensaio-1");
        estado.receber_fotos("ensaio-1", vec!["a".into()]);

        estado.abrir("ensaio-2");
        assert!(
            !estado.aceita("a"),
            "os ids da coleção anterior sobreviveram à troca"
        );
    }

    /// A tela precisa dizer o nome do que está mostrando, e não o id.
    #[test]
    fn o_nome_da_aberta_sai_da_lista() {
        let mut estado = Estado {
            lista: vec![colecao("id-1", "Casamento Ana e João")],
            ..Default::default()
        };
        estado.abrir("id-1");

        assert_eq!(estado.nome_da_aberta(), Some("Casamento Ana e João"));
    }
}
