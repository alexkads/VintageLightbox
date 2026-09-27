//! O que o chatbot e a agenda pedem à raiz: avisar, dar o resultado de um gesto, e
//! trazer a janela quando o operador clica num aviso do sistema.
//!
//! # Toast ou aviso do sistema — quem decide é a janela
//!
//! | A conversa | A janela | O que aparece |
//! |---|---|---|
//! | aberta e na frente | qualquer | nada: o operador está lendo |
//! | outra, ou outra tela | em foco | toast no canto de baixo, à esquerda |
//! | outra, ou outra tela | atrás, minimizada, na bandeja | aviso do sistema (se o sino estiver ligado) |
//!
//! É a regra do site (`tempo-real.ts`): o toast quando a conversa não está
//! na tela, e a `Notification` só com a aba escondida.
//!
//! # 🍞 O toast da mensagem não é o toast da ação
//!
//! 🚨 **Mensagem que chega é o `Toast` do shadcn, no canto de baixo à
//! esquerda** (dono, 27/set/2026). Até ali ela caía na pilha verde do alto,
//! a mesma do "Revelação salva": um recado de cliente com cara de "deu
//! certo", e no lugar em que o operador espera o resultado do próprio gesto.
//!
//! 🔑 **É a `Notification` do gpui-kit, e não uma pilha nossa**: ela já é o
//! `Toast` do shadcn (fundo `popover`, borda, título e descrição, X no
//! hover), empilha por canto (`placement`), segura o autohide enquanto o
//! mouse está em cima e troca o toast da mesma conversa em vez de empilhar
//! (`id1` com o destino) — o que o aviso do sistema já fazia.

use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::{div, prelude::*, Anchor, Context, SharedString, Window};

use super::{Aplicativo, Tela};
use crate::agenda::PedidoDaAgenda;
use crate::chatbot::PedidoDoChatbot;
use crate::tempo_real::Aviso;

impl Aplicativo {
    pub(super) fn atender_o_chatbot(
        &mut self,
        pedido: PedidoDoChatbot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match pedido {
            PedidoDoChatbot::Avisar {
                titulo,
                corpo,
                aviso,
                na_tela,
            } => {
                if na_tela {
                    return;
                }
                let (ligados, cliques) = {
                    let chatbot = self.chatbot.read(cx);
                    (chatbot.avisos_ligados(), chatbot.canal_dos_cliques())
                };
                self.avisar(titulo, corpo, aviso, ligados, cliques, window, cx);
            }
            PedidoDoChatbot::Toast { texto, erro } => self.avisar_em_toast(texto, erro, cx),
            PedidoDoChatbot::TrazerParaAFrente => {
                self.trazer_para_a_frente(Tela::Chatbot, window, cx)
            }
            PedidoDoChatbot::Mudou => cx.notify(),
        }
    }

    /// A agenda avisa de agendamento novo e cancelado — com a mesma regra da
    /// janela, e sem o "na tela": o site avisa a agenda sempre.
    pub(super) fn atender_a_agenda(
        &mut self,
        pedido: PedidoDaAgenda,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match pedido {
            PedidoDaAgenda::Avisar {
                titulo,
                corpo,
                aviso,
            } => {
                let (ligados, cliques) = {
                    let agenda = self.agenda.read(cx);
                    (agenda.avisos_ligados(), agenda.canal_dos_cliques())
                };
                self.avisar(titulo, corpo, aviso, ligados, cliques, window, cx);
            }
            PedidoDaAgenda::Toast { texto, erro } => self.avisar_em_toast(texto, erro, cx),
            PedidoDaAgenda::TrazerParaAFrente => {
                self.trazer_para_a_frente(Tela::Agenda, window, cx)
            }
            PedidoDaAgenda::Mudou => cx.notify(),
        }
    }

    /// Janela na frente: toast. Atrás: aviso do sistema, se o sino deixar.
    #[allow(clippy::too_many_arguments)]
    fn avisar(
        &mut self,
        titulo: String,
        corpo: Option<String>,
        aviso: Aviso,
        ligados: bool,
        cliques: std::sync::mpsc::Sender<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.is_window_active() {
            self.toast_de_mensagem(titulo, corpo, aviso.destino, cliques, window, cx);
        } else if ligados {
            self.avisador.avisar(aviso, cliques);
        }
        cx.notify();
    }

    /// A mensagem no canto de baixo, à esquerda. O clique faz o que o clique no
    /// aviso do sistema faz: manda o destino pelo canal de quem avisou, que
    /// traz a tela e abre a conversa.
    fn toast_de_mensagem(
        &mut self,
        titulo: String,
        corpo: Option<String>,
        destino: String,
        cliques: std::sync::mpsc::Sender<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let corpo = corpo.filter(|c| !c.trim().is_empty());
        #[cfg(test)]
        self.mensagens_dadas
            .push((titulo.clone(), corpo.clone(), destino.clone()));
        let chave = SharedString::from(destino.clone());
        let nota = Notification::new()
            .id1::<ToastDeMensagem>(chave)
            .title(titulo)
            // 🎨 A descrição do `Toast` do shadcn é `text-muted-foreground`; o
            // `message` do kit sai na cor do título, e a prévia competia com
            // o nome de quem escreveu.
            .when_some(corpo, |nota, corpo| {
                let corpo = SharedString::from(corpo);
                nota.content(move |_, _, cx| {
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(corpo.clone())
                        .into_any_element()
                })
            })
            .placement(Anchor::BottomLeft)
            .cursor_pointer()
            .on_click(move |_, _, _| {
                let _ = cliques.send(destino.clone());
            });
        window.push_notification(nota, cx);
    }

    /// O clique num aviso do sistema: a janela vem, na tela de quem avisou.
    fn trazer_para_a_frente(&mut self, tela: Tela, window: &mut Window, cx: &mut Context<Self>) {
        crate::segundo_plano::trazer_para_a_frente(cx);
        window.activate_window();
        if self.tela != tela {
            self.ir_para(tela, window, cx);
        }
    }
}

/// A marca dos toasts de mensagem no `id1` do kit: com o destino, dois avisos
/// da mesma conversa são um toast só.
struct ToastDeMensagem;
