//! **Tirar foto do acervo é trazer os bytes para cá primeiro** — a cláusula
//! C21 do contrato da foto, do lado do desktop.
//!
//! # A regra de ouro, e por que ela mora num módulo só dela
//!
//! 🚨 `tirar_do_site` apaga a linha **e** os arquivos do bucket, sem lixeira.
//! O desfazer disso é o catálogo local, e ele só desfaz o que tem: a foto que
//! subiu de outra máquina não existe neste SQLite, e a devolução saía calada.
//! Era a **divergência D14** (`docs/CONTRATO_DA_FOTO.md` §8): no desktop, a
//! tecla `0` numa foto do site era recusada com *"use Apagar"* — e "Apagar"
//! removia da nuvem sem trazer BRUTO nem PARÂMETROS para cá.
//!
//! A ordem é a correção inteira, e ordem escrita no meio de um `match` é ordem
//! que a próxima mão desfaz sem perceber. É o mesmo raciocínio — e o mesmo
//! desenho — do `resgate.ts` da web, que nasceu do dono em 2026-09-11, depois
//! de uma sessão de 8 fotos voltar com 7.
//!
//! # Os quatro passos, por foto
//!
//! 1. **O BRUTO vem para cá** (`Publicador::original`). Os mesmos bytes que
//!    subiram, nunca a versão revelada no lugar deles (C3).
//! 2. **O arquivo é gravado e catalogado** pelo importador, como qualquer foto
//!    do cartão: daí em diante ela é uma foto local, com prévia e linha no
//!    SQLite.
//! 3. **Os PARÂMETROS voltam junto** — os 171 que estavam na nuvem, gravados na
//!    foto nova. Sem isto ela voltaria crua, e a nota seguinte a subiria crua
//!    (é o que a web corrigiu em 2026-09-13, `devolverEdicaoLocal`).
//! 4. **Só então a nuvem perde a foto.** A que falhar em qualquer passo acima
//!    **continua no acervo**, classificada como estava.
//!
//! ⚠️ **Uma de cada vez.** São brutos de 24 MP; baixar dez em paralelo para
//! gravar dez arquivos é o caminho conhecido para a máquina engasgar no meio — e
//! engasgar no meio aqui significa ter removido umas e não outras.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;

use domain::value_objects::{ImportMode, ImportOptions};
use gpui::{AsyncApp, Context};

use super::Aplicativo;
use crate::importacao::explorador::{Andamento, Freios};
use crate::pos_venda::porta::Recado;
use crate::revelacao::persistencia::{self, Corte};
use crate::revelacao::processador::Ajustes;

/// Quanto tempo se espera por cada passo antes de desistir **sem remover nada**.
///
/// 🔑 **Desistir é o desfecho seguro**: a foto continua no acervo, com a nota
/// que tinha. O contrário — remover sem a cópia — é o que custou uma foto na
/// web, duas vezes.
const ESPERA_MAXIMA: Duration = Duration::from_secs(180);
const RESPIRO: Duration = Duration::from_millis(100);

/// Uma foto que o "tirar a nota" mandou voltar para esta máquina.
#[derive(Debug, Clone)]
pub struct AFotoQueVolta {
    /// O id dela no site — é por ele que se baixa e se remove.
    pub no_site: String,
    /// O nome do arquivo, que é o que sobrevive à viagem de ida e volta.
    pub arquivo: String,
    /// Os PARÂMETROS que estavam na nuvem, para voltarem com ela.
    pub ajustes: Ajustes,
    pub corte: Corte,
}

/// O que aconteceu com cada foto — o que a tela conta no fim.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Desfecho {
    /// Os nomes das que voltaram e saíram do acervo.
    pub voltaram: Vec<String>,
    /// Os nomes das que **ficaram** no acervo, por não ter sido possível
    /// trazê-las. É o `avisoDoQueFicou` da web.
    pub ficaram: Vec<String>,
}

impl Desfecho {
    /// As duas frases do fim, como na web: uma por desfecho, e só quando há o
    /// que dizer.
    pub fn avisos(&self) -> (Option<String>, Option<String>) {
        let sucesso = (!self.voltaram.is_empty()).then(|| {
            format!(
                "{} foto(s) voltaram para esta máquina e saíram do acervo",
                self.voltaram.len()
            )
        });
        let falha = (!self.ficaram.is_empty()).then(|| {
            format!(
                "{} foto(s) ficaram no acervo: não deu para trazer o arquivo de volta ({}). \
                 Tirar a nota delas apagaria a foto.",
                self.ficaram.len(),
                self.ficaram.join(" / ")
            )
        });
        (sucesso, falha)
    }
}

impl Aplicativo {
    /// Tira estas fotos do acervo — **depois** de trazer cada uma para cá.
    ///
    /// 🔑 **É a tecla `0` da grade da sessão**, e o gesto que a web chama de
    /// "Tirar do acervo". Quem confirma com o operador é a tela; aqui já se
    /// executa.
    pub fn desclassificar_do_acervo(&mut self, fotos: Vec<AFotoQueVolta>, cx: &mut Context<Self>) {
        if fotos.is_empty() || !self.pode_trabalhar() {
            return;
        }
        let quantas = fotos.len();
        self.avisar_onde_esta_olhando(
            format!("trazendo {quantas} foto(s) de volta para esta máquina…"),
            cx,
        );
        self._resgate = Some(cx.spawn(async move |raiz, cx| {
            let mut desfecho = Desfecho::default();
            for foto in fotos {
                // ⚠️ Uma de cada vez, e a que falha não derruba as outras.
                match resgatar_uma(&raiz, &foto, cx).await {
                    Ok(()) => desfecho.voltaram.push(foto.arquivo.clone()),
                    Err(_) => desfecho.ficaram.push(foto.arquivo.clone()),
                }
            }
            let _ = raiz.update(cx, |raiz, cx| raiz.terminar_o_resgate(desfecho, cx));
        }));
    }

    /// O fim do lote: a grade relê, e o operador ouve o que aconteceu.
    fn terminar_o_resgate(&mut self, desfecho: Desfecho, cx: &mut Context<Self>) {
        let (sucesso, falha) = desfecho.avisos();
        // O catálogo ganhou fotos e o site perdeu: as duas listas mudaram.
        self.reler_o_acervo(cx);
        if let Some(galeria) = self.sessao_aberta.clone() {
            self.detalhe.update(cx, |tela, cx| tela.entrar(galeria, cx));
        }
        if let Some(texto) = sucesso {
            self.avisar_onde_esta_olhando(texto, cx);
        }
        if let Some(texto) = falha {
            self.avisar_falha(texto, cx);
        }
        self.ultimo_resgate = Some(desfecho);
        cx.notify();
    }

    /// A galeria aberta — é nela que a foto que volta entra, como foto local.
    pub(crate) fn sessao_aberta_para_o_resgate(&self) -> Option<String> {
        self.sessao_aberta.clone()
    }

    /// 🧪 O desfecho do último resgate — o que os cenários afirmam.
    #[cfg(test)]
    pub(crate) fn ultimo_resgate(&self) -> Option<Desfecho> {
        self.ultimo_resgate.clone()
    }
}

/// O caminho de volta de **uma** foto. `Err` é "ficou no acervo".
async fn resgatar_uma(
    raiz: &gpui::WeakEntity<Aplicativo>,
    foto: &AFotoQueVolta,
    cx: &mut AsyncApp,
) -> Result<(), ()> {
    let Ok(Some((sessao, galeria))) = raiz.update(cx, |raiz, _| {
        raiz.sessao()
            .cloned()
            .zip(raiz.sessao_aberta_para_o_resgate())
    }) else {
        return Err(());
    };

    // 1 · 🚨 **O BRUTO vem para cá antes de qualquer outra coisa.**
    let (para_o_bruto, do_bruto) = channel::<Recado>();
    let no_site = foto.no_site.clone();
    raiz.update(cx, |raiz, _| {
        raiz.publicador
            .original(sessao.clone(), no_site.clone(), para_o_bruto)
    })
    .map_err(|_| ())?;
    let bytes = match esperar(&do_bruto, cx).await {
        Some(Recado::Original { bytes, .. }) => bytes,
        // `OriginalIndisponivel` é o caso da foto cujo arquivo já não está no
        // storage: sem bytes não há cópia, e sem cópia não se apaga nada.
        _ => return Err(()),
    };

    // 2 · O arquivo, e o catálogo.
    let caminho = gravar_o_arquivo(&foto.arquivo, &bytes, cx).await?;
    let opcoes = ImportOptions {
        // **Add**, e não `Copy`: o arquivo já nasceu no destino, e copiá-lo
        // deixaria dois brutos iguais no disco.
        mode: ImportMode::Add,
        skip_duplicates: false,
        sessao_id: Some(galeria.clone()),
        ..Default::default()
    };
    let (para_o_lote, do_lote) = channel::<Andamento>();
    let arquivo = caminho.to_string_lossy().to_string();
    raiz.update(cx, |raiz, _| {
        raiz.importador.importar(
            vec![arquivo.clone()],
            opcoes,
            Freios::default(),
            para_o_lote,
        )
    })
    .map_err(|_| ())?;
    esperar_o_lote(&do_lote, cx).await?;

    // 3 · Os PARÂMETROS voltam com ela — senão a foto volta crua e a nota
    // seguinte a sobe crua (o defeito que a web pagou em 13/set/2026).
    let id_local = achar_no_catalogo(raiz, &arquivo, cx).await?;
    raiz.update(cx, |raiz, _| {
        raiz.gravador.gravar(id_local, foto.ajustes, foto.corte)
    })
    .map_err(|_| ())?;

    // 4 · **Só agora** a nuvem perde a foto.
    let (para_o_site, do_site) = channel::<Recado>();
    raiz.update(cx, |raiz, _| {
        raiz.publicador
            .tirar_do_site(sessao, foto.no_site.clone(), para_o_site)
    })
    .map_err(|_| ())?;
    match esperar(&do_site, cx).await {
        Some(Recado::Falhou(_)) | None => Err(()),
        Some(_) => {
            // 🔑 **A prévia local da foto que saiu não fica para trás**: o id do
            // site deixou de existir, e o cache sob ele viraria lixo que ninguém
            // atualiza.
            let no_acervo = format!("{}{}", persistencia::PREFIXO_DO_SITE, foto.no_site);
            let _ = raiz.update(cx, |raiz, cx| raiz.esquecer_a_previa_local(&no_acervo, cx));
            Ok(())
        }
    }
}

/// Grava o bruto na pasta das resgatadas, sem repetir nome.
async fn gravar_o_arquivo(
    arquivo: &str,
    bytes: &Arc<Vec<u8>>,
    cx: &mut AsyncApp,
) -> Result<PathBuf, ()> {
    let pasta = pasta_das_resgatadas();
    let nome = arquivo.to_string();
    let bytes = bytes.clone();
    cx.background_executor()
        .spawn(async move {
            std::fs::create_dir_all(&pasta).map_err(|_| ())?;
            let mut destino = pasta.join(&nome);
            // Mesmo nome duas vezes é o caso de quem tira do acervo, classifica
            // de novo e tira outra vez: o segundo arquivo não pode calar o
            // primeiro.
            let mut n = 1;
            while destino.exists() {
                let base = std::path::Path::new(&nome);
                let tronco = base.file_stem().unwrap_or_default().to_string_lossy();
                let extensao = base
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default();
                destino = pasta.join(format!("{tronco} ({n}){extensao}"));
                n += 1;
            }
            std::fs::write(&destino, bytes.as_slice()).map_err(|_| ())?;
            Ok(destino)
        })
        .await
}

/// Onde o bruto que volta é gravado: uma pasta do catálogo, ao lado das outras.
///
/// 🔑 **Pasta própria, e não a da importação**: quem olhar o disco vê de onde
/// aquele arquivo veio, e a próxima importação do cartão não a mistura com o
/// que chegou de volta da nuvem.
#[cfg(not(test))]
pub(crate) fn pasta_das_resgatadas() -> PathBuf {
    infrastructure::paths::AppPaths::catalog_root().join("Resgatadas")
}

/// ⚠️ Em teste a pasta é temporária **e por cenário**: nenhum escreve no
/// catálogo de quem está desenvolvendo, e o nome da thread (que no Rust é o
/// nome do teste) impede que dois cenários em paralelo apaguem o arquivo um do
/// outro.
#[cfg(test)]
pub(crate) fn pasta_das_resgatadas() -> PathBuf {
    let cenario = std::thread::current()
        .name()
        .unwrap_or("sem-nome")
        .replace("::", "-");
    std::env::temp_dir().join(format!(
        "vlb-resgatadas-teste-{}-{cenario}",
        std::process::id()
    ))
}

/// Espera um recado do canal, com teto. `None` é desistência.
async fn esperar(canal: &Receiver<Recado>, cx: &mut AsyncApp) -> Option<Recado> {
    let voltas = (ESPERA_MAXIMA.as_millis() / RESPIRO.as_millis()) as u32;
    for _ in 0..voltas {
        if let Ok(recado) = canal.try_recv() {
            return Some(recado);
        }
        cx.background_executor().timer(RESPIRO).await;
    }
    None
}

/// Espera o lote de importação terminar. `Err` quando ele falha ou some.
async fn esperar_o_lote(canal: &Receiver<Andamento>, cx: &mut AsyncApp) -> Result<(), ()> {
    let voltas = (ESPERA_MAXIMA.as_millis() / RESPIRO.as_millis()) as u32;
    for _ in 0..voltas {
        while let Ok(andamento) = canal.try_recv() {
            match andamento {
                Andamento::Terminou { sucesso, .. } if sucesso > 0 => return Ok(()),
                Andamento::Terminou { .. } | Andamento::Falhou { .. } => return Err(()),
                _ => {}
            }
        }
        cx.background_executor().timer(RESPIRO).await;
    }
    Err(())
}

/// Relê o catálogo e acha a foto que acabou de entrar, pelo caminho do arquivo.
async fn achar_no_catalogo(
    raiz: &gpui::WeakEntity<Aplicativo>,
    arquivo: &str,
    cx: &mut AsyncApp,
) -> Result<String, ()> {
    let (para_o_acervo, do_acervo) = channel::<Vec<adapters::view_models::PhotoViewModel>>();
    raiz.update(cx, |raiz, _| raiz.acervo.recarregar(para_o_acervo))
        .map_err(|_| ())?;
    let voltas = (ESPERA_MAXIMA.as_millis() / RESPIRO.as_millis()) as u32;
    for _ in 0..voltas {
        if let Ok(fotos) = do_acervo.try_recv() {
            return fotos
                .into_iter()
                .find(|f| f.path == arquivo)
                .map(|f| f.id)
                .ok_or(());
        }
        cx.background_executor().timer(RESPIRO).await;
    }
    Err(())
}

/// A foto da grade pode voltar para a máquina?
///
/// 🚨 **As três recusas são as da web**, e cada uma tem dono:
///
/// - **Comprada** não se desclassifica: há cobrança e entrega atrás dela (C21).
/// - **Levada no balcão** não perde a nota (dono, 2026-09-12: *"se eu não posso
///   sinalizar sem classificar, o inverso deve ser protegido"*).
/// - **Apagada** já não está lá.
///
/// As recusadas **não bloqueiam o lote**: as outras seguem, e a tela conta as
/// que ficaram — recusar tudo faria o operador procurar qual foi.
pub fn pode_voltar(estado: biblioteca_core::acervo::Estado, apagada: bool) -> bool {
    use biblioteca_core::acervo::Estado;
    !apagada && !matches!(estado, Estado::Comprada | Estado::LevadaNoBalcao)
}

#[cfg(test)]
mod testes {
    use super::*;
    use biblioteca_core::acervo::Estado;

    /// 🔑 As três recusas, e o que passa.
    #[test]
    fn so_a_que_esta_a_venda_volta_para_a_maquina() {
        assert!(pode_voltar(Estado::Disponivel, false));
        assert!(!pode_voltar(Estado::Comprada, false), "há cobrança atrás");
        assert!(
            !pode_voltar(Estado::LevadaNoBalcao, false),
            "a levada não perde a nota"
        );
        assert!(
            !pode_voltar(Estado::Disponivel, true),
            "apagada já não está"
        );
    }

    /// As duas frases do fim — e o silêncio quando não há o que dizer.
    #[test]
    fn o_aviso_conta_os_dois_lados() {
        let vazio = Desfecho::default();
        assert_eq!(vazio.avisos(), (None, None));

        let misto = Desfecho {
            voltaram: vec!["a.jpg".into(), "b.jpg".into()],
            ficaram: vec!["c.jpg".into()],
        };
        let (sucesso, falha) = misto.avisos();
        assert_eq!(
            sucesso.as_deref(),
            Some("2 foto(s) voltaram para esta máquina e saíram do acervo")
        );
        let falha = falha.expect("a frase do que ficou");
        assert!(falha.contains("c.jpg"), "o nome é o que o operador procura");
        assert!(
            falha.contains("apagaria a foto"),
            "e a frase diz por que elas ficaram: {falha}"
        );
    }
}
