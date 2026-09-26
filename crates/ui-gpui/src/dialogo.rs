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
//! 🎯 **No meio da janela, como o site** (`top-1/2 -translate-y-1/2`). O kit
//! põe a caixa a um décimo do topo, e a única alavanca dele é o `margin_top`:
//! a altura da caixa é medida no quadro em que ela é desenhada e o respiro de
//! cima sai dela ([`Altura`]). No primeiro quadro, ainda sem medida, a caixa
//! nasce transparente, para não aparecer no alto e pular para o meio.
//!
//! O `Dialog` já se desenha `deferred`. No GPUI do gpui-kit 0.6 isso pode ir
//! dentro de outro `deferred` (ele desenha em rodadas, até 10 níveis) — o
//! pânico do gpui 0.2.2 com um dentro do outro virou teste de regressão —, mas
//! envolvê-lo em outro não serve para nada.

use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{ActiveTheme, Root};
use gpui_kit::{prelude::*, px, AnyElement, Context, Pixels, SharedString, Window};

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

/// A altura medida da caixa, guardada de um quadro para o outro: o miolo e o
/// rodapé, cada um no seu `on_children_prepainted`.
#[derive(Default, Clone, Copy, PartialEq)]
struct Altura {
    miolo: Option<Pixels>,
    rodape: Pixels,
}

/// O respiro que o kit guarda nas bordas da janela (`spacing_tokens().lg`).
const MARGEM: f32 = 16.;

/// O conteúdo do diálogo, lido da tela.
pub type Montar<T> = fn(&mut T, &mut Window, &mut Context<T>) -> Option<AnyElement>;
/// O que a tela faz quando o kit pede para fechar (`Esc`, clique fora, X).
pub type Cancelar<T> = fn(&mut T, &mut Window, &mut Context<T>);

/// O diálogo da tela, se ela o quer aberto. Chamar no `render` e pôr o
/// resultado na árvore dela.
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
    desenhar_conteudo(conteudo, None, jeito, cancelar, window, cx)
}

/// Como [`desenhar`], com o conteúdo já montado — para as telas em que ele sai
/// de um estado com dados (o ensaio aberto, a urgência escolhida).
///
/// `rodape` é a faixa de baixo do `AlertDialogFooter` do site (`bg-muted/50`,
/// borda em cima, de ponta a ponta): no espaço de rodapé do `Dialog`, porque
/// dentro do corpo o kit a cortaria nos 16 px de respiro.
pub fn desenhar_conteudo<T: 'static>(
    conteudo: Option<AnyElement>,
    rodape: Option<AnyElement>,
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
    // Um estado por diálogo da tela: a caixa do PDV e a pergunta por cima dela
    // são dois, com larguras e rodapés diferentes.
    let chave = SharedString::from(format!(
        "dialogo-altura-{}-{}-{}",
        cx.entity_id(),
        jeito.largura,
        rodape.is_some()
    ));
    let altura = window.use_keyed_state(chave, cx, |_, _| Altura::default());
    let medida = *altura.read(cx);
    let medir = |altura: &gpui_kit::Entity<Altura>, qual: fn(&mut Altura, Pixels)| {
        let altura = altura.clone();
        move |filhos: Vec<gpui_kit::Bounds<Pixels>>, _: &mut Window, cx: &mut gpui_kit::App| {
            let alto = filhos.iter().map(|b| b.size.height).sum::<Pixels>();
            altura.update(cx, |altura, cx| {
                let antes = *altura;
                qual(altura, alto);
                if *altura != antes {
                    cx.notify();
                }
            });
        }
    };
    // A caixa: o miolo, o `gap` de 8 do kit antes do rodapé, e a borda de 1.
    let topo = medida.miolo.map(|miolo| {
        let caixa = miolo
            + if rodape.is_some() {
                medida.rodape + px(8.)
            } else {
                px(0.)
            }
            + px(2.);
        ((window.viewport_size().height - caixa) / 2.).max(px(MARGEM))
    });
    // 🚨 **O clique fora é nosso, e não do kit.** O véu do kit só fecha o
    // diálogo "de cima", e ele descobre qual é contando os diálogos guardados
    // na `Root` (`layer_ix + 1 == active_dialogs.len()`): desenhado pela tela,
    // a conta dá zero e o clique no véu era engolido sem fechar nada (o
    // `o_veu_fecha_o_dialogo_e_a_caixa_nao` pegou). Por isso a caixa do kit
    // fica sem respiro, o respiro vai no invólucro do conteúdo — que passa a
    // ter o tamanho exato da caixa — e o clique fora dele pede o cancelar.
    let miolo = gpui_kit::div()
        .p(px(16.))
        .child(
            gpui_kit::div()
                .on_children_prepainted(medir(&altura, |a, alto| a.miolo = Some(alto + px(32.))))
                .child(conteudo),
        )
        .when(jeito.veu, |miolo| {
            miolo.on_mouse_down_out(move |_, window, cx| pedir_cancelar(window, cx))
        });
    Some(
        Dialog::new(cx)
            .w(px(jeito.largura))
            .p(px(0.))
            // O `rounded-xl` do `DialogContent` do site: 14 px com o
            // `--radius` de 10.
            .rounded(px(14.))
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
            .when_some(topo, |dialogo, topo| dialogo.margin_top(topo))
            .when(topo.is_none(), |dialogo| dialogo.opacity(0.))
            .when_some(rodape, |dialogo, rodape| {
                dialogo.footer(
                    gpui_kit::div()
                        .w_full()
                        .on_children_prepainted(medir(&altura, |a, alto| a.rodape = alto))
                        .child(rodape),
                )
            })
            .into_any_element(),
    )
}

/// O miolo do `useConfirmacao` do site: `AlertDialogTitle` (`text-base
/// font-medium`) e `AlertDialogDescription` (`text-sm`, apagado).
pub fn miolo_da_pergunta(
    id: &'static str,
    titulo: impl Into<SharedString>,
    descricao: impl Into<SharedString>,
    cx: &gpui_kit::App,
) -> AnyElement {
    gpui_kit::component::v_flex()
        .id(id)
        .debug_selector(move || id.into())
        .gap(px(6.))
        .child(
            gpui_kit::div()
                .text_size(px(16.))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .child(titulo.into()),
        )
        .child(
            gpui_kit::div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(descricao.into()),
        )
        .into_any_element()
}

/// A faixa do `AlertDialogFooter` (`bg-muted/50 border-t`, de ponta a ponta,
/// com o canto de baixo da caixa): os botões vão dentro, à direita. Vai no
/// `rodape` de [`desenhar_conteudo`].
pub fn rodape_da_pergunta(cx: &gpui_kit::App) -> gpui_kit::Div {
    let tema = cx.theme();
    crate::estilo::rodape_do_dialogo()
        .w_full()
        .p(px(16.))
        .border_t_1()
        .border_color(tema.border)
        .bg(tema.muted.opacity(0.5))
        .rounded_b(px(14.))
}

/// O `DialogHeader` do site com o X do `DialogContent` (`opacity-70`, 16 px),
/// para os diálogos em que o X tem nome próprio — os testes o acham pelo
/// `debug_selector`, e o do kit não tem um.
pub fn cabecalho_com_x(
    id: &'static str,
    titulo: impl Into<SharedString>,
    fechar: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
    cx: &gpui_kit::App,
) -> gpui_kit::Div {
    let acento = cx.theme().accent;
    gpui_kit::component::h_flex()
        .items_start()
        .gap(px(8.))
        .child(
            gpui_kit::div()
                .flex_1()
                .min_w(px(0.))
                .text_size(px(16.))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .child(titulo.into()),
        )
        .child(
            gpui_kit::div()
                .id(id)
                .debug_selector(move || id.into())
                .flex_none()
                .size(px(20.))
                .rounded(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .opacity(0.7)
                .cursor_pointer()
                .hover(move |s| s.opacity(1.).bg(acento))
                .child(gpui_kit::component::Icon::new(crate::recursos::Icone::X).size(px(16.)))
                .on_click(fechar),
        )
}
