//! O cliente HTTP do pós-venda — o único lugar deste app que fala com a rede.

pub mod autorizacao;
pub mod cofre;
pub mod http;

pub use cofre::{CofreDoSistema, CofreEmMemoria};
pub use http::PosVendaApiHttp;
