//! Escolher fotos **pelo seletor do sistema** — e não por um explorador nosso.
//!
//! # 🚨 Por que isto existe
//!
//! O app tem um explorador de arquivos próprio, no modal de importação: origens,
//! varredura, grade com caixinhas, painel de destino. Ele existe para a triagem
//! em RAW, onde o operador escolhe entre duzentas do cartão.
//!
//! **Para mandar fotos ao cliente, ele é atrito.** Na web o gesto é um só —
//! arrastar a pasta exportada, ou abrir a janela do sistema — e o dono pediu o
//! mesmo aqui: *"tem que usar o mesmo explorador de arquivos do sistema
//! operacional"*. Quem exportou do Lightroom já está com a pasta aberta ao lado.
//!
//! ⚠️ **Janela do sistema, e não do framework** — a mesma razão que fez o legado
//! trocar o `egui_file` pelo `rfd`: um seletor desenhado pelo framework disputa
//! camada com o modal e aparece escurecido, sem responder ao clique.

use std::sync::mpsc::Sender;

/// Os formatos que um estúdio entrega.
///
/// 📸 São os mesmos que a web aceita (`envio.tsx`), **e mais os RAW**: aqui o
/// preparador do envio abre RAW pela LibRaw, coisa que o navegador não faz. Quem
/// solta um `.NEF` na sessão recebe o JPEG revelado dele — o que o cliente veria
/// de qualquer jeito.
pub const EXTENSOES: [&str; 17] = [
    "jpg", "jpeg", "png", "tif", "tiff", "webp", "avif", "heic", "heif", "bmp", "gif", "nef",
    "cr2", "cr3", "arw", "dng", "raf",
];

pub trait SeletorDeFotos: Send + Sync + 'static {
    /// Abre o seletor do sistema. **Responde sempre** — lista vazia é
    /// desistência, e silêncio deixaria a tela esperando para sempre.
    fn escolher(&self, canal: Sender<Vec<String>>);
}

/// O seletor do sistema, via `rfd`.
pub struct SeletorDeFotosNativo {
    tokio: tokio::runtime::Handle,
}

impl SeletorDeFotosNativo {
    pub fn novo(tokio: tokio::runtime::Handle) -> Self {
        Self { tokio }
    }
}

impl SeletorDeFotos for SeletorDeFotosNativo {
    fn escolher(&self, canal: Sender<Vec<String>>) {
        self.tokio.spawn(async move {
            let escolhidos = rfd::AsyncFileDialog::new()
                .set_title("Escolher as fotos para esta sessão")
                .add_filter("Fotos", &EXTENSOES)
                .pick_files()
                .await
                .map(|arquivos| {
                    arquivos
                        .iter()
                        .map(|a| a.path().to_string_lossy().to_string())
                        .collect()
                })
                .unwrap_or_default();
            let _ = canal.send(escolhidos);
        });
    }
}

/// Fica só com o que parece foto, e ignora o resto do que foi solto.
///
/// 🔑 **Arrastar uma pasta traz a pasta, e não o conteúdo dela** — o sistema
/// entrega o caminho do diretório. Aqui ele é aberto um nível, que é o que uma
/// pasta de exportação do Lightroom tem: arquivos, não subpastas.
pub fn so_as_fotos(caminhos: &[std::path::PathBuf]) -> Vec<String> {
    let mut fotos = Vec::new();
    for caminho in caminhos {
        if caminho.is_dir() {
            let Ok(itens) = std::fs::read_dir(caminho) else {
                continue;
            };
            for item in itens.flatten() {
                let dentro = item.path();
                if e_foto(&dentro) {
                    fotos.push(dentro.to_string_lossy().to_string());
                }
            }
        } else if e_foto(caminho) {
            fotos.push(caminho.to_string_lossy().to_string());
        }
    }
    // Ordem estável: a pasta do sistema não promete nenhuma, e a ordem daqui
    // vira a ordem no site.
    fotos.sort();
    fotos
}

fn e_foto(caminho: &std::path::Path) -> bool {
    caminho
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .is_some_and(|e| EXTENSOES.contains(&e.as_str()))
}

#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    /// Devolve o que o teste mandar, sem abrir janela nenhuma.
    #[derive(Default)]
    pub struct SeletorDeMentira {
        pub escolha: Mutex<Vec<String>>,
        pub pedidos: Mutex<usize>,
        /// Segura a resposta até [`SeletorDeMentira::responder`] — a janela do
        /// sistema que fica aberta enquanto o operador procura a pasta.
        ///
        /// 🚨 **O `Default` responde no mesmo instante, e isso escondeu um
        /// defeito por completo** (8/set/2026): a janela real fica aberta
        /// *segundos*, e nesse tempo a colheita da tela desistia. Com a
        /// resposta imediata não havia esse tempo — nenhum teste podia ver.
        pub demorado: bool,
        pub guardado: Mutex<Option<Sender<Vec<String>>>>,
    }

    impl SeletorDeMentira {
        pub fn escolhe(caminhos: &[&str]) -> Self {
            Self {
                escolha: Mutex::new(caminhos.iter().map(|c| c.to_string()).collect()),
                ..Default::default()
            }
        }

        /// O mesmo, mas só responde quando o teste mandar.
        pub fn demorado(caminhos: &[&str]) -> Self {
            Self {
                demorado: true,
                ..Self::escolhe(caminhos)
            }
        }

        pub fn pedidos(&self) -> usize {
            *self.pedidos.lock().expect("os pedidos")
        }

        /// O operador enfim escolheu, e apertou "Abrir".
        pub fn responder(&self) {
            let canal = self.guardado.lock().expect("o guardado").take();
            if let Some(canal) = canal {
                let _ = canal.send(self.escolha.lock().expect("a escolha").clone());
            }
        }
    }

    impl SeletorDeFotos for SeletorDeMentira {
        fn escolher(&self, canal: Sender<Vec<String>>) {
            *self.pedidos.lock().expect("os pedidos") += 1;
            if self.demorado {
                *self.guardado.lock().expect("o guardado") = Some(canal);
                return;
            }
            let _ = canal.send(self.escolha.lock().expect("a escolha").clone());
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::path::PathBuf;

    /// 🔑 Uma pasta solta vira o conteúdo dela — é o gesto de quem exportou.
    #[test]
    fn a_pasta_solta_vira_as_fotos_de_dentro() {
        let dir = tempfile::TempDir::new().expect("diretório");
        for nome in ["b.jpg", "a.NEF", "leia-me.txt", "c.tif"] {
            std::fs::write(dir.path().join(nome), b"x").expect("gravar");
        }

        let achadas = so_as_fotos(&[dir.path().to_path_buf()]);
        let nomes: Vec<String> = achadas
            .iter()
            .map(|c| {
                PathBuf::from(c)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        assert_eq!(
            nomes,
            vec!["a.NEF".to_string(), "b.jpg".into(), "c.tif".into()],
            "só as fotos, em ordem estável — ela vira a ordem no site"
        );
    }

    /// ⚠️ O que não é foto é ignorado em silêncio.
    ///
    /// Quem arrasta uma seleção do Finder pega o `.DS_Store` junto sem saber.
    /// Recusar o lote inteiro por causa dele seria cobrar do operador uma
    /// limpeza que o sistema fez por ele.
    #[test]
    fn arquivo_que_nao_e_foto_e_ignorado_sem_reclamar() {
        let dir = tempfile::TempDir::new().expect("diretório");
        let foto = dir.path().join("DSC_001.jpg");
        let outro = dir.path().join("recibo.pdf");
        std::fs::write(&foto, b"x").expect("gravar");
        std::fs::write(&outro, b"x").expect("gravar");

        let achadas = so_as_fotos(&[foto.clone(), outro]);
        assert_eq!(achadas, vec![foto.to_string_lossy().to_string()]);
    }
}
