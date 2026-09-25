//! 💬 O painel do chatbot — a rota `/dashboard/chatbot` do site no desktop.
//!
//! O operador do balcão fica sabendo de quem escreveu (WhatsApp, Instagram,
//! Messenger, Telegram e o chat do site) sem sair da triagem, e responde daqui
//! como responderia no site (dono, 2026-09-25).
//!
//! | Peça | Papel |
//! |---|---|
//! | [`modelo`] | as regras do site em funções puras (lista, janela de 24 h, textos) |
//! | [`pedidos`] | cada gesto como pedido à API, com o caminho e o corpo do site |
//! | [`tela`] | o estado, o tempo real e os gestos |
//! | [`desenho`] | a tela |

pub mod desenho;
pub mod modelo;
pub mod pedidos;
pub mod tela;

pub use tela::{Chatbot, PedidoDoChatbot};

gpui::actions!(chatbot, [EnviarMensagem]);

/// As teclas do compositor: **Enter envia, Shift+Enter quebra linha**, como no
/// site.
///
/// 🔑 O campo de várias linhas do `gpui-component` põe a quebra de linha no
/// Enter **antes** de avisar a tela — interceptar o aviso chegaria tarde. Por
/// isso o Enter é religado aqui, no contexto do compositor acima do campo, e o
/// Shift+Enter passa a ser o Enter do próprio campo (o que quebra a linha).
/// Chamado depois do `gpui_component::init`: entre ligações do mesmo nível, a
/// mais nova vale.
pub fn ligar_teclas(cx: &mut gpui::App) {
    let contexto = format!("{} > Input", desenho::COMPOSITOR);
    cx.bind_keys([
        gpui::KeyBinding::new("enter", EnviarMensagem, Some(&contexto)),
        gpui::KeyBinding::new(
            "shift-enter",
            gpui_component::input::Enter { secondary: false },
            Some(&contexto),
        ),
    ]);
}
