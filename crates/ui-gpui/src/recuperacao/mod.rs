//! 💾 Recuperar as fotos de um cartão formatado sem querer.
//!
//! Aberta de dentro da Importação ("Recuperar cartão formatado…"): o operador
//! escolhe o cartão e uma pasta **fora dele**, autoriza com a senha de
//! administrador, acompanha a varredura e, no fim, importa o que voltou pela
//! grade de sempre.
//!
//! | módulo | o quê |
//! |---|---|
//! | [`estado`] | a máquina de estados, pura |
//! | [`porta`] | quem lista os cartões e roda o processo elevado |
//! | [`elevar`] | o comando que pede a senha, por sistema |
//! | [`tela`] | o painel |
//!
//! O motor é o `recuperacao-core`; a gravação, o `infrastructure::recuperacao`.

pub mod elevar;
pub mod estado;
pub mod porta;
pub mod tela;
