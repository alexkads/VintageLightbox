//! 📖 O visualizador de PDF: o fotolivro que o balcão acabou de gerar, visto
//! sem sair do app.
//!
//! Dono, 10/out/2026, olhando o diálogo de exportação: *"Acho que cabe uma
//! visualização de PDF, crie um módulo de visualização de PDF com base nesse
//! projeto https://github.com/storytold/pdfcraft"*.
//!
//! São duas metades, como na Revelação:
//!
//! - o crate **`visualizador-pdf`** abre o arquivo, mede as folhas e desenha
//!   cada página em pixels, numa thread própria (é lá que está a atribuição ao
//!   PdfCraft e ao `hayro`);
//! - este módulo é a tela: [`folhas`] faz a conta da coluna de páginas (sem
//!   janela, com teste) e [`tela`] a desenha com as peças do gpui-kit.
//!
//! Quem abre é a raiz (`app.rs`), no `Dialog` de sempre: o botão "Ver o
//! fotolivro" do fim da exportação.

pub mod folhas;
pub mod tela;

pub use tela::VisualizadorDePdf;
