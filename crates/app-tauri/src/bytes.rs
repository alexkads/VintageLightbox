//! Como os bytes atravessam a ponte.
//!
//! 🚨 **No macOS (e no Linux) o canal rápido do Tauri não existe para o site.**
//! O Tauri manda cada chamada por `fetch("ipc://localhost/…")`, e o WebKit recusa
//! qualquer esquema próprio a partir de uma página `https`. Medido em
//! 2026-09-16 com o site de produção: `ipc://`, `asset://` e `tauri://` falham
//! todos com "Load failed". O Tauri cai então para `postMessage`, que serializa
//! um `Uint8Array` como **array JSON de números**. Resultado: 30 MB levavam 5 s
//! para ir, chegavam ao Rust como JSON, e não como binário, e os comandos
//! respondiam "pedido incompleto".
//!
//! Por isso a página manda os bytes como **base64 numa string** (`{ "base64":
//! "…" }`), que o WebKit e o `serde` tratam depressa, e as respostas com bytes
//! voltam do mesmo jeito. O corpo cru continua aceito: é o que chega no
//! Windows, onde o canal rápido funciona.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use tauri::ipc::{InvokeBody, Request};

use crate::erro::ErroDaPonte;

/// Os bytes do pedido, venham crus ou em base64.
pub fn do_pedido(request: &Request<'_>) -> Result<Vec<u8>, ErroDaPonte> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(valor) => valor
            .get("base64")
            .and_then(|v| v.as_str())
            .ok_or(ErroDaPonte::PedidoIncompleto)
            .and_then(|texto| {
                STANDARD
                    .decode(texto)
                    .map_err(|_| ErroDaPonte::PedidoIncompleto)
            }),
    }
}

/// A resposta com bytes, em base64.
#[derive(Debug, Serialize)]
pub struct Bytes {
    pub base64: String,
}

impl From<Vec<u8>> for Bytes {
    fn from(bytes: Vec<u8>) -> Self {
        Bytes {
            base64: STANDARD.encode(bytes),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_resposta_volta_em_base64() {
        assert_eq!(Bytes::from(vec![0xFF, 0xD8, 0xFF]).base64, "/9j/");
    }
}
