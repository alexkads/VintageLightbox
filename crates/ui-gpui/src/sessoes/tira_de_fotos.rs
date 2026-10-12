//! O desenho da tira de fotos — **um só**, para a sessão e para a Revelação.
//!
//! 🚨 *"O conjunto filtro, atalhos e filmstrip da sessão de fotos deveria ser o
//! mesmo em revelação"* — dono, 11/out/2026. Eram duas tiras desenhadas à mão,
//! cada uma na sua tela, e divergiam em tudo o que se vê: a ficha do recorte, a
//! linha de atalhos (que a Revelação nem tinha), o puxador, a borda da
//! escolhida, o tamanho da estrela e a cor do ponto da levada.
//!
//! 🔑 **Aqui mora só o que se vê.** Quem é a foto em foco, o que o clique faz e
//! de onde vem a miniatura continuam de cada tela — a sessão anda sobre o
//! `Acervo` do core, a Revelação sobre a cópia dela. O que as duas passam a
//! dividir é a régua: mexer numa medida aqui mexe nas duas.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::{div, img, prelude::*, px, AnyElement, App, Div, SharedString, Stateful};
use std::sync::Arc;

use crate::recursos::Icone;
use crate::tema::cores;

/// O respiro entre duas miniaturas.
pub const VAO: f32 = 6.0;
/// O recuo da faixa, dos dois lados.
pub const RECUO: f32 = 10.0;
/// O que a faixa tem de altura além do lado da miniatura.
const FOLGA_DA_FAIXA: f32 = 8.0;

/// O modificador do sistema, como o `useTeclaDeAtalho` do site: `⌘` no Mac,
/// `Ctrl` no resto. A dica que mostra a tecla errada ensina o gesto errado.
#[cfg(target_os = "macos")]
pub const MODIFICADOR: &str = "⌘";
#[cfg(not(target_os = "macos"))]
pub const MODIFICADOR: &str = "Ctrl";
#[cfg(target_os = "macos")]
const MODIFICADOR_A: &str = "⌘+A";
#[cfg(not(target_os = "macos"))]
const MODIFICADOR_A: &str = "Ctrl+A";
#[cfg(target_os = "macos")]
const MODIFICADOR_D: &str = "⌘+D";
#[cfg(not(target_os = "macos"))]
const MODIFICADOR_D: &str = "Ctrl+D";

/// O que a tela pede quando o operador mexe nas fichas da tela do cliente.
pub type AoAlternarOCliente = Arc<dyn Fn(&mut gpui_kit::Window, &mut App)>;
/// `(grade, fotos por linha)`.
pub type AoMudarOModoDoCliente = Arc<dyn Fn(bool, usize, &mut gpui_kit::Window, &mut App)>;

/// 🖥️ **A tela do cliente e o que ela mostra, junto dos recortes** (dono,
/// 11/out/2026): no Lightroom os botões do segundo monitor moram no cabeçalho
/// da tira, ao lado dos filtros. As mesmas fichas na sessão e na Revelação —
/// *"deveria ter feito a mesma coisa na filmstrip da revelação"*.
///
/// No molde das fichas — acesa é ligada —, com um traço separando o que liga
/// a outra tela do que recorta a lista. `prefixo` é o da tela (`sessao`,
/// `revelacao`): os seletores ficam `<prefixo>-tela-do-cliente` e
/// `<prefixo>-modo-do-cliente`.
///
/// `na_revelacao`: lá a segunda tela mostra sempre a foto aberta, e o modo
/// escolhido vale quando se volta à galeria — a dica diz isso.
pub fn fichas_do_cliente(
    prefixo: &'static str,
    aberta: bool,
    (grade, por_linha): (bool, usize),
    na_revelacao: bool,
    ao_alternar: AoAlternarOCliente,
    ao_mudar_o_modo: AoMudarOModoDoCliente,
    cx: &App,
) -> Vec<AnyElement> {
    use biblioteca_core::grade_do_cliente as conta;
    use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};

    let tela_do_cliente =
        crate::estilo::ficha_de_acao(format!("{prefixo}-tela-do-cliente"), aberta, cx)
            .child(Icon::new(Icone::Monitor).size(px(14.)))
            .child("Tela do cliente")
            // O atalho da tela cheia, à vista enquanto ela está aberta
            // (dono, 22/set/2026).
            .when(aberta, |b| b.child(crate::cliente::tecla_da_tela_cheia()))
            .tooltip(if aberta {
                format!(
                    "Fechar a tela do cliente · {} põe e tira a tela cheia",
                    crate::cliente::texto_da_tela_cheia()
                )
            } else if na_revelacao {
                format!(
                    "Abrir a tela do cliente no outro monitor: ela mostra esta foto, revelada, enquanto você ajusta · depois, {} põe e tira a tela cheia",
                    crate::cliente::texto_da_tela_cheia()
                )
            } else {
                format!(
                    "Abrir a tela do cliente · depois, {} põe e tira a tela cheia",
                    crate::cliente::texto_da_tela_cheia()
                )
            })
            .on_click(move |_, window, cx| ao_alternar(window, cx));
    // 🖼️ O que a tela do cliente mostra: a foto em foco ou a grade numerada,
    // e quantas por linha (dono, 11/out/2026). No mesmo molde do botão do
    // bot: o estado escrito, as opções no menu.
    let modo = crate::estilo::ficha_de_acao(format!("{prefixo}-modo-do-cliente"), grade, cx)
        .child(Icon::new(Icone::Grid3x3).size(px(14.)))
        .child(if grade {
            format!("Grade · {por_linha} por linha")
        } else {
            "Uma foto".to_string()
        })
        .child(Icon::new(Icone::ChevronDown).size(px(12.)))
        .tooltip(if na_revelacao {
            "O que a tela do cliente mostra na galeria: a foto em foco, ou a grade com o número de cada foto. Aqui na Revelação ela mostra sempre a foto aberta."
        } else {
            "O que a tela do cliente mostra: a foto em foco, ou a grade com o número de cada foto"
        })
        .dropdown_menu(move |menu, _window, _cx| {
            let pedir = |grade: bool, por_linha: usize| {
                let ao_mudar = ao_mudar_o_modo.clone();
                move |_: &gpui_kit::ClickEvent, window: &mut gpui_kit::Window, cx: &mut App| {
                    ao_mudar(grade, por_linha, window, cx)
                }
            };
            let mut menu = menu
                .item(
                    PopupMenuItem::new("Uma foto — a que está em foco")
                        .checked(!grade)
                        .on_click(pedir(false, por_linha)),
                )
                .item(
                    PopupMenuItem::new("Grade — várias fotos, com o número de cada uma")
                        .checked(grade)
                        .on_click(pedir(true, por_linha)),
                )
                .separator();
            for n in conta::MENOS_POR_LINHA..=conta::MAIS_POR_LINHA {
                menu = menu.item(
                    PopupMenuItem::new(format!("{n} fotos por linha"))
                        .checked(grade && n == por_linha)
                        .on_click(pedir(true, n)),
                );
            }
            menu
        });
    vec![
        tela_do_cliente.into_any_element(),
        modo.into_any_element(),
        div()
            .flex_none()
            .w(px(1.))
            .h(px(16.))
            .mx(px(4.))
            .bg(cx.theme().border)
            .into_any_element(),
    ]
}

/// A barra dos recortes, colada em cima da tira: o filtro fica junto do que
/// ele filtra. As fichas são [`crate::estilo::ficha`], nas duas telas.
pub fn barra_de_recortes(fichas: Vec<AnyElement>, cx: &App) -> Div {
    div()
        .flex()
        .flex_none()
        .flex_wrap()
        .items_center()
        .gap(px(6.))
        .px(px(12.))
        .py(px(6.))
        .border_t_1()
        .border_color(cx.theme().border)
        .children(fichas)
}

/// A barra que arrasta a altura da tira — só o desenho; o arrasto é de quem
/// guarda a altura.
pub fn puxador(id: &'static str, cx: &App) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .h(px(6.))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .border_t_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .cursor(gpui_kit::CursorStyle::ResizeUpDown)
        .child(
            div()
                .h(px(2.))
                .w(px(32.))
                .rounded_full()
                .bg(cx.theme().border),
        )
}

/// Uma tecla escrita, como os `<kbd>` do site.
pub fn tecla(rotulo: &str, cx: &App) -> Div {
    div()
        .px(px(3.))
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .child(SharedString::from(rotulo.to_string()))
}

/// O que a linha de atalhos anuncia além do que as duas telas têm.
///
/// 🚨 **A linha só diz o que a tecla faz naquela tela.** Na Revelação não há
/// fileira para `↑`/`↓` mudarem, e o `⌫` é do retoque e da máscara, nunca da
/// foto: anunciar os dois lá seria a tela prometendo o que não entrega.
#[derive(Clone, Copy)]
pub struct Atalhos {
    /// `↑` `↓` mudam de fileira (há uma grade acima da tira).
    pub fileiras: bool,
    /// `⌫` exclui a foto.
    pub exclui: bool,
}

/// O gesto de uma das setas da ponta da linha.
pub type AoAndar = Arc<dyn Fn(i32, &mut gpui_kit::Window, &mut App)>;

/// A linha de cima da faixa: onde se está, os atalhos e as setas `‹` `›`.
pub fn linha_de_atalhos(
    onde: SharedString,
    atalhos: Atalhos,
    tem_fotos: bool,
    ao_andar: AoAndar,
    cx: &App,
) -> Div {
    let seta = |id: &'static str, rotulo: &'static str, passo: i32| {
        let ao_andar = ao_andar.clone();
        Button::new(id)
            .debug_selector(move || id.to_string())
            .label(rotulo)
            .xsmall()
            .ghost()
            .disabled(!tem_fotos)
            .on_click(move |_ev, window, cx| ao_andar(passo, window, cx))
    };
    div()
        .debug_selector(|| "tira-atalhos".into())
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(RECUO))
        .pt(px(3.))
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(onde)
        .child(tecla("←", cx))
        .child(tecla("→", cx))
        .child("andam ·")
        .when(atalhos.fileiras, |linha| {
            linha
                .child(tecla("↑", cx))
                .child(tecla("↓", cx))
                .child("mudam de fileira ·")
        })
        .child(tecla("1", cx))
        .child("–")
        .child(tecla("5", cx))
        .child("nota ·")
        // 🔑 **O `0` é metade do gesto**: a nota sobe a foto, e tirá-la a traz
        // de volta (C20–C22). A linha do site o traz desde sempre.
        .child(tecla("0", cx))
        .child("tira a nota ·")
        .child(tecla("P", cx))
        .child("levada no balcão ·")
        .when(atalhos.exclui, |linha| {
            linha.child(tecla("⌫", cx)).child("exclui ·")
        })
        .child(tecla(MODIFICADOR, cx))
        .child("ou")
        .child(tecla("Shift", cx))
        .child("no clique marcam várias ·")
        .child(tecla(MODIFICADOR_A, cx))
        .child("marca tudo,")
        .child(tecla(MODIFICADOR_D, cx))
        .child("desmarca")
        // As setas ficam na ponta direita, como no site.
        .child(
            div()
                .ml_auto()
                .flex()
                .gap(px(2.))
                .child(seta("tira-anterior", "‹", -1))
                .child(seta("tira-proxima", "›", 1)),
        )
}

/// A faixa que rola, já com recuo, vão e altura — quem chama põe o
/// `track_scroll`, a roda e os itens.
pub fn faixa(id: &'static str, lado: f32) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(VAO))
        .px(px(RECUO))
        .pb(px(6.))
        .h(px(lado + FOLGA_DA_FAIXA))
        .overflow_x_scroll()
}

/// O lugar das miniaturas que não viraram elemento (`faixa_desenhada`).
pub fn espacador(itens: usize, largura: f32) -> AnyElement {
    div()
        .flex_none()
        .w(px(itens as f32 * (largura + VAO) - VAO))
        .h(px(1.))
        .into_any_element()
}

/// A seta de uma ponta: diz que há foto fora da vista, e o clique rola a
/// faixa um passo de tela para aquele lado (o `andar` da tira do site).
pub fn seta(esquerda: bool, ao_rolar: AoAndar, cx: &App) -> Stateful<Div> {
    let tema = cx.theme();
    let (fundo, texto) = (tema.background, tema.foreground);
    div()
        .id(if esquerda {
            "tira-seta-esquerda"
        } else {
            "tira-seta-direita"
        })
        .absolute()
        .top_0()
        .bottom_0()
        .w(px(40.))
        .when(esquerda, |lado| lado.left_0())
        .when(!esquerda, |lado| lado.right_0())
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(gpui_kit::linear_gradient(
            if esquerda { 90.0 } else { 270.0 },
            gpui_kit::linear_color_stop(fundo, 0.0),
            gpui_kit::linear_color_stop(fundo.opacity(0.0), 1.0),
        ))
        .text_color(texto.opacity(0.9))
        .hover(move |s| s.text_color(texto))
        .tooltip(move |window, cx| {
            Tooltip::new(if esquerda {
                "Rolar a tira para a esquerda"
            } else {
                "Rolar a tira para a direita"
            })
            .build(window, cx)
        })
        .child(
            Icon::new(if esquerda {
                Icone::ChevronLeft
            } else {
                Icone::ChevronRight
            })
            .size(px(20.)),
        )
        .on_click(move |_ev, window, cx| ao_rolar(if esquerda { -1 } else { 1 }, window, cx))
}

/// Rola a faixa um passo de tela (`andar` do site): 80% da largura, ou 200.
pub fn rolar(rolagem: &gpui_kit::ScrollHandle, direcao: i32) {
    let largura = f32::from(rolagem.bounds().size.width);
    let passo = (largura * 0.8).max(200.0);
    let maximo = rolagem.max_offset().x;
    let atual = rolagem.offset();
    let x = (atual.x - px(direcao as f32 * passo))
        .min(px(0.))
        .max(-maximo);
    rolagem.set_offset(gpui_kit::point(x, atual.y));
}

/// A roda vertical rola a faixa na horizontal: é o gesto que a mão faz sobre
/// uma faixa, e o mesmo que o site escuta com `passive: false`. Devolve se
/// rolou — o gesto horizontal fica com a rolagem do próprio elemento.
pub fn rolar_com_a_roda(
    rolagem: &gpui_kit::ScrollHandle,
    evento: &gpui_kit::ScrollWheelEvent,
    window: &gpui_kit::Window,
) -> bool {
    let delta = evento.delta.pixel_delta(window.line_height());
    if delta.y.abs() <= delta.x.abs() {
        return false;
    }
    let maximo = rolagem.max_offset().x;
    let atual = rolagem.offset();
    let x = (atual.x + delta.y).min(px(0.)).max(-maximo);
    rolagem.set_offset(gpui_kit::point(x, atual.y));
    true
}

/// Há foto fora da vista antes e depois do que a faixa mostra.
pub fn tem_fora_da_vista(rolagem: &gpui_kit::ScrollHandle) -> (bool, bool) {
    let deslocamento = -rolagem.offset().x;
    let maximo = rolagem.max_offset().x;
    (
        deslocamento > px(4.),
        maximo > px(4.) && deslocamento < maximo - px(4.),
    )
}

/// Como a miniatura está na seleção.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Destaque {
    /// A foto da vez — a borda cheia.
    EmFoco,
    /// Marcada no lote — a borda esmaecida.
    Marcada,
    Nenhum,
}

/// A célula de uma miniatura, vazia.
///
/// 🔑 **Borda de 2px, e três estados** — como no site: cheia no foco,
/// esmaecida na marcada, invisível no resto.
pub fn celula(
    id: SharedString,
    largura: f32,
    lado: f32,
    destaque: Destaque,
    cx: &App,
) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .w(px(largura))
        .h(px(lado))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .rounded(cx.theme().radius)
        .bg(cx.theme().muted)
        .border_2()
        .border_color(match destaque {
            Destaque::EmFoco => cx.theme().primary,
            Destaque::Marcada => cx.theme().primary.opacity(0.5),
            Destaque::Nenhum => gpui_kit::transparent_black(),
        })
        .cursor_pointer()
}

/// A foto dentro da célula de `largura`×`lado`.
///
/// 🔑 **A foto enche a célula**, como no site (dono, 28/set/2026: *"a tira da
/// WEB preenche todo o espaço vertical"*). Com só `max_w_full`/`max_h_full`
/// ela ficava no tamanho natural da miniatura — 320 px — e a tira alta sobrava
/// em volta.
/// 🚨 E **nunca `size_full`**: o `Img` do GPUI grava `style.aspect_ratio` com a
/// proporção da foto, o taffy tira a altura da largura e o `overflow_hidden`
/// da célula vira **corte** no retrato (17 e 18/set/2026). A conta vem pronta.
/// 🚨 `Contain`, e não `Cover`: preencher o retângulo custa **cortar**, e numa
/// foto que já foi **enquadrada** isso corta o que o operador escolheu manter
/// (dono, 2026-09-11).
pub fn foto(
    imagem: Arc<gpui_kit::RenderImage>,
    largura: f32,
    lado: f32,
    apagada: bool,
) -> gpui_kit::Img {
    // O miolo da célula, dentro da borda de 2px.
    let miolo = gpui_kit::size(px(largura - 4.0), px(lado - 4.0));
    let encaixe = crate::imagem::cabe_em(miolo, imagem.size(0));
    img(imagem)
        .w(encaixe.width)
        .h(encaixe.height)
        .object_fit(gpui_kit::ObjectFit::Contain)
        .when(apagada, |imagem| imagem.opacity(0.4))
}

/// As estrelas da nota, no canto de cima à esquerda.
pub fn estrelas(nota: usize) -> Div {
    div()
        .absolute()
        .top(px(1.))
        .left(px(3.))
        .text_xs()
        .text_color(cores::nota())
        .child(SharedString::from("★".repeat(nota.min(5))))
}

/// O ponto da levada no balcão, no canto de cima à direita.
pub fn ponto_da_levada() -> Div {
    div()
        .absolute()
        .top(px(3.))
        .right(px(3.))
        .size(px(7.))
        .rounded_full()
        .bg(cores::quente())
}

/// A tarja da comprada, embaixo.
pub fn tarja_da_comprada(cx: &App) -> Div {
    div()
        .absolute()
        .bottom_0()
        .left_0()
        .right_0()
        .py(px(1.))
        .text_center()
        .text_xs()
        .bg(gpui_kit::black().opacity(0.7))
        .text_color(cx.theme().foreground)
        .child("comprada")
}
