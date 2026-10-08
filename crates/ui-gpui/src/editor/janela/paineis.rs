//! 🎨 Os painéis Cor, Amostras, Propriedades e Pincel do editor.
//!
//! - **Cor**: a frente e o fundo (clicar escolhe qual editar) e R, G, B da
//!   escolhida — a mesma cor do pincel, sem cópia;
//! - **Amostras**: as cores prontas; clique é a frente, ⌘/Ctrl + clique o
//!   fundo (na máscara, o cinza delas);
//! - **Propriedades**: acompanha o alvo — o ajuste da camada, a máscara, ou
//!   o que a camada de pixels tem —, e embaixo o documento. As opções da
//!   ferramenta moram na barra de opções, não aqui;
//! - **Pincel**: as configurações avançadas (tamanho, dureza, espaçamento,
//!   suavização), sobre os mesmos estados da barra de opções.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent};

use super::aparencia;
use super::ferramentas::na_plataforma;
use super::{EditorDeFoto, AMOSTRAS};
use crate::recursos::Icone;

fn cor_de(c: [u8; 3]) -> gpui_kit::Hsla {
    gpui_kit::rgb((c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32).into()
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// O corpo de um painel: rola por inteiro, com o respiro das docas.
fn corpo(id: &'static str) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(px(8.))
        .p(px(8.))
        .size_full()
        .min_h(px(0.))
        .overflow_y_scroll()
}

impl EditorDeFoto {
    /// A frente e o fundo, e R G B da escolhida.
    pub(super) fn painel_de_cor(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let (frente, fundo) = self
            .sessao()
            .map(|s| (s.pincel.cor, s.pincel.cor_de_fundo))
            .unwrap_or(([0; 3], [255; 3]));
        let no_fundo = self.cor_do_painel_e_o_fundo;
        let editada = if no_fundo { fundo } else { frente };
        let quadrado = |id: &'static str, cor: [u8; 3], escolhido: bool, fundo: bool| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .size(px(30.))
                .bg(cor_de(cor))
                .border_2()
                .border_color(if escolhido { c.alvo } else { c.borda })
                .cursor_pointer()
                .on_click(cx.listener(move |ed, _, _, cx| {
                    ed.cor_do_painel_e_o_fundo = fundo;
                    cx.notify();
                }))
        };
        let canal = |nome: &'static str, i: usize, valor: u8, ed: &EditorDeFoto| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().w(px(12.)).text_xs().text_color(c.apagado).child(nome))
                .child(
                    div()
                        .flex_1()
                        .h(px(20.))
                        .debug_selector(move || format!("editor-cor-canal-{i}"))
                        .child(crate::estilo::slider(&ed.rgb_do_painel[i])),
                )
                .child(
                    div()
                        .w(px(28.))
                        .text_xs()
                        .text_color(c.apagado)
                        .child(format!("{valor}")),
                )
        };
        corpo("editor-corpo-cor")
            .child(
                div()
                    .flex()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .relative()
                                    .flex_shrink_0()
                                    .size(px(46.))
                                    .child(div().absolute().left(px(16.)).top(px(16.)).child(
                                        quadrado("editor-cor-fundo-painel", fundo, no_fundo, true),
                                    ))
                                    .child(div().absolute().left(px(0.)).top(px(0.)).child(
                                        quadrado(
                                            "editor-cor-frente-painel",
                                            frente,
                                            !no_fundo,
                                            false,
                                        ),
                                    )),
                            )
                            .child(div().text_xs().text_color(c.apagado).child(hex(editada))),
                    )
                    .child(if self.na_mascara() {
                        // Na máscara, como no Photoshop: um cinza só (K),
                        // preto esconde e branco revela.
                        div()
                            .id("editor-cor-cinza")
                            .flex_1()
                            .min_w(px(0.))
                            .child(canal("K", 0, editada[0], self))
                            .tooltip(|window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(
                                    "Na máscara as cores viram cinza: preto esconde, branco revela",
                                )
                                .build(window, cx)
                            })
                            .into_any_element()
                    } else {
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(canal("R", 0, editada[0], self))
                            .child(canal("G", 1, editada[1], self))
                            .child(canal("B", 2, editada[2], self))
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }

    /// Uma cor do painel Cor mudou (um dos três canais).
    pub(super) fn canal_do_painel_mudou(
        &mut self,
        canal: usize,
        valor: f32,
        cx: &mut Context<Self>,
    ) {
        let no_fundo = self.cor_do_painel_e_o_fundo;
        let na_mascara = self.na_mascara();
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let mut cor = if no_fundo {
            s.pincel.cor_de_fundo
        } else {
            s.pincel.cor
        };
        let v = valor.round().clamp(0.0, 255.0) as u8;
        if na_mascara {
            // O K do painel na máscara: o cinza nos três canais.
            cor = [v; 3];
        } else {
            cor[canal.min(2)] = v;
        }
        if no_fundo {
            s.definir_cor_de_fundo(cor);
        } else {
            s.definir_cor_de_frente(cor);
        }
        cx.notify();
    }

    /// As amostras: clique é a frente, ⌘/Ctrl + clique o fundo.
    pub(super) fn painel_de_amostras(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let frente = self.sessao().map(|s| s.pincel.cor);
        corpo("editor-corpo-amostras")
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .children(AMOSTRAS.iter().enumerate().map(|(i, cor)| {
                        let cor = *cor;
                        div()
                            .id(("editor-cor", i))
                            .debug_selector(move || format!("editor-amostra-{i}"))
                            .size(px(20.))
                            .bg(cor_de(cor))
                            .border_1()
                            .border_color(if frente == Some(cor) { c.alvo } else { c.borda })
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |ed, e: &MouseDownEvent, window, cx| {
                                    if e.modifiers.secondary() {
                                        if let Some(s) = ed.sessao_mut() {
                                            s.definir_cor_de_fundo(cor);
                                        }
                                        cx.notify();
                                    } else {
                                        ed.escolher_cor(cor, cx);
                                    }
                                    window.focus(&ed.foco, cx);
                                }),
                            )
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(c.apagado)
                    .child(na_plataforma("Clique: frente · ⌘ + clique: fundo")),
            )
            .into_any_element()
    }

    /// As Propriedades do alvo, e as do documento embaixo.
    pub(super) fn painel_de_propriedades(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let tema = cx.theme().clone();
        let mut coluna = corpo("editor-corpo-propriedades");
        // Numa camada de ajuste o pincel sempre pinta na máscara: o aviso só
        // vale para a máscara de uma camada de pixels.
        if self.na_mascara() && self.ajuste_da_camada().is_none() {
            let nome = self
                .sessao()
                .map(|s| s.camada_ativa().nome.clone())
                .unwrap_or_default();
            coluna = coluna.child(
                div()
                    .debug_selector(|| "editor-na-mascara".into())
                    .text_xs()
                    .px(px(6.))
                    .py(px(4.))
                    .bg(tema.muted)
                    .child(format!(
                        "Pintando na máscara de {nome}: preto esconde, branco revela. X troca as cores, D volta a preto e branco"
                    )),
            );
        }
        let mut alguma = false;
        if let Some(a) = self.ajuste_da_camada() {
            coluna = coluna.child(self.propriedades_do_ajuste(a, cx));
            alguma = true;
        }
        if let Some(m) = self.propriedades_da_mascara(cx) {
            coluna = coluna.child(m);
            alguma = true;
        }
        if !alguma {
            if let Some(info) = self.propriedades_da_camada(cx) {
                coluna = coluna.child(info);
            }
        }
        coluna
            .child(div().h(px(1.)).bg(c.borda))
            .child(self.propriedades_do_documento(cx))
            .into_any_element()
    }

    /// A camada de pixels: o que dá para saber dela.
    fn propriedades_da_camada(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = aparencia::cores(cx);
        let s = self.sessao()?;
        let camada = s.camada_ativa();
        let linha = |nome: &'static str, valor: String| {
            div()
                .flex()
                .justify_between()
                .gap(px(8.))
                .text_xs()
                .child(div().text_color(c.apagado).child(nome))
                .child(valor)
        };
        // A caixa dos ladrilhos com pixels (256 px cada) — barata de saber a
        // cada quadro; a borda exata pediria varrer os pixels.
        let lado = editor_core::LADO_DO_TILE as i64;
        let caixa =
            camada
                .pixels
                .existentes()
                .fold(None, |c: Option<(i64, i64, i64, i64)>, (p, _)| {
                    let (x, y) = (p.0 as i64, p.1 as i64);
                    Some(c.map_or((x, y, x, y), |(a, b, c, d)| {
                        (a.min(x), b.min(y), c.max(x), d.max(y))
                    }))
                });
        let conteudo = match caixa {
            None => "vazia".to_string(),
            Some((x0, y0, x1, y1)) => {
                let (l, a) = (
                    s.documento().largura() as i64,
                    s.documento().altura() as i64,
                );
                let (px0, py0) = (x0 * lado, y0 * lado);
                let (px1, py1) = (((x1 + 1) * lado).min(l), ((y1 + 1) * lado).min(a));
                format!(
                    "até {} × {} px ({} ladrilhos)",
                    px1 - px0,
                    py1 - py0,
                    camada.pixels.quantos()
                )
            }
        };
        let mut bloqueios = Vec::new();
        if camada.bloqueio.algum() {
            bloqueios.push(format!("{:?}", camada.bloqueio));
        }
        Some(
            div()
                .debug_selector(|| "editor-propriedades-da-camada".into())
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .child(format!("Camada de pixels — {}", camada.nome)),
                )
                .child(linha("Conteúdo", conteudo))
                .child(linha("Mesclagem", camada.modo.nome().to_string()))
                .child(linha(
                    "Opacidade",
                    format!("{:.0}%", camada.opacidade * 100.0),
                ))
                .child(linha(
                    "Visível",
                    if camada.visivel { "sim" } else { "não" }.into(),
                ))
                .child(linha(
                    "Máscara",
                    if camada.mascara.is_some() {
                        "sim (clique na miniatura para editá-la)"
                    } else {
                        "não"
                    }
                    .into(),
                ))
                .when(camada.recortada, |d| {
                    d.child(linha("Recorte", "na camada de baixo".into()))
                })
                .when(!bloqueios.is_empty(), |d| {
                    d.child(linha("Bloqueios", bloqueios.join(", ")))
                })
                .into_any_element(),
        )
    }

    /// O documento: medidas, formato e camadas.
    fn propriedades_do_documento(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let linha = |nome: &'static str, valor: String| {
            div()
                .flex()
                .justify_between()
                .gap(px(8.))
                .text_xs()
                .child(div().text_color(c.apagado).child(nome))
                .child(valor)
        };
        let Some(s) = self.sessao() else {
            return div().into_any_element();
        };
        let doc = s.documento();
        div()
            .debug_selector(|| "editor-propriedades-do-documento".into())
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child("Documento"),
            )
            .child(linha(
                "Tamanho",
                format!("{} × {} px", doc.largura(), doc.altura()),
            ))
            .child(linha(
                "Modo",
                format!("RGB, {} bits/canal", editor_core::tiles::BITS_POR_CANAL),
            ))
            .child(linha("Camadas", format!("{}", doc.camadas.len())))
            .into_any_element()
    }

    /// As configurações do pincel: os mesmos estados da barra de opções.
    pub(super) fn painel_do_pincel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let p = self.sessao().map(|s| s.pincel).unwrap_or_default();
        let linha = |nome: &'static str, valor: String| {
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(c.apagado)
                .child(nome)
                .child(valor)
        };
        let slider =
            |seletor: &'static str,
             estado: &gpui_kit::Entity<gpui_kit::component::slider::SliderState>| {
                div()
                    .h(px(20.))
                    .debug_selector(move || seletor.into())
                    .child(crate::estilo::slider(estado))
            };
        corpo("editor-corpo-pincel")
            .child(linha("Tamanho  [ ]", format!("{:.0} px", p.raio * 2.0)))
            .child(slider("editor-pincel-tamanho", &self.tamanho))
            .child(linha("Dureza  { }", format!("{:.0}%", p.dureza * 100.0)))
            .child(slider("editor-pincel-dureza", &self.dureza))
            .child(linha("Espaçamento", format!("{:.0}%", p.espacamento * 100.0)))
            .child(slider("editor-espacamento", &self.espacamento))
            .child(linha("Suavização", format!("{:.0}%", p.suavizacao * 100.0)))
            .child(slider("editor-pincel-suavizacao", &self.suavizacao_do_pincel))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_xs()
                    .text_color(c.apagado)
                    .child(gpui_kit::component::Icon::new(Icone::Info).size(px(12.)))
                    .child(na_plataforma(if cfg!(target_os = "macos") {
                        "⇧ + clique: reta desde o último ponto · ⌃⌥ + arrasto: tamanho e dureza"
                    } else {
                        "⇧ + clique: reta desde o último ponto · ⌥ + botão direito arrastando: tamanho e dureza"
                    })),
            )
            .into_any_element()
    }
}
