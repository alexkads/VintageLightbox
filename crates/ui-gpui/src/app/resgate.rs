//! ❌ **Rejeitar a foto que já subiu: ela volta para cá e sai da nuvem.**
//!
//! Regra do dono de 2026-09-21: *"o importador envia as fotos em segundo
//! plano; quando o usuário rejeitar, a mesma voltará para o arquivo local e
//! sairá da nuvem. Quase nunca o usuário rejeita uma foto."* Ela substitui a
//! C21 de 2026-09-20, em que a rejeitada só era marcada e ficava na nuvem.
//!
//! # A regra de ouro — a mesma do resgate de 2026-09-18
//!
//! 🚨 **A nuvem só perde a foto depois de ela estar inteira aqui.** Tirar do
//! site apaga a linha **e** os arquivos do bucket, sem lixeira; o desfazer é o
//! catálogo local, e ele só desfaz o que tem. A web pagou duas fotos por
//! esquecer essa ordem. Aqui ela está num módulo só dela porque ordem escrita
//! no meio de um `match` é ordem que a próxima mão desfaz sem perceber.
//!
//! # Os dois caminhos, por foto
//!
//! - **A · a foto tem cópia neste catálogo** — o caso de quase sempre: ela foi
//!   importada aqui e subiu em segundo plano (C20.1, a cópia local continua).
//!   Um pedido só: [`Publicador::rejeitar_tirando_da_nuvem`] tira do site e,
//!   com a resposta, marca a cópia daqui como rejeitada e desliga o id remoto.
//! - **B · não tem** (subiu de outra máquina, ou pelo site): o resgate de
//!   2026-09-18, inteiro — o BRUTO vem para cá, é gravado e catalogado, os
//!   PARÂMETROS voltam com ele, a cópia nova é marcada como rejeitada, e **só
//!   então** a nuvem perde a foto.
//!
//! A que falhar em qualquer passo **continua na nuvem, sem a rejeição**, e a
//! tela diz qual foi.
//!
//! 🔑 **Tirar a rejeição sobe de novo**: a foto local sem a marca volta a ser
//! candidata à subida em segundo plano (`subir_o_que_falta_do_ensaio`), como
//! qualquer outra do ensaio.
//!
//! ⚠️ **Uma de cada vez.** São brutos de 24 MP; baixar dez em paralelo é o
//! caminho conhecido para a máquina engasgar no meio — e engasgar aqui é ter
//! tirado umas e não outras.
//!
//! [`Publicador::rejeitar_tirando_da_nuvem`]: crate::pos_venda::porta::Publicador::rejeitar_tirando_da_nuvem

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;

use domain::value_objects::{ImportMode, ImportOptions};
use gpui_kit::{AsyncApp, Context};

use super::Aplicativo;
use crate::biblioteca::marcacao::{Marca, REJEITADA_NO_CATALOGO};
use crate::importacao::explorador::{Andamento, Freios};
use crate::pos_venda::porta::Recado;
use crate::revelacao::persistencia::{self, Corte};
use crate::revelacao::processador::Ajustes;

/// Quanto tempo se espera por cada passo antes de desistir **sem remover nada**.
///
/// 🔑 **Desistir é o desfecho seguro**: a foto continua na nuvem, como estava.
const ESPERA_MAXIMA: Duration = Duration::from_secs(180);
const RESPIRO: Duration = Duration::from_millis(100);

/// Uma foto do site que o `X` mandou sair da nuvem.
#[derive(Debug, Clone, PartialEq)]
pub struct AFotoQueVolta {
    /// O id dela no site — é por ele que se baixa e se remove.
    pub no_site: String,
    /// O nome do arquivo, que é o que sobrevive à viagem de ida e volta.
    pub arquivo: String,
    /// Os PARÂMETROS que estão na nuvem, para voltarem com ela (caminho B).
    pub ajustes: Ajustes,
    pub corte: Corte,
    /// A sessão da foto. `None` = a aberta na tela — o `X` de agora.
    ///
    /// 📴 O resgate que espera a rede (`Gesto::RejeitarDaNuvem`) leva a sessão
    /// dele: quando a rede volta, o operador pode estar em outra sessão, ou na
    /// lista, e a foto sem cópia aqui tem de voltar para a sessão dela.
    pub galeria: Option<String>,
}

/// O que aconteceu com cada foto — o que a tela conta no fim.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Desfecho {
    /// As que saíram da nuvem e ficaram aqui, rejeitadas.
    pub voltaram: Vec<String>,
    /// As que **ficaram na nuvem**, sem a rejeição, por não ter sido possível
    /// garantir a cópia daqui ou o site ter recusado.
    pub ficaram: Vec<String>,
    /// 📴 As que a rede caída segurou: a rejeição ficou na fila e o resgate
    /// roda quando ela voltar.
    pub guardadas: Vec<String>,
}

impl Desfecho {
    /// As duas frases do fim: uma por desfecho, e só quando há o que dizer.
    pub fn avisos(&self) -> (Option<String>, Option<String>) {
        let sucesso = (!self.voltaram.is_empty()).then(|| {
            format!(
                "{} foto(s) rejeitada(s): saíram da nuvem e ficaram neste computador",
                self.voltaram.len()
            )
        });
        let falha = (!self.ficaram.is_empty()).then(|| {
            format!(
                "{} foto(s) continuam na nuvem, sem a rejeição: não deu para garantir a cópia \
                 daqui ({}). Tente o X de novo.",
                self.ficaram.len(),
                self.ficaram.join(" / ")
            )
        });
        let sucesso = match (sucesso, self.guardadas.is_empty()) {
            (s, true) => s,
            (s, false) => {
                let guardadas = format!(
                    "Sem internet: {} foto(s) rejeitada(s). Saem da nuvem e voltam para este \
                     computador quando a rede voltar.",
                    self.guardadas.len()
                );
                Some(match s {
                    Some(s) => format!("{s}. {guardadas}"),
                    None => guardadas,
                })
            }
        };
        (sucesso, falha)
    }
}

impl Aplicativo {
    /// Rejeita estas fotos do site — cada uma volta para cá antes de a nuvem
    /// a perder.
    ///
    /// 📴 **Sem internet** (ou com a fila de sincronização esperando, para não
    /// furar a vez) o resgate não roda — ele baixa o bruto. A rejeição vai para
    /// a fila: a grade já a mostra rejeitada, o site a marca quando a rede
    /// voltar, e só então o resgate de sempre traz a foto e a tira da nuvem.
    pub fn rejeitar_da_nuvem(&mut self, fotos: Vec<AFotoQueVolta>, cx: &mut Context<Self>) {
        if fotos.is_empty() || !self.pode_trabalhar() {
            return;
        }
        self.resgatar(fotos, cx);
    }

    /// 📴 O resgate desta foto espera a rede? Sem internet, sim. Com a fila de
    /// sincronização esperando, também — o `X` de agora não fura a vez do que
    /// foi feito antes —, a não ser que o resgate venha da própria fila.
    fn resgate_espera(&self, foto: &AFotoQueVolta) -> bool {
        let Some(fila) = crate::sincronizacao::a_do_app() else {
            return false;
        };
        let veio_da_fila = foto.galeria.is_some();
        !fila.rede().com_rede() || (!veio_da_fila && fila.tem_pendente())
    }

    /// 📴 A parte do site vai para a fila: ela marca a foto como rejeitada lá
    /// e pede o resgate quando a rede voltar.
    fn guardar_a_rejeicao(&mut self, foto: &AFotoQueVolta) {
        let Some(fila) = crate::sincronizacao::a_do_app() else {
            return;
        };
        fila.guardar(crate::sincronizacao::Gesto::RejeitarDaNuvem {
            foto_no_site: foto.no_site.clone(),
            arquivo: foto.arquivo.clone(),
            parametros: crate::pos_venda::porta::ajustes_em_json(
                &foto.ajustes,
                &persistencia::para_crop_settings(&foto.corte),
            ),
            galeria_id: foto.galeria.clone().or_else(|| self.sessao_aberta.clone()),
        });
    }

    /// O resgate de sempre, com a rede de pé — ver o topo do módulo.
    ///
    /// 🚨 **Um lote por vez, e o que chega no meio espera.** O lote novo
    /// substituía a tarefa do que estava rodando — e cancelava uma foto no
    /// meio do caminho. Agora ele entra na fila do resgate e sai quando o
    /// anterior termina (revisão de 20/10/2026).
    pub(crate) fn resgatar(&mut self, fotos: Vec<AFotoQueVolta>, cx: &mut Context<Self>) {
        if fotos.is_empty() || self.sessao().is_none() {
            return;
        }
        if self.resgatando {
            self.resgates_na_fila.extend(fotos);
            return;
        }
        self.resgatando = true;
        self._resgate = Some(cx.spawn(async move |raiz, cx| {
            let mut desfecho = Desfecho::default();
            for foto in fotos {
                // 📴 **O resgate que vem da fila confere o desfazer.** O `X`
                // desfeito sem rede numa foto com cópia aqui tira só a marca
                // do catálogo; a fila não sabe disso. Cópia sem a marca: o
                // site perde a rejeição, e a foto fica onde está.
                if foto.galeria.is_some() && copia_sem_rejeicao(&raiz, &foto.no_site, cx).await {
                    let _ = raiz.update(cx, |raiz, _| {
                        let Some(sessao) = raiz.sessao().cloned() else {
                            return;
                        };
                        let (para_o_site, _) = channel::<Recado>();
                        raiz.publicador.negociar(
                            sessao,
                            foto.no_site.clone(),
                            domain::services::pos_venda::MudancaDaFoto {
                                rejeitada: Some(false),
                                ..Default::default()
                            },
                            para_o_site,
                        );
                    });
                    continue;
                }
                // 📴 Sem rede (ou com a fila esperando): a cópia daqui fica
                // rejeitada já, e o site espera na fila.
                if raiz
                    .update(cx, |raiz, _| raiz.resgate_espera(&foto))
                    .unwrap_or(false)
                {
                    guardar_sem_rede(&raiz, &foto, cx).await;
                    desfecho.guardadas.push(foto.arquivo.clone());
                    continue;
                }
                // 🔑 **Marcada antes do `DELETE`**: um envio para ela que já
                // esteja no ar volta com 404, e esse 404 não é recusa — ver
                // `largar_o_que_ia_para`.
                let _ = raiz.update(cx, |raiz, _| {
                    raiz.tiradas_do_site.insert(foto.no_site.clone())
                });
                // ⚠️ Uma de cada vez, e a que falha não derruba as outras.
                match rejeitar_uma(&raiz, &foto, cx).await {
                    Ok(()) => {
                        let _ = raiz
                            .update(cx, |raiz, cx| raiz.largar_o_que_ia_para(&foto.no_site, cx));
                        desfecho.voltaram.push(foto.arquivo.clone())
                    }
                    Err(()) => {
                        // Ficou no site: o que for para ela volta a valer.
                        let _ =
                            raiz.update(cx, |raiz, _| raiz.tiradas_do_site.remove(&foto.no_site));
                        // 📴 Foi a rede que caiu no meio: a rejeição vai para a
                        // fila, e o resgate roda quando ela voltar.
                        let sem_rede = crate::sincronizacao::a_do_app()
                            .is_some_and(|fila| !fila.rede().com_rede());
                        if sem_rede {
                            guardar_sem_rede(&raiz, &foto, cx).await;
                            desfecho.guardadas.push(foto.arquivo.clone())
                        } else {
                            desfecho.ficaram.push(foto.arquivo.clone())
                        }
                    }
                }
            }
            let _ = raiz.update(cx, |raiz, cx| raiz.terminar_o_resgate(desfecho, cx));
        }));
    }

    /// O fim do lote: as duas listas relêem, e o operador ouve o que houve.
    fn terminar_o_resgate(&mut self, desfecho: Desfecho, cx: &mut Context<Self>) {
        self.resgatando = false;
        let (sucesso, falha) = desfecho.avisos();
        // O catálogo ganhou (ou mudou) fotos e o site perdeu: as duas mudaram.
        self.pedir_releitura_do_acervo(cx);
        if self.sessao_aberta.is_some() {
            self.detalhe.update(cx, |tela, cx| tela.reler(cx));
        }
        if let Some(texto) = sucesso {
            self.avisar_onde_esta_olhando(texto, cx);
        }
        if let Some(texto) = falha {
            self.avisar_falha(texto, cx);
        }
        self.ultimo_resgate = Some(desfecho);
        // O que chegou enquanto este lote rodava.
        let proximas = std::mem::take(&mut self.resgates_na_fila);
        self.resgatar(proximas, cx);
        cx.notify();
    }

    /// 🧪 O desfecho do último resgate — o que os cenários afirmam.
    #[cfg(test)]
    pub(crate) fn ultimo_resgate(&self) -> Option<Desfecho> {
        self.ultimo_resgate.clone()
    }
}

/// O caminho de **uma** foto. `Err` é "ficou na nuvem, como estava".
async fn rejeitar_uma(
    raiz: &gpui_kit::WeakEntity<Aplicativo>,
    foto: &AFotoQueVolta,
    cx: &mut AsyncApp,
) -> Result<(), ()> {
    let Ok(Some((sessao, galeria))) = raiz.update(cx, |raiz, _| {
        raiz.sessao()
            .cloned()
            .zip(foto.galeria.clone().or_else(|| raiz.sessao_aberta.clone()))
    }) else {
        return Err(());
    };

    // A cópia daqui, se houver: a linha do catálogo que aponta para esta foto.
    let copia_aqui = achar_a_copia(raiz, &foto.no_site, cx).await;

    match copia_aqui {
        // A · o arquivo já está no disco: um pedido só.
        Some(id_local) => {
            // 🔑 Antes da ida: a releitura pode voltar sem a marca, e a passada
            // de subida poria a foto de novo na fila.
            let _ = raiz.update(cx, |raiz, _| {
                raiz.rejeitadas_agora.insert(id_local.clone());
            });
            let (para_o_site, do_site) = channel::<Recado>();
            let pedido = id_local.clone();
            raiz.update(cx, |raiz, _| {
                raiz.publicador
                    .rejeitar_tirando_da_nuvem(sessao.clone(), pedido, para_o_site)
            })
            .map_err(|_| ())?;
            match esperar(&do_site, cx).await {
                Some(Recado::Falhou(_)) | None => {
                    let _ = raiz.update(cx, |raiz, _| raiz.rejeitadas_agora.remove(&id_local));
                    return Err(());
                }
                Some(_) => {}
            }
        }
        // B · o resgate inteiro, e a nuvem por último.
        None => {
            let id_local = trazer_para_ca(raiz, foto, &sessao, &galeria, cx).await?;
            let _ = raiz.update(cx, |raiz, _| {
                raiz.rejeitadas_agora.insert(id_local.clone());
                raiz.marcador
                    .marcar(id_local.clone(), Marca::Sinalizador(REJEITADA_NO_CATALOGO));
            });
            let (para_o_site, do_site) = channel::<Recado>();
            raiz.update(cx, |raiz, _| {
                raiz.publicador
                    .remover_remoto(sessao.clone(), foto.no_site.clone(), para_o_site)
            })
            .map_err(|_| ())?;
            match esperar(&do_site, cx).await {
                // ⚠️ A cópia daqui fica, rejeitada: ela é a foto, e a nuvem
                // continuar com ela é o desfecho seguro. O operador vê a falha
                // e tenta de novo.
                Some(Recado::Falhou(_)) | None => return Err(()),
                Some(_) => {}
            }
        }
    };

    // 🔑 A prévia sob o id do site não fica para trás: ele deixou de existir, e
    // o cache sob ele viraria lixo que ninguém atualiza.
    let no_acervo = format!("{}{}", persistencia::PREFIXO_DO_SITE, foto.no_site);
    let _ = raiz.update(cx, |raiz, cx| raiz.esquecer_a_previa_local(&no_acervo, cx));
    Ok(())
}

/// B · 1–3: o BRUTO vem, é gravado e catalogado, e os PARÂMETROS voltam com
/// ele. Devolve o id da cópia nova no catálogo.
async fn trazer_para_ca(
    raiz: &gpui_kit::WeakEntity<Aplicativo>,
    foto: &AFotoQueVolta,
    sessao: &domain::services::pos_venda::Sessao,
    galeria: &str,
    cx: &mut AsyncApp,
) -> Result<String, ()> {
    // 1 · 🚨 **O BRUTO, e nunca a versão revelada no lugar dele** (C3).
    let (para_o_bruto, do_bruto) = channel::<Recado>();
    let no_site = foto.no_site.clone();
    let sessao_do_pedido = sessao.clone();
    raiz.update(cx, |raiz, _| {
        raiz.publicador
            .original(sessao_do_pedido, no_site, para_o_bruto)
    })
    .map_err(|_| ())?;
    let bytes = match esperar(&do_bruto, cx).await {
        Some(Recado::Original { bytes, .. }) => bytes,
        // `OriginalIndisponivel`: sem bytes não há cópia, e sem cópia não se
        // apaga nada.
        _ => return Err(()),
    };
    // 🚨 **Bytes que não são imagem não são cópia.** O cliente não recebe a
    // impressão digital do bruto; o mínimo que dá para conferir daqui é que
    // chegou algo e que decodifica.
    if bytes.is_empty() || image::guess_format(&bytes).is_err() {
        return Err(());
    }

    // 2 · O arquivo, e o catálogo.
    let caminho = gravar_o_arquivo(&foto.arquivo, &bytes, cx).await?;
    let opcoes = ImportOptions {
        // **Add**, e não `Copy`: o arquivo já nasceu no destino.
        mode: ImportMode::Add,
        skip_duplicates: false,
        sessao_id: Some(galeria.to_string()),
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

    // 3 · Os PARÂMETROS voltam com ela — senão ela volta crua, e tirar a
    // rejeição a subiria crua.
    let id_local = achar_pelo_caminho(raiz, &arquivo, cx).await?;
    raiz.update(cx, |raiz, _| {
        raiz.gravador
            .gravar(id_local.clone(), foto.ajustes, foto.corte)
    })
    .map_err(|_| ())?;
    Ok(id_local)
}

/// Grava o bruto na pasta das que voltaram, sem repetir nome.
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
            // Mesmo nome duas vezes é o caso de quem rejeita, desfaz e rejeita
            // de novo: o segundo arquivo não pode calar o primeiro.
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
/// aquele arquivo veio.
#[cfg(not(test))]
pub(crate) fn pasta_das_resgatadas() -> PathBuf {
    infrastructure::paths::AppPaths::catalog_root().join("Resgatadas")
}

/// ⚠️ Em teste a pasta é temporária **e por cenário**: nenhum escreve no
/// catálogo de quem está desenvolvendo.
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

/// Relê o catálogo e devolve a primeira foto que satisfaz `achar`.
async fn reler_e_achar(
    raiz: &gpui_kit::WeakEntity<Aplicativo>,
    achar: impl Fn(&adapters::view_models::PhotoViewModel) -> bool,
    cx: &mut AsyncApp,
) -> Option<String> {
    let (para_o_acervo, do_acervo) = channel::<Vec<adapters::view_models::PhotoViewModel>>();
    raiz.update(cx, |raiz, _| raiz.acervo.recarregar(para_o_acervo))
        .ok()?;
    let voltas = (ESPERA_MAXIMA.as_millis() / RESPIRO.as_millis()) as u32;
    for _ in 0..voltas {
        if let Ok(fotos) = do_acervo.try_recv() {
            return fotos.into_iter().find(|f| achar(f)).map(|f| f.id);
        }
        cx.background_executor().timer(RESPIRO).await;
    }
    None
}

/// A cópia desta foto do site no catálogo daqui, se houver.
/// 📴 A rejeição sem rede: a cópia daqui (se houver) fica rejeitada no
/// catálogo na hora — é ela que a grade mostra, e a marca segura a subida —, e
/// a parte do site vai para a fila.
async fn guardar_sem_rede(
    raiz: &gpui_kit::WeakEntity<Aplicativo>,
    foto: &AFotoQueVolta,
    cx: &mut AsyncApp,
) {
    let copia = achar_a_copia(raiz, &foto.no_site, cx).await;
    let _ = raiz.update(cx, |raiz, _| {
        if let Some(id_local) = copia {
            raiz.marcador
                .marcar(id_local, Marca::Sinalizador(REJEITADA_NO_CATALOGO));
        }
        raiz.guardar_a_rejeicao(foto);
    });
}

/// A cópia daqui desta foto do site existe e **não** está rejeitada no
/// catálogo — o `X` foi desfeito nela.
async fn copia_sem_rejeicao(
    raiz: &gpui_kit::WeakEntity<Aplicativo>,
    no_site: &str,
    cx: &mut AsyncApp,
) -> bool {
    reler_e_achar(
        raiz,
        |f| {
            f.pos_venda_foto_id.as_deref() == Some(no_site) && f.flag != Some(REJEITADA_NO_CATALOGO)
        },
        cx,
    )
    .await
    .is_some()
}

async fn achar_a_copia(
    raiz: &gpui_kit::WeakEntity<Aplicativo>,
    no_site: &str,
    cx: &mut AsyncApp,
) -> Option<String> {
    reler_e_achar(
        raiz,
        |f| f.pos_venda_foto_id.as_deref() == Some(no_site),
        cx,
    )
    .await
}

/// A foto que acabou de entrar, pelo caminho do arquivo.
async fn achar_pelo_caminho(
    raiz: &gpui_kit::WeakEntity<Aplicativo>,
    arquivo: &str,
    cx: &mut AsyncApp,
) -> Result<String, ()> {
    reler_e_achar(raiz, |f| f.path == arquivo, cx)
        .await
        .ok_or(())
}

#[cfg(test)]
mod testes {
    use super::*;

    /// As duas frases do fim — e o silêncio quando não há o que dizer.
    #[test]
    fn o_aviso_conta_os_dois_lados() {
        assert_eq!(Desfecho::default().avisos(), (None, None));

        let misto = Desfecho {
            voltaram: vec!["a.jpg".into(), "b.jpg".into()],
            ficaram: vec!["c.jpg".into()],
            guardadas: Vec::new(),
        };
        let (sucesso, falha) = misto.avisos();
        assert!(sucesso.is_some_and(|s| s.starts_with("2 foto(s) rejeitada(s)")));
        let falha = falha.expect("a frase do que ficou");
        assert!(falha.contains("c.jpg"), "o nome é o que o operador procura");
        assert!(falha.contains("continuam na nuvem"), "{falha}");
    }

    /// 📴 A que a rede caída segurou não é falha: ela espera na fila.
    #[test]
    fn a_rejeicao_que_a_rede_segurou_e_dita_como_guardada() {
        let so_guardada = Desfecho {
            guardadas: vec!["d.jpg".into()],
            ..Default::default()
        };
        let (sucesso, falha) = so_guardada.avisos();
        assert_eq!(falha, None);
        assert!(sucesso.is_some_and(|s| s.starts_with("Sem internet: 1 foto(s) rejeitada(s)")));
    }
}
