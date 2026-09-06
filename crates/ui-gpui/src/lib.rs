//! A interface do VintageLightbox em GPUI.
//!
//! Foi construída ao lado do `crates/ui` (egui), que saiu do workspace em
//! 17/ago/2026 com a migração concluída — a história está em
//! `docs/10-MIGRACAO-GPUI.md`. Hoje é a única interface, e o alvo deixou de ser
//! o app antigo: é o Lightroom (`docs/00-OBJETIVO.md`).

pub mod app;
pub mod balcao;
pub mod biblioteca;
pub mod cliente;
pub mod configuracoes;
pub mod entrada;
pub mod exportacao;
/// O fluxo dos onze passos, de ponta a ponta. Só testes.
#[cfg(test)]
mod fluxo;
pub mod imagem;
pub mod importacao;
pub mod impressao;
pub mod pos_venda;
pub mod revelacao;
pub mod sessoes;
pub mod tema;
