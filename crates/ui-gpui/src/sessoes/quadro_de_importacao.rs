//! O quadro de importar fotos: ícone, título, o texto de "arraste ou escolha"
//! e os botões — o `Empty` da etapa 2 do assistente do site
//! (`nova/etapa-fotos.tsx`), que o site também usa no modal "Importar fotos"
//! da sessão (`importacao/quadro-de-importacao.tsx`).
//!
//! 🔑 **Um desenho só para as duas portas** (dono, 22/set/2026: *"seria bom ser
//! um único componente para reaproveitamento"*). A etapa 2 da nova sessão e o
//! "Importar fotos" de dentro da sessão levam ao mesmo lugar, e o operador tem
//! de reconhecer o gesto nos dois. Os botões vêm de quem chama: cada porta
//! sabe o que o "Escolher fotos" dela faz, e só o assistente tem o "Do cartão
//! ou pasta…" — dentro da sessão não há explorador nosso, de propósito
//! (`detalhe.rs`, a área de envio).

use gpui::{div, prelude::*, px, App, Div, FontWeight, SharedString};
use gpui_component::{v_flex, ActiveTheme, Icon};

use crate::recursos::Icone;

/// O texto do assistente, e o ponto de partida do da sessão.
pub const ARRASTE_OU_ESCOLHA: &str = "Arraste a pasta do Lightroom para qualquer lugar desta \
     tela, ou escolha os arquivos. As fotos ficam neste computador e sobem quando forem \
     classificadas, dentro da sessão.";

/// O texto do modal da sessão — o `descricaoDaImportacaoNaSessao` do site,
/// com o "Entram como" da barra dito por extenso.
pub fn descricao_na_sessao(rotulo_do_estado: &str) -> String {
    format!(
        "Arraste a pasta do Lightroom para qualquer lugar desta tela, ou escolha os arquivos. \
         As fotos entram como {rotulo_do_estado} e sobem quando forem classificadas."
    )
}

/// O quadro tracejado, com os botões de quem chama embaixo do texto.
pub fn quadro_de_importacao(
    titulo: impl Into<SharedString>,
    descricao: impl Into<SharedString>,
    botoes: impl IntoElement,
    cx: &App,
) -> Div {
    let tema = cx.theme();
    v_flex()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .p(px(32.))
        .rounded(px(10.))
        .border_1()
        .border_dashed()
        .border_color(tema.border)
        .child(
            div()
                .p(px(8.))
                .rounded(px(8.))
                .bg(tema.muted)
                .child(Icon::new(Icone::ImagePlus).size(px(24.))),
        )
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::MEDIUM)
                .child(titulo.into()),
        )
        .child(
            div()
                .max_w(px(360.))
                .text_sm()
                .text_center()
                .text_color(tema.muted_foreground)
                .child(descricao.into()),
        )
        .child(botoes)
}
