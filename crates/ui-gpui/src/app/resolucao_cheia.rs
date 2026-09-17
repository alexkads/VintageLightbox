//! Os downloads da Revelação que não são envio: o bruto da foto aberta (para
//! medir o lado dele e para o zoom em resolução cheia), o "Baixar JPEG" e a
//! cópia de trabalho do passo 11 (a foto que só existe no site).
//!
//! 🔑 **Canal próprio**, e não o das sincronias: aquele conta "Subindo" na
//! bandeja, "N envios na fila" no canto e segura o G9, e baixar não é enviar
//! ([`PedidoDeFoto::natureza`]). A cópia de trabalho passava por lá até
//! 17/set/2026, e a bandeja dizia "Subindo" enquanto a tira baixava.
//!
//! O lado do bruto é **lembrado entre aberturas** (`lados-dos-originais.json`,
//! ao lado do catálogo), como o `ladoOriginalDaFoto` do site: a rota do
//! original baixa o arquivo inteiro, e medir de novo a cada abertura custaria
//! um original por foto.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use gpui::{Context, Task};

use super::Aplicativo;
use crate::pos_venda::porta::{PedidoDeFoto, Recado};
use domain::services::pos_venda::Sessao;

pub(super) struct Baixas {
    canal: (Sender<Recado>, Receiver<Recado>),
    /// Só as cópias de trabalho. Separadas do resto porque o `Falhou` não diz
    /// de quem é: no canal de cima ele desliga o "Gerando JPEG", e uma cópia
    /// que falhasse não pode fazer isso com um JPEG ainda a caminho.
    copias: (Sender<Recado>, Receiver<Recado>),
    /// Respostas esperadas, dos dois canais.
    pendentes: usize,
    _laco: Option<Task<()>>,
    /// O maior lado do bruto, por id do site.
    lados: HashMap<String, f32>,
    arquivo: Option<PathBuf>,
    /// O último bruto baixado — o zoom quase sempre pede o da foto medida.
    em_maos: Option<(String, Arc<Vec<u8>>)>,
    medindo: HashSet<String>,
    /// A foto (id do site) cujo bruto a tela pediu.
    quer_bruto: Option<String>,
    _decodificando: Option<Task<()>>,
    /// Onde o "Baixar JPEG" grava. `None` é a pasta Downloads de quem usa.
    ///
    /// 🚨 **Nos testes é uma pasta temporária por tela**: até 2026-09-17 o
    /// caminho era sempre o `download_dir()`, e um teste que recebesse o JPEG
    /// escreveria na pasta Downloads de quem roda a suíte.
    downloads: Option<PathBuf>,
}

impl Baixas {
    pub(super) fn nova() -> Self {
        // Os testes nunca tocam o arquivo de quem trabalha.
        let arquivo = (!cfg!(test)).then(|| {
            infrastructure::paths::AppPaths::catalog_root().join("lados-dos-originais.json")
        });
        let lados = arquivo
            .as_ref()
            .and_then(|a| std::fs::read(a).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            canal: channel(),
            copias: channel(),
            pendentes: 0,
            _laco: None,
            lados,
            arquivo,
            em_maos: None,
            medindo: HashSet::new(),
            quer_bruto: None,
            _decodificando: None,
            downloads: downloads_de_teste(),
        }
    }

    /// A pasta do "Baixar JPEG": a Downloads do sistema (ou a temporária, se
    /// ele não disser qual é); nos testes, a de [`downloads_de_teste`].
    pub(super) fn pasta_dos_downloads(&self) -> PathBuf {
        if let Some(pasta) = &self.downloads {
            let _ = std::fs::create_dir_all(pasta);
            return pasta.clone();
        }
        directories::UserDirs::new()
            .and_then(|d| d.download_dir().map(std::path::Path::to_path_buf))
            .unwrap_or_else(std::env::temp_dir)
    }

    pub(super) fn canal(&self) -> Sender<Recado> {
        self.canal.0.clone()
    }

    fn lembrar(&mut self, no_site: &str, lado: f32) {
        self.lados.insert(no_site.to_string(), lado);
        if let Some(arquivo) = &self.arquivo {
            if let Ok(json) = serde_json::to_vec(&self.lados) {
                let _ = std::fs::write(arquivo, json);
            }
        }
    }
}

#[cfg(not(test))]
fn downloads_de_teste() -> Option<PathBuf> {
    None
}

/// Uma pasta temporária por tela: testes em paralelo não disputam o nome do
/// arquivo, e nenhum deles toca a pasta Downloads de quem roda a suíte.
#[cfg(test)]
fn downloads_de_teste() -> Option<PathBuf> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PROXIMA: AtomicUsize = AtomicUsize::new(0);
    Some(std::env::temp_dir().join(format!(
        "vlb-downloads-teste-{}-{}",
        std::process::id(),
        PROXIMA.fetch_add(1, Ordering::SeqCst)
    )))
}

/// O maior lado de uma imagem, lendo só o cabeçalho.
fn lado_de(bytes: &[u8]) -> Option<f32> {
    image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
        .map(|(l, a)| l.max(a) as f32)
}

impl Aplicativo {
    /// A foto aberta, e o id dela no site quando ela só existe lá.
    fn aberta_para_o_bruto(&self, cx: &Context<Self>) -> Option<(String, Option<String>, String)> {
        let foto = self.revelacao.read(cx).foto_aberta()?.clone();
        Some((foto.id, foto.pos_venda_foto_id, foto.path))
    }

    /// Outra foto no palco: mede o bruto dela (do disco, ou do site).
    pub(super) fn medir_o_original(&mut self, cx: &mut Context<Self>) {
        let Some((id, no_site, caminho)) = self.aberta_para_o_bruto(cx) else {
            return;
        };
        if !caminho.is_empty() {
            if let Ok((l, a)) = image::image_dimensions(&caminho) {
                let lado = l.max(a) as f32;
                self.revelacao
                    .update(cx, |tela, cx| tela.definir_lado_do_bruto(&id, lado, cx));
            }
            return;
        }
        let Some(no_site) = no_site else {
            return;
        };
        if let Some(lado) = self.baixas.lados.get(&no_site).copied() {
            self.revelacao
                .update(cx, |tela, cx| tela.definir_lado_do_bruto(&id, lado, cx));
            return;
        }
        if self.baixas.medindo.contains(&no_site) {
            return;
        }
        let Some(sessao) = self.sessao().cloned() else {
            return;
        };
        self.baixas.medindo.insert(no_site.clone());
        self.publicador
            .original(sessao, no_site, self.baixas.canal());
        self.esperar_as_baixas(1, cx);
    }

    /// A tela pediu o bruto da foto aberta.
    pub(super) fn pedir_o_bruto(&mut self, cx: &mut Context<Self>) {
        let Some((id, no_site, caminho)) = self.aberta_para_o_bruto(cx) else {
            return;
        };
        if !caminho.is_empty() {
            let arquivo = PathBuf::from(caminho);
            self.decodificar_o_bruto(
                id,
                move || image::open(&arquivo).map_err(|e| e.to_string()),
                cx,
            );
            return;
        }
        let Some(no_site) = no_site else {
            self.revelacao
                .update(cx, |tela, cx| tela.bruto_indisponivel(&id, cx));
            return;
        };
        if let Some((ja, bytes)) = &self.baixas.em_maos {
            if *ja == no_site {
                let bytes = bytes.clone();
                self.decodificar_o_bruto(
                    id,
                    move || image::load_from_memory(&bytes).map_err(|e| e.to_string()),
                    cx,
                );
                return;
            }
        }
        let Some(sessao) = self.sessao().cloned() else {
            return;
        };
        self.baixas.quer_bruto = Some(no_site.clone());
        if self.baixas.medindo.insert(no_site.clone()) {
            self.publicador
                .original(sessao, no_site, self.baixas.canal());
            self.esperar_as_baixas(1, cx);
        }
    }

    /// Decodifica fora da thread da interface — são dezenas de MB.
    fn decodificar_o_bruto(
        &mut self,
        id: String,
        decodificar: impl FnOnce() -> Result<image::DynamicImage, String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let tarefa = cx.background_executor().spawn(async move { decodificar() });
        self.baixas._decodificando = Some(cx.spawn(async move |raiz, cx| {
            let resultado = tarefa.await;
            let _ = raiz.update(cx, |raiz, cx| {
                raiz.revelacao.update(cx, |tela, cx| match resultado {
                    Ok(imagem) => tela.receber_bruto(&id, imagem, cx),
                    Err(_) => tela.bruto_indisponivel(&id, cx),
                });
            });
        }));
    }

    /// O passo 11: pede ao site a cópia de trabalho de uma foto.
    ///
    /// `local` é o id do catálogo, e volta no recado: é por ele que a tela
    /// confere se a foto na frente ainda é a mesma.
    pub(super) fn pedir_a_copia_de_trabalho(
        &mut self,
        sessao: Sessao,
        local: String,
        no_site: String,
        cx: &mut Context<Self>,
    ) {
        self.publicador
            .copia_de_trabalho(sessao, local, no_site, self.baixas.copias.0.clone());
        self.esperar_o_site(PedidoDeFoto::CopiaDeTrabalho, 1, cx);
    }

    /// Quantos downloads ainda não responderam.
    #[cfg(test)]
    pub(crate) fn baixas_pendentes(&self) -> usize {
        self.baixas.pendentes
    }

    /// Espera as respostas dos downloads — uma por pedido, até a última.
    pub(super) fn esperar_as_baixas(&mut self, quantas: usize, cx: &mut Context<Self>) {
        self.baixas.pendentes += quantas;
        if self.baixas._laco.is_some() {
            return;
        }
        self.baixas._laco = Some(cx.spawn(async move |raiz, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let Ok(acabou) = raiz.update(cx, |raiz, cx| {
                raiz.colher_as_baixas(cx);
                let acabou = raiz.baixas.pendentes == 0;
                if acabou {
                    raiz.baixas._laco = None;
                }
                acabou
            }) else {
                return;
            };
            if acabou {
                return;
            }
        }));
    }

    pub(crate) fn colher_as_baixas(&mut self, cx: &mut Context<Self>) {
        while let Ok(recado) = self.baixas.copias.1.try_recv() {
            self.baixas.pendentes = self.baixas.pendentes.saturating_sub(1);
            match recado {
                Recado::Pixels { foto_id, bytes } => {
                    self.receber_a_copia_de_trabalho(foto_id, &bytes, cx)
                }
                // 🔑 **Download que falhou não é recusa do site**: não entra no
                // canto das recusas (que é dos envios, como no Tauri) nem conta
                // como resposta do "Salvar na galeria". Só avisa.
                Recado::Falhou(erro) => self.avisar_onde_esta_olhando(erro, cx),
                _ => {}
            }
        }
        while let Ok(recado) = self.baixas.canal.1.try_recv() {
            self.baixas.pendentes = self.baixas.pendentes.saturating_sub(1);
            match recado {
                Recado::Original {
                    foto_no_site,
                    bytes,
                } => {
                    self.baixas.medindo.remove(&foto_no_site);
                    let aberta = self.aberta_para_o_bruto(cx);
                    if let Some(lado) = lado_de(&bytes) {
                        self.baixas.lembrar(&foto_no_site, lado);
                        if let Some((id, Some(no_site), _)) = &aberta {
                            if *no_site == foto_no_site {
                                let id = id.clone();
                                self.revelacao.update(cx, |tela, cx| {
                                    tela.definir_lado_do_bruto(&id, lado, cx)
                                });
                            }
                        }
                    }
                    self.baixas.em_maos = Some((foto_no_site.clone(), bytes.clone()));
                    if self.baixas.quer_bruto.as_deref() == Some(foto_no_site.as_str()) {
                        self.baixas.quer_bruto = None;
                        if let Some((id, Some(no_site), _)) = aberta {
                            if no_site == foto_no_site {
                                self.decodificar_o_bruto(
                                    id,
                                    move || {
                                        image::load_from_memory(&bytes).map_err(|e| e.to_string())
                                    },
                                    cx,
                                );
                            }
                        }
                    }
                }
                Recado::OriginalIndisponivel { foto_no_site } => {
                    self.baixas.medindo.remove(&foto_no_site);
                    if self.baixas.quer_bruto.as_deref() == Some(foto_no_site.as_str()) {
                        self.baixas.quer_bruto = None;
                        if let Some((id, _, _)) = self.aberta_para_o_bruto(cx) {
                            self.revelacao
                                .update(cx, |tela, cx| tela.bruto_indisponivel(&id, cx));
                        }
                    }
                }
                Recado::JpegRevelado {
                    foto_no_site,
                    bytes,
                } => self.guardar_o_jpeg(&foto_no_site, &bytes, cx),
                Recado::Falhou(erro) => {
                    self.revelacao
                        .update(cx, |tela, cx| tela.definir_gerando_jpeg(false, cx));
                    self.avisar_onde_esta_olhando(erro, cx);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_lado_sai_do_cabecalho() {
        let imagem = image::DynamicImage::new_rgb8(30, 12);
        let mut bytes = Vec::new();
        imagem
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("codificar");
        assert_eq!(lado_de(&bytes), Some(30.));
        assert_eq!(lado_de(b"nada"), None);
    }
}
