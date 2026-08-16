//! A Importação — a fase 3 da migração (docs/10-MIGRACAO-GPUI.md §5).
//!
//! O modal de quatro etapas assíncronas do legado (`import_view.rs`, 2.101 LOC),
//! reescrito em 15/ago e ainda fresco. A ordem das leituras — escanear,
//! descrever, miniaturar o visível, conferir duplicatas — é regra conquistada, e
//! é o que faz um cartão de 2.000 RAWs abrir a tela em vez de travá-la.

pub mod estado;
pub mod explorador;
pub mod tela;
