//! As peças do shadcn do site, com as mesmas medidas, para as telas do app.
//!
//! 🎨 **As medidas são as do estilo do template** (`tema::medidas`): a altura
//! do botão, o respiro do diálogo e os cantos mudam com o `estilo` e o `raio`
//! do `template.toml`. O visual da casa é o `nova` do site, com os números de
//! sempre (32 px de botão, 16 de respiro, 12 de canto no diálogo).
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
use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle, ToggleVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme, Disableable as _, Icon, Selectable as _, Sizable as _,
};
use gpui_kit::{
    div, prelude::*, px, Anchor, AnyElement, App, Div, FontWeight, Hsla, SharedString, Stateful,
    Window,
};

use crate::recursos::Icone;
use crate::tema::fontes;
use crate::tema::letra::em;
use crate::tema::medidas::{Canto, Medidas};

/// As medidas do estilo do template (ver `tema::medidas`).
fn m() -> &'static Medidas {
    crate::tema::medidas()
}

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
    Button::new(id.into())
        .small()
        .h(em(m().botao.altura))
        .px(em(m().botao.lados))
}

/// `Button variant="outline"`.
pub fn botao_contorno(id: impl Into<SharedString>, _cx: &App) -> Button {
    botao(id).outline()
}

/// 📏 **O `Button size="sm"` do template** — as barras densas, como as três da
/// sessão, na escala da barra da Revelação (dono, 28/09/2026: *"eles são
/// maiores em relação à tela de revelação"*). Altura, respiro e letra saem de
/// `Medidas::botao_pequeno`: 28 px e 12,8 no visual da casa.
///
/// ⚠️ **A letra vem do tamanho do kit, e não de `text_size`**: o `Button`
/// escreve o rótulo num conteúdo interno com o `button_text_size` do tamanho
/// (`small` = 14, `xsmall` = 12) e ignora o `text_size` de fora — a primeira
/// versão pedia 12,8 e saía 14, do tamanho do botão normal. O `xsmall` dá 12,
/// a letra das pílulas que a Revelação tinha; altura e respiro são os do
/// template.
fn botao_pequeno(id: impl Into<SharedString>) -> Button {
    let c = m().botao_pequeno;
    Button::new(id.into())
        .xsmall()
        .h(em(c.altura))
        .px(em(c.lados))
}

/// [`botao_contorno`] no tamanho pequeno.
pub fn botao_contorno_pequeno(id: impl Into<SharedString>, _cx: &App) -> Button {
    botao_pequeno(id).outline()
}

/// [`botao_primario`] no tamanho pequeno.
pub fn botao_primario_pequeno(id: impl Into<SharedString>, _cx: &App) -> Button {
    botao_pequeno(id).primary()
}

/// [`botao_secundario`] no tamanho pequeno.
pub fn botao_secundario_pequeno(id: impl Into<SharedString>, _cx: &App) -> Button {
    botao_pequeno(id).secondary()
}

/// Um campo (`Select`, `Input`) na altura do botão pequeno, para ficar
/// alinhado a ele numa barra densa.
pub fn campo_pequeno<E: Styled>(elemento: E) -> E {
    elemento.h(em(m().botao_pequeno.altura))
}

/// `Button` padrão: a cor da marca.
pub fn botao_primario(id: impl Into<SharedString>, _cx: &App) -> Button {
    botao(id).primary()
}

/// `Button variant="secondary"`: o fundo apagado, sem borda.
pub fn botao_secundario(id: impl Into<SharedString>, _cx: &App) -> Button {
    botao(id).secondary()
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

/// O `SidebarTrigger` (`Button size="icon-sm"`): fantasma, com o ícone do
/// painel em 16 — 28 px no visual da casa.
pub fn botao_do_menu(id: impl Into<SharedString>, _cx: &App) -> Button {
    botao_icone(id, Icone::PanelLeft, m().botao_icone, 16.)
}

/// 📏 **Um campo na altura do template** (`Input`, `Select`): a mesma do
/// botão, para campo e botão lado a lado ficarem alinhados (dono,
/// 28/09/2026: *"não deixe os tamanhos fora de padrão"*). O `xsmall` das
/// linhas densas (filtros, preço na grade) é escolha, e fica.
pub fn campo<E: Styled>(elemento: E) -> E {
    elemento.h(em(m().campo.altura))
}

/// O botão só de ícone na altura do botão do template — o que fica ao lado
/// dos botões com texto de uma barra.
pub fn botao_icone_padrao(id: impl Into<SharedString>, icone: Icone) -> Button {
    botao_icone(id, icone, m().botao.altura, 16.)
}

/// O botão só de ícone no lado do `SidebarTrigger` (`size="icon-sm"`) — o
/// "voltar" que mora ao lado dele num cabeçalho.
pub fn botao_icone_pequeno(id: impl Into<SharedString>, icone: Icone) -> Button {
    botao_icone(id, icone, m().botao_icone, 16.)
}

/// `Button variant="ghost" size="icon"`: quadrado de `lado` px, só o ícone.
/// É o X dos diálogos, as setas e os "voltar" dos cabeçalhos.
pub fn botao_icone(id: impl Into<SharedString>, icone: Icone, lado: f32, icone_px: f32) -> Button {
    Button::new(id.into())
        .ghost()
        .small()
        .size(px(lado))
        .px(px(0.))
        .child(Icon::new(icone).size(px(icone_px)))
}

/// Um item clicável de barra (o rodapé, os links pequenos): o `Button`
/// fantasma do kit em 20 px e `text-xs`, sem a altura de 32 do botão do site.
/// Cor e fundo próprios vão por cima, como no resto do kit.
pub fn botao_raso(id: impl Into<SharedString>) -> Button {
    Button::new(id.into())
        .ghost()
        .xsmall()
        .h(px(20.))
        .px(px(6.))
        .rounded(crate::tema::canto(4.))
}

/// `Badge variant="outline"`: a `Tag` do gpui-kit, em pílula de 22 px com a
/// borda da página — as medidas do site por cima das do kit.
pub fn selo_contorno(_cx: &App) -> Tag {
    let selo = m().selo;
    Tag::secondary()
        .outline()
        .rounded(crate::tema::no_tema(px(m().canto(selo.canto))))
        .flex_none()
        .h(px(selo.altura + 2.))
        .px(px(selo.lados))
        .py(px(0.))
        .gap(px(6.))
        .font_weight(FontWeight::MEDIUM)
}

/// Um selo colorido: a `Tag` do gpui-kit com fundo, borda e texto próprios
/// (ver `tema::cores::selo_*`).
pub fn selo_colorido(cores: (Hsla, Hsla, Hsla)) -> Tag {
    let (fundo, borda, texto) = cores;
    Tag::custom(fundo, texto, borda)
        .rounded(crate::tema::canto(6.))
        .flex_none()
        .h(px(m().selo.altura))
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
        .rounded(crate::tema::canto(4.))
        .border_1()
        .border_color(gpui_kit::rgba(0x80808066))
        .text_size(em(10.))
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
                        .when_some(fontes::dos_titulos(), |d, f| d.font_family(f))
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

/// O tipo de um aviso passageiro — ver [`toast`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toast {
    /// Deu certo: verde, no alto e no meio.
    Sucesso,
    /// O site (ou o disco) recusou: vermelho, no alto e no meio.
    Erro,
    /// O gesto deixou algo de fora: o alerta do kit, no canto de baixo.
    Alerta,
}

/// Um aviso do que **acabou de acontecer** — a `Notification` do gpui-kit, que
/// some sozinha em 5 s. O que persiste (a tela que não carregou, o campo
/// recusado) é [`aviso`], na tela.
///
/// 🎨 **Sucesso e erro com as cores do `richColors` do site**, no alto e no
/// meio como o `sonner` (`position="top-center"`): no canto superior direito,
/// o padrão da lista, o aviso tapava "Tela do cliente" e "Baixar JPEG". O
/// alerta vai para o canto de baixo, longe dos botões do alto.
pub fn toast(texto: impl Into<SharedString>, tipo: Toast, cx: &App) -> Notification {
    let texto: SharedString = texto.into();
    let tema = cx.theme();
    let (fundo, letra, icone) = match tipo {
        Toast::Alerta => {
            return Notification::warning(texto).placement(Anchor::BottomRight);
        }
        Toast::Sucesso => (
            tema.success,
            tema.success_foreground,
            gpui_kit::component::IconName::CircleCheck,
        ),
        Toast::Erro => (
            tema.danger,
            tema.danger_foreground,
            gpui_kit::component::IconName::CircleX,
        ),
    };
    // O erro do site chega cru (`o site respondeu 500 …: <html>`).
    let texto: SharedString = if tipo == Toast::Erro {
        crate::erro_da_api::legivel(&texto).into()
    } else {
        texto
    };
    Notification::new()
        .message(texto)
        .icon(Icon::new(icone).text_color(letra))
        .placement(Anchor::TopCenter)
        .bg(fundo)
        .border_color(fundo)
        .text_color(letra)
}

/// Mostra o `toast` na janela — se ela tiver a raiz do kit, que é onde mora a
/// lista das notificações. As janelas dos testes de uma tela só não têm, e o
/// `push_notification` do kit entra em pânico sem ela.
pub fn mostrar_toast(nota: Notification, window: &mut Window, cx: &mut App) {
    use gpui_kit::component::WindowExt as _;
    if window
        .root::<gpui_kit::component::Root>()
        .flatten()
        .is_some()
    {
        window.push_notification(nota, cx);
    }
}

/// 🍞 Um erro que a tela guarda (`erro: Option<…>`) vira **toast do kit uma
/// vez**, quando aparece ou muda — e não uma faixa fixa na página (dono,
/// 28/09/2026: *"Tem que usar o Toast do GPUI Kit"*). `visto` é a memória da
/// tela do último mostrado. Chamar no `render`; o toast sai depois do quadro.
pub fn toast_quando_mudar(
    visto: &mut Option<SharedString>,
    texto: Option<SharedString>,
    tipo: Toast,
    window: &mut Window,
    cx: &mut App,
) {
    if *visto == texto {
        return;
    }
    visto.clone_from(&texto);
    if let Some(texto) = texto {
        let nota = toast(texto, tipo, cx);
        window.defer(cx, move |window, cx| mostrar_toast(nota, window, cx));
    }
}

/// `Alert`: o do gpui-kit — o padrão já é o do site (texto, fundo da página,
/// borda); com `perigo`, o vermelho do texto e da borda, **sem** o fundo
/// tingido que o `error` do kit põe, porque o do site não tem. Respiro e canto
/// do site por cima dos do kit.
pub fn aviso(texto: impl Into<SharedString>, perigo: bool, cx: &App) -> Alert {
    let tema = cx.theme();
    let texto: SharedString = texto.into();
    // O erro do site chega cru (`o site respondeu 500 …: <html>`); aqui ele
    // vira a frase que o operador lê (`crate::erro_da_api`).
    let texto: SharedString = if perigo {
        crate::erro_da_api::legivel(&texto).into()
    } else {
        texto
    };
    let alerta = if perigo {
        Alert::error(texto.clone(), texto)
            .icon(Icon::new(Icone::CircleAlert).size(px(16.)))
            .bg(tema.background)
            .border_color(tema.danger.opacity(0.5))
    } else {
        Alert::new(texto.clone(), texto).icon(Icon::new(Icone::Info).size(px(16.)))
    };
    alerta
        .small()
        .px(px(16.))
        .py(px(12.))
        .rounded(crate::tema::no_tema(px(m().canto(Canto::LG))))
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

/// O miolo de um diálogo, **sem moldura**: o arranjo da caixa (16 px entre as
/// partes). A moldura — véu, caixa, X — é o `Dialog` do gpui-kit
/// (`crate::dialogo`).
pub fn conteudo_do_dialogo() -> Div {
    v_flex().gap(px(16.))
}

/// A caixa do `DialogContent`: fundo do `popover`, com o respiro, o vão e o
/// canto do estilo (16, 16 e `rounded-xl` no visual da casa).
///
/// A largura fica de fora do template: o conteúdo de cada diálogo do app foi
/// escrito para 440 px, e o `max-w-sm`/`max-w-md` do shadcn o quebraria.
pub fn caixa_do_dialogo(cx: &App) -> Div {
    let tema = cx.theme();
    let dialogo = m().dialogo;
    v_flex()
        .w(px(440.))
        .p(px(dialogo.respiro))
        .gap(px(dialogo.vao))
        .rounded(crate::tema::no_tema(px(m().canto(dialogo.canto))))
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
                .when_some(fontes::dos_titulos(), |d, f| d.font_family(f))
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
        .rounded(crate::tema::no_tema(px(m().canto(m().botao.canto))))
        .border_1()
        .border_color(borda)
        .bg(fundo)
        .shadow_xs()
        .text_size(em(m().letra))
        .cursor_pointer()
        .hover(move |s| s.bg(acento))
}

/// A linha dos botões do fim do diálogo (`DialogFooter`): à direita, 8 px entre
/// eles.
pub fn rodape_do_dialogo() -> Div {
    h_flex().justify_end().gap(px(8.))
}

/// Um cartão (`Card`), com o fundo da página e o canto do estilo.
pub fn cartao(cx: &App) -> Div {
    let tema = cx.theme();
    div()
        .rounded(crate::tema::no_tema(px(m().canto(m().cartao.canto))))
        .border_1()
        .border_color(tema.border)
        .overflow_hidden()
}

/// Um item do `PopupMenu` do gpui-kit **com nome**: o rótulo vai num `div`
/// com `debug_selector`, para os testes o acharem e clicarem como antes (o
/// item do kit não tem nome próprio). O clique no rótulo sobe até o item, que
/// fecha o menu e chama o `on_click`.
///
/// 📏 O rótulo ocupa a linha inteira: o ✓ de `check_side(Side::Right)` vem
/// depois dele, e com o rótulo do tamanho do texto o ✓ ficava colado na
/// palavra em vez de na borda do menu.
pub fn item_de_menu(
    id: &'static str,
    rotulo: impl Into<SharedString>,
    cor: Option<Hsla>,
) -> gpui_kit::component::menu::PopupMenuItem {
    linha_de_menu(id, None, rotulo.into(), cor, None)
}

/// O item com ícone do `DropdownMenuItem` do site: o ícone de 16 px
/// (`[&_svg]:size-4`) e o vão do template entre ele e o rótulo. O
/// `.icon(..)` do kit desenha o ícone em 12 px, colado no texto.
pub fn item_de_menu_com_icone(
    id: &'static str,
    icone: Icone,
    rotulo: impl Into<SharedString>,
    cor: Option<Hsla>,
) -> gpui_kit::component::menu::PopupMenuItem {
    linha_de_menu(id, Some(icone), rotulo.into(), cor, None)
}

/// O item com ícone e um texto apagado no fim (o `DropdownMenuShortcut`):
/// a versão ao lado de "Verificar atualizações".
pub fn item_de_menu_com_fim(
    id: &'static str,
    icone: Icone,
    rotulo: impl Into<SharedString>,
    fim: impl Into<SharedString>,
) -> gpui_kit::component::menu::PopupMenuItem {
    linha_de_menu(id, Some(icone), rotulo.into(), None, Some(fim.into()))
}

fn linha_de_menu(
    id: &'static str,
    icone: Option<Icone>,
    rotulo: SharedString,
    cor: Option<Hsla>,
    fim: Option<SharedString>,
) -> gpui_kit::component::menu::PopupMenuItem {
    gpui_kit::component::menu::PopupMenuItem::element(move |_, cx| {
        let apagado = cx.theme().muted_foreground;
        let item = m().item_de_menu;
        h_flex()
            .debug_selector(move || id.into())
            .flex_1()
            .min_w(px(0.))
            .min_h(px(item.altura))
            .gap(px(item.vao))
            .when_some(cor, |d, cor| d.text_color(cor))
            .when_some(icone, |d, icone| {
                d.child(
                    Icon::new(icone)
                        .size(px(16.))
                        .when(cor.is_none(), |i| i.text_color(apagado)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .child(rotulo.clone()),
            )
            .when_some(fim.clone(), |d, fim| {
                d.child(div().flex_none().text_xs().text_color(apagado).child(fim))
            })
    })
}

/// Uma opção de um grupo de escolha (o `ToggleGroupItem` do site): o `Button`
/// fantasma do kit, pequeno, **aceso quando escolhido** (`selected`).
pub fn chip(id: impl Into<SharedString>, escolhido: bool) -> Button {
    Button::new(id.into())
        .ghost()
        .xsmall()
        .px(px(8.))
        .rounded(crate::tema::canto(4.))
        .text_xs()
        .selected(escolhido)
}

/// 🏷️ **Uma ficha de recorte** (as pílulas "Todas 21", "sem marcação"…): o
/// `Toggle` do gpui-kit, de contorno, em pílula e **na altura do botão do
/// template** — elas dividem a linha com os botões, e com 28 px à mão ficavam
/// mais baixas que eles (dono, 28/09/2026: *"componentes fora de padrão, com
/// altura errada"*). A acesa leva o âmbar do tema; o número vem apagado.
pub fn ficha(
    id: impl Into<SharedString>,
    rotulo: impl Into<SharedString>,
    quantas: Option<usize>,
    acesa: bool,
    cx: &App,
) -> Toggle {
    let apagado = cx.theme().muted_foreground;
    // 📏 A ficha mora nas barras densas da sessão: o tamanho pequeno.
    let controle = m().botao_pequeno;
    Toggle::new(id.into())
        .outline()
        .xsmall()
        .checked(acesa)
        .flex_none()
        .gap(px(6.))
        .h(px(controle.altura))
        .px(px(controle.lados))
        // 🎞️ A pílula do site; no Lightroom, retângulo.
        .map(|f| {
            if crate::tema::cantos_retos() {
                f.rounded(crate::tema::canto(2.))
            } else {
                f.rounded_full()
            }
        })
        .when(acesa, |f| {
            f.bg(crate::tema::cores::aceso())
                .border_color(crate::tema::cores::aceso())
                .text_color(crate::tema::cores::sobre_aceso())
        })
        .child(div().child(rotulo.into()))
        .children(quantas.map(|n| {
            div()
                .when(!acesa, |d| d.text_color(apagado))
                .when(acesa, |d| d.opacity(0.75))
                .child(n.to_string())
        }))
}

/// Um botão de ligar e desligar (o `Toggle` do site): contorno apagado, e o
/// aceso do tema quando ligado (o âmbar do site, o cinza do Lightroom).
pub fn alternador(id: impl Into<SharedString>, ligado: bool, cx: &App) -> Button {
    Button::new(id.into())
        .small()
        .rounded(crate::tema::canto(4.))
        .map(|b| {
            if ligado {
                b.custom(crate::tema::botao_aceso(cx))
            } else {
                b.outline()
            }
        })
}

/// 🎚️ O slider do app: o `SliderState` do kit com o desenho do tema — trilho
/// colorido e preenchimento a partir do neutro (ver `slider_da_casa`).
pub fn slider(
    estado: &gpui_kit::Entity<gpui_kit::component::slider::SliderState>,
) -> crate::slider_da_casa::SliderDaCasa {
    crate::slider_da_casa::SliderDaCasa::new(estado)
}
