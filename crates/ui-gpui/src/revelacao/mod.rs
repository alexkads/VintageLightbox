//! A Revelação — a fase 2 da migração (docs/10-MIGRACAO-GPUI.md §5).
//!
//! Começa pelo que tem de existir antes dos ~50 ajustes: a foto grande na tela.
//! Slider sem imagem embaixo não tem como ser conferido — e o critério de saída
//! da fase é **igualdade de pixel** com o app de egui.

pub mod controles;
pub mod persistencia;
pub mod processador;
pub mod tela;
