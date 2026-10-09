//! 🌫️ Filtro › Desfoque gaussiano… e Máscara de nitidez… — o diálogo do
//! Photoshop: uma caixa pequena por cima do palco, **sem escurecer a foto**,
//! porque a prévia é a própria foto ("Visualizar" ligado).
//!
//! - A conta ([`editor_core::filtros::filtrada`]) corre em segundo plano a
//!   cada mudança de controle (com um respiro de 80 ms para o arrasto não
//!   empilhar contas); a de uma geração velha é jogada fora;
//! - OK grava **um** passo com o nome do filtro (se a prévia ainda está
//!   correndo, a conta termina ali mesmo); Cancelar e Esc voltam a camada;
//! - os valores ficam para a próxima vez, como no Photoshop;
//! - o raio anda numa escala exponencial (0,1 a 250 px): perto do zero o
//!   controle é fino, e o fim ainda alcança o desfoque forte.

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, Context, Entity, MouseButton,
    Subscription, Task, Window,
};

use super::aparencia;
use super::EditorDeFoto;
use editor_core::filtros::{filtrada, Filtro, RAIO_MAXIMO};
use editor_core::selecao::Selecao;
use editor_core::tiles::CamadaDePixels;

/// O menor raio, em px.
const RAIO_MINIMO: f32 = 0.1;
/// A largura da caixa, em pontos.
const LARGURA: f32 = 300.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tipo {
    Desfoque,
    Nitidez,
}

impl Tipo {
    pub fn titulo(self) -> &'static str {
        match self {
            Tipo::Desfoque => "Desfoque gaussiano",
            Tipo::Nitidez => "Máscara de nitidez",
        }
    }
}

/// O que os controles valiam no último OK (a próxima abertura começa deles).
#[derive(Clone, Copy, Debug)]
struct Lembrado {
    raio_do_desfoque: f32,
    quantidade: f32,
    raio_da_nitidez: f32,
    limiar: f32,
}

impl Default for Lembrado {
    fn default() -> Self {
        // Os padrões do Photoshop: desfoque 1 px... que mal se vê numa foto
        // de 24 MP; aqui 4 px. Nitidez 100%, 1 px, limiar 0.
        Self {
            raio_do_desfoque: 4.0,
            quantidade: 100.0,
            raio_da_nitidez: 1.0,
            limiar: 0.0,
        }
    }
}

pub struct Aberto {
    pub tipo: Tipo,
    original: CamadaDePixels,
    selecao: Option<Arc<Selecao>>,
}

/// O estado do diálogo dentro do editor.
pub struct Estado {
    pub aberto: Option<Aberto>,
    raio: Entity<SliderState>,
    quantidade: Entity<SliderState>,
    limiar: Entity<SliderState>,
    pub visualizar: bool,
    lembrado: Lembrado,
    /// Sobe a cada pedido de conta; `mostrada` é a que está na tela.
    geracao: u64,
    mostrada: u64,
    _conta: Option<Task<()>>,
    _assinaturas: Vec<Subscription>,
}

/// Raio (px) ↔ posição do slider (0–100), exponencial.
pub fn raio_da_posicao(p: f32) -> f32 {
    let p = p.clamp(0.0, 100.0) / 100.0;
    let r = RAIO_MINIMO * (RAIO_MAXIMO / RAIO_MINIMO).powf(p);
    // Um décimo de pixel.
    (r * 10.0).round() / 10.0
}

pub fn posicao_do_raio(r: f32) -> f32 {
    let r = r.clamp(RAIO_MINIMO, RAIO_MAXIMO);
    100.0 * (r / RAIO_MINIMO).ln() / (RAIO_MAXIMO / RAIO_MINIMO).ln()
}

fn numero(x: f32) -> String {
    let t = format!("{:.1}", x);
    let t = t.strip_suffix(".0").map(str::to_string).unwrap_or(t);
    t.replace('.', ",")
}

impl Estado {
    pub fn novo(window: &mut Window, cx: &mut Context<EditorDeFoto>) -> Self {
        let l = Lembrado::default();
        let novo = |min: f32, max: f32, passo: f32, v: f32, cx: &mut Context<EditorDeFoto>| {
            cx.new(|_| {
                SliderState::new()
                    .min(min)
                    .max(max)
                    .step(passo)
                    .default_value(v)
            })
        };
        let raio = novo(0.0, 100.0, 0.1, posicao_do_raio(l.raio_do_desfoque), cx);
        let quantidade = novo(1.0, 500.0, 1.0, l.quantidade, cx);
        let limiar = novo(0.0, 255.0, 1.0, l.limiar, cx);
        let assinaturas = [&raio, &quantidade, &limiar]
            .into_iter()
            .map(|s| {
                cx.subscribe_in(
                    s,
                    window,
                    |ed: &mut EditorDeFoto, _s, _e: &SliderEvent, _w, cx| {
                        ed.controle_do_filtro_mudou(cx)
                    },
                )
            })
            .collect();
        Self {
            aberto: None,
            raio,
            quantidade,
            limiar,
            visualizar: true,
            lembrado: l,
            geracao: 0,
            mostrada: 0,
            _conta: None,
            _assinaturas: assinaturas,
        }
    }
}

impl EditorDeFoto {
    pub fn filtro_aberto(&self) -> Option<Tipo> {
        self.filtro.aberto.as_ref().map(|a| a.tipo)
    }

    /// Filtro › … : abre o diálogo na camada escolhida (ou na máscara dela).
    pub fn abrir_filtro(&mut self, tipo: Tipo, window: &mut Window, cx: &mut Context<Self>) {
        if self.filtro.aberto.is_some() || self.area_do_preenchimento.is_some() {
            return;
        }
        let Some(s) = self.sessao_mut() else {
            return;
        };
        let Some((original, selecao)) = s.comecar_filtro() else {
            self.aviso = Some((
                "O filtro precisa de uma camada de pixels visível e com conteúdo — para a foto, dê duplo clique no Fundo"
                    .into(),
                true,
            ));
            cx.notify();
            return;
        };
        let l = self.filtro.lembrado;
        let (raio, quantidade, limiar) = match tipo {
            Tipo::Desfoque => (l.raio_do_desfoque, l.quantidade, l.limiar),
            Tipo::Nitidez => (l.raio_da_nitidez, l.quantidade, l.limiar),
        };
        let f = &self.filtro;
        f.raio
            .clone()
            .update(cx, |s, cx| s.set_value(posicao_do_raio(raio), window, cx));
        f.quantidade
            .clone()
            .update(cx, |s, cx| s.set_value(quantidade, window, cx));
        f.limiar
            .clone()
            .update(cx, |s, cx| s.set_value(limiar, window, cx));
        self.filtro.aberto = Some(Aberto {
            tipo,
            original,
            selecao,
        });
        self.recalcular_o_filtro(Duration::ZERO, cx);
        cx.notify();
    }

    /// O filtro com os valores dos controles de agora.
    pub fn filtro_dos_controles(&self, cx: &gpui_kit::App) -> Option<Filtro> {
        let aberto = self.filtro.aberto.as_ref()?;
        let raio = raio_da_posicao(self.filtro.raio.read(cx).value().start());
        Some(match aberto.tipo {
            Tipo::Desfoque => Filtro::DesfoqueGaussiano { raio },
            Tipo::Nitidez => Filtro::MascaraDeNitidez {
                quantidade: self.filtro.quantidade.read(cx).value().start() / 100.0,
                raio,
                limiar: self
                    .filtro
                    .limiar
                    .read(cx)
                    .value()
                    .start()
                    .round()
                    .clamp(0.0, 255.0) as u8,
            },
        })
    }

    /// O roteiro e os testes: um controle pelo valor ("raio" em px,
    /// "quantidade" em %, "limiar" em níveis), como se o slider andasse.
    pub fn definir_controle_do_filtro(
        &mut self,
        qual: &str,
        valor: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (estado, v) = match qual {
            "raio" => (self.filtro.raio.clone(), posicao_do_raio(valor)),
            "quantidade" => (self.filtro.quantidade.clone(), valor),
            "limiar" => (self.filtro.limiar.clone(), valor),
            _ => return,
        };
        estado.update(cx, |s, cx| s.set_value(v, window, cx));
        self.controle_do_filtro_mudou(cx);
    }

    fn controle_do_filtro_mudou(&mut self, cx: &mut Context<Self>) {
        if self.filtro.aberto.is_some() {
            self.recalcular_o_filtro(Duration::from_millis(80), cx);
            cx.notify();
        }
    }

    /// Pede a conta da prévia (ou volta a original, com "Visualizar"
    /// desligado).
    fn recalcular_o_filtro(&mut self, respiro: Duration, cx: &mut Context<Self>) {
        let Some(filtro) = self.filtro_dos_controles(cx) else {
            return;
        };
        let Some(aberto) = self.filtro.aberto.as_ref() else {
            return;
        };
        self.filtro.geracao += 1;
        let geracao = self.filtro.geracao;
        if !self.filtro.visualizar {
            let original = aberto.original.clone();
            self.filtro._conta = None;
            self.filtro.mostrada = 0;
            if let Some(s) = self.sessao_mut() {
                s.mostrar_filtro(original);
            }
            cx.notify();
            return;
        }
        let (original, selecao) = (aberto.original.clone(), aberto.selecao.clone());
        self.filtro._conta = Some(cx.spawn(async move |ed, cx| {
            if !respiro.is_zero() {
                cx.background_executor().timer(respiro).await;
            }
            let pixels = cx
                .background_executor()
                .spawn(async move { filtrada(&original, filtro, selecao.as_deref()) })
                .await;
            let _ = ed.update(cx, |ed, cx| {
                if ed.filtro.geracao != geracao || ed.filtro.aberto.is_none() {
                    return;
                }
                ed.filtro.mostrada = geracao;
                if let Some(s) = ed.sessao_mut() {
                    s.mostrar_filtro(pixels);
                }
                cx.notify();
            });
        }));
    }

    /// A prévia está em dia com os controles.
    pub fn previa_do_filtro_pronta(&self) -> bool {
        self.filtro.aberto.is_some()
            && self.filtro.visualizar
            && self.filtro.mostrada == self.filtro.geracao
    }

    pub fn alternar_visualizar_o_filtro(&mut self, cx: &mut Context<Self>) {
        self.filtro.visualizar = !self.filtro.visualizar;
        self.recalcular_o_filtro(Duration::ZERO, cx);
    }

    /// OK (Enter).
    pub fn confirmar_filtro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(filtro) = self.filtro_dos_controles(cx) else {
            return;
        };
        let Some(aberto) = self.filtro.aberto.take() else {
            return;
        };
        self.filtro._conta = None;
        // A prévia não está em dia (correndo, ou "Visualizar" desligado): a
        // conta termina aqui.
        if !(self.filtro.visualizar && self.filtro.mostrada == self.filtro.geracao) {
            let pixels = filtrada(&aberto.original, filtro, aberto.selecao.as_deref());
            if let Some(s) = self.sessao_mut() {
                s.mostrar_filtro(pixels);
            }
        }
        self.filtro.geracao += 1;
        let l = &mut self.filtro.lembrado;
        match filtro {
            Filtro::DesfoqueGaussiano { raio } => l.raio_do_desfoque = raio,
            Filtro::MascaraDeNitidez {
                quantidade,
                raio,
                limiar,
            } => {
                l.quantidade = quantidade * 100.0;
                l.raio_da_nitidez = raio;
                l.limiar = limiar as f32;
            }
        }
        self.na_sessao(cx, |s| {
            s.aplicar_filtro(filtro.nome());
        });
        window.focus(&self.foco, cx);
    }

    /// Cancelar (Esc).
    pub fn cancelar_o_filtro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.filtro.aberto.take().is_none() {
            return;
        }
        self.filtro._conta = None;
        self.filtro.geracao += 1;
        if let Some(s) = self.sessao_mut() {
            s.cancelar_filtro();
        }
        window.focus(&self.foco, cx);
        cx.notify();
    }

    /// A caixa do diálogo, no canto de cima à direita do palco.
    pub(super) fn caixa_do_filtro(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let aberto = self.filtro.aberto.as_ref()?;
        let c = aparencia::cores(cx);
        let tema = cx.theme().clone();
        let raio = raio_da_posicao(self.filtro.raio.read(cx).value().start());
        let quantidade = self.filtro.quantidade.read(cx).value().start();
        let limiar = self.filtro.limiar.read(cx).value().start();
        let linha = |id: &'static str,
                     rotulo: &'static str,
                     estado: &Entity<SliderState>,
                     texto: String| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_sm()
                        .child(rotulo)
                        .child(div().text_color(c.apagado).child(texto)),
                )
                .child(
                    div()
                        .h(px(20.))
                        .debug_selector(move || id.into())
                        .child(crate::estilo::slider(estado)),
                )
        };
        let calculando = self.filtro.visualizar && self.filtro.mostrada != self.filtro.geracao;
        let onde = point(
            self.palco.right() - px(LARGURA + 12.),
            self.palco.top() + px(12.),
        );
        let corpo = div()
            .id("editor-filtro")
            .debug_selector(|| "editor-filtro".into())
            .occlude()
            .w(px(LARGURA))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .bg(tema.popover)
            .text_color(tema.popover_foreground)
            .border_1()
            .border_color(c.borda)
            .rounded(px(6.))
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(aberto.tipo.titulo()),
                    )
                    .child(div().text_xs().text_color(c.apagado).child(if calculando {
                        "calculando…"
                    } else {
                        ""
                    })),
            )
            .when(aberto.tipo == Tipo::Nitidez, |d| {
                d.child(linha(
                    "editor-filtro-quantidade",
                    "Quantidade",
                    &self.filtro.quantidade,
                    format!("{}%", quantidade.round() as i32),
                ))
            })
            .child(linha(
                "editor-filtro-raio",
                "Raio",
                &self.filtro.raio,
                format!("{} px", numero(raio)),
            ))
            .when(aberto.tipo == Tipo::Nitidez, |d| {
                d.child(linha(
                    "editor-filtro-limiar",
                    "Limiar",
                    &self.filtro.limiar,
                    format!("{} níveis", limiar.round() as i32),
                ))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        Checkbox::new("editor-filtro-visualizar")
                            .small()
                            .label("Visualizar")
                            .checked(self.filtro.visualizar)
                            .on_click(cx.listener(|ed, _: &bool, _, cx| {
                                ed.alternar_visualizar_o_filtro(cx)
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                crate::estilo::botao_contorno("editor-filtro-cancelar", cx)
                                    .child("Cancelar")
                                    .on_click(cx.listener(|ed, _, window, cx| {
                                        ed.cancelar_o_filtro(window, cx)
                                    })),
                            )
                            .child(
                                crate::estilo::botao_primario("editor-filtro-ok", cx)
                                    .child("OK")
                                    .on_click(cx.listener(|ed, _, window, cx| {
                                        ed.confirmar_filtro(window, cx)
                                    })),
                            ),
                    ),
            );
        // 🔒 Modal, como lá: um véu transparente toma os cliques do resto da
        // janela (as teclas, o contexto `FiltroDoEditor` já barra).
        let veu = deferred(
            anchored().position(point(px(0.), px(0.))).child(
                div()
                    .id("editor-filtro-veu")
                    .occlude()
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height),
            ),
        )
        .with_priority(1);
        Some(
            div()
                .child(veu)
                .child(
                    deferred(
                        anchored()
                            .position(onde)
                            .snap_to_window_with_margin(px(8.))
                            .child(corpo),
                    )
                    .with_priority(2),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_raio_vai_e_volta_pela_posicao() {
        assert_eq!(raio_da_posicao(0.0), 0.1);
        assert_eq!(raio_da_posicao(100.0), 250.0);
        for r in [0.5f32, 1.0, 4.0, 12.5, 100.0] {
            let p = posicao_do_raio(r);
            assert!((raio_da_posicao(p) - r).abs() <= 0.1, "{r} → {p}");
        }
        assert_eq!(numero(4.0), "4");
        assert_eq!(numero(2.5), "2,5");
    }
}
