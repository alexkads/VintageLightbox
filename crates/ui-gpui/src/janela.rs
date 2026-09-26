//! Os gestos da **janela** na barra do app: dois cliques para maximizar e
//! arrastar para mover.
//!
//! # Por que isto existe
//!
//! *"No `crates/ui-gpui` eu não consigo maximizar com dois cliques na barra e
//! nem movimentar no Fedora"* (dono, 17/set/2026), e logo depois: *"o ajuste
//! precisa ser no Mac, Linux e Windows"*.
//!
//! A barra que o operador vê no alto do app **não é a barra da janela** — é o
//! cabeçalho de 56 px do próprio app, interface como qualquer outra. Nos três
//! sistemas ela ignorava os dois gestos; o que mudava era o quanto isso doía:
//!
//! | | barra do sistema | o que faltava |
//! |---|---|---|
//! | macOS | sim, acima do cabeçalho | os gestos **no cabeçalho** |
//! | Windows | sim, acima do cabeçalho | os gestos **no cabeçalho** |
//! | Fedora (GNOME/Wayland) | **não existe** | os gestos, e não havia outro lugar |
//!
//! O GNOME roda Wayland e **não implementa `xdg-decoration`**, o protocolo com
//! que um app pede a barra ao compositor: toda janela ali é decorada pelo
//! próprio app. O GPUI, nesse modo, desenha a moldura e a sombra — e mais nada.
//! Por isso a queixa veio de lá: no Mac e no Windows sobrava a barra do sistema
//! logo acima, e no Fedora não sobrava nada.
//!
//! # 🚨 Duas armadilhas que custaram leitura do GPUI
//!
//! 1. **`WindowControlArea` não resolve.** Marcar a região com
//!    `.window_control_area(WindowControlArea::Drag)` parece ser a resposta, e é
//!    — **só no Windows**, onde vira `HTCAPTION`. No Wayland, no X11 e no macOS
//!    o `on_hit_test_window_control` do GPUI 0.2.2 é um método vazio: a marcação
//!    é aceita e ignorada. E no Windows ela entregaria ao sistema a faixa
//!    inteira, **com os botões que moram nela** — o do menu lateral e o da
//!    conta —, que é troca ruim.
//! 2. **`start_window_move` só existe no Linux.** No `platform.rs` do GPUI ele
//!    tem corpo vazio por padrão, e nem o backend do macOS nem o do Windows o
//!    sobrescrevem. Chamá-lo lá não falha: não faz nada.
//!
//! # O que fica valendo, por sistema
//!
//! - **Dois cliques → maximiza e restaura: nos três.** No macOS por
//!   `titlebar_double_click`, que obedece à preferência do sistema
//!   (`AppleActionOnDoubleClick` — há quem a ponha em minimizar, ou em nada);
//!   nos outros por `zoom_window`.
//! - **Arrastar → move: no Linux.** É onde não há alternativa. No macOS e no
//!   Windows a barra do sistema continua logo acima e move a janela como sempre
//!   moveu; fazer o **cabeçalho** arrastar exigiria descer ao AppKit
//!   (`performWindowDragWithEvent:`) e ao Win32 (`WM_NCLBUTTONDOWN`), e os dois
//!   abrem laço de evento próprio dentro de um `update` do GPUI — que é
//!   exatamente o que derruba o app com "RefCell already borrowed"
//!   (`segundo_plano/janela.rs` registra a lição, conferida em 17/set/2026).
//!   Fica para quando houver uma máquina de cada para conferir.

use gpui_kit::component::{ActiveTheme as _, Icon, InteractiveElementExt as _};
use gpui_kit::{
    div, prelude::*, px, App, Decorations, Div, Hsla, InteractiveElement, MouseButton,
    SharedString, Stateful, Window,
};

use crate::recursos::Icone;

/// Um elemento que passa a se comportar como barra de título.
///
/// 🔑 **O arrasto não começa no `mouse_down`, e sim no primeiro movimento com o
/// botão apertado.** `start_window_move` entrega o ponteiro ao compositor:
/// chamá-lo já na descida roubaria o clique dos botões que moram dentro da
/// barra, que nunca chegariam a disparar. Esperar o movimento é o que separa
/// "clicou" de "arrastou" — e é como o `TitleBar` do `gpui-component` faz.
///
/// `id` é o do elemento no GPUI: duas barras com o mesmo id na mesma janela
/// dividiriam estado sem querer. Hoje elas nunca coexistem — a tela da sessão
/// não tem o cabeçalho do app —, e é justamente por isso que o dia em que
/// coexistirem não pode passar despercebido.
pub fn como_barra_de_titulo(
    elemento: Div,
    id: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let arrastando = window.use_state(cx, |_, _| Arrastando(false));

    elemento
        .id(id)
        .on_double_click(|_, window, cx| maximizar_ou_restaurar(window, cx))
        // O botão direito abre o menu da janela **do sistema** (no GNOME:
        // Minimizar, Maximizar, Mover, Redimensionar, Sempre no topo, Fechar),
        // como na barra de qualquer app — e como o Zed faz. Só onde a barra é
        // nossa: com a do sistema acima, o menu já está lá.
        .on_mouse_down(MouseButton::Right, |evento, window, _| {
            if app_desenha_a_barra(window) && window.window_controls().window_menu {
                window.show_window_menu(evento.position);
            }
        })
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(&arrastando, |estado, _, _, _| estado.0 = true),
        )
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(&arrastando, |estado, _, _, _| estado.0 = false),
        )
        .on_mouse_down_out(window.listener_for(&arrastando, |estado, _, _, _| estado.0 = false))
        .on_mouse_move(window.listener_for(&arrastando, |estado, _, window, _| {
            if estado.0 {
                // Uma vez só: daqui em diante quem manda no ponteiro é o
                // compositor, e o `mouse_up` que apagaria isto não chega mais.
                estado.0 = false;
                // Em tela cheia a janela não se move: sai dela primeiro, e o
                // arrasto continua — é assim que a tela do cliente vai de um
                // monitor para o outro (dono, 25/set/2026).
                if window.is_fullscreen() {
                    window.toggle_fullscreen();
                }
                window.start_window_move();
            }
        }))
}

/// Maximiza, ou restaura se já estiver maximizada — e, em tela cheia, sai
/// dela: é o "Restaurar" de quem está em tela cheia.
///
/// ⚠️ **No macOS não é `zoom_window`.** O duplo clique na barra de título é
/// configurável no sistema (`AppleActionOnDoubleClick`: aumentar/reduzir,
/// minimizar ou nada), e `titlebar_double_click` é o que consulta essa
/// preferência. Maximizar à força seria o app decidindo por quem já decidiu.
pub fn maximizar_ou_restaurar(window: &mut Window, _cx: &mut App) {
    if window.is_fullscreen() {
        window.toggle_fullscreen();
        return;
    }
    #[cfg(target_os = "macos")]
    window.titlebar_double_click();
    #[cfg(not(target_os = "macos"))]
    window.zoom_window();
}

/// Se o botão esquerdo está apertado sobre a barra.
///
/// Entidade, e não campo da tela: a barra é montada em lugares diferentes, e
/// nenhum deles tem estado próprio para guardar isto.
struct Arrastando(bool);

impl Render for Arrastando {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Os botões de janela — minimizar, maximizar/restaurar e fechar.
///
/// # 🚨 Por que isto existe
///
/// *"O comportamento no Ubuntu 26 GNOME tá muito ruim, sem botão de fechar"*
/// (dono, 19/set/2026). O GNOME não decora janela nenhuma (o porquê está no
/// alto deste módulo), então **os botões que o operador procura no canto não
/// existem** — quem os desenha tem de ser o app. Até aqui o `ui-gpui` desenhava
/// nenhum: no Fedora e no Ubuntu a única saída era matar o processo.
///
/// Pior na **tela de entrada**, que não tem cabeçalho: ali não havia barra, nem
/// botão, nem gesto — a janela não fechava nem se movia, e é a primeira coisa
/// que o app mostra.
///
/// # No macOS e no Windows isto não desenha nada, de propósito
///
/// Lá o sistema já põe os seus logo acima do cabeçalho. Desenhar os nossos
/// deixaria **dois** jogos de botão na mesma janela, e o de baixo faria o que o
/// de cima já faz. Por isso a função devolve um `div()` vazio fora do Linux, em
/// vez de o chamador lembrar de um `cfg!` em cada lugar.
///
/// `prefixo` entra no id de cada botão: duas barras na mesma janela com o mesmo
/// id dividiriam estado sem querer.
pub fn controles(prefixo: &'static str, cor: Hsla, window: &Window, cx: &App) -> Div {
    // Em tela cheia a janela principal é o app inteiro: os botões somem, como
    // no Zed. A tela do cliente é diferente — ver [`controles_mesmo_em_tela_cheia`].
    if !app_desenha_a_barra(window) || window.is_fullscreen() {
        return div();
    }
    desenhar_controles(prefixo, cor, window, cx)
}

/// Os botões de janela que **não somem em tela cheia** — os da tela do
/// cliente.
///
/// # 🚨 Por que a tela do cliente é diferente
///
/// *"Na tela de visualização do cliente, mesmo que estiver com tela cheia
/// precisa ter o minimizar, restaurar e fechar, sempre precisa ter como
/// arrastar essa tela para outro monitor, assim como é feito no Darktable"*
/// (dono, 25/set/2026). Ela vive no monitor virado para o cliente, quase
/// sempre em tela cheia — e em tela cheia **nenhum** sistema mostra barra, nem
/// o GNOME, nem o macOS, nem o Windows. Sem os nossos, o operador não tinha
/// como tirá-la de lá nem levá-la ao monitor certo.
///
/// Em tela cheia, "Restaurar" **sai da tela cheia** (é a janela voltando ao
/// tamanho de antes), e o arrasto da barra sai dela e já começa a mover
/// ([`como_barra_de_titulo`]).
pub fn controles_mesmo_em_tela_cheia(
    prefixo: &'static str,
    cor: Hsla,
    window: &Window,
    cx: &App,
) -> Div {
    if !(app_desenha_a_barra(window) || window.is_fullscreen()) {
        return div();
    }
    desenhar_controles(prefixo, cor, window, cx)
}

fn desenhar_controles(prefixo: &'static str, cor: Hsla, window: &Window, cx: &App) -> Div {
    #[cfg(test)]
    teste::DESENHADOS.with(|d| d.borrow_mut().insert(prefixo));

    // 🔑 **O desenho é o do Zed no GNOME** (dono, 24/set/2026: *"faça
    // igual"*): três círculos de 20 px, 12 px entre eles, traço fino, **sem
    // fundo** até o ponteiro passar — e o mesmo cinza no realce dos três, o
    // fechar incluso. Até aqui eram quadrados de 34×28 com os ícones do lucide
    // e o fechar em vermelho, no jeito do Windows.
    // (`crates/platform_title_bar/src/platforms/platform_linux.rs` no Zed.)
    let (fundo, realce) = (gpui_kit::transparent_black(), cx.theme().secondary_hover);

    let (icone_do_meio, dica_do_meio) = if window.is_maximized() || window.is_fullscreen() {
        (Icone::JanelaRestaurar, "Restaurar")
    } else {
        (Icone::JanelaMaximizar, "Maximizar")
    };
    // O compositor diz o que sabe fazer (`xdg_toplevel.wm_capabilities`): um
    // botão que ele não atende seria um botão que não faz nada. Fechar não se
    // pergunta — é o app que fecha.
    let suportados = window.window_controls();

    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(12.))
        .px(px(12.))
        .when(suportados.minimize, |d| {
            d.child(botao(
                SharedString::from(format!("{prefixo}-minimizar")),
                "janela-minimizar",
                Icone::JanelaMinimizar,
                "Minimizar",
                cor,
                (fundo, realce),
                |window, _| window.minimize_window(),
            ))
        })
        .when(suportados.maximize, |d| {
            d.child(botao(
                SharedString::from(format!("{prefixo}-maximizar")),
                "janela-maximizar",
                icone_do_meio,
                dica_do_meio,
                cor,
                (fundo, realce),
                maximizar_ou_restaurar,
            ))
        })
        .child(botao(
            SharedString::from(format!("{prefixo}-fechar")),
            "janela-fechar",
            Icone::JanelaFechar,
            "Fechar",
            cor,
            (fundo, realce),
            // 🚨 O mesmo caminho do fechar do sistema: na janela principal ele
            // leva à bandeja (dono, 21/set/2026), e não encerra o app.
            crate::segundo_plano::fechar_pelo_botao,
        ))
}

/// A raiz do conteúdo da janela: **o clique não sobe até a moldura**.
///
/// # 🚨 A camada que comia os cliques com a janela maximizada
///
/// *"Quando o programa fica maximizado parece que tem uma camada na parte de
/// baixo atrapalhando os cliques nos botões"* (dono, 25/set/2026). É a moldura
/// do `gpui-component` (`window_border`, que o `Root` põe em volta de tudo):
/// o `on_mouse_down` dela começa a **redimensionar** a janela sempre que o
/// clique cai a menos de 12 px da borda — **sem olhar se a janela está
/// maximizada**. Com a janela solta esses 12 px são a sombra, fora do
/// conteúdo, e está certo. Maximizada, a sombra some, o conteúdo encosta na
/// borda, e o botão a 12 px do rodapé (ou do alto, ou dos lados) passava o
/// clique ao compositor: o GNOME tomava o ponteiro e o botão nunca recebia o
/// `mouse_up`. O Zed não tem o defeito porque a moldura dele pula as bordas
/// encostadas (`resize_edge(…, tiling)` em `workspace::client_side_decorations`).
///
/// Parar o clique aqui é seguro porque a área legítima de redimensionar — a
/// sombra — **não é conteúdo**: ela continua chegando à moldura.
pub fn raiz_do_conteudo(elemento: Div) -> Div {
    elemento.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// O fundo de um cabeçalho que é barra de título: `ativo` com a janela em
/// foco, e o tom apagado do tema sem ele.
///
/// É o que o Zed faz (`title_bar_inactive_background`) e o que toda janela do
/// GNOME faz — o único jeito de ver, com duas janelas lado a lado, qual delas
/// recebe o teclado. Onde o sistema desenha a barra, é a dele que apaga, e
/// esta não muda.
pub fn fundo_da_barra(ativo: Hsla, window: &Window, cx: &App) -> Hsla {
    if app_desenha_a_barra(window) && !window.is_window_active() {
        cx.theme().muted
    } else {
        ativo
    }
}

/// Se **o app** tem de desenhar a barra desta janela.
///
/// # 🚨 A pergunta é à janela, e não ao sistema operacional
///
/// Até 22/set/2026 a regra era "no Linux, o app desenha", e isso só é
/// verdade no GNOME. O KDE Plasma (e o Sway, o Hyprland com o plugin certo, o
/// X11 com qualquer gerenciador de janelas) **desenha a barra dele**: lá a
/// janela ficaria com duas, a do sistema e a nossa logo abaixo.
///
/// Quem sabe é o GPUI, depois de negociar com o compositor: no Wayland ele
/// pede a barra ao sistema pelo `xdg-decoration`, e só quando o compositor
/// não o implementa (o GNOME) a janela fica `Decorations::Client`. É uma
/// resposta por janela e **pode mudar depois do primeiro quadro** — a
/// negociação chega junto com a primeira configuração —, por isso se
/// pergunta a cada desenho, e não uma vez ao abrir.
///
/// Fora do Linux o sistema sempre desenha; o `cfg!` só poupa a pergunta.
///
/// ⚠️ A resposta só é verdadeira se a janela **pediu** `Client` ao abrir —
/// ver [`decoracoes_ao_abrir`].
pub fn app_desenha_a_barra(window: &Window) -> bool {
    #[cfg(test)]
    if teste::FORCADA.with(|f| f.get()) {
        return true;
    }
    cfg!(target_os = "linux") && matches!(window.window_decorations(), Decorations::Client { .. })
}

/// Os botões de janela de uma **tela** — o cabeçalho do app, a barra da
/// galeria, a da nova sessão, a da Revelação.
///
/// # 🚨 A regra: os botões moram na primeira linha da janela, e só nela
///
/// *"Tem tela que não tem o minimizar, maximizar e fechar"* (dono,
/// 25/set/2026): cada tela punha os seus, e a Revelação não punha — com a
/// janela maximizada no GNOME, não havia como fechar nem minimizar ali. E
/// na galeria eles ficavam na **segunda** linha, debaixo da faixa das guias.
/// O operador procurava no canto e não achava: *"o usuário se perde"*.
///
/// Agora, como no Zed, ficam sempre no canto de cima: **na faixa das guias
/// quando ela está na tela** ([`marcar_faixa_com_controles`], chamada pela
/// raiz antes de as telas se desenharem), e senão na barra da tela. O teste
/// `e2e::janela` passa por todas as telas e afirma: um jogo, e no alto.
pub fn controles_da_tela(prefixo: &'static str, cor: Hsla, window: &Window, cx: &App) -> Div {
    let na_faixa = cx
        .try_global::<FaixaComControles>()
        .and_then(|f| f.0)
        .is_some_and(|dona| dona == window.window_handle());
    if na_faixa {
        return div();
    }
    controles(prefixo, cor, window, cx)
}

/// A faixa das guias desta janela está na tela, com os botões de janela nela.
struct FaixaComControles(Option<gpui_kit::AnyWindowHandle>);

impl gpui_kit::Global for FaixaComControles {}

/// Diz, a cada desenho da raiz, se os botões foram para a faixa das guias —
/// **antes** de as telas se desenharem, que é quando elas perguntam.
pub fn marcar_faixa_com_controles(sim: bool, window: &Window, cx: &mut App) {
    cx.set_global(FaixaComControles(sim.then(|| window.window_handle())));
}

/// 🧪 O GPUI de teste não decora janela nenhuma: sem forçar, os botões nunca
/// seriam desenhados e o teste de todas as telas não teria o que conferir.
#[cfg(test)]
pub mod teste {
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeSet;

    thread_local! {
        pub(super) static FORCADA: Cell<bool> = const { Cell::new(false) };
        pub(super) static DESENHADOS: RefCell<BTreeSet<&'static str>> =
            const { RefCell::new(BTreeSet::new()) };
    }

    /// Faz de conta que é o GNOME: o app desenha a barra.
    pub fn forcar_barra_do_app() {
        FORCADA.with(|f| f.set(true));
    }

    /// Quais barras desenharam botões de janela desde a última pergunta —
    /// pelo prefixo de cada uma. Duas no mesmo quadro são dois jogos.
    pub fn barras_com_botoes() -> BTreeSet<&'static str> {
        DESENHADOS.with(|d| std::mem::take(&mut *d.borrow_mut()))
    }
}

/// O `window_decorations` de toda `WindowOptions` do app.
///
/// # 🚨 Sem isto, no GNOME, a janela nasce sem barra nenhuma
///
/// O GPUI 0.2.2, ao abrir a janela, chama
/// `request_decorations(window_decorations.unwrap_or(Server))`, e no Wayland
/// isso **grava `Server` no estado da janela mesmo quando o compositor não tem
/// `xdg-decoration`** — que é o GNOME. A janela passa a dizer "o sistema
/// desenha", [`app_desenha_a_barra`] responde que não, e ninguém desenha:
/// nem o GNOME, nem o app. Foi o que aconteceu de 22 a 24/set/2026 (*"não
/// aparece a barra!"*, dono), depois que a barra passou a perguntar à janela.
///
/// Pedindo `Client`, como o Zed pede, cada compositor responde por si: o
/// GNOME não responde e a janela fica `Client` (o app desenha); o KDE atende
/// o pedido e também fica `Client` — uma barra só, a nossa, como no Zed; e
/// quem força a barra do sistema (um Sway configurado assim) devolve
/// `ServerSide`, e a nossa some sozinha. Fora do Linux o campo não vale nada
/// e fica `None`.
pub fn decoracoes_ao_abrir() -> Option<gpui_kit::WindowDecorations> {
    cfg!(target_os = "linux").then_some(gpui_kit::WindowDecorations::Client)
}

/// Em que área de trabalho o app está rodando.
///
/// 🔑 **Serve para dizer, e não para decidir.** A barra é decidida por
/// [`app_desenha_a_barra`], que pergunta à janela: um KDE em X11, um GNOME
/// com extensão de barra, um Sway — todos se resolvem sozinhos por ali, e
/// uma lista de nomes nunca estaria completa. Este nome vai para o log de
/// abertura e para o diagnóstico, que é onde "está no KDE?" se pergunta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AreaDeTrabalho {
    Gnome,
    Kde,
    /// Outra, com o nome que o sistema deu.
    Outra(String),
    /// Linux sem `XDG_CURRENT_DESKTOP` (um gerenciador de janelas cru), ou
    /// outro sistema.
    Desconhecida,
}

impl AreaDeTrabalho {
    /// Lida de `XDG_CURRENT_DESKTOP`, que pode trazer vários nomes separados
    /// por `:` (o Ubuntu manda `ubuntu:GNOME`, o Plasma manda `KDE`).
    pub fn de(valor: Option<&str>) -> Self {
        let Some(valor) = valor.map(str::trim).filter(|v| !v.is_empty()) else {
            return Self::Desconhecida;
        };
        let nomes: Vec<String> = valor
            .split(':')
            .map(|nome| nome.to_ascii_uppercase())
            .collect();
        if nomes.iter().any(|nome| nome == "KDE" || nome == "PLASMA") {
            Self::Kde
        } else if nomes
            .iter()
            .any(|nome| nome == "GNOME" || nome.starts_with("GNOME-"))
        {
            Self::Gnome
        } else {
            Self::Outra(valor.to_string())
        }
    }

    pub fn atual() -> Self {
        if !cfg!(target_os = "linux") {
            return Self::Desconhecida;
        }
        Self::de(std::env::var("XDG_CURRENT_DESKTOP").ok().as_deref())
    }

    /// Como aparece para quem lê: `KDE Plasma (Wayland)`.
    pub fn descricao(&self) -> String {
        let nome = match self {
            Self::Gnome => "GNOME".to_string(),
            Self::Kde => "KDE Plasma".to_string(),
            Self::Outra(nome) => nome.clone(),
            Self::Desconhecida => "desconhecida".to_string(),
        };
        let sessao = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
        if sessao.is_empty() {
            nome
        } else {
            format!("{nome} ({sessao})")
        }
    }
}

/// Um botão da barra: o quadrado com o ícone, o realce e o clique.
fn botao(
    id: impl Into<gpui_kit::ElementId>,
    seletor: &'static str,
    icone: Icone,
    dica: &'static str,
    cor: Hsla,
    (fundo, realce): (Hsla, Hsla),
    acao: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .debug_selector(move || seletor.to_string())
        .size(px(20.))
        .rounded_full()
        .cursor_pointer()
        .text_color(cor)
        .bg(fundo)
        .hover(move |s| s.bg(realce))
        .active(move |s| s.bg(realce.opacity(1.6)))
        .tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(dica).build(window, cx)
        })
        // 🔑 O clique **para aqui**. Estes botões moram dentro da barra que
        // arrasta a janela: sem isto, apertar "fechar" começaria um arrasto e o
        // compositor levaria o ponteiro embora antes do clique acontecer.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            acao(window, cx);
        })
        .child(Icon::new(icone).size(px(16.)))
}

#[cfg(test)]
mod testes_da_area_de_trabalho {
    use super::AreaDeTrabalho;

    #[test]
    fn reconhece_o_kde_e_o_gnome_pelo_xdg_current_desktop() {
        assert_eq!(AreaDeTrabalho::de(Some("KDE")), AreaDeTrabalho::Kde);
        assert_eq!(AreaDeTrabalho::de(Some("GNOME")), AreaDeTrabalho::Gnome);
        // O Ubuntu antepõe o próprio nome, e o Fedora às vezes o modo clássico.
        assert_eq!(
            AreaDeTrabalho::de(Some("ubuntu:GNOME")),
            AreaDeTrabalho::Gnome
        );
        assert_eq!(
            AreaDeTrabalho::de(Some("GNOME-Classic:GNOME")),
            AreaDeTrabalho::Gnome
        );
        assert_eq!(
            AreaDeTrabalho::de(Some("sway")),
            AreaDeTrabalho::Outra("sway".into())
        );
        assert_eq!(AreaDeTrabalho::de(Some("")), AreaDeTrabalho::Desconhecida);
        assert_eq!(AreaDeTrabalho::de(None), AreaDeTrabalho::Desconhecida);
    }
}
