//! O caixa do balcão numa tela inteira — a rota `/dashboard/caixa` do site
//! (`frontend/src/app/(dashboard)/dashboard/caixa/pdv-do-caixa.tsx`).
//!
//! | Aqui | O que faz | No site |
//! |---|---|---|
//! | [`tela`] | a carga, a lista de sessões, o cupom, o movimento e as teclas F | `carregar.ts`, `pdv-do-caixa.tsx`, `usar-pdv.tsx` |
//! | `dialogos` | abrir, desconto, pessoas, pagamento, sangria, vendas, estorno, fechamento e atalhos | `pdv-dialogos.tsx`, `caixa-actions.ts` |
//! | `flutuante` | o painel da galeria: o total arrastável, o cupom com o ajuste rápido do item e as teclas | `caixa-da-negociacao.tsx`, `edicao-rapida.tsx` |
//! | [`dados`] | o JSON da API, lido e escrito | `lib/schemas/caixa.ts`, `lib/api/caixa.ts` |
//!
//! 🔑 **A conta não mora aqui.** O cupom, o desconto no total, o troco e o que
//! cada diálogo recusa estão em [`biblioteca_core::caixa`], com os testes do
//! `caixa.test.ts` — duas versões da mesma conta divergem, e aqui a divergência
//! é dinheiro.

pub mod dados;
mod dialogos;
mod flutuante;
/// Os gestos do caixa para os cenários de ponta a ponta. Só testes.
#[cfg(test)]
mod para_e2e;
pub mod tela;

pub use dialogos::TipoDeDialogo;
