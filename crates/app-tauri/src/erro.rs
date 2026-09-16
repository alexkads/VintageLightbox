//! O que a ponte responde quando não faz o que foi pedido.
//!
//! A página recebe a mensagem pronta para mostrar. Nenhum erro diz se um arquivo
//! fora das raízes existe: a resposta é a mesma nos dois casos.

use serde::{Serialize, Serializer};

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ErroDaPonte {
    #[error("este arquivo não foi escolhido pelo operador")]
    ForaDasRaizes,
    #[error("o arquivo escolhido não existe mais")]
    ArquivoInexistente,
    #[error("não é um arquivo RAW")]
    NaoERaw,
    #[error("não foi possível ler o RAW: {0}")]
    Decodificacao(String),
    #[error("a leitura foi interrompida")]
    Interrompida,
}

impl Serialize for ErroDaPonte {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
