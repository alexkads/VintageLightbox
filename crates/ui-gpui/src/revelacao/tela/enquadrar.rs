//! O Enquadrar — `painel-corte.tsx`, `overlay-corte.tsx` e `transferidor.tsx`
//! do site, sobre a conta de [`crate::revelacao::corte`].
//!
//! 🔑 **Não há "Aplicar" nem "Cancelar"**, como no site: cada gesto já é o
//! enquadramento da foto — vira um passo no histórico ao terminar e vai para o
//! banco. `Esc` e `R` só saem da ferramenta; `Enter` confirma e sai. Quem se
//! arrepende usa `⌘Z`, que é o mesmo caminho de qualquer outro ajuste.
//!
//! | Gesto | Faz |
//! |---|---|
//! | arrastar o meio | move o retângulo, sem mudar o tamanho |
//! | arrastar uma alça | redimensiona; com proporção, a borda oposta fica parada |
//! | girar ↺ ↻ | um quarto de volta, levando o retângulo junto |
//! | espelhar ⇆ ⇅ | liga e desliga |
//! | slider ou transferidor | endireita, e o retângulo encolhe para caber |
//! | duplo clique no rótulo ou no transferidor | endireitar volta a zero |
//! | proporção | remodela o retângulo na hora, mantendo a área |

use domain::value_objects::CropSettings;
use gpui::{
    canvas, div, point, prelude::*, px, AnyElement, Bounds, Context, CursorStyle, MouseButton,
    MouseDownEvent, PathBuilder, Pixels, Point, SharedString, Window,
};
use gpui_component::{h_flex, slider::Slider, v_flex, ActiveTheme, Icon};

use super::Revelacao;
use crate::estilo;
use crate::recursos::Icone;
use crate::revelacao::corte::{self, Alca, Retangulo, ANGULO_MAXIMO};
use crate::revelacao::persistencia::Corte;

/// Quantos graus cada pixel de arrasto do transferidor vale. 0,2 põe a faixa
/// inteira (±45°) em 450 px — endireitar é ajuste fino.
const GRAUS_POR_PIXEL: f32 = 0.2;
/// O passo do arrasto, igual ao do slider do painel.
pub(super) const PASSO_DO_ANGULO: f32 = 0.1;

const LARGURA_DO_TRANSFERIDOR: f32 = 280.;
const ALTURA_DO_TRANSFERIDOR: f32 = 62.;
/// O raio do arco. Grande = arco raso, como no Lightroom.
const RAIO: f32 = 190.;
/// Onde o arco toca o topo, abaixo do rótulo.
const TOPO: f32 = 20.;
/// Quantos graus de arco os 45° ocupam — 34 desenham a curva rasa que cabe em
/// 62 px.
const ARCO: f32 = 34.;
/// O lado das alças: o `size-4` do site.
const LADO_DA_ALCA: f32 = 16.;

/// O Enquadrar, enquanto está aberto.
#[derive(Debug, Default)]
pub(super) struct Edicao {
    /// O arrasto do retângulo em curso.
    pub arrasto: Option<ArrastoDoCorte>,
    /// A proporção travada (`largura ÷ altura`); `None` é livre.
    pub proporcao: Option<f32>,
    /// O retângulo que o **operador** pediu, antes de qualquer encolhimento do
    /// endireitar. É o que faz o controle crescer de volta ao voltar a zero.
    pub desejado: Option<Retangulo>,
    /// O arrasto do transferidor: `(x inicial, ângulo inicial)`.
    pub transferidor: Option<(f32, f32)>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ArrastoDoCorte {
    /// `None` é o meio do retângulo: só move.
    alca: Option<Alca>,
    inicio: Point<Pixels>,
    inicial: Retangulo,
}

/// O enquadramento na forma em que a foto o guarda.
fn corte_de(c: &CropSettings) -> Corte {
    Corte {
        x: Some(c.crop_x()),
        y: Some(c.crop_y()),
        largura: Some(c.crop_width()),
        altura: Some(c.crop_height()),
        rotacao: Some(c.rotation_90()),
        angulo: Some(c.angle()),
        espelho_h: Some(c.flip_horizontal()),
        espelho_v: Some(c.flip_vertical()),
    }
}

/// Onde o traço daquele ângulo cai no transferidor.
fn ponto_do_arco(angulo: f32, raio: f32) -> (f32, f32) {
    let v = (angulo / ANGULO_MAXIMO * ARCO).to_radians();
    (
        LARGURA_DO_TRANSFERIDOR / 2. + raio * v.sin(),
        TOPO + RAIO - raio * v.cos(),
    )
}

/// O ângulo como o site escreve: `+3.5°`, `-0.1°`, `0.0°`.
pub(super) fn rotulo_do_angulo(angulo: f32) -> String {
    if angulo > 0. {
        format!("+{angulo:.1}°")
    } else {
        format!("{:.1}°", if angulo == 0. { 0. } else { angulo })
    }
}

impl Revelacao {
    /// Entra ou sai do Enquadrar. É o `R` do site.
    pub fn alternar_corte(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edicao.is_some() {
            self.sair_do_corte(cx);
            return;
        }
        // Sem pixels não há onde pôr o retângulo.
        if self.tamanho_da_foto().is_none() {
            return;
        }
        // O slider nasce no ângulo da foto: numa foto já endireitada, uma barra
        // no meio diria que ela está reta.
        let graus = self.corte_atual().angle();
        self.angulo
            .update(cx, |estado, cx| estado.set_value(graus, window, cx));
        self.edicao = Some(Edicao::default());
        self.atualizar_exibicao();
        self.revelar_de_novo_se_a_vinheta_segue_o_corte(cx);
        cx.notify();
    }

    /// Sai da ferramenta. O que foi feito nela já é da foto.
    pub fn sair_do_corte(&mut self, cx: &mut Context<Self>) {
        if self.edicao.take().is_none() {
            return;
        }
        self.gravar_o_que_estiver_pendente();
        self.atualizar_exibicao();
        self.revelar_de_novo_se_a_vinheta_segue_o_corte(cx);
        cx.notify();
    }

    /// `Enter`: confirma e sai.
    pub fn aplicar_corte(&mut self, cx: &mut Context<Self>) {
        self.sair_do_corte(cx);
    }

    /// `Esc`: sai da ferramenta — como no site, **sem desfazer** nada.
    pub fn cancelar_corte(&mut self, cx: &mut Context<Self>) {
        self.sair_do_corte(cx);
    }

    pub fn cortando(&self) -> bool {
        self.edicao.is_some()
    }

    /// O espaço girado da foto aberta, em pixels da cópia de trabalho.
    fn espaco(&self) -> Option<(f32, f32)> {
        self.tamanho_da_foto()
            .map(|foto| corte::espaco_de(&self.corte_atual(), foto))
    }

    /// Onde o espaço girado está desenhado dentro do palco, em pixels.
    ///
    /// 🚨 **O espaço, e não a foto de origem**: com giro ímpar os lados trocam,
    /// e medir pela origem punha o retângulo fora da foto.
    pub(super) fn area_da_foto(&self) -> Option<(f32, f32, f32, f32)> {
        let espaco = self.espaco()?;
        let palco = (
            f32::from(self.palco.size.width),
            f32::from(self.palco.size.height),
        );
        let area = corte::area_da_foto(palco, espaco);
        (area.2 > 0.0 && area.3 > 0.0).then_some(area)
    }

    /// Troca o enquadramento da foto, sem fechar o gesto.
    ///
    /// Toda mudança que não é o ângulo redefine o retângulo pedido — é o
    /// `mudarCorte` do site.
    fn trocar_corte(&mut self, novo: CropSettings, refazer: bool, cx: &mut Context<Self>) {
        let antes = self.corte_atual();
        if let (Some(foto), Some(edicao)) = (self.tamanho_da_foto(), self.edicao.as_mut()) {
            if novo.angle() == antes.angle() {
                edicao.desejado = Some(corte::retangulo_de(&novo, corte::espaco_de(&novo, foto)));
            }
        }
        self.corte = corte_de(&novo);
        self.pendente = true;
        if refazer {
            self.atualizar_exibicao();
        }
        self.revelar_de_novo_se_a_vinheta_segue_o_corte(cx);
        cx.notify();
    }

    /// Um clique é um gesto inteiro: fecha o anterior, muda e vira passo.
    pub(super) fn gesto_do_corte(&mut self, novo: CropSettings, cx: &mut Context<Self>) {
        if self.tamanho_da_foto().is_none() {
            return;
        }
        self.gravar_o_que_estiver_pendente();
        self.trocar_corte(novo, true, cx);
        self.gravar_o_que_estiver_pendente();
    }

    /// Gira um quarto de volta no sentido horário (`]`).
    pub fn girar(&mut self, cx: &mut Context<Self>) {
        let novo = corte::girar(&self.corte_atual());
        self.gesto_do_corte(novo, cx);
    }

    /// Gira um quarto de volta no sentido anti-horário (`[`).
    pub fn girar_a_esquerda(&mut self, cx: &mut Context<Self>) {
        let novo = corte::girar_a_esquerda(&self.corte_atual());
        self.gesto_do_corte(novo, cx);
    }

    pub fn espelhar_horizontal(&mut self, cx: &mut Context<Self>) {
        let novo = corte::espelhar_horizontal(&self.corte_atual());
        self.gesto_do_corte(novo, cx);
    }

    pub fn espelhar_vertical(&mut self, cx: &mut Context<Self>) {
        let novo = corte::espelhar_vertical(&self.corte_atual());
        self.gesto_do_corte(novo, cx);
    }

    /// "Voltar à foto inteira".
    pub fn recomecar_corte(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.gesto_do_corte(corte::foto_inteira(), cx);
        if let Some(edicao) = self.edicao.as_mut() {
            edicao.desejado = None;
        }
        self.angulo
            .update(cx, |estado, cx| estado.set_value(0., window, cx));
    }

    /// A proporção remodela o retângulo **na hora**, mantendo a área.
    pub fn travar_proporcao(&mut self, proporcao: Option<f32>, cx: &mut Context<Self>) {
        let Some(edicao) = self.edicao.as_mut() else {
            return;
        };
        edicao.proporcao = proporcao;
        let (Some(p), Some(espaco)) = (proporcao, self.espaco()) else {
            cx.notify();
            return;
        };
        let atual = self.corte_atual();
        let remodelado = corte::com_proporcao_no_centro(
            corte::retangulo_de(&atual, espaco),
            p,
            espaco,
            atual.angle(),
        );
        self.gravar_o_que_estiver_pendente();
        self.trocar_corte(corte::com_retangulo(&atual, remodelado, espaco), false, cx);
        if let Some(edicao) = self.edicao.as_mut() {
            edicao.desejado = Some(remodelado);
        }
        self.gravar_o_que_estiver_pendente();
    }

    /// Endireita **e traz o retângulo para dentro da foto** — o zoom do
    /// endireitar. Não fecha o gesto: quem fecha é o soltar (transferidor) ou a
    /// espera da gravação (slider).
    pub(super) fn definir_angulo(&mut self, graus: f32, cx: &mut Context<Self>) {
        let (Some(espaco), Some(edicao)) = (self.espaco(), self.edicao.as_ref()) else {
            return;
        };
        let atual = self.corte_atual();
        if (graus - atual.angle()).abs() < f32::EPSILON {
            return;
        }
        let alvo = edicao
            .desejado
            .unwrap_or_else(|| corte::retangulo_de(&atual, espaco));
        if let Some(edicao) = self.edicao.as_mut() {
            edicao.desejado = Some(alvo);
        }
        let novo = corte::endireitar(&atual, graus, espaco, Some(alvo));
        self.corte = corte_de(&novo);
        self.pendente = true;
        // 🔑 O giro da foto fica para o quadro: um por desenho, e não um por
        // evento (ver `exibicao_atrasada`).
        self.exibicao_atrasada = true;
        self.revelar_de_novo_se_a_vinheta_segue_o_corte(cx);
        cx.notify();
    }

    /// O slider mudou: endireita e deixa a espera fechar o gesto.
    pub(super) fn angulo_do_slider(&mut self, graus: f32, cx: &mut Context<Self>) {
        let graus = (graus / PASSO_DO_ANGULO).round() * PASSO_DO_ANGULO;
        self.definir_angulo(graus, cx);
        if self.pendente {
            self.adiar_gravacao(cx);
        }
    }

    /// Endireitar volta ao neutro — o duplo clique do site.
    fn zerar_angulo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.gravar_o_que_estiver_pendente();
        self.definir_angulo(0., cx);
        self.gravar_o_que_estiver_pendente();
        self.angulo
            .update(cx, |estado, cx| estado.set_value(0., window, cx));
    }

    pub(super) fn comecar_arrasto(
        &mut self,
        alca: Option<Alca>,
        inicio: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(espaco) = self.espaco() else {
            return;
        };
        self.gravar_o_que_estiver_pendente();
        let inicial = corte::retangulo_de(&self.corte_atual(), espaco);
        if let Some(edicao) = self.edicao.as_mut() {
            edicao.arrasto = Some(ArrastoDoCorte {
                alca,
                inicio,
                inicial,
            });
            cx.notify();
        }
    }

    fn comecar_transferidor(
        &mut self,
        evento: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if evento.click_count >= 2 {
            self.zerar_angulo(window, cx);
            return;
        }
        self.gravar_o_que_estiver_pendente();
        let angulo = self.corte_atual().angle();
        if let Some(edicao) = self.edicao.as_mut() {
            edicao.transferidor = Some((f32::from(evento.position.x), angulo));
            cx.notify();
        }
    }

    /// Há um gesto do Enquadrar em curso (retângulo ou transferidor)?
    pub(super) fn arrastando_no_corte(&self) -> bool {
        self.edicao
            .as_ref()
            .is_some_and(|e| e.arrasto.is_some() || e.transferidor.is_some())
    }

    /// O ponteiro andou durante um gesto do Enquadrar.
    pub(super) fn mover_no_corte(
        &mut self,
        ponteiro: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edicao) = self.edicao.as_ref() else {
            return;
        };
        if let Some((x0, inicial)) = edicao.transferidor {
            let bruto = inicial + (f32::from(ponteiro.x) - x0) * GRAUS_POR_PIXEL;
            let preso = bruto.clamp(-ANGULO_MAXIMO, ANGULO_MAXIMO);
            let passo = ((preso / PASSO_DO_ANGULO).round() * PASSO_DO_ANGULO * 10.).round() / 10.;
            if (passo - self.corte_atual().angle()).abs() >= PASSO_DO_ANGULO / 2. {
                self.definir_angulo(passo, cx);
                self.angulo
                    .update(cx, |estado, cx| estado.set_value(passo, window, cx));
            }
            return;
        }
        let Some(arrasto) = edicao.arrasto else {
            return;
        };
        let proporcao = edicao.proporcao;
        let (Some(espaco), Some(area)) = (self.espaco(), self.area_da_foto()) else {
            return;
        };
        // Os deltas viram pixels do espaço girado: é nele que o retângulo mora.
        let escala = area.2 / espaco.0;
        let dx = f32::from(ponteiro.x - arrasto.inicio.x) / escala;
        let dy = f32::from(ponteiro.y - arrasto.inicio.y) / escala;
        let novo = corte::arrastar_em_pixels(
            arrasto.inicial,
            arrasto.alca,
            dx,
            dy,
            espaco,
            arrasto.alca.and(proporcao),
        );
        let atual = self.corte_atual();
        self.trocar_corte(corte::com_retangulo(&atual, novo, espaco), false, cx);
    }

    /// O botão subiu: o gesto vira um passo e vai para o banco.
    pub(super) fn soltar_no_corte(&mut self, cx: &mut Context<Self>) {
        let Some(edicao) = self.edicao.as_mut() else {
            return;
        };
        if edicao.arrasto.take().is_none() && edicao.transferidor.take().is_none() {
            return;
        }
        self.gravar_o_que_estiver_pendente();
        cx.notify();
    }

    /// O véu, o retângulo com a grade de terços e as oito alças.
    ///
    /// 🚨 **Fixos nos dois temas**, como no site: o véu preto a 60 %, o contorno
    /// e as alças brancas se leem contra a foto, e não contra o tema.
    pub(super) fn overlay_de_corte(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.edicao.as_ref()?;
        let espaco = self.espaco()?;
        let (ax, ay, aw, _) = self.area_da_foto()?;
        let escala = aw / espaco.0;
        let r = corte::retangulo_de(&self.corte_atual(), espaco);
        let (x0, y0, w, h) = (
            ax + r.x * escala,
            ay + r.y * escala,
            r.w * escala,
            r.h * escala,
        );

        let veu = gpui::rgba(0x00000099);
        // Quatro faixas até a borda do palco — o `shadow 9999px` do site.
        let (pw, ph) = (
            f32::from(self.palco.size.width),
            f32::from(self.palco.size.height),
        );
        let faixas = [
            (0., 0., pw, y0),
            (0., y0 + h, pw, ph - (y0 + h)),
            (0., y0, x0, h),
            (x0 + w, y0, pw - (x0 + w), h),
        ]
        .into_iter()
        .filter(|(_, _, fw, fh)| *fw > 0. && *fh > 0.)
        .map(|(fx, fy, fw, fh)| {
            div()
                .absolute()
                .left(px(fx))
                .top(px(fy))
                .w(px(fw))
                .h(px(fh))
                .bg(veu)
                .into_any_element()
        });

        // A grade de terços: branco a 50 %, com a camada a 60 %.
        let linha = gpui::rgba(0xffffff4d);
        let grade = (1..3).flat_map(|i| {
            let f = i as f32 / 3.;
            [
                div()
                    .absolute()
                    .left(px(w * f))
                    .top_0()
                    .w(px(1.))
                    .h_full()
                    .bg(linha),
                div()
                    .absolute()
                    .top(px(h * f))
                    .left_0()
                    .h(px(1.))
                    .w_full()
                    .bg(linha),
            ]
        });

        let alcas = Alca::TODAS.into_iter().map(|alca| {
            let (fx, fy) = alca.posicao();
            let cursor = match alca {
                Alca::SuperiorEsquerda | Alca::InferiorDireita => {
                    CursorStyle::ResizeUpLeftDownRight
                }
                Alca::SuperiorDireita | Alca::InferiorEsquerda => {
                    CursorStyle::ResizeUpRightDownLeft
                }
                Alca::Superior | Alca::Inferior => CursorStyle::ResizeUpDown,
                Alca::Esquerda | Alca::Direita => CursorStyle::ResizeLeftRight,
            };
            div()
                .id(SharedString::from(format!("alca-{alca:?}")))
                .absolute()
                .left(px(w * fx - LADO_DA_ALCA / 2.))
                .top(px(h * fy - LADO_DA_ALCA / 2.))
                .size(px(LADO_DA_ALCA))
                .rounded(px(2.))
                .bg(gpui::white())
                .border_1()
                .border_color(gpui::rgba(0x17171799))
                .shadow_sm()
                .cursor(cursor)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |tela, evento: &MouseDownEvent, _window, cx| {
                        cx.stop_propagation();
                        tela.comecar_arrasto(Some(alca), evento.position, cx);
                    }),
                )
        });

        Some(
            div()
                .absolute()
                .inset_0()
                .children(faixas)
                .child(
                    div()
                        .id("retangulo-de-corte")
                        .absolute()
                        .left(px(x0))
                        .top(px(y0))
                        .w(px(w))
                        .h(px(h))
                        .border_1()
                        .border_color(gpui::rgba(0xffffffcc))
                        .cursor(CursorStyle::OpenHand)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|tela, evento: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                                tela.comecar_arrasto(None, evento.position, cx);
                            }),
                        )
                        .children(grade)
                        .children(alcas),
                )
                .child(self.transferidor(cx))
                .into_any_element(),
        )
    }

    /// O transferidor, debaixo da foto: arrastar sobre o arco endireita.
    fn transferidor(&self, cx: &mut Context<Self>) -> AnyElement {
        let angulo = self.corte_atual().angle();
        let arrastando = self
            .edicao
            .as_ref()
            .is_some_and(|e| e.transferidor.is_some());
        // No claro a faixa é mais densa: fora da foto ela cai sobre o poço claro.
        let faixa = if cx.theme().mode.is_dark() {
            gpui::rgba(0x00000073)
        } else {
            gpui::rgba(0x000000b3)
        };
        let ambar = gpui::rgb(0xfbbf24);

        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(8.))
            .flex()
            .justify_center()
            .child(
                div()
                    .id("transferidor")
                    .relative()
                    .w(px(LARGURA_DO_TRANSFERIDOR))
                    .h(px(ALTURA_DO_TRANSFERIDOR))
                    .cursor(if arrastando {
                        CursorStyle::ClosedHand
                    } else {
                        CursorStyle::ResizeLeftRight
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|tela, evento: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            tela.comecar_transferidor(evento, window, cx);
                        }),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left(px(LARGURA_DO_TRANSFERIDOR / 2. - 130.))
                            .w(px(260.))
                            .h_full()
                            .rounded(px(10.))
                            .bg(faixa),
                    )
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds: Bounds<Pixels>, _, window, _| {
                                let (ox, oy) =
                                    (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
                                let mut a = -ANGULO_MAXIMO;
                                while a <= ANGULO_MAXIMO + 0.01 {
                                    let grande = (a % 15.).abs() < 0.01;
                                    let de = ponto_do_arco(a, RAIO);
                                    let ate = ponto_do_arco(a, RAIO - if grande { 8. } else { 4. });
                                    let cor = if a.abs() < 0.01 {
                                        gpui::rgba(0xffffffe6)
                                    } else if grande {
                                        gpui::rgba(0xffffff99)
                                    } else {
                                        gpui::rgba(0xffffff59)
                                    };
                                    let mut traco =
                                        PathBuilder::stroke(px(if grande { 1.5 } else { 1. }));
                                    traco.move_to(point(px(ox + de.0), px(oy + de.1)));
                                    traco.line_to(point(px(ox + ate.0), px(oy + ate.1)));
                                    if let Ok(caminho) = traco.build() {
                                        window.paint_path(caminho, cor);
                                    }
                                    a += 2.5;
                                }
                                let de = ponto_do_arco(angulo, RAIO + 2.);
                                let ate = ponto_do_arco(angulo, RAIO - 13.);
                                let mut cursor = PathBuilder::stroke(px(2.5));
                                cursor.move_to(point(px(ox + de.0), px(oy + de.1)));
                                cursor.line_to(point(px(ox + ate.0), px(oy + ate.1)));
                                if let Ok(caminho) = cursor.build() {
                                    window.paint_path(caminho, ambar);
                                }
                            },
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(2.))
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .font_family("Menlo")
                            .text_size(px(11.))
                            .text_color(if angulo == 0. {
                                gpui::rgba(0xffffff99).into()
                            } else {
                                gpui::Hsla::from(ambar)
                            })
                            .child(rotulo_do_angulo(angulo)),
                    ),
            )
            .into_any_element()
    }

    /// A coluna da direita no Enquadrar — o `PainelDeCorte` do site.
    pub(super) fn painel_de_corte(&self, cx: &mut Context<Self>) -> AnyElement {
        let atual = self.corte_atual();
        let angulo = atual.angle();
        let proporcao = self.edicao.as_ref().and_then(|e| e.proporcao);
        let tema = cx.theme();
        let (mudo, frente, borda, muted) = (
            tema.muted_foreground,
            tema.foreground,
            tema.border,
            tema.muted,
        );
        let saida = self.espaco().map(|espaco| {
            let r = corte::retangulo_de(&atual, espaco);
            (r.w.round() as u32, r.h.round() as u32)
        });

        let botao = |id: &'static str, icone: Icone, ligado: bool| {
            div()
                .id(id)
                .h(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .border_1()
                .border_color(borda)
                .cursor_pointer()
                .when(ligado, |b| {
                    b.bg(gpui::rgb(0xfbbf24)).text_color(gpui::black())
                })
                .when(!ligado, |b| {
                    b.text_color(frente.opacity(0.9))
                        .hover(move |s| s.bg(muted))
                })
                .child(Icon::new(icone).size(px(16.)))
        };

        v_flex()
            .gap(px(16.))
            .child(
                v_flex()
                    .child(div().mb(px(6.)).text_xs().text_color(mudo).child("Girar e espelhar"))
                    .child(
                        div()
                            .grid()
                            .grid_cols(4)
                            .gap(px(4.))
                            .child(
                                botao("corte-girar-esquerda", Icone::RotateCcw, false)
                                    .tooltip(|w, cx| gpui_component::tooltip::Tooltip::new("Girar à esquerda").build(w, cx))
                                    .on_click(cx.listener(|tela, _, _, cx| tela.girar_a_esquerda(cx))),
                            )
                            .child(
                                botao("corte-girar", Icone::RotateCw, false)
                                    .tooltip(|w, cx| gpui_component::tooltip::Tooltip::new("Girar à direita").build(w, cx))
                                    .on_click(cx.listener(|tela, _, _, cx| tela.girar(cx))),
                            )
                            .child(
                                botao("corte-espelho-h", Icone::FlipHorizontal, atual.flip_horizontal())
                                    .tooltip(|w, cx| gpui_component::tooltip::Tooltip::new("Espelhar na horizontal").build(w, cx))
                                    .on_click(cx.listener(|tela, _, _, cx| tela.espelhar_horizontal(cx))),
                            )
                            .child(
                                botao("corte-espelho-v", Icone::FlipVertical, atual.flip_vertical())
                                    .tooltip(|w, cx| gpui_component::tooltip::Tooltip::new("Espelhar na vertical").build(w, cx))
                                    .on_click(cx.listener(|tela, _, _, cx| tela.espelhar_vertical(cx))),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .text_xs()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_baseline()
                            .child(
                                div()
                                    .id("corte-angulo-rotulo")
                                    .text_color(if angulo != 0. { frente } else { mudo })
                                    .tooltip(|w, cx| gpui_component::tooltip::Tooltip::new("Duplo clique volta ao neutro").build(w, cx))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|tela, evento: &MouseDownEvent, window, cx| {
                                            if evento.click_count >= 2 {
                                                tela.zerar_angulo(window, cx);
                                            }
                                        }),
                                    )
                                    .child("Endireitar"),
                            )
                            .child(
                                div()
                                    .text_color(frente.opacity(0.9))
                                    .child(rotulo_do_angulo(angulo)),
                            ),
                    )
                    .child(div().mt(px(2.)).child(Slider::new(&self.angulo).horizontal())),
            )
            .child(
                v_flex()
                    .child(div().mb(px(6.)).text_xs().text_color(mudo).child("Proporção"))
                    .child(
                        h_flex().flex_wrap().gap(px(4.)).children(corte::PROPORCOES.iter().map(
                            |(rotulo, valor)| {
                                let escolhida = proporcao == *valor;
                                let valor = *valor;
                                div()
                                    .id(SharedString::from(format!("prop-{rotulo}")))
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .text_xs()
                                    .cursor_pointer()
                                    .when(escolhida, |b| b.bg(muted).text_color(frente))
                                    .when(!escolhida, |b| {
                                        b.text_color(mudo).hover(move |s| s.text_color(frente))
                                    })
                                    .on_click(cx.listener(move |tela, _, _, cx| {
                                        tela.travar_proporcao(valor, cx)
                                    }))
                                    .child(*rotulo)
                            },
                        )),
                    )
                    .child(
                        div()
                            .mt(px(4.))
                            .text_size(px(11.))
                            .text_color(mudo)
                            .child(
                                "Escolher remodela o retângulo na hora, mantendo a área; depois ela vale para cada arrasto de alça. Arrastar o meio só move.",
                            ),
                    ),
            )
            .child({
                let inteiro = corte::e_inteiro(&atual);
                estilo::desligado(
                    estilo::botao_contorno("corte-recomecar", cx)
                        .w_full()
                        .h(px(32.))
                        .text_sm()
                        .gap(px(8.))
                        .child(Icon::new(Icone::Undo).size(px(16.)))
                        .child("Voltar à foto inteira"),
                    inteiro,
                )
                .when(!inteiro, |b| {
                    b.on_click(cx.listener(|tela, _, window, cx| tela.recomecar_corte(window, cx)))
                })
            })
            .children(saida.map(|(largura, altura)| {
                div()
                    .text_size(px(11.))
                    .line_height(px(15.))
                    .text_color(mudo)
                    .child(
                        h_flex()
                            .flex_wrap()
                            .child("A foto sai com\u{a0}")
                            .child(
                                div()
                                    .text_color(frente.opacity(0.9))
                                    .child(format!("{largura}×{altura}")),
                            )
                            .child("\u{a0}na resolução da tela; no arquivo, na proporção do original."),
                    )
            }))
            .into_any_element()
    }
}

impl Revelacao {
    /// Um gesto do roteiro de depuração (`revelacao …`).
    pub fn seguir_o_roteiro(&mut self, gesto: &str, window: &mut Window, cx: &mut Context<Self>) {
        let mut partes = gesto.split_whitespace();
        let nome = partes.next().unwrap_or_default();
        let numero = partes.next().and_then(|n| n.parse::<f32>().ok());
        match nome {
            "enquadrar" => self.alternar_corte(window, cx),
            "angulo" => {
                let graus = numero.unwrap_or(0.);
                self.gravar_o_que_estiver_pendente();
                self.definir_angulo(graus, cx);
                self.gravar_o_que_estiver_pendente();
                self.angulo
                    .update(cx, |estado, cx| estado.set_value(graus, window, cx));
            }
            "proporcao" => self.travar_proporcao(numero, cx),
            "girar" => self.girar(cx),
            "zoom" => self.z_apertado(cx),
            "ajuda" => self.alternar_ajuda(cx),
            "antes" => self.ver_o_antes(numero.unwrap_or(1.) > 0., cx),
            // 🧪 O lote da tira e o "Sincronizar N", para o roteiro chegar ao
            // diálogo sem mouse.
            "marcar_tudo" => self.marcar_todas(cx),
            "sincronizar" => self.abrir_sincronizacao(window, cx),
            "sincronizar_ok" => cx.emit(super::PedidoDaRevelacao::Sincronizar),
            "baixar_jpeg" => cx.emit(super::PedidoDaRevelacao::Exportar),
            outro => eprintln!("[roteiro] gesto da revelação desconhecido: {outro}"),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_rotulo_do_angulo_e_o_do_site() {
        assert_eq!(rotulo_do_angulo(0.), "0.0°");
        assert_eq!(rotulo_do_angulo(3.5), "+3.5°");
        assert_eq!(rotulo_do_angulo(-0.1), "-0.1°");
    }

    #[test]
    fn o_arco_tem_o_neutro_no_topo_e_e_simetrico() {
        let (x, y) = ponto_do_arco(0., RAIO);
        assert!((x - LARGURA_DO_TRANSFERIDOR / 2.).abs() < 1e-3);
        assert!((y - TOPO).abs() < 1e-3);
        let (xe, ye) = ponto_do_arco(-ANGULO_MAXIMO, RAIO);
        let (xd, yd) = ponto_do_arco(ANGULO_MAXIMO, RAIO);
        assert!((xe + xd - LARGURA_DO_TRANSFERIDOR).abs() < 1e-3);
        assert!((ye - yd).abs() < 1e-3);
        assert!(yd < ALTURA_DO_TRANSFERIDOR);
    }
}
