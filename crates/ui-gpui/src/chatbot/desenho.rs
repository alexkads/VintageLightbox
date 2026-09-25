//! O desenho do painel do chatbot — a tela do site (`page.tsx`, `painel.tsx`,
//! `conversa.tsx`, `baloes.tsx`, `urgencias.tsx`) com as mesmas palavras.
//!
//! Duas colunas: a lista dos cinco canais à esquerda, a conversa à direita.
//! Por cima, os diálogos (urgências, resolver, descartar, quem assume,
//! excluir histórico).

use gpui::{
    div, prelude::*, px, rgb, AnyElement, Context, FontWeight, Hsla, MouseButton, SharedString,
    Window,
};
use gpui_component::input::Input;
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon};

use super::modelo::{self, Canal, Conversa, FiltroDeCanal, Mensagem, Status, Urgencia};
use super::tela::{Chatbot, Dialogo, Pendente};
use super::EnviarMensagem;
use crate::estilo;
use crate::recursos::Icone;
use crate::tempo_real::EstadoDaConexao;

/// O contexto de teclas do compositor: Enter envia, Shift+Enter quebra linha.
pub const COMPOSITOR: &str = "CompositorDoChatbot";

/// A cor de cada canal — a mesma do avatar e do balão do site.
fn cor_do_canal(canal: Canal) -> Hsla {
    match canal {
        Canal::WhatsApp => rgb(0x16a34a).into(),
        Canal::Instagram => rgb(0xf43f5e).into(),
        Canal::Messenger => rgb(0x7c3aed).into(),
        Canal::Telegram => rgb(0x0ea5e9).into(),
        Canal::Web => rgb(0xf59e0b).into(),
    }
}

fn icone_do_canal(canal: Canal) -> Icone {
    match canal {
        Canal::WhatsApp => Icone::MessageCircle,
        Canal::Instagram | Canal::Messenger => Icone::MessageSquare,
        Canal::Telegram => Icone::Send,
        Canal::Web => Icone::Globe,
    }
}

/// O âmbar de "alguém assumiu" — o anel do avatar e o selo "Atendente".
fn ambar() -> Hsla {
    rgb(0xf59e0b).into()
}

fn verde() -> Hsla {
    rgb(0x16a34a).into()
}

fn vermelho() -> Hsla {
    rgb(0xef4444).into()
}

/// O avatar: a cor diz o canal (no WhatsApp, o volume sem resposta) e o anel
/// âmbar diz que alguém assumiu.
fn avatar(conversa: &Conversa, lado: f32, redondo: bool) -> impl IntoElement {
    let cor = if conversa.chave.canal == Canal::WhatsApp {
        match conversa.prioridade() {
            modelo::Prioridade::Alta => vermelho(),
            modelo::Prioridade::Media => rgb(0xeab308).into(),
            modelo::Prioridade::Baixa => verde(),
        }
    } else {
        cor_do_canal(conversa.chave.canal)
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(lado))
        .rounded(if redondo { px(lado / 2.) } else { px(8.) })
        .bg(cor)
        .text_color(gpui::white())
        .font_weight(FontWeight::SEMIBOLD)
        .when(conversa.atendimento_humano, |d| {
            d.border_2().border_color(ambar())
        })
        .child(modelo::inicial(&conversa.nome))
}

/// Marca o botão para o teste clicar onde o dedo clica (`debug_selector`).
fn marcado(botao: gpui::Stateful<gpui::Div>, nome: impl Into<String>) -> gpui::Stateful<gpui::Div> {
    let nome = nome.into();
    botao.debug_selector(move || nome.clone())
}

fn selo(texto: impl Into<SharedString>, cx: &gpui::App) -> gpui::Div {
    estilo::selo_contorno(cx).child(texto.into())
}

impl Render for Chatbot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.limpar_compositor) {
            self.compositor
                .update(cx, |campo, cx| campo.set_value("", window, cx));
        }
        let tema = cx.theme();
        let (fundo, texto) = (tema.background, tema.foreground);
        let dialogo = self.dialogo.clone().map(|d| self.dialogo_aberto(d, cx));

        v_flex()
            .id("chatbot")
            .relative()
            .size_full()
            .min_h(px(0.))
            .p(px(16.))
            .gap(px(8.))
            .bg(fundo)
            .text_color(texto)
            .text_sm()
            .child(self.cabecalho(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .items_start()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .overflow_hidden()
                    .child(self.coluna_da_lista(cx))
                    .child(self.coluna_da_conversa(window, cx)),
            )
            .children(dialogo)
    }
}

impl Chatbot {
    // ── Cabeçalho ──────────────────────────────────────────────────────────

    fn cabecalho(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let apagado = tema.muted_foreground;
        let conexao = self.conexao();
        let cor_da_conexao = match conexao {
            EstadoDaConexao::Conectado => verde(),
            EstadoDaConexao::Recusado => vermelho(),
            _ => ambar(),
        };
        let esperando = self.alguem_esperando();
        let total = self.urgencias.len();
        let ligados = self.avisos_ligados();

        h_flex()
            .h(px(48.))
            .flex_none()
            .gap(px(12.))
            .items_center()
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Chatbot"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(apagado)
                            .truncate()
                            .child(self.linha_de_resumo()),
                    ),
            )
            .child(
                h_flex()
                    .id("chatbot-indicador")
                    .gap(px(6.))
                    .items_center()
                    .text_xs()
                    .text_color(apagado)
                    .child(div().size(px(8.)).rounded_full().bg(cor_da_conexao))
                    .child(conexao.rotulo()),
            )
            .child(
                marcado(
                    estilo::botao_perigo("chatbot-urgentes", cx),
                    "chatbot-urgentes",
                )
                .gap(px(6.))
                .child("🚨")
                .child(if esperando {
                    "Esperando você"
                } else {
                    "Urgentes"
                })
                .child(
                    div()
                        .px(px(6.))
                        .rounded_full()
                        .bg(gpui::white().opacity(0.25))
                        .text_xs()
                        .child(total.to_string()),
                )
                .on_click(cx.listener(|tela, _, _, cx| tela.abrir_urgencias(cx))),
            )
            .child(
                marcado(estilo::botao_fantasma("chatbot-sino", cx), "chatbot-sino")
                    .child(
                        Icon::new(if ligados { Icone::Bell } else { Icone::BellOff }).size(px(16.)),
                    )
                    .tooltip(move |window, cx| {
                        gpui_component::tooltip::Tooltip::new(if ligados {
                            "Desligar notificações"
                        } else {
                            "Ligar notificações do sistema"
                        })
                        .build(window, cx)
                    })
                    .on_click(cx.listener(|tela, _, _, cx| tela.alternar_avisos(cx))),
            )
    }

    // ── A coluna da lista ──────────────────────────────────────────────────

    fn coluna_da_lista(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (borda, apagado) = (tema.border, tema.muted_foreground);
        let todas = self.todas();
        let contagem = modelo::contagem_por_canal(&todas, self.status, &self.busca_aplicada);
        let total: usize = contagem.iter().map(|(_, n)| n).sum();
        let lista = self.visiveis();
        let aberta = self.aberta.clone();
        let busca_escrita = !self.busca.read(cx).value().is_empty();

        let aba = |id: &'static str,
                   filtro: FiltroDeCanal,
                   rotulo: &'static str,
                   n: usize,
                   cx: &mut Context<Self>| {
            let ativa = self.canal == filtro;
            let botao = marcado(
                if ativa {
                    estilo::botao_primario(id, cx)
                } else {
                    estilo::botao_fantasma(id, cx)
                },
                id,
            );
            botao
                .h(px(26.))
                .px(px(8.))
                .text_xs()
                .gap(px(4.))
                .child(rotulo)
                .child(div().opacity(0.7).child(n.to_string()))
                .on_click(cx.listener(move |tela, _, _, cx| tela.escolher_canal(filtro, cx)))
        };

        let mut abas = vec![aba("aba-todas", FiltroDeCanal::Todas, "Todas", total, cx)];
        for (canal, n) in contagem {
            let id = match canal {
                Canal::WhatsApp => "aba-whatsapp",
                Canal::Instagram => "aba-instagram",
                Canal::Messenger => "aba-messenger",
                Canal::Telegram => "aba-telegram",
                Canal::Web => "aba-site",
            };
            abas.push(aba(id, FiltroDeCanal::So(canal), canal.rotulo(), n, cx));
        }

        let pilulas: Vec<_> = Status::TODOS
            .into_iter()
            .map(|status| {
                let ativa = self.status == status;
                let id = match status {
                    Status::Todas => "status-todas",
                    Status::Bot => "status-bot",
                    Status::Humano => "status-humano",
                    Status::NaoLidas => "status-nao-lidas",
                };
                let botao = marcado(
                    if ativa {
                        estilo::botao_primario(id, cx)
                    } else {
                        estilo::botao_contorno(id, cx)
                    },
                    id,
                );
                botao
                    .h(px(24.))
                    .px(px(8.))
                    .text_xs()
                    .child(status.rotulo())
                    .on_click(cx.listener(move |tela, _, _, cx| tela.escolher_status(status, cx)))
            })
            .collect();

        let paginas = self.paginas();
        let paginado = self.canal != FiltroDeCanal::So(Canal::Instagram)
            && self.pagina_do_whatsapp.total > super::pedidos::POR_PAGINA as i64;

        let corpo: AnyElement = if lista.is_empty() {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .p(px(24.))
                .text_center()
                .text_color(apagado)
                .child(Icon::new(Icone::Inbox).size(px(32.)))
                .child(if !self.carregou && !self.falhou_a_carga {
                    "Carregando…"
                } else if self.falhou_a_carga {
                    "Não foi possível carregar as conversas. Tente de novo; se continuar, a API pode estar fora do ar."
                } else {
                    "Nenhuma conversa bate com esses filtros. As dos cinco canais aparecem aqui juntas, da mais recente para a mais antiga."
                })
                .into_any_element()
        } else {
            v_flex()
                .id("chatbot-lista")
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scroll()
                .children(lista.into_iter().enumerate().map(|(i, conversa)| {
                    let selecionada = aberta.as_ref() == Some(&conversa.chave);
                    let nova = self.novidades.contains(&conversa.chave);
                    self.linha(i, conversa, selecionada, nova, cx)
                }))
                .into_any_element()
        };

        v_flex()
            .w(px(380.))
            .h_full()
            .flex_none()
            .min_h(px(0.))
            .border_r_1()
            .border_color(borda)
            .child(
                v_flex()
                    .p(px(8.))
                    .gap(px(8.))
                    .border_b_1()
                    .border_color(borda)
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .child(
                                // 🔑 O Enter é **consumido** aqui. O campo de uma
                                // linha só repassa a ação; sem ninguém que a
                                // tome, a tecla vira texto — um `\n` num campo
                                // de uma linha, que o GPUI se recusa a desenhar.
                                div()
                                    .flex_1()
                                    .key_context("BuscaDoChatbot")
                                    .on_action(cx.listener(
                                        |tela, _: &gpui_component::input::Enter, _, cx| {
                                            tela.aplicar_busca(cx)
                                        },
                                    ))
                                    .child(Input::new(&self.busca).prefix(
                                        Icon::new(Icone::Search).size(px(16.)).text_color(apagado),
                                    )),
                            )
                            .when(busca_escrita, |d| {
                                d.child(
                                    marcado(
                                        estilo::botao_fantasma("chatbot-limpar-busca", cx),
                                        "chatbot-limpar-busca",
                                    )
                                    .child(Icon::new(Icone::X).size(px(14.)))
                                    .tooltip(|window, cx| {
                                        gpui_component::tooltip::Tooltip::new("Limpar busca")
                                            .build(window, cx)
                                    })
                                    .on_click(cx.listener(
                                        |tela, _, window, cx| tela.limpar_busca(window, cx),
                                    )),
                                )
                            }),
                    )
                    .child(h_flex().flex_wrap().gap(px(2.)).children(abas))
                    .child(h_flex().flex_wrap().gap(px(4.)).children(pilulas)),
            )
            .child(corpo)
            .child(
                h_flex()
                    .h(px(44.))
                    .flex_none()
                    .px(px(12.))
                    .gap(px(8.))
                    .items_center()
                    .justify_between()
                    .border_t_1()
                    .border_color(borda)
                    .text_xs()
                    .child(div().text_color(apagado).truncate().child(format!(
                        "{} na lista{}",
                        self.visiveis().len(),
                        if paginado {
                            format!(" · WhatsApp {}/{}", self.pagina, paginas)
                        } else {
                            String::new()
                        }
                    )))
                    .when(paginado, |d| {
                        d.child(
                            h_flex()
                                .gap(px(4.))
                                .child(estilo::desligado(
                                    marcado(
                                        estilo::botao_contorno("chatbot-anterior", cx),
                                        "chatbot-anterior",
                                    )
                                    .h(px(26.))
                                    .child("Anterior")
                                    .on_click(
                                        cx.listener(|tela, _, _, cx| tela.mudar_pagina(-1, cx)),
                                    ),
                                    self.pagina <= 1,
                                ))
                                .child(estilo::desligado(
                                    marcado(
                                        estilo::botao_contorno("chatbot-proxima", cx),
                                        "chatbot-proxima",
                                    )
                                    .h(px(26.))
                                    .child("Próxima")
                                    .on_click(
                                        cx.listener(|tela, _, _, cx| tela.mudar_pagina(1, cx)),
                                    ),
                                    self.pagina >= paginas,
                                )),
                        )
                    }),
            )
    }

    fn linha(
        &self,
        i: usize,
        conversa: Conversa,
        selecionada: bool,
        nova: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, acento, borda) = (tema.muted_foreground, tema.accent, tema.border);
        let agora = chrono::Utc::now();
        let chave = conversa.chave.clone();
        let mut selos: Vec<AnyElement> = Vec::new();
        if conversa.atendimento_humano {
            selos.push(
                estilo::selo_colorido(crate::tema::cores::selo_ambar())
                    .child("Atendente")
                    .into_any_element(),
            );
        } else {
            selos.push(selo("Bot", cx).into_any_element());
        }
        if let Some(voucher) = conversa.voucher.as_ref().filter(|v| v.has_voucher) {
            selos.push(
                selo(
                    format!(
                        "Voucher {}",
                        if voucher.voucher_used {
                            "usado"
                        } else {
                            "ativo"
                        }
                    ),
                    cx,
                )
                .into_any_element(),
            );
            if voucher.reminders_sent > 0 {
                selos.push(
                    selo(format!("{} lembrete(s)", voucher.reminders_sent), cx).into_any_element(),
                );
            }
        }
        if let Some(cadastro) = conversa.cadastro.as_ref().filter(|c| c.abandoned_carts > 0) {
            selos.push(
                selo(format!("{} carrinho(s)", cadastro.abandoned_carts), cx).into_any_element(),
            );
        }
        if conversa.sem_resposta > 0 {
            selos.push(
                div()
                    .px(px(6.))
                    .rounded(px(6.))
                    .bg(vermelho())
                    .text_color(gpui::white())
                    .text_xs()
                    .child(format!("{} sem resposta", conversa.sem_resposta))
                    .into_any_element(),
            );
        }
        if conversa.so_automaticas {
            selos.push(selo("só automáticas", cx).into_any_element());
        }

        h_flex()
            .id(("chatbot-conversa", i))
            .debug_selector(move || format!("chatbot-conversa-{i}"))
            .relative()
            .w_full()
            .items_start()
            .gap(px(12.))
            .p(px(12.))
            .border_b_1()
            .border_color(borda)
            .cursor_pointer()
            .when(selecionada, |d| d.bg(acento))
            .when(!selecionada, |d| {
                d.hover(move |s| s.bg(acento.opacity(0.5)))
            })
            .when(nova, |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(4.))
                        .bg(verde()),
                )
            })
            .child(avatar(&conversa, 40., false))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .justify_between()
                            .gap(px(8.))
                            .child(
                                h_flex()
                                    .min_w(px(0.))
                                    .gap(px(6.))
                                    .items_center()
                                    .when(nova, |d| {
                                        d.child(
                                            div()
                                                .size(px(8.))
                                                .flex_none()
                                                .rounded_full()
                                                .bg(verde()),
                                        )
                                    })
                                    .child(
                                        Icon::new(icone_do_canal(conversa.chave.canal))
                                            .size(px(14.))
                                            .text_color(apagado),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(conversa.nome.clone()),
                                    ),
                            )
                            .child(
                                div().flex_none().text_xs().text_color(apagado).child(
                                    conversa
                                        .quando
                                        .map(|q| modelo::carimbo(q, agora))
                                        .unwrap_or_default(),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_color(apagado)
                            .child(conversa.previa.clone().unwrap_or_else(|| "—".into())),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .items_center()
                            .children(selos)
                            .when_some(conversa.mensagens, |d, n| {
                                d.child(
                                    div()
                                        .text_xs()
                                        .text_color(apagado)
                                        .child(format!("{n} mensagens")),
                                )
                            }),
                    ),
            )
            .on_click(cx.listener(move |tela, _, _, cx| tela.abrir(chave.clone(), cx)))
    }

    // ── A coluna da conversa ───────────────────────────────────────────────

    fn coluna_da_conversa(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(conversa) = self.conversa_aberta() else {
            let apagado = cx.theme().muted_foreground;
            return v_flex()
                .flex_1()
                .h_full()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .p(px(32.))
                .text_center()
                .child(Icon::new(Icone::MessageSquare).size(px(40.)).text_color(apagado))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Nenhuma conversa aberta"),
                )
                .child(div().text_color(apagado).child(
                    "Escolha um contato à esquerda — WhatsApp, Instagram, Messenger, Telegram e o site, no mesmo lugar.",
                ))
                .into_any_element();
        };
        v_flex()
            .flex_1()
            .h_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .child(self.cabecalho_da_conversa(&conversa, cx))
            .child(self.historico(&conversa, window, cx))
            .child(self.rodape(&conversa, cx))
            .into_any_element()
    }

    fn cabecalho_da_conversa(
        &self,
        conversa: &Conversa,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, borda) = (tema.muted_foreground, tema.border);
        let canal = conversa.chave.canal;
        let atendendo = conversa.atendimento_humano;
        let subtitulo = match canal {
            Canal::WhatsApp => modelo::formatar_whatsapp(&conversa.chave.id),
            Canal::Web => conversa.presenca(),
            _ => conversa.chave.id.clone(),
        };
        let menu = self.mais_acoes.then(|| self.menu_de_acoes(canal, cx));

        h_flex()
            .relative()
            .h(px(56.))
            .flex_none()
            .px(px(12.))
            .gap(px(8.))
            .items_center()
            .border_b_1()
            .border_color(borda)
            .child(avatar(conversa, 36., true))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .items_center()
                            .child(
                                Icon::new(icone_do_canal(canal))
                                    .size(px(14.))
                                    .text_color(apagado),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(conversa.nome.clone()),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .text_xs()
                            .text_color(apagado)
                            .child(div().truncate().child(subtitulo))
                            .child("·")
                            .child(if atendendo {
                                h_flex()
                                    .gap(px(4.))
                                    .items_center()
                                    .text_color(ambar())
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(div().size(px(8.)).rounded_full().bg(ambar()))
                                    .child("você está atendendo")
                            } else {
                                h_flex().child("bot ativo")
                            }),
                    ),
            )
            .child(
                if atendendo {
                    marcado(
                        estilo::botao_primario("chatbot-alternar", cx),
                        "chatbot-alternar",
                    )
                    .gap(px(4.))
                    .child(Icon::new(Icone::Bot).size(px(16.)))
                    .child("Devolver ao bot")
                } else {
                    marcado(
                        estilo::botao_contorno("chatbot-alternar", cx),
                        "chatbot-alternar",
                    )
                    .gap(px(4.))
                    .child(Icon::new(Icone::Hand).size(px(16.)))
                    .child("Assumir")
                }
                .tooltip(move |window, cx| {
                    gpui_component::tooltip::Tooltip::new(if atendendo {
                        "Devolver a conversa ao bot"
                    } else if canal == Canal::WhatsApp {
                        "Pausar o bot neste contato e responder à mão"
                    } else {
                        "Assumir a conversa e calar o bot"
                    })
                    .build(window, cx)
                })
                .on_click(cx.listener(|tela, _, window, cx| tela.alternar_atendimento(window, cx))),
            )
            .child(
                marcado(
                    estilo::botao_fantasma("chatbot-mais-acoes", cx),
                    "chatbot-mais-acoes",
                )
                .child(Icon::new(Icone::EllipsisVertical).size(px(16.)))
                .tooltip(|window, cx| {
                    gpui_component::tooltip::Tooltip::new("Mais ações").build(window, cx)
                })
                .on_click(cx.listener(|tela, _, _, cx| tela.alternar_mais_acoes(cx))),
            )
            .children(menu)
    }

    fn menu_de_acoes(&self, canal: Canal, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (popover, borda, acento, perigo) =
            (tema.popover, tema.border, tema.accent, tema.danger);
        let item = |id: &'static str, rotulo: &'static str| {
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .px(px(8.))
                .py(px(6.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(move |s| s.bg(acento))
                .child(rotulo)
        };
        v_flex()
            .absolute()
            .top(px(48.))
            .right(px(8.))
            .w(px(220.))
            .p(px(4.))
            .rounded(px(8.))
            .border_1()
            .border_color(borda)
            .bg(popover)
            .shadow_lg()
            .occlude()
            .child(
                item(
                    "chatbot-copiar",
                    match canal {
                        Canal::WhatsApp => "Copiar número",
                        Canal::Instagram => "Copiar IGSID",
                        _ => "Copiar ID da conversa",
                    },
                )
                .on_click(cx.listener(|tela, _, _, cx| tela.copiar_id(cx))),
            )
            .when(canal == Canal::WhatsApp, |d| {
                d.child(
                    item("chatbot-excluir-historico", "Excluir histórico")
                        .text_color(perigo)
                        .on_click(
                            cx.listener(|tela, _, window, cx| tela.pedir_exclusao(window, cx)),
                        ),
                )
            })
    }

    fn historico(
        &mut self,
        conversa: &Conversa,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let _ = window;
        let tema = cx.theme();
        let (apagado, muted, borda, fundo) = (
            tema.muted_foreground,
            tema.muted,
            tema.border,
            tema.background,
        );
        let (mensagens, escondidas) = self.mensagens_da_aberta();
        let pendentes = self.pendentes_da_aberta();
        let assinatura = (conversa.chave.clone(), mensagens.len(), pendentes.len());
        if self.rolagem_vista.as_ref() != Some(&assinatura) {
            self.rolagem.scroll_to_bottom();
            self.rolagem_vista = Some(assinatura);
        }
        let agora = chrono::Utc::now();
        let canal = conversa.chave.canal;

        let mut linhas: Vec<AnyElement> = Vec::new();
        if escondidas > 0 && !self.tudo {
            linhas.push(
                h_flex()
                    .justify_center()
                    .child(
                        marcado(
                            estilo::botao_contorno("chatbot-anteriores", cx),
                            "chatbot-anteriores",
                        )
                        .child(format!("Carregar mensagens anteriores ({escondidas})"))
                        .on_click(cx.listener(|tela, _, _, cx| tela.carregar_anteriores(cx))),
                    )
                    .into_any_element(),
            );
        }
        if mensagens.is_empty() && pendentes.is_empty() && !self.carregando_historico {
            linhas.push(
                div()
                    .py(px(48.))
                    .text_center()
                    .text_color(apagado)
                    .child("Nenhuma mensagem para mostrar.")
                    .into_any_element(),
            );
        }
        let mut anterior: Option<&Mensagem> = None;
        for mensagem in &mensagens {
            if let Some(quando) = mensagem.quando {
                if modelo::mudou_o_dia(quando, anterior.and_then(|a| a.quando)) {
                    linhas.push(
                        h_flex()
                            .justify_center()
                            .mt(px(12.))
                            .child(
                                div()
                                    .px(px(12.))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(borda)
                                    .bg(fundo)
                                    .text_xs()
                                    .text_color(apagado)
                                    .child(modelo::rotulo_do_dia(quando, agora)),
                            )
                            .into_any_element(),
                    );
                }
            }
            linhas.push(balao(mensagem, canal, cx).into_any_element());
            anterior = Some(mensagem);
        }
        for pendente in pendentes {
            linhas.push(self.balao_pendente(pendente, canal, cx).into_any_element());
        }

        let respostas = self.respostas_abertas.then(|| self.painel_de_respostas(cx));

        div()
            .relative()
            .flex_1()
            .min_h(px(0.))
            .bg(muted.opacity(0.3))
            .child(
                v_flex()
                    .id("chatbot-historico")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.rolagem)
                    .p(px(12.))
                    .gap(px(4.))
                    .children(linhas),
            )
            .children(respostas)
            .when(self.carregando_historico, |d| {
                d.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(fundo.opacity(0.6))
                        .text_color(apagado)
                        .child("Carregando…"),
                )
            })
    }

    fn painel_de_respostas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (fundo, borda, apagado, acento) = (
            tema.background,
            tema.border,
            tema.muted_foreground,
            tema.accent,
        );
        let itens: Vec<AnyElement> = if self.respostas.is_empty() {
            vec![div()
                .text_color(apagado)
                .child("Nenhuma resposta cadastrada.")
                .into_any_element()]
        } else {
            self.respostas
                .iter()
                .enumerate()
                .map(|(i, resposta)| {
                    let r = resposta.clone();
                    v_flex()
                        .id(("chatbot-resposta", i))
                        .debug_selector(move || format!("chatbot-resposta-{i}"))
                        .w_full()
                        .p(px(8.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(borda)
                        .cursor_pointer()
                        .hover(move |s| s.bg(acento))
                        .child(
                            h_flex()
                                .justify_between()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(resposta.titulo.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(apagado)
                                        .child(format!("{} usos", resposta.usos)),
                                ),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(apagado)
                                .child(resposta.mensagem.clone()),
                        )
                        .on_click(cx.listener(move |tela, _, _, cx| tela.enviar_resposta(&r, cx)))
                        .into_any_element()
                })
                .collect()
        };
        v_flex()
            .id("chatbot-respostas")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .max_h(px(224.))
            .overflow_y_scroll()
            .p(px(12.))
            .gap(px(8.))
            .border_b_1()
            .border_color(borda)
            .bg(fundo)
            .shadow_md()
            .occlude()
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child("Respostas rápidas"),
            )
            .children(itens)
    }

    fn balao_pendente(
        &self,
        pendente: Pendente,
        canal: Canal,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let perigo = cx.theme().danger;
        let id = pendente.id;
        let falhou = pendente.erro.is_some();
        h_flex().justify_end().child(
            v_flex()
                .max_w(px(480.))
                .px(px(12.))
                .py(px(6.))
                .rounded(px(10.))
                .bg(cor_do_canal(canal).opacity(0.15))
                .when(falhou, |d| d.border_1().border_color(perigo))
                .child(div().child(pendente.texto.clone()))
                .child(
                    h_flex()
                        .justify_end()
                        .gap(px(4.))
                        .text_xs()
                        .text_color(if falhou {
                            perigo
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(if falhou { "não enviada" } else { "enviando" }),
                )
                .when_some(pendente.erro.clone(), |d, erro| {
                    d.child(
                        h_flex()
                            .mt(px(6.))
                            .pt(px(6.))
                            .gap(px(8.))
                            .justify_between()
                            .border_t_1()
                            .border_color(perigo.opacity(0.3))
                            .child(div().truncate().text_xs().text_color(perigo).child(erro))
                            .child(
                                h_flex()
                                    .gap(px(4.))
                                    .child(
                                        marcado(
                                            estilo::botao_contorno(
                                                format!("chatbot-tentar-{}", id),
                                                cx,
                                            ),
                                            format!("chatbot-tentar-{}", id),
                                        )
                                        .h(px(24.))
                                        .text_xs()
                                        .child(Icon::new(Icone::RotateCw).size(px(12.)))
                                        .child("Tentar de novo")
                                        .on_click(
                                            cx.listener(move |tela, _, _, cx| {
                                                tela.reenviar(id, cx)
                                            }),
                                        ),
                                    )
                                    .child(
                                        marcado(
                                            estilo::botao_fantasma(
                                                format!("chatbot-descartar-{}", id),
                                                cx,
                                            ),
                                            format!("chatbot-descartar-{}", id),
                                        )
                                        .h(px(24.))
                                        .text_xs()
                                        .child("Descartar")
                                        .on_click(
                                            cx.listener(move |tela, _, _, cx| {
                                                tela.descartar(id, cx)
                                            }),
                                        ),
                                    ),
                            ),
                    )
                }),
        )
    }

    fn rodape(&self, conversa: &Conversa, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (borda, apagado, muted) = (tema.border, tema.muted_foreground, tema.muted);
        let decisao = modelo::decisao_do_compositor(
            conversa.chave.canal,
            self.janela_da_aberta(chrono::Utc::now()),
        );
        if let Some(motivo) = decisao.motivo {
            return h_flex()
                .flex_none()
                .min_h(px(56.))
                .p(px(12.))
                .gap(px(8.))
                .items_start()
                .border_t_1()
                .border_color(borda)
                .bg(muted.opacity(0.4))
                .text_xs()
                .text_color(apagado)
                .child(Icon::new(Icone::CircleAlert).size(px(16.)))
                .child(div().flex_1().child(motivo))
                .into_any_element();
        }
        let vazio = self.compositor.read(cx).value().trim().is_empty();
        v_flex()
            .flex_none()
            .border_t_1()
            .border_color(borda)
            .when_some(decisao.aviso, |d, aviso| {
                d.child(
                    div()
                        .px(px(12.))
                        .py(px(4.))
                        .text_xs()
                        .bg(ambar().opacity(0.1))
                        .text_color(ambar())
                        .child(aviso),
                )
            })
            .child(
                h_flex()
                    .key_context(COMPOSITOR)
                    .on_action(cx.listener(|tela, _: &EnviarMensagem, window, cx| {
                        tela.enviar_o_escrito(window, cx)
                    }))
                    .p(px(8.))
                    .gap(px(8.))
                    .items_end()
                    .child({
                        let botao = if self.respostas_abertas {
                            marcado(estilo::botao_primario("chatbot-raio", cx), "chatbot-raio")
                        } else {
                            marcado(estilo::botao_fantasma("chatbot-raio", cx), "chatbot-raio")
                        };
                        botao
                            .child(Icon::new(Icone::Zap).size(px(16.)))
                            .tooltip(|window, cx| {
                                gpui_component::tooltip::Tooltip::new("Respostas rápidas")
                                    .build(window, cx)
                            })
                            .on_click(cx.listener(|tela, _, _, cx| tela.alternar_respostas(cx)))
                    })
                    .child(div().flex_1().child(Input::new(&self.compositor)))
                    .child(estilo::desligado(
                        marcado(
                            estilo::botao_primario("chatbot-enviar", cx),
                            "chatbot-enviar",
                        )
                        .child(Icon::new(Icone::Send).size(px(16.)))
                        .tooltip(|window, cx| {
                            gpui_component::tooltip::Tooltip::new("Enviar").build(window, cx)
                        })
                        .on_click(
                            cx.listener(|tela, _, window, cx| tela.enviar_o_escrito(window, cx)),
                        ),
                        vazio,
                    )),
            )
            .into_any_element()
    }

    // ── Diálogos ───────────────────────────────────────────────────────────

    fn dialogo_aberto(&self, dialogo: Dialogo, cx: &mut Context<Self>) -> AnyElement {
        let caixa = match dialogo {
            Dialogo::Urgencias => self.dialogo_de_urgencias(cx).into_any_element(),
            Dialogo::Resolver(urgencia) => self.dialogo_de_resolucao(&urgencia, cx).into_any_element(),
            Dialogo::Descartar(_) => estilo::caixa_do_dialogo(cx)
                .child(estilo::cabecalho_do_dialogo(
                    "Descartar este alerta?",
                    "Ele é marcado como falso positivo e sai da lista.",
                    None,
                    cx,
                ))
                .child(
                    estilo::rodape_do_dialogo()
                        .child(
                            marcado(estilo::botao_contorno("descartar-cancelar", cx), "descartar-cancelar")
                                .child("Cancelar")
                                .on_click(cx.listener(|tela, _, _, cx| tela.fechar_dialogo(cx))),
                        )
                        .child(estilo::desligado(
                            marcado(estilo::botao_perigo("descartar-confirmar", cx), "descartar-confirmar")
                                .child("Descartar")
                                .on_click(
                                    cx.listener(|tela, _, _, cx| tela.confirmar_descarte(cx)),
                                ),
                            self.em_acao,
                        )),
                )
                .into_any_element(),
            Dialogo::QuemAssume(_) => estilo::caixa_do_dialogo(cx)
                .w(px(384.))
                .child(estilo::cabecalho_do_dialogo(
                    "Quem está assumindo?",
                    "O visitante lê “fulano entrou na conversa” — no site ele não tem outro jeito de saber com quem está falando.",
                    None,
                    cx,
                ))
                .child(
                    div()
                        .key_context("QuemAssume")
                        .on_action(cx.listener(
                            |tela, _: &gpui_component::input::Enter, window, cx| {
                                tela.confirmar_quem_assume(window, cx)
                            },
                        ))
                        .child(Input::new(&self.nome_do_atendente)),
                )
                .child(
                    estilo::rodape_do_dialogo()
                        .child(
                            marcado(estilo::botao_fantasma("quem-cancelar", cx), "quem-cancelar")
                                .child("Cancelar")
                                .on_click(cx.listener(|tela, _, _, cx| tela.fechar_dialogo(cx))),
                        )
                        .child(
                            marcado(estilo::botao_primario("quem-assumir", cx), "quem-assumir")
                                .child("Assumir conversa")
                                .on_click(cx.listener(|tela, _, window, cx| {
                                    tela.confirmar_quem_assume(window, cx)
                                })),
                        ),
                )
                .into_any_element(),
            Dialogo::ExcluirHistorico(chave) => estilo::caixa_do_dialogo(cx)
                .child(estilo::cabecalho_do_dialogo(
                    "Excluir todo o histórico",
                    format!(
                        "Apaga todas as mensagens de {} do banco. Não dá para desfazer, e a conversa some também do painel de WhatsApp — é a mesma tabela.",
                        modelo::formatar_whatsapp(&chave.id)
                    ),
                    None,
                    cx,
                ))
                .child(
                    v_flex()
                        .gap(px(6.))
                        .child(format!(
                            "Digite {} para confirmar",
                            super::pedidos::CONFIRMACAO_DE_EXCLUSAO
                        ))
                        .child(Input::new(&self.confirmacao)),
                )
                .child(
                    estilo::rodape_do_dialogo()
                        .child(estilo::desligado(
                            marcado(estilo::botao_contorno("excluir-cancelar", cx), "excluir-cancelar")
                                .child("Cancelar")
                                .on_click(cx.listener(|tela, _, _, cx| tela.fechar_dialogo(cx))),
                            self.em_acao,
                        ))
                        .child(estilo::desligado(
                            marcado(estilo::botao_perigo("excluir-confirmar", cx), "excluir-confirmar")
                                .child(if self.em_acao { "Excluindo…" } else { "Excluir histórico" })
                                .on_click(cx.listener(|tela, _, _, cx| tela.excluir_historico(cx))),
                            self.em_acao || !self.exclusao_confirmada(cx),
                        )),
                )
                .into_any_element(),
        };
        estilo::veu_do_dialogo()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|tela, _, _, cx| tela.fechar_dialogo(cx)),
            )
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(caixa),
            )
            .into_any_element()
    }

    fn dialogo_de_urgencias(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, borda) = (tema.muted_foreground, tema.border);
        let linhas: Vec<AnyElement> = self
            .urgencias
            .iter()
            .enumerate()
            .map(|(i, u)| self.linha_de_urgencia(i, u, cx).into_any_element())
            .collect();
        estilo::caixa_do_dialogo(cx)
            .w(px(720.))
            .max_h(px(640.))
            .child(estilo::cabecalho_do_dialogo(
                "🚨 Interações urgentes",
                format!(
                    "Clientes esperando atendimento humano — {} no total.",
                    self.urgencias.len()
                ),
                None,
                cx,
            ))
            .child(
                v_flex()
                    .id("chatbot-urgencias")
                    .max_h(px(520.))
                    .overflow_y_scroll()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(if self.urgencias.is_empty() {
                        borda
                    } else {
                        vermelho().opacity(0.5)
                    })
                    .when(self.urgencias.is_empty(), |d| {
                        d.child(
                            div()
                                .p(px(16.))
                                .text_color(apagado)
                                .child("Nenhuma urgência no momento."),
                        )
                    })
                    .children(linhas),
            )
            .child(
                estilo::rodape_do_dialogo().child(
                    marcado(
                        estilo::botao_contorno("urgencias-fechar", cx),
                        "urgencias-fechar",
                    )
                    .child("Fechar")
                    .on_click(cx.listener(|tela, _, _, cx| tela.fechar_dialogo(cx))),
                ),
            )
    }

    fn linha_de_urgencia(
        &self,
        i: usize,
        u: &Urgencia,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tema = cx.theme();
        let (apagado, borda) = (tema.muted_foreground, tema.border);
        let cor = match u.priority_level.as_str() {
            "CRITICAL" => vermelho(),
            "HIGH" => rgb(0xf97316).into(),
            "MEDIUM" => rgb(0xeab308).into(),
            _ => rgb(0x3b82f6).into(),
        };
        let (a, r, d, v) = (u.clone(), u.clone(), u.clone(), u.clone());
        h_flex()
            .items_start()
            .justify_between()
            .gap(px(16.))
            .p(px(16.))
            .border_b_1()
            .border_color(borda)
            .border_l_4()
            .border_color(cor)
            .bg(cor.opacity(0.06))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(4.))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap(px(8.))
                            .items_center()
                            .child(selo(u.rotulo_da_prioridade(), cx))
                            .when(u.pediu_atendente(), |d| {
                                d.child(div().size(px(8.)).rounded_full().bg(vermelho()))
                            })
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(u.profile_name.clone()),
                            )
                            .child(div().text_color(apagado).child(u.contact_id.clone()))
                            .when(u.em_atendimento(), |d| {
                                d.child(
                                    estilo::selo_colorido(crate::tema::cores::selo_ceu())
                                        .child("Em atendimento"),
                                )
                            }),
                    )
                    .child(format!("{} · urgência {}/10", u.motivo(), u.urgency_score))
                    .when_some(u.context_summary.clone(), |d, resumo| {
                        d.child(
                            div()
                                .italic()
                                .text_color(apagado)
                                .child(format!("“{resumo}”")),
                        )
                    })
                    .child(div().text_xs().text_color(apagado).child(format!(
                        "Aguardando {} · {} mensagens recentes",
                        u.espera(),
                        u.message_count
                    ))),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .when(u.pendente(), |d| {
                        d.child(estilo::desligado(
                            marcado(
                                estilo::botao_contorno(format!("urgencia-assumir-{}", i), cx),
                                format!("urgencia-assumir-{}", i),
                            )
                            .child("Assumir")
                            .on_click(
                                cx.listener(move |tela, _, _, cx| tela.assumir_urgencia(&a, cx)),
                            ),
                            self.em_acao,
                        ))
                    })
                    .child(
                        marcado(
                            estilo::botao_primario(format!("urgencia-resolver-{}", i), cx),
                            format!("urgencia-resolver-{}", i),
                        )
                        .child("Resolver")
                        .on_click(cx.listener(
                            move |tela, _, window, cx| tela.pedir_resolucao(r.clone(), window, cx),
                        )),
                    )
                    .child(estilo::desligado(
                        marcado(
                            estilo::botao_contorno(format!("urgencia-descartar-{}", i), cx),
                            format!("urgencia-descartar-{}", i),
                        )
                        .child("Descartar")
                        .on_click(
                            cx.listener(move |tela, _, _, cx| tela.pedir_descarte(d.clone(), cx)),
                        ),
                        self.em_acao,
                    ))
                    .child(
                        marcado(
                            estilo::botao_contorno(format!("urgencia-ver-{}", i), cx),
                            format!("urgencia-ver-{}", i),
                        )
                        .child("Ver conversa")
                        .on_click(
                            cx.listener(move |tela, _, _, cx| {
                                tela.ver_conversa_da_urgencia(&v, cx)
                            }),
                        ),
                    ),
            )
    }

    fn dialogo_de_resolucao(
        &self,
        urgencia: &Urgencia,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let apagado = cx.theme().muted_foreground;
        let sem_notas = self.notas.read(cx).value().trim().is_empty();
        estilo::caixa_do_dialogo(cx)
            .child(estilo::cabecalho_do_dialogo(
                "Resolver urgência",
                format!("{} · {}", urgencia.profile_name, urgencia.contact_id),
                None,
                cx,
            ))
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(div().font_weight(FontWeight::MEDIUM).child("Notas de resolução *"))
                    .child(div().text_xs().text_color(apagado).child(
                        "Como a situação foi resolvida. É o que fica no histórico do contato — e o que explica, meses depois, por que esta urgência saiu da fila.",
                    ))
                    .child(Input::new(&self.notas)),
            )
            .child(
                estilo::rodape_do_dialogo()
                    .child(
                        marcado(estilo::botao_contorno("resolver-cancelar", cx), "resolver-cancelar")
                            .child("Cancelar")
                            .on_click(cx.listener(|tela, _, _, cx| tela.fechar_dialogo(cx))),
                    )
                    .child(estilo::desligado(
                        marcado(estilo::botao_primario("resolver-confirmar", cx), "resolver-confirmar")
                            .child(if self.em_acao { "Resolvendo…" } else { "Confirmar resolução" })
                            .on_click(cx.listener(|tela, _, _, cx| tela.confirmar_resolucao(cx))),
                        self.em_acao || sem_notas,
                    )),
            )
    }
}

/// Um balão do histórico: do cliente à esquerda, do estúdio à direita, com a
/// hora e — no WhatsApp — o selo de automática.
fn balao(mensagem: &Mensagem, canal: Canal, cx: &mut Context<Chatbot>) -> impl IntoElement {
    let tema = cx.theme();
    let (fundo, apagado, borda) = (tema.background, tema.muted_foreground, tema.border);
    let hora = mensagem.quando.map(modelo::hora).unwrap_or_default();
    let automacao = mensagem.rotulo_da_automacao();
    h_flex().when(mensagem.saida, |d| d.justify_end()).child(
        v_flex()
            .max_w(px(480.))
            .px(px(12.))
            .py(px(6.))
            .rounded(px(10.))
            .when(mensagem.saida, |d| d.bg(cor_do_canal(canal).opacity(0.15)))
            .when(!mensagem.saida, |d| {
                d.bg(fundo).border_1().border_color(borda)
            })
            .child(div().child(mensagem.texto.clone()))
            .child(
                h_flex()
                    .justify_end()
                    .gap(px(6.))
                    .text_xs()
                    .text_color(apagado)
                    .when_some(automacao, |d, rotulo| d.child(div().italic().child(rotulo)))
                    .child(hora),
            ),
    )
}
