//! A atualização automática: descobrir versão nova, avisar e instalar.
//!
//! O app não passa por loja nenhuma — ele se atualiza sozinho: pelo pacote
//! assinado ([`porta`]) quando há um para esta máquina, ou rodando o
//! instalador que compila o `main` ([`compilar`]) quando não há. O que mudou e
//! por que atualizar vem de [`novidades`]; o que a tela mostra, de [`faixa`].
pub mod compilar;
pub mod faixa;
pub mod novidades;
pub mod porta;
