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

    /// Passa as fotos da sessão `de` para a sessão `para` — o rascunho da nova
    /// sessão virando a sessão criada no site. Responde quantas mudaram.
    fn trocar_sessao(&self, de: String, para: String, canal: Sender<Result<usize, String>>) {
        let _ = (de, para);
        let _ = canal.send(Err("este acervo não troca sessão".into()));
    }

    /// Tira do catálogo **e do disco** as fotos de uma sessão — o "Descartar"
    /// do rascunho. Os arquivos são as cópias da importação, nunca o cartão.
    fn apagar_da_sessao(&self, sessao: String, canal: Sender<Result<usize, String>>) {
        let _ = sessao;
        let _ = canal.send(Err("este acervo não apaga".into()));
    }

    /// 🧹 Quanto a limpeza por período levaria das **fotos locais que já
    /// estão no site** (e do espelho), para cada período — o número que a aba
    /// Cache mostra antes do clique. Ver `infrastructure::fotos_locais`.
    ///
    /// Sem peça ligada, responde que não há nada: a tela segue só com o cache.
    fn medir_fotos_locais(&self, periodos: Vec<u32>, canal: Sender<Vec<(u32, Retrato)>>) {
        let _ = canal.send(
            periodos
                .into_iter()
                .map(|d| (d, Retrato::default()))
                .collect(),
        );
    }

    /// 🧹 Apaga as fotos locais sem uso há mais de `dias` que o site já guarda.
    fn limpar_fotos_locais(&self, dias: u32, canal: Sender<Result<Retrato, String>>) {
        let _ = dias;
        let _ = canal.send(Ok(Retrato::default()));
    }
}

pub use infrastructure::fotos_locais::Retrato;

/// O acervo de verdade: o `LibraryController` numa `Handle` do tokio.
pub struct AcervoDoBanco {
    biblioteca: Arc<LibraryController>,
    /// ⚠️ Capturada no `main`, **antes** de `Application::run` tomar a thread.
    /// Um `tokio::spawn` de dentro do GPUI entra em pânico com *there is no
    /// reactor running*.
    tokio: tokio::runtime::Handle,
    /// As fotos locais que a limpeza por período leva (issue #22).
    fotos_locais: Option<Arc<infrastructure::fotos_locais::FotosLocais>>,
}

impl AcervoDoBanco {
    pub fn novo(biblioteca: Arc<LibraryController>, tokio: tokio::runtime::Handle) -> Self {
        Self {
            biblioteca,
            tokio,
            fotos_locais: None,
        }
    }

    pub fn com_fotos_locais(mut self, fotos: infrastructure::fotos_locais::FotosLocais) -> Self {
        self.fotos_locais = Some(Arc::new(fotos));
        self
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

    fn trocar_sessao(&self, de: String, para: String, canal: Sender<Result<usize, String>>) {
        let biblioteca = self.biblioteca.clone();
        self.tokio.spawn(async move {
            let _ = canal.send(biblioteca.trocar_sessao(&de, &para).await);
        });
    }

    fn apagar_da_sessao(&self, sessao: String, canal: Sender<Result<usize, String>>) {
        let biblioteca = self.biblioteca.clone();
        self.tokio.spawn(async move {
            let resultado = biblioteca.apagar_da_sessao(&sessao).await.map(|caminhos| {
                // 🔑 Só as cópias que a importação fez para a pasta do ensaio: o
                // arquivo de origem (cartão, pasta do Lightroom) nunca é deste
                // catálogo, porque a sessão importa sempre copiando.
                for caminho in &caminhos {
                    let _ = std::fs::remove_file(caminho);
                }
                caminhos.len()
            });
            let _ = canal.send(resultado);
        });
    }

    fn medir_fotos_locais(&self, periodos: Vec<u32>, canal: Sender<Vec<(u32, Retrato)>>) {
        let Some(fotos) = self.fotos_locais.clone() else {
            let _ = canal.send(
                periodos
                    .into_iter()
                    .map(|d| (d, Retrato::default()))
                    .collect(),
            );
            return;
        };
        self.tokio.spawn(async move {
            let mut medidas = Vec::with_capacity(periodos.len());
            for dias in periodos {
                // Uma medida que falha conta como nada a limpar: o botão apaga,
                // e não promete o que não mediu.
                medidas.push((dias, fotos.sem_uso_ha(dias).await.unwrap_or_default()));
            }
            let _ = canal.send(medidas);
        });
    }

    fn limpar_fotos_locais(&self, dias: u32, canal: Sender<Result<Retrato, String>>) {
        let Some(fotos) = self.fotos_locais.clone() else {
            let _ = canal.send(Ok(Retrato::default()));
            return;
        };
        self.tokio.spawn(async move {
            let _ = canal.send(fotos.limpar_sem_uso_ha(dias).await);
        });
    }
}

/// O acervo dos testes: responde o que lhe mandarem, e conta quantas vezes.
#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    /// Quem espera a resposta da troca, e quantas ela mudou.
    pub type TrocaSegurada = (Sender<Result<usize, String>>, usize);

    #[derive(Default)]
    pub struct AcervoDeMentira {
        pub fotos: Mutex<Vec<PhotoViewModel>>,
        pub pedidos: Mutex<usize>,
        /// Uma leitura que começou antes da última gravação e terminou
        /// depois dela: a próxima releitura devolve este retrato, uma vez.
        pub leitura_atrasada: Mutex<Option<Vec<PhotoViewModel>>>,
        /// A troca acontece na hora, mas a resposta espera
        /// [`AcervoDeMentira::responder_as_trocas`] — o `UPDATE` que já rodou no
        /// banco enquanto a tela ainda não soube, e a cópia segue gravando.
        pub segurar_trocas: std::sync::atomic::AtomicBool,
        pub trocas_seguradas: Mutex<Vec<TrocaSegurada>>,
    }

    impl AcervoDeMentira {
        /// Põe no catálogo uma foto que "acabou de ser importada" deste
        /// caminho — é o que o resgate procura depois que o importador termina.
        pub fn acrescentar_do_caminho(&self, id: &str, caminho: &str) {
            let foto = PhotoViewModel {
                id: id.to_string(),
                path: caminho.to_string(),
                name: std::path::Path::new(caminho)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                ..Default::default()
            };
            self.fotos.lock().expect("as fotos").push(foto);
        }

        pub fn responder_as_trocas(&self) {
            for (canal, trocadas) in self.trocas_seguradas.lock().expect("as trocas").drain(..) {
                let _ = canal.send(Ok(trocadas));
            }
        }

        fn mudar_sessao(&self, de: &str, para: Option<&str>) -> usize {
            let mut fotos = self.fotos.lock().expect("as fotos");
            let antes = fotos.len();
            let mut trocadas = 0;
            match para {
                Some(para) => {
                    for foto in fotos
                        .iter_mut()
                        .filter(|f| f.sessao_id.as_deref() == Some(de))
                    {
                        foto.sessao_id = Some(para.to_string());
                        trocadas += 1;
                    }
                }
                None => {
                    fotos.retain(|f| f.sessao_id.as_deref() != Some(de));
                    trocadas = antes - fotos.len();
                }
            }
            trocadas
        }
    }

    impl Acervo for AcervoDeMentira {
        fn recarregar(&self, canal: Sender<Vec<PhotoViewModel>>) {
            *self.pedidos.lock().expect("os pedidos") += 1;
            if let Some(velhas) = self.leitura_atrasada.lock().expect("o atraso").take() {
                let _ = canal.send(velhas);
                return;
            }
            let fotos = self.fotos.lock().expect("as fotos").clone();
            let _ = canal.send(fotos);
        }

        fn trocar_sessao(&self, de: String, para: String, canal: Sender<Result<usize, String>>) {
            let trocadas = self.mudar_sessao(&de, Some(&para));
            if self
                .segurar_trocas
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                self.trocas_seguradas
                    .lock()
                    .expect("as trocas")
                    .push((canal, trocadas));
                return;
            }
            let _ = canal.send(Ok(trocadas));
        }

        fn apagar_da_sessao(&self, sessao: String, canal: Sender<Result<usize, String>>) {
            let _ = canal.send(Ok(self.mudar_sessao(&sessao, None)));
        }
    }
}
