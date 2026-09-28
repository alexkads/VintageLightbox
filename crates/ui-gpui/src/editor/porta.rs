//! A porta das edições em camadas: o que a Revelação pergunta (qual imagem
//! editada vale para esta foto) e o que a janela do editor pede (abrir e salvar
//! o projeto).
//!
//! 🔑 **O padrão das portas da casa** (`CLAUDE.md`, "Portas para o mundo
//! assíncrono"): a consulta da Revelação é síncrona e responde de um **espelho
//! em memória** do catálogo, carregado no `main`; abrir e salvar são
//! bloqueantes e rodam no executor de fundo do GPUI — nunca na thread que
//! desenha. E há uma versão de mentira (`mod mentira`) para os testes afirmarem
//! o que foi salvo sem banco.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use adapters::view_models::PhotoViewModel;
use editor_core::projeto::{DiscoReal, ErroDoProjeto, Projeto};
use editor_core::{Documento, Historico, VersaoEditada};
use image::RgbImage;
use infrastructure::database::{CatalogoDeEdicoes, EdicaoRegistrada};

/// A foto que o editor abriu — o que ele precisa saber dela, sem o resto do
/// `PhotoViewModel`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FotoDoEditor {
    /// O id do catálogo (`site:<id>` para a foto que só existe no site).
    pub id: String,
    pub pos_venda_foto_id: Option<String>,
    pub nome: String,
    /// O arquivo no disco; vazio na foto do site.
    pub caminho: String,
}

impl FotoDoEditor {
    pub fn da(foto: &PhotoViewModel) -> Self {
        Self {
            id: foto.id.clone(),
            pos_venda_foto_id: foto.pos_venda_foto_id.clone(),
            nome: foto.name.clone(),
            caminho: foto.path.clone(),
        }
    }

    /// A chave com que a raiz lembra a janela aberta desta foto: o id do site
    /// quando há — a mesma foto aparece com ids diferentes no catálogo e na
    /// sessão.
    pub fn chave(&self) -> String {
        self.pos_venda_foto_id
            .clone()
            .map(|pv| format!("site:{pv}"))
            .unwrap_or_else(|| self.id.clone())
    }
}

/// O que a abertura do projeto encontrou.
pub enum Abertura {
    /// Nunca editada: um documento novo sobre a base.
    Nova,
    /// O projeto salvo, intacto.
    Existente {
        documento: Documento,
        historico: Historico,
    },
}

pub trait Edicoes: Send + Sync + 'static {
    /// A imagem editada vigente da foto — **síncrona**, do espelho em memória:
    /// quem pergunta é a Revelação, no meio de um quadro.
    fn versao_de(&self, foto_id: &str, pos_venda_foto_id: Option<&str>) -> Option<VersaoEditada>;

    /// Reabre o projeto da foto sobre a base neutra. **Bloqueante** — só no
    /// executor de fundo.
    fn abrir(&self, foto: &FotoDoEditor, base: &RgbImage) -> Result<Abertura, String>;

    /// Grava o projeto, confirma no catálogo e coleta o que sobrou.
    /// **Bloqueante**. Devolve a imagem editada nova, ou `None` quando o
    /// documento não muda a foto (C30).
    ///
    /// 🚨 Um `Err` quer dizer que a Revelação **continua** com a versão de
    /// antes (C33) — é isso que a janela diz ao operador.
    fn salvar(
        &self,
        foto: &FotoDoEditor,
        base: &RgbImage,
        documento: &Documento,
        historico: &Historico,
    ) -> Result<Option<VersaoEditada>, String>;
}

/// A porta do app, para quem mora numa thread própria e não recebe portas pela
/// raiz — a receita padrão da sessão (`sessoes/receita_padrao.rs`).
///
/// ⚠️ **Só o `main` a define.** Os testes montam as portas deles, e uma global
/// definida por um teste vazaria para os outros.
static DO_APP: std::sync::RwLock<Option<Arc<dyn Edicoes>>> = std::sync::RwLock::new(None);

pub fn definir_as_do_app(edicoes: Arc<dyn Edicoes>) {
    if let Ok(mut global) = DO_APP.write() {
        *global = Some(edicoes);
    }
}

pub fn as_do_app() -> Option<Arc<dyn Edicoes>> {
    DO_APP.read().ok()?.clone()
}

/// A versão de uma foto, a partir do `PhotoViewModel`.
pub fn versao_da_foto(edicoes: &dyn Edicoes, foto: &PhotoViewModel) -> Option<VersaoEditada> {
    edicoes.versao_de(&foto.id, foto.pos_venda_foto_id.as_deref())
}

// ------------------------------------------------------------ a de verdade

/// A porta de verdade: o catálogo SQLite (`edicoes_de_foto`) e os projetos em
/// `<catálogo>/edicoes/<id>/`.
pub struct EdicoesDoCatalogo {
    raiz: PathBuf,
    catalogo: CatalogoDeEdicoes,
    tokio: tokio::runtime::Handle,
    espelho: Mutex<Vec<EdicaoRegistrada>>,
    /// Um `Projeto` por edição, para o hash dos tiles já gravados não ser
    /// recalculado a cada salvamento.
    projetos: Mutex<HashMap<String, Arc<Projeto>>>,
}

impl EdicoesDoCatalogo {
    /// Lê o catálogo e **reconcilia** cada edição com o manifesto dela: a
    /// gravação que caiu entre o manifesto e o banco é adotada agora
    /// (`docs/editor-em-camadas/03-GRAVACAO-E-CATALOGO.md`).
    ///
    /// Chamado no `main` (que é `async`), antes de a janela existir.
    ///
    /// ⚠️ `salvar` usa `block_on` — ele roda no executor de fundo do GPUI, fora
    /// do runtime do tokio. Aqui, dentro do runtime, é `await`.
    pub async fn carregar(
        raiz: PathBuf,
        catalogo: CatalogoDeEdicoes,
        tokio: tokio::runtime::Handle,
    ) -> Self {
        let mut linhas = catalogo.todas().await.unwrap_or_else(|erro| {
            crate::telemetria::avisar!("⚠️ [Editor] {erro}");
            Vec::new()
        });
        for linha in &mut linhas {
            let projeto = Projeto::novo(linha.pasta(&raiz), Arc::new(DiscoReal));
            if let Ok(Some((revisao, versao))) = projeto.a_adotar(&linha.edicao_id, linha.revisao) {
                if catalogo
                    .confirmar(&linha.edicao_id, revisao, versao.as_ref())
                    .await
                    .is_ok()
                {
                    crate::telemetria::avisar!(
                        "🖌️ [Editor] {} retomou a revisão {revisao}, gravada antes de uma interrupção",
                        linha.edicao_id
                    );
                    aplicar(linha, revisao, versao.as_ref());
                    let _ = projeto.coletar(revisao);
                }
            }
        }
        Self {
            raiz,
            catalogo,
            tokio,
            espelho: Mutex::new(linhas),
            projetos: Mutex::new(HashMap::new()),
        }
    }

    fn linha_de(&self, foto_id: &str, pos_venda: Option<&str>) -> Option<EdicaoRegistrada> {
        self.espelho
            .lock()
            .ok()?
            .iter()
            .find(|l| l.e_da_foto(foto_id, pos_venda))
            .cloned()
    }

    fn projeto(&self, linha: &EdicaoRegistrada) -> Arc<Projeto> {
        let mut projetos = self.projetos.lock().unwrap_or_else(|e| e.into_inner());
        projetos
            .entry(linha.edicao_id.clone())
            .or_insert_with(|| {
                Arc::new(Projeto::novo(linha.pasta(&self.raiz), Arc::new(DiscoReal)))
            })
            .clone()
    }

    fn guardar_no_espelho(&self, linha: EdicaoRegistrada) {
        let mut espelho = self.espelho.lock().unwrap_or_else(|e| e.into_inner());
        match espelho.iter_mut().find(|l| l.edicao_id == linha.edicao_id) {
            Some(velha) => *velha = linha,
            None => espelho.push(linha),
        }
    }
}

fn aplicar(linha: &mut EdicaoRegistrada, revisao: u64, versao: Option<&VersaoEditada>) {
    linha.revisao = revisao;
    linha.ativa = versao.is_some();
    linha.arquivo = versao.and_then(|v| {
        v.arquivo
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
    });
    linha.sha256 = versao.map(|v| v.sha256.clone());
    linha.largura = versao.map(|v| v.largura);
    linha.altura = versao.map(|v| v.altura);
}

fn texto(erro: ErroDoProjeto) -> String {
    erro.to_string()
}

impl Edicoes for EdicoesDoCatalogo {
    fn versao_de(&self, foto_id: &str, pos_venda_foto_id: Option<&str>) -> Option<VersaoEditada> {
        self.linha_de(foto_id, pos_venda_foto_id)?
            .versao(&self.raiz)
    }

    fn abrir(&self, foto: &FotoDoEditor, base: &RgbImage) -> Result<Abertura, String> {
        let Some(linha) = self.linha_de(&foto.id, foto.pos_venda_foto_id.as_deref()) else {
            return Ok(Abertura::Nova);
        };
        match self.projeto(&linha).abrir(base).map_err(texto)? {
            Some(aberto) => Ok(Abertura::Existente {
                documento: aberto.documento,
                historico: aberto.historico,
            }),
            None => Ok(Abertura::Nova),
        }
    }

    fn salvar(
        &self,
        foto: &FotoDoEditor,
        base: &RgbImage,
        documento: &Documento,
        historico: &Historico,
    ) -> Result<Option<VersaoEditada>, String> {
        let mut linha = match self.linha_de(&foto.id, foto.pos_venda_foto_id.as_deref()) {
            Some(linha) => linha,
            None => {
                let edicao_id = uuid::Uuid::new_v4().to_string();
                let nova = EdicaoRegistrada {
                    diretorio: format!("edicoes/{edicao_id}"),
                    edicao_id,
                    foto_id: (!foto
                        .id
                        .starts_with(crate::revelacao::persistencia::PREFIXO_DO_SITE))
                    .then(|| foto.id.clone()),
                    pos_venda_foto_id: foto.pos_venda_foto_id.clone(),
                    revisao: 0,
                    arquivo: None,
                    sha256: None,
                    largura: None,
                    altura: None,
                    base_sha256: documento.base.sha256.clone(),
                    ativa: false,
                };
                self.tokio.block_on(self.catalogo.criar(&nova))?;
                self.guardar_no_espelho(nova.clone());
                nova
            }
        };
        if linha.pos_venda_foto_id.is_none() {
            if let Some(pv) = &foto.pos_venda_foto_id {
                self.tokio
                    .block_on(self.catalogo.ligar_ao_site(&linha.edicao_id, pv))?;
                linha.pos_venda_foto_id = Some(pv.clone());
            }
        }

        let projeto = self.projeto(&linha);
        let no_manifesto = projeto
            .manifesto()
            .ok()
            .flatten()
            .map(|m| m.revisao)
            .unwrap_or(0);
        let revisao = linha.revisao.max(no_manifesto) + 1;
        let salvo = projeto
            .salvar(&linha.edicao_id, base, documento, historico, revisao)
            .map_err(|e| format!("{} — a versão anterior continua valendo", texto(e)))?;
        self.tokio
            .block_on(
                self.catalogo
                    .confirmar(&linha.edicao_id, revisao, salvo.versao.as_ref()),
            )
            .map_err(|e| format!("{e} — a versão anterior continua valendo"))?;
        aplicar(&mut linha, revisao, salvo.versao.as_ref());
        self.guardar_no_espelho(linha);
        if let Err(erro) = projeto.coletar(revisao) {
            // A coleta é faxina: falhar nela não desfaz o que foi salvo.
            crate::telemetria::avisar!("⚠️ [Editor] a faxina do projeto falhou: {erro}");
        }
        Ok(salvo.versao)
    }
}

// --------------------------------------------------------------- mentira

#[cfg(test)]
pub mod mentira {
    use super::*;

    /// O que a mentira lembra de cada foto: documento, histórico, versão e revisão.
    type Guardado = (Documento, Historico, Option<VersaoEditada>, u64);

    /// As edições em memória, com os projetos numa pasta temporária — para a
    /// Revelação ler a imagem editada de verdade.
    pub struct EdicoesDeMentira {
        pasta: tempfile::TempDir,
        estado: Mutex<HashMap<String, Guardado>>,
        /// Faz o próximo `salvar` falhar, como um disco cheio.
        pub falhar: std::sync::atomic::AtomicBool,
        pub salvamentos: std::sync::atomic::AtomicUsize,
    }

    impl Default for EdicoesDeMentira {
        fn default() -> Self {
            Self {
                pasta: tempfile::tempdir().unwrap(),
                estado: Mutex::new(HashMap::new()),
                falhar: Default::default(),
                salvamentos: Default::default(),
            }
        }
    }

    impl EdicoesDeMentira {
        fn chave(foto_id: &str, pos_venda: Option<&str>) -> String {
            pos_venda
                .map(|pv| format!("site:{pv}"))
                .unwrap_or_else(|| foto_id.to_string())
        }

        /// O documento salvo da foto, se houver.
        pub fn documento_de(&self, chave: &str) -> Option<Documento> {
            self.estado.lock().unwrap().get(chave).map(|e| e.0.clone())
        }
    }

    impl Edicoes for EdicoesDeMentira {
        fn versao_de(&self, foto_id: &str, pos_venda: Option<&str>) -> Option<VersaoEditada> {
            self.estado
                .lock()
                .unwrap()
                .get(&Self::chave(foto_id, pos_venda))
                .and_then(|e| e.2.clone())
        }

        fn abrir(&self, foto: &FotoDoEditor, _base: &RgbImage) -> Result<Abertura, String> {
            Ok(
                match self.estado.lock().unwrap().get(&foto.chave()).cloned() {
                    Some((documento, historico, _, _)) => Abertura::Existente {
                        documento,
                        historico,
                    },
                    None => Abertura::Nova,
                },
            )
        }

        fn salvar(
            &self,
            foto: &FotoDoEditor,
            base: &RgbImage,
            documento: &Documento,
            historico: &Historico,
        ) -> Result<Option<VersaoEditada>, String> {
            if self.falhar.swap(false, std::sync::atomic::Ordering::SeqCst) {
                return Err("disco cheio — a versão anterior continua valendo".into());
            }
            self.salvamentos
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let chave = foto.chave();
            let revisao = self
                .estado
                .lock()
                .unwrap()
                .get(&chave)
                .map(|e| e.3)
                .unwrap_or(0)
                + 1;
            let projeto = Projeto::novo(
                self.pasta.path().join(chave.replace(':', "_")),
                Arc::new(DiscoReal),
            );
            let salvo = projeto
                .salvar(&chave, base, documento, historico, revisao)
                .map_err(texto)?;
            let mut salvo_hist = historico.clone();
            salvo_hist.marcar_salvo();
            self.estado.lock().unwrap().insert(
                chave,
                (documento.clone(), salvo_hist, salvo.versao.clone(), revisao),
            );
            Ok(salvo.versao)
        }
    }
}
