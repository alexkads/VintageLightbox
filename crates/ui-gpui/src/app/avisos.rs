//! O que o chatbot e a agenda pedem à raiz: avisar, dar o resultado de um gesto, e
//! trazer a janela quando o operador clica num aviso do sistema.
//!
//! # Toast ou aviso do sistema — quem decide é a janela
//!
//! | A conversa | A janela | O que aparece |
//! |---|---|---|
//! | aberta e na frente | qualquer | nada: o operador está lendo |
//! | outra, ou outra tela | em foco | toast dentro do app |
//! | outra, ou outra tela | atrás, minimizada, na bandeja | aviso do sistema (se o sino estiver ligado) |
//!
//! É a regra do site (`tempo-real.ts`): o toast quando a conversa não está
//! na tela, e a `Notification` só com a aba escondida.

use gpui_kit::{Context, Window};

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
            let texto = match corpo.filter(|c| !c.trim().is_empty()) {
                Some(corpo) => format!("{titulo} — {corpo}"),
                None => titulo,
            };
            self.avisar_em_toast(texto, false, cx);
        } else if ligados {
            self.avisador.avisar(aviso, cliques);
        }
        cx.notify();
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
