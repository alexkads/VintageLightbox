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

use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable as _, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, Div, FontWeight, Hsla, SharedString, Stateful,
};

use crate::recursos::Icone;

// 🔘 **Os botões são o `Button` do gpui-kit** desde 2026-09-26 (dono: trocar
// o que é desenhado à mão por componentes do kit, *"para reduzir a
// preocupação com UX"*). O de antes era um `div` com as medidas do shadcn; o
// `Button` traz o que o `div` não tinha: o desligado que **não clica** (o
// antigo só apagava), o foco pelo teclado, o carregando e a dica.
//
// As assinaturas ficaram: as telas continuam montando o conteúdo (ícone,
// rótulo, tecla) com `.child()` e chamando `.on_click()` no que voltar.

/// 🎯 **As medidas do site, e não as do gpui-kit.** O `Button` médio do kit
/// escreve em `text-base` (16 px); o do site é `text-sm` (14 px), com 32 px de
/// altura e 10 de respiro. O pequeno do kit já escreve em 14 — e a altura e o
/// respiro do site vão por cima, porque o estilo do botão vale depois do
/// tamanho. (Comparado lado a lado com as fotos de antes: o médio deixava
/// todo rótulo maior que o do site.)
fn botao(id: impl Into<SharedString>) -> Button {
    Button::new(id.into()).small().h(px(32.)).px(px(10.))
}

/// `Button variant="outline"`.
pub fn botao_contorno(id: impl Into<SharedString>, _cx: &App) -> Button {
    botao(id).outline()
}

/// `Button` padrão: a cor da marca.
pub fn botao_primario(id: impl Into<SharedString>, _cx: &App) -> Button {
    botao(id).primary()
}

/// `Button variant="destructive"`.
pub fn botao_perigo(id: impl Into<SharedString>, _cx: &App) -> Button {
    botao(id).danger()
}

/// `Button variant="ghost"`.
pub fn botao_fantasma(id: impl Into<SharedString>, _cx: &App) -> Button {
    botao(id).ghost()
}

/// Um botão desligado: o `disabled` do `Button`, que apaga **e não clica**.
/// Encadear depois do botão.
pub fn desligado(botao: Button, desligar: bool) -> Button {
    botao.disabled(desligar)
}

/// O `SidebarTrigger`: 28 px, fantasma, com o ícone do painel em 16.
pub fn botao_do_menu(id: impl Into<SharedString>, _cx: &App) -> Button {
    Button::new(id.into())
        .ghost()
        .small()
        .size(px(28.))
        .px(px(0.))
        .child(Icon::new(Icone::PanelLeft).size(px(16.)))
}

/// `Badge variant="outline"`: a `Tag` do gpui-kit, em pílula de 22 px com a
/// borda da página — as medidas do site por cima das do kit.
pub fn selo_contorno(_cx: &App) -> Tag {
    Tag::secondary()
        .outline()
        .rounded_full()
        .flex_none()
        .h(px(22.))
        .px(px(8.))
        .py(px(0.))
        .gap(px(6.))
        .font_weight(FontWeight::MEDIUM)
}

/// Um selo colorido: a `Tag` do gpui-kit com fundo, borda e texto próprios
/// (ver `tema::cores::selo_*`).
pub fn selo_colorido(cores: (Hsla, Hsla, Hsla)) -> Tag {
    let (fundo, borda, texto) = cores;
    Tag::custom(fundo, texto, borda)
        .rounded(px(6.))
        .flex_none()
        .h(px(20.))
        .px(px(6.))
        .py(px(0.))
        .font_weight(FontWeight::MEDIUM)
}

/// 🔑 **A tecla continua desenhada aqui, e não é o `Kbd` do gpui-kit.** O
/// `Kbd` impõe a cor dele (`muted_foreground` sobre o fundo) — e a tecla do
/// site herda a do botão em que está: no "Abrir caixa F8" escuro, o "F8" é
/// claro sobre o próprio botão. Com o `Kbd` viraria uma caixinha cinza dentro
/// do botão da marca, diferente da web (diretriz do dono: padronizar no kit
/// *"sem ficar diferente da versão Web"*).
/// A tecla ao lado do rótulo (`F2`, `F4`…).
pub fn tecla(texto: impl Into<SharedString>) -> Div {
    div()
        .px(px(4.))
        .rounded(px(4.))
        .border_1()
        .border_color(gpui_kit::rgba(0x80808066))
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

/// `Alert`: o do gpui-kit — o padrão já é o do site (texto, fundo da página,
/// borda); com `perigo`, o vermelho do texto e da borda, **sem** o fundo
/// tingido que o `error` do kit põe, porque o do site não tem. Respiro e canto
/// do site por cima dos do kit.
pub fn aviso(texto: impl Into<SharedString>, perigo: bool, cx: &App) -> Alert {
    let tema = cx.theme();
    let texto: SharedString = texto.into();
    let alerta = if perigo {
        Alert::error(texto.clone(), texto)
            .icon(Icon::new(Icone::CircleAlert).size(px(16.)))
            .bg(tema.background)
            .border_color(tema.danger.opacity(0.5))
    } else {
        Alert::new(texto.clone(), texto).icon(Icon::new(Icone::Info).size(px(16.)))
    };
    alerta.small().px(px(16.)).py(px(12.)).rounded(px(10.))
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
        .bg(gpui_kit::black().opacity(0.5))
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
