//! A interface do VintageLightbox em GPUI.
//!
//! Construída **ao lado** de `crates/ui`, que continua compilando e rodando até
//! a fase 5 da migração (docs/10-MIGRACAO-GPUI.md). Os dois abrem o mesmo
//! catálogo e falam com as mesmas quatro camadas internas.

pub mod biblioteca;
pub mod imagem;
pub mod tema;
