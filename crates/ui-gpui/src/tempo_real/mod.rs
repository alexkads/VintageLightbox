//! 📡 O tempo real das telas de atendimento — o chatbot e a agenda.
//!
//! Três peças, as mesmas para as duas telas:
//!
//! - [`sse`]: bytes do fluxo viram eventos inteiros;
//! - [`porta`]: os fluxos da API abertos enquanto a conta está dentro, com a
//!   reconexão do site (`components/tempo-real/politica.ts`);
//! - [`aviso`]: o aviso do sistema operacional, para o operador que está em
//!   outra tela ou com o app escondido.

pub mod aviso;
pub mod porta;
pub mod preferencias;
pub mod sse;

pub use aviso::{Avisador, Aviso, AvisoDoSistema};
pub use porta::{Escuta, EscutaHttp, EstadoDaConexao, Guarda, Sinal};
