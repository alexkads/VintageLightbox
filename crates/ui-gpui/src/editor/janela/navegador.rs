//! 🧭 Os painéis Navegador e Info do Photoshop (Janela › Navegador, Info).
//!
//! - **Navegador**: a foto inteira em miniatura (da vista reduzida, refeita
//!   quando o documento muda e não há traço em curso), o retângulo vermelho do
//!   que o palco mostra; clicar ou arrastar nela leva a vista para lá; − e +
//!   são os passos do zoom, com o zoom de agora entre eles;
//! - **Info**: R, G, B da foto sob o ponteiro (a cor composta, a mesma do
//!   conta-gotas), X e Y em pixels, e L × A da seleção (a caixa exata).

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    canvas, div, fill, img, point, prelude::*, px, size, AnyElement, Bounds, Context, MouseButton,
    MouseDownEvent, MouseMoveEvent, ObjectFit, Pixels, RenderImage,
};

use super::aparencia;
use super::EditorDeFoto;
use crate::recursos::Icone;
use crate::revelacao::zoom::Ponto;

/// O maior lado da miniatura do Navegador, em pixels.
const LADO_DO_NAVEGADOR: u32 = 320;

/// A miniatura do Navegador e a versão do documento de que ela saiu.
pub struct MiniaturaDoNavegador {
    versao: u64,
    imagem: Arc<RenderImage>,
}

fn f(p: Pixels) -> f32 {
    f32::from(p)
}

impl EditorDeFoto {
    /// Refaz a miniatura do Navegador quando o documento mudou (não a cada
    /// movimento do pincel).
    pub(super) fn atualizar_o_navegador(&mut self) {
        let Some(s) = self.sessao() else {
            return;
        };
        if s.tracando()
            || self
                .miniatura_do_navegador
                .as_ref()
                .is_some_and(|m| m.versao == s.versao())
        {
            return;
        }
        let versao = s.versao();
        let vista = s.vista().imagem();
        let (lv, av) = (vista.width().max(1), vista.height().max(1));
        let (l, a) = if lv >= av {
            (LADO_DO_NAVEGADOR, (LADO_DO_NAVEGADOR * av / lv).max(1))
        } else {
            ((LADO_DO_NAVEGADOR * lv / av).max(1), LADO_DO_NAVEGADOR)
        };
        let reduzida = image::imageops::thumbnail(vista, l, a);
        let bgra = reduzida
            .pixels()
            .flat_map(|p| [p[2], p[1], p[0], 255])
            .collect();
        if let Some(imagem) = crate::imagem::de_bgra(l, a, bgra) {
            self.miniatura_do_navegador = Some(MiniaturaDoNavegador { versao, imagem });
        }
    }

    /// O pedaço da foto que o palco mostra, em frações (x0, y0, x1, y1).
    pub fn area_visivel(&self) -> Option<(f32, f32, f32, f32)> {
        let (cena, v) = self.vista_do_zoom()?;
        let (l, a) = (
            cena.janela.largura * v.escala,
            cena.janela.altura * v.escala,
        );
        if l <= 0.0 || a <= 0.0 {
            return None;
        }
        let (pl, pa) = (f(self.palco.size.width), f(self.palco.size.height));
        Some((
            (-v.x / l).clamp(0.0, 1.0),
            (-v.y / a).clamp(0.0, 1.0),
            ((pl - v.x) / l).clamp(0.0, 1.0),
            ((pa - v.y) / a).clamp(0.0, 1.0),
        ))
    }

    /// O clique (ou arrasto) no Navegador: o palco centra ali.
    pub fn centrar_em(&mut self, fx: f32, fy: f32, cx: &mut Context<Self>) {
        self.zoom.centro = Ponto {
            x: fx.clamp(0.0, 1.0),
            y: fy.clamp(0.0, 1.0),
        };
        cx.notify();
    }

    pub(super) fn painel_do_navegador(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let Some(m) = self.miniatura_do_navegador.as_ref() else {
            return div().into_any_element();
        };
        let imagem = m.imagem.clone();
        let (li, ai) = (
            imagem.size(0).width.0 as f32,
            imagem.size(0).height.0 as f32,
        );
        let visivel = self.area_visivel();
        // A caixa da miniatura, medida no quadro (o clique vira fração dela).
        let caixa: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
        let fracao = {
            let caixa = caixa.clone();
            move |p: gpui_kit::Point<Pixels>| {
                let b = caixa.get()?;
                // A imagem fica contida (object-fit) no meio da caixa.
                let escala = (f(b.size.width) / li).min(f(b.size.height) / ai);
                let (w, h) = (li * escala, ai * escala);
                let x0 = f(b.origin.x) + (f(b.size.width) - w) / 2.0;
                let y0 = f(b.origin.y) + (f(b.size.height) - h) / 2.0;
                Some(((f(p.x) - x0) / w, (f(p.y) - y0) / h))
            }
        };
        let fracao_ao_mover = fracao.clone();
        let medida = {
            let caixa = caixa.clone();
            canvas(
                move |b, _, _| caixa.set(Some(b)),
                move |b, _, window, _| {
                    let Some((x0, y0, x1, y1)) = visivel else {
                        return;
                    };
                    let escala = (f(b.size.width) / li).min(f(b.size.height) / ai);
                    let (w, h) = (li * escala, ai * escala);
                    let ox = f(b.origin.x) + (f(b.size.width) - w) / 2.0;
                    let oy = f(b.origin.y) + (f(b.size.height) - h) / 2.0;
                    let r = Bounds::new(
                        point(px(ox + x0 * w), px(oy + y0 * h)),
                        size(px((x1 - x0) * w), px((y1 - y0) * h)),
                    );
                    let vermelho = gpui_kit::hsla(0.0, 0.85, 0.55, 1.0);
                    let t = px(1.5);
                    for faixa in [
                        Bounds::new(r.origin, size(r.size.width, t)),
                        Bounds::new(point(r.left(), r.bottom() - t), size(r.size.width, t)),
                        Bounds::new(r.origin, size(t, r.size.height)),
                        Bounds::new(point(r.right() - t, r.top()), size(t, r.size.height)),
                    ] {
                        window.paint_quad(fill(faixa, vermelho));
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };
        let zoom = self.porcentagem_do_zoom();
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h(px(0.))
            .p(px(6.))
            .gap(px(6.))
            .child(
                div()
                    .id("editor-navegador")
                    .debug_selector(|| "editor-navegador".into())
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .bg(c.poco)
                    .cursor_pointer()
                    // Por cima, solta: o tamanho dela não mexe na caixa.
                    .child(
                        img(imagem)
                            .object_fit(ObjectFit::Contain)
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full(),
                    )
                    .child(medida)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ed, e: &MouseDownEvent, window, cx| {
                            window.focus(&ed.foco, cx);
                            if let Some((x, y)) = fracao(e.position) {
                                ed.centrar_em(x, y, cx);
                            }
                        }),
                    )
                    .on_mouse_move(cx.listener(move |ed, e: &MouseMoveEvent, _, cx| {
                        if e.pressed_button == Some(MouseButton::Left) {
                            if let Some((x, y)) = fracao_ao_mover(e.position) {
                                ed.centrar_em(x, y, cx);
                            }
                        }
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        crate::estilo::botao_icone_pequeno(
                            "editor-navegador-menos",
                            Icone::ZoomOut,
                        )
                        .tooltip("Reduzir")
                        .on_click(cx.listener(|ed, _, _, cx| ed.passo_de_zoom(-1, cx))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_center()
                            .text_color(c.apagado)
                            .child(zoom),
                    )
                    .child(
                        crate::estilo::botao_icone_pequeno("editor-navegador-mais", Icone::ZoomIn)
                            .tooltip("Ampliar")
                            .on_click(cx.listener(|ed, _, _, cx| ed.passo_de_zoom(1, cx))),
                    ),
            )
            .into_any_element()
    }

    /// A caixa exata da seleção, guardada pela seleção que a deu (a conta
    /// varre os tiles; o painel redesenha a cada movimento do ponteiro).
    fn medida_da_selecao(&mut self) -> Option<(u32, u32)> {
        let s = self.sessao()?;
        let sel = s.selecao()?;
        let chave = s.versao_da_selecao();
        if let Some((c, l, a)) = self.selecao_medida {
            if c == chave {
                return Some((l, a));
            }
        }
        let r = sel.caixa_justa();
        self.selecao_medida = Some((chave, r.largura, r.altura));
        Some((r.largura, r.altura))
    }

    /// O que o Info mostra: a cor sob o ponteiro, o ponto (pixels da foto) e
    /// L × A da seleção.
    #[allow(clippy::type_complexity)]
    pub fn leitura_do_info(&mut self) -> (Option<[u8; 3]>, Option<(f32, f32)>, Option<(u32, u32)>) {
        let ponto = self.ponteiro.and_then(|p| self.na_foto(p));
        let cor = ponto.and_then(|(x, y)| self.sessao().and_then(|s| s.cor_em(x, y)));
        (cor, ponto, self.medida_da_selecao())
    }

    pub(super) fn painel_de_info(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let tema = cx.theme().clone();
        let (cor, ponto, medida) = self.leitura_do_info();
        let linha = |rotulo: &'static str, valor: String| {
            div()
                .flex()
                .gap(px(8.))
                .child(div().w(px(14.)).text_color(c.apagado).child(rotulo))
                .child(div().child(valor))
        };
        let traco = || "—".to_string();
        let coluna = || div().flex().flex_col().gap(px(2.)).flex_1();
        div()
            .debug_selector(|| "editor-info".into())
            .flex()
            .gap(px(12.))
            .p(px(8.))
            .text_xs()
            .text_color(tema.foreground)
            .child(
                coluna()
                    .child(linha("R", cor.map_or_else(traco, |c| c[0].to_string())))
                    .child(linha("G", cor.map_or_else(traco, |c| c[1].to_string())))
                    .child(linha("B", cor.map_or_else(traco, |c| c[2].to_string()))),
            )
            .child(
                coluna()
                    .child(linha(
                        "X",
                        ponto.map_or_else(traco, |p| (p.0 as u32).to_string()),
                    ))
                    .child(linha(
                        "Y",
                        ponto.map_or_else(traco, |p| (p.1 as u32).to_string()),
                    ))
                    .child(linha("L", medida.map_or_else(traco, |m| m.0.to_string())))
                    .child(linha("A", medida.map_or_else(traco, |m| m.1.to_string()))),
            )
            .into_any_element()
    }
}
