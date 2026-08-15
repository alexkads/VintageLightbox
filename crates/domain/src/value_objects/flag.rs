use serde::{Deserialize, Serialize};
use std::fmt;

/// Representa a flag de uma foto (Pick ou Reject)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Flag {
    Pick,
    Reject,
}

impl Flag {
    /// Retorna o código numérico da flag (persistência)
    /// 1 = Pick, -1 (ou 2) = Reject.
    /// Lightroom usa: 1 = Pick, -1 = Reject, 0 = Unflagged.
    /// Como Option<Flag> trata o unflagged (None), aqui precisamos definir valores para o Some(Flag).
    /// Vamos usar:
    /// 1: Pick
    /// 2: Reject
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
