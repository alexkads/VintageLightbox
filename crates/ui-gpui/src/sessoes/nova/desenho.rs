//! O desenho do assistente — `assistente.tsx`, `passos.tsx`, `etapa-*.tsx`,
//! `barra-da-importacao.tsx`, `resumo.tsx` e os campos de associação do site,
//! com os mesmos textos.

use std::sync::Arc;

use gpui::{
    div, img, prelude::*, px, relative, AnyElement, App, ClickEvent, Context, Div, Entity,
    FontWeight, Hsla, KeyDownEvent, RenderImage, SharedString, Stateful, Window,
};
use gpui_component::checkbox::Checkbox;
use gpui_component::input::{Input, InputState};
use gpui_component::progress::Progress;
use gpui_component::select::Select;
use gpui_component::slider::Slider;
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon};

use super::associacoes::{self as assoc};
use super::estado::{self, EstadoDaEtapa};
use super::receita::Grupo;
use super::tela::{
    Confirmacao, Fase, ItemDaBusca, NovaSessao, PedidoDaNova, SelecaoDaPasta, TipoDeBusca,
};
use crate::estilo;
use crate::recursos::Icone;
use crate::tema;

const ESMERALDA: u32 = 0x059669;
const VERMELHO: u32 = 0xdc2626;
const AMBAR: u32 = 0xf59e0b;

fn cor(hex: u32) -> Hsla {
    gpui::rgb(hex).into()
}

/// O rótulo de um campo, com `*` vermelho quando obrigatório.
fn rotulo(texto: &str, obrigatorio: bool) -> Div {
    h_flex()
        .gap(px(2.))
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .child(SharedString::from(texto.to_string()))
        .when(obrigatorio, |r| {
            r.child(div().text_color(cor(VERMELHO)).child("*"))
        })
}

/// `Campo` do site: rótulo em cima; embaixo, o erro ou a ajuda.
/// Um campo de texto que **ocupa a largura do campo** — a do `campo` acima.
///
/// 🚨 **Sem isto o `Input` fica do tamanho intrínseco dele** (uns 50 px), e o
/// operador vê uma caixinha quadrada onde devia haver uma linha inteira —
/// achado pelo dono em 2026-09-20, na etapa "Cliente e preço". Todo `Select`
/// deste assistente já levava `.w_full()`; os `Input` não levavam nenhum, e a
/// diferença não aparece em teste de método: é geometria, e só se vê na tela.
///
/// 🔑 **Existe para não haver o que esquecer**: o campo de texto do assistente
/// passa a nascer com a largura certa, em vez de depender de quem o escreve
/// lembrar do `.w_full()`.
fn entrada(estado: &Entity<InputState>) -> Input {
    Input::new(estado).w_full()
}

fn campo(
    texto: &str,
    obrigatorio: bool,
    controle: impl IntoElement,
    erro: Option<&str>,
    ajuda: Option<&str>,
    cx: &App,
) -> Div {
    let apagado = cx.theme().muted_foreground;
    v_flex()
        .w_full()
        .gap(px(8.))
        .child(rotulo(texto, obrigatorio))
        .child(controle)
        .map(|c| match (erro, ajuda) {
            (Some(erro), _) => c.child(
                div()
                    .text_sm()
                    .text_color(cor(VERMELHO))
                    .child(SharedString::from(erro.to_string())),
            ),
            (None, Some(ajuda)) => c.child(
                div()
                    .text_sm()
                    .text_color(apagado)
                    .child(SharedString::from(ajuda.to_string())),
            ),
            _ => c,
        })
}

fn alerta(
    icone: Icone,
    titulo: Option<String>,
    texto: impl IntoElement,
    tom: Option<u32>,
    cx: &App,
) -> Div {
    let tema = cx.theme();
    let (borda, fundo, cor_do_texto) = match tom {
        Some(hex) => (cor(hex).opacity(0.5), cor(hex).opacity(0.08), cor(hex)),
        None => (tema.border, tema.background, tema.foreground),
    };
    h_flex()
        .items_start()
        .gap(px(12.))
        .p(px(16.))
        .rounded(px(10.))
        .border_1()
        .border_color(borda)
        .bg(fundo)
        .child(
            Icon::new(icone)
                .size(px(16.))
                .text_color(cor_do_texto)
                .mt(px(2.)),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.))
                .gap(px(4.))
                .text_sm()
                .when_some(titulo, |c, t| {
                    c.child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cor_do_texto)
                            .child(t),
                    )
                })
                .child(texto),
        )
}

fn linha_apagada(texto: String, cx: &App) -> Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .truncate()
        .child(texto)
}

impl Render for NovaSessao {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.focar_pendente(window, cx);
        self.preparar_miniaturas();
        // As amostras pedidas neste quadro chegam por canal: alguém tem de
        // acordar para recolhê-las.
        //
        // 🚨 **A receita padrão entra na mesma condição.** Ela revela numa
        // thread e avisa por canal; sem alguém acordando, o trabalho acontecia
        // no disco e a tela não mudava — as miniaturas ficavam nas de antes e a
        // barra "Preset padrão" parava em 0/N.
        // 🚨 **E as miniaturas.** Quem as pede é `preparar_miniaturas`, logo
        // acima, no meio deste desenho — se a colheita já tinha parado, o
        // pedido sairia para a thread e a resposta ficaria no canal para
        // sempre: grade cinza, sem ninguém para recolher.
        if self.amostras.esperando()
            || self.miniaturas.esperando()
            || self.pedir_colheita_das_reveladas
            || self.portas.receita_padrao.progresso().andando()
        {
            self.acompanhar(window, cx);
        }
        let tema = cx.theme();
        let (fundo, borda) = (tema.background, tema.border);

        v_flex()
            .id("nova-sessao")
            .key_context("NovaSessao")
            .track_focus(&self.foco)
            .relative()
            .size_full()
            .bg(fundo)
            .on_key_down(cx.listener(|tela, evento: &KeyDownEvent, window, cx| {
                tela.tecla(evento, window, cx);
            }))
            .on_drag_move(cx.listener(
                |tela, _: &gpui::DragMoveEvent<gpui::ExternalPaths>, _, cx| {
                    tela.destacar(true, cx);
                },
            ))
            .on_drop(
                cx.listener(|tela, arrastados: &gpui::ExternalPaths, window, cx| {
                    let fotos = crate::sessoes::arquivos::so_as_fotos(arrastados.paths());
                    if fotos.is_empty() {
                        tela.destacar(false, cx);
                        tela.avisar(
                        "Formato não aceito. Envie RAW, JPEG, PNG, TIFF, WebP, HEIC, BMP ou GIF.",
                        true,
                    );
                        return;
                    }
                    tela.importar_arquivos(fotos, window, cx);
                }),
            )
            .child(self.cabecalho(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .items_start()
                    .child(self.passos(cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .min_w(px(0.))
                            .border_l_1()
                            .border_color(borda)
                            .child(self.corpo(window, cx))
                            .child(self.rodape(cx)),
                    ),
            )
            .when(self.arrastando, |t| t.child(self.sobreposicao(cx)))
            .when_some(self.selecao_da_pasta.as_ref(), |t, selecao| {
                t.child(self.dialogo_da_pasta(selecao, window, cx))
            })
            .when_some(self.confirmacao, |t, qual| t.child(self.dialogo(qual, cx)))
            .when(self.busca.is_some(), |t| t.child(self.modal_de_busca(cx)))
            .when_some(self.aviso.as_ref(), |t, aviso| {
                t.child(
                    div()
                        .absolute()
                        .bottom(px(88.))
                        .right(px(24.))
                        .max_w(px(420.))
                        .px(px(16.))
                        .py(px(12.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(if aviso.erro {
                            cor(VERMELHO).opacity(0.5)
                        } else {
                            borda
                        })
                        .bg(cx.theme().popover)
                        .shadow_lg()
                        .text_sm()
                        .text_color(if aviso.erro {
                            cor(VERMELHO)
                        } else {
                            cx.theme().foreground
                        })
                        .child(aviso.texto.clone()),
                )
            })
    }
}

impl NovaSessao {
    fn tecla(&mut self, evento: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let tecla = evento.keystroke.key.as_str();
        let m = &evento.keystroke.modifiers;
        if tecla.eq_ignore_ascii_case("a")
            && (m.platform || m.control)
            && self.selecao_da_pasta.is_some()
        {
            self.marcar_todas_da_pasta(true, cx);
            return;
        }
        if tecla.eq_ignore_ascii_case("d")
            && (m.platform || m.control)
            && self.selecao_da_pasta.is_some()
        {
            self.marcar_todas_da_pasta(false, cx);
            return;
        }
        if m.shift || m.alt || m.control || m.platform {
            return;
        }
        if tecla == "escape" {
            if self.busca.is_some() {
                self.fechar_busca(cx);
            } else if self.confirmacao.is_some() {
                self.cancelar_confirmacao(cx);
            } else if self.menu_da_origem.take().is_some() {
                cx.notify();
            }
            return;
        }
        // As setas e o Espaço do seletor de preset, só quando nenhum campo tem
        // o foco (o foco está na tela).
        if !self.foco.is_focused(window) {
            return;
        }
        match (self.etapa(), tecla) {
            (2, "left") => self.mover_foco_do_preset(-1, cx),
            (2, "right") => self.mover_foco_do_preset(1, cx),
            (2, "up") => self.mover_foco_do_preset(-3, cx),
            (2, "down") => self.mover_foco_do_preset(3, cx),
            (2, "home") => self.mover_foco_do_preset(-10_000, cx),
            (2, "end") => self.mover_foco_do_preset(10_000, cx),
            (2, "space") => self.marcar_preset_em_foco(cx),
            (_, "enter") => self.acao_principal(window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    // ── Cabeçalho ────────────────────────────────────────────────────────

    fn cabecalho(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (borda, apagado, texto) = (tema.border, tema.muted_foreground, tema.foreground);
        let (total, copiadas, previas) = self.numeros_da_copia();
        let receita = self.receita;
        let trabalhando = copiadas < total || previas < copiadas || receita.prontas < receita.total;
        let fracoes = {
            let mut partes = vec![
                if total > 0 {
                    copiadas as f32 / total as f32
                } else {
                    1.
                },
                if copiadas > 0 {
                    previas as f32 / copiadas as f32
                } else {
                    1.
                },
            ];
            if receita.total > 0 {
                partes.push(receita.prontas as f32 / receita.total as f32);
            }
            partes.iter().sum::<f32>() / partes.len() as f32
        };
        let pode_criar =
            estado::pode_criar(self.formulario()) && self.etapa() < estado::TOTAL_DE_ETAPAS;

        div()
            .relative()
            .flex_none()
            .child(
                h_flex()
                    .h(px(56.))
                    .px(px(16.))
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(borda)
                    .child(
                        estilo::botao_do_menu("nova-menu", cx).on_click(
                            cx.listener(|_, _, _, cx| cx.emit(PedidoDaNova::AlternarMenu)),
                        ),
                    )
                    .child(
                        div()
                            .id("nova-voltar")
                            .flex_none()
                            .cursor_pointer()
                            .text_color(apagado)
                            .hover(move |s| s.text_color(texto))
                            .child(Icon::new(Icone::ChevronLeft).size(px(20.)))
                            .on_click(cx.listener(|tela, _, _, cx| tela.voltar_as_sessoes(cx))),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Nova sessão"),
                    )
                    .when(total > 0, |c| {
                        let mut frase = format!(
                            "{} · cópia {copiadas}/{total} · prévias {previas}/{copiadas}",
                            estado::plural(total, "foto", "fotos")
                        );
                        if receita.total > 0 {
                            frase.push_str(&format!(
                                " · receita {}/{}",
                                receita.prontas, receita.total
                            ));
                        }
                        c.child(
                            h_flex()
                                .min_w(px(0.))
                                .gap(px(6.))
                                .text_xs()
                                .text_color(apagado)
                                .child(if trabalhando {
                                    Icon::new(Icone::LoaderCircle).size(px(14.))
                                } else {
                                    Icon::new(Icone::CircleCheck)
                                        .size(px(14.))
                                        .text_color(cor(ESMERALDA))
                                })
                                .child(div().truncate().child(frase)),
                        )
                    })
                    .child(div().flex_1())
                    .when(pode_criar, |c| {
                        let rotulo = self.rotulo_de_criar();
                        c.child(
                            estilo::botao_contorno("nova-criar-cabecalho", cx)
                                .h(px(32.))
                                .child(rotulo)
                                .on_click(
                                    cx.listener(|tela, _, window, cx| tela.criar(window, cx)),
                                ),
                        )
                    })
                    .when(self.guardado.is_none(), |c| {
                        c.child({
                            let ocupado = self.fase.is_some();
                            estilo::desligado(
                                estilo::botao_fantasma("nova-descartar", cx)
                                    .child(Icon::new(Icone::Trash2).size(px(16.)))
                                    .child("Descartar"),
                                ocupado,
                            )
                            .when(!ocupado, |b| {
                                b.on_click(cx.listener(|tela, _, _, cx| {
                                    tela.pedir_confirmacao(Confirmacao::Descartar, cx)
                                }))
                            })
                        })
                    }),
            )
            .when(total > 0, |c| {
                c.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .h(px(2.))
                        .w(relative(fracoes.clamp(0., 1.)))
                        .bg(if trabalhando {
                            cor(AMBAR)
                        } else {
                            cor(ESMERALDA)
                        }),
                )
            })
    }

    /// `(total, copiadas, prévias)` — as contas do resumo e da barra.
    fn numeros_da_copia(&self) -> (usize, usize, usize) {
        let gravadas = self.fotos.len();
        // As levas que esperam a vez já contam no total: o site publica
        // `total + arquivos.length` no instante em que a leva é solta, e sem
        // isto a barra fecharia em 12/12 com mais 20 fotos por copiar.
        let na_fila: usize = self.fila_de_levas.iter().map(Vec::len).sum();
        let (total, copiadas) = match self.importacao {
            Some(lote) => (
                gravadas + lote.total.saturating_sub(lote.falhas) + na_fila,
                gravadas + lote.feitas,
            ),
            None => (gravadas + na_fila, gravadas),
        };
        (total, copiadas, self.quantas_previas().min(copiadas))
    }

    // ── Passos ───────────────────────────────────────────────────────────

    fn passos(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let atual = self.etapa();
        v_flex()
            .id("nova-passos")
            .h_full()
            .w(px(256.))
            .flex_none()
            .p(px(12.))
            .gap(px(4.))
            .overflow_y_scroll()
            .children((1..=estado::TOTAL_DE_ETAPAS).map(|etapa| {
                let situacao = self.estado_da(etapa);
                let e_atual = etapa == atual;
                let tema = cx.theme();
                let legenda = estado::legenda(etapa, situacao, e_atual);
                let bolinha = {
                    let base = div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(28.))
                        .rounded_full()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM);
                    match situacao {
                        EstadoDaEtapa::Preenchida => base
                            .bg(cor(ESMERALDA))
                            .text_color(gpui::white())
                            .child(Icon::new(Icone::Check).size(px(16.))),
                        EstadoDaEtapa::Pendente => base
                            .border_1()
                            .border_color(cor(VERMELHO))
                            .text_color(cor(VERMELHO))
                            .child(Icon::new(Icone::CircleAlert).size(px(16.))),
                        EstadoDaEtapa::Vazia if e_atual => base
                            .bg(tema.foreground)
                            .text_color(tema.background)
                            .child(etapa.to_string()),
                        EstadoDaEtapa::Vazia => base
                            .border_1()
                            .border_color(tema.border)
                            .text_color(tema.muted_foreground)
                            .child(etapa.to_string()),
                    }
                };
                h_flex()
                    .id(SharedString::from(format!("nova-passo-{etapa}")))
                    .gap(px(12.))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .when(e_atual, |p| p.bg(tema.muted))
                    .hover(|p| p.bg(tema.accent))
                    .child(bolinha)
                    .child(
                        v_flex()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .text_sm()
                                    .when(e_atual, |t| t.font_weight(FontWeight::SEMIBOLD))
                                    .child(estado::titulo_da_etapa(etapa)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if situacao == EstadoDaEtapa::Pendente {
                                        cor(VERMELHO)
                                    } else {
                                        tema.muted_foreground
                                    })
                                    .child(legenda),
                            ),
                    )
                    .on_click(cx.listener(move |tela, _, window, cx| tela.ir(etapa, window, cx)))
            }))
    }

    // ── Corpo ────────────────────────────────────────────────────────────

    fn corpo(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let etapa = self.etapa();
        let apagado = cx.theme().muted_foreground;
        let conteudo: AnyElement = if self.procurando_rascunho && self.guardado.is_none() {
            div()
                .text_sm()
                .text_color(apagado)
                .child("Procurando rascunho nesta máquina…")
                .into_any_element()
        } else if self.guardado.is_some() {
            self.cartao_de_retomada(cx).into_any_element()
        } else {
            v_flex()
                .gap(px(24.))
                .children(self.alertas_do_topo(cx))
                .child(
                    h_flex()
                        .gap(px(6.))
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(div().text_color(apagado).child(format!("{etapa}.")))
                        .child(estado::titulo_da_etapa(etapa)),
                )
                .child(match etapa {
                    1 => self.etapa_aplicativo(cx).into_any_element(),
                    2 => self.etapa_fotos(window, cx).into_any_element(),
                    3 => self.etapa_cliente(cx).into_any_element(),
                    4 => self.etapa_agendamento(cx).into_any_element(),
                    5 => self.etapa_voucher(cx).into_any_element(),
                    6 => self.etapa_como_conheceu(cx).into_any_element(),
                    _ => self.etapa_compra(cx).into_any_element(),
                })
                .into_any_element()
        };
        div()
            .id("nova-corpo")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .track_scroll(&self.rolagem)
            .child(
                div()
                    .w_full()
                    .max_w(px(1024.))
                    .px(px(32.))
                    .py(px(32.))
                    .when(etapa == 2, |c| c.h_full().min_h(px(544.)))
                    .child(conteudo),
            )
    }

    fn alertas_do_topo(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut alertas = Vec::new();
        if !self.orfas.is_empty() {
            alertas.push(
                alerta(
                    Icone::TriangleAlert,
                    None,
                    h_flex()
                        .gap(px(12.))
                        .child(
                            div().flex_1().child(
                                "Há fotos de um rascunho antigo neste computador, sem sessão.",
                            ),
                        )
                        .child(
                            estilo::botao_contorno("nova-apagar-orfas", cx)
                                .child("Apagar")
                                .on_click(cx.listener(|tela, _, _, cx| {
                                    tela.pedir_confirmacao(Confirmacao::ApagarOrfas, cx)
                                })),
                        ),
                    Some(AMBAR),
                    cx,
                )
                .into_any_element(),
            );
        }
        if let Some((mensagem, etapa)) = self.erro.clone() {
            alertas.push(
                alerta(
                    Icone::CircleAlert,
                    None,
                    h_flex()
                        .gap(px(12.))
                        .child(div().flex_1().child(mensagem))
                        .when_some(etapa, |c, etapa| {
                            c.child(
                                estilo::botao_contorno("nova-ir-ao-erro", cx)
                                    .child(format!("Ir para a etapa {etapa}"))
                                    .on_click(cx.listener(move |tela, _, window, cx| {
                                        tela.ir(etapa, window, cx)
                                    })),
                            )
                        }),
                    Some(VERMELHO),
                    cx,
                )
                .into_any_element(),
            );
        }
        alertas
    }

    fn cartao_de_retomada(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(guardado) = self.guardado.as_ref() else {
            return div();
        };
        let f = &guardado.formulario;
        let mut descricao = if f.titulo.trim().is_empty() {
            "Sem título".to_string()
        } else {
            format!("\"{}\"", f.titulo.trim())
        };
        if self.fotos_do_guardado > 0 {
            descricao.push_str(&format!(
                " · {}",
                estado::plural(self.fotos_do_guardado, "foto importada", "fotos importadas")
            ));
        }
        if guardado.atualizado_em > 0 {
            if let Some(quando) = chrono::DateTime::from_timestamp(guardado.atualizado_em, 0) {
                let brasilia = chrono::FixedOffset::west_opt(3 * 3600).expect("fuso");
                descricao.push_str(&format!(
                    " · mexida por último em {}",
                    quando.with_timezone(&brasilia).format("%d/%m/%Y, %H:%M")
                ));
            }
        }
        let tema = cx.theme();
        v_flex()
            .max_w(px(640.))
            .gap(px(16.))
            .p(px(24.))
            .rounded(px(12.))
            .border_1()
            .border_color(tema.border)
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Há uma nova sessão pela metade nesta máquina"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(tema.muted_foreground)
                    .child(descricao),
            )
            .when(guardado.criada_id.is_some(), |c| {
                c.child(alerta(
                    Icone::TriangleAlert,
                    None,
                    "A sessão já foi criada no banco, mas as fotos não chegaram a passar para ela. \
                     Retome e clique em \"Criar sessão\" para terminar.",
                    Some(AMBAR),
                    cx,
                ))
            })
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(
                        estilo::botao_primario("nova-retomar", cx)
                            .child("Retomar")
                            .on_click(cx.listener(|tela, _, window, cx| tela.retomar(window, cx))),
                    )
                    .child(
                        estilo::botao_contorno("nova-descartar-guardado", cx)
                            .child("Descartar e começar outra")
                            .on_click(cx.listener(|tela, _, _, cx| {
                                tela.pedir_confirmacao(Confirmacao::DescartarGuardado, cx)
                            })),
                    ),
            )
    }

    // ── Etapa 1 ──────────────────────────────────────────────────────────

    fn etapa_aplicativo(&self, cx: &mut Context<Self>) -> impl IntoElement {
        alerta(
            Icone::CircleCheck,
            Some("Tudo certo: o painel está aberto como aplicativo.".into()),
            "A importação e os envios desta sessão ficam na janela própria dele. Siga para as fotos.",
            Some(ESMERALDA),
            cx,
        )
    }

    // ── Etapa 2 ──────────────────────────────────────────────────────────

    fn etapa_fotos(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = window;
        h_flex()
            .items_start()
            .gap(px(32.))
            .size_full()
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Fotos do ensaio"),
                    )
                    .child(self.area_das_fotos(cx)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(24.))
                    .child(self.seletor_de_preset(cx))
                    .child(self.seletor_de_proporcao(cx)),
            )
    }

    fn botao_da_origem(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        div()
            .relative()
            .child(
                estilo::botao_contorno("nova-origem", cx)
                    .child(Icon::new(Icone::HardDrive).size(px(16.)))
                    .child("Do cartão ou pasta…")
                    .on_click(
                        cx.listener(|tela, _, window, cx| tela.abrir_menu_da_origem(window, cx)),
                    ),
            )
            .when_some(self.menu_da_origem.as_ref(), |c, menu| {
                c.child(
                    v_flex()
                        .absolute()
                        // A área das fotos fica dentro de uma coluna com
                        // `overflow_hidden`. Abrir para baixo fazia o menu
                        // ser cortado pela borda inferior dessa etapa.
                        .bottom(px(36.))
                        .left_0()
                        .min_w(px(240.))
                        .p(px(4.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(tema.border)
                        .bg(tema.popover)
                        .shadow_lg()
                        .child(
                            div()
                                .px(px(8.))
                                .py(px(6.))
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tema.muted_foreground)
                                .child("Cartões conectados"),
                        )
                        .when(menu.procurando, |m| {
                            m.child(div().px(px(8.)).py(px(6.)).text_sm().child("Procurando…"))
                        })
                        .when(!menu.procurando && menu.cartoes.is_empty(), |m| {
                            m.child(
                                div()
                                    .px(px(8.))
                                    .py(px(6.))
                                    .text_sm()
                                    .text_color(tema.muted_foreground)
                                    .child("Nenhum cartão encontrado"),
                            )
                        })
                        .children(menu.cartoes.iter().enumerate().map(|(i, (nome, caminho))| {
                            let caminho = caminho.clone();
                            h_flex()
                                .id(SharedString::from(format!("nova-cartao-{i}")))
                                .gap(px(8.))
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(px(4.))
                                .text_sm()
                                .cursor_pointer()
                                .hover(|h| h.bg(tema.accent))
                                .child(Icon::new(Icone::HardDrive).size(px(16.)))
                                .child(nome.clone())
                                .on_click(cx.listener(move |tela, _, window, cx| {
                                    cx.stop_propagation();
                                    tela.ler_cartao(caminho.clone(), window, cx)
                                }))
                        }))
                        .child(div().my(px(4.)).h(px(1.)).bg(tema.border))
                        .child(
                            h_flex()
                                .id("nova-escolher-pasta")
                                .gap(px(8.))
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(px(4.))
                                .text_sm()
                                .cursor_pointer()
                                .hover(|h| h.bg(tema.accent))
                                .child(Icon::new(Icone::FolderInput).size(px(16.)))
                                .child("Escolher pasta…")
                                .on_click(cx.listener(|tela, _, window, cx| {
                                    cx.stop_propagation();
                                    tela.escolher_pasta(window, cx)
                                })),
                        )
                        .child(
                            h_flex()
                                .id("nova-escolher-fotos-do-menu")
                                .gap(px(8.))
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(px(4.))
                                .text_sm()
                                .cursor_pointer()
                                .hover(|h| h.bg(tema.accent))
                                .child(Icon::new(Icone::ImagePlus).size(px(16.)))
                                .child("Escolher fotos…")
                                .on_click(cx.listener(|tela, _, window, cx| {
                                    cx.stop_propagation();
                                    tela.escolher_fotos(window, cx)
                                })),
                        ),
                )
            })
    }

    fn area_das_fotos(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let (total, copiadas, previas) = self.numeros_da_copia();
        if total == 0 {
            return v_flex()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .p(px(32.))
                .min_h(px(320.))
                .rounded(px(10.))
                .border_1()
                .border_dashed()
                .border_color(tema.border)
                .child(
                    div()
                        .p(px(8.))
                        .rounded(px(8.))
                        .bg(tema.muted)
                        .child(Icon::new(Icone::ImagePlus).size(px(24.))),
                )
                .child(div().text_lg().font_weight(FontWeight::MEDIUM).child("Nenhuma foto ainda"))
                .child(
                    div()
                        .max_w(px(360.))
                        .text_sm()
                        .text_center()
                        .text_color(tema.muted_foreground)
                        .child(
                            "Arraste a pasta do Lightroom para qualquer lugar desta tela, ou escolha \
                             os arquivos. As fotos ficam neste computador e sobem quando forem \
                             classificadas, dentro da sessão.",
                        ),
                )
                .child(
                    h_flex()
                        .gap(px(8.))
                        .child(
                            estilo::botao_primario("nova-escolher-fotos", cx)
                                .child("Escolher fotos")
                                .on_click(cx.listener(|tela, _, window, cx| {
                                    tela.escolher_fotos(window, cx)
                                })),
                        )
                        .child(self.botao_da_origem(cx)),
                )
                .into_any_element();
        }

        let medidor = |nome: &'static str, feitos: usize, de: usize| {
            h_flex()
                .gap(px(12.))
                .text_sm()
                .child(
                    div()
                        .w(px(110.))
                        .text_color(tema.muted_foreground)
                        .child(nome),
                )
                .child(
                    div().flex_1().child(
                        Progress::new()
                            .value(if de == 0 {
                                0.
                            } else {
                                feitos as f32 * 100. / de as f32
                            })
                            .h(px(6.)),
                    ),
                )
                .child(
                    div()
                        .w(px(56.))
                        .text_right()
                        .child(format!("{feitos}/{de}")),
                )
        };
        let copiando = copiadas < total;
        let lote = self.receita;

        v_flex()
            .gap(px(12.))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(px(12.))
                    .p(px(12.))
                    .rounded(px(10.))
                    .border_1()
                    .border_dashed()
                    .border_color(tema.border)
                    .child(
                        Icon::new(Icone::ImagePlus)
                            .size(px(18.))
                            .text_color(tema.muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(160.))
                            .text_sm()
                            .text_color(tema.muted_foreground)
                            .child("Arraste mais fotos para qualquer lugar desta tela, ou"),
                    )
                    .child(
                        estilo::botao_contorno("nova-mais-fotos", cx)
                            .child("Adicionar mais fotos")
                            .on_click(
                                cx.listener(|tela, _, window, cx| tela.escolher_fotos(window, cx)),
                            ),
                    )
                    .child(self.botao_da_origem(cx)),
            )
            .child(
                v_flex()
                    .gap(px(8.))
                    .p(px(12.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(tema.border)
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(if copiando || previas < copiadas {
                                Icon::new(Icone::LoaderCircle).size(px(14.))
                            } else {
                                Icon::new(Icone::CircleCheck)
                                    .size(px(14.))
                                    .text_color(cor(ESMERALDA))
                            })
                            .child(estado::plural(total, "foto", "fotos")),
                    )
                    .child(medidor("Cópia local", copiadas, total))
                    .child(medidor("Prévias", previas, copiadas))
                    .when(lote.total > 0, |c| {
                        c.child(medidor("Preset padrão", lote.prontas, lote.total))
                    })
                    .when(copiando, |c| {
                        c.child(linha_apagada(
                            "Não feche o app enquanto a cópia local anda: até terminar, as fotos \
                             ainda dependem dos arquivos no disco."
                                .into(),
                            cx,
                        ))
                    })
                    .when_some(self.falhas_da_copia.clone(), |c, (falhas, erro)| {
                        c.child(div().text_xs().text_color(cor(VERMELHO)).child(format!(
                            "{} não foram copiadas: {erro}",
                            estado::plural(falhas, "foto", "fotos")
                        )))
                    }),
            )
            .child(self.miniaturas_das_fotos(cx))
            .into_any_element()
    }

    fn miniaturas_das_fotos(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let com_previa: Vec<(String, Arc<RenderImage>)> = self
            .fotos
            .iter()
            .filter_map(|f| {
                self.miniaturas
                    .obter(&f.id)
                    .map(|m| (f.name.clone(), m.clone()))
            })
            .take(60)
            .collect();
        let sem_previa = self.fotos.len().saturating_sub(
            self.fotos
                .iter()
                .filter(|f| self.miniaturas.tem(&f.id))
                .count(),
        );
        let alem = self.fotos.len().saturating_sub(60);
        v_flex()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .children(com_previa.into_iter().map(|(nome, imagem)| {
                        div()
                            .size(px(72.))
                            .rounded(px(6.))
                            .overflow_hidden()
                            .bg(tema.muted)
                            .child(img(imagem).size_full().object_fit(gpui::ObjectFit::Cover))
                            .id(SharedString::from(format!("nova-miniatura-{nome}")))
                            .tooltip(move |window, cx| {
                                gpui_component::tooltip::Tooltip::new(nome.clone())
                                    .build(window, cx)
                            })
                    })),
            )
            .when(alem > 0, |c| {
                c.child(linha_apagada(
                    format!("e mais {alem} — a sessão mostra todas."),
                    cx,
                ))
            })
            .when(alem == 0 && sem_previa > 0, |c| {
                c.child(linha_apagada(format!("{sem_previa} ainda sem prévia."), cx))
            })
    }

    fn seletor_de_preset(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let escolhido = self.formulario().preset_id.clone();
        let proporcao = receita_da_tela(self.formulario().proporcao.as_deref());
        let cartao = |id: Option<String>,
                      nome: String,
                      indice: usize,
                      tela: &mut NovaSessao,
                      cx: &mut Context<NovaSessao>| {
            let tema = cx.theme();
            let marcado = escolhido == id;
            let em_foco = tela.foco_do_preset == indice && tela.teclado_no_preset;
            let amostra = tela.amostra(id.as_deref());
            let chave = id.clone().unwrap_or_else(|| "nenhum".into());
            v_flex()
                .id(SharedString::from(format!("nova-preset-{chave}")))
                .relative()
                .w(px(112.))
                .p(px(6.))
                .gap(px(6.))
                .rounded(px(8.))
                .border_1()
                .border_color(if marcado || em_foco {
                    tema.primary
                } else {
                    tema.border
                })
                .when(marcado, |c| c.border_2())
                .cursor_pointer()
                .hover(|c| c.bg(tema.accent))
                .child(
                    div()
                        .w_full()
                        .h(px(100. / proporcao.max(0.4)))
                        .max_h(px(160.))
                        .rounded(px(4.))
                        .overflow_hidden()
                        .bg(tema.muted)
                        .when_some(amostra, |c, imagem| {
                            c.child(img(imagem).size_full().object_fit(gpui::ObjectFit::Cover))
                        }),
                )
                .when(marcado, |c| {
                    c.child(
                        div()
                            .absolute()
                            .top(px(12.))
                            .right(px(12.))
                            .size(px(20.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(tema.primary)
                            .text_color(tema.primary_foreground)
                            .child(Icon::new(Icone::Check).size(px(12.))),
                    )
                })
                .child(div().text_xs().line_clamp(2).child(nome))
                .on_click(cx.listener(move |tela, _, window, cx| {
                    tela.foco_do_preset = indice;
                    window.focus(&tela.foco);
                    tela.escolher_preset(id.clone(), cx);
                }))
        };

        let mut indice = 0;
        let nenhum = cartao(None, "Nenhum".into(), indice, self, cx);
        indice += 1;
        let fora_da_lista = escolhido
            .as_deref()
            .filter(|id| self.presets.iter().all(|p| p.id != *id))
            .is_some();
        let mut do_sistema = Vec::new();
        let mut minhas = Vec::new();
        let lista: Vec<(String, String, Grupo)> = self
            .presets
            .iter()
            .map(|p| (p.id.clone(), p.nome.clone(), p.grupo))
            .collect();
        for (id, nome, grupo) in lista {
            let c = cartao(Some(id), nome, indice, self, cx);
            indice += 1;
            match grupo {
                Grupo::Sistema => do_sistema.push(c),
                Grupo::Minhas => minhas.push(c),
            }
        }
        let grade = |cartoes: Vec<Stateful<Div>>| {
            div()
                .w_full()
                .flex()
                .flex_wrap()
                .gap(px(8.))
                .children(cartoes)
        };
        let secao = |titulo: &'static str| {
            div()
                .mt(px(8.))
                .mb(px(4.))
                .text_xs()
                .text_color(tema.muted_foreground)
                .child(titulo)
        };

        campo(
            "Preset padrão",
            false,
            v_flex()
                .id("sessao-preset")
                .w_full()
                .max_h(px(420.))
                .overflow_y_scroll()
                .p(px(8.))
                .rounded(px(8.))
                .border_1()
                .border_color(tema.border)
                .child({
                    let mut primeira = vec![nenhum];
                    if fora_da_lista {
                        primeira.push(
                            div()
                                .id("nova-preset-fora")
                                .w(px(112.))
                                .p(px(6.))
                                .rounded(px(8.))
                                .border_2()
                                .border_color(tema.primary)
                                .text_xs()
                                .child("Preset fora da lista"),
                        );
                    }
                    grade(primeira)
                })
                .when(!do_sistema.is_empty(), |c| {
                    c.child(secao("Do sistema")).child(grade(do_sistema))
                })
                .when(!minhas.is_empty(), |c| {
                    c.child(secao("Minhas")).child(grade(minhas))
                }),
            None,
            Some("Aplicado sozinho às fotos. Setas percorrem, Espaço escolhe."),
            cx,
        )
    }

    fn seletor_de_proporcao(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let atual = self.formulario().proporcao.clone();
        let opcoes: Vec<(Option<String>, &str)> = std::iter::once((None, "Sem corte"))
            .chain(
                estado::PROPORCOES_PADRAO
                    .iter()
                    .map(|p| (Some(p.to_string()), estado::rotulo_da_proporcao(p))),
            )
            .collect();
        campo(
            "Proporção do corte",
            false,
            div()
                .id("sessao-proporcao")
                .grid()
                .grid_cols(4)
                .gap(px(8.))
                .children(opcoes.into_iter().map(|(valor, texto)| {
                    let marcado = atual == valor;
                    let chave = valor.clone().unwrap_or_else(|| "sem".into());
                    div()
                        .id(SharedString::from(format!("nova-proporcao-{chave}")))
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(36.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(tema.input)
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .map(|b| {
                            if marcado {
                                b.bg(tema.primary).text_color(tema.primary_foreground)
                            } else {
                                b.hover(|h| h.bg(tema.accent))
                            }
                        })
                        .child(texto.to_string())
                        .on_click(cx.listener(move |tela, _, _, cx| {
                            tela.escolher_proporcao(valor.clone(), cx)
                        }))
                })),
            None,
            Some("Corte centralizado; dá para ajustar foto a foto na revelação."),
            cx,
        )
    }

    // ── Etapa 3 ──────────────────────────────────────────────────────────

    fn erro_de(&self, mensagem: &'static str) -> Option<&'static str> {
        (self.tentou && estado::pendencias_da_etapa(3, self.formulario()).contains(&mensagem))
            .then_some(mensagem)
    }

    fn etapa_cliente(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let apagado = cx.theme().muted_foreground;
        v_flex()
            .gap(px(20.))
            .child(campo(
                "Título",
                true,
                // `debug_selector` para o teste poder clicar onde o dedo clica:
                // o caminho clique → foco → tecla é o que nenhum teste pegava.
                // O `w_full` é de `entrada` — sem ele o campo nascia com 50 px —,
                // e o `div` em volta precisa dele também, senão encolhe junto.
                div()
                    .w_full()
                    .debug_selector(|| "nova-titulo".into())
                    .child(entrada(&self.titulo)),
                self.erro_de(estado::FALTA_TITULO),
                None,
                cx,
            ))
            .child(
                h_flex()
                    .items_start()
                    .gap(px(16.))
                    .child(
                        v_flex().flex_1().min_w(px(0.)).child(campo(
                            "Preço por foto",
                            true,
                            Select::new(&self.escolha_do_produto)
                                .placeholder("Escolha…")
                                .w_full(),
                            self.erro_de(estado::FALTA_PRECO),
                            None,
                            cx,
                        )),
                    )
                    .child(
                        v_flex().flex_1().min_w(px(0.)).child(campo(
                            "Estúdio",
                            true,
                            Select::new(&self.escolha_do_estudio)
                                .placeholder("Escolha…")
                                .w_full(),
                            self.erro_de(estado::FALTA_ESTUDIO),
                            None,
                            cx,
                        )),
                    ),
            )
            .child(
                h_flex()
                    .items_start()
                    .gap(px(16.))
                    .child(v_flex().flex_1().min_w(px(0.)).child(campo(
                        "E-mail do cliente",
                        false,
                        entrada(&self.email),
                        self.erro_de(estado::EMAIL_INCOMPLETO),
                        Some("Prefira o e-mail: é por ele que o link vai."),
                        cx,
                    )))
                    .child(v_flex().w(px(260.)).flex_none().child(campo(
                        "WhatsApp",
                        false,
                        entrada(&self.whatsapp),
                        None,
                        Some("Opcional agora."),
                        cx,
                    ))),
            )
            .child(div().text_sm().text_color(apagado).child(
                "O contato pode ficar para o fim da sessão: sem ele a sessão é criada, e o \
                 \"Copiar link\" e o \"Avisar\" pedem o e-mail antes de sair. O agendamento, o \
                 voucher e a compra antecipada completam os campos que ficarem vazios — o que já \
                 foi digitado não muda.",
            ))
    }

    // ── Etapas 4 a 7 ─────────────────────────────────────────────────────

    /// `CampoDeAssociacao` do site.
    #[allow(clippy::too_many_arguments)]
    fn campo_de_associacao(
        &self,
        tipo: TipoDeBusca,
        icone: Icone,
        vazio: &'static str,
        preenchido: Option<(String, Option<String>, Vec<String>)>,
        ajuda: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tema = cx.theme();
        let chave = format!("{tipo:?}").to_lowercase();
        let buscar = estilo::botao_contorno(SharedString::from(format!("nova-buscar-{chave}")), cx)
            .on_click(cx.listener(move |tela, _, window, cx| tela.abrir_busca(tipo, window, cx)));
        let caixa = match preenchido {
            None => h_flex()
                .gap(px(12.))
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_dashed()
                .border_color(tema.border)
                .child(
                    Icon::new(icone)
                        .size(px(18.))
                        .text_color(tema.muted_foreground),
                )
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .text_color(tema.muted_foreground)
                        .child(vazio),
                )
                .child(
                    buscar
                        .child(Icon::new(Icone::Search).size(px(16.)))
                        .child("Buscar…"),
                ),
            Some((titulo, selo, linhas)) => h_flex()
                .items_start()
                .gap(px(12.))
                .p(px(12.))
                .rounded(px(8.))
                .border_1()
                .border_color(tema.border)
                .child(Icon::new(icone).size(px(18.)).mt(px(2.)))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(0.))
                        .gap(px(2.))
                        .child(
                            h_flex()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .truncate()
                                        .child(titulo),
                                )
                                .when_some(selo, |c, selo| {
                                    c.child(estilo::selo_contorno(cx).child(selo))
                                }),
                        )
                        .children(
                            linhas
                                .into_iter()
                                .filter(|l| !l.is_empty())
                                .map(|l| linha_apagada(l, cx)),
                        ),
                )
                .child(buscar.child("Trocar"))
                .child(
                    estilo::botao_fantasma(SharedString::from(format!("nova-remover-{chave}")), cx)
                        .child("Remover")
                        .on_click(cx.listener(move |tela, _, _, cx| tela.remover(tipo, cx))),
                ),
        };
        v_flex()
            .gap(px(8.))
            .child(caixa)
            .when_some(ajuda, |c, ajuda| {
                c.child(
                    div()
                        .text_sm()
                        .text_color(tema.muted_foreground)
                        .child(ajuda),
                )
            })
    }

    fn etapa_agendamento(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let preenchido = self.formulario().agendamento.as_ref().map(|a| {
            (
                a.nome.clone().unwrap_or_else(|| "Sem nome".into()),
                Some(assoc::rotulo_do_status_do_agendamento(&a.status)),
                vec![
                    assoc::juntar(&[a.quando.as_deref(), a.estudio_nome.as_deref()]),
                    assoc::juntar(&[a.whatsapp.as_deref(), a.email.as_deref()]),
                ],
            )
        });
        self.campo_de_associacao(
            TipoDeBusca::Agendamento,
            Icone::CalendarCheck,
            "Nenhum agendamento associado.",
            preenchido,
            Some(
                "Título, contato e estúdio que estiverem vazios são completados pelo agendamento.",
            ),
            cx,
        )
    }

    fn etapa_voucher(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let preenchido = self.formulario().voucher.as_ref().map(|v| {
            (
                format!(
                    "Nº {} · {}",
                    v.numero,
                    v.nome.clone().unwrap_or_else(|| "Sem nome".into())
                ),
                Some(match &v.utilizado_em {
                    Some(quando) => format!("usado em {quando}"),
                    None => "não usado".into(),
                }),
                vec![
                    assoc::juntar(&[v.parceiro.as_deref(), v.agendamento.as_deref()]),
                    assoc::juntar(&[v.whatsapp.as_deref(), v.email.as_deref()]),
                ],
            )
        });
        self.campo_de_associacao(
            TipoDeBusca::Voucher,
            Icone::Ticket,
            "Nenhum voucher associado.",
            preenchido,
            Some("O contato do voucher entra só nos campos vazios."),
            cx,
        )
    }

    fn etapa_compra(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let preenchido = self.formulario().compra.as_ref().map(|c| {
            (
                format!(
                    "{} · {}",
                    c.comprador_nome
                        .clone()
                        .unwrap_or_else(|| "Comprador sem nome".into()),
                    c.total
                        .map(biblioteca_core::dinheiro::formatar)
                        .unwrap_or_default()
                ),
                None,
                vec![
                    assoc::juntar(&[
                        c.pago_em
                            .as_ref()
                            .map(|p| format!("paga em {p}"))
                            .as_deref(),
                        Some(assoc::resumo_dos_itens(&c.itens, 3).as_str()),
                    ]),
                    assoc::juntar(&[c.comprador_email.as_deref(), c.whatsapp.as_deref()]),
                ],
            )
        });
        let pendencias = estado::pendencias_para_criar(self.formulario());
        v_flex()
            .gap(px(24.))
            .child(self.campo_de_associacao(
                TipoDeBusca::Compra,
                Icone::ShoppingBag,
                "Nenhuma compra antecipada associada.",
                preenchido,
                Some(
                    "A compra feita pelo site antes do ensaio. O contato do comprador entra só nos \
                     campos vazios.",
                ),
                cx,
            ))
            .when(!pendencias.is_empty(), |c| {
                c.child(alerta(
                    Icone::CircleAlert,
                    Some("Antes de criar a sessão, falta:".into()),
                    v_flex().gap(px(2.)).children(pendencias.into_iter().enumerate().map(
                        |(i, (etapa, mensagem))| {
                            div()
                                .id(SharedString::from(format!("nova-falta-{i}")))
                                .cursor_pointer()
                                .underline()
                                .child(format!("{mensagem} (etapa {etapa})"))
                                .on_click(cx.listener(move |tela, _, window, cx| {
                                    tela.tentou = true;
                                    tela.ir(etapa, window, cx);
                                }))
                        },
                    )),
                    Some(VERMELHO),
                    cx,
                ))
            })
            .child(self.resumo(cx))
    }

    fn etapa_como_conheceu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let atual = self.formulario().como_conheceu.clone();
        let parceiro = self.formulario().parceiro.clone();
        v_flex()
            .gap(px(20.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .children(assoc::COMO_CONHECEU.iter().map(|(valor, texto)| {
                        let marcado = atual.as_deref() == Some(*valor);
                        div()
                            .id(SharedString::from(format!("nova-conheceu-{valor}")))
                            .h(px(32.))
                            .px(px(14.))
                            .flex()
                            .items_center()
                            .rounded_full()
                            .border_1()
                            .border_color(tema.input)
                            .text_sm()
                            .cursor_pointer()
                            .map(|b| {
                                if marcado {
                                    b.bg(tema.primary).text_color(tema.primary_foreground)
                                } else {
                                    b.hover(|h| h.bg(tema.accent))
                                }
                            })
                            .child(*texto)
                            .on_click(cx.listener(move |tela, _, window, cx| {
                                tela.escolher_como_conheceu(valor, window, cx)
                            }))
                    })),
            )
            .when(atual.as_deref() == Some("parceiro"), |c| {
                let preenchido = parceiro.as_ref().map(|p| {
                    (
                        p.nome.clone(),
                        Some(assoc::rotulo_do_tipo_de_parceiro(&p.tipo)),
                        vec![assoc::juntar(&[p.whatsapp.as_deref(), p.email.as_deref()])],
                    )
                });
                c.child(
                    v_flex()
                        .gap(px(8.))
                        .child(rotulo("Qual parceiro?", false))
                        .child(self.campo_de_associacao(
                            TipoDeBusca::Parceiro,
                            Icone::Handshake,
                            "Nenhum parceiro escolhido.",
                            preenchido,
                            None,
                            cx,
                        ))
                        .when(parceiro.is_none(), |c| {
                            c.child(self.cadastro_de_parceiro(false, cx))
                        })
                        .child(
                            div()
                                .text_sm()
                                .text_color(cor(VERMELHO))
                                .when(parceiro.is_some(), |d| d.invisible())
                                .child(estado::FALTA_PARCEIRO),
                        ),
                )
            })
            .when(atual.as_deref() == Some("outro"), |c| {
                c.child(campo(
                    "Como foi?",
                    false,
                    entrada(&self.detalhe),
                    None,
                    None,
                    cx,
                ))
            })
    }

    fn cadastro_de_parceiro(&self, no_modal: bool, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let observacao = "Cadastre apenas se for um parceiro recorrente";
        let Some(cadastro) = self.cadastro.as_ref() else {
            let texto_da_busca = self
                .busca
                .as_ref()
                .map(|b| b.campo.read(cx).value().trim().to_string())
                .filter(|t| !t.is_empty());
            let rotulo = match (&texto_da_busca, no_modal) {
                (Some(t), true) => format!("Cadastrar \"{t}\""),
                _ => "Cadastrar novo parceiro".into(),
            };
            return h_flex()
                .gap(px(12.))
                .child(
                    estilo::botao_contorno("nova-abrir-cadastro", cx)
                        .child(Icon::new(Icone::Plus).size(px(16.)))
                        .child(rotulo)
                        .on_click(cx.listener(move |tela, _, window, cx| {
                            tela.abrir_cadastro(texto_da_busca.clone(), window, cx)
                        })),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(tema.muted_foreground)
                        .child(observacao),
                )
                .into_any_element();
        };
        let pronto = !cadastro.nome.read(cx).value().trim().is_empty()
            && cadastro.tipo_escolhido.is_some()
            && !cadastro.enviando;
        v_flex()
            .gap(px(12.))
            .p(px(16.))
            .rounded(px(8.))
            .border_1()
            .border_color(tema.border)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Novo parceiro"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(tema.muted_foreground)
                    .child(observacao),
            )
            .child(
                h_flex()
                    .items_start()
                    .gap(px(12.))
                    .child(v_flex().flex_1().min_w(px(0.)).child(campo(
                        "Nome",
                        false,
                        entrada(&cadastro.nome),
                        None,
                        None,
                        cx,
                    )))
                    .child(v_flex().flex_1().min_w(px(0.)).child(campo(
                        "Tipo",
                        false,
                        Select::new(&cadastro.tipo).placeholder("Escolha…").w_full(),
                        None,
                        None,
                        cx,
                    ))),
            )
            .child(
                h_flex()
                    .items_start()
                    .gap(px(12.))
                    .child(v_flex().flex_1().min_w(px(0.)).child(campo(
                        "WhatsApp (opcional)",
                        false,
                        entrada(&cadastro.whatsapp),
                        None,
                        None,
                        cx,
                    )))
                    .child(v_flex().flex_1().min_w(px(0.)).child(campo(
                        "E-mail (opcional)",
                        false,
                        entrada(&cadastro.email),
                        None,
                        None,
                        cx,
                    ))),
            )
            .when_some(cadastro.erro.clone(), |c, erro| {
                c.child(
                    h_flex()
                        .gap(px(8.))
                        .child(div().text_sm().text_color(cor(VERMELHO)).child(erro))
                        .when_some(cadastro.existente.clone(), |c, p| {
                            c.child(
                                estilo::botao_contorno("nova-usar-existente", cx)
                                    .child(format!(
                                        "Usar \"{}\" ({})",
                                        p.nome,
                                        assoc::rotulo_do_tipo_de_parceiro(&p.tipo)
                                    ))
                                    .on_click(cx.listener(|tela, _, window, cx| {
                                        tela.usar_existente(window, cx)
                                    })),
                            )
                        }),
                )
            })
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(
                        estilo::desligado(
                            estilo::botao_primario("nova-cadastrar-parceiro", cx).child(
                                if cadastro.enviando {
                                    "Cadastrando…"
                                } else {
                                    "Cadastrar e usar"
                                },
                            ),
                            !pronto,
                        )
                        .when(pronto, |b| {
                            b.on_click(cx.listener(|tela, _, window, cx| {
                                tela.cadastrar_parceiro(window, cx)
                            }))
                        }),
                    )
                    .child(
                        estilo::botao_fantasma("nova-cancelar-cadastro", cx)
                            .child("Cancelar")
                            .on_click(cx.listener(|tela, _, _, cx| tela.fechar_cadastro(cx))),
                    ),
            )
            .into_any_element()
    }

    fn resumo(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let f = self.formulario();
        let ou_traco = |v: String| {
            if v.trim().is_empty() {
                "—".to_string()
            } else {
                v
            }
        };
        let produto = self
            .produtos
            .iter()
            .find(|p| p.id == f.produto_id)
            .map(|p| p.nome.clone())
            .unwrap_or_default();
        let estudio = self
            .estudios
            .iter()
            .find(|e| e.id == f.estudio_id)
            .map(|e| e.nome.clone())
            .unwrap_or_default();
        let preset = super::receita::nome_do_preset(&self.presets, f.preset_id.as_deref());
        let proporcao = f.proporcao.as_deref().map(estado::rotulo_da_proporcao);
        let receita = match (preset, proporcao) {
            (None, None) => String::new(),
            (p, c) => assoc::juntar(&[p.as_deref(), c.map(|c| format!("corte {c}")).as_deref()]),
        };
        let fotos = self.quantas_fotos();
        let conheceu = f
            .como_conheceu
            .as_deref()
            .map(|c| {
                let mut t = assoc::rotulo_de_como_conheceu(c);
                if let Some(p) = &f.parceiro {
                    t.push_str(&format!(" · {}", p.nome));
                }
                if c == "outro" && !f.como_conheceu_detalhe.trim().is_empty() {
                    t.push_str(&format!(" · {}", f.como_conheceu_detalhe.trim()));
                }
                t
            })
            .unwrap_or_default();
        let linhas: Vec<(&str, usize, String)> = vec![
            ("Título", 3, f.titulo.trim().to_string()),
            (
                "Contato",
                3,
                assoc::juntar(&[Some(f.email.as_str()), Some(f.whatsapp.as_str())]),
            ),
            (
                "Fotos",
                2,
                if fotos > 0 {
                    format!(
                        "{} nesta máquina",
                        estado::plural(fotos, "importada", "importadas")
                    )
                } else {
                    String::new()
                },
            ),
            ("Preço por foto", 3, produto),
            ("Estúdio", 3, estudio),
            ("Preset padrão", 2, receita),
            (
                "Agendamento",
                4,
                f.agendamento
                    .as_ref()
                    .map(|a| assoc::juntar(&[a.nome.as_deref(), a.quando.as_deref()]))
                    .unwrap_or_default(),
            ),
            (
                "Voucher",
                5,
                f.voucher
                    .as_ref()
                    .map(|v| format!("Nº {}", v.numero))
                    .unwrap_or_default(),
            ),
            ("Como conheceu", 6, conheceu),
            (
                "Compra antecipada",
                7,
                f.compra
                    .as_ref()
                    .map(|c| {
                        assoc::juntar(&[
                            c.comprador_nome.as_deref(),
                            c.total.map(biblioteca_core::dinheiro::formatar).as_deref(),
                        ])
                    })
                    .unwrap_or_default(),
            ),
        ];
        v_flex()
            .p(px(20.))
            .gap(px(10.))
            .rounded(px(12.))
            .border_1()
            .border_color(tema.border)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("O que a sessão vai levar"),
            )
            .children(
                linhas
                    .into_iter()
                    .enumerate()
                    .map(|(i, (nome, etapa, valor))| {
                        h_flex()
                            .items_start()
                            .gap(px(16.))
                            .text_sm()
                            .child(
                                div()
                                    .id(SharedString::from(format!("nova-resumo-{i}")))
                                    .w(px(150.))
                                    .flex_none()
                                    .text_color(tema.muted_foreground)
                                    .cursor_pointer()
                                    .hover(|h| h.underline())
                                    .child(nome)
                                    .on_click(cx.listener(move |tela, _, window, cx| {
                                        tela.ir(etapa, window, cx)
                                    })),
                            )
                            .child(div().flex_1().min_w(px(0.)).child(ou_traco(valor)))
                    }),
            )
    }

    // ── Rodapé ───────────────────────────────────────────────────────────

    fn rodape(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let etapa = self.etapa();
        let ultima = etapa >= estado::TOTAL_DE_ETAPAS;
        let pode_pular = estado::etapa_opcional(etapa)
            && !ultima
            && self.estado_da(etapa) == EstadoDaEtapa::Vazia;
        let escondido = self.guardado.is_some();
        h_flex()
            .flex_none()
            .h(px(64.))
            .px(px(24.))
            .gap(px(12.))
            .border_t_1()
            .border_color(tema.border)
            .when(escondido, |r| r.invisible())
            .child(div().w(px(160.)).when(etapa > 1, |c| {
                c.child(
                    estilo::botao_fantasma("nova-voltar-etapa", cx)
                        .child(Icon::new(Icone::ArrowLeft).size(px(16.)))
                        .child("Voltar")
                        .on_click(cx.listener(|tela, _, window, cx| tela.voltar_etapa(window, cx))),
                )
            }))
            .child(
                h_flex()
                    .flex_1()
                    .justify_center()
                    .gap(px(4.))
                    .text_sm()
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("Etapa {etapa} de {}", estado::TOTAL_DE_ETAPAS)),
                    )
                    .child(
                        div()
                            .text_color(tema.muted_foreground)
                            .child(format!("· {}", estado::titulo_da_etapa(etapa))),
                    ),
            )
            .child(
                h_flex()
                    .w(px(260.))
                    .justify_end()
                    .gap(px(8.))
                    .when(pode_pular, |c| {
                        c.child(
                            estilo::botao_fantasma("nova-pular", cx)
                                .text_color(tema.muted_foreground)
                                .child("Pular")
                                .on_click(cx.listener(move |tela, _, window, cx| {
                                    tela.ir(etapa + 1, window, cx)
                                })),
                        )
                    })
                    .map(|c| {
                        if !ultima {
                            return c.child(
                                estilo::botao_primario("nova-avancar", cx)
                                    .child("Avançar")
                                    .child(Icon::new(Icone::ArrowRight).size(px(16.)))
                                    .on_click(
                                        cx.listener(|tela, _, window, cx| tela.avancar(window, cx)),
                                    ),
                            );
                        }
                        let (rotulo, ocupado): (String, bool) = match self.fase {
                            Some(Fase::Copiando) => ("Terminando a cópia…".into(), true),
                            Some(_) => ("Criando…".into(), true),
                            None => (self.rotulo_de_criar().into(), false),
                        };
                        c.child(
                            estilo::desligado(
                                estilo::botao_primario("nova-criar", cx)
                                    .when(ocupado, |b| {
                                        b.child(Icon::new(Icone::LoaderCircle).size(px(16.)))
                                    })
                                    .child(rotulo),
                                ocupado,
                            )
                            .when(!ocupado, |b| {
                                b.on_click(
                                    cx.listener(|tela, _, window, cx| tela.criar(window, cx)),
                                )
                            }),
                        )
                    }),
            )
    }

    // ── Sobreposições ────────────────────────────────────────────────────

    fn sobreposicao(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        div()
            .absolute()
            .inset_0()
            .p(px(24.))
            .bg(tema.background.opacity(0.85))
            .child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap(px(12.))
                    .rounded(px(16.))
                    .border_2()
                    .border_dashed()
                    .border_color(cor(AMBAR))
                    .child(
                        Icon::new(Icone::Upload)
                            .size(px(32.))
                            .text_color(cor(AMBAR)),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Solte para importar nesta nova sessão"),
                    )
                    .child(div().text_sm().text_color(tema.muted_foreground).child(
                        "As fotos ficam neste computador enquanto você termina o atendimento.",
                    )),
            )
            .on_mouse_down_out(cx.listener(|tela, _, _, cx| tela.destacar(false, cx)))
    }

    fn veu(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("nova-veu")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(tema::cores::veu())
            .occlude()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|tela, _, _, cx| tela.fechar_busca(cx)),
            )
    }

    fn dialogo(&self, qual: Confirmacao, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme().clone();
        let fotos = self.quantas_fotos();
        let (titulo, descricao) = match qual {
            Confirmacao::Descartar => {
                let mut d = if fotos > 0 {
                    format!(
                        "{} só {} neste computador e {} apagada{}. ",
                        estado::plural(fotos, "foto importada", "fotos importadas"),
                        if fotos == 1 { "existe" } else { "existem" },
                        if fotos == 1 { "será" } else { "serão" },
                        if fotos == 1 { "" } else { "s" },
                    )
                } else {
                    String::new()
                };
                d.push_str(if self.rascunho.criada_id.is_some() {
                    "A sessão já existe no banco e continua lá — só as fotos locais e o \
                     preenchimento saem."
                } else {
                    "O que foi preenchido também se perde."
                });
                ("Descartar esta nova sessão?".to_string(), d)
            }
            Confirmacao::DescartarGuardado => {
                let n = self.fotos_do_guardado;
                let mut d = if n > 0 {
                    format!(
                        "{} só {} neste computador e {} apagada{} — nunca subiram. ",
                        estado::plural(n, "foto importada", "fotos importadas"),
                        if n == 1 { "existe" } else { "existem" },
                        if n == 1 { "será" } else { "serão" },
                        if n == 1 { "" } else { "s" },
                    )
                } else {
                    String::new()
                };
                d.push_str("O que foi preenchido também se perde.");
                (
                    "Descartar o rascunho e começar outra sessão?".to_string(),
                    d,
                )
            }
            Confirmacao::ApagarOrfas => (
                "Apagar as fotos de rascunhos antigos?".to_string(),
                "Elas foram importadas numa nova sessão que não chegou a ser criada, e só \
                 existem neste computador."
                    .to_string(),
            ),
        };
        let rotulo = match qual {
            Confirmacao::ApagarOrfas => "Apagar",
            _ => "Descartar",
        };
        self.veu(cx).child(
            v_flex()
                .w(px(460.))
                .p(px(24.))
                .gap(px(12.))
                .rounded(px(12.))
                .border_1()
                .border_color(tema.border)
                .bg(tema.background)
                .shadow_lg()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(titulo),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(tema.muted_foreground)
                        .child(descricao),
                )
                .child(
                    h_flex()
                        .mt(px(8.))
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            estilo::botao_contorno("nova-confirmacao-cancelar", cx)
                                .child("Cancelar")
                                .on_click(
                                    cx.listener(|tela, _, _, cx| tela.cancelar_confirmacao(cx)),
                                ),
                        )
                        .child(
                            estilo::botao_perigo("nova-confirmacao-ok", cx)
                                .child(rotulo)
                                .on_click(
                                    cx.listener(|tela, _, window, cx| tela.confirmar(window, cx)),
                                ),
                        ),
                ),
        )
    }

    fn dialogo_da_pasta(
        &self,
        selecao: &SelecaoDaPasta,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tema = cx.theme().clone();
        let selecionadas = selecao.fotos.iter().filter(|(_, marcado)| *marcado).count();
        let todas = selecionadas == selecao.fotos.len();
        let minimizada = self.selecao_da_pasta_minimizada;
        let maximizada = self.selecao_da_pasta_maximizada;
        let zoom = self.zoom_da_pasta_valor;
        let largura_janela = f32::from(window.viewport_size().width);
        let largura_modal = if maximizada {
            (largura_janela * 0.94).min(1440.)
        } else {
            (largura_janela * 0.88).min(820.)
        };
        let colunas = ((largura_modal - 64.) / (178. * zoom))
            .floor()
            .clamp(2., 8.) as u16;
        let nome_da_pasta = std::path::Path::new(&selecao.raiz)
            .file_name()
            .map(|nome| nome.to_string_lossy().to_string())
            .unwrap_or_else(|| selecao.raiz.clone());

        self.veu(cx).child(
            v_flex()
                .w(relative(if maximizada { 0.94 } else { 0.88 }))
                .max_w(px(if maximizada { 1440. } else { 820. }))
                .max_h(if maximizada { relative(0.94) } else { relative(0.82) })
                .p(px(24.))
                .gap(px(14.))
                .rounded(px(16.))
                .border_1()
                .border_color(tema.border)
                .bg(tema.background)
                .shadow_lg()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_key_down(cx.listener(|tela, evento: &KeyDownEvent, window, cx| {
                    tela.tecla(evento, window, cx);
                }))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(16.))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w(px(0.))
                                .gap(px(3.))
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Escolha as fotos da pasta"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(tema.muted_foreground)
                                        .truncate()
                                        .child(format!(
                                            "{nome_da_pasta} — clique para selecionar; Shift seleciona um intervalo."
                                        )),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .id("nova-minimizar-selecao-pasta")
                                        .p(px(8.))
                                        .rounded(px(7.))
                                        .cursor_pointer()
                                        .hover(|d| d.bg(tema.accent))
                                        .tooltip(|window, cx| {
                                            gpui_component::tooltip::Tooltip::new("Minimizar")
                                                .build(window, cx)
                                        })
                                        .child(Icon::new(Icone::Minus).size(px(17.)))
                                        .on_click(cx.listener(|tela, _, _, cx| {
                                            tela.alternar_minimizacao_da_pasta(cx)
                                        })),
                                )
                                .child(
                                    div()
                                        .id("nova-maximizar-selecao-pasta")
                                        .p(px(8.))
                                        .rounded(px(7.))
                                        .cursor_pointer()
                                        .hover(|d| d.bg(tema.accent))
                                        .tooltip(move |window, cx| {
                                            gpui_component::tooltip::Tooltip::new(if maximizada {
                                                "Restaurar tamanho"
                                            } else {
                                                "Maximizar"
                                            })
                                            .build(window, cx)
                                        })
                                        .child(
                                            Icon::new(if maximizada {
                                                Icone::Minimize2
                                            } else {
                                                Icone::Maximize2
                                            })
                                            .size(px(17.)),
                                        )
                                        .on_click(cx.listener(|tela, _, _, cx| {
                                            tela.alternar_maximizacao_da_pasta(cx)
                                        })),
                                ),
                        ),
                )
                .when(!minimizada, |modal| {
                    modal
                        .child(
                            h_flex()
                                .items_center()
                                .justify_between()
                                .gap(px(16.))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .text_color(tema.muted_foreground)
                                        .child(format!(
                                            "{} de {} selecionadas",
                                            selecionadas,
                                            selecao.fotos.len()
                                        )),
                                )
                        )
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .gap(px(8.))
                                .p(px(8.))
                                .rounded(px(9.))
                                .bg(tema.muted)
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(Icon::new(Icone::ZoomOut).size(px(15.)))
                                        .child(
                                            div()
                                                .w(px(240.))
                                                .child(Slider::new(&self.zoom_da_pasta).horizontal()),
                                        )
                                        .child(Icon::new(Icone::ZoomIn).size(px(15.)))
                                        .child(
                                            div()
                                                .w(px(42.))
                                                .text_xs()
                                                .text_color(tema.muted_foreground)
                                                .child(format!("{:.0}%", zoom * 100.)),
                                        ),
                                )
                                .child(
                                    Checkbox::new("nova-marcar-todas-da-pasta")
                                        .label(if todas {
                                            "Desmarcar todas"
                                        } else {
                                            "Marcar todas"
                                        })
                                        .checked(todas)
                                        .on_click(cx.listener(move |tela, marcado: &bool, _, cx| {
                                            tela.marcar_todas_da_pasta(*marcado, cx)
                                        })),
                                ),
                        )
                        .child(
                            v_flex()
                                .id("nova-fotos-da-pasta-lista")
                                .flex_1()
                                .min_h(px(0.))
                                .overflow_y_scroll()
                                .child(
                                    div()
                                        .id("nova-fotos-da-pasta-grade")
                                        .grid()
                                        .grid_cols(colunas)
                                        .gap(px(10.))
                                        .children(selecao.fotos.iter().enumerate().map(
                                            |(indice, (caminho, marcado))| {
                                                let nome = std::path::Path::new(caminho)
                                                    .file_name()
                                                    .map(|n| n.to_string_lossy().to_string())
                                                    .unwrap_or_else(|| caminho.clone());
                                                let miniatura =
                                                    self.miniaturas_da_pasta.get(caminho).cloned();
                                                let tem_miniatura = miniatura.is_some();
                                                v_flex()
                                                    .id(SharedString::from(format!(
                                                        "nova-foto-da-pasta-{indice}"
                                                    )))
                                                    .relative()
                                                    .min_w(px(0.))
                                                    .p(px(7.))
                                                    .gap(px(6.))
                                                    .rounded(px(10.))
                                                    .border_1()
                                                    .border_color(if *marcado {
                                                        tema.primary
                                                    } else {
                                                        tema.border
                                                    })
                                                    .when(*marcado, |card| {
                                                        card.bg(tema.primary.opacity(0.12))
                                                    })
                                                    .hover(|card| card.bg(tema.muted))
                                                    .on_click(cx.listener(
                                                        move |tela, evento: &ClickEvent, _, cx| {
                                                            tela.marcar_foto_da_pasta(
                                                                indice,
                                                                evento.modifiers().shift,
                                                                cx,
                                                            )
                                                        },
                                                    ))
                                                    .child(
                                                        div()
                                                            .relative()
                                                            .w_full()
                                                            .h(px(132. * zoom))
                                                            .rounded(px(7.))
                                                            .overflow_hidden()
                                                            .bg(tema.muted)
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .when_some(miniatura, |quadro, imagem| {
                                                                quadro.child(
                                                                    img(imagem).size_full().object_fit(
                                                                        gpui::ObjectFit::Cover,
                                                                    ),
                                                                )
                                                            })
                                                            .when(!tem_miniatura, |quadro| {
                                                                quadro.child(
                                                                    Icon::new(Icone::ImagePlus)
                                                                        .size(px(26.))
                                                                        .text_color(
                                                                            tema.muted_foreground,
                                                                        ),
                                                                )
                                                            })
                                                            .child(
                                                                div()
                                                                    .absolute()
                                                                    .top(px(7.))
                                                                    .right(px(7.))
                                                                    .size(px(25.))
                                                                    .rounded_full()
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .border_1()
                                                                    .border_color(if *marcado {
                                                                        tema.primary
                                                                    } else {
                                                                        tema.border
                                                                    })
                                                                    .when(*marcado, |selo| {
                                                                        selo.bg(tema.primary)
                                                                            .text_color(tema.primary_foreground)
                                                                            .child(
                                                                                Icon::new(Icone::Check)
                                                                                    .size(px(14.)),
                                                                            )
                                                                    }),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .truncate()
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .child(nome),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(tema.muted_foreground)
                                                            .child(format!("Foto {}", indice + 1)),
                                                    )
                                                    .into_any_element()
                                            },
                                        )),
                                ),
                        )
                })
                .child(
                    h_flex()
                        .mt(px(4.))
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            estilo::botao_contorno("nova-cancelar-selecao-pasta", cx)
                                .child("Cancelar")
                                .on_click(
                                    cx.listener(|tela, _, _, cx| {
                                        tela.cancelar_selecao_da_pasta(cx)
                                    }),
                                ),
                        )
                        .child(
                            estilo::botao_primario("nova-importar-selecao-pasta", cx)
                                .child(format!(
                                    "Importar {}",
                                    estado::plural(selecionadas, "foto", "fotos")
                                ))
                                .on_click(cx.listener(|tela, _, window, cx| {
                                    tela.importar_selecao_da_pasta(window, cx)
                                })),
                        ),
                ),
        )
    }

    fn modal_de_busca(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(busca) = self.busca.as_ref() else {
            return div().id("nova-sem-busca");
        };
        let tema = cx.theme().clone();
        let (titulo, descricao, colunas): (&str, &str, [&str; 4]) = match busca.tipo {
            TipoDeBusca::Agendamento => (
                "Associar agendamento",
                "Sem busca, os mais perto de hoje. Com busca, qualquer data.",
                ["Nome", "Contato", "Quando", "Situação"],
            ),
            TipoDeBusca::Voucher => (
                "Associar voucher",
                "Os não usados primeiro, do mais recente ao mais antigo.",
                ["Voucher", "Contato", "Emitido / agendado", "Uso"],
            ),
            TipoDeBusca::Compra => (
                "Associar compra antecipada",
                "As pagas mais recentes primeiro.",
                ["Comprador e itens", "Contato", "Pagamento", "Situação"],
            ),
            TipoDeBusca::Parceiro => (
                "Parceiro que indicou",
                "Não achou? Cadastre abaixo — nome e tipo bastam.",
                ["Parceiro", "Contato", "Tipo", "Situação"],
            ),
        };
        let vazio = match (busca.tipo, busca.com_texto) {
            (TipoDeBusca::Agendamento, true) => "Nenhum agendamento bate com a busca.",
            (TipoDeBusca::Agendamento, false) => "Nenhum agendamento perto de hoje.",
            (TipoDeBusca::Voucher, true) => "Nenhum voucher bate com a busca.",
            (TipoDeBusca::Voucher, false) => "Nenhum voucher encontrado.",
            (TipoDeBusca::Compra, true) => "Nenhuma compra bate com a busca.",
            (TipoDeBusca::Compra, false) => "Nenhuma compra antecipada encontrada.",
            (TipoDeBusca::Parceiro, true) => "Nenhum parceiro com esse nome.",
            (TipoDeBusca::Parceiro, false) => "Nenhum parceiro cadastrado ainda.",
        };
        let celula = |texto: String, forte: bool| {
            div()
                .flex_1()
                .min_w(px(0.))
                .text_sm()
                .when(forte, |d| d.font_weight(FontWeight::MEDIUM))
                .when(!forte, |d| d.text_color(tema.muted_foreground))
                .truncate()
                .child(texto)
        };
        let linhas: Vec<AnyElement> = busca
            .itens
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let [a, b, c, d] = colunas_do_item(item);
                let item = item.clone();
                h_flex()
                    .id(SharedString::from(format!("nova-resultado-{i}")))
                    .gap(px(12.))
                    .px(px(12.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(tema.border)
                    .cursor_pointer()
                    .hover(|h| h.bg(tema.accent))
                    .child(celula(a, true))
                    .child(celula(b, false))
                    .child(celula(c, false))
                    .child(celula(d, false))
                    .on_click(cx.listener(move |tela, _, window, cx| {
                        tela.escolher_da_busca(item.clone(), window, cx)
                    }))
                    .into_any_element()
            })
            .collect();

        self.veu(cx).child(
            v_flex()
                .w(px(760.))
                .max_h(relative(0.85))
                .p(px(24.))
                .gap(px(12.))
                .rounded(px(12.))
                .border_1()
                .border_color(tema.border)
                .bg(tema.background)
                .shadow_lg()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .child(
                            div()
                                .flex_1()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(titulo),
                        )
                        .child(
                            estilo::botao_fantasma("nova-fechar-busca", cx)
                                .child(Icon::new(Icone::X).size(px(16.)))
                                .on_click(cx.listener(|tela, _, _, cx| tela.fechar_busca(cx))),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(tema.muted_foreground)
                        .child(descricao),
                )
                .child(entrada(&busca.campo).prefix(Icon::new(Icone::Search).size(px(16.))))
                .child(
                    v_flex()
                        .id("nova-resultados")
                        .flex_1()
                        .min_h(px(160.))
                        .overflow_y_scroll()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(tema.border)
                        .child(
                            h_flex()
                                .gap(px(12.))
                                .px(px(12.))
                                .py(px(8.))
                                .border_b_1()
                                .border_color(tema.border)
                                .bg(tema.muted)
                                .children(colunas.iter().map(|c| {
                                    div()
                                        .flex_1()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(tema.muted_foreground)
                                        .child(*c)
                                })),
                        )
                        .map(|lista| {
                            if busca.carregando && busca.itens.is_empty() {
                                lista.child(div().p(px(16.)).text_sm().child("Carregando…"))
                            } else if let Some(erro) = &busca.erro {
                                lista.child(
                                    div()
                                        .p(px(16.))
                                        .text_sm()
                                        .text_color(cor(VERMELHO))
                                        .child(erro.clone()),
                                )
                            } else if busca.itens.is_empty() {
                                lista.child(
                                    div()
                                        .p(px(16.))
                                        .text_sm()
                                        .text_color(tema.muted_foreground)
                                        .child(vazio),
                                )
                            } else {
                                lista.children(linhas)
                            }
                        }),
                )
                .when(busca.tipo == TipoDeBusca::Parceiro, |c| {
                    c.child(self.cadastro_de_parceiro(true, cx))
                }),
        )
    }
}

/// As quatro colunas de uma linha do modal.
fn colunas_do_item(item: &ItemDaBusca) -> [String; 4] {
    match item {
        ItemDaBusca::Agendamento(a) => [
            a.nome.clone().unwrap_or_else(|| "Sem nome".into()),
            assoc::juntar(&[a.whatsapp.as_deref(), a.email.as_deref()]),
            assoc::juntar(&[a.quando.as_deref(), a.estudio_nome.as_deref()]),
            assoc::rotulo_do_status_do_agendamento(&a.status),
        ],
        ItemDaBusca::Voucher(v) => [
            format!(
                "Nº {} · {}",
                v.numero,
                v.nome.clone().unwrap_or_else(|| "Sem nome".into())
            ),
            assoc::juntar(&[v.whatsapp.as_deref(), v.email.as_deref()]),
            assoc::juntar(&[v.criado_em.as_deref(), v.agendamento.as_deref()]),
            match &v.utilizado_em {
                Some(quando) => format!("usado em {quando}"),
                None => "não usado".into(),
            },
        ],
        ItemDaBusca::Compra(c) => [
            assoc::juntar(&[
                Some(
                    c.comprador_nome
                        .clone()
                        .unwrap_or_else(|| "Comprador sem nome".into())
                        .as_str(),
                ),
                Some(assoc::resumo_dos_itens(&c.itens, 3).as_str()),
            ]),
            assoc::juntar(&[c.comprador_email.as_deref(), c.whatsapp.as_deref()]),
            match &c.pago_em {
                Some(p) => format!("paga em {p}"),
                None => "não paga".into(),
            },
            format!(
                "{} · {}",
                assoc::rotulo_do_status_da_compra(&c.status),
                c.total
                    .map(biblioteca_core::dinheiro::formatar)
                    .unwrap_or_default()
            ),
        ],
        ItemDaBusca::Parceiro(p) => [
            p.nome.clone(),
            assoc::juntar(&[p.whatsapp.as_deref(), p.email.as_deref()]),
            assoc::rotulo_do_tipo_de_parceiro(&p.tipo),
            if p.ativo { "ativo" } else { "inativo" }.into(),
        ],
    }
}

/// A razão largura÷altura do quadro das amostras: a do corte escolhido, ou
/// 3:2 quando não há corte.
fn receita_da_tela(proporcao: Option<&str>) -> f32 {
    super::receita::valor_da_proporcao(proporcao).unwrap_or(3. / 2.)
}

#[cfg(test)]
mod testes {
    /// 🚨 **Todo campo de texto do assistente passa por `entrada`** — e é ela
    /// que põe o `.w_full()`.
    ///
    /// O `Input` do `gpui-component` **não ocupa a largura sozinho**: sem a
    /// chamada, ele fica do tamanho intrínseco (uns 50 px) dentro de um campo
    /// de coluna inteira. Foi o que o dono viu em 2026-09-20 na etapa "Cliente
    /// e preço" — uma caixinha quadrada onde devia haver uma linha. Os `Select`
    /// deste arquivo já levavam `.w_full()` um a um; os `Input`, nenhum.
    ///
    /// 🔑 **O teste é de fonte porque o defeito é de geometria**: a largura só
    /// existe depois de a tela ser desenhada, e o `Input` do `gpui-component`
    /// não aceita `debug_selector` para o cenário medir. O que dá para cobrar
    /// sem abrir o app é que ninguém escreva `Input::new` solto aqui — que é
    /// exatamente o descuido que produz a caixinha.
    #[test]
    fn nenhum_campo_de_texto_nasce_sem_largura() {
        let fonte = include_str!("desenho.rs");
        // Só o desenho: o próprio teste fala de `Input::new` e se acusaria.
        let desenho = fonte
            .split("#[cfg(test)]")
            .next()
            .expect("o corpo do arquivo");
        let soltos: Vec<&str> = desenho
            .lines()
            .map(str::trim)
            .filter(|l| l.contains("Input::new("))
            .collect();
        assert_eq!(
            soltos,
            vec!["Input::new(estado).w_full()"],
            "há `Input::new` fora de `entrada()` — ele nasceria sem largura"
        );
    }
}
