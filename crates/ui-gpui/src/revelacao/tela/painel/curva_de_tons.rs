//! 🎞️ A Curva de tons no formato do Lightroom (dono, 30/09/2026: *"Também
//! quero a curva de tons assim"*, com a foto do painel do Lightroom Classic).
//!
//! Um painel só, de cima para baixo:
//!
//! - a fileira **Ajustes:** — a curva paramétrica e os quatro canais da curva
//!   por ponto (RGB, vermelho, verde e azul), cada um num círculo da sua cor;
//! - o gráfico, com o histograma da foto por trás;
//! - na paramétrica, a barra com os três pinos da divisão das regiões e os
//!   quatro sliders na ordem do Lightroom: Realces, Claros, Escuros, Sombras.
//!
//! Até aqui eram dois painéis: a "Curva de tons" só com os sliders, sem
//! gráfico, e a "Curva por ponto" com abas de texto. No site ainda são dois.
//!
//! ⚠️ **O ⊙ do canto (ajuste direto sobre a foto) ainda não existe.** Ele é
//! uma ferramenta do palco, e o ícone só aparece quando ela mover a foto.
//!
//! 🔧 **A barra dos pinos é desenhada à mão**: o `Slider` do kit tem no máximo
//! dois punhos, e a divisão tem três.

use gpui_kit::component::button::Button;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme, Disableable, Icon, Selectable as _, Sizable,
};
use gpui_kit::{
    canvas, div, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, Bounds, Context,
    DragMoveEvent, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, PathBuilder, Pixels, Point,
    SharedString, Window,
};

use super::{
    altura_do_ponteiro, no_sob_o_ponteiro, ArrastoDoNo, SemFantasma, LADO_DA_CURVA,
    MARGEM_DA_CURVA, RAIO_DO_NO,
};
use crate::recursos::Icone;
use crate::revelacao::controles::Secao;
use crate::revelacao::curva::{self, Canal, Regiao, PONTOS, PONTOS_DA_CURVA};
use crate::revelacao::processador::Ajustes;
use crate::revelacao::tela::Revelacao;

/// O que o gráfico mostra e o que o arrasto nele move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModoDaCurva {
    /// As quatro regiões e os três pinos — o ícone da curva.
    Parametrica,
    /// Os nove nós de um canal da curva por ponto — os círculos.
    Ponto(Canal),
}

/// O arrasto vertical dentro do gráfico paramétrico: a região que o ponteiro
/// pegou, o valor dela no começo e a altura de onde ele partiu.
#[derive(Debug, Clone, Copy)]
pub(in crate::revelacao::tela) struct ArrastoDeRegiao {
    regiao: Regiao,
    valor_inicial: f32,
    y_inicial: f32,
}

/// O tipo do arrasto de uma região no gráfico.
#[derive(Clone, Copy)]
struct ArrastoNaParametrica;

/// O tipo do arrasto de um pino da barra de divisão.
#[derive(Clone, Copy)]
struct ArrastoDoPino;

/// A altura da barra de divisão — o trilho e os pinos embaixo dele.
const ALTURA_DA_BARRA: f32 = 16.0;
/// A espessura do trilho da barra.
const TRILHO_DA_BARRA: f32 = 5.0;
/// Até onde o clique ainda pega um pino, em pixels.
const ALCANCE_DO_PINO: f32 = 9.0;

/// A conta de pixel do quadro — a mesma do editor de curva do site, num
/// `viewBox` de [`LADO_DA_CURVA`] com [`MARGEM_DA_CURVA`] de folga.
#[derive(Clone, Copy)]
struct Geometria {
    x0: f32,
    y0: f32,
    escala: f32,
    util: f32,
}

impl Geometria {
    fn de(bounds: Bounds<Pixels>) -> Self {
        Self {
            x0: f32::from(bounds.origin.x),
            y0: f32::from(bounds.origin.y),
            escala: f32::from(bounds.size.width) / LADO_DA_CURVA,
            util: LADO_DA_CURVA - MARGEM_DA_CURVA * 2.0,
        }
    }

    /// O x de um nível de 0 a 1.
    fn x(&self, nivel: f32) -> f32 {
        self.x0 + (MARGEM_DA_CURVA + nivel * self.util) * self.escala
    }

    /// O y de um nível de 0 a 1.
    fn y(&self, nivel: f32) -> f32 {
        self.y0 + (MARGEM_DA_CURVA + self.util - nivel * self.util) * self.escala
    }

    /// O nível (0–1) de um x da janela.
    fn nivel_do_x(&self, x: f32) -> f32 {
        if self.escala <= 0.0 {
            return 0.0;
        }
        ((x - self.x0) / self.escala - MARGEM_DA_CURVA) / self.util
    }

    /// Quantos pixels de tela a área útil tem de altura.
    fn altura_util(&self) -> f32 {
        self.util * self.escala
    }
}

fn ponto(x: f32, y: f32) -> Point<Pixels> {
    gpui_kit::point(px(x), px(y))
}

/// O histograma em uma curva de luminância, de 0 a 1 — a média dos três
/// canais em cada nível, com o pico no teto.
///
/// ⚠️ **O pico é medido sem os dois extremos** (o nível 0 e o 255), e o que
/// passa do teto é cortado. O histograma é o da foto revelada: com os
/// Claros lá em cima, os brancos estourados empilham num nível só, e medido
/// por ele o resto do histograma sumia do gráfico (conferido na janela, 01/10).
fn luminancia_do_histograma(barras: &[(f32, f32, f32)]) -> Vec<f32> {
    let media: Vec<f32> = barras.iter().map(|(r, g, b)| (r + g + b) / 3.0).collect();
    let miolo = media.get(1..media.len().saturating_sub(1)).unwrap_or(&[]);
    let pico = miolo.iter().copied().fold(0.0_f32, f32::max);
    let pico = if pico > 0.0 {
        pico
    } else {
        media.iter().copied().fold(0.0_f32, f32::max)
    };
    if pico <= 0.0 {
        return Vec::new();
    }
    media.iter().map(|v| (v / pico).min(1.0)).collect()
}

/// O valor de uma região depois de um arrasto vertical: a altura inteira da
/// área útil leva de −100 a +100, e o resultado é inteiro, como o slider.
fn valor_do_arrasto(arrasto: &ArrastoDeRegiao, y: f32, altura_util: f32) -> f32 {
    if altura_util <= 0.0 {
        return arrasto.valor_inicial;
    }
    let delta = (arrasto.y_inicial - y) / altura_util * 200.0;
    (arrasto.valor_inicial + delta).clamp(-100.0, 100.0).round()
}

/// O pino mais perto do ponteiro, se algum estiver ao alcance.
fn pino_sob_o_ponteiro(geometria: &Geometria, x: f32, ajustes: &Ajustes) -> Option<usize> {
    curva::divisores(ajustes)
        .iter()
        .enumerate()
        .map(|(i, d)| (i, (geometria.x(d / 100.0) - x).abs()))
        .filter(|(_, distancia)| *distancia <= ALCANCE_DO_PINO)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Fundo, histograma, grade e a diagonal — o que os dois modos têm.
fn pintar_o_fundo(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    g: &Geometria,
    histograma: &[f32],
    cores: &Cores,
) {
    window.paint_quad(gpui_kit::fill(bounds, cores.fundo).corner_radii(px(4.)));

    // O histograma por trás, em cinza, como no Lightroom.
    if !histograma.is_empty() {
        let ultimo = (histograma.len() - 1).max(1) as f32;
        let largura = (g.x(1.0) - g.x(0.0)) / histograma.len() as f32;
        let base = g.y(0.0);
        for (i, altura) in histograma.iter().enumerate() {
            let alta = altura * g.altura_util();
            if alta <= 0.0 {
                continue;
            }
            let x = g.x(i as f32 / ultimo) - largura * 0.5;
            window.paint_quad(gpui_kit::fill(
                Bounds {
                    origin: ponto(x, base - alta),
                    size: gpui_kit::size(px(largura.max(1.0)), px(alta)),
                },
                cores.histograma,
            ));
        }
    }

    // A grade dos quartos de tom, e a diagonal do neutro por baixo.
    for f in [0.25, 0.5, 0.75] {
        for (a, b) in [
            ((g.x(f), g.y(0.0)), (g.x(f), g.y(1.0))),
            ((g.x(0.0), g.y(f)), (g.x(1.0), g.y(f))),
        ] {
            let mut linha = PathBuilder::stroke(px(0.5 * g.escala));
            linha.move_to(ponto(a.0, a.1));
            linha.line_to(ponto(b.0, b.1));
            if let Ok(caminho) = linha.build() {
                window.paint_path(caminho, cores.grade);
            }
        }
    }
    let mut diagonal = PathBuilder::stroke(px(0.5 * g.escala))
        .dash_array(&[px(3.0 * g.escala), px(3.0 * g.escala)]);
    diagonal.move_to(ponto(g.x(0.0), g.y(0.0)));
    diagonal.line_to(ponto(g.x(1.0), g.y(1.0)));
    if let Ok(caminho) = diagonal.build() {
        window.paint_path(caminho, cores.grade);
    }
}

/// Um traço pelos `PONTOS` de uma curva de 0–1.
fn tracar(window: &mut Window, g: &Geometria, pontos: &[f32; PONTOS], cor: Hsla) {
    let mut traco = PathBuilder::stroke(px(1.5 * g.escala));
    let ultimo = (PONTOS - 1) as f32;
    for (i, v) in pontos.iter().enumerate() {
        let p = ponto(g.x(i as f32 / ultimo), g.y(*v));
        if i == 0 {
            traco.move_to(p);
        } else {
            traco.line_to(p);
        }
    }
    if let Ok(caminho) = traco.build() {
        window.paint_path(caminho, cor);
    }
}

/// As cores do quadro, lidas do tema antes de desenhar.
#[derive(Clone, Copy)]
struct Cores {
    fundo: Hsla,
    grade: Hsla,
    histograma: Hsla,
    traco: Hsla,
    alcance: Hsla,
}

impl Revelacao {
    /// O painel "Curva de tons" inteiro, aberto.
    pub(super) fn painel_da_curva_de_tons(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let modo = self.estado_do_painel.modo;
        let mut dentro = vec![
            self.fileira_dos_ajustes_da_curva(cx),
            self.grafico_da_curva(cx),
        ];
        if modo == ModoDaCurva::Parametrica {
            dentro.push(self.barra_de_divisao(cx));
            dentro.push(
                div()
                    .w_full()
                    .text_center()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Região")
                    .into_any_element(),
            );
            // Realces em cima e Sombras embaixo, como no Lightroom: a ordem
            // do gráfico, de cima para baixo.
            let mut regioes: Vec<(usize, Regiao)> = self
                .controles
                .iter()
                .enumerate()
                .filter(|(_, c)| c.definicao.secao == Secao::CurvaDeTons)
                .map(|(i, _)| i)
                .zip(Regiao::TODAS)
                .collect();
            regioes.reverse();
            for (indice, regiao) in regioes {
                let linha = self.linha_do_controle(indice, &self.controles[indice], cx);
                dentro.push(
                    div()
                        .id(SharedString::from(format!("regiao-{}", regiao.rotulo())))
                        .on_hover(cx.listener(move |tela, entrou: &bool, _window, cx| {
                            let foco = entrou.then_some(regiao);
                            if tela.estado_do_painel.regiao_em_foco != foco {
                                tela.estado_do_painel.regiao_em_foco = foco;
                                cx.notify();
                            }
                        }))
                        .child(linha)
                        .into_any_element(),
                );
            }
        }
        dentro
    }

    /// "Ajustes:" e os cinco botões — a paramétrica e os quatro canais.
    fn fileira_dos_ajustes_da_curva(&self, cx: &mut Context<Self>) -> AnyElement {
        let modo = self.estado_do_painel.modo;
        let ligado = self.controles_ligados();
        let texto = cx.theme().foreground;

        let botao = |modo_do_botao: ModoDaCurva, cx: &mut Context<Self>| -> AnyElement {
            let escolhido = modo == modo_do_botao;
            let (id, dica, conteudo): (&str, &str, AnyElement) = match modo_do_botao {
                ModoDaCurva::Parametrica => (
                    "curva-parametrica",
                    "Curva paramétrica",
                    Icon::new(Icone::Spline).size(px(14.)).into_any_element(),
                ),
                ModoDaCurva::Ponto(canal) => {
                    let cor: Hsla = canal.cor().map_or(texto, |hex| gpui_kit::rgb(hex).into());
                    // 🔑 O círculo cheio avisa que o canal tem curva: sem ele,
                    // um preset que mexe só no azul parece não ter feito nada.
                    let usado = !curva::curva_eh_neutra(&canal.alturas(&self.ajustes));
                    let (id, dica) = match canal {
                        Canal::Rgb => ("canal-RGB", "Curva por ponto: RGB"),
                        Canal::Vermelho => ("canal-Vermelho", "Curva por ponto: vermelho"),
                        Canal::Verde => ("canal-Verde", "Curva por ponto: verde"),
                        Canal::Azul => ("canal-Azul", "Curva por ponto: azul"),
                    };
                    let circulo = div()
                        .size(px(12.))
                        .rounded_full()
                        .border_2()
                        .border_color(cor)
                        .when(usado, |c| c.bg(cor.opacity(0.6)));
                    (id, dica, circulo.into_any_element())
                }
            };
            v_flex()
                .items_center()
                .gap(px(1.))
                .child(
                    Button::new(id)
                        .ghost()
                        .xsmall()
                        .selected(escolhido)
                        .tooltip(dica)
                        .debug_selector(move || id.to_string())
                        .child(conteudo)
                        .on_click(cx.listener(move |tela, _ev, _window, cx| {
                            tela.estado_do_painel.modo = modo_do_botao;
                            tela.estado_do_painel.regiao_em_foco = None;
                            cx.notify();
                        })),
                )
                // O pontinho embaixo do escolhido, como no Lightroom.
                .child(
                    div()
                        .size(px(3.))
                        .rounded_full()
                        .when(escolhido, |p| p.bg(cx.theme().foreground)),
                )
                .into_any_element()
        };

        let mut botoes = vec![botao(ModoDaCurva::Parametrica, cx)];
        for canal in Canal::TODOS {
            botoes.push(botao(ModoDaCurva::Ponto(canal), cx));
        }

        let zerar = match modo {
            ModoDaCurva::Ponto(canal) => {
                let neutro = curva::curva_eh_neutra(&canal.alturas(&self.ajustes));
                Some(
                    Button::new("zerar-canal")
                        .ghost()
                        .xsmall()
                        .text_color(cx.theme().muted_foreground)
                        .tooltip("Devolve este canal à reta")
                        .disabled(!ligado || neutro)
                        .when(ligado && !neutro, |z| {
                            z.on_click(cx.listener(move |tela, _ev, window, cx| {
                                let neutra = curva::curva_neutra();
                                tela.gesto_discreto(
                                    |a| {
                                        for (i, v) in neutra.iter().enumerate() {
                                            canal.definir(a, i, *v);
                                        }
                                    },
                                    window,
                                    cx,
                                );
                            }))
                        })
                        .child("Zerar"),
                )
            }
            ModoDaCurva::Parametrica => None,
        };

        h_flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .child(
                h_flex()
                    .items_start()
                    .gap(px(2.))
                    .child(
                        div()
                            .pt(px(4.))
                            .pr(px(4.))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Ajustes:"),
                    )
                    .children(botoes),
            )
            .children(zerar)
            .into_any_element()
    }

    /// O gráfico — paramétrico ou por ponto — com o histograma por trás.
    fn grafico_da_curva(&self, cx: &mut Context<Self>) -> AnyElement {
        let estado = &self.estado_do_painel;
        let modo = estado.modo;
        let ligado = self.controles_ligados();
        let ajustes = self.ajustes;

        let histograma = self
            .histograma
            .as_ref()
            .map(|h| luminancia_do_histograma(&h.alturas().collect::<Vec<_>>()))
            .unwrap_or_default();
        let cores = Cores {
            fundo: cx.theme().background,
            grade: cx.theme().border,
            histograma: cx.theme().muted_foreground.opacity(0.28),
            traco: cx.theme().foreground,
            alcance: cx.theme().foreground.opacity(0.12),
        };
        let traco = cores.traco;
        let cor_do_canal =
            move |c: Canal| -> Hsla { c.cor().map_or(traco, |hex| gpui_kit::rgb(hex).into()) };

        // A região em foco: a do arrasto, ou a que está sob o ponteiro.
        let foco = estado
            .arrasto_da_regiao
            .map(|a| a.regiao)
            .or(estado.regiao_em_foco);

        let area = estado.area_da_curva.clone();
        let desenho = canvas(
            move |bounds, _window, _cx| {
                area.set(bounds);
            },
            move |bounds, _prepaint, window, _cx| {
                let g = Geometria::de(bounds);
                pintar_o_fundo(window, bounds, &g, &histograma, &cores);

                match modo {
                    ModoDaCurva::Parametrica => {
                        // A faixa do alcance da região em foco: a curva com o
                        // slider dela em −100 e em +100.
                        if let Some(regiao) = foco {
                            let (baixo, alto) = curva::alcance_da_regiao(&ajustes, regiao);
                            let ultimo = (PONTOS - 1) as f32;
                            let mut faixa = PathBuilder::fill();
                            for (i, v) in alto.iter().enumerate() {
                                let p = ponto(g.x(i as f32 / ultimo), g.y(*v));
                                if i == 0 {
                                    faixa.move_to(p);
                                } else {
                                    faixa.line_to(p);
                                }
                            }
                            for (i, v) in baixo.iter().enumerate().rev() {
                                faixa.line_to(ponto(g.x(i as f32 / ultimo), g.y(*v)));
                            }
                            faixa.close();
                            if let Ok(caminho) = faixa.build() {
                                window.paint_path(caminho, cores.alcance);
                            }
                        }
                        tracar(window, &g, &curva::curva_parametrica(&ajustes), cores.traco);
                    }
                    ModoDaCurva::Ponto(canal) => {
                        let alturas = canal.alturas(&ajustes);
                        let cor = cor_do_canal(canal);
                        // O traço, 65 amostras como no site — a conta do motor
                        // (`avaliar_curva`), e não uma spline bonita.
                        let mut traco = PathBuilder::stroke(px(1.75 * g.escala));
                        for i in 0..=64 {
                            let x = i as f32 / 64.0;
                            let y = curva::avaliar_curva(&alturas, x * 255.0) / 255.0;
                            let p = ponto(g.x(x), g.y(y));
                            if i == 0 {
                                traco.move_to(p);
                            } else {
                                traco.line_to(p);
                            }
                        }
                        if let Ok(caminho) = traco.build() {
                            window.paint_path(caminho, cor);
                        }
                        let raio = RAIO_DO_NO * g.escala;
                        for (i, altura) in alturas.iter().enumerate() {
                            let cx_ = g.x(i as f32 / (PONTOS_DA_CURVA - 1) as f32);
                            let cy_ = g.y(altura / 255.0);
                            window.paint_quad(
                                gpui_kit::fill(
                                    Bounds {
                                        origin: ponto(cx_ - raio, cy_ - raio),
                                        size: gpui_kit::size(px(raio * 2.0), px(raio * 2.0)),
                                    },
                                    cor,
                                )
                                .corner_radii(px(raio)),
                            );
                        }
                    }
                }
            },
        )
        .size_full();

        // "Claros: +12" no canto, como o Lightroom escreve a região em foco.
        let rotulo = (modo == ModoDaCurva::Parametrica)
            .then_some(foco)
            .flatten()
            .map(|regiao| {
                let valor = regiao.ler(&self.ajustes);
                let texto = if valor > 0.0 {
                    format!("{}: +{valor:.0}", regiao.rotulo())
                } else {
                    format!("{}: {valor:.0}", regiao.rotulo())
                };
                div()
                    .absolute()
                    .top(px(6.))
                    .left(px(10.))
                    .text_xs()
                    .text_color(cx.theme().foreground)
                    .child(texto)
            });

        let quadro = div()
            .id("editor-de-curva")
            .debug_selector(|| "grafico-da-curva".into())
            .relative()
            .w_full()
            // Quadrado, como o `viewBox` do site: a conta de pixel usa a
            // largura para os dois eixos, e uma altura fixa cortava o pé do
            // gráfico numa coluna mais larga.
            .aspect_ratio(1.0)
            .when(!ligado, |q| q.opacity(0.4))
            .child(desenho)
            .children(rotulo);

        let area = estado.area_da_curva.clone();
        match modo {
            ModoDaCurva::Parametrica => quadro
                .on_hover(cx.listener(|tela, entrou: &bool, _window, cx| {
                    if !*entrou && tela.estado_do_painel.regiao_em_foco.is_some() {
                        tela.estado_do_painel.regiao_em_foco = None;
                        cx.notify();
                    }
                }))
                .on_mouse_move(cx.listener({
                    let area = area.clone();
                    move |tela, evento: &MouseMoveEvent, _window, cx| {
                        if tela.estado_do_painel.arrasto_da_regiao.is_some() {
                            return;
                        }
                        let g = Geometria::de(area.get());
                        let nivel = g.nivel_do_x(f32::from(evento.position.x)).clamp(0.0, 1.0);
                        let foco = Some(curva::regiao_em(nivel, &tela.ajustes));
                        if tela.estado_do_painel.regiao_em_foco != foco {
                            tela.estado_do_painel.regiao_em_foco = foco;
                            cx.notify();
                        }
                    }
                }))
                .when(ligado, |q| {
                    q.cursor_ns_resize()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |tela, evento: &MouseDownEvent, window, cx| {
                                let g = Geometria::de(area.get());
                                let nivel =
                                    g.nivel_do_x(f32::from(evento.position.x)).clamp(0.0, 1.0);
                                let regiao = curva::regiao_em(nivel, &tela.ajustes);
                                if evento.click_count >= 2 {
                                    // Duplo clique devolve a região ao zero.
                                    tela.estado_do_painel.arrasto_da_regiao = None;
                                    tela.gesto_discreto(|a| regiao.definir(a, 0.0), window, cx);
                                    return;
                                }
                                tela.estado_do_painel.arrasto_da_regiao = Some(ArrastoDeRegiao {
                                    regiao,
                                    valor_inicial: regiao.ler(&tela.ajustes),
                                    y_inicial: f32::from(evento.position.y),
                                });
                                tela.estado_do_painel.regiao_em_foco = Some(regiao);
                                cx.notify();
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|tela, _evento, _window, cx| {
                                tela.estado_do_painel.arrasto_da_regiao = None;
                                cx.notify();
                            }),
                        )
                        .on_drag(ArrastoNaParametrica, |_, _, _, cx| cx.new(|_| SemFantasma))
                        .on_drag_move(cx.listener(
                            move |tela,
                                  evento: &DragMoveEvent<ArrastoNaParametrica>,
                                  window,
                                  cx| {
                                let Some(arrasto) = tela.estado_do_painel.arrasto_da_regiao else {
                                    return;
                                };
                                let g = Geometria::de(evento.bounds);
                                let valor = valor_do_arrasto(
                                    &arrasto,
                                    f32::from(evento.event.position.y),
                                    g.altura_util(),
                                );
                                tela.mover_regiao(arrasto.regiao, valor, window, cx);
                            },
                        ))
                })
                .into_any_element(),
            ModoDaCurva::Ponto(canal) => {
                let alturas = canal.alturas(&self.ajustes);
                quadro
                    .when(ligado, |q| {
                        q.cursor_ns_resize()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |tela, evento: &MouseDownEvent, window, cx| {
                                    let limites = area.get();
                                    let no = no_sob_o_ponteiro(limites, evento.position, &alturas);
                                    tela.estado_do_painel.no_arrastado = no;
                                    if let (Some(i), true) = (no, evento.click_count >= 2) {
                                        tela.devolver_no_a_reta(canal, i, window, cx);
                                    }
                                }),
                            )
                            .on_drag(ArrastoDoNo, |_, _, _, cx| cx.new(|_| SemFantasma))
                            .on_drag_move(cx.listener(
                                move |tela, evento: &DragMoveEvent<ArrastoDoNo>, _window, cx| {
                                    let Some(i) = tela.estado_do_painel.no_arrastado else {
                                        return;
                                    };
                                    if !tela.controles_ligados() {
                                        return;
                                    }
                                    let altura =
                                        altura_do_ponteiro(evento.bounds, evento.event.position);
                                    tela.mover_no_da_curva(i, altura, cx);
                                },
                            ))
                    })
                    .into_any_element()
            }
        }
    }

    /// A barra de divisão: o trilho do preto ao branco e os três pinos.
    fn barra_de_divisao(&self, cx: &mut Context<Self>) -> AnyElement {
        let ligado = self.controles_ligados();
        let divisores = curva::divisores(&self.ajustes);
        let arrastado = self.estado_do_painel.pino_arrastado;
        let pino = cx.theme().foreground.opacity(0.85);
        let pino_ativo = cx.theme().foreground;
        let area = self.estado_do_painel.area_da_barra.clone();

        let desenho = canvas(
            move |bounds, _window, _cx| {
                area.set(bounds);
            },
            move |bounds, _prepaint, window, _cx| {
                let g = Geometria::de(bounds);
                let topo = f32::from(bounds.origin.y);
                let trilho = Bounds {
                    origin: ponto(g.x(0.0), topo),
                    size: gpui_kit::size(px(g.x(1.0) - g.x(0.0)), px(TRILHO_DA_BARRA)),
                };
                window.paint_quad(
                    gpui_kit::fill(
                        trilho,
                        linear_gradient(
                            90.,
                            linear_color_stop(gpui_kit::black(), 0.),
                            linear_color_stop(gpui_kit::rgb(0xd4d4d4), 1.),
                        ),
                    )
                    .corner_radii(px(TRILHO_DA_BARRA / 2.0)),
                );
                // Os pinos: triângulos com a ponta no trilho.
                for (i, d) in divisores.iter().enumerate() {
                    let x = g.x(d / 100.0);
                    let ponta = topo + TRILHO_DA_BARRA + 1.0;
                    let mut triangulo = PathBuilder::fill();
                    triangulo.move_to(ponto(x, ponta));
                    triangulo.line_to(ponto(x + 5.0, ponta + 8.0));
                    triangulo.line_to(ponto(x - 5.0, ponta + 8.0));
                    triangulo.close();
                    if let Ok(caminho) = triangulo.build() {
                        let cor = if arrastado == Some(i) {
                            pino_ativo
                        } else {
                            pino
                        };
                        window.paint_path(caminho, cor);
                    }
                }
            },
        )
        .size_full();

        let area = self.estado_do_painel.area_da_barra.clone();
        div()
            .id("divisao-das-regioes")
            .debug_selector(|| "barra-de-divisao".into())
            .w_full()
            .h(px(ALTURA_DA_BARRA))
            .when(!ligado, |b| b.opacity(0.4))
            .child(desenho)
            .when(ligado, |b| {
                b.cursor_ew_resize()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |tela, evento: &MouseDownEvent, window, cx| {
                            let g = Geometria::de(area.get());
                            let i = pino_sob_o_ponteiro(
                                &g,
                                f32::from(evento.position.x),
                                &tela.ajustes,
                            );
                            tela.estado_do_painel.pino_arrastado = i;
                            if let (Some(i), true) = (i, evento.click_count >= 2) {
                                // Duplo clique devolve o pino ao neutro.
                                let neutro = curva::divisores(&Ajustes::default())[i];
                                tela.estado_do_painel.pino_arrastado = None;
                                tela.gesto_discreto(
                                    |a| curva::definir_divisor(a, i, neutro),
                                    window,
                                    cx,
                                );
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|tela, _evento, _window, cx| {
                            tela.estado_do_painel.pino_arrastado = None;
                            cx.notify();
                        }),
                    )
                    .on_drag(ArrastoDoPino, |_, _, _, cx| cx.new(|_| SemFantasma))
                    .on_drag_move(cx.listener(
                        move |tela, evento: &DragMoveEvent<ArrastoDoPino>, _window, cx| {
                            let Some(i) = tela.estado_do_painel.pino_arrastado else {
                                return;
                            };
                            let g = Geometria::de(evento.bounds);
                            let valor = g.nivel_do_x(f32::from(evento.event.position.x)) * 100.0;
                            tela.mover_divisor(i, valor, cx);
                        },
                    ))
            })
            .into_any_element()
    }

    /// Uma região arrastada no gráfico: como o slider dela, pede a GPU e deixa
    /// a espera fechar o gesto — e o slider acompanha.
    fn mover_regiao(
        &mut self,
        regiao: Regiao,
        valor: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.controles_ligados() || regiao.ler(&self.ajustes) == valor {
            return;
        }
        regiao.definir(&mut self.ajustes, valor);
        for controle in self
            .controles
            .iter()
            .filter(|c| c.definicao.secao == Secao::CurvaDeTons)
        {
            let valor = (controle.definicao.ler)(&self.ajustes);
            controle
                .estado
                .update(cx, |estado, cx| estado.set_value(valor, window, cx));
        }
        self.pedir_revelacao(cx);
        self.adiar_gravacao(cx);
        cx.notify();
    }

    /// Um pino arrastado até `valor` (0–100): inteiro, dentro dos limites do
    /// Lightroom e sem cruzar os vizinhos.
    pub(crate) fn mover_divisor(&mut self, i: usize, valor: f32, cx: &mut Context<Self>) {
        if !self.controles_ligados() || i > 2 {
            return;
        }
        let (minimo, maximo) = curva::limites_do_divisor(&self.ajustes, i);
        let valor = valor.round().clamp(minimo, maximo);
        if curva::divisores(&self.ajustes)[i] == valor {
            return;
        }
        curva::definir_divisor(&mut self.ajustes, i, valor);
        self.pedir_revelacao(cx);
        self.adiar_gravacao(cx);
        cx.notify();
    }
}

/// 🧪 O que a curva mostra, para os cenários de ponta a ponta.
#[cfg(test)]
impl Revelacao {
    pub(crate) fn modo_da_curva(&self) -> ModoDaCurva {
        self.estado_do_painel.modo
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn quadro() -> Bounds<Pixels> {
        Bounds {
            origin: gpui_kit::point(px(100.), px(50.)),
            size: gpui_kit::size(px(260.), px(260.)),
        }
    }

    #[test]
    fn o_x_e_o_nivel_se_desfazem() {
        let g = Geometria::de(quadro());
        for nivel in [0.0, 0.25, 0.5, 1.0] {
            assert!((g.nivel_do_x(g.x(nivel)) - nivel).abs() < 1e-5);
        }
        assert_eq!(g.x(0.0), 108.0, "a margem de 8 do viewBox");
    }

    /// A área útil inteira leva de −100 a +100; o resultado é inteiro e não
    /// passa dos limites.
    #[test]
    fn o_arrasto_vertical_vira_o_valor_da_regiao() {
        let arrasto = ArrastoDeRegiao {
            regiao: Regiao::Claros,
            valor_inicial: 10.0,
            y_inicial: 200.0,
        };
        assert_eq!(valor_do_arrasto(&arrasto, 200.0, 244.0), 10.0);
        assert_eq!(valor_do_arrasto(&arrasto, 200.0 - 61.0, 244.0), 60.0);
        assert_eq!(valor_do_arrasto(&arrasto, 900.0, 244.0), -100.0);
    }

    #[test]
    fn o_clique_pega_o_pino_mais_perto() {
        let g = Geometria::de(quadro());
        let neutro = Ajustes::default();
        // O pino dos médios fica no meio: 108 + 122.
        assert_eq!(pino_sob_o_ponteiro(&g, 232.0, &neutro), Some(1));
        assert_eq!(pino_sob_o_ponteiro(&g, 170.0, &neutro), Some(0));
        assert_eq!(pino_sob_o_ponteiro(&g, 200.0, &neutro), None);
    }

    #[test]
    fn o_histograma_vira_luminancia_com_o_pico_no_teto() {
        let lum = luminancia_do_histograma(&[(0.0, 0.0, 0.0), (0.3, 0.6, 0.9), (0.1, 0.1, 0.1)]);
        assert_eq!(lum.len(), 3);
        assert_eq!(lum[1], 1.0);
        assert!((lum[2] - 0.1 / 0.6).abs() < 1e-6);
        assert!(luminancia_do_histograma(&[(0.0, 0.0, 0.0)]).is_empty());
        // O branco estourado não achata o resto: o pico é medido no miolo.
        let estourado = luminancia_do_histograma(&[
            (0.0, 0.0, 0.0),
            (0.1, 0.1, 0.1),
            (0.05, 0.05, 0.05),
            (1.0, 1.0, 1.0),
        ]);
        assert_eq!(estourado[1], 1.0);
        assert_eq!(estourado[2], 0.5);
        assert_eq!(estourado[3], 1.0, "o extremo é cortado no teto");
    }
}
