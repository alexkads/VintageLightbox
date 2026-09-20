//! O laço que olha a janela e a raiz, e mantém a bandeja em dia.
//!
//! 🔑 **Uma volta a cada 150 ms para os cliques, e a cada ~0,7 s para o resto.**
//! Perguntar ao sistema se a janela está minimizada custa nada e não falha, que
//! é mais do que se pode dizer de um evento que o GPUI não tem.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui::{App, BorrowAppContext, Global, WeakEntity, Window};

use super::frases::{self, Linhas};
use super::janela;
use super::vigia::{AoFechar, Passo, Vigia};
use crate::app::Aplicativo;
use crate::bandeja::{self, Clique, Icone};

const PASSO: Duration = Duration::from_millis(150);
/// Voltas de `PASSO` entre uma olhada na janela e a seguinte (~0,75 s).
const VOLTAS_POR_OLHADA: u32 = 5;
/// Somar a pasta do catálogo inteira é ler o disco: no máximo uma vez por minuto.
const VALIDADE_DO_TAMANHO: Duration = Duration::from_secs(60);

#[derive(Default)]
struct Medida {
    quando: Option<Instant>,
    bytes: Option<u64>,
}

struct SegundoPlano {
    principal: gpui::AnyWindowHandle,
    raiz: WeakEntity<Aplicativo>,
    /// `None` até a primeira ida à bandeja; `Err` se o sistema recusou.
    icone: Option<Result<Icone, String>>,
    vigia: Vigia,
    ultimas: Option<Linhas>,
    pasta: PathBuf,
    pilha_local: bool,
    medida: Arc<Mutex<Medida>>,
    voltas: u32,
}

impl Global for SegundoPlano {}

impl SegundoPlano {
    fn icone(&mut self) -> Option<&Icone> {
        // 🧪 Nos testes a bandeja não nasce: o ícone é do sistema, e a suíte
        // não roda na thread principal que o AppKit exige.
        if cfg!(test) {
            return None;
        }
        let icone = self.icone.get_or_insert_with(|| {
            Icone::criar().inspect_err(|erro| {
                eprintln!("⚠️ [Bandeja] o sistema não deu o ícone: {erro}");
            })
        });
        let icone = icone.as_ref().ok()?;
        // Nasce com as linhas de agora, e não com "…" até algo mudar.
        if let Some(linhas) = &self.ultimas {
            icone.atualizar(linhas);
        }
        Some(icone)
    }

    fn para_a_bandeja(&mut self, cx: &App) {
        rastro("para a bandeja");
        if let Some(icone) = self.icone() {
            icone.mostrar(true);
        }
        depois(cx, Box::new(|| janela::no_dock(false)));
    }

    fn tirar_da_bandeja(&mut self) {
        rastro("fora da bandeja");
        if let Some(Ok(icone)) = &self.icone {
            icone.mostrar(false);
        }
    }
}

/// Liga o segundo plano à janela principal. Chamado ao abri-la.
pub fn ligar(raiz: WeakEntity<Aplicativo>, window: &mut Window, cx: &mut App) {
    let base = crate::pos_venda::config::ler().base_url;
    let pilha_local = base.contains("://localhost") || base.contains("://127.0.0.1");
    cx.set_global(SegundoPlano {
        principal: window.window_handle(),
        raiz,
        icone: None,
        vigia: Vigia::default(),
        ultimas: None,
        // Nos testes a medição do espaço não varre o catálogo de ninguém.
        pasta: if cfg!(test) {
            std::env::temp_dir().join("vlb-catalogo-inexistente-dos-testes")
        } else {
            infrastructure::paths::AppPaths::catalog_root()
        },
        pilha_local,
        medida: Arc::default(),
        voltas: 0,
    });

    // G9: fechar com envio na fila só esconde a janela.
    window.on_window_should_close(cx, ao_fechar);

    cx.spawn(async move |cx| loop {
        cx.background_executor().timer(PASSO).await;
        if cx.update(volta).is_err() {
            return;
        }
    })
    .detach();
}

/// `janela fechar` e `janela abrir` do roteiro de depuração.
pub fn gesto_de_roteiro(gesto: &str, cx: &mut App) {
    match gesto {
        "abrir" => ao_reabrir(cx),
        "fechar" => {
            let Some(principal) = cx.try_global::<SegundoPlano>().map(|sp| sp.principal) else {
                return;
            };
            if let Ok(fechar) =
                principal.update(cx, |_, window, _cx| janela::pedir_para_fechar(window))
            {
                depois(cx, fechar);
            }
        }
        _ => {}
    }
}

/// O ícone do Dock (ou abrir o app de novo) com o app na bandeja traz a janela.
pub fn ao_reabrir(cx: &mut App) {
    if !cx.has_global::<SegundoPlano>() {
        return;
    }
    cx.update_global::<SegundoPlano, _>(|sp, cx| {
        if sp.vigia.na_bandeja() {
            mostrar_janela(sp, cx);
        }
    });
}

fn retrato(sp: &SegundoPlano, cx: &App) -> Option<frases::Retrato> {
    let raiz = sp.raiz.upgrade()?;
    let mut r = raiz.read(cx).retrato_do_segundo_plano(cx);
    r.pilha_local = sp.pilha_local;
    r.bytes_do_catalogo = sp.medida.lock().ok().and_then(|m| m.bytes);
    r.agora = chrono::Utc::now().timestamp();
    Some(r)
}

fn ao_fechar(window: &mut Window, cx: &mut App) -> bool {
    if !cx.has_global::<SegundoPlano>() {
        return true;
    }
    cx.update_global::<SegundoPlano, _>(|sp, cx| {
        let retrato = retrato(sp, cx);
        let pendente = retrato.as_ref().is_some_and(|r| r.ha_envio_pendente());
        rastro(&format!("pedido de fechar, envio pendente: {pendente}"));
        match sp.vigia.ao_fechar(pendente) {
            AoFechar::Fechar => true,
            // 🚪 **O aviso, e a janela fica** (dono, 2026-09-20). A frase é a
            // mesma linha que a bandeja mostra — o operador vê o que está
            // pendente, e não só que "há algo".
            AoFechar::Avisar => {
                let frase = retrato
                    .map(|r| frases::linhas(&r).subindo)
                    .unwrap_or_else(|| "Subindo: fotos na fila".into());
                if let Some(raiz) = sp.raiz.upgrade() {
                    raiz.update(cx, |raiz, cx| {
                        raiz.avisar_fechamento_pendente(frase.into(), cx)
                    });
                }
                false
            }
            AoFechar::Esconder => {
                depois(cx, janela::esconder(window));
                para_a_bandeja(sp, cx);
                false
            }
        }
    })
}

/// A janela some e a bandeja assume — o G9, de onde quer que o pedido venha.
fn para_a_bandeja(sp: &mut SegundoPlano, cx: &mut App) {
    sp.para_a_bandeja(cx);
    // A segunda tela não fica sozinha no monitor do cliente.
    if let Some(raiz) = sp.raiz.upgrade() {
        raiz.update(cx, |raiz, cx| raiz.fechar_tela_do_cliente(cx));
    }
}

/// O "Continuar em segundo plano" do aviso: a resposta que o aviso esperava.
///
/// 🚨 **Esconde aqui, e não repete o pedido de fechar.** No Linux
/// `janela::pedir_para_fechar` não faz nada — é o próprio GPUI que manda na
/// janela —, e o botão ficaria mudo justamente na máquina do balcão.
///
/// ⚠️ **Não mexe na raiz**: quem chama é um clique dentro dela, e um
/// `raiz.update` aqui seria "cannot update while it is already being updated".
/// A segunda tela é fechada por quem clicou, antes de chamar.
pub fn fechar_mesmo(cx: &mut App) {
    if !cx.has_global::<SegundoPlano>() {
        return;
    }
    cx.update_global::<SegundoPlano, _>(|sp, cx| {
        // A vigia já avisou: este `ao_fechar` é a resposta, e devolve `Esconder`.
        if sp.vigia.ao_fechar(true) != AoFechar::Esconder {
            return;
        }
        // ⚠️ **A janela também está em uso** — o clique que chegou aqui saiu
        // dela. Pedir o `esconder` numa tarefa é o que tira as duas coisas do
        // `update` de agora, como todo gesto de janela deste módulo.
        let principal = sp.principal;
        cx.spawn(async move |cx| {
            let esconder = cx
                .update(|cx| {
                    principal
                        .update(cx, |_, window, _cx| janela::esconder(window))
                        .ok()
                })
                .ok()
                .flatten();
            if let Some(esconder) = esconder {
                esconder();
            }
        })
        .detach();
        sp.para_a_bandeja(cx);
    });
}

/// O "Ficar no app": a vigia esquece que avisou, e o próximo fechamento
/// pergunta de novo.
pub fn desistiu_de_fechar(cx: &mut App) {
    if !cx.has_global::<SegundoPlano>() {
        return;
    }
    cx.update_global::<SegundoPlano, _>(|sp, _cx| sp.vigia.desistiu_de_fechar());
}

fn mostrar_janela(sp: &mut SegundoPlano, cx: &mut App) {
    sp.vigia.restaurar(Instant::now());
    let mostrar = sp
        .principal
        .update(cx, |_, window, _cx| janela::mostrar(window))
        .ok();
    // O app volta ao Dock antes de a janela voltar.
    depois(
        cx,
        Box::new(move || {
            janela::no_dock(true);
            if let Some(mostrar) = mostrar {
                mostrar();
            }
        }),
    );
    sp.tirar_da_bandeja();
}

/// Roda o gesto na janela fora do `update` de agora (ver `janela`).
fn depois(cx: &App, gesto: janela::Adiado) {
    cx.spawn(async move |_| gesto()).detach();
}

fn volta(cx: &mut App) {
    if !cx.has_global::<SegundoPlano>() {
        return;
    }
    let sair = cx.update_global::<SegundoPlano, _>(|sp, cx| {
        let mut sair = false;
        for clique in bandeja::cliques() {
            match clique {
                Clique::Abrir => mostrar_janela(sp, cx),
                Clique::AbrirPasta => bandeja::abrir_pasta(&sp.pasta),
                // Pela bandeja, sair é escolha explícita: não espera a fila.
                // O que não subiu segue guardado no depósito.
                Clique::Sair => sair = true,
            }
        }

        sp.voltas = sp.voltas.wrapping_add(1);
        if sair || sp.voltas % VOLTAS_POR_OLHADA != 0 {
            return sair;
        }

        let minimizada = sp
            .principal
            .update(cx, |_, window, _cx| janela::minimizada(window))
            .unwrap_or(false);
        match sp.vigia.volta(minimizada, Instant::now()) {
            Passo::ParaABandeja => sp.para_a_bandeja(cx),
            Passo::Voltou => {
                depois(cx, Box::new(|| janela::no_dock(true)));
                sp.tirar_da_bandeja();
            }
            Passo::Nada => {}
        }

        if sp.vigia.na_bandeja() {
            medir_o_catalogo(sp, cx);
        }

        let Some(retrato) = retrato(sp, cx) else {
            return false;
        };
        let novas = frases::linhas(&retrato);
        if sp.ultimas.as_ref() != Some(&novas) {
            if let Some(Ok(icone)) = &sp.icone {
                icone.atualizar(&novas);
            }
            rastro(&format!("{novas:?}"));
            sp.ultimas = Some(novas);
        }
        sp.vigia.deve_sair(retrato.ha_envio_pendente())
    });
    if sair {
        rastro("saindo");
        #[cfg(test)]
        SAIU.with(|s| s.set(true));
        cx.quit();
    }
}

#[cfg(test)]
thread_local! {
    /// O `cx.quit()` da plataforma de teste não faz nada: é aqui que o
    /// cenário vê que o app decidiu terminar.
    static SAIU: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 🧪 O que o segundo plano decidiu até agora: `(na bandeja, pediu para sair)`.
#[cfg(test)]
pub(crate) fn estado_para_teste(cx: &App) -> Option<(bool, bool)> {
    let sp = cx.try_global::<SegundoPlano>()?;
    Some((sp.vigia.na_bandeja(), SAIU.with(std::cell::Cell::get)))
}

/// O que a bandeja fez, no terminal — só em depuração: a barra de menus não
/// sai na foto da janela, e é por aqui que um roteiro a confere.
fn rastro(texto: &str) {
    if cfg!(debug_assertions) {
        eprintln!("[bandeja] {texto}");
    }
}

/// Mede a pasta do catálogo fora da thread da interface, se a medida venceu.
fn medir_o_catalogo(sp: &SegundoPlano, cx: &App) {
    {
        let Ok(mut medida) = sp.medida.lock() else {
            return;
        };
        if medida
            .quando
            .is_some_and(|q| q.elapsed() < VALIDADE_DO_TAMANHO)
        {
            return;
        }
        // Marca antes de medir: a próxima volta não dispara outra medição.
        medida.quando = Some(Instant::now());
    }
    let (pasta, medida) = (sp.pasta.clone(), sp.medida.clone());
    cx.background_executor()
        .spawn(async move {
            let bytes = frases::tamanho_da_pasta(&pasta);
            if let Ok(mut m) = medida.lock() {
                m.bytes = Some(bytes);
            }
        })
        .detach();
}
