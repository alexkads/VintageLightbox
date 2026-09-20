//! Escolher pasta ou arquivos **sem arrastar**.
//!
//! # Por que existe
//!
//! 🔑 *"Tinha que ter opção sem arrastar e soltar pra fazer backup"* — dono,
//! 2026-09-19. Arrastar é o gesto rápido de quem já está com o Finder aberto;
//! quem chegou pelo menu não tem de onde arrastar, quem usa teclado não
//! arrasta, e numa máquina com um monitor só a janela do app costuma estar
//! cobrindo justamente a pasta que se quer soltar.
//!
//! O site já tinha os dois botões desde o começo (`webkitdirectory` e `multiple`
//! no `tela.tsx`); esta é a paridade que faltava no app.
//!
//! ⚠️ **Porta própria, e não o `SeletorDeFotos` da sessão.** Aquele filtra por
//! extensão de foto, que é o certo para escolher as fotos de um ensaio e o
//! errado aqui: um backup guarda contrato em PDF, recibo, planilha e o `.NEF`
//! junto. Reusá-lo faria a tela recusar em silêncio metade do que o dono
//! escolhesse.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Abre a janela **do sistema** para escolher o que vai para o acervo.
pub trait EscolhaDoBackup: Send + Sync + 'static {
    /// Uma pasta, com tudo dentro dela.
    ///
    /// **Responde sempre** — lista vazia é desistência. Silêncio deixaria a
    /// tela esperando para sempre por uma pasta que não vem.
    fn escolher_pasta(&self, canal: Sender<Vec<PathBuf>>);

    /// Arquivos avulsos, de qualquer tipo.
    fn escolher_arquivos(&self, canal: Sender<Vec<PathBuf>>);

    /// Onde gravar o zip da pasta. Lista vazia é desistência.
    fn escolher_destino_do_zip(&self, nome: String, canal: Sender<Vec<PathBuf>>);
}

/// A janela do sistema, via `rfd` — a mesma dos outros seletores do app.
///
/// ⚠️ **Janela do sistema, e não do framework**: um seletor desenhado pelo GPUI
/// disputa camada com o modal e aparece escurecido e sem responder ao clique.
/// É a mesma decisão de `importacao::explorador::SeletorNativo`.
pub struct EscolhaNativa {
    tokio: tokio::runtime::Handle,
}

impl EscolhaNativa {
    pub fn nova(tokio: tokio::runtime::Handle) -> Self {
        Self { tokio }
    }
}

impl EscolhaDoBackup for EscolhaNativa {
    fn escolher_pasta(&self, canal: Sender<Vec<PathBuf>>) {
        self.tokio.spawn(async move {
            let escolhida = rfd::AsyncFileDialog::new()
                .set_title("Escolher a pasta para o backup")
                .pick_folder()
                .await
                .map(|pasta| vec![pasta.path().to_path_buf()])
                .unwrap_or_default();
            let _ = canal.send(escolhida);
        });
    }

    fn escolher_arquivos(&self, canal: Sender<Vec<PathBuf>>) {
        self.tokio.spawn(async move {
            let escolhidos = rfd::AsyncFileDialog::new()
                .set_title("Escolher os arquivos para o backup")
                .pick_files()
                .await
                .map(|arquivos| arquivos.iter().map(|a| a.path().to_path_buf()).collect())
                .unwrap_or_default();
            let _ = canal.send(escolhidos);
        });
    }

    fn escolher_destino_do_zip(&self, nome: String, canal: Sender<Vec<PathBuf>>) {
        self.tokio.spawn(async move {
            let destino = rfd::AsyncFileDialog::new()
                .set_title("Salvar a pasta compactada")
                .set_file_name(&nome)
                .add_filter("Arquivo ZIP", &["zip"])
                .save_file()
                .await
                .map(|arquivo| vec![arquivo.path().to_path_buf()])
                .unwrap_or_default();
            let _ = canal.send(destino);
        });
    }
}

/// A escolha dos testes: responde o que lhe mandarem responder.
#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct EscolhaDeMentira {
        /// O que o seletor "devolve". Vazio é desistência, como no de verdade.
        pub resposta: Mutex<Vec<PathBuf>>,
        pub pediram_pasta: Mutex<usize>,
        pub pediram_arquivos: Mutex<usize>,
        /// O destino do zip, e o nome que a tela sugeriu.
        pub destino: Mutex<Vec<PathBuf>>,
        pub nome_pedido: Mutex<Option<String>>,
    }

    impl EscolhaDoBackup for EscolhaDeMentira {
        fn escolher_pasta(&self, canal: Sender<Vec<PathBuf>>) {
            *self.pediram_pasta.lock().unwrap() += 1;
            let _ = canal.send(self.resposta.lock().unwrap().clone());
        }

        fn escolher_arquivos(&self, canal: Sender<Vec<PathBuf>>) {
            *self.pediram_arquivos.lock().unwrap() += 1;
            let _ = canal.send(self.resposta.lock().unwrap().clone());
        }

        fn escolher_destino_do_zip(&self, nome: String, canal: Sender<Vec<PathBuf>>) {
            *self.nome_pedido.lock().unwrap() = Some(nome);
            let _ = canal.send(self.destino.lock().unwrap().clone());
        }
    }
}
