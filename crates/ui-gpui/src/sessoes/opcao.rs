//! Uma opção de lista (`Select` do gpui-kit): o valor que se lê e o rótulo que
//! se vê. Nasceu nos filtros por coluna da lista de sessões, que saíram com o
//! `DataTable` (0.1.169); caixa, editor, exportação e sons seguem com ela.

use gpui_kit::component::select::SelectItem;
use gpui_kit::SharedString;

/// Uma opção de lista: o valor que se lê e o rótulo que se vê.
#[derive(Debug, Clone)]
pub struct Opcao {
    valor: String,
    titulo: SharedString,
}

impl Opcao {
    pub fn nova(valor: impl Into<String>, titulo: impl Into<SharedString>) -> Self {
        Self {
            valor: valor.into(),
            titulo: titulo.into(),
        }
    }
}

impl SelectItem for Opcao {
    type Value = String;

    fn title(&self) -> SharedString {
        self.titulo.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.valor
    }

    fn display_title(&self) -> Option<gpui_kit::AnyElement> {
        None
    }
}
