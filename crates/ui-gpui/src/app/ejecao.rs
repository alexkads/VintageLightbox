//! ⏏️ "Pode retirar o cartão" — ou "NÃO retire" — por cima de qualquer tela.
//!
//! 🔑 **Contundente de propósito** (dono, 24/out/2026): no Linux com PTP, o
//! cartão tirado sem ejetar quebra a importação seguinte, e um toast no canto
//! passa sem ser lido. Por isso é o `AlertDialog` do kit no meio da janela,
//! que só sai com o clique, com o som do aviso e, com a janela atrás de outra,
//! o aviso do sistema. A regra de quando ejetar mora em `importacao::ejecao`.

use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::{v_flex, ActiveTheme as _, Icon, WindowExt as _};
use gpui_kit::{div, prelude::*, px, Context, Hsla, SharedString, Task, Window};

use super::Aplicativo;
use crate::importacao::ejecao::{CartaoDoLote, Ejecao, Ejecoes};
use crate::recursos::Icone;

impl Aplicativo {
    /// Fica ouvindo os avisos da ejeção pelo resto da sessão.
    ///
    /// ⚠️ **Não acorda à toa**: espera o canal, sem relógio. Um laço com timer
    /// cobraria um quadro a cada volta pelo resto do dia.
    pub(super) fn ouvir_as_ejecoes(
        ejecoes: &Ejecoes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Task<()>> {
        let mut avisos = ejecoes.tomar_avisos()?;
        let ejecoes = ejecoes.clone();
        Some(cx.spawn_in(window, async move |raiz, cx| {
            while let Some(ejecao) = avisos.recv().await {
                let ejecoes = ejecoes.clone();
                if raiz
                    .update_in(cx, |raiz, window, cx| {
                        raiz.avisar_da_ejecao(ejecao, ejecoes, window, cx)
                    })
                    .is_err()
                {
                    break;
                }
            }
        }))
    }

    pub(crate) fn avisar_da_ejecao(
        &mut self,
        ejecao: Ejecao,
        ejecoes: Ejecoes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // O "Ejetando" que ainda está na tela sai: o resultado toma o lugar dele.
        if self.ejetando_na_tela {
            self.ejetando_na_tela = false;
            window.close_dialog(cx);
        }
        let (titulo, corpo) = textos(&ejecao);
        if let Ejecao::Ejetando { .. } = &ejecao {
            self.ejetando_na_tela = true;
            let raiz = cx.entity();
            window.open_alert_dialog(cx, move |dialogo, _window, cx| {
                let cor = cx.theme().warning;
                let raiz = raiz.clone();
                dialogo
                    .width(px(520.))
                    .icon(selo(Icone::LoaderCircle, cor))
                    .title(titulo_colorido(&titulo, cor))
                    .child(
                        v_flex()
                            .gap(px(12.))
                            .child(faixa("AGUARDE — NÃO RETIRE", Icone::TriangleAlert, cor))
                            .child(div().text_base().child(SharedString::from(corpo.clone()))),
                    )
                    .button_props(
                        DialogButtonProps::default()
                            .ok_text("Ok, vou esperar")
                            .show_cancel(false),
                    )
                    .on_ok(move |_ev, window, cx| {
                        raiz.update(cx, |raiz, _| raiz.ejetando_na_tela = false);
                        window.close_dialog(cx);
                        false
                    })
            });
            return;
        }
        match &ejecao {
            Ejecao::Ejetado { cartao } => {
                crate::sons::soar(
                    crate::sons::Evento::CartaoEjetado,
                    "Cartão ejetado",
                    &cartao.nome,
                    cx,
                );
            }
            Ejecao::Ejetando { .. } => {}
            Ejecao::NaoEjetado { .. } => {
                crate::sons::soar(
                    crate::sons::Evento::Falha,
                    "Falha",
                    "o cartão não foi ejetado, não retire",
                    cx,
                );
            }
        }
        if !window.is_window_active() {
            let (cliques, _) = std::sync::mpsc::channel();
            self.avisador.avisar(
                crate::tempo_real::Aviso {
                    titulo: titulo.clone(),
                    corpo: corpo.clone(),
                    destino: "cartao".into(),
                },
                cliques,
            );
        }

        let tentar: Option<CartaoDoLote> = match &ejecao {
            Ejecao::NaoEjetado { cartao, .. } => Some(cartao.clone()),
            Ejecao::Ejetado { .. } | Ejecao::Ejetando { .. } => None,
        };
        let deu_certo = tentar.is_none();

        window.open_alert_dialog(cx, move |dialogo, _window, cx| {
            let cor = if deu_certo {
                cx.theme().success
            } else {
                cx.theme().danger
            };
            let ejecoes = ejecoes.clone();
            let tentar = tentar.clone();
            let dialogo = dialogo
                .width(px(520.))
                .icon(selo(
                    if deu_certo {
                        Icone::CircleCheck
                    } else {
                        Icone::TriangleAlert
                    },
                    cor,
                ))
                .title(titulo_colorido(&titulo, cor))
                .child(
                    v_flex()
                        .gap(px(12.))
                        .child(if deu_certo {
                            faixa("PODE RETIRAR O CARTÃO", Icone::CircleCheck, cor)
                        } else {
                            faixa("NÃO RETIRE O CARTÃO", Icone::TriangleAlert, cor)
                        })
                        .child(div().text_base().child(SharedString::from(corpo.clone()))),
                );
            if deu_certo {
                dialogo
                    .button_props(
                        DialogButtonProps::default()
                            .ok_text("Entendi")
                            .show_cancel(false),
                    )
                    // 🚨 Fecha na hora e devolve `false` — o mesmo do
                    // "Sincronizar": deixado ao kit, o foco fica num diálogo
                    // que já saiu da tela.
                    .on_ok(|_ev, window, cx| {
                        window.close_dialog(cx);
                        false
                    })
            } else {
                dialogo
                    .button_props(
                        DialogButtonProps::default()
                            .ok_text("Tentar ejetar de novo")
                            .ok_variant(ButtonVariant::Danger)
                            .cancel_text("Fechar")
                            .show_cancel(true),
                    )
                    .on_ok(move |_ev, window, cx| {
                        if let Some(cartao) = tentar.clone() {
                            ejecoes.tentar_de_novo(cartao);
                        }
                        window.close_dialog(cx);
                        false
                    })
                    .on_cancel(|_ev, window, cx| {
                        window.close_dialog(cx);
                        false
                    })
            }
        });
    }
}

/// O título e o corpo do aviso — os mesmos no diálogo e no aviso do sistema.
pub(crate) fn textos(ejecao: &Ejecao) -> (String, String) {
    match ejecao {
        Ejecao::Ejetando { cartao } => (
            "Ejetando o cartão…".into(),
            format!(
                "A cópia terminou e o cartão «{}» está sendo ejetado. Espere o aviso de \
                 que pode retirar — tirar agora pode corromper o cartão e travar a \
                 próxima importação.",
                cartao.nome
            ),
        ),
        Ejecao::Ejetado { cartao } => (
            "Cartão ejetado — pode retirar".into(),
            format!(
                "A importação terminou e o cartão «{}» foi ejetado com segurança. \
                 Retire-o do leitor (ou desligue a câmera) agora.",
                cartao.nome
            ),
        ),
        Ejecao::NaoEjetado { cartao, motivo } => (
            "NÃO retire o cartão".into(),
            format!(
                "A importação terminou, mas o cartão «{}» não foi ejetado ({motivo}). \
                 Feche o que estiver usando o cartão e tente de novo. Tirar agora pode \
                 corromper o cartão e travar a próxima importação.",
                cartao.nome
            ),
        ),
    }
}

/// 🚨 **A faixa cheia, em maiúsculas** (dono, 24/out/2026: *"Aviso sonoro não
/// somente não adianta, também precisa ser visual"*). O som pode estar mudo e o
/// balconista de costas; a faixa é o que se lê de longe, do outro lado do
/// balcão.
fn faixa(texto: &'static str, icone: Icone, cor: Hsla) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .w_full()
        .py(px(18.))
        .px(px(16.))
        .rounded(crate::tema::canto(8.))
        .bg(cor)
        .text_color(gpui_kit::white())
        .text_2xl()
        .font_weight(gpui_kit::FontWeight::BOLD)
        .child(Icon::new(icone).size(px(32.)))
        .child(texto)
}

fn titulo_colorido(titulo: &str, cor: Hsla) -> impl IntoElement {
    div()
        .text_xl()
        .font_weight(gpui_kit::FontWeight::BOLD)
        .text_color(cor)
        .child(SharedString::from(titulo.to_string()))
}

/// O ícone grande ao lado do título, num quadrado tingido da cor do aviso.
fn selo(icone: Icone, cor: Hsla) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(48.))
        .rounded(crate::tema::canto(10.))
        .bg(cor.opacity(0.14))
        .text_color(cor)
        .child(Icon::new(icone).size(px(28.)))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn cartao() -> CartaoDoLote {
        CartaoDoLote {
            nome: "NIKON D3100".into(),
            raiz: "/run/media/alex/NIKON D3100".into(),
        }
    }

    #[test]
    fn ejetado_diz_que_pode_retirar_e_o_nome_do_cartao() {
        let (titulo, corpo) = textos(&Ejecao::Ejetado { cartao: cartao() });
        assert!(titulo.contains("pode retirar"));
        assert!(corpo.contains("NIKON D3100"));
    }

    #[test]
    fn preso_diz_que_nao_pode_retirar_e_por_que() {
        let (titulo, corpo) = textos(&Ejecao::NaoEjetado {
            cartao: cartao(),
            motivo: "o cartão está em uso".into(),
        });
        assert_eq!(titulo, "NÃO retire o cartão");
        assert!(corpo.contains("o cartão está em uso"));
    }
}
