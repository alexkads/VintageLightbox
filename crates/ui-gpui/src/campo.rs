//! Trocar o texto de um campo **avisando quem o escuta**.
//!
//! 🚨 **No gpui-kit 0.6 o `set_value` ficou mudo.** Até o `gpui-component` 0.5.1
//! ele terminava em `cx.emit(InputEvent::Change)`, e o app inteiro foi escrito
//! contando com isso: o "Limpar" da busca da Biblioteca zera o campo e é o
//! `Change` que refiltra a grade; restaurar um formulário é o `Change` que
//! atualiza o modelo. O 0.6 desliga os eventos durante o `set_value` (para que
//! trocar o valor por código não volte como se fosse o usuário digitando), e
//! os dois testes da busca da Biblioteca foram os primeiros a acusar.
//!
//! [`TrocarValor::trocar_valor`] é o `set_value` de antes: troca e avisa. Use o
//! `set_value` puro só quando o aviso é o que **não** se quer.

use gpui_kit::component::input::{InputEvent, InputModeKind};
use gpui_kit::{Context, SharedString, Window};

/// O estado de um campo de texto de qualquer modo (uma linha, várias linhas).
type Estado<M> = gpui_kit::base::input::InputBaseState<M>;

pub trait TrocarValor {
    /// Troca o texto e emite `InputEvent::Change`, como o `set_value` do 0.5.
    fn trocar_valor(
        &mut self,
        valor: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) where
        Self: Sized;
}

impl<M: InputModeKind> TrocarValor for Estado<M> {
    fn trocar_valor(
        &mut self,
        valor: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_value(valor, window, cx);
        cx.emit(InputEvent::Change);
    }
}
