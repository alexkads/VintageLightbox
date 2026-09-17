//! A interface do VintageLightbox em GPUI.
//!
//! Foi construída ao lado do `crates/ui` (egui), que saiu do workspace em
//! 17/ago/2026 com a migração concluída — a história está em
//! `docs/10-MIGRACAO-GPUI.md`. Hoje é a única interface, e o alvo deixou de ser
//! o app antigo: é o Lightroom (`docs/00-OBJETIVO.md`).

pub mod app;
/// A atualização automática — o app não passa por loja e se atualiza sozinho.
pub mod atualizacao;
pub mod balcao;
/// O ícone na bandeja do sistema, com os envios à vista.
pub mod bandeja;
pub mod biblioteca;
/// O caixa do balcão numa tela inteira, como a rota `/dashboard/caixa`.
pub mod caixa;
pub mod cliente;
pub mod configuracoes;
/// Fotografar a janela e seguir um roteiro, só em build de depuração.
pub mod depuracao;
/// Quando o app acaba — fechar a janela principal encerra o processo.
pub mod encerramento;
pub mod entrada;
/// As peças do shadcn do site (botões, selos, cabeçalho de página).
pub mod estilo;
pub mod exportacao;
/// O fluxo dos onze passos, de ponta a ponta. Só testes.
#[cfg(test)]
mod fluxo;
pub mod imagem;
pub mod importacao;
pub mod impressao;
/// O menu do app no macOS (Sobre, Ocultar, Sair).
pub mod menu;
pub mod pos_venda;
/// Os ícones do site e as imagens da capa, embutidos.
pub mod recursos;
pub mod revelacao;
/// As marcas da triagem desenhadas: nota, etiqueta, sinalizador e balcão.
/// Vivem na raiz porque duas telas as usam — a grade da Biblioteca e a do ensaio.
pub mod selos;
/// O trabalho que continua com a janela minimizada ou fechada (a bandeja).
pub mod segundo_plano;
pub mod sessoes;
pub mod tema;
