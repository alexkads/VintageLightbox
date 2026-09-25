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
use super::vigia::{Passo, Vigia};
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
    /// O operador pediu para sair quando a fila esvaziar — o "Esperar terminar
    /// e sair" do aviso de saída, onde não há bandeja (`app/segundo_plano.rs`).
    sair_ao_esvaziar: bool,
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
        sair_ao_esvaziar: false,
    });

    // Fechar leva para a bandeja, sempre (dono, 2026-09-21).
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

/// O clique num aviso do sistema (chatbot, agenda) traz a janela para a
/// frente — da bandeja, se estiver lá, e de minimizada ou atrás de outra
/// janela nos outros casos. Sem o laço ligado (os testes) não faz nada: quem
/// chama cuida do resto.
pub fn trazer_para_a_frente(cx: &mut App) {
    if !cx.has_global::<SegundoPlano>() {
        return;
    }
    cx.update_global::<SegundoPlano, _>(|sp, cx| {
        if sp.vigia.na_bandeja() {
            mostrar_janela(sp, cx);
            return;
        }
        if let Ok(mostrar) = sp
            .principal
            .update(cx, |_, window, _cx| janela::mostrar(window))
        {
            depois(cx, mostrar);
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

/// O "Fechar" da barra que o app desenha (`crate::janela::controles`).
///
/// 🚨 **Não é `remove_window` direto.** O `remove_window` do GPUI não passa
/// pelo `on_window_should_close`, e é ali que mora a regra de fechar levar à
/// bandeja: o botão da barra encerrava o app, enquanto o fechar do sistema o
/// mantinha vivo (achado do dono no GNOME, 24/set/2026 — *"os botões não
/// funcionam direito"*). Na janela principal o botão pergunta o mesmo que o
/// sistema perguntaria; nas outras (a tela do cliente) fechar é fechar.
pub fn fechar_pelo_botao(window: &mut Window, cx: &mut App) {
    let principal = cx.try_global::<SegundoPlano>().map(|sp| sp.principal);
    if principal == Some(window.window_handle()) && !ao_fechar(window, cx) {
        return;
    }
    window.remove_window();
}

fn ao_fechar(window: &mut Window, cx: &mut App) -> bool {
    if !cx.has_global::<SegundoPlano>() {
        return true;
    }
    // 🚨 **Sem bandeja no sistema, não há para onde ir** (dono, 25/set/2026).
    // No GNOME sem a extensão AppIndicator o ícone não aparece: "ir para a
    // bandeja" era minimizar para sempre, sem o "Sair" à vista, e o app não
    // tinha como ser encerrado (`bandeja::existe_no_sistema`).
    //
    // Mas fechar com envio no ar não pode ser calado: sem bandeja, o app
    // **pergunta** (dono, no mesmo dia: *"quando não tiver como mostrar a
    // bandeja, deve se adotar outra estratégia"*) — esperar a fila e sair,
    // minimizar e continuar, ou sair mesmo assim. Com a fila vazia, fecha.
    if !bandeja::existe_no_sistema() {
        let raiz = cx
            .try_global::<SegundoPlano>()
            .and_then(|sp| sp.raiz.upgrade());
        let pendente = raiz
            .as_ref()
            .is_some_and(|raiz| raiz.read(cx).retrato_do_segundo_plano(cx).ha_envio_pendente());
        if !pendente {
            rastro("pedido de fechar: sem bandeja e sem envio, o app encerra");
            return true;
        }
        rastro("pedido de fechar: sem bandeja e com envio, pergunta");
        if let Some(raiz) = raiz {
            raiz.update(cx, |raiz, cx| raiz.perguntar_antes_de_sair(cx));
        }
        return false;
    }
    cx.update_global::<SegundoPlano, _>(|sp, cx| {
        rastro("pedido de fechar: para a bandeja");
        sp.vigia.ao_fechar();
        depois(cx, janela::esconder(window));
        para_a_bandeja(sp, cx);
        false
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
        // O "Esperar terminar e sair" do aviso de saída: a fila esvaziou.
        if sp.sair_ao_esvaziar && !retrato.ha_envio_pendente() {
            rastro("a fila esvaziou: saindo, como pedido");
            return true;
        }
        // O app não termina sozinho: sair é o "Sair" da bandeja (ou `⌘Q`).
        false
    });
    if sair {
        rastro("saindo");
        #[cfg(test)]
        SAIU.with(|s| s.set(true));
        cx.quit();
    }
}

/// O "Esperar terminar e sair" do aviso de saída: o laço encerra o app na
/// primeira volta com a fila vazia. `false` desiste.
pub fn sair_quando_a_fila_esvaziar(sim: bool, cx: &mut App) {
    if cx.has_global::<SegundoPlano>() {
        cx.update_global::<SegundoPlano, _>(|sp, _cx| sp.sair_ao_esvaziar = sim);
    }
}

/// O "Sair mesmo assim": encerra agora. O que não subiu segue guardado no
/// depósito, e sobe na próxima abertura — como no "Sair" da bandeja.
pub fn sair_agora(cx: &mut App) {
    rastro("saindo agora, a pedido");
    #[cfg(test)]
    SAIU.with(|s| s.set(true));
    cx.quit();
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
