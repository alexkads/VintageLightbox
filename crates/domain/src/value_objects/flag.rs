use serde::{Deserialize, Serialize};
use std::fmt;

/// Representa a flag de uma foto (Pick ou Reject)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Flag {
    Pick,
    Reject,
}

impl Flag {
    /// Retorna o código numérico da flag (persistência).
    ///
    /// **`1` = Pick, `-1` = Reject** — a convenção do Lightroom, e o que está
    /// gravado na coluna `photos.flag`. "Sem sinalizador" não tem código: é o
    /// `None` do `Option<Flag>`, e no banco é `NULL`.
    ///
    /// ⚠️ O comentário aqui dizia *"Vamos usar: 1: Pick, 2: Reject"*, e o
    /// código nunca usou `2`. Quem escrevesse um filtro a partir da
    /// documentação procuraria por `2` e não acharia foto rejeitada nenhuma —
    /// sem erro, sem aviso, só uma lista vazia que parece "não há nenhuma".
    /// O código estava certo; o texto é que envelheceu.
    pub fn as_code(&self) -> i32 {
        match self {
            Flag::Pick => 1,
            Flag::Reject => -1,
        }
    }

    /// Cria uma Flag a partir do código numérico
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            1 => Some(Flag::Pick),
            -1 => Some(Flag::Reject),
            _ => None,
        }
    }
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Flag::Pick => write!(f, "Picked"),
            Flag::Reject => write!(f, "Rejected"),
        }
    }
}
