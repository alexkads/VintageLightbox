//! O painel **Correção de cores** — o "Color Grading" do Lightroom, com rodas.
//!
//! 🎡 *"Eu quero o nosso sistema de tonalização exatamente assim! Muito
//! parecido com o Lightroom"* (dono, 2026-09-30, com o print do painel). De cima
//! para baixo, como lá:
//!
//! - **Ajustar**: cinco botões — 3 rodas, Sombras, Tons médios, Realces e
//!   Global —, cada um com o ponto que diz se a faixa tem ajuste;
//! - **as rodas**: o puck move matiz e saturação, a alça da borda gira só o
//!   matiz; Shift trava o matiz, Cmd/Ctrl dá ajuste fino, duplo clique zera;
//! - embaixo de cada roda, **o olho** (segurar para ver a foto sem aquela
//!   faixa) e **a luminância**;
//! - no pé, **Mesclagem** e **Equilíbrio**.
//!
//! A conta de tudo isso é de [`rodas`]; aqui só se desenha e se encaminha o
//! ponteiro.

use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme, Disableable, Sizable};
use gpui_kit::{
    canvas, div, prelude::*, px, AnyElement, Bounds, Context, DragMoveEvent, Hsla, MouseButton,
    MouseDownEvent, MouseUpEvent, Pixels, SharedString, Window,
};

use super::painel::{ponto, SemFantasma};
use super::Revelacao;
use crate::recursos::Icone;
use crate::revelacao::controles;
use crate::revelacao::processador::Ajustes;
use crate::revelacao::rodas::{self, Faixa, Pegada, Vista};

/// O lado da roda dos tons médios na vista de três, e das duas de baixo.
const LADO_DA_RODA_DO_MEIO: f32 = 132.0;
const LADO_DA_RODA_DE_BAIXO: f32 = 108.0;
/// A roda da vista de uma faixa só.
const LADO_DA_RODA_GRANDE: f32 = 200.0;

/// O que o olho apertado esconde da prévia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VerSem {
    Faixa(Faixa),
    /// O painel inteiro.
    Tudo,
}

impl VerSem {
    pub(crate) fn aplicar(&self, ajustes: &Ajustes) -> Ajustes {
        match self {
            VerSem::Faixa(faixa) => {
                let mut sem = *ajustes;
                faixa.neutralizar(&mut sem);
                sem
            }
            VerSem::Tudo => rodas::sem_correcao_de_cores(ajustes),
        }
    }

    fn chave(&self) -> &'static str {
        match self {
            VerSem::Faixa(f) => f.chave(),
            VerSem::Tudo => "tudo",
        }
    }
}

/// O tipo do arrasto de uma roda.
#[derive(Clone, Copy)]
struct ArrastoDaRoda;

impl Revelacao {
    /// O índice de um controle da Correção de cores pelo rótulo.
    ///
    /// ⚠️ **Só na seção dela**: "Sombras — matiz" também é a tinta das sombras
    /// da Calibração, e "Realces — matiz" era do Color balance do darktable (saiu em 2/out/2026).
    fn indice_do_controle(&self, rotulo: &str) -> Option<usize> {
        self.controles.iter().position(|c| {
            c.definicao.secao == controles::Secao::Tonalizacao && c.definicao.rotulo == rotulo
        })
    }

    /// Os ajustes como a prévia os quer, com o olho apertado ou não.
    pub(super) fn sem_o_que_o_olho_esconde(&self, ajustes: Ajustes) -> Ajustes {
        match self.estado_do_painel.ver_sem {
            Some(ver_sem) => ver_sem.aplicar(&ajustes),
            None => ajustes,
        }
    }

    /// O ponto de uma faixa, pela mesma regra dos painéis: âmbar se mudou
    /// desde a abertura, cinza se a salva tem ajuste.
    fn marca_das_faixas(&self, faixas: &[Faixa]) -> Option<controles::Marca> {
        let salvo = self.salvo();
        if faixas.iter().any(|f| f.ler(&self.ajustes) != f.ler(&salvo)) {
            Some(controles::Marca::NaoSalvo)
        } else if faixas.iter().any(|f| f.alterada(&self.ajustes)) {
            Some(controles::Marca::Ajustado)
        } else {
            None
        }
    }

    /// O conteúdo do painel.
    pub(super) fn correcao_de_cores(&self, cx: &mut Context<Self>) -> AnyElement {
        let vista = self.estado_do_painel.vista_das_rodas;
        let corpo: AnyElement = match vista {
            Vista::TresRodas => div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(10.))
                .child(self.bloco_da_roda(Faixa::TonsMedios, LADO_DA_RODA_DO_MEIO, cx))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .justify_between()
                        .gap(px(8.))
                        .child(self.bloco_da_roda(Faixa::Sombras, LADO_DA_RODA_DE_BAIXO, cx))
                        .child(self.bloco_da_roda(Faixa::Realces, LADO_DA_RODA_DE_BAIXO, cx)),
                )
                .into_any_element(),
            Vista::Uma(faixa) => {
                let mut coluna = div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().flex().justify_center().child(self.roda(
                        faixa,
                        LADO_DA_RODA_GRANDE,
                        cx,
                    )))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .child(self.olho(VerSem::Faixa(faixa), cx)),
                    );
                for (rotulo_na_tabela, rotulo) in [
                    (faixa.rotulo_do_matiz(), "Matiz"),
                    (faixa.rotulo_da_saturacao(), "Saturação"),
                    (faixa.rotulo_da_luminancia(), "Luminância"),
                ] {
                    if let Some(i) = self.indice_do_controle(rotulo_na_tabela) {
                        coluna = coluna.child(self.linha_do_controle_rotulada(
                            i,
                            &self.controles[i],
                            rotulo,
                            cx,
                        ));
                    }
                }
                coluna.into_any_element()
            }
        };

        // O pé: Mesclagem e Equilíbrio, separados das rodas por um fio.
        let pe = ["Mesclagem", "Equilíbrio"]
            .into_iter()
            .filter_map(|rotulo| self.indice_do_controle(rotulo))
            .map(|i| {
                self.linha_do_controle_rotulada(
                    i,
                    &self.controles[i],
                    self.controles[i].definicao.rotulo,
                    cx,
                )
            })
            .collect::<Vec<_>>();

        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(self.seletor_ajustar(vista, cx))
            .child(corpo)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .pt(px(10.))
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .children(pe),
            )
            .into_any_element()
    }

    /// "Ajustar:" e os cinco botões, com o olho do painel inteiro na ponta.
    fn seletor_ajustar(&self, vista: Vista, cx: &mut Context<Self>) -> AnyElement {
        let tela = cx.entity().downgrade();
        let escolhida = Vista::TODAS.iter().position(|v| *v == vista).unwrap_or(0);
        let abas = TabBar::new("ajustar-correcao-de-cores")
            .segmented()
            .xsmall()
            .selected_index(escolhida)
            .children(Vista::TODAS.into_iter().map(|v| {
                let marca = self.marca_das_faixas(v.faixas());
                let chave = v.chave();
                Tab::new()
                    .aria_label(v.dica())
                    .debug_selector(move || format!("ajustar-{chave}"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(2.))
                            .py(px(1.))
                            .child(icone_da_vista(v, cx))
                            .child(match marca {
                                Some(m) => ponto(m, cx).size(px(4.)).into_any_element(),
                                None => div().size(px(4.)).into_any_element(),
                            }),
                    )
            }))
            .on_click(move |i, _window, cx| {
                let Some(v) = Vista::TODAS.get(*i).copied() else {
                    return;
                };
                let _ = tela.update(cx, |tela, cx| {
                    tela.estado_do_painel.vista_das_rodas = v;
                    cx.notify();
                });
            });

        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(6.))
            .text_xs()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("Ajustar:"),
                    )
                    .child(abas),
            )
            .child(self.olho(VerSem::Tudo, cx))
            .into_any_element()
    }

    /// Uma roda da vista de três: o nome em cima, a roda, o olho e a
    /// luminância embaixo.
    fn bloco_da_roda(&self, faixa: Faixa, lado: f32, cx: &mut Context<Self>) -> AnyElement {
        let luminancia = self
            .indice_do_controle(faixa.rotulo_da_luminancia())
            .map(|i| self.barra_do_controle(i, &self.controles[i], cx));
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .w(px(lado))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(faixa.rotulo()),
            )
            .child(self.roda(faixa, lado, cx))
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_end()
                    .child(self.olho(VerSem::Faixa(faixa), cx)),
            )
            .child(div().w_full().children(luminancia))
            .into_any_element()
    }

    /// O olho: segurar para ver a foto sem aquela parte.
    ///
    /// 🔑 **Segurar, e não ligar.** O que ele mostra nunca é a revelação da
    /// foto — é uma comparação. Um olho que ficasse ligado deixaria o operador
    /// salvar uma foto achando que ela está sem a cor das sombras, e o que vai
    /// para a galeria é com. Soltar (mesmo fora do botão) devolve a prévia.
    fn olho(&self, ver_sem: VerSem, cx: &mut Context<Self>) -> AnyElement {
        let ligado = self.controles_ligados();
        let apertado = self.estado_do_painel.ver_sem == Some(ver_sem);
        let chave = ver_sem.chave();
        let dica = match ver_sem {
            VerSem::Faixa(f) => format!("Segure para ver sem {}", f.rotulo().to_lowercase()),
            VerSem::Tudo => "Segure para ver sem a correção de cores".to_string(),
        };
        div()
            .id(SharedString::from(format!("olho-{chave}")))
            .debug_selector(move || format!("olho-{chave}"))
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(dica.clone()).build(window, cx)
            })
            .when(ligado, |d| {
                d.capture_any_mouse_down(cx.listener(
                    move |tela, evento: &MouseDownEvent, _window, cx| {
                        if evento.button == MouseButton::Left {
                            tela.segurar_o_olho(Some(ver_sem), cx);
                        }
                    },
                ))
                .capture_any_mouse_up(cx.listener(move |tela, _: &MouseUpEvent, _window, cx| {
                    tela.segurar_o_olho(None, cx);
                }))
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(move |tela, _: &MouseUpEvent, _window, cx| {
                        tela.segurar_o_olho(None, cx);
                    }),
                )
            })
            .child(
                crate::estilo::botao_icone(
                    SharedString::from(format!("botao-olho-{chave}")),
                    if apertado { Icone::EyeOff } else { Icone::Eye },
                    20.,
                    13.,
                )
                .xsmall()
                .text_color(if apertado {
                    cx.theme().foreground
                } else {
                    cx.theme().muted_foreground
                })
                .disabled(!ligado),
            )
            .into_any_element()
    }

    /// Aperta (ou solta) o olho e pede a prévia de novo.
    pub(crate) fn segurar_o_olho(&mut self, ver_sem: Option<VerSem>, cx: &mut Context<Self>) {
        if self.estado_do_painel.ver_sem == ver_sem {
            return;
        }
        self.estado_do_painel.ver_sem = ver_sem;
        self.pedir_revelacao(cx);
        cx.notify();
    }

    /// Uma roda: o disco, o centro, o puck e a alça.
    fn roda(&self, faixa: Faixa, lado: f32, cx: &mut Context<Self>) -> AnyElement {
        let ligado = self.controles_ligados();
        let (matiz, saturacao, _) = faixa.ler(&self.ajustes);
        let areas = self.estado_do_painel.areas_das_rodas.clone();
        let contorno = cx.theme().border;
        let desenho = canvas(
            move |bounds, _window, _cx| {
                areas.borrow_mut().insert(faixa, bounds);
            },
            move |bounds, _prepaint, window, _cx| {
                let lado = f32::from(bounds.size.width);
                let raio = rodas::raio_no_quadro(lado);
                let centro = bounds.center();
                let (cx_, cy_) = (f32::from(centro.x), f32::from(centro.y));
                let circulo = |x: f32, y: f32, r: f32| Bounds {
                    origin: gpui_kit::point(px(x - r), px(y - r)),
                    size: gpui_kit::size(px(r * 2.0), px(r * 2.0)),
                };

                // O disco, feito uma vez por tamanho em pixels físicos.
                let fisico = (raio * 2.0 * window.scale_factor()).round() as u32;
                if let Some(disco) = rodas::imagem_do_disco(fisico) {
                    let onde = circulo(cx_, cy_, raio);
                    let _ = window.paint_image(onde, onde, px(raio).into(), disco, 0, false);
                }
                window.paint_quad(
                    gpui_kit::fill(circulo(cx_, cy_, raio), gpui_kit::transparent_black())
                        .corner_radii(px(raio))
                        .border_widths(px(1.))
                        .border_color(contorno),
                );

                // O centro.
                window.paint_quad(
                    gpui_kit::fill(circulo(cx_, cy_, 1.5), Hsla::black().opacity(0.6))
                        .corner_radii(px(1.5)),
                );

                // A alça na borda, na cor plena do matiz.
                let (ax, ay) = rodas::ponto_da_roda(matiz, 100.0);
                let (ax, ay) = (
                    cx_ + ax * raio * rodas::RAIO_DA_ALCA,
                    cy_ + ay * raio * rodas::RAIO_DA_ALCA,
                );
                let [r, g, b] = rodas::cor_do_disco(matiz, 1.0);
                let cor_plena = Hsla::from(gpui_kit::Rgba { r, g, b, a: 1.0 });
                window.paint_quad(
                    gpui_kit::fill(circulo(ax, ay, 4.5), cor_plena)
                        .corner_radii(px(4.5))
                        .border_widths(px(1.))
                        .border_color(Hsla::black().opacity(0.5)),
                );

                // O puck: anel claro sobre anel escuro, para aparecer em
                // qualquer cor.
                let (px_, py_) = rodas::ponto_da_roda(matiz, saturacao);
                let (px_, py_) = (cx_ + px_ * raio, cy_ + py_ * raio);
                window.paint_quad(
                    gpui_kit::fill(circulo(px_, py_, 6.5), gpui_kit::transparent_black())
                        .corner_radii(px(6.5))
                        .border_widths(px(1.))
                        .border_color(Hsla::black().opacity(0.7)),
                );
                window.paint_quad(
                    gpui_kit::fill(circulo(px_, py_, 5.5), gpui_kit::transparent_black())
                        .corner_radii(px(5.5))
                        .border_widths(px(1.5))
                        .border_color(Hsla::white().opacity(0.9)),
                );
            },
        )
        .size_full();

        let chave = faixa.chave();
        div()
            .id(SharedString::from(format!("roda-{chave}")))
            .debug_selector(move || format!("roda-{chave}"))
            .size(px(lado))
            .flex_none()
            .when(!ligado, |q| q.opacity(0.4))
            .child(desenho)
            .when(ligado, |q| {
                q.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |tela, evento: &MouseDownEvent, window, cx| {
                        tela.apertar_a_roda(faixa, evento, window, cx);
                    }),
                )
                .on_drag(ArrastoDaRoda, |_, _, _, cx| cx.new(|_| SemFantasma))
                .on_drag_move(cx.listener(
                    move |tela, evento: &DragMoveEvent<ArrastoDaRoda>, window, cx| {
                        // 🚨 **Toda roda escuta o arrasto de toda roda**: o
                        // GPUI entrega o `DragMoveEvent` a cada elemento que
                        // se inscreveu no tipo. Só a que foi apertada responde.
                        let Some(arrasto) = tela.estado_do_painel.arrasto_da_roda else {
                            return;
                        };
                        if arrasto.faixa != faixa || !tela.controles_ligados() {
                            return;
                        }
                        let (dx, dy, raio) =
                            relativo_ao_centro(evento.bounds, evento.event.position);
                        let mods = evento.event.modifiers;
                        let (matiz, sat) = arrasto.cor(dx, dy, raio, mods.secondary(), mods.shift);
                        tela.mover_a_roda(faixa, matiz, sat, window, cx);
                    },
                ))
            })
            .into_any_element()
    }

    /// O clique numa roda: pega o puck ou a alça; o duplo clique zera a faixa.
    pub(super) fn apertar_a_roda(
        &mut self,
        faixa: Faixa,
        evento: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(limites) = self
            .estado_do_painel
            .areas_das_rodas
            .borrow()
            .get(&faixa)
            .copied()
        else {
            return;
        };
        let (dx, dy, raio) = relativo_ao_centro(limites, evento.position);
        let (matiz, sat, _) = faixa.ler(&self.ajustes);
        let Some(pegada) = rodas::pegar(dx, dy, raio, matiz) else {
            self.estado_do_painel.arrasto_da_roda = None;
            return;
        };
        if evento.click_count >= 2 {
            self.estado_do_painel.arrasto_da_roda = None;
            self.devolver_a_roda_ao_neutro(faixa, window, cx);
            return;
        }
        let arrasto = rodas::Arrasto {
            faixa,
            pegada,
            ponteiro_inicial: (dx, dy),
            cor_inicial: (matiz, sat),
        };
        self.estado_do_painel.arrasto_da_roda = Some(arrasto);
        // 🔑 **O clique no disco já leva o puck até ali**, como no Lightroom —
        // menos no modo fino, que existe justamente para não pular.
        let fino = evento.modifiers.secondary();
        if pegada == Pegada::Puck && !fino {
            let (m, s) = arrasto.cor(dx, dy, raio, false, evento.modifiers.shift);
            self.mover_a_roda(faixa, m, s, window, cx);
        }
    }

    /// Matiz e saturação novos para uma faixa — como um slider arrastado:
    /// pede a GPU e deixa a espera fechar o gesto.
    pub(crate) fn mover_a_roda(
        &mut self,
        faixa: Faixa,
        matiz: f32,
        saturacao: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (m0, s0, _) = faixa.ler(&self.ajustes);
        if (m0, s0) == (matiz, saturacao) {
            return;
        }
        faixa.definir_cor(&mut self.ajustes, matiz, saturacao);
        // As duas barras da vista de uma roda acompanham o puck.
        for (rotulo, valor) in [
            (faixa.rotulo_do_matiz(), matiz),
            (faixa.rotulo_da_saturacao(), saturacao),
        ] {
            if let Some(i) = self.indice_do_controle(rotulo) {
                self.controles[i]
                    .estado
                    .update(cx, |estado, cx| estado.set_value(valor, window, cx));
            }
        }
        self.pedir_revelacao(cx);
        self.adiar_gravacao(cx);
        cx.notify();
    }

    /// O duplo clique na roda: matiz e saturação ao neutro, num passo de
    /// histórico. A luminância tem a barra dela, e o duplo clique dela.
    pub(crate) fn devolver_a_roda_ao_neutro(
        &mut self,
        faixa: Faixa,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (h, s, _) = faixa.ler(&Ajustes::default());
        self.gesto_discreto(|a| faixa.definir_cor(a, h, s), window, cx);
    }
}

/// O ponteiro relativo ao centro do quadro da roda, e o raio do disco nele.
fn relativo_ao_centro(
    limites: Bounds<Pixels>,
    ponteiro: gpui_kit::Point<Pixels>,
) -> (f32, f32, f32) {
    let centro = limites.center();
    (
        f32::from(ponteiro.x - centro.x),
        f32::from(ponteiro.y - centro.y),
        rodas::raio_no_quadro(f32::from(limites.size.width)),
    )
}

/// O desenho de cada botão do "Ajustar", feito de círculos como o do
/// Lightroom: três rodinhas, o escuro, o meio-tom, o claro e o meio a meio.
fn icone_da_vista(vista: Vista, cx: &gpui_kit::App) -> AnyElement {
    let borda = cx.theme().muted_foreground;
    let circulo = |lado: f32, fundo: Hsla| {
        div()
            .size(px(lado))
            .flex_none()
            .rounded_full()
            .border_1()
            .border_color(borda)
            .bg(fundo)
    };
    let cinza = |l: f32| gpui_kit::hsla(0., 0., l, 1.);
    match vista {
        Vista::TresRodas => div()
            .flex()
            .flex_col()
            .items_center()
            .child(circulo(5., cinza(0.5)))
            .child(
                div()
                    .flex()
                    .gap(px(1.))
                    .child(circulo(5., cinza(0.2)))
                    .child(circulo(5., cinza(0.85))),
            )
            .into_any_element(),
        Vista::Uma(Faixa::Sombras) => circulo(11., cinza(0.15)).into_any_element(),
        Vista::Uma(Faixa::TonsMedios) => circulo(11., cinza(0.5)).into_any_element(),
        Vista::Uma(Faixa::Realces) => circulo(11., cinza(0.9)).into_any_element(),
        Vista::Uma(Faixa::Global) => circulo(11., cinza(0.15))
            .overflow_hidden()
            .flex()
            .justify_end()
            .child(div().w(px(5.)).h_full().bg(cinza(0.9)))
            .into_any_element(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_olho_esconde_so_o_que_diz() {
        let a = Ajustes {
            split_shadow_sat: 40.0,
            split_highlight_sat: 30.0,
            split_balance: 20.0,
            ..Default::default()
        };
        let sem_sombras = VerSem::Faixa(Faixa::Sombras).aplicar(&a);
        assert_eq!(sem_sombras.split_shadow_sat, 0.0);
        assert_eq!(sem_sombras.split_highlight_sat, 30.0);
        assert_eq!(sem_sombras.split_balance, 20.0);
        let sem_nada = VerSem::Tudo.aplicar(&a);
        assert_eq!(sem_nada.split_highlight_sat, 0.0);
        assert_eq!(sem_nada.split_balance, 0.0);
    }

    /// A conta de pixel → centro que o clique e o arrasto usam.
    #[test]
    fn o_ponteiro_e_medido_a_partir_do_centro() {
        let limites = Bounds {
            origin: gpui_kit::point(px(100.), px(50.)),
            size: gpui_kit::size(px(120.), px(120.)),
        };
        let (dx, dy, raio) = relativo_ao_centro(limites, gpui_kit::point(px(160.), px(60.)));
        assert_eq!((dx, dy), (0.0, -50.0));
        assert_eq!(raio, 60.0 - rodas::FOLGA_DA_ALCA);
    }
}
