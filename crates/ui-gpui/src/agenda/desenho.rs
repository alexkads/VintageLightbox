//! O desenho da agenda — `page.tsx`, `indicadores.tsx`, `calendario/`,
//! `lista-de-hoje.tsx` e `detalhe-do-agendamento.tsx` do site, com as mesmas
//! palavras.
//!
//! ⚠️ **Semana e Dia são listas por horário, e não a grade de horas** do
//! react-big-calendar: o que o operador lê ali (quem, que horas, qual
//! estúdio) é o mesmo, e a grade posicionada por minuto não mudaria gesto
//! nenhum. Está na lista de divergências (`FLUXO_UNICO`, §14).

use chrono::{Datelike, NaiveDate, Utc};
use gpui::{
    div, prelude::*, px, rgb, AnyElement, Context, FontWeight, Hsla, MouseButton, SharedString,
    Window,
};
use gpui_component::input::Input;
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon};

use super::modelo::{self, Ensaio, Visao};
use super::tela::{Agenda, Modo};
use super::VoltarNaAgenda;
use crate::estilo;
use crate::recursos::Icone;
use crate::tempo_real::EstadoDaConexao;

/// O contexto de teclas do diálogo (o Esc que volta um passo).
pub const DIALOGO: &str = "DialogoDaAgenda";

fn marcado(botao: gpui::Stateful<gpui::Div>, nome: impl Into<String>) -> gpui::Stateful<gpui::Div> {
    let nome = nome.into();
    botao.debug_selector(move || nome.clone())
}

fn cor(status_rgb: u32) -> Hsla {
    rgb(status_rgb).into()
}

const INICIAIS: [&str; 7] = ["D", "S", "T", "Q", "Q", "S", "S"];
const DIAS_CURTOS: [&str; 7] = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];

impl Render for Agenda {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.preencher_se_preciso(window, cx);
        let tema = cx.theme();
        let (fundo, texto) = (tema.background, tema.foreground);
        let dialogo = self.aberto.aberto().cloned().map(|e| self.dialogo(e, cx));
        let dia_aberto = self.dia_aberto.map(|d| self.popup_do_dia(d, cx));

        v_flex()
            .id("agenda")
            .key_context(DIALOGO)
            .track_focus(&self.foco)
            .on_action(cx.listener(|tela, _: &VoltarNaAgenda, window, cx| tela.voltar(window, cx)))
            // O Esc de dentro de um campo é do campo; ele o repassa, e aqui
            // vira o mesmo "voltar um passo".
            .on_action(
                cx.listener(|tela, _: &gpui_component::input::Escape, window, cx| {
                    tela.voltar(window, cx)
                }),
            )
            .relative()
            .size_full()
            .min_h(px(0.))
            .bg(fundo)
            .text_color(texto)
            .text_sm()
            .child(
                v_flex()
                    .id("agenda-rolagem")
                    .size_full()
                    .overflow_y_scroll()
                    .p(px(16.))
                    .gap(px(16.))
                    .child(self.cabecalho(cx))
                    .child(self.indicadores(cx))
                    .child(
                        h_flex()
                            .items_start()
                            .gap(px(16.))
                            .child(div().flex_1().min_w(px(0.)).child(self.calendario(cx)))
                            .child(
                                v_flex()
                                    .w(px(340.))
                                    .flex_none()
                                    .gap(px(16.))
                                    .child(self.lista_de_hoje(cx))
                                    .child(self.visao_da_semana(cx)),
                            ),
                    ),
            )
            .children(dia_aberto)
            .children(dialogo)
    }
}

impl Agenda {
    fn cabecalho(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, popover, borda, acento) = (
            tema.muted_foreground,
            tema.popover,
            tema.border,
            tema.accent,
        );
        let cor_da_conexao: Hsla = match self.conexao {
            EstadoDaConexao::Conectado => rgb(0x16a34a).into(),
            EstadoDaConexao::Recusado => rgb(0xef4444).into(),
            _ => rgb(0xf59e0b).into(),
        };
        let opcoes: Vec<AnyElement> = std::iter::once((None, "Todos os Estúdios".to_string()))
            .chain(
                self.estudios
                    .iter()
                    .map(|e| (Some(e.id.clone()), e.nome.clone())),
            )
            .enumerate()
            .map(|(i, (id, nome))| {
                let escolhido = self.estudio == id;
                div()
                    .id(("agenda-estudio", i))
                    .debug_selector(move || format!("agenda-estudio-{i}"))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(move |s| s.bg(acento))
                    .when(escolhido, |d| d.font_weight(FontWeight::MEDIUM))
                    .child(nome)
                    .on_click(
                        cx.listener(move |tela, _, _, cx| tela.escolher_estudio(id.clone(), cx)),
                    )
                    .into_any_element()
            })
            .collect();

        h_flex()
            .relative()
            .gap(px(12.))
            .items_center()
            .child(
                v_flex()
                    .flex_1()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Agendamentos"),
                    )
                    .child(
                        div()
                            .text_color(apagado)
                            .child("Agenda do estúdio — reservas do site e do WhatsApp."),
                    ),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .text_xs()
                    .text_color(apagado)
                    .child(div().size(px(8.)).rounded_full().bg(cor_da_conexao))
                    .child(self.conexao.rotulo()),
            )
            .child(
                marcado(
                    estilo::botao_contorno("agenda-seletor", cx),
                    "agenda-seletor",
                )
                .child(Icon::new(Icone::MapPin).size(px(16.)))
                .child(self.nome_do_estudio_escolhido())
                .child(Icon::new(Icone::ChevronDown).size(px(14.)))
                .on_click(cx.listener(|tela, _, _, cx| tela.alternar_seletor_de_estudio(cx))),
            )
            .when(self.escolhendo_estudio, |d| {
                d.child(
                    v_flex()
                        .absolute()
                        .top(px(44.))
                        .right_0()
                        .w(px(240.))
                        .p(px(4.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(borda)
                        .bg(popover)
                        .shadow_lg()
                        .occlude()
                        .children(opcoes),
                )
            })
    }

    fn indicadores(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, borda) = (tema.muted_foreground, tema.border);
        if self.falhou_os_indicadores {
            return estilo::aviso(
                "Os indicadores não carregaram. A agenda abaixo continua valendo. Recarregue para tentar de novo.",
                true,
                cx,
            )
            .into_any_element();
        }
        let i = self.indicadores.unwrap_or_default();
        let celula = |rotulo: &'static str, valor: String, ponto: Option<u32>| {
            v_flex()
                .flex_1()
                .p(px(12.))
                .gap(px(4.))
                .child(
                    h_flex()
                        .gap(px(6.))
                        .items_center()
                        .text_xs()
                        .text_color(apagado)
                        .when_some(ponto, |d, c| {
                            d.child(div().size(px(8.)).rounded_full().bg(cor(c)))
                        })
                        .child(rotulo),
                )
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(valor),
                )
        };
        v_flex()
            .rounded(px(10.))
            .border_1()
            .border_color(borda)
            .child(
                h_flex()
                    .child(celula("Total", i.total.to_string(), None))
                    .child(celula("Confirmados", i.confirmados.to_string(), Some(0x22c55e)))
                    .child(celula("Pendentes", i.pendentes.to_string(), Some(0xeab308)))
                    .child(celula("Cancelados", i.cancelados.to_string(), Some(0xef4444)))
                    .child(celula("Concluídos", i.concluidos.to_string(), Some(0xa855f7)))
                    .child(celula("Ocupação hoje", format!("{}%", i.ocupacao_hoje), None)),
            )
            .child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .border_t_1()
                    .border_color(borda)
                    .text_xs()
                    .text_color(apagado)
                    .child("De hoje em diante, somando todos os estúdios — o seletor acima não recorta estes números."),
            )
            .into_any_element()
    }

    // ── O calendário ───────────────────────────────────────────────────────

    fn calendario(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let borda = tema.border;
        let visoes: Vec<AnyElement> = Visao::TODAS
            .into_iter()
            .map(|visao| {
                let id = format!("agenda-visao-{}", visao.rotulo());
                let botao = if self.visao == visao {
                    estilo::botao_primario(id.clone(), cx)
                } else {
                    estilo::botao_contorno(id.clone(), cx)
                };
                marcado(botao, id)
                    .h(px(28.))
                    .child(visao.rotulo())
                    .on_click(cx.listener(move |tela, _, _, cx| tela.escolher_visao(visao, cx)))
                    .into_any_element()
            })
            .collect();
        let corpo: AnyElement = if self.falhou_a_agenda {
            estilo::aviso(
                "Não foi possível carregar a agenda. Verifique o período — o máximo é de um ano — e tente de novo.",
                true,
                cx,
            )
            .into_any_element()
        } else {
            match self.visao {
                Visao::Mes => self.grade_do_mes(cx).into_any_element(),
                Visao::Semana => self.semana(cx).into_any_element(),
                Visao::Dia => self.lista_de_dias(vec![self.dia], cx).into_any_element(),
                Visao::Agenda => {
                    let (de, ate) = modelo::periodo_visivel(Visao::Agenda, self.dia);
                    let dias = de.iter_days().take_while(|d| *d <= ate).collect();
                    self.lista_de_dias(dias, cx).into_any_element()
                }
                Visao::Ano | Visao::Estacao => self.meses_pequenos(cx).into_any_element(),
            }
        };
        v_flex()
            .rounded(px(10.))
            .border_1()
            .border_color(borda)
            .p(px(12.))
            .gap(px(12.))
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        marcado(estilo::botao_contorno("agenda-hoje", cx), "agenda-hoje")
                            .h(px(28.))
                            .child("Hoje")
                            .on_click(cx.listener(|tela, _, _, cx| tela.ir_para_hoje(cx))),
                    )
                    .child(
                        marcado(
                            estilo::botao_contorno("agenda-anterior", cx),
                            "agenda-anterior",
                        )
                        .h(px(28.))
                        .child("Anterior")
                        .on_click(cx.listener(|tela, _, _, cx| tela.andar(-1, cx))),
                    )
                    .child(
                        marcado(
                            estilo::botao_contorno("agenda-proximo", cx),
                            "agenda-proximo",
                        )
                        .h(px(28.))
                        .child("Próximo")
                        .on_click(cx.listener(|tela, _, _, cx| tela.andar(1, cx))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_center()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(modelo::titulo_do_periodo(self.visao, self.dia)),
                    )
                    .child(h_flex().gap(px(4.)).children(visoes)),
            )
            .child(corpo)
    }

    /// Um evento no calendário: a cor do status e o título.
    fn chip(
        &self,
        ensaio: &Ensaio,
        id: String,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let e = ensaio.clone();
        let c = cor(ensaio.status.cor());
        div()
            .id(SharedString::from(id.clone()))
            .debug_selector(move || id.clone())
            .w_full()
            .px(px(6.))
            .py(px(2.))
            .rounded(px(4.))
            .bg(c.opacity(0.18))
            .border_l_2()
            .border_color(c)
            .text_xs()
            .truncate()
            .cursor_pointer()
            .when(ensaio.status.cancelado(), |d| d.opacity(0.6))
            .child(format!(
                "{} {}",
                modelo::hora(ensaio.inicio),
                ensaio.titulo()
            ))
            .on_click(cx.listener(move |tela, _, window, cx| {
                tela.abrir_ensaio(e.clone(), window, cx);
            }))
    }

    fn grade_do_mes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, borda, primario) = (tema.muted_foreground, tema.border, tema.primary);
        let hoje = modelo::hoje(Utc::now());
        let mes = self.dia.month();
        let cabecalho = h_flex().children(DIAS_CURTOS.map(|d| {
            div()
                .flex_1()
                .text_center()
                .text_xs()
                .text_color(apagado)
                .child(d)
        }));
        let semanas: Vec<AnyElement> = modelo::semanas_do_mes(self.dia)
            .into_iter()
            .map(|semana| {
                h_flex()
                    .items_start()
                    .children(semana.into_iter().map(|dia| {
                        let do_dia = modelo::ensaios_do_dia(&self.ensaios, dia);
                        let mais = do_dia.len().saturating_sub(3);
                        let chips: Vec<AnyElement> = do_dia
                            .iter()
                            .take(3)
                            .map(|e| {
                                self.chip(e, format!("agenda-evento-{}", e.id), cx)
                                    .into_any_element()
                            })
                            .collect();
                        v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .h(px(104.))
                            .p(px(4.))
                            .gap(px(2.))
                            .border_1()
                            .border_color(borda)
                            .when(dia.month() != mes, |d| d.opacity(0.5))
                            .child(
                                div()
                                    .text_xs()
                                    .when(dia == hoje, |d| {
                                        d.font_weight(FontWeight::BOLD).text_color(primario)
                                    })
                                    .child(dia.day().to_string()),
                            )
                            .children(chips)
                            .when(mais > 0, |d| {
                                let id = format!("agenda-mais-{}", dia.format("%Y-%m-%d"));
                                d.child(
                                    div()
                                        .id(SharedString::from(id.clone()))
                                        .debug_selector(move || id.clone())
                                        .text_xs()
                                        .text_color(apagado)
                                        .cursor_pointer()
                                        .child(format!("+ {mais} mais"))
                                        .on_click(cx.listener(move |tela, _, _, cx| {
                                            tela.mostrar_o_dia(Some(dia), cx)
                                        })),
                                )
                            })
                            .into_any_element()
                    }))
                    .into_any_element()
            })
            .collect();
        v_flex().gap(px(4.)).child(cabecalho).children(semanas)
    }

    fn semana(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, borda) = (tema.muted_foreground, tema.border);
        let domingo = modelo::domingo_da_semana(self.dia);
        h_flex().items_start().children((0..7).map(|i| {
            let dia = domingo + chrono::Duration::days(i);
            let chips: Vec<AnyElement> = modelo::ensaios_do_dia(&self.ensaios, dia)
                .iter()
                .map(|e| {
                    self.chip(e, format!("agenda-evento-{}", e.id), cx)
                        .into_any_element()
                })
                .collect();
            v_flex()
                .flex_1()
                .min_w(px(0.))
                .min_h(px(240.))
                .p(px(4.))
                .gap(px(4.))
                .border_1()
                .border_color(borda)
                .child(div().text_xs().text_color(apagado).child(format!(
                    "{} {}",
                    DIAS_CURTOS[i as usize],
                    dia.day()
                )))
                .children(chips)
        }))
    }

    /// Dia e Agenda: os dias com ensaio, cada um com seus horários.
    fn lista_de_dias(&self, dias: Vec<NaiveDate>, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, borda) = (tema.muted_foreground, tema.border);
        let blocos: Vec<AnyElement> = dias
            .into_iter()
            .filter_map(|dia| {
                let do_dia = modelo::ensaios_do_dia(&self.ensaios, dia);
                if do_dia.is_empty() {
                    return None;
                }
                let linhas: Vec<AnyElement> = do_dia
                    .iter()
                    .map(|e| {
                        h_flex()
                            .gap(px(12.))
                            .items_center()
                            .child(
                                div()
                                    .w(px(110.))
                                    .flex_none()
                                    .text_xs()
                                    .text_color(apagado)
                                    .child(format!(
                                        "{} – {}",
                                        modelo::hora(e.inicio),
                                        modelo::hora(e.fim)
                                    )),
                            )
                            .child(div().flex_1().min_w(px(0.)).child(self.chip(
                                e,
                                format!("agenda-evento-{}", e.id),
                                cx,
                            )))
                            .into_any_element()
                    })
                    .collect();
                Some(
                    v_flex()
                        .gap(px(4.))
                        .pb(px(8.))
                        .border_b_1()
                        .border_color(borda)
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .child(modelo::data_longa(dia)),
                        )
                        .children(linhas)
                        .into_any_element(),
                )
            })
            .collect();
        if blocos.is_empty() {
            return v_flex()
                .py(px(32.))
                .items_center()
                .text_color(apagado)
                .child("Não há agendamentos neste período.");
        }
        v_flex().gap(px(8.)).children(blocos)
    }

    /// Ano (12) e Estação (3): os meses pequenos, com o pontinho nos dias com
    /// ensaio. O dia abre a visão de dia; o nome do mês, a de mês.
    fn meses_pequenos(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, primario, sobre_primario) =
            (tema.muted_foreground, tema.primary, tema.primary_foreground);
        let hoje = modelo::hoje(Utc::now());
        let colunas = if self.visao == Visao::Ano { 4 } else { 3 };
        let meses: Vec<AnyElement> = modelo::meses_da_visao(self.visao, self.dia)
            .into_iter()
            .map(|primeiro| {
                let mes = primeiro.month();
                let titulo_id = format!("agenda-mes-{}", primeiro.format("%Y-%m"));
                let semanas: Vec<AnyElement> = modelo::semanas_do_mes(primeiro)
                    .into_iter()
                    .map(|semana| {
                        h_flex()
                            .children(semana.into_iter().map(|dia| {
                                if dia.month() != mes {
                                    return div().flex_1().h(px(24.)).into_any_element();
                                }
                                let ponto = modelo::dia_com_ponto(&self.ensaios, dia);
                                let e_hoje = dia == hoje;
                                let id = format!("agenda-dia-{}", dia.format("%Y-%m-%d"));
                                v_flex()
                                    .id(SharedString::from(id.clone()))
                                    .debug_selector(move || id.clone())
                                    .flex_1()
                                    .h(px(24.))
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_xs()
                                    .when(e_hoje, |d| d.bg(primario).text_color(sobre_primario))
                                    .child(dia.day().to_string())
                                    .when(ponto, |d| {
                                        d.child(div().size(px(4.)).rounded_full().bg(if e_hoje {
                                            sobre_primario
                                        } else {
                                            primario
                                        }))
                                    })
                                    .on_click(cx.listener(move |tela, _, _, cx| {
                                        tela.abrir_dia(dia, Visao::Dia, cx)
                                    }))
                                    .into_any_element()
                            }))
                            .into_any_element()
                    })
                    .collect();
                v_flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .id(SharedString::from(titulo_id.clone()))
                            .debug_selector(move || titulo_id.clone())
                            .font_weight(FontWeight::MEDIUM)
                            .cursor_pointer()
                            .child(modelo::nome_do_mes(primeiro.month0()).to_string())
                            .on_click(cx.listener(move |tela, _, _, cx| {
                                tela.abrir_dia(primeiro, Visao::Mes, cx)
                            })),
                    )
                    .child(h_flex().children(INICIAIS.map(|l| {
                        div()
                            .flex_1()
                            .text_center()
                            .text_xs()
                            .text_color(apagado)
                            .child(l)
                    })))
                    .children(semanas)
                    .into_any_element()
            })
            .collect();
        let mut linhas = Vec::new();
        let mut meses = meses.into_iter().peekable();
        while meses.peek().is_some() {
            let linha: Vec<AnyElement> = meses.by_ref().take(colunas).collect();
            linhas.push(
                h_flex()
                    .gap(px(16.))
                    .items_start()
                    .children(linha.into_iter().map(|m| div().flex_1().child(m)))
                    .into_any_element(),
            );
        }
        v_flex().gap(px(16.)).children(linhas)
    }

    // ── As colunas da direita ──────────────────────────────────────────────

    fn lista_de_hoje(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, borda) = (tema.muted_foreground, tema.border);
        let hoje = modelo::hoje(Utc::now());
        let n = self.de_hoje.len();
        let itens: Vec<AnyElement> = self
            .de_hoje
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let (abrir, descancelar) = (e.clone(), e.clone());
                let c = cor(e.status.cor());
                v_flex()
                    .gap(px(2.))
                    .p(px(8.))
                    .border_l_4()
                    .border_color(c)
                    .when(e.status.cancelado(), |d| d.opacity(0.6))
                    .child(
                        h_flex()
                            .justify_between()
                            .gap(px(8.))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(e.nome_na_lista()),
                            )
                            .child(div().text_xs().text_color(c).child(e.status.rotulo())),
                    )
                    .child(div().text_xs().text_color(apagado).child(format!(
                        "{} · {} · {}",
                        modelo::hora(e.inicio),
                        e.estudio_ou_padrao(),
                        e.whatsapp.clone().unwrap_or_else(|| "sem telefone".into())
                    )))
                    .when_some(e.notas.clone(), |d, notas| {
                        d.child(
                            div()
                                .text_xs()
                                .italic()
                                .text_color(apagado)
                                .child(format!("“{notas}”")),
                        )
                    })
                    .when(e.tem_atendimento(), |d| {
                        let mut partes = vec!["Atendimento".to_string()];
                        if let Some(v) = e.valor_pago {
                            partes.push(modelo::dinheiro(v));
                        }
                        if let Some(h) = e.hora_entrada {
                            partes.push(format!("entrada {}", modelo::hora(h)));
                        }
                        if let Some(h) = e.hora_saida {
                            partes.push(format!("saída {}", modelo::hora(h)));
                        }
                        d.child(div().text_xs().child(partes.join(" · ")))
                    })
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child(
                                marcado(
                                    estilo::botao_contorno(format!("hoje-abrir-{i}"), cx),
                                    format!("hoje-abrir-{i}"),
                                )
                                .h(px(24.))
                                .text_xs()
                                .child("Abrir")
                                .on_click(cx.listener(
                                    move |tela, _, window, cx| {
                                        tela.abrir_ensaio(abrir.clone(), window, cx);
                                    },
                                )),
                            )
                            .when(e.status.cancelado(), |d| {
                                d.child(estilo::desligado(
                                    marcado(
                                        estilo::botao_contorno(format!("hoje-descancelar-{i}"), cx),
                                        format!("hoje-descancelar-{i}"),
                                    )
                                    .h(px(24.))
                                    .text_xs()
                                    .child(if self.em_acao { "…" } else { "Descancelar" })
                                    .on_click(cx.listener(
                                        move |tela, _, _, cx| tela.descancelar(&descancelar, cx),
                                    )),
                                    self.em_acao,
                                ))
                            }),
                    )
                    .into_any_element()
            })
            .collect();
        v_flex()
            .rounded(px(10.))
            .border_1()
            .border_color(borda)
            .p(px(12.))
            .gap(px(8.))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Agendamentos de hoje"),
            )
            .child(div().text_xs().text_color(apagado).child(if n == 0 {
                "Nenhum agendamento para hoje".to_string()
            } else {
                format!("{n} agendamento(s) para {}", modelo::data_curta(hoje))
            }))
            .child(
                v_flex()
                    .id("agenda-de-hoje")
                    .max_h(px(384.))
                    .overflow_y_scroll()
                    .gap(px(6.))
                    .children(itens),
            )
    }

    fn visao_da_semana(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, borda, primario, muted) =
            (tema.muted_foreground, tema.border, tema.primary, tema.muted);
        let i = self.indicadores.unwrap_or_default();
        let taxa = i.taxa_de_confirmacao();
        v_flex()
            .rounded(px(10.))
            .border_1()
            .border_color(borda)
            .p(px(12.))
            .gap(px(8.))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Visão geral da semana"),
            )
            .child(
                h_flex()
                    .justify_between()
                    .child(div().text_color(apagado).child("Ensaios na semana"))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(i.total_da_semana.to_string()),
                    ),
            )
            .child(
                h_flex()
                    .justify_between()
                    .child(div().text_color(apagado).child("Taxa de confirmação"))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{taxa}%")),
                    ),
            )
            .child(
                div().h(px(6.)).w_full().rounded_full().bg(muted).child(
                    div()
                        .h_full()
                        .rounded_full()
                        .bg(primario)
                        .w(gpui::relative(taxa as f32 / 100.)),
                ),
            )
            .child(div().text_xs().text_color(apagado).child(format!(
                "{} de {} agendamentos futuros",
                i.confirmados, i.total
            )))
    }

    // ── O "+ N mais" ───────────────────────────────────────────────────────

    fn popup_do_dia(&self, dia: NaiveDate, cx: &mut Context<Self>) -> AnyElement {
        let chips: Vec<AnyElement> = modelo::ensaios_do_dia(&self.ensaios, dia)
            .iter()
            .map(|e| {
                self.chip(e, format!("agenda-dia-evento-{}", e.id), cx)
                    .into_any_element()
            })
            .collect();
        estilo::veu_do_dialogo()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|tela, _, _, cx| tela.mostrar_o_dia(None, cx)),
            )
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        estilo::caixa_do_dialogo(cx)
                            .w(px(360.))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(modelo::data_longa(dia)),
                            )
                            .child(v_flex().gap(px(4.)).children(chips)),
                    ),
            )
            .into_any_element()
    }

    // ── O diálogo do agendamento ───────────────────────────────────────────

    fn dialogo(&self, e: Ensaio, cx: &mut Context<Self>) -> AnyElement {
        let caixa = match self.modo {
            Modo::Detalhes => self.detalhes(&e, cx).into_any_element(),
            Modo::Reagendar => self.formulario_de_reagendamento(&e, cx).into_any_element(),
            Modo::Atendimento => self.formulario_de_atendimento(&e, cx).into_any_element(),
            Modo::Excluir => estilo::caixa_do_dialogo(cx)
                .child(estilo::cabecalho_do_dialogo(
                    "Excluir o agendamento?",
                    format!(
                        "O agendamento de {} do dia {} será excluído permanentemente. Esta ação não pode ser desfeita.",
                        e.titulo(),
                        modelo::data_longa(e.dia())
                    ),
                    None,
                    cx,
                ))
                .child(
                    estilo::rodape_do_dialogo()
                        .child(
                            marcado(estilo::botao_contorno("excluir-cancelar", cx), "excluir-cancelar")
                                .child("Cancelar")
                                .on_click(cx.listener(|tela, _, _, cx| {
                                    tela.entrar_no_modo(Modo::Detalhes, cx)
                                })),
                        )
                        .child(estilo::desligado(
                            marcado(estilo::botao_perigo("excluir-confirmar", cx), "excluir-confirmar")
                                .child("Excluir")
                                .on_click(cx.listener(|tela, _, _, cx| tela.confirmar_exclusao(cx))),
                            self.em_acao,
                        )),
                )
                .into_any_element(),
        };
        estilo::veu_do_dialogo()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|tela, _, window, cx| tela.fechar(window, cx)),
            )
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(caixa),
            )
            .into_any_element()
    }

    fn linha_do_detalhe(
        rotulo: &'static str,
        valor: String,
        secundario: Option<String>,
        apagado: Hsla,
    ) -> impl IntoElement {
        v_flex()
            .gap(px(2.))
            .child(
                div()
                    .text_xs()
                    .text_color(apagado)
                    .child(rotulo.to_uppercase()),
            )
            .child(div().child(valor))
            .when_some(secundario, |d, s| {
                d.child(div().text_xs().text_color(apagado).child(s))
            })
    }

    fn detalhes(&self, e: &Ensaio, cx: &mut Context<Self>) -> impl IntoElement {
        let apagado = cx.theme().muted_foreground;
        let c = cor(e.status.cor());
        let mut linhas: Vec<AnyElement> = vec![
            Self::linha_do_detalhe(
                "Horário",
                format!("{} – {}", modelo::hora(e.inicio), modelo::hora(e.fim)),
                Some(format!(
                    "Duração: {}",
                    modelo::formatar_duracao(e.duracao_em_minutos())
                )),
                apagado,
            )
            .into_any_element(),
            Self::linha_do_detalhe("Estúdio", e.estudio_ou_padrao(), e.cidade.clone(), apagado)
                .into_any_element(),
        ];
        if let Some(p) = e.pessoas {
            linhas.push(
                Self::linha_do_detalhe("Pessoas", format!("{p} pessoa(s)"), None, apagado)
                    .into_any_element(),
            );
        }
        if let Some(t) = &e.tipo_ensaio {
            linhas.push(
                Self::linha_do_detalhe("Tipo de ensaio", modelo::tipo_de_ensaio(t), None, apagado)
                    .into_any_element(),
            );
        }
        if let Some(w) = &e.whatsapp {
            linhas.push(
                Self::linha_do_detalhe("WhatsApp", w.clone(), None, apagado).into_any_element(),
            );
        }
        if let Some(o) = e.origem.as_deref().and_then(modelo::nome_da_origem) {
            linhas.push(Self::linha_do_detalhe("Origem", o, None, apagado).into_any_element());
        }
        if let Some(n) = &e.notas {
            linhas.push(
                Self::linha_do_detalhe("Observações", n.clone(), None, apagado).into_any_element(),
            );
        }
        if let Some(v) = e.valor_pago {
            linhas.push(
                Self::linha_do_detalhe("Valor pago", modelo::dinheiro(v), None, apagado)
                    .into_any_element(),
            );
        }
        if e.hora_entrada.is_some() || e.hora_saida.is_some() {
            linhas.push(
                Self::linha_do_detalhe(
                    "Atendimento",
                    format!(
                        "Entrada: {} · Saída: {}",
                        e.hora_entrada
                            .map(modelo::hora)
                            .unwrap_or_else(|| "—".into()),
                        e.hora_saida.map(modelo::hora).unwrap_or_else(|| "—".into())
                    ),
                    e.observacoes_atendimento.clone(),
                    apagado,
                )
                .into_any_element(),
            );
        }
        let descancelar = e.clone();
        let link = e.link_do_whatsapp();
        estilo::caixa_do_dialogo(cx)
            .w(px(520.))
            .child(div().h(px(4.)).rounded_full().bg(c))
            .child(
                h_flex()
                    .justify_between()
                    .items_start()
                    .child(
                        v_flex()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(e.titulo()),
                            )
                            .child(div().text_color(apagado).child(modelo::data_longa(e.dia()))),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(
                                div()
                                    .px(px(8.))
                                    .rounded_full()
                                    .bg(c.opacity(0.18))
                                    .text_color(c)
                                    .text_xs()
                                    .child(e.status.rotulo()),
                            )
                            .child(
                                marcado(
                                    estilo::botao_fantasma("detalhe-fechar", cx),
                                    "detalhe-fechar",
                                )
                                .child(Icon::new(Icone::X).size(px(16.)))
                                .on_click(
                                    cx.listener(|tela, _, window, cx| tela.fechar(window, cx)),
                                ),
                            ),
                    ),
            )
            .child(v_flex().gap(px(10.)).children(linhas))
            .child(
                h_flex()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .child(
                                marcado(
                                    estilo::botao_contorno("detalhe-reagendar", cx),
                                    "detalhe-reagendar",
                                )
                                .child("Reagendar")
                                .on_click(cx.listener(
                                    |tela, _, _, cx| tela.entrar_no_modo(Modo::Reagendar, cx),
                                )),
                            )
                            .child(
                                marcado(
                                    estilo::botao_perigo("detalhe-excluir", cx),
                                    "detalhe-excluir",
                                )
                                .child("Excluir")
                                .on_click(cx.listener(
                                    |tela, _, _, cx| tela.entrar_no_modo(Modo::Excluir, cx),
                                )),
                            )
                            .child(
                                marcado(
                                    estilo::botao_contorno("detalhe-atendimento", cx),
                                    "detalhe-atendimento",
                                )
                                .child("Atendimento")
                                .on_click(cx.listener(
                                    |tela, _, _, cx| tela.entrar_no_modo(Modo::Atendimento, cx),
                                )),
                            )
                            .when(e.status.cancelado(), |d| {
                                d.child(estilo::desligado(
                                    marcado(
                                        estilo::botao_contorno("detalhe-descancelar", cx),
                                        "detalhe-descancelar",
                                    )
                                    .child(if self.em_acao { "…" } else { "Descancelar" })
                                    .on_click(cx.listener(
                                        move |tela, _, _, cx| tela.descancelar(&descancelar, cx),
                                    )),
                                    self.em_acao,
                                ))
                            }),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .when_some(link, |d, link| {
                                d.child(
                                    marcado(
                                        estilo::botao_contorno("detalhe-mensagem", cx),
                                        "detalhe-mensagem",
                                    )
                                    .child(Icon::new(Icone::MessageCircle).size(px(16.)))
                                    .child("Mensagem")
                                    .on_click(move |_, _, cx| cx.open_url(&link)),
                                )
                            })
                            .child(
                                marcado(
                                    estilo::botao_primario("detalhe-fechar-rodape", cx),
                                    "detalhe-fechar-rodape",
                                )
                                .child("Fechar")
                                .on_click(
                                    cx.listener(|tela, _, window, cx| tela.fechar(window, cx)),
                                ),
                            ),
                    ),
            )
    }

    fn campo(
        rotulo: &'static str,
        campo: &gpui::Entity<gpui_component::input::InputState>,
    ) -> impl IntoElement {
        v_flex()
            .gap(px(4.))
            .child(div().font_weight(FontWeight::MEDIUM).child(rotulo))
            .child(Input::new(campo))
    }

    fn rodape_do_formulario(
        &self,
        confirmar_id: &'static str,
        confirmar: &'static str,
        desligar: bool,
        ao_confirmar: fn(&mut Agenda, &mut Context<Agenda>),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        estilo::rodape_do_dialogo()
            .child(estilo::desligado(
                marcado(
                    estilo::botao_contorno("formulario-voltar", cx),
                    "formulario-voltar",
                )
                .child("Voltar")
                .on_click(cx.listener(|tela, _, _, cx| tela.entrar_no_modo(Modo::Detalhes, cx))),
                self.em_acao,
            ))
            .child(estilo::desligado(
                marcado(estilo::botao_primario(confirmar_id, cx), confirmar_id)
                    .child(if self.em_acao {
                        "Salvando…"
                    } else {
                        confirmar
                    })
                    .on_click(cx.listener(move |tela, _, _, cx| ao_confirmar(tela, cx))),
                desligar || self.em_acao,
            ))
    }

    fn formulario_de_reagendamento(&self, e: &Ensaio, cx: &mut Context<Self>) -> impl IntoElement {
        let perigo = cx.theme().danger;
        let validacao = self.datas_do_reagendamento(cx);
        let erro = self.erro_do_formulario.clone().or_else(|| match validacao {
            Err("A data de término deve ser após a data de início.") => {
                Some("A data de término deve ser após a data de início.".into())
            }
            _ => None,
        });
        estilo::caixa_do_dialogo(cx)
            .child(estilo::cabecalho_do_dialogo(
                "Reagendar",
                e.titulo(),
                None,
                cx,
            ))
            .child(Self::campo("Nova data e hora de início", &self.inicio))
            .child(Self::campo("Nova data e hora de término", &self.fim))
            .when_some(erro, |d, erro| {
                d.child(div().text_color(perigo).child(erro))
            })
            .child(self.rodape_do_formulario(
                "reagendar-confirmar",
                "Confirmar reagendamento",
                validacao.is_err(),
                |tela, cx| tela.confirmar_reagendamento(cx),
                cx,
            ))
    }

    fn formulario_de_atendimento(&self, e: &Ensaio, cx: &mut Context<Self>) -> impl IntoElement {
        let perigo = cx.theme().danger;
        let validacao = self.dados_do_atendimento(cx);
        let erro = self
            .erro_do_formulario
            .clone()
            .or_else(|| validacao.as_ref().err().cloned());
        estilo::caixa_do_dialogo(cx)
            .child(estilo::cabecalho_do_dialogo(
                "Registrar atendimento",
                format!("{} — marca como Concluído", e.titulo()),
                None,
                cx,
            ))
            .child(Self::campo("Valor pago (R$)", &self.valor))
            .child(Self::campo("Hora de entrada", &self.entrada))
            .child(Self::campo("Hora de saída", &self.saida))
            .child(Self::campo("Observações do atendimento", &self.observacoes))
            .when_some(erro, |d, erro| {
                d.child(div().text_color(perigo).child(erro))
            })
            .child(self.rodape_do_formulario(
                "atendimento-confirmar",
                "Confirmar atendimento",
                validacao.is_err(),
                |tela, cx| tela.confirmar_atendimento(cx),
                cx,
            ))
    }
}
