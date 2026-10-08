//! 📄 A aba do documento, em cima do palco, e a barra de status embaixo.
//!
//! - **Aba**: uma só (uma janela por foto — o contrato do editor), com o nome,
//!   o zoom, o formato do documento (o dos tiles: RGB/8) e o `•` de alteração
//!   pendente, e o × que fecha (perguntando, se houver o que salvar);
//! - **Status**: o zoom editável (Enter aplica), 100% e Encaixar; as medidas
//!   da foto; salvo / não salvo / salvando, com o Salvar; e o aviso curto da
//!   última ação. Ajuda comprida fica nas dicas e na Ajuda.

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Disableable as _, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, Entity, Focusable as _, Window};

use super::aparencia::{self, medida};
use super::ferramentas::na_plataforma;
use super::EditorDeFoto;
use crate::revelacao::zoom::{self, Nivel};

impl EditorDeFoto {
    /// O texto do zoom de agora ("25%").
    pub(super) fn porcentagem_do_zoom(&self) -> String {
        match self.zoom.nivel {
            Nivel::Razao(r) => zoom::porcentagem(r),
            _ => self
                .razao_do_zoom()
                .map(zoom::porcentagem)
                .unwrap_or_default(),
        }
    }

    /// O campo do zoom foi confirmado: o número vira a razão (100% é um
    /// pixel da foto num da tela).
    pub(super) fn campo_do_zoom_mudou(
        &mut self,
        campo: &Entity<InputState>,
        evento: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match evento {
            InputEvent::PressEnter { .. } => {
                let texto = campo.read(cx).value().to_string();
                let numero: Option<f32> = texto
                    .trim()
                    .trim_end_matches('%')
                    .trim()
                    .replace(',', ".")
                    .parse()
                    .ok();
                if let Some(p) = numero.filter(|p| p.is_finite() && *p > 0.0) {
                    let ponto = None;
                    self.ir_para_nivel(Nivel::Razao((p / 100.0).clamp(0.01, 32.0)), ponto, cx);
                }
                self.zoom_mostrado = None;
                window.focus(&self.foco, cx);
            }
            InputEvent::Blur => {
                self.zoom_mostrado = None;
                cx.notify();
            }
            _ => {}
        }
    }

    /// O campo do zoom acompanha o zoom (menos enquanto se digita nele).
    pub(super) fn sincronizar_o_campo_do_zoom(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let texto = self.porcentagem_do_zoom();
        if self.zoom_mostrado.as_deref() == Some(texto.as_str()) {
            return;
        }
        if self
            .campo_do_zoom
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
        {
            return;
        }
        self.campo_do_zoom
            .update(cx, |c, cx| c.set_value(texto.clone(), window, cx));
        self.zoom_mostrado = Some(texto);
    }

    /// A aba do documento, em cima do palco.
    pub(super) fn aba_do_documento(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let formato = format!("RGB/{}", editor_core::tiles::BITS_POR_CANAL);
        let zoom = self.porcentagem_do_zoom();
        let titulo = if zoom.is_empty() {
            format!("{} ({formato})", self.foto.nome)
        } else {
            format!("{} @ {zoom} ({formato})", self.foto.nome)
        };
        div()
            .flex()
            .flex_shrink_0()
            .items_end()
            .h(px(medida::ALTURA_DA_ABA))
            .bg(c.cromo)
            .border_b_1()
            .border_color(c.borda)
            .child(
                div()
                    .id("editor-aba-do-documento")
                    .debug_selector(|| "editor-aba-do-documento".into())
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h_full()
                    .max_w(px(520.))
                    .pl(px(10.))
                    .pr(px(4.))
                    .bg(c.poco)
                    .border_r_1()
                    .border_color(c.borda)
                    .text_size(crate::tema::letra::em(aparencia::LETRA))
                    .child(
                        div()
                            .min_w(px(0.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(titulo),
                    )
                    .when(self.alterado(), |d| {
                        d.child(
                            div()
                                .id("editor-alterado")
                                .debug_selector(|| "editor-alterado".into())
                                .text_color(crate::tema::cores::quente())
                                .child("•")
                                .tooltip(|window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new(
                                        "Alterações não salvas",
                                    )
                                    .build(window, cx)
                                }),
                        )
                    })
                    .child(
                        crate::estilo::botao_icone(
                            "editor-fechar",
                            crate::recursos::Icone::X,
                            18.,
                            11.,
                        )
                        .tooltip(na_plataforma("Fechar (⌘W)"))
                        .on_click(cx.listener(|ed, _, window, cx| ed.fechar(window, cx))),
                    ),
            )
            .into_any_element()
    }

    /// A barra de status, embaixo de tudo.
    pub(super) fn barra_de_status(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let pronta = self.pronta();
        let medidas = self.sessao().map(|s| {
            let d = s.documento();
            format!("{} × {} px", d.largura(), d.altura())
        });
        let estado = if self.salvando {
            "Salvando…"
        } else if self.alterado() {
            "Não salvo"
        } else {
            "Salvo"
        };
        let separador = || div().flex_shrink_0().w(px(1.)).h(px(14.)).bg(c.borda);
        div()
            .id("editor-barra-de-status")
            .debug_selector(|| "editor-barra-de-status".into())
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(medida::VAO))
            .h(px(medida::ALTURA_DO_STATUS))
            .px(px(6.))
            .bg(c.cromo)
            .border_t_1()
            .border_color(c.borda)
            .text_size(crate::tema::letra::em(aparencia::LETRA_MIUDA))
            .text_color(c.apagado)
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(62.))
                    .debug_selector(|| "editor-zoom".into())
                    .child(Input::new(&self.campo_do_zoom).xsmall().h(px(18.))),
            )
            .child(
                crate::estilo::botao_raso("editor-1-1")
                    .label("100%")
                    .tooltip(na_plataforma("Um pixel da foto num pixel da tela (⌘⌥0)"))
                    .disabled(!pronta)
                    .on_click(
                        cx.listener(|ed, _, _, cx| ed.ir_para_nivel(Nivel::Razao(1.0), None, cx)),
                    ),
            )
            .child(
                crate::estilo::botao_raso("editor-encaixar")
                    .label("Encaixar")
                    .tooltip(na_plataforma("A foto inteira na janela (⌘0)"))
                    .disabled(!pronta)
                    .on_click(
                        cx.listener(|ed, _, _, cx| ed.ir_para_nivel(Nivel::Encaixar, None, cx)),
                    ),
            )
            .child(separador())
            .children(medidas.map(|m| {
                div()
                    .flex_shrink_0()
                    .debug_selector(|| "editor-medidas-da-foto".into())
                    .child(m)
            }))
            .when(self.giro != 0.0, |d| {
                d.child(separador()).child(
                    div()
                        .flex_shrink_0()
                        .child(format!("Vista girada {:.0}°", self.giro.to_degrees())),
                )
            })
            .when(self.mostrando_antes(), |d| {
                d.child(separador())
                    .child(div().flex_shrink_0().text_color(c.texto).child("Antes (Y)"))
            })
            .child(separador())
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .debug_selector(|| "editor-aviso".into())
                    .when_some(self.aviso.clone(), |d, (texto, erro)| {
                        d.when(erro, |d| d.text_color(cx_theme_danger(cx)))
                            .child(texto)
                    }),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .debug_selector(|| "editor-estado-do-salvamento".into())
                    .when(self.alterado(), |d| {
                        d.text_color(crate::tema::cores::quente())
                    })
                    .child(estado),
            )
            .child(
                crate::estilo::botao_raso("editor-salvar")
                    .label(if self.salvando {
                        "Salvando…"
                    } else {
                        "Salvar"
                    })
                    .tooltip(na_plataforma("Salvar a edição (⌘S)"))
                    .disabled(!pronta || self.salvando)
                    .on_click(cx.listener(|ed, _, window, cx| ed.salvar(false, window, cx))),
            )
            .into_any_element()
    }
}

fn cx_theme_danger(cx: &gpui_kit::App) -> gpui_kit::Hsla {
    use gpui_kit::component::ActiveTheme as _;
    cx.theme().danger
}
