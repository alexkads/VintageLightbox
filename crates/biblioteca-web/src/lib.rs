//! A tela da galeria do pós-venda, inteira, no navegador — ver [`web`].
//!
//! 🔑 **Só existe no alvo `wasm32`**, como o `revelacao-web`: a superfície do
//! wgpu nasce de um `HtmlCanvasElement`, e o `fetch` é o do navegador. Em
//! nativo este crate é vazio de propósito — o que se prova dele fora do
//! navegador é o `biblioteca-core`, que é onde as decisões moram.

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod entrada;
#[cfg(target_arch = "wasm32")]
mod local;
#[cfg(target_arch = "wasm32")]
mod modelo;
#[cfg(target_arch = "wasm32")]
mod rede;
#[cfg(target_arch = "wasm32")]
mod render;
#[cfg(target_arch = "wasm32")]
mod telas;
#[cfg(target_arch = "wasm32")]
mod tema;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::*;
