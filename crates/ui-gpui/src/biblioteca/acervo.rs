//! Reler o catálogo depois que ele muda por fora da Biblioteca.
//!
//! ## 🚨 Por que isto existe
//!
//! As fotos são lidas **uma vez**, no `main.rs`, antes da janela existir, e
//! entregues à [`super::tela::Biblioteca`] como um `Vec` pronto. Enquanto a
//! grade era a única coisa que mexia no acervo isso bastava.
//!
//! A importação quebrou a conta, e de um jeito que **não falha em lugar
//! nenhum**: ela grava no banco, o modal conta "65 importadas · 1 falharam", e a
//! grade atrás continua exatamente como estava. Quem importa vê a contagem certa
//! e o acervo vazio — e a conclusão razoável é "a importação não funciona",
//! quando as 65 fotos estão no banco e no disco. Só apareciam ao reabrir o app.
//!
//! ## Por que é uma porta, e não uma chamada
//!
//! [`LibraryController::get_all_photos`] é `async` do tokio, e o GPUI não roda
//! futuros dele — é a mesma razão do `Gravador` e do `Marcador`. E, como eles, a
//! porta existe também para o teste poder afirmar **quando** o acervo é relido:
//! com um acervo de mentira, "importar recarrega a grade" é uma linha.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use adapters::controllers::LibraryController;
use adapters::view_models::PhotoViewModel;

/// Quem sabe reler o catálogo inteiro.
pub trait Acervo: Send + Sync + 'static {
    /// Pede a releitura e **devolve na hora**. A resposta chega pelo canal.
    ///
    /// ⚠️ **Sem `Result`.** Uma releitura que falha é a grade continuar
    /// mostrando o que mostrava — ruim, mas não é motivo para derrubar quem
    /// pediu. É a mesma decisão da porta de publicação de eventos: anunciar é
    /// acessório, e um `?` faria a falha do acessório derrubar o principal.
    fn recarregar(&self, canal: Sender<Vec<PhotoViewModel>>);
}

/// O acervo de verdade: o `LibraryController` numa `Handle` do tokio.
pub struct AcervoDoBanco {
    biblioteca: Arc<LibraryController>,
    /// ⚠️ Capturada no `main`, **antes** de `Application::run` tomar a thread.
    /// Um `tokio::spawn` de dentro do GPUI entra em pânico com *there is no
    /// reactor running*.
    tokio: tokio::runtime::Handle,
}

impl AcervoDoBanco {
    pub fn novo(biblioteca: Arc<LibraryController>, tokio: tokio::runtime::Handle) -> Self {
        Self { biblioteca, tokio }
    }
}

impl Acervo for AcervoDoBanco {
    fn recarregar(&self, canal: Sender<Vec<PhotoViewModel>>) {
        let biblioteca = self.biblioteca.clone();
        self.tokio.spawn(async move {
            // 🔑 O erro morre aqui de propósito: quem espera é uma grade, e o
            // pior desfecho de uma leitura que falhou é ela seguir com a lista
            // anterior. Mandar `Vec` vazio seria pior — apagaria o acervo da
            // vista por causa de um erro de leitura.
            if let Ok(fotos) = biblioteca.get_all_photos().await {
                let _ = canal.send(fotos);
            }
        });
    }
}

/// O acervo dos testes: responde o que lhe mandarem, e conta quantas vezes.
#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct AcervoDeMentira {
        pub fotos: Mutex<Vec<PhotoViewModel>>,
        pub pedidos: Mutex<usize>,
    }

    impl Acervo for AcervoDeMentira {
        fn recarregar(&self, canal: Sender<Vec<PhotoViewModel>>) {
            *self.pedidos.lock().expect("os pedidos") += 1;
            let fotos = self.fotos.lock().expect("as fotos").clone();
            let _ = canal.send(fotos);
        }
    }
}
