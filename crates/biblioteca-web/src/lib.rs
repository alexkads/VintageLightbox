//! A grade de miniaturas do pós-venda, no navegador — **só o motor**. Ver [`web`].
//!
//! É o mesmo desenho do `revelacao-web`: o wasm faz o que é pesado ou o que
//! tem de ser uma conta só (geometria, seleção, miniaturas, o desenho dos
//! tiles na GPU), e **o site desenha todo texto e todo controle** — cabeçalho,
//! barra, painel, diálogos, o nome sob cada foto — em React, com os componentes
//! e a fonte dele.
//!
//! ⚠️ Já foi diferente: em 2026-09-05 esta crate desenhou a tela inteira em
//! egui, a pedido do dono ("tudo dentro do wasm, não híbrido"). No mesmo dia
//! ele viu o resultado ao lado do editor de revelação e reverteu — os widgets
//! do egui nunca ficam iguais aos do site. A decisão e o que não refazer estão
//! em `docs/BIBLIOTECA_NO_NAVEGADOR.md` do `recordarfotos-e-commerce`.
//!
//! 🔑 **Só existe no alvo `wasm32`**, como o `revelacao-web`: a superfície do
//! wgpu nasce de um `HtmlCanvasElement`, e o `fetch` é o do navegador. Em
//! nativo este crate é vazio de propósito — o que se prova dele fora do
//! navegador é o `biblioteca-core`, que é onde as decisões moram.

#[cfg(target_arch = "wasm32")]
mod grade;
#[cfg(target_arch = "wasm32")]
mod local;
#[cfg(target_arch = "wasm32")]
mod modelo;
#[cfg(target_arch = "wasm32")]
mod pintor;
#[cfg(target_arch = "wasm32")]
mod rede;
#[cfg(target_arch = "wasm32")]
mod render;
#[cfg(target_arch = "wasm32")]
mod tema;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::*;
