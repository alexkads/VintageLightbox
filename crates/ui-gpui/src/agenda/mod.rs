//! 📅 A agenda — a rota `/dashboard/agendamentos` do site no desktop.
//!
//! O operador fica sabendo de agendamento novo (do site, do WhatsApp, do
//! telefone) e de cancelamento sem sair da triagem, e mexe na agenda como no
//! site: reagendar, registrar o atendimento, excluir, descancelar (dono,
//! 2026-09-25).
//!
//! | Peça | Papel |
//! |---|---|
//! | [`modelo`] | período visível, estações, status, origem, relevância do evento |
//! | [`pedidos`] | cada gesto como pedido à API, com o caminho e o corpo do site |
//! | [`tela`] | o estado, o tempo real e os gestos |
//! | [`desenho`] | a tela |

pub mod desenho;
pub mod modelo;
pub mod pedidos;
pub mod tela;

pub use tela::{Agenda, PedidoDaAgenda};

gpui::actions!(agenda, [VoltarNaAgenda]);

/// O Esc do diálogo: formulário → detalhes → fechar, como no site.
pub fn ligar_teclas(cx: &mut gpui::App) {
    cx.bind_keys([gpui::KeyBinding::new(
        "escape",
        VoltarNaAgenda,
        Some(desenho::DIALOGO),
    )]);
}
