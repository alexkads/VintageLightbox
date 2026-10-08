//! 🎭 As Propriedades da máscara e os cadeados do painel Camadas (etapa 16):
//! densidade, difusão, inverter, aplicar, ligar e desligar, a máscara à vista
//! (sozinha ou em rubi), a corrente com a camada, e "Bloquear:" transparência,
//! pixels, posição e tudo.

use editor_core::{Cadeado, Exibicao, Sessao};
use gpui_kit::component::{ActiveTheme, Disableable, Selectable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, Context};

use super::EditorDeFoto;
use crate::recursos::Icone;

impl EditorDeFoto {
    /// O aviso de quando um gesto esbarra num cadeado.
    pub(super) fn avisar_cadeado(&mut self, gesto: &str, cx: &mut Context<Self>) {
        let Some(s) = self.sessao() else {
            return;
        };
        let b = s.camada_ativa().bloqueio;
        let qual = if b.tudo() {
            "a camada está toda bloqueada"
        } else if b.pixels && !s.na_mascara() {
            "os pixels da camada estão bloqueados"
        } else if b.posicao {
            "a posição da camada está bloqueada"
        } else {
            "a camada está bloqueada"
        };
        self.aviso = Some((
            format!("Não dá para {gesto}: {qual} — solte o cadeado no painel Camadas").into(),
            true,
        ));
        cx.notify();
    }

    /// Com os pixels da escolhida bloqueados (e o gesto neles), avisa e
    /// devolve `true`.
    pub(super) fn recusar_se_bloqueada(&mut self, gesto: &str, cx: &mut Context<Self>) -> bool {
        if !self.sessao().is_some_and(Sessao::pixels_bloqueados) {
            return false;
        }
        self.avisar_cadeado(gesto, cx);
        true
    }

    /// A corrente entre a miniatura da camada e a da máscara.
    pub fn alternar_vinculo_de(&mut self, indice: usize, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.alternar_vinculo_de(indice);
        });
    }

    /// A corrente da escolhida (o menu da camada).
    pub fn alternar_vinculo_de_ativa(&mut self, cx: &mut Context<Self>) {
        if let Some(i) = self.sessao().map(Sessao::ativa) {
            self.alternar_vinculo_de(i, cx);
        }
    }

    /// ⌘I: a máscara escolhida, ou o negativo das cores da camada.
    pub fn inverter(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.sessao() else {
            return;
        };
        if s.pixels_bloqueados() {
            self.avisar_cadeado("inverter", cx);
            return;
        }
        self.na_sessao(cx, |s| {
            s.inverter();
        });
    }

    /// "Inverter" nas Propriedades da máscara.
    pub fn inverter_mascara(&mut self, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            let i = s.ativa();
            s.inverter_mascara_de(i);
        });
    }

    /// "Aplicar máscara": ela entra no alfa da camada e sai.
    pub fn aplicar_mascara(&mut self, cx: &mut Context<Self>) {
        let bloqueada = self
            .sessao()
            .is_some_and(|s| s.camada_ativa().bloqueio.pixels);
        if bloqueada {
            self.avisar_cadeado("aplicar a máscara", cx);
            return;
        }
        self.na_sessao(cx, |s| {
            s.aplicar_mascara();
        });
    }

    /// ⌥ + clique na miniatura da máscara: só ela no palco, em cinza.
    pub fn alternar_so_a_mascara(&mut self, indice: usize, cx: &mut Context<Self>) {
        self.na_sessao(cx, |s| {
            s.alternar_so_a_mascara(indice);
        });
    }

    /// `\`: a máscara da escolhida em rubi sobre a foto.
    pub fn alternar_rubi(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.sessao_mut() else {
            return;
        };
        if !s.alternar_rubi() {
            self.aviso = Some((
                "A camada escolhida não tem máscara para mostrar em rubi".into(),
                true,
            ));
        } else {
            self.aviso = None;
        }
        cx.notify();
    }

    /// O que o palco mostra (a foto, ou a máscara).
    pub fn exibicao(&self) -> Exibicao {
        self.sessao().map_or(Exibicao::Foto, Sessao::exibicao)
    }

    /// Um cadeado do painel Camadas na camada `indice`.
    pub fn alternar_bloqueio_de(
        &mut self,
        indice: usize,
        cadeado: Cadeado,
        cx: &mut Context<Self>,
    ) {
        self.na_sessao(cx, |s| {
            s.alternar_bloqueio_de(indice, cadeado);
        });
    }

    /// `/`: o cadeado da transparência da escolhida.
    pub fn bloquear_transparencia(&mut self, cx: &mut Context<Self>) {
        if let Some(i) = self.sessao().map(Sessao::ativa) {
            self.alternar_bloqueio_de(i, Cadeado::Transparencia, cx);
        }
    }

    /// O slider da densidade (0–100) andou, ou soltou: vira um passo.
    pub fn mover_densidade_da_mascara(&mut self, valor: f32, soltou: bool, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.mover_densidade(valor / 100.0);
            if soltou {
                s.confirmar_mascara();
            }
        }
        cx.notify();
    }

    /// O slider da difusão (px) andou, ou soltou.
    pub fn mover_difusao_da_mascara(&mut self, valor: f32, soltou: bool, cx: &mut Context<Self>) {
        if let Some(s) = self.sessao_mut() {
            s.mover_difusao(valor);
            if soltou {
                s.confirmar_mascara();
            }
        }
        cx.notify();
    }

    /// Arrastar a camada `de` no painel e soltar sobre a linha `para` (o
    /// roteiro e os testes): um passo.
    pub fn arrastar_camada(&mut self, de: usize, para: usize, cx: &mut Context<Self>) {
        if de == para {
            return;
        }
        self.na_sessao(cx, |s| {
            s.escolher_camada(de);
            s.mover_camada_para(para);
        });
    }

    /// A linha de uma camada foi apertada: um arrasto pode começar.
    pub(super) fn apertou_na_camada(&mut self) {
        self.historico_no_aperto_da_camada = self.sessao().map(|s| s.historico().posicao());
    }

    /// O arrasto da escolhida passou sobre a linha `para`: ela vai para lá
    /// na hora, e o arrasto inteiro continua um passo só do desfazer.
    pub(super) fn arrastar_camada_ate(&mut self, para: usize, cx: &mut Context<Self>) {
        let desde = self.historico_no_aperto_da_camada;
        let Some(s) = self.sessao_mut() else {
            return;
        };
        if s.ativa() == para {
            return;
        }
        if s.mover_camada_arrastando(para, desde) {
            self.aviso = None;
            cx.notify();
        }
    }

    /// As Propriedades da máscara escolhida, como o painel do Photoshop:
    /// densidade, difusão e os botões. `None` fora da máscara.
    pub(super) fn propriedades_da_mascara(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let s = self.sessao()?;
        if !s.na_mascara() {
            return None;
        }
        let indice = s.ativa();
        let camada = s.camada_ativa();
        let m = camada.mascara.as_ref()?;
        let (densidade, difusao, ligada, vinculada) =
            (m.densidade, m.difusao, m.ativa, m.vinculada);
        let de_pixels = camada.ajuste.is_none();
        let exibicao = s.exibicao();
        let tema = cx.theme().clone();
        let rotulo = |nome: &'static str, valor: String| {
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(tema.muted_foreground)
                .child(nome)
                .child(valor)
        };
        let botao = |id: &'static str, texto: &'static str, dica: &'static str| {
            crate::estilo::botao_secundario_pequeno(id, cx)
                .debug_selector(move || id.into())
                .label(texto)
                .tooltip(dica)
        };
        Some(
            div()
                .debug_selector(|| "editor-propriedades-da-mascara".into())
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .child(format!("Máscara de {}", camada.nome)),
                )
                .child(rotulo("Densidade", format!("{:.0}%", densidade * 100.0)))
                .child(
                    div()
                        .h(px(20.))
                        .debug_selector(|| "editor-mascara-densidade".into())
                        .child(crate::estilo::slider(&self.densidade_da_mascara)),
                )
                .child(rotulo("Difusão", format!("{difusao:.1} px")))
                .child(
                    div()
                        .h(px(20.))
                        .debug_selector(|| "editor-mascara-difusao".into())
                        .child(crate::estilo::slider(&self.difusao_da_mascara)),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .child(
                            botao("editor-mascara-inverter", "Inverter", "Inverter a máscara (⌘I)")
                                .on_click(cx.listener(|ed, _, _, cx| ed.inverter_mascara(cx))),
                        )
                        .child(
                            botao(
                                "editor-mascara-aplicar",
                                "Aplicar",
                                "Aplicar a máscara: ela entra na transparência da camada e sai",
                            )
                            .disabled(!de_pixels)
                            .on_click(cx.listener(|ed, _, _, cx| ed.aplicar_mascara(cx))),
                        )
                        .child(
                            botao(
                                "editor-mascara-ligar",
                                if ligada { "Desligar" } else { "Ligar" },
                                "Desligar ou ligar a máscara (⇧ + clique na miniatura)",
                            )
                            .on_click(cx.listener(move |ed, _, _, cx| {
                                ed.alternar_mascara_de(indice, cx)
                            })),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.))
                        .child(
                            botao(
                                "editor-mascara-ver",
                                "Ver só a máscara",
                                "A máscara sozinha no palco, em cinza (⌥ + clique na miniatura)",
                            )
                            .selected(exibicao == Exibicao::SoAMascara(indice))
                            .on_click(cx.listener(move |ed, _, _, cx| {
                                ed.alternar_so_a_mascara(indice, cx)
                            })),
                        )
                        .child(
                            botao(
                                "editor-mascara-rubi",
                                "Rubi",
                                "O que a máscara esconde em vermelho sobre a foto (\\)",
                            )
                            .selected(exibicao == Exibicao::Rubi(indice))
                            .on_click(cx.listener(|ed, _, _, cx| ed.alternar_rubi(cx))),
                        )
                        .when(de_pixels, |d| {
                            d.child(
                                botao(
                                    "editor-mascara-vinculo",
                                    if vinculada { "Vinculada" } else { "Solta" },
                                    "Vinculada, o Mover, o ⌘T e o Deformar levam a camada e a máscara juntas (a corrente entre as miniaturas)",
                                )
                                .selected(vinculada)
                                .on_click(cx.listener(move |ed, _, _, cx| {
                                    ed.alternar_vinculo_de(indice, cx)
                                })),
                            )
                        }),
                )
                .into_any_element(),
        )
    }

    /// "Bloquear:" do painel Camadas — os quatro cadeados da escolhida.
    pub(super) fn barra_de_cadeados(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme().clone();
        let (indice, b) = self
            .sessao()
            .map(|s| (s.ativa(), s.camada_ativa().bloqueio))
            .unwrap_or_default();
        let ligado = self.sessao().is_some();
        let cadeado =
            |id: &'static str, icone: Icone, dica: &'static str, ativo: bool, qual: Cadeado| {
                crate::estilo::botao_icone_pequeno(id, icone)
                    .debug_selector(move || id.into())
                    .tooltip(dica)
                    .selected(ativo)
                    .disabled(!ligado)
                    .on_click(
                        cx.listener(move |ed, _, _, cx| ed.alternar_bloqueio_de(indice, qual, cx)),
                    )
            };
        div()
            .flex()
            .items_center()
            .gap(px(2.))
            .child(
                div()
                    .text_xs()
                    .text_color(tema.muted_foreground)
                    .mr(px(4.))
                    .child("Bloquear:"),
            )
            .child(cadeado(
                "editor-cadeado-transparencia",
                Icone::Transparencia,
                "Bloquear pixels transparentes (/): a tinta muda só a cor do que existe",
                b.transparencia,
                Cadeado::Transparencia,
            ))
            .child(cadeado(
                "editor-cadeado-pixels",
                Icone::Paintbrush,
                "Bloquear pixels: nada pinta, apaga ou preenche a camada (a máscara continua aberta)",
                b.pixels,
                Cadeado::Pixels,
            ))
            .child(cadeado(
                "editor-cadeado-posicao",
                Icone::Move,
                "Bloquear posição: o Mover, o ⌘T e o Deformar não levam a camada",
                b.posicao,
                Cadeado::Posicao,
            ))
            .child(cadeado(
                "editor-cadeado-tudo",
                Icone::Lock,
                "Bloquear tudo: pixels, posição, transparência, opacidade e modo",
                b.tudo(),
                Cadeado::Tudo,
            ))
            .into_any_element()
    }
}

/// A camada sendo arrastada no painel Camadas.
pub(super) struct ArrastoDeCamada {
    pub nome: gpui_kit::SharedString,
}

/// O que acompanha o ponteiro enquanto a camada é arrastada.
pub(super) struct FantasmaDaCamada {
    pub nome: gpui_kit::SharedString,
}

impl gpui_kit::Render for FantasmaDaCamada {
    fn render(
        &mut self,
        _window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(4.))
            .rounded(crate::tema::canto(4.))
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .opacity(0.9)
            .text_sm()
            .text_color(cx.theme().foreground)
            .child(self.nome.clone())
    }
}
