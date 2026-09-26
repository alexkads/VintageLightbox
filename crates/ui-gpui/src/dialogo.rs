//! 🪟 Os diálogos das telas no `Dialog` do gpui-kit.
//!
//! # Desenhado pela tela, e não aberto na `Root`
//!
//! Diretriz do dono em 2026-09-26: padronizar no gpui-kit, *"sem ficar
//! diferente da versão Web"*. Os diálogos do app são **declarativos**: cada
//! tela os desenha a partir do próprio estado (`escolhendo_estudio`,
//! `restaurar: Option<…>`…), e o contrato de foco (`crate::modal::Modal`) põe
//! o foco no campo no mesmo gesto que abre.
//!
//! O `Dialog` do kit 0.6.6 não tem modo controlado, e o caminho dele —
//! `window.open_dialog` — guarda o diálogo na `Root`. A primeira tentativa foi
//! uma ponte entre o estado da tela e a `Root`, e ela custou caro: o diálogo
//! aparecia um quadro depois do gesto (o foco pedido caía num campo que ainda
//! não existia), a `Root` devolvia o foco ao fechar por cima do contrato da
//! tela, e o diálogo ficava aberto quando a tela saía da vista.
//!
//! Mas o `Dialog` é um elemento comum: ele mesmo se desenha num `deferred`
//! ancorado no canto da janela, com o véu na janela inteira. A `Root` só o
//! guarda. Então a tela o desenha **no lugar**, a partir do estado, como
//! desenhava o véu e a caixa dela — o mesmo quadro, o mesmo foco, e ele some
//! com a tela.
//!
//! # O que é do kit e o que é do site
//!
//! Do kit: o véu, a caixa, o X e o `Esc` (pedem o `Cancelar` da tela, e o kit
//! não fecha nada sozinho). O clique fora é da tela — ver [`desenhar_conteudo`]. Do site, por cima: o
//! respiro de 16 (`p-4 gap-4`), o canto de 12, o fundo do `popover` (o kit usa
//! o da página, que no escuro é outro) e a largura.
//!
//! ⚠️ **O `Dialog` já é `deferred`**: ele não pode ir dentro de outro `deferred`
//! (o GPUI recusa um dentro do outro).

use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{ActiveTheme, Root};
use gpui_kit::{prelude::*, px, AnyElement, Context, Window};

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

/// O conteúdo do diálogo, lido da tela.
pub type Montar<T> = fn(&mut T, &mut Window, &mut Context<T>) -> Option<AnyElement>;
/// O que a tela faz quando o kit pede para fechar (`Esc`, clique fora, X).
pub type Cancelar<T> = fn(&mut T, &mut Window, &mut Context<T>);

/// O diálogo da tela, se ela o quer aberto. Chamar no `render` e pôr o
/// resultado na árvore dela — **fora** de qualquer `deferred`.
pub fn desenhar<T: 'static>(
    tela: &mut T,
    quer: bool,
    jeito: Jeito,
    montar: Montar<T>,
    cancelar: Cancelar<T>,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Option<AnyElement> {
    if !quer {
        return None;
    }
    let conteudo = montar(tela, window, cx);
    desenhar_conteudo(conteudo, jeito, cancelar, window, cx)
}

/// Como [`desenhar`], com o conteúdo já montado — para as telas em que ele sai
/// de um estado com dados (o ensaio aberto, a urgência escolhida).
pub fn desenhar_conteudo<T: 'static>(
    conteudo: Option<AnyElement>,
    jeito: Jeito,
    cancelar: Cancelar<T>,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Option<AnyElement> {
    let conteudo = conteudo?;
    // Sem a `Root` do kit (os testes que montam uma tela sozinha), o `Dialog`
    // não desenha — ele lê a `Root`. Ali vale o véu e a caixa do `estilo.rs`.
    if window.root::<Root>().flatten().is_none() {
        return Some(
            crate::estilo::veu_do_dialogo()
                .child(
                    crate::estilo::caixa_do_dialogo(cx)
                        .w(px(jeito.largura))
                        .child(conteudo),
                )
                .into_any_element(),
        );
    }
    let fraca = cx.entity().downgrade();
    let pedir_cancelar = move |window: &mut Window, cx: &mut gpui_kit::App| {
        let _ = fraca.update(cx, |tela, cx| {
            cancelar(tela, window, cx);
            cx.notify();
        });
    };
    let ao_cancelar = pedir_cancelar.clone();
    // 🚨 **O clique fora é nosso, e não do kit.** O véu do kit só fecha o
    // diálogo "de cima", e ele descobre qual é contando os diálogos guardados
    // na `Root` (`layer_ix + 1 == active_dialogs.len()`): desenhado pela tela,
    // a conta dá zero e o clique no véu era engolido sem fechar nada (o
    // `o_veu_fecha_o_dialogo_e_a_caixa_nao` pegou). Por isso a caixa do kit
    // fica sem respiro, o respiro vai no invólucro do conteúdo — que passa a
    // ter o tamanho exato da caixa — e o clique fora dele pede o cancelar.
    let miolo = gpui_kit::div()
        .p(px(16.))
        .child(conteudo)
        .when(jeito.veu, |miolo| {
            miolo.on_mouse_down_out(move |_, window, cx| pedir_cancelar(window, cx))
        });
    Some(
        Dialog::new(cx)
            .w(px(jeito.largura))
            .p(px(0.))
            .rounded(px(12.))
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .close_button(jeito.x)
            .overlay_closable(false)
            .keyboard(jeito.esc)
            // O kit não fecha nada sozinho: quem fecha é o estado da tela.
            .on_cancel(move |_, window, cx| {
                ao_cancelar(window, cx);
                false
            })
            .child(miolo)
            .into_any_element(),
    )
}
