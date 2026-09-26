//! 🪟 A ponte entre os diálogos das telas e o `Dialog` do gpui-kit.
//!
//! # Por que uma ponte
//!
//! Diretriz do dono em 2026-09-26: padronizar no gpui-kit, *"sem ficar
//! diferente da versão Web"*. Os diálogos do app são **declarativos**: cada
//! tela os desenha a partir do próprio estado (`escolhendo_estudio`,
//! `restaurar: Option<…>`…). O `Dialog` do kit 0.6.6 **não tem modo
//! controlado** — ou abre por um gatilho, com estado dele, ou por
//! `window.open_dialog`. E abrir de dentro do `render` da tela é pedir a `Root`
//! emprestada enquanto ela mesma desenha.
//!
//! A ponte faz a conta a cada quadro: a tela diz se **quer** o diálogo, a
//! ponte sabe se ele **está** na `Root`, e a diferença se acerta no quadro
//! seguinte (`defer_in`). O conteúdo continua saindo da tela, lido a cada
//! quadro pela `Root` — então o que muda no estado aparece no diálogo.
//!
//! # O que é do kit e o que é do site
//!
//! Do kit: o véu, a caixa, o X, o Esc, o clique fora, o foco preso nele. Do
//! site, por cima: o respiro de 16 (`p-4 gap-4`), o canto de 12, o fundo do
//! `popover` (o kit usa o da página, que no escuro é outro) e a largura.
//!
//! # Sem a `Root` do kit
//!
//! Os testes que montam uma tela sozinha não têm `Root`, e `open_dialog` ali é
//! `panic`. Nesses, [`Ponte::sincronizar`] devolve o diálogo **desenhado no
//! lugar**, com o véu e a caixa do `estilo.rs` — o comportamento de antes.

use std::rc::Rc;

use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{ActiveTheme, Root, WindowExt as _};
use gpui_kit::{prelude::*, px, AnyElement, Context, Window};

use crate::estilo;

/// Como o diálogo se comporta — o do site, caso a caso.
#[derive(Clone, Copy)]
pub struct Jeito {
    /// A largura da caixa (a de hoje é 440; a exclusão usa 512).
    pub largura: f32,
    /// O `Esc` fecha.
    pub esc: bool,
    /// O clique fora fecha.
    pub veu: bool,
    /// O X no canto.
    pub x: bool,
}

impl Jeito {
    /// O `Dialog` do site: X, `Esc` e clique fora fecham.
    pub const fn dialogo(largura: f32) -> Self {
        Self {
            largura,
            esc: true,
            veu: true,
            x: true,
        }
    }

    /// O `AlertDialog` do site (e o `useConfirmacao`): só o `Esc` e os botões
    /// fecham — um clique perdido fora não descarta o que foi digitado.
    pub const fn alerta(largura: f32) -> Self {
        Self {
            largura,
            esc: true,
            veu: false,
            x: false,
        }
    }

    /// O "sem saída" do site (a escolha do estúdio): só fecha escolhendo.
    pub const fn sem_saida(largura: f32) -> Self {
        Self {
            largura,
            esc: false,
            veu: false,
            x: false,
        }
    }
}

/// O conteúdo do diálogo, lido da tela a cada quadro.
pub type Montar<T> = fn(&mut T, &mut Window, &mut Context<T>) -> Option<AnyElement>;
/// O que a tela faz quando o kit fecha o diálogo (Esc, clique fora, X).
pub type Cancelar<T> = fn(&mut T, &mut Window, &mut Context<T>);
/// Onde a tela guarda a ponte deste diálogo.
pub type AcharPonte<T> = fn(&mut T) -> &mut Ponte;
/// Se a tela ainda quer o diálogo aberto — lido pelo próprio diálogo.
pub type Querer<T> = fn(&T) -> bool;

/// Um diálogo de uma tela: sabe se ele está aberto na `Root`.
#[derive(Default)]
pub struct Ponte {
    na_raiz: bool,
    pedido: bool,
    /// A tela dona foi desenhada neste quadro. O diálogo da tela vivia dentro
    /// dela e sumia quando o operador ia para outra; na `Root` ele cobriria
    /// qualquer tela. Sem a tela à vista, ele fecha — e volta com ela.
    vista: bool,
}

/// Chamar no `render` da tela, a cada quadro. Devolve o diálogo para desenhar
/// no lugar **só** quando não há `Root` do kit (testes).
///
/// Recebe a tela, e não a ponte: sem `Root` o conteúdo sai dela ali mesmo, no
/// meio do `render` dela — pedir a entidade emprestada de novo seria `panic`.
#[allow(clippy::too_many_arguments)]
pub fn sincronizar<T: 'static>(
    tela: &mut T,
    querer: Querer<T>,
    jeito: Jeito,
    achar: AcharPonte<T>,
    montar: Montar<T>,
    cancelar: Cancelar<T>,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Option<AnyElement> {
    let quer = querer(tela);
    let com_raiz = window.root::<Root>().flatten().is_some();
    if !com_raiz {
        if !quer {
            return None;
        }
        let conteudo = montar(tela, window, cx)?;
        return Some(
            estilo::veu_do_dialogo()
                .child(
                    estilo::caixa_do_dialogo(cx)
                        .w(px(jeito.largura))
                        .child(conteudo),
                )
                .into_any_element(),
        );
    }
    let ponte = achar(tela);
    ponte.vista = true;
    if quer == ponte.na_raiz || ponte.pedido {
        return None;
    }
    // O acerto vai para o quadro seguinte: agora a `Root` está desenhando.
    ponte.pedido = true;
    cx.defer_in(window, move |tela, window, cx| {
        let ponte = achar(tela);
        ponte.pedido = false;
        if quer == ponte.na_raiz {
            return;
        }
        ponte.na_raiz = quer;
        if quer {
            abrir(jeito, querer, montar, cancelar, achar, window, cx);
        } else if window.has_active_dialog(cx) {
            window.close_dialog(cx);
        }
    });
    None
}

fn abrir<T: 'static>(
    jeito: Jeito,
    querer: Querer<T>,
    montar: Montar<T>,
    cancelar: Cancelar<T>,
    achar: AcharPonte<T>,
    window: &mut Window,
    cx: &mut Context<T>,
) {
    let fraca = cx.entity().downgrade();
    let ao_cancelar = Rc::new(move |window: &mut Window, cx: &mut gpui_kit::App| {
        let _ = fraca.update(cx, |tela, cx| {
            achar(tela).na_raiz = false;
            cancelar(tela, window, cx);
            cx.notify();
        });
        // Fecha na hora: o fechamento animado do kit devolve o foco só depois
        // da animação, e a rede da raiz o apanharia caído no meio.
        window.close_dialog(cx);
    });
    let fraca = cx.entity().downgrade();
    window.open_dialog(cx, move |dialogo: Dialog, window, cx| {
        // 🚨 **O diálogo se fecha sozinho quando a tela deixa de querê-lo.**
        // A ponte só acerta a diferença quando a tela dona é desenhada — e o
        // estado pode fechar o diálogo com ela fora da vista (o operador foi
        // para a agenda): o véu ficava por cima da agenda (os e2e pegaram).
        // E fecha também quando a tela sai da vista querendo-o: o diálogo era
        // dela, e não da janela.
        let ainda = fraca.upgrade().is_some_and(|tela| {
            tela.update(cx, |tela, _| {
                let vista = std::mem::take(&mut achar(tela).vista);
                vista && querer(tela)
            })
        });
        if !ainda {
            let fraca = fraca.clone();
            window.defer(cx, move |window, cx| {
                let _ = fraca.update(cx, |tela, cx| {
                    achar(tela).na_raiz = false;
                    // A tela pode voltar à vista querendo o diálogo.
                    cx.notify();
                });
                if window.has_active_dialog(cx) {
                    window.close_dialog(cx);
                }
            });
            return dialogo;
        }
        let conteudo = fraca
            .upgrade()
            .and_then(|tela| tela.update(cx, |tela, cx| montar(tela, window, cx)))
            .unwrap_or_else(|| gpui_kit::div().into_any_element());
        let ao_cancelar = ao_cancelar.clone();
        dialogo
            .w(px(jeito.largura))
            .p(px(16.))
            .rounded(px(12.))
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .close_button(jeito.x)
            .overlay_closable(jeito.veu)
            .keyboard(jeito.esc)
            .on_cancel(move |_, window, cx| {
                ao_cancelar(window, cx);
                false
            })
            .child(conteudo)
    });
}
