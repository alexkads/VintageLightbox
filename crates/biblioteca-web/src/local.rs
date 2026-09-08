//! O depósito local — o esquema do IndexedDB que o editor de revelação e a
//! biblioteca compartilham.
//!
//! # Por que o esquema mora aqui, e só aqui
//!
//! 🔑 [`esquema`] é o que o lado TypeScript (`lib/biblioteca/local.ts`) lê
//! antes de abrir o banco: nome, versão e lojas vêm de uma fonte só. Dois
//! esquemas para o mesmo banco seriam a armadilha nº 8 no lugar em que ela
//! apaga dado de cliente — um `onupgradeneeded` de cada lado, com listas
//! diferentes, e o banco fica sem a loja que o outro esperava.
//!
//! # Quem lê e quem grava
//!
//! O editor grava os ajustes **a cada gesto** (sem API); "Salvar na galeria" é
//! o passo que sincroniza. A biblioteca lê a mesma loja e marca "editada · não
//! salva" — **em React**, pelo `local.ts`. Este wasm não abre o banco: ele só
//! diz como o banco é.
//!
//! ⚠️ As lojas `galerias` e `parametros` ficaram sem uso quando a tela voltou
//! ao React (2026-09-05). Continuam no esquema porque tirá-las é uma versão
//! nova do banco (`onupgradeneeded` nos navegadores de todo mundo), e isso é
//! decisão para quando houver um segundo motivo.

use serde::Serialize;

pub const NOME: &str = "recordarfotos-biblioteca";
pub const VERSAO: u32 = 1;
/// As lojas: cada uma guarda JSON em texto, com a chave fora do valor.
pub const LOJAS: [&str; 3] = [LOJA_REVELACOES, LOJA_GALERIAS, LOJA_PARAMETROS];
/// Por foto: os ajustes e o enquadramento, e se já subiram.
pub const LOJA_REVELACOES: &str = "revelacoes";
/// Por galeria: reservada (sem uso desde que a tela voltou ao React).
pub const LOJA_GALERIAS: &str = "galerias";
/// Preferências: reservada (idem).
pub const LOJA_PARAMETROS: &str = "parametros";

#[derive(Serialize)]
pub struct Esquema {
    pub nome: &'static str,
    pub versao: u32,
    pub lojas: [&'static str; 3],
}

pub fn esquema() -> Esquema {
    Esquema {
        nome: NOME,
        versao: VERSAO,
        lojas: LOJAS,
    }
}
