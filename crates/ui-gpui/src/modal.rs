//! 🪟 **O contrato de toda sobreposição**: diálogo, gaveta, pergunta, modal.
//!
//! Uma sobreposição que some da tela leva junto o que estava focado dentro
//! dela — o campo que o diálogo focou ao abrir, ou o que o operador clicou. O
//! foco fica apontando para um elemento que ninguém desenha, e **nenhum atalho
//! chega a ninguém** até o próximo clique: a grade, a tira e a Revelação
//! morrem em silêncio. Já aconteceu com o balcão, com o caixa e com o `Esc` da
//! Revelação (dono, 2026-09-26: *"isso não pode ser assim"*).
//!
//! [`Modal`] guarda o estado **e** quem tinha o foco quando abriu; fechar é
//! devolver. Não há como fechar sem passar por ele, porque o estado mora
//! dentro. Cada jeito de fechar diz o que faz com o foco: `fechar` (na hora),
//! `fechar_depois` (sem janela à mão) e `largar` (o foco já saiu de propósito).
//!
//! 🛟 Por baixo há a rede da raiz (`Aplicativo::foco_perdido`): se o foco cair
//! num elemento que sumiu, ele volta para a tela — e nos testes isso conta
//! como defeito (`e2e::Estudio` recusa seguir), porque a rede salva o operador
//! mas devolve o foco ao lugar genérico, e não para onde ele estava.

use gpui_kit::{AnyWindowHandle, App, FocusHandle, Window};

/// Quem tinha o foco antes de uma sobreposição tomá-lo — a peça do [`Modal`]
/// que devolve. Existe solta para quem já guarda o próprio estado aberto de
/// outro jeito (o `Option<Dialogo>` do caixa, com dezenas de leitores).
#[derive(Default)]
pub struct DevolverFoco(Option<(FocusHandle, AnyWindowHandle)>);

impl DevolverFoco {
    /// Guarda quem tem o foco agora, **se ainda não guardou**: o segundo passo
    /// do mesmo diálogo não pode guardar o campo do primeiro.
    pub fn lembrar(&mut self, window: &Window, cx: &App) {
        if self.0.is_none() {
            self.0 = window
                .focused(cx)
                .map(|foco| (foco, window.window_handle()));
        }
    }

    /// Guarda um destino escolhido por quem abre.
    pub fn lembrar_este(&mut self, foco: FocusHandle, window: &Window) {
        self.0 = Some((foco, window.window_handle()));
    }

    /// Devolve o foco na hora.
    pub fn devolver(&mut self, window: &mut Window, cx: &mut App) {
        if let Some((foco, _)) = self.0.take() {
            window.focus(&foco, cx);
        }
    }

    /// Devolve assim que a janela estiver livre (`defer`) — de onde não há
    /// `Window` à mão: pedir a janela agora, de dentro da atualização dela,
    /// falharia.
    pub fn devolver_depois(&mut self, cx: &mut App) {
        if let Some((foco, janela)) = self.0.take() {
            cx.defer(move |cx| {
                let _ = janela.update(cx, |_, window, cx| window.focus(&foco, cx));
            });
        }
    }

    /// Há alguém guardado para receber o foco de volta.
    pub fn guardado(&self) -> bool {
        self.0.is_some()
    }

    /// Esquece sem devolver.
    pub fn esquecer(&mut self) {
        self.0 = None;
    }

    /// Toma o foco para `campo`, guardando antes quem o tinha.
    pub fn focar(&mut self, campo: &FocusHandle, window: &mut Window, cx: &mut App) {
        self.lembrar(window, cx);
        window.focus(campo, cx);
    }
}

/// Uma sobreposição aberta ou fechada, com o foco para devolver.
///
/// `T` é o que ela mostra (o formulário, a pergunta); `()` para a que é só
/// aberta ou fechada.
pub struct Modal<T> {
    estado: Option<T>,
    foco: DevolverFoco,
}

impl<T> Default for Modal<T> {
    fn default() -> Self {
        Self {
            estado: None,
            foco: DevolverFoco::default(),
        }
    }
}

impl<T> Modal<T> {
    /// Abre com `estado` e guarda quem tem o foco. Com ele já aberto só troca
    /// o conteúdo (da confirmação para o formulário, de um passo para o
    /// outro), e o foco guardado continua o da primeira abertura.
    pub fn abrir(&mut self, estado: T, window: &Window, cx: &App) {
        if self.estado.is_none() {
            self.foco.esquecer();
            self.foco.lembrar(window, cx);
        }
        self.estado = Some(estado);
    }

    /// Abre de onde não há `Window` — a recusa do site que chega por recado.
    /// O foco é guardado quando o diálogo o tomar ([`Modal::focar`]).
    pub fn abrir_sem_janela(&mut self, estado: T) {
        if self.estado.is_none() {
            self.foco.esquecer();
        }
        self.estado = Some(estado);
    }

    /// Abre dizendo para onde o foco volta — quando "quem tinha o foco" não
    /// serve. O caso é o menu de contexto: quem tem o foco no clique é o
    /// próprio menu, que some antes do diálogo.
    pub fn abrir_devolvendo_a(&mut self, estado: T, foco: FocusHandle, window: &Window) {
        self.foco.lembrar_este(foco, window);
        self.estado = Some(estado);
    }

    /// O diálogo aberto toma o foco para um campo dele — guardando antes quem
    /// o tinha, se a abertura não pôde.
    pub fn focar(&mut self, campo: &FocusHandle, window: &mut Window, cx: &mut App) {
        self.foco.focar(campo, window, cx);
    }

    /// Fecha e devolve o foco a quem o tinha, na hora.
    pub fn fechar(&mut self, window: &mut Window, cx: &mut App) -> Option<T> {
        self.foco.devolver(window, cx);
        self.estado.take()
    }

    /// Fecha de onde não há `Window` à mão — a resposta do site que chega por
    /// um recado, o Enter de um campo assinado sem janela. O foco volta assim
    /// que a janela estiver livre.
    pub fn fechar_depois(&mut self, cx: &mut App) -> Option<T> {
        self.foco.devolver_depois(cx);
        self.estado.take()
    }

    /// Fecha **sem** devolver o foco: ele já saiu de propósito. É o `Blur` de
    /// um campo (o operador clicou em outro lugar focável, e devolver roubaria
    /// o foco de onde ele o pôs), ou a troca de tela, que foca a tela nova.
    pub fn largar(&mut self) -> Option<T> {
        self.foco.esquecer();
        self.estado.take()
    }

    pub fn aberto(&self) -> Option<&T> {
        self.estado.as_ref()
    }

    pub fn aberto_mut(&mut self) -> Option<&mut T> {
        self.estado.as_mut()
    }

    pub fn esta_aberto(&self) -> bool {
        self.estado.is_some()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use gpui_kit::{div, Context, Focusable, IntoElement, ParentElement, Render, TestAppContext};
    use gpui_kit::{InteractiveElement, Styled};

    /// Uma tela com um foco próprio e um diálogo com campo focável.
    struct Tela {
        foco: FocusHandle,
        campo: FocusHandle,
        dialogo: Modal<()>,
    }

    impl Focusable for Tela {
        fn focus_handle(&self, _: &App) -> FocusHandle {
            self.foco.clone()
        }
    }

    impl Render for Tela {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().track_focus(&self.foco).size_full().children(
                self.dialogo
                    .esta_aberto()
                    .then(|| div().track_focus(&self.campo).size_full()),
            )
        }
    }

    fn tela(cx: &mut TestAppContext) -> (gpui_kit::Entity<Tela>, &mut gpui_kit::VisualTestContext) {
        cx.add_window_view(|window, cx| {
            let foco = cx.focus_handle();
            window.focus(&foco, cx);
            Tela {
                foco,
                campo: cx.focus_handle(),
                dialogo: Modal::default(),
            }
        })
    }

    #[gpui_kit::test]
    fn fechar_devolve_o_foco_a_quem_o_tinha(cx: &mut TestAppContext) {
        let (tela, cx) = tela(cx);
        cx.update(|window, cx| {
            tela.update(cx, |t, cx| {
                t.dialogo.abrir((), window, cx);
                window.focus(&t.campo, cx);
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            tela.update(cx, |t, cx| {
                assert!(t.campo.is_focused(window));
                t.dialogo.fechar(window, cx);
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            let t = tela.read(cx);
            assert!(t.foco.is_focused(window), "o foco voltou para a tela");
            assert!(t.foco.contains_focused(window, cx), "e ela está desenhada");
        });
    }

    #[gpui_kit::test]
    fn fechar_depois_devolve_quando_a_janela_fica_livre(cx: &mut TestAppContext) {
        let (tela, cx) = tela(cx);
        cx.update(|window, cx| {
            tela.update(cx, |t, cx| {
                t.dialogo.abrir((), window, cx);
                window.focus(&t.campo, cx);
                cx.notify();
            })
        });
        cx.run_until_parked();
        // Sem janela à mão, como a resposta do site que fecha o diálogo.
        cx.update(|_, cx| {
            tela.update(cx, |t, cx| {
                t.dialogo.fechar_depois(cx);
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert!(tela.read(cx).foco.is_focused(window));
        });
    }

    #[gpui_kit::test]
    fn reabrir_por_cima_nao_guarda_o_campo_do_dialogo(cx: &mut TestAppContext) {
        let (tela, cx) = tela(cx);
        cx.update(|window, cx| {
            tela.update(cx, |t, cx| {
                t.dialogo.abrir((), window, cx);
                window.focus(&t.campo, cx);
                // O segundo passo do mesmo diálogo, com o campo focado.
                t.dialogo.abrir((), window, cx);
                t.dialogo.fechar(window, cx);
                assert!(t.foco.is_focused(window));
            })
        });
    }
}
