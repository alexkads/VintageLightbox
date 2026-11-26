//! ⏰ **O portão do caixa do dia, do lado da raiz** (#52).
//!
//! A tela do caixa decide o portão a cada carga e conta à raiz
//! ([`PedidoDoCaixa::Portao`](crate::caixa::tela::PedidoDoCaixa)); aqui a raiz
//! obedece:
//!
//! - **Segurando** (o caixa vencido aberto, ou nenhum aberto): a rota do caixa
//!   fica na frente, o menu lateral só abre o Caixa e o atalho de troca de
//!   tela não leva a outra. A conta, a troca de estúdio e a faixa de
//!   atualização continuam à mão (menu da conta e rodapé não passam por aqui).
//! - **Solto** depois de ter segurado: volta à lista de sessões, que é por onde
//!   o dia começa, e a retomada que esperava o portão segue.
//! - **Com uma venda gravando ou um diálogo aberto no painel da galeria**, o
//!   portão espera ([`Aplicativo::portao_pendente`]): a virada nunca corta uma
//!   cobrança no meio.

use biblioteca_core::caixa::Portao;
use gpui_kit::{Context, Window};

use super::{Aplicativo, Tela};

// 🧪 Os cenários do ciclo de vida que não são do portão desligam ele: cada um
// cria o próprio estúdio, e o portão abriria o caixa de outro estúdio qualquer
// no meio dos atos deles. O cenário do portão o liga (`e2e::ciclo_de_vida::portao`).
#[cfg(test)]
thread_local! {
    static DESLIGADO_NO_TESTE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn desligar_no_teste(desligado: bool) {
    DESLIGADO_NO_TESTE.with(|d| d.set(desligado));
}

/// Fora dos testes, nunca.
pub(crate) fn desligado_no_teste() -> bool {
    #[cfg(test)]
    {
        DESLIGADO_NO_TESTE.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// A frase de quem tenta ir a outra tela com o portão de pé.
const ESPERA_O_CAIXA: &str = "Feche o caixa que ficou aberto e abra o de hoje antes de começar.";

impl Aplicativo {
    /// O portão está segurando as outras telas?
    pub(crate) fn portao_segura(&self) -> bool {
        self.portao_do_caixa != Portao::Livre
    }

    /// A tela do caixa contou um portão.
    pub(super) fn receber_o_portao(
        &mut self,
        portao: Portao,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if portao != Portao::Livre && self.caixa_flutuante.read(cx).ocupado() {
            // 🧾 Uma cobrança no meio, no painel da galeria: espera ela terminar
            // (a vigia de cada segundo aplica depois).
            self.portao_pendente = Some(portao);
            return;
        }
        // O foco só muda quando a tela muda: o "livre" de quem não passa pelo
        // portão chega no meio do trabalho e não pode tirar o cursor do campo.
        let muda_a_tela = portao != Portao::Livre || self.portao_segura();
        self.aplicar_o_portao(portao, cx);
        if muda_a_tela {
            window.focus(&self.foco, cx);
        }
    }

    /// ⏰ Uma vez por segundo: a virada com o app aberto e o portão que
    /// esperava a cobrança terminar.
    pub(crate) fn vigiar_o_portao(&mut self, cx: &mut Context<Self>) {
        self.caixa.update(cx, |caixa, cx| caixa.vigiar_a_virada(cx));
        if let Some(portao) = self.portao_pendente {
            if !self.caixa_flutuante.read(cx).ocupado() {
                self.aplicar_o_portao(portao, cx);
            }
        }
    }

    fn aplicar_o_portao(&mut self, portao: Portao, cx: &mut Context<Self>) {
        self.portao_pendente = None;
        let segurava = self.portao_segura();
        self.portao_do_caixa = portao;
        if portao != Portao::Livre {
            self.segurar_no_caixa(cx);
        } else if segurava {
            // Solto: o dia começa pela lista de sessões, como depois da entrada.
            self.tela = Tela::Sessoes;
            self.sessoes.update(cx, |t, cx| t.recarregar(cx));
        }
        if portao == Portao::Livre && std::mem::take(&mut self.retomada_espera_o_portao) {
            self.decidir_a_retomada(cx);
        }
        cx.notify();
    }

    /// A rota do caixa na frente, saindo da sessão e da revelação como o
    /// [`Aplicativo::ir_para`] faria (aqui pode não haver janela: a virada vem
    /// da vigia).
    fn segurar_no_caixa(&mut self, cx: &mut Context<Self>) {
        self.menu_da_conta = None;
        if self.tela == Tela::Chatbot {
            self.chatbot.update(cx, |t, cx| t.mostrar(false, cx));
        }
        if self.tela == Tela::Agenda {
            self.agenda.update(cx, |t, cx| t.mostrar(false, cx));
        }
        if self.tela != Tela::Caixa {
            self.estacionar_a_revelacao(cx);
            if self.sessao_aberta.is_some() {
                self.sair_da_sessao(cx);
            }
            self.caixa.update(cx, |t, cx| t.abrir(cx));
            self.tela = Tela::Caixa;
        }
    }

    /// 🔐 A troca de tela que o portão recusa: tudo menos o caixa. Diz por quê.
    pub(super) fn portao_recusa(&mut self, tela: Tela, cx: &mut Context<Self>) -> bool {
        if !self.portao_segura() || tela == Tela::Caixa {
            return false;
        }
        self.avisar_em_toast(ESPERA_O_CAIXA.into(), false, cx);
        true
    }
}
