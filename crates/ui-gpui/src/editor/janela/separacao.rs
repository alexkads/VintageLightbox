//! 🧴 "Separação de frequências…" (Filtro › Tratamento de pele, e o painel
//! Tratamento de pele): o diálogo que prepara o conjunto baixa + alta.
//!
//! - **Origem**: a foto como aparece (o conjunto vai para o topo) ou a camada
//!   escolhida como aparece (o conjunto entra logo acima dela); "Regenerar"
//!   parte do conjunto já retocado e troca os pixels das mesmas camadas;
//! - **Raio** (σ, em pixels da foto — não depende do zoom): o tamanho da
//!   textura que fica na alta. Começa do sugerido para a resolução;
//! - **Prévia**: recomposta (a foto — tem de ser a mesma), só a baixa, só a
//!   alta, e o antes (sem o conjunto);
//! - a origem é composta **uma vez** em segundo plano; cada raio refaz só a
//!   separação (um desfoque), também fora da thread da tela, com um respiro
//!   de 80 ms no arrasto. A prévia é a conta inteira, na resolução da foto;
//! - OK grava **um** passo; Cancelar e Esc voltam o documento como era.
//!
//! Modal como os filtros: o contexto de teclas `FiltroDoEditor` (só Enter e
//! Esc) e um véu que toma os cliques do resto da janela.

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::button::ButtonGroup;
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, Context, Entity, Subscription,
    Task, Window,
};

use super::aparencia;
use super::EditorDeFoto;
use editor_core::frequencias::{self, RAIO_MAXIMO, RAIO_MINIMO};
use editor_core::tiles::CamadaDePixels;
use editor_core::{OrigemDaSeparacao, PedidoDeSeparacao, VistaDaSeparacao};

const LARGURA: f32 = 320.0;

/// De onde o diálogo parte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origem {
    Visivel,
    Camada,
    /// O conjunto já retocado (regenerar com outro raio).
    Regenerar,
}

pub struct Aberto {
    pub origem: Origem,
    pedido: Arc<PedidoDeSeparacao>,
    /// A origem composta (uma vez por origem).
    fonte: Option<Arc<CamadaDePixels>>,
}

pub struct Estado {
    pub aberto: Option<Aberto>,
    raio: Entity<SliderState>,
    pub vista: VistaDaSeparacao,
    /// O último raio confirmado.
    lembrado: Option<f32>,
    geracao: u64,
    mostrada: u64,
    _conta: Option<Task<()>>,
    _assinatura: Subscription,
}

/// Raio (px) ↔ posição do slider (0–100), exponencial: fino perto do
/// mínimo, e o fim ainda alcança manchas grandes.
pub fn raio_da_posicao(p: f32) -> f32 {
    let p = p.clamp(0.0, 100.0) / 100.0;
    let r = RAIO_MINIMO * (RAIO_MAXIMO / RAIO_MINIMO).powf(p);
    (r * 10.0).round() / 10.0
}

pub fn posicao_do_raio(r: f32) -> f32 {
    let r = r.clamp(RAIO_MINIMO, RAIO_MAXIMO);
    100.0 * (r / RAIO_MINIMO).ln() / (RAIO_MAXIMO / RAIO_MINIMO).ln()
}

impl Estado {
    pub fn novo(window: &mut Window, cx: &mut Context<EditorDeFoto>) -> Self {
        let raio = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(0.1)
                .default_value(posicao_do_raio(4.0))
        });
        let assinatura = cx.subscribe_in(
            &raio,
            window,
            |ed: &mut EditorDeFoto, _s, _e: &SliderEvent, _w, cx| {
                if ed.separacao.aberto.is_some() {
                    ed.recalcular_a_separacao(Duration::from_millis(80), cx);
                }
            },
        );
        Self {
            aberto: None,
            raio,
            vista: VistaDaSeparacao::Recomposta,
            lembrado: None,
            geracao: 0,
            mostrada: 0,
            _conta: None,
            _assinatura: assinatura,
        }
    }
}

impl EditorDeFoto {
    pub fn separacao_aberta(&self) -> Option<Origem> {
        self.separacao.aberto.as_ref().map(|a| a.origem)
    }

    /// O raio dos controles, em px.
    pub fn raio_da_separacao(&self, cx: &gpui_kit::App) -> f32 {
        raio_da_posicao(self.separacao.raio.read(cx).value().start())
    }

    /// Abre o diálogo. `regenerar`: parte do conjunto da camada escolhida.
    pub fn abrir_separacao(
        &mut self,
        regenerar: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.separacao.aberto.is_some()
            || self.filtro.aberto.is_some()
            || self.area_do_preenchimento.is_some()
        {
            return;
        }
        let origem = if regenerar {
            Origem::Regenerar
        } else {
            Origem::Visivel
        };
        let Some((pedido, raio)) = self.pedido_da_origem(origem) else {
            self.aviso = Some((
                "Regenerar precisa de uma separação de frequências no documento".into(),
                true,
            ));
            cx.notify();
            return;
        };
        let raio = self
            .separacao
            .lembrado
            .filter(|_| !regenerar)
            .unwrap_or(raio);
        self.separacao
            .raio
            .clone()
            .update(cx, |s, cx| s.set_value(posicao_do_raio(raio), window, cx));
        self.separacao.vista = VistaDaSeparacao::Recomposta;
        self.separacao.aberto = Some(Aberto {
            origem,
            pedido: Arc::new(pedido),
            fonte: None,
        });
        self.recalcular_a_separacao(Duration::ZERO, cx);
        cx.notify();
    }

    /// O pedido da origem e o raio com que ela começa (o do conjunto, ao
    /// regenerar; o sugerido para a resolução, senão).
    fn pedido_da_origem(&self, origem: Origem) -> Option<(PedidoDeSeparacao, f32)> {
        let s = self.sessao()?;
        let doc = s.documento();
        let sugerido = frequencias::raio_sugerido(doc.largura(), doc.altura());
        match origem {
            Origem::Visivel => Some((s.pedido_de_separacao(OrigemDaSeparacao::Visivel), sugerido)),
            Origem::Camada => Some((
                s.pedido_de_separacao(OrigemDaSeparacao::CamadaSelecionada),
                sugerido,
            )),
            Origem::Regenerar => {
                let (b, _) = s.conjunto_de_pele()?;
                let raio = doc.camadas[b]
                    .retoque
                    .and_then(|r| r.raio())
                    .unwrap_or(sugerido);
                Some((s.pedido_de_regeneracao()?, raio))
            }
        }
    }

    /// Troca a origem: a prévia sai e a conta recomeça da nova.
    pub fn trocar_a_origem_da_separacao(&mut self, origem: Origem, cx: &mut Context<Self>) {
        let Some(aberto) = self.separacao.aberto.as_ref() else {
            return;
        };
        if aberto.origem == origem || aberto.origem == Origem::Regenerar {
            return;
        }
        if let Some(s) = self.sessao_mut() {
            s.cancelar_separacao();
        }
        let Some((pedido, _)) = self.pedido_da_origem(origem) else {
            return;
        };
        self.separacao.aberto = Some(Aberto {
            origem,
            pedido: Arc::new(pedido),
            fonte: None,
        });
        self.recalcular_a_separacao(Duration::ZERO, cx);
        cx.notify();
    }

    /// Recalcula a prévia com o raio de agora.
    fn recalcular_a_separacao(&mut self, respiro: Duration, cx: &mut Context<Self>) {
        let raio = self.raio_da_separacao(cx);
        let Some(aberto) = self.separacao.aberto.as_ref() else {
            return;
        };
        self.separacao.geracao += 1;
        let geracao = self.separacao.geracao;
        let (pedido, fonte) = (aberto.pedido.clone(), aberto.fonte.clone());
        self.separacao._conta = Some(cx.spawn(async move |ed, cx| {
            if !respiro.is_zero() {
                cx.background_executor().timer(respiro).await;
            }
            let (fonte, sep) = cx
                .background_executor()
                .spawn(async move {
                    let fonte = fonte.unwrap_or_else(|| Arc::new(pedido.fonte()));
                    let sep = frequencias::separar(&fonte, raio);
                    (fonte, sep)
                })
                .await;
            let _ = ed.update(cx, |ed, cx| {
                if ed.separacao.geracao != geracao {
                    return;
                }
                let Some(aberto) = ed.separacao.aberto.as_mut() else {
                    return;
                };
                aberto.fonte = Some(fonte);
                let pedido = aberto.pedido.clone();
                let vista = ed.separacao.vista;
                let entrou = ed.sessao_mut().is_some_and(|s| {
                    let ok = s.mostrar_separacao(&pedido, raio, sep);
                    if ok {
                        s.exibir_separacao(vista);
                    }
                    ok
                });
                if entrou {
                    ed.separacao.mostrada = geracao;
                } else {
                    ed.separacao.aberto = None;
                    ed.aviso = Some((
                        "A foto mudou enquanto a separação era calculada — abra de novo".into(),
                        true,
                    ));
                }
                cx.notify();
            });
        }));
    }

    /// A prévia está em dia com os controles.
    pub fn previa_da_separacao_pronta(&self) -> bool {
        self.separacao.aberto.is_some() && self.separacao.mostrada == self.separacao.geracao
    }

    /// O que a prévia mostra.
    pub fn ver_na_separacao(&mut self, vista: VistaDaSeparacao, cx: &mut Context<Self>) {
        self.separacao.vista = vista;
        if let Some(s) = self.sessao_mut() {
            s.exibir_separacao(vista);
        }
        cx.notify();
    }

    /// O roteiro e os testes: o raio pelo valor, como se o slider andasse.
    pub fn definir_raio_da_separacao(
        &mut self,
        raio: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.separacao
            .raio
            .clone()
            .update(cx, |s, cx| s.set_value(posicao_do_raio(raio), window, cx));
        if self.separacao.aberto.is_some() {
            self.recalcular_a_separacao(Duration::from_millis(80), cx);
        }
    }

    /// OK (Enter): a conta termina aqui se a prévia não está em dia.
    pub fn confirmar_separacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raio = self.raio_da_separacao(cx);
        let Some(aberto) = self.separacao.aberto.take() else {
            return;
        };
        self.separacao._conta = None;
        let em_dia = self.separacao.mostrada == self.separacao.geracao;
        self.separacao.geracao += 1;
        if !em_dia {
            let fonte = aberto
                .fonte
                .clone()
                .unwrap_or_else(|| Arc::new(aberto.pedido.fonte()));
            let sep = frequencias::separar(&fonte, raio);
            let entrou = self
                .sessao_mut()
                .is_some_and(|s| s.mostrar_separacao(&aberto.pedido, raio, sep));
            if !entrou {
                self.aviso = Some(("A foto mudou — a separação não entrou".into(), true));
                cx.notify();
                return;
            }
        }
        if aberto.origem != Origem::Regenerar {
            self.separacao.lembrado = Some(raio);
        }
        self.na_sessao(cx, |s| {
            s.confirmar_separacao();
        });
        window.focus(&self.foco, cx);
    }

    /// Cancelar (Esc).
    pub fn cancelar_a_separacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.separacao.aberto.take().is_none() {
            return;
        }
        self.separacao._conta = None;
        self.separacao.geracao += 1;
        if let Some(s) = self.sessao_mut() {
            s.cancelar_separacao();
        }
        window.focus(&self.foco, cx);
        cx.notify();
    }

    /// A caixa do diálogo, no canto de cima à direita do palco.
    pub(super) fn caixa_da_separacao(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let aberto = self.separacao.aberto.as_ref()?;
        let c = aparencia::cores(cx);
        let tema = cx.theme().clone();
        let raio = self.raio_da_separacao(cx);
        let calculando = !self.previa_da_separacao_pronta();
        let rotulo = |t: &'static str| div().text_xs().text_color(c.apagado).child(t);
        let escolha = |id: &'static str, texto: &'static str, aceso: bool, cx: &Context<Self>| {
            if aceso {
                crate::estilo::botao_primario_pequeno(id, cx)
            } else {
                crate::estilo::botao_contorno_pequeno(id, cx)
            }
            .debug_selector(move || id.into())
            .label(texto)
            .selected(aceso)
        };
        let origem = aberto.origem;
        let origens = if origem == Origem::Regenerar {
            div()
                .text_sm()
                .child("O conjunto como está retocado — os retoques ficam na nova divisão.")
                .into_any_element()
        } else {
            ButtonGroup::new("editor-separacao-origem")
                .xsmall()
                .child(escolha(
                    "editor-separacao-visivel",
                    "Composição visível",
                    origem == Origem::Visivel,
                    cx,
                ))
                .child(escolha(
                    "editor-separacao-camada",
                    "Camada selecionada",
                    origem == Origem::Camada,
                    cx,
                ))
                .on_click(cx.listener(|ed, cliques: &Vec<usize>, _, cx| {
                    let o = match cliques.first() {
                        Some(0) => Origem::Visivel,
                        Some(1) => Origem::Camada,
                        _ => return,
                    };
                    ed.trocar_a_origem_da_separacao(o, cx);
                }))
                .into_any_element()
        };
        let vista = self.separacao.vista;
        const VISTAS: [(VistaDaSeparacao, &str, &str); 4] = [
            (
                VistaDaSeparacao::Recomposta,
                "editor-separacao-ver-recomposta",
                "Recomposta",
            ),
            (
                VistaDaSeparacao::Baixa,
                "editor-separacao-ver-baixa",
                "Baixa",
            ),
            (VistaDaSeparacao::Alta, "editor-separacao-ver-alta", "Alta"),
            (
                VistaDaSeparacao::Original,
                "editor-separacao-ver-antes",
                "Antes",
            ),
        ];
        let vistas = ButtonGroup::new("editor-separacao-vista")
            .xsmall()
            .children(
                VISTAS
                    .iter()
                    .map(|(v, id, texto)| escolha(id, texto, vista == *v, cx)),
            )
            .on_click(cx.listener(|ed, cliques: &Vec<usize>, _, cx| {
                if let Some((v, _, _)) = cliques.first().and_then(|i| VISTAS.get(*i)) {
                    ed.ver_na_separacao(*v, cx);
                }
            }));
        let titulo = if origem == Origem::Regenerar {
            "Regenerar separação"
        } else {
            "Separação de frequências"
        };
        let corpo = div()
            .id("editor-separacao")
            .debug_selector(|| "editor-separacao".into())
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
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(titulo),
                    )
                    .child(div().text_xs().text_color(c.apagado).child(if calculando {
                        "calculando…"
                    } else {
                        ""
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(rotulo("Origem"))
                    .child(origens),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_sm()
                            .child("Raio")
                            .child(div().text_color(c.apagado).child(format!(
                                "{} px",
                                format!("{raio:.1}").replace('.', ",")
                            ))),
                    )
                    .child(
                        div()
                            .h(px(20.))
                            .debug_selector(|| "editor-separacao-raio".into())
                            .child(crate::estilo::slider(&self.separacao.raio)),
                    )
                    .child(rotulo(
                        "Em pixels da foto: o maior detalhe que fica na alta (poros, pelos). Manchas e transições de cor, maiores que ele, ficam na baixa.",
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(rotulo("Prévia"))
                    .child(vistas),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        crate::estilo::botao_contorno("editor-separacao-cancelar", cx)
                            .child("Cancelar")
                            .on_click(cx.listener(|ed, _, window, cx| {
                                ed.cancelar_a_separacao(window, cx)
                            })),
                    )
                    .child(
                        crate::estilo::botao_primario("editor-separacao-ok", cx)
                            .child("OK")
                            .on_click(
                                cx.listener(|ed, _, window, cx| ed.confirmar_separacao(window, cx)),
                            ),
                    ),
            );
        let onde = point(
            self.palco.right() - px(LARGURA + 12.),
            self.palco.top() + px(12.),
        );
        // 🔒 Modal: o véu transparente toma os cliques do resto da janela.
        let veu = deferred(
            anchored().position(point(px(0.), px(0.))).child(
                div()
                    .id("editor-separacao-veu")
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
        assert_eq!(raio_da_posicao(0.0), RAIO_MINIMO);
        assert_eq!(raio_da_posicao(100.0), RAIO_MAXIMO);
        for r in [0.5f32, 2.0, 5.0, 12.5, 40.0] {
            assert!(
                (raio_da_posicao(posicao_do_raio(r)) - r).abs() <= 0.1,
                "{r}"
            );
        }
    }
}
