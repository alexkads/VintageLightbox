//! O cliente HTTP do pós-venda — o único lugar deste app que fala com a rede.

pub mod autorizacao;
pub mod cofre;
pub mod espelho;
pub mod http;
pub mod parametros;

pub use cofre::{CofreDoSistema, CofreEmMemoria};
pub use espelho::{
    DadoGuardado, Espelho, EspelhoEmDisco, EspelhoEmMemoria, EstadoDaRede, MotivoDaLembranca,
};
pub use http::{CorpoCru, PosVendaApiHttp, RespostaCrua};
