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

use std::cell::{Cell, RefCell};

use biblioteca_core::acesso::Acesso;
use biblioteca_core::local::{LocalDaSessao, LocalDoCaixa};

thread_local! {
    static ESTUDIOS_RESTRITOS: Cell<bool> = const { Cell::new(false) };
    /// 📍 O acesso inteiro — o que as telas do local leem (18/out/2026).
    static ACESSO: RefCell<Option<Acesso>> = const { RefCell::new(None) };
}

/// A raiz guarda o acesso da conta que entrou; na saída, `None`.
pub fn definir_acesso(acesso: Option<Acesso>) {
    ACESSO.with(|a| *a.borrow_mut() = acesso);
}

fn com_acesso<T>(f: impl FnOnce(Option<&Acesso>) -> T) -> T {
    ACESSO.with(|a| f(a.borrow().as_ref()))
}

/// 📍 Em qual local a sessão nasce.
pub fn local_padrao() -> LocalDaSessao {
    com_acesso(|a| a.map(|a| a.local_padrao).unwrap_or_default())
}

/// 📍 Pode escolher outro local ao criar e corrigir o local depois? O
/// administrador sempre pode.
pub fn pode_mudar_local() -> bool {
    com_acesso(|a| a.is_some_and(|a| a.admin || a.pode_mudar_local))
}

/// 📍 Esta conta vê o rótulo deste local?
pub fn mostra_local(local: LocalDaSessao) -> bool {
    com_acesso(|a| a.is_none_or(|a| a.mostra_local(local)))
}

/// 📍 Vê os dois locais — e então a lista mostra a coluna e os totais por local.
pub fn mostra_os_locais() -> bool {
    LocalDaSessao::TODOS.into_iter().all(mostra_local)
}

/// 📍 Os locais que esta conta não vê.
pub fn locais_ocultos() -> Vec<LocalDaSessao> {
    com_acesso(|a| a.map(|a| a.locais_ocultos.clone()).unwrap_or_default())
}

/// 📍 Os caixas que a conta pode ter abertos em cada estúdio.
pub fn caixas() -> Vec<LocalDoCaixa> {
    com_acesso(|a| {
        a.map(|a| a.caixas.clone())
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| vec![Some(LocalDaSessao::Estudio)])
    })
}

/// 📍 O caixa que recebe a venda de uma sessão neste local.
pub fn caixa_da_sessao(local: LocalDaSessao) -> LocalDoCaixa {
    com_acesso(|a| {
        a.map(|a| a.caixa_de_cada_local)
            .unwrap_or_default()
            .da_sessao(local)
    })
}

/// 📍 "Caixa Externo", "Caixa Geral" — ou só "Caixa" para quem tem um só.
pub fn nome_do_caixa(local: LocalDoCaixa) -> String {
    biblioteca_core::local::nome_do_caixa(local, &caixas(), &locais_ocultos())
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
