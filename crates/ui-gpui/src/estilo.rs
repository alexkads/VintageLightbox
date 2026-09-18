//! As peças do shadcn do site, com as mesmas medidas, para as telas do app.
//!
//! 🔑 **Uma peça, um lugar.** O botão de contorno do site tem 32 px de altura,
//! canto de 8 px e `shadow-xs`; escrito à mão em cada tela, ele diverge na
//! terceira. As telas montam o conteúdo (ícone, rótulo, tecla) e chamam
//! `on_click` no que estas funções devolvem.
//!
//! | Site | Aqui |
//! |---|---|
//! | `Button variant="outline" size="sm"` | [`botao_contorno`] |
//! | `Button` (padrão, a marca) | [`botao_primario`] |
//! | `Button variant="destructive"` | [`botao_perigo`] |
//! | `Button variant="ghost"` | [`botao_fantasma`] |
//! | `SidebarTrigger` | [`botao_do_menu`] |
//! | `Badge variant="outline"` | [`selo_contorno`] |
//! | `<kbd>` das teclas F do caixa | [`tecla`] |
//! | `CabecalhoDaPagina` | [`cabecalho_da_pagina`] |
//! | `Alert` | [`aviso`] |
//! | `Dialog` (véu, caixa, cabeçalho, opção, rodapé) | [`veu_do_dialogo`], [`caixa_do_dialogo`], [`cabecalho_do_dialogo`], [`opcao_do_dialogo`], [`rodape_do_dialogo`] |

use gpui::{div, prelude::*, px, AnyElement, App, Div, FontWeight, Hsla, SharedString, Stateful};
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon};

use crate::recursos::Icone;

/// `Button variant="outline"`: 32 px, borda do campo, fundo da página.
pub fn botao_contorno(id: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    let tema = cx.theme();
    let (borda, fundo, acento) = (tema.input, tema.background, tema.accent);
    base(id)
        .border_1()
        .border_color(borda)
        .bg(fundo)
        .shadow_xs()
        .hover(move |s| s.bg(acento))
}

/// `Button` padrão: a cor da marca.
pub fn botao_primario(id: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    let tema = cx.theme();
    let (fundo, texto, pairando) = (tema.primary, tema.primary_foreground, tema.primary_hover);
    base(id)
        .bg(fundo)
        .text_color(texto)
        .font_weight(FontWeight::MEDIUM)
        .hover(move |s| s.bg(pairando))
}

/// `Button variant="destructive"`: vermelho claro, texto vermelho.
pub fn botao_perigo(id: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    let perigo = cx.theme().danger;
    base(id)
        .bg(perigo.opacity(0.2))
        .text_color(perigo)
        .font_weight(FontWeight::MEDIUM)
        .hover(move |s| s.bg(perigo.opacity(0.3)))
}

/// `Button variant="ghost"`.
pub fn botao_fantasma(id: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    let acento = cx.theme().accent;
    base(id).hover(move |s| s.bg(acento))
}

/// Um botão desligado: sem ponteiro e meio apagado. Encadear depois do botão.
pub fn desligado(botao: Stateful<Div>, desligar: bool) -> Stateful<Div> {
    if desligar {
        botao.opacity(0.5).cursor_default()
    } else {
        botao
    }
}

fn base(id: impl Into<SharedString>) -> Stateful<Div> {
    h_flex()
        .id(id.into())
        .flex_none()
        .h(px(32.))
        .px(px(10.))
        .gap(px(6.))
        .rounded(px(8.))
        .justify_center()
        .text_sm()
        .whitespace_nowrap()
        .cursor_pointer()
}

/// O `SidebarTrigger`: 28 px, fantasma, com o ícone do painel.
pub fn botao_do_menu(id: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    let tema = cx.theme();
    let (apagado, acento, texto) = (tema.muted_foreground, tema.accent, tema.foreground);
    div()
        .id(id.into())
        .flex_none()
        .size(px(28.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(apagado)
        .hover(move |s| s.bg(acento).text_color(texto))
        .child(Icon::new(Icone::PanelLeft).size(px(16.)))
}

/// `Badge variant="outline"`: pílula de 20 px com borda.
pub fn selo_contorno(cx: &App) -> Div {
    let tema = cx.theme();
    h_flex()
        .flex_none()
        .h(px(22.))
        .px(px(8.))
        .gap(px(6.))
        .rounded_full()
        .border_1()
        .border_color(tema.border)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
}

/// Um selo colorido: fundo, borda e texto (ver `tema::cores::selo_*`).
pub fn selo_colorido(cores: (Hsla, Hsla, Hsla)) -> Div {
    let (fundo, borda, texto) = cores;
    h_flex()
        .flex_none()
        .h(px(20.))
        .px(px(6.))
        .rounded(px(6.))
        .border_1()
        .border_color(borda)
        .bg(fundo)
        .text_color(texto)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
}

/// A tecla ao lado do rótulo (`F2`, `F4`…).
pub fn tecla(texto: impl Into<SharedString>) -> Div {
    div()
        .px(px(4.))
        .rounded(px(4.))
        .border_1()
        .border_color(gpui::rgba(0x80808066))
        .text_size(px(10.))
        .opacity(0.7)
        .child(texto.into())
}

/// O cabeçalho de uma página do painel: título grande, descrição, selos e
/// ações à direita.
pub fn cabecalho_da_pagina(
    titulo: impl Into<SharedString>,
    descricao: Option<AnyElement>,
    depois: Option<AnyElement>,
    acoes: Option<AnyElement>,
    cx: &App,
) -> Div {
    let apagado = cx.theme().muted_foreground;
    h_flex()
        .w_full()
        .items_start()
        .gap(px(16.))
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.))
                .gap(px(4.))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(titulo.into()),
                )
                .when_some(descricao, |d, descricao| {
                    d.child(div().text_sm().text_color(apagado).child(descricao))
                })
                .when_some(depois, |d, depois| {
                    d.child(h_flex().pt(px(4.)).gap(px(8.)).child(depois))
                }),
        )
        .when_some(acoes, |d, acoes| {
            d.child(h_flex().flex_none().gap(px(8.)).child(acoes))
        })
}

/// `Alert`: uma faixa com borda; `perigo` pinta de vermelho.
pub fn aviso(texto: impl Into<SharedString>, perigo: bool, cx: &App) -> Div {
    let tema = cx.theme();
    let cor = if perigo { tema.danger } else { tema.foreground };
    h_flex()
        .w_full()
        .gap(px(8.))
        .px(px(16.))
        .py(px(12.))
        .rounded(px(10.))
        .border_1()
        .border_color(if perigo {
            cor.opacity(0.5)
        } else {
            tema.border
        })
        .bg(tema.background)
        .text_sm()
        .text_color(cor)
        .child(
            Icon::new(if perigo {
                Icone::CircleAlert
            } else {
                Icone::Info
            })
            .size(px(16.)),
        )
        .child(div().flex_1().child(texto.into()))
}

/// O véu do `Dialog` do site, e a caixa dele — as duas peças de um diálogo.
///
/// # 🎨 O que o shadcn desenha, e o que dá para reproduzir aqui
///
/// O `DialogContent` do site é `rounded-xl bg-popover p-4 gap-4 ring-1
/// ring-foreground/10` sobre um `DialogOverlay` de `bg-black/10` **com
/// `backdrop-blur`**. O borrão é o que permite ao véu ser tão claro: ele
/// separa o diálogo do fundo sem escurecer a tela.
///
/// ⚠️ **O GPUI não tem `backdrop-filter`.** Com 10% e sem borrão, o fundo
/// continua nítido e a caixa parece solta no meio da tela. O véu daqui é mais
/// escuro de propósito — é a mesma separação, pelo único meio disponível.
pub fn veu_do_dialogo() -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::black().opacity(0.5))
        // 🚨 **O véu para o mouse aqui, e é o que faz dele um véu** (dono,
        // 18/set/2026: *"tem que selecionar o estúdio e ficar na listagem de
        // sessões, e não abrir uma sessão"*). No GPUI, desenhar por cima não
        // bloqueia o clique: sem `occlude`, o clique atravessava a caixa e
        // chegava à linha da tabela por baixo — escolher o estúdio abria a
        // sessão que estivesse atrás do botão.
        .occlude()
}

/// A caixa do `DialogContent`: canto de 12 px, fundo do `popover`, 16 px de
/// respiro e 16 px entre as partes.
pub fn caixa_do_dialogo(cx: &App) -> Div {
    let tema = cx.theme();
    v_flex()
        .w(px(440.))
        .p(px(16.))
        .gap(px(16.))
        .rounded(px(12.))
        .border_1()
        .border_color(tema.border)
        .bg(tema.popover)
        .text_color(tema.popover_foreground)
        .shadow_lg()
}

/// O `DialogHeader`: título (`text-lg font-semibold`) e descrição
/// (`text-sm text-muted-foreground`), com 8 px entre eles.
pub fn cabecalho_do_dialogo(
    titulo: impl Into<SharedString>,
    descricao: impl Into<SharedString>,
    icone: Option<Icone>,
    cx: &App,
) -> Div {
    let apagado = cx.theme().muted_foreground;
    v_flex()
        .gap(px(8.))
        .child(
            h_flex()
                .gap(px(8.))
                .items_center()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .children(icone.map(|i| Icon::new(i).size(px(18.))))
                .child(titulo.into()),
        )
        .child(
            div()
                .text_sm()
                .text_color(apagado)
                // 🔑 A descrição **quebra linha**: ela explica o gesto, e
                // `whitespace_nowrap` (o padrão dos botões) a cortaria.
                .child(descricao.into()),
        )
}

/// Uma opção de escolha dentro de um diálogo — o `Button variant="outline"` do
/// site com `h-auto justify-start py-3 text-left`.
///
/// 🚨 **Não é [`botao_contorno`]**: aquele tem 32 px de altura fixa, conteúdo
/// centrado e `whitespace_nowrap`, que é o certo para uma barra e o errado para
/// uma lista de opções com duas linhas — o nome e o lugar ficavam espremidos no
/// meio (dono, 18/set/2026: *"precisa parecer shadcn"*).
pub fn opcao_do_dialogo(id: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    let tema = cx.theme();
    let (borda, fundo, acento) = (tema.input, tema.background, tema.accent);
    div()
        .id(id.into())
        .flex()
        .flex_col()
        .items_start()
        .gap(px(2.))
        .w_full()
        .px(px(12.))
        // `py-3` do site: 12 px em cima e embaixo, e a altura sai do conteúdo.
        .py(px(12.))
        .rounded(px(8.))
        .border_1()
        .border_color(borda)
        .bg(fundo)
        .shadow_xs()
        .text_sm()
        .cursor_pointer()
        .hover(move |s| s.bg(acento))
}

/// A linha dos botões do fim do diálogo (`DialogFooter`): à direita, 8 px entre
/// eles.
pub fn rodape_do_dialogo() -> Div {
    h_flex().justify_end().gap(px(8.))
}

/// Um cartão (`rounded-lg border`), com o fundo da página.
pub fn cartao(cx: &App) -> Div {
    let tema = cx.theme();
    div()
        .rounded(px(10.))
        .border_1()
        .border_color(tema.border)
        .overflow_hidden()
}
