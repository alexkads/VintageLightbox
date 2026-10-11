//! 🔐 O que a conta logada alcança, para as telas se desenharem.
//!
//! Vem do `acesso` do `/auth/me` (os perfis de acesso do site, 11/out/2026) e é
//! guardado pela raiz quando a conta chega (`app/painel.rs`) — as telas que
//! têm seletor de estúdio (caixa, nova sessão, cabeçalho da sessão) leem daqui
//! em vez de cada uma receber a conta. **Só desenho**: quem autoriza é o
//! backend. A regra é `biblioteca_core::acesso`.
//!
//! `thread_local`, e não global: a interface inteira roda na thread principal,
//! e cada teste roda na sua — um não enxerga o perfil que o outro ligou.

use std::cell::Cell;

thread_local! {
    static ESTUDIOS_RESTRITOS: Cell<bool> = const { Cell::new(false) };
}

/// A raiz diz se o perfil de acesso da conta é de **alguns** estúdios. Na
/// saída da conta volta a `false`.
pub fn definir_estudios_restritos(sim: bool) {
    ESTUDIOS_RESTRITOS.with(|r| r.set(sim));
}

pub fn estudios_restritos() -> bool {
    ESTUDIOS_RESTRITOS.with(Cell::get)
}

/// O estúdio único da conta nesta lista — ver
/// [`biblioteca_core::acesso::estudio_unico`]. Com ele, a tela não pergunta o
/// estúdio nem oferece seletor.
pub fn estudio_unico<E>(estudios: &[E]) -> Option<&E> {
    biblioteca_core::acesso::estudio_unico(estudios, estudios_restritos())
}
