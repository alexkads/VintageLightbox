//! O rodapé da janela: a versão, o que o app tem a dizer sobre ela, e o
//! servidor em que ele está.
//!
//! # Por que um rodapé fixo
//!
//! O app se comunica com o operador sobre **ele mesmo** — versão nova,
//! atualização compilando, "reabra para usar" — e até 27/set/2026 isso só
//! aparecia numa faixa que surgia sobre o pé da tela e sumia. Quem queria saber
//! que versão tinha nas mãos precisava abrir o menu da conta. O dono pediu o
//! rodapé *"para comunicação de versão e afins"*: uma linha que está sempre
//! lá, como a barra de status dos editores.
//!
//! | Trecho | O que diz | Clique |
//! |---|---|---|
//! | esquerda | `VintageLightbox 0.1.22` | as novidades desta versão |
//! | meio | a faixa de atualização, quando há aviso ([`faixa`]) | os botões dela |
//! | direita | o site em que o app está; âmbar fora da produção | abre o site |
//!
//! 🔑 **Nunca atrapalha**: 28 px, uma linha, sem foco. A faixa que tinha
//! botões continua com eles, e o destaque da versão importante pinta só o
//! trecho do meio — o rodapé não vira um aviso que grita a sessão inteira.
//!
//! 🚨 **O servidor aparece porque já enganou gente**: com só a API apontada
//! para a pilha local, a autorização abria em produção e o operador entrava na
//! conta real achando que estava local (17/set/2026). Em produção o endereço
//! fica apagado; fora dela, âmbar.
//!
//! O rodapé é da janela principal: a tela do cliente é outra janela, com
//! outra raiz, e nada daqui chega lá.

use std::sync::{Arc, OnceLock};

use gpui_kit::component::{h_flex, ActiveTheme, Icon};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, FontWeight, SharedString};

use super::{Aplicativo, PedidoDeAtualizacao};
use crate::atualizacao::faixa;
use crate::pos_venda::config::SITE_PADRAO;
use crate::recursos::Icone;

/// A altura do rodapé. Os toasts do canto de baixo sobem esta medida
/// (`tema::aplicar`) para não caírem em cima dele.
pub const ALTURA_DO_RODAPE: f32 = 28.;

/// O site desta abertura, e se ele é a produção.
///
/// 🔑 Lido **uma vez**: o endereço vem do `pos-venda.json` e das variáveis de
/// ambiente, e nenhum dos dois muda com o app aberto — reler o disco a cada
/// quadro seria custo sem ganho.
fn servidor() -> &'static (String, bool) {
    static SERVIDOR: OnceLock<(String, bool)> = OnceLock::new();
    SERVIDOR.get_or_init(|| {
        let site = crate::pos_venda::config::ler().site();
        let producao = site.trim_end_matches('/') == SITE_PADRAO;
        (site, producao)
    })
}

/// O endereço como o rodapé o escreve: sem o esquema.
pub fn endereco_curto(site: &str) -> &str {
    let sem_esquema = site.split_once("://").map_or(site, |(_, resto)| resto);
    sem_esquema.trim_end_matches('/')
}

impl Aplicativo {
    /// O rodapé inteiro. Ver o [módulo](self).
    pub(super) fn rodape(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (borda, fundo, apagado, aviso) = (
            tema.border,
            tema.background,
            tema.muted_foreground,
            tema.warning,
        );
        let destaque = faixa::em_destaque(&self.atualizacao);

        let versao = h_flex()
            .id("rodape-versao")
            .debug_selector(|| "rodape-versao".into())
            .flex_none()
            .h(px(20.))
            .px(px(6.))
            .gap(px(6.))
            .rounded(px(4.))
            .items_center()
            .text_xs()
            .text_color(apagado)
            .cursor_pointer()
            .hover(|s| s.bg(tema.accent).text_color(tema.accent_foreground))
            .child(Icon::new(Icone::Sparkles).size(px(12.)))
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child("VintageLightbox"),
            )
            .child(concat!("v", env!("CARGO_PKG_VERSION")))
            .tooltip(|window, cx| {
                gpui_kit::component::tooltip::Tooltip::new("Ver as novidades desta versão")
                    .build(window, cx)
            })
            .on_click(cx.listener(|raiz, _, _window, cx| {
                raiz.atender(PedidoDeAtualizacao::VerEstaVersao, cx);
            }));

        let agir = cx.listener(|raiz, pedido: &PedidoDeAtualizacao, _window, cx| {
            raiz.atender(*pedido, cx);
        });
        let meio: Option<AnyElement> = faixa::desenhar(
            &self.atualizacao,
            cx,
            Arc::new(move |pedido, window, app| agir(&pedido, window, app)),
        );

        let (site, producao) = servidor();
        let producao = *producao;
        let endereco: SharedString = endereco_curto(site).to_string().into();
        let para_abrir = site.clone();
        let servidor = h_flex()
            .id("rodape-servidor")
            .debug_selector(|| "rodape-servidor".into())
            .flex_none()
            .h(px(20.))
            .px(px(6.))
            .gap(px(6.))
            .rounded(px(4.))
            .items_center()
            .text_xs()
            .cursor_pointer()
            .hover(|s| s.bg(tema.accent))
            .map(|d| {
                if producao {
                    d.text_color(apagado)
                } else {
                    d.text_color(aviso).font_weight(FontWeight::MEDIUM)
                }
            })
            .child(Icon::new(Icone::Globe).size(px(12.)))
            .when(!producao, |d| d.child("Fora da produção ·"))
            .child(endereco)
            .tooltip(move |window, cx| {
                let dica = if producao {
                    "O site em que as fotos e as vendas ficam"
                } else {
                    "Este app não está falando com o site de produção"
                };
                gpui_kit::component::tooltip::Tooltip::new(dica).build(window, cx)
            })
            .on_click(move |_, _window, cx| cx.open_url(&para_abrir));

        h_flex()
            .id("rodape")
            .flex_none()
            .h(px(ALTURA_DO_RODAPE))
            .px(px(6.))
            .gap(px(8.))
            .items_center()
            .border_t_1()
            .border_color(borda)
            .bg(fundo)
            .child(versao)
            .child(
                // O meio cresce e encolhe: a frase da faixa se corta antes de
                // empurrar a versão ou o servidor para fora.
                h_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .px(px(6.))
                    .items_center()
                    .when(destaque, |d| {
                        d.bg(aviso.opacity(0.15))
                            .border_x_1()
                            .border_color(aviso.opacity(0.4))
                    })
                    .children(meio),
            )
            .child(servidor)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_endereco_sai_sem_esquema_e_sem_barra() {
        assert_eq!(
            endereco_curto("https://recordarfotos.com.br/"),
            "recordarfotos.com.br"
        );
        assert_eq!(endereco_curto("http://localhost:8001"), "localhost:8001");
        assert_eq!(
            endereco_curto("recordarfotos.com.br"),
            "recordarfotos.com.br"
        );
    }
}
