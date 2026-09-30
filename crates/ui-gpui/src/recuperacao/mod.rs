//! 💾 Recuperar as fotos apagadas de um cartão, ou de um cartão formatado sem
//! querer.
//!
//! Tela própria do menu lateral ("Recuperar cartão"): o operador
//! escolhe o cartão e uma pasta **fora dele**, autoriza com a senha de
//! administrador, acompanha a varredura e, no fim, leva o que voltou à
//! importação de sempre, dentro da sessão do cliente.
//!
//! | módulo | o quê |
//! |---|---|
//! | [`estado`] | a máquina de estados, pura |
//! | [`porta`] | quem lista os cartões e roda o processo elevado |
//! | [`elevar`] | o comando que pede a senha, por sistema |
//! | [`tela`] | o painel |
//!
//! O motor é o `recuperacao-core`; a gravação, o `infrastructure::recuperacao`.

/// 💾 A pasta onde a última recuperação deixou as fotos. O "Do cartão ou
/// pasta…" da sessão (e da nova sessão) a oferece em primeiro lugar, até o app
/// fechar ou outra recuperação terminar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PastaRecuperada {
    pub caminho: String,
    pub fotos: usize,
}

impl gpui_kit::Global for PastaRecuperada {}

pub mod elevar;
pub mod estado;
pub mod porta;
pub mod tela;
