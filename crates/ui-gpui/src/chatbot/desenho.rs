//! O desenho do painel do chatbot — a tela do site (`page.tsx`, `painel.tsx`,
//! `conversa.tsx`, `baloes.tsx`, `urgencias.tsx`) com as mesmas palavras.
//!
//! Duas colunas: a lista dos cinco canais à esquerda, a conversa à direita.
//! Por cima, os diálogos (urgências, resolver, descartar, quem assume,
//! excluir histórico).

use crate::campo::TrocarValor as _;
use gpui_kit::component::avatar::Avatar;
use gpui_kit::component::badge::Badge;
use gpui_kit::component::bubble::{Bubble, BubbleContent, BubbleVariant};
use gpui_kit::component::empty::{
    Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle,
};
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::list::ListItem;
use gpui_kit::component::marker::{Marker, MarkerContent, MarkerVariant};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::message::{Message, MessageAlignment, MessageContent, MessageFooter};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, rgb, AnyElement, Context, FontWeight, Hsla, SharedString, Window,
};

use super::midia::{self, Midia, TipoDeMidia};
use super::modelo::{self, Canal, Conversa, FiltroDeCanal, Mensagem, Status, Urgencia};
use super::tela::{Chatbot, Dialogo, Miniatura, Pendente};
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

/// O avatar: o `Avatar` do gpui-kit com as iniciais; o crachá do canto traz o
/// ícone do canal na cor dele (no WhatsApp, a do volume sem resposta) e o anel
/// âmbar diz que alguém assumiu.
fn avatar(conversa: &Conversa, lado: f32) -> impl IntoElement {
    let cor = if conversa.chave.canal == Canal::WhatsApp {
        match conversa.prioridade() {
            modelo::Prioridade::Alta => vermelho(),
            modelo::Prioridade::Media => rgb(0xeab308).into(),
            modelo::Prioridade::Baixa => verde(),
        }
    } else {
        cor_do_canal(conversa.chave.canal)
    };
    div().flex_none().child(
        Badge::new()
            .icon(Icon::new(icone_do_canal(conversa.chave.canal)).size(px(10.)))
            .color(cor)
            .child(
                Avatar::new()
                    .name(conversa.nome.clone())
                    .with_size(gpui_kit::component::Size::Size(px(lado)))
                    .when(conversa.atendimento_humano, |a| {
                        a.border_2().border_color(ambar())
                    }),
            ),
    )
}

/// O estado vazio: o `Empty` do gpui-kit, com o ícone no quadro do kit, o
/// título (quando há) e a frase que explica.
fn vazio(icone: Icone, titulo: Option<&'static str>, texto: &'static str) -> Empty {
    let mut cabecalho = EmptyHeader::new().media(
        EmptyMedia::new()
            .with_variant(EmptyMediaVariant::Icon)
            .child(Icon::new(icone).size(px(20.))),
    );
    if let Some(titulo) = titulo {
        cabecalho = cabecalho.title(EmptyTitle::new().child(titulo));
    }
    Empty::new().header(cabecalho.description(EmptyDescription::new().child(texto)))
}

/// Marca o botão para o teste clicar onde o dedo clica (`debug_selector`).
fn marcado(
    botao: gpui_kit::component::button::Button,
    nome: impl Into<String>,
) -> gpui_kit::component::button::Button {
    let nome = nome.into();
    botao.debug_selector(move || nome.clone())
}

fn selo(texto: impl Into<SharedString>, cx: &gpui_kit::App) -> gpui_kit::component::tag::Tag {
    estilo::selo_contorno(cx).child(texto.into())
}

impl Render for Chatbot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.limpar_compositor) {
            self.compositor
                .update(cx, |campo, cx| campo.trocar_valor("", window, cx));
        }
        let tema = cx.theme();
        let (fundo, texto) = (tema.background, tema.foreground);
        // 🪟 Os diálogos são o `Dialog` do gpui-kit (`crate::dialogo`), com a
        // largura de cada um.
        let aberto = self.dialogo.aberto().cloned();
        let largura = match &aberto {
            Some(Dialogo::Urgencias) => 720.,
            Some(Dialogo::QuemAssume(_)) => 384.,
            _ => 440.,
        };
        let conteudo = aberto.map(|d| self.dialogo_aberto(d, cx));
        let dialogo = crate::dialogo::desenhar_conteudo(
            conteudo,
            None,
            crate::dialogo::Jeito {
                largura,
                esc: true,
                veu: true,
                x: false,
            },
            |tela, window, cx| tela.fechar_dialogo(window, cx),
            window,
            cx,
        );

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
            .child(
                h_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .items_start()
                    .rounded(crate::tema::canto(12.))
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

    /// O assunto do chatbot na faixa do cabeçalho do app: o título, o resumo
    /// da lista, a conexão em tempo real, as urgências e o sino.
    pub(crate) fn no_cabecalho(&self, cx: &mut Context<Self>) -> gpui_kit::Div {
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

        let acoes = h_flex()
            .gap(px(12.))
            .items_center()
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
                    Tag::custom(
                        gpui_kit::white().opacity(0.25),
                        gpui_kit::white(),
                        gpui_kit::transparent_black(),
                    )
                    .rounded_full()
                    .child(total.to_string()),
                )
                .on_click(cx.listener(|tela, _, window, cx| tela.abrir_urgencias(window, cx))),
            )
            .child(
                marcado(estilo::botao_fantasma("chatbot-sino", cx), "chatbot-sino")
                    .child(
                        Icon::new(if ligados { Icone::Bell } else { Icone::BellOff }).size(px(16.)),
                    )
                    .tooltip(if ligados {
                        "Desligar notificações"
                    } else {
                        "Ligar notificações do sistema"
                    })
                    .on_click(cx.listener(|tela, _, _, cx| tela.alternar_avisos(cx))),
            );

        estilo::assunto_do_cabecalho(
            "Chatbot",
            Some(self.linha_de_resumo()),
            None,
            Some(acoes.into_any_element()),
            cx,
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

        // 🗂️ Canal e status são `TabBar` do gpui-kit (uma escolha só): o canal
        // em pílulas, com a contagem ao lado do nome; o status, segmentado.
        let tela = cx.entity().downgrade();
        let mut canais: Vec<(&'static str, FiltroDeCanal, &'static str, usize)> =
            vec![("aba-todas", FiltroDeCanal::Todas, "Todas", total)];
        for (canal, n) in contagem {
            let id = match canal {
                Canal::WhatsApp => "aba-whatsapp",
                Canal::Instagram => "aba-instagram",
                Canal::Messenger => "aba-messenger",
                Canal::Telegram => "aba-telegram",
                Canal::Web => "aba-site",
            };
            canais.push((id, FiltroDeCanal::So(canal), canal.rotulo(), n));
        }
        let filtros: Vec<FiltroDeCanal> = canais.iter().map(|c| c.1).collect();
        let escolhido = filtros.iter().position(|f| *f == self.canal).unwrap_or(0);
        let abas = TabBar::new("chatbot-canais")
            .pill()
            .xsmall()
            .menu(true)
            .selected_index(escolhido)
            .children(canais.into_iter().map(|(id, _, rotulo, n)| {
                Tab::new()
                    .label(rotulo)
                    .when(n > 0, |t| t.suffix(div().opacity(0.7).child(n.to_string())))
                    .debug_selector(move || id.to_string())
            }))
            .on_click({
                let tela = tela.clone();
                move |i, _, cx| {
                    let Some(filtro) = filtros.get(*i).copied() else {
                        return;
                    };
                    let _ = tela.update(cx, |tela, cx| tela.escolher_canal(filtro, cx));
                }
            });

        let situacoes = Status::TODOS;
        let status_escolhido = situacoes
            .iter()
            .position(|s| *s == self.status)
            .unwrap_or(0);
        let pilulas = TabBar::new("chatbot-status")
            .segmented()
            .xsmall()
            .w_full()
            .selected_index(status_escolhido)
            .children(situacoes.into_iter().map(|status| {
                let id = match status {
                    Status::Todas => "status-todas",
                    Status::Bot => "status-bot",
                    Status::Humano => "status-humano",
                    Status::NaoLidas => "status-nao-lidas",
                };
                Tab::new()
                    .label(status.rotulo())
                    .flex_1()
                    .debug_selector(move || id.to_string())
            }))
            .on_click(move |i, _, cx| {
                let Some(status) = situacoes.get(*i).copied() else {
                    return;
                };
                let _ = tela.update(cx, |tela, cx| tela.escolher_status(status, cx));
            });

        let paginas = self.paginas();
        let paginado = self.canal != FiltroDeCanal::So(Canal::Instagram)
            && self.pagina_do_whatsapp.total > super::pedidos::POR_PAGINA as i64;

        let corpo: AnyElement = if lista.is_empty() {
            vazio(
                Icone::Inbox,
                None,
                if !self.carregou && !self.falhou_a_carga {
                    "Carregando…"
                } else if self.falhou_a_carga {
                    "Não foi possível carregar as conversas. Tente de novo; se continuar, a API pode estar fora do ar."
                } else {
                    "Nenhuma conversa bate com esses filtros. As dos cinco canais aparecem aqui juntas, da mais recente para a mais antiga."
                },
            )
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
                                        |tela, _: &gpui_kit::component::input::Enter, _, cx| {
                                            tela.aplicar_busca(cx)
                                        },
                                    ))
                                    .child(crate::estilo::campo(Input::new(&self.busca)).prefix(
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
                                    .tooltip("Limpar busca")
                                    .on_click(cx.listener(
                                        |tela, _, window, cx| tela.limpar_busca(window, cx),
                                    )),
                                )
                            }),
                    )
                    .child(abas)
                    .child(pilulas),
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
                                        estilo::botao_contorno_pequeno("chatbot-anterior", cx),
                                        "chatbot-anterior",
                                    )
                                    .child("Anterior")
                                    .on_click(
                                        cx.listener(|tela, _, _, cx| tela.mudar_pagina(-1, cx)),
                                    ),
                                    self.pagina <= 1,
                                ))
                                .child(estilo::desligado(
                                    marcado(
                                        estilo::botao_contorno_pequeno("chatbot-proxima", cx),
                                        "chatbot-proxima",
                                    )
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
        let (apagado, borda) = (tema.muted_foreground, tema.border);
        let agora = chrono::Utc::now();
        let chave = conversa.chave.clone();
        let tela = cx.entity().downgrade();
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
                estilo::selo_perigo()
                    .child(format!("{} sem resposta", conversa.sem_resposta))
                    .into_any_element(),
            );
        }
        if conversa.so_automaticas {
            selos.push(selo("só automáticas", cx).into_any_element());
        }

        // 📋 A linha é um `ListItem` do gpui-kit (selecionada, hover e clique do
        // kit); dentro dele, o avatar e o texto lado a lado.
        ListItem::new(("chatbot-conversa", i))
            .debug_selector(move || format!("chatbot-conversa-{i}"))
            .selected(selecionada)
            .w_full()
            .p(px(12.))
            .border_b_1()
            .border_color(borda)
            .text_sm()
            .on_click(move |_, _, cx| {
                let _ = tela.update(cx, |tela, cx| tela.abrir(chave.clone(), cx));
            })
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap(px(12.))
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
                    .child(avatar(&conversa, 40.))
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
                    ),
            )
    }

    // ── A coluna da conversa ───────────────────────────────────────────────

    fn coluna_da_conversa(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(conversa) = self.conversa_aberta() else {
            return vazio(
                Icone::MessageSquare,
                Some("Nenhuma conversa aberta"),
                "Escolha um contato à esquerda — WhatsApp, Instagram, Messenger, Telegram e o site, no mesmo lugar.",
            )
            .h_full()
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

        h_flex()
            .relative()
            .h(px(56.))
            .flex_none()
            .px(px(12.))
            .gap(px(8.))
            .items_center()
            .border_b_1()
            .border_color(borda)
            .child(avatar(conversa, 36.))
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
                .tooltip(if atendendo {
                    "Devolver a conversa ao bot"
                } else if canal == Canal::WhatsApp {
                    "Pausar o bot neste contato e responder à mão"
                } else {
                    "Assumir a conversa e calar o bot"
                })
                .on_click(cx.listener(|tela, _, window, cx| tela.alternar_atendimento(window, cx))),
            )
            .child(
                marcado(
                    estilo::botao_fantasma("chatbot-mais-acoes", cx),
                    "chatbot-mais-acoes",
                )
                .child(Icon::new(Icone::EllipsisVertical).size(px(16.)))
                .tooltip("Mais ações")
                .map(|botao| self.menu_de_acoes(botao, canal, cx)),
            )
    }

    /// "Mais ações": o `DropdownMenu` do gpui-kit, preso ao botão ⋮.
    fn menu_de_acoes(
        &self,
        botao: gpui_kit::component::button::Button,
        canal: Canal,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let perigo = cx.theme().danger;
        let tela = cx.entity().downgrade();
        botao.dropdown_menu_with_anchor(gpui_kit::Anchor::TopRight, move |menu, window, cx| {
            let menu = match window.focused(cx) {
                Some(antes) => menu.action_context(antes),
                None => menu,
            };
            let copiar = tela.clone();
            let menu = menu.min_w(px(220.)).item(
                estilo::item_de_menu(
                    "chatbot-copiar",
                    match canal {
                        Canal::WhatsApp => "Copiar número",
                        Canal::Instagram => "Copiar IGSID",
                        _ => "Copiar ID da conversa",
                    },
                    None,
                )
                .on_click(move |_, _, cx| {
                    let _ = copiar.update(cx, |tela, cx| tela.copiar_id(cx));
                }),
            );
            if canal != Canal::WhatsApp {
                return menu;
            }
            let excluir = tela.clone();
            menu.item(
                estilo::item_de_menu(
                    "chatbot-excluir-historico",
                    "Excluir histórico",
                    Some(perigo),
                )
                .on_click(move |_, window, cx| {
                    let _ = excluir.update(cx, |tela, cx| tela.pedir_exclusao(window, cx));
                }),
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
        let (apagado, muted, fundo) = (tema.muted_foreground, tema.muted, tema.background);
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
                vazio(Icone::MessageSquare, None, "Nenhuma mensagem para mostrar.")
                    .py(px(24.))
                    .into_any_element(),
            );
        }
        let mut anterior: Option<&Mensagem> = None;
        for mensagem in &mensagens {
            if let Some(quando) = mensagem.quando {
                if modelo::mudou_o_dia(quando, anterior.and_then(|a| a.quando)) {
                    linhas.push(
                        Marker::new()
                            .with_variant(MarkerVariant::Separator)
                            .content(
                                MarkerContent::new().text(modelo::rotulo_do_dia(quando, agora)),
                            )
                            .mt(px(12.))
                            .into_any_element(),
                    );
                }
            }
            let midia = self.midia_do_balao(mensagem, cx);
            linhas.push(balao(mensagem, canal, midia).into_any_element());
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

    /// 📎 A foto, o documento ou o áudio de um balão.
    ///
    /// A foto e a figurinha aparecem dentro do balão; o resto é um cartão.
    /// Nos dois, o clique baixa o arquivo e o entrega ao programa do sistema —
    /// o visualizador de fotos, o leitor de PDF, o tocador de áudio.
    ///
    /// 🚨 O arquivo é sempre pedido à API pelo **id da mensagem**
    /// (`midia::caminho_da_midia`), nunca por endereço que veio no conteúdo.
    fn midia_do_balao(&self, mensagem: &Mensagem, cx: &mut Context<Self>) -> Option<AnyElement> {
        let midia = mensagem.midia.as_ref()?;
        let (apagado, muted, borda) = {
            let tema = cx.theme();
            (tema.muted_foreground, tema.muted, tema.border)
        };
        let id = mensagem.id.clone();

        if midia.tipo.aparece_no_balao() {
            let lado = if midia.tipo == TipoDeMidia::Figurinha {
                112.
            } else {
                240.
            };
            return Some(match self.miniaturas.get(&mensagem.id) {
                Some(Miniatura::Pronta(imagem)) => {
                    let nome = format!("chatbot-foto-{id}");
                    div()
                        .id(SharedString::from(nome.clone()))
                        .debug_selector(move || nome.clone())
                        .cursor_pointer()
                        // A miniatura já vem quadrada (`tela::miniatura_de`),
                        // do tamanho do quadro que estava reservado para ela.
                        .child(
                            gpui_kit::img(imagem.clone())
                                .w(px(lado))
                                .h(px(lado))
                                .rounded(px(8.)),
                        )
                        .on_click(cx.listener(move |tela, _, _, cx| tela.abrir_midia(&id, cx)))
                        .into_any_element()
                }
                // A foto que não veio vira o cartão: o clique tenta de novo e
                // diz o motivo quando não dá.
                Some(Miniatura::Falhou) => self.cartao_do_arquivo(&mensagem.id, midia, cx),
                // Do tamanho da foto que vem: o histórico não pula quando ela
                // chega.
                _ => div()
                    .w(px(lado))
                    .h(px(lado))
                    .rounded(px(8.))
                    .bg(muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(apagado)
                    .child("Carregando a foto…")
                    .into_any_element(),
            });
        }

        let cartao = self.cartao_do_arquivo(&mensagem.id, midia, cx);
        if midia.tipo != TipoDeMidia::Audio {
            return Some(cartao);
        }

        // 🎤 Debaixo do áudio, o que foi dito — ou o botão que pede a
        // transcrição. O áudio que o cliente manda já chega transcrito; o
        // botão cobre o que ficou sem texto.
        let transcricao = match &midia.transcricao {
            Some(texto) => div()
                .max_w(px(280.))
                .pl(px(8.))
                .border_l_2()
                .border_color(borda)
                .italic()
                .text_sm()
                .child(if texto.trim().is_empty() {
                    "(áudio sem fala)".to_string()
                } else {
                    texto.clone()
                })
                .into_any_element(),
            None => {
                let transcrevendo = self.transcrevendo.contains(&mensagem.id);
                let nome = format!("chatbot-transcrever-{id}");
                h_flex()
                    .child(estilo::desligado(
                        marcado(estilo::botao_fantasma_pequeno(nome.clone(), cx), nome)
                            .child(if transcrevendo {
                                "Transcrevendo…"
                            } else {
                                "Transcrever"
                            })
                            .on_click(cx.listener(move |tela, _, _, cx| tela.transcrever(&id, cx))),
                        transcrevendo,
                    ))
                    .into_any_element()
            }
        };
        Some(
            v_flex()
                .gap(px(4.))
                .child(cartao)
                .child(transcricao)
                .into_any_element(),
        )
    }

    /// O cartão de um arquivo: o nome (ou o tipo), o tamanho e "abrir".
    fn cartao_do_arquivo(
        &self,
        mensagem_id: &str,
        midia: &Midia,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (apagado, muted) = {
            let tema = cx.theme();
            (tema.muted_foreground, tema.muted)
        };
        let abrindo = self.abrindo.contains(mensagem_id);
        let id = mensagem_id.to_string();
        let nome = format!("chatbot-midia-{id}");
        h_flex()
            .id(SharedString::from(nome.clone()))
            .debug_selector(move || nome.clone())
            .gap(px(8.))
            .px(px(8.))
            .py(px(6.))
            .min_w(px(180.))
            .max_w(px(280.))
            .rounded(px(8.))
            .bg(muted)
            .cursor_pointer()
            .hover(|estilo| estilo.opacity(0.8))
            .child(Icon::new(Icone::File).size(px(20.)))
            .child(
                v_flex()
                    .min_w(px(0.))
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(midia.titulo()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(apagado)
                            // A mesma linha nos dois estados: o cartão não
                            // muda de tamanho enquanto o arquivo baixa.
                            .child(if abrindo {
                                "abrindo…".to_string()
                            } else {
                                midia.detalhe()
                            }),
                    ),
            )
            .on_click(cx.listener(move |tela, _, _, cx| tela.abrir_midia(&id, cx)))
            .into_any_element()
    }

    fn painel_de_respostas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (fundo, borda, apagado) = (tema.background, tema.border, tema.muted_foreground);
        let tela = cx.entity().downgrade();
        let itens: Vec<AnyElement> = if self.respostas.is_empty() {
            vec![vazio(Icone::Zap, None, "Nenhuma resposta cadastrada.").into_any_element()]
        } else {
            self.respostas
                .iter()
                .enumerate()
                .map(|(i, resposta)| {
                    let r = resposta.clone();
                    let tela = tela.clone();
                    ListItem::new(("chatbot-resposta", i))
                        .debug_selector(move || format!("chatbot-resposta-{i}"))
                        .w_full()
                        .p(px(8.))
                        .rounded(crate::tema::canto(6.))
                        .border_1()
                        .border_color(borda)
                        .text_sm()
                        .child(
                            v_flex()
                                .w_full()
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
                                ),
                        )
                        .on_click(move |_, _, cx| {
                            let _ = tela.update(cx, |tela, cx| tela.enviar_resposta(&r, cx));
                        })
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
        let balao = Bubble::new()
            .with_variant(if falhou {
                BubbleVariant::Destructive
            } else {
                BubbleVariant::Tinted
            })
            .content(
                BubbleContent::new().when(!falhou, |c| c.bg(cor_do_canal(canal).opacity(0.15))),
            )
            .child(pendente.texto.clone())
            .when_some(pendente.erro.clone(), |d, erro| {
                d.child(
                    h_flex()
                        .mt(px(6.))
                        .pt(px(6.))
                        .gap(px(8.))
                        .justify_between()
                        .border_t_1()
                        .border_color(perigo.opacity(0.3))
                        .child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(perigo)
                                .child(crate::erro_da_api::legivel(&erro)),
                        )
                        .child(
                            h_flex()
                                .gap(px(4.))
                                .child(
                                    marcado(
                                        estilo::botao_contorno_pequeno(
                                            format!("chatbot-tentar-{}", id),
                                            cx,
                                        ),
                                        format!("chatbot-tentar-{}", id),
                                    )
                                    .child(Icon::new(Icone::RotateCw).size(px(12.)))
                                    .child("Tentar de novo")
                                    .on_click(
                                        cx.listener(move |tela, _, _, cx| tela.reenviar(id, cx)),
                                    ),
                                )
                                .child(
                                    marcado(
                                        estilo::botao_fantasma_pequeno(
                                            format!("chatbot-descartar-{}", id),
                                            cx,
                                        ),
                                        format!("chatbot-descartar-{}", id),
                                    )
                                    .child("Descartar")
                                    .on_click(
                                        cx.listener(move |tela, _, _, cx| tela.descartar(id, cx)),
                                    ),
                                ),
                        ),
                )
            });
        Message::new()
            .alignment(MessageAlignment::End)
            .content(MessageContent::new().bubble(balao))
            .footer(
                MessageFooter::new()
                    .text_color(if falhou {
                        perigo
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(if falhou { "não enviada" } else { "enviando" }),
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
        // 📎 Só o WhatsApp anexa: é o único canal com rota de mídia na API. O
        // clipe mora no compositor, que some inteiro com a janela de 24 h
        // fechada (o `motivo` acima) — anexo é mensagem de sessão como o texto.
        let pode_anexar = conversa.chave.canal == Canal::WhatsApp;
        let anexo = self.anexo.clone();
        let tem_anexo = anexo.is_some();
        v_flex()
            .flex_none()
            .border_t_1()
            .border_color(borda)
            // O arquivo escolhido fica à vista até sair: nome, tamanho e como
            // tirar. O que se escrever embaixo vira a legenda dele.
            .when_some(anexo, |d, anexo| {
                d.child(
                    h_flex()
                        .debug_selector(|| "chatbot-anexo".into())
                        .px(px(12.))
                        .py(px(4.))
                        .gap(px(8.))
                        .items_center()
                        .text_xs()
                        .bg(muted.opacity(0.4))
                        .child(
                            Icon::new(Icone::Paperclip)
                                .size(px(14.))
                                .text_color(apagado),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .font_weight(FontWeight::MEDIUM)
                                .child(anexo.nome.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_color(apagado)
                                .child(midia::formatar_tamanho(anexo.bytes.len() as u64)),
                        )
                        .child(
                            marcado(
                                estilo::botao_fantasma_pequeno("chatbot-tirar-anexo", cx),
                                "chatbot-tirar-anexo",
                            )
                            .child(Icon::new(Icone::X).size(px(12.)))
                            .tooltip("Tirar o anexo")
                            .on_click(cx.listener(|tela, _, _, cx| tela.tirar_anexo(cx))),
                        ),
                )
            })
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
                            .tooltip("Respostas rápidas")
                            .on_click(cx.listener(|tela, _, _, cx| tela.alternar_respostas(cx)))
                    })
                    .when(pode_anexar, |d| {
                        let botao = if tem_anexo {
                            marcado(estilo::botao_primario("chatbot-clipe", cx), "chatbot-clipe")
                        } else {
                            marcado(estilo::botao_fantasma("chatbot-clipe", cx), "chatbot-clipe")
                        };
                        d.child(
                            botao
                                .child(Icon::new(Icone::Paperclip).size(px(16.)))
                                .tooltip("Anexar foto, áudio ou documento")
                                .on_click(cx.listener(|tela, _, _, cx| tela.anexar(cx))),
                        )
                    })
                    .child(div().flex_1().child(Textarea::new(&self.compositor)))
                    .child(estilo::desligado(
                        marcado(
                            estilo::botao_primario("chatbot-enviar", cx),
                            "chatbot-enviar",
                        )
                        .child(Icon::new(Icone::Send).size(px(16.)))
                        .tooltip("Enviar")
                        .on_click(
                            cx.listener(|tela, _, window, cx| tela.enviar_o_escrito(window, cx)),
                        ),
                        // Com anexo, o texto é a legenda — e pode ir vazio.
                        vazio && !tem_anexo,
                    )),
            )
            .into_any_element()
    }

    // ── Diálogos ───────────────────────────────────────────────────────────

    fn dialogo_aberto(&self, dialogo: Dialogo, cx: &mut Context<Self>) -> AnyElement {
        let caixa = match dialogo {
            Dialogo::Urgencias => self.dialogo_de_urgencias(cx).into_any_element(),
            Dialogo::Resolver(urgencia) => self.dialogo_de_resolucao(&urgencia, cx).into_any_element(),
            Dialogo::Descartar(_) => estilo::conteudo_do_dialogo()
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
                                .on_click(cx.listener(|tela, _, window, cx| tela.fechar_dialogo(window, cx))),
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
            Dialogo::QuemAssume(_) => estilo::conteudo_do_dialogo()
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
                            |tela, _: &gpui_kit::component::input::Enter, window, cx| {
                                tela.confirmar_quem_assume(window, cx)
                            },
                        ))
                        .child(crate::estilo::campo(Input::new(&self.nome_do_atendente))),
                )
                .child(
                    estilo::rodape_do_dialogo()
                        .child(
                            marcado(estilo::botao_fantasma("quem-cancelar", cx), "quem-cancelar")
                                .child("Cancelar")
                                .on_click(cx.listener(|tela, _, window, cx| tela.fechar_dialogo(window, cx))),
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
            Dialogo::ExcluirHistorico(chave) => estilo::conteudo_do_dialogo()
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
                        .child(crate::estilo::campo(Input::new(&self.confirmacao))),
                )
                .child(
                    estilo::rodape_do_dialogo()
                        .child(estilo::desligado(
                            marcado(estilo::botao_contorno("excluir-cancelar", cx), "excluir-cancelar")
                                .child("Cancelar")
                                .on_click(cx.listener(|tela, _, window, cx| tela.fechar_dialogo(window, cx))),
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
        caixa
    }

    fn dialogo_de_urgencias(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let borda = cx.theme().border;
        let linhas: Vec<AnyElement> = self
            .urgencias
            .iter()
            .enumerate()
            .map(|(i, u)| self.linha_de_urgencia(i, u, cx).into_any_element())
            .collect();
        estilo::conteudo_do_dialogo()
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
                    .rounded(crate::tema::canto(8.))
                    .border_1()
                    .border_color(if self.urgencias.is_empty() {
                        borda
                    } else {
                        vermelho().opacity(0.5)
                    })
                    .when(self.urgencias.is_empty(), |d| {
                        d.child(vazio(Icone::Inbox, None, "Nenhuma urgência no momento."))
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
                    .on_click(cx.listener(|tela, _, window, cx| tela.fechar_dialogo(window, cx))),
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
                        .on_click(cx.listener(
                            move |tela, _, window, cx| tela.pedir_descarte(d.clone(), window, cx),
                        )),
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
        estilo::conteudo_do_dialogo()
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
                    .child(Textarea::new(&self.notas)),
            )
            .child(
                estilo::rodape_do_dialogo()
                    .child(
                        marcado(estilo::botao_contorno("resolver-cancelar", cx), "resolver-cancelar")
                            .child("Cancelar")
                            .on_click(cx.listener(|tela, _, window, cx| tela.fechar_dialogo(window, cx))),
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
fn balao(mensagem: &Mensagem, canal: Canal, midia: Option<AnyElement>) -> impl IntoElement {
    let hora = mensagem.quando.map(modelo::hora).unwrap_or_default();
    let automacao = mensagem.rotulo_da_automacao();
    let saida = mensagem.saida;
    // 💬 A `Message` do gpui-kit: o `Bubble` com o texto e, no rodapé da
    // mensagem, a hora (e o selo de automática). O do estúdio vai à direita,
    // tingido com a cor do canal; o do cliente, à esquerda, de contorno.
    Message::new()
        .alignment(if saida {
            MessageAlignment::End
        } else {
            MessageAlignment::Start
        })
        .content(
            MessageContent::new().bubble(
                Bubble::new()
                    .with_variant(if saida {
                        BubbleVariant::Tinted
                    } else {
                        BubbleVariant::Outline
                    })
                    .content(
                        BubbleContent::new()
                            .when(saida, |c| c.bg(cor_do_canal(canal).opacity(0.15))),
                    )
                    // 📎 A mídia vem antes do texto, como no WhatsApp. O texto
                    // só aparece quando é legenda de verdade: o rótulo
                    // ("📷 Foto") debaixo da própria foto é ruído.
                    .when_some(midia, |b, midia| b.child(midia))
                    .when_some(mensagem.texto_do_balao().map(str::to_string), |b, texto| {
                        b.child(texto)
                    }),
            ),
        )
        .footer(
            MessageFooter::new()
                .when_some(automacao, |f, rotulo| f.child(div().italic().child(rotulo)))
                .child(hora),
        )
}
