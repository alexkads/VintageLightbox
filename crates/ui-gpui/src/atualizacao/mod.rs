//! A atualização automática: descobrir versão nova, avisar e instalar.
//!
//! O app não passa por loja nenhuma — ele se atualiza sozinho, com pacote
//! assinado por chave própria. A regra está em [`porta`]; o que a tela mostra,
//! em [`faixa`].
pub mod faixa;
pub mod porta;
