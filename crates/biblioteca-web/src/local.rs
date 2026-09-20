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
//! ⚠️ A loja `galerias` ficou sem uso quando a tela voltou ao React
//! (2026-09-05). Continua no esquema porque tirá-la é `onupgradeneeded` nos
//! navegadores de todo mundo para não ganhar nada.
//!
//! # A versão 2 — a miniatura revelada (2026-09-11)
//!
//! 🚨 **Guardar a receita e não guardar a imagem deixava a tela mentindo.**
//! Desde que o "Sincronizar" passou a copiar só parâmetros, as fotos alvo
//! ficam com a receita nova e o JPEG **antigo** no acervo — e a tira e a grade
//! desenham o JPEG do servidor. O operador sincronizava sete fotos e continuava
//! vendo sete miniaturas sem efeito (dono, 2026-09-11): *"tá deixando as
//! miniaturas e a foto central sem efeito"*.
//!
//! A [`LOJA_PREVIAS`] é onde a miniatura revelada **aqui** espera a subida. Ela
//! guarda `Blob`, e não texto: é a única loja que não é JSON, e o wasm não a
//! lê — mas o esquema continua saindo de um lugar só, que é o ponto deste
//! módulo.

use serde::Serialize;

pub const NOME: &str = "recordarfotos-biblioteca";
/// 🚨 **Subir esta versão dispara `onupgradeneeded` em toda aba aberta.** O
/// laço do `local.ts` cria só a loja que falta e não toca no que há, então
/// nada se perde — mas uma aba velha segurando o banco **bloqueia** a nova, e
/// é por isso que o `local.ts` trata `onblocked` em vez de esperar para
/// sempre.
pub const VERSAO: u32 = 2;
/// As lojas. As três primeiras guardam JSON em texto, com a chave fora do
/// valor; a quarta guarda `Blob`.
pub const LOJAS: [&str; 4] = [
    LOJA_REVELACOES,
    LOJA_GALERIAS,
    LOJA_PARAMETROS,
    LOJA_PREVIAS,
];
/// Por foto: os ajustes e o enquadramento, e se já subiram.
pub const LOJA_REVELACOES: &str = "revelacoes";
/// Por galeria: reservada (sem uso desde que a tela voltou ao React).
pub const LOJA_GALERIAS: &str = "galerias";
/// Preferências de quem opera.
pub const LOJA_PARAMETROS: &str = "parametros";
/// Por foto: a miniatura revelada que ainda não subiu para a galeria.
///
/// 🔑 **É cache, e não verdade.** A verdade da foto é a receita em
/// [`LOJA_REVELACOES`]; esta loja só evita refazer a imagem a cada abertura da
/// tela. Some com o depósito sem prejuízo nenhum além de uma espera.
pub const LOJA_PREVIAS: &str = "previas";

#[derive(Serialize)]
pub struct Esquema {
    pub nome: &'static str,
    pub versao: u32,
    pub lojas: [&'static str; 4],
}

pub fn esquema() -> Esquema {
    Esquema {
        nome: NOME,
        versao: VERSAO,
        lojas: LOJAS,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 🚨 **A lista e o array andam juntos.** `LOJAS` tem tamanho fixo, e
    /// acrescentar uma loja sem mexer no `Esquema` não compila — mas trocar
    /// uma pela outra compila e calado. Este caso prende os nomes.
    #[test]
    fn o_esquema_leva_as_quatro_lojas() {
        let e = esquema();
        assert_eq!(e.versao, 2, "a loja das prévias entrou na 2");
        assert_eq!(
            e.lojas,
            ["revelacoes", "galerias", "parametros", "previas"],
            "é esta lista que o local.ts cria no onupgradeneeded"
        );
    }
}
