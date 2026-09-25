//! ⏱️ **A régua da thread que desenha** — quanto cada trecho prende a tela.
//!
//! O teste de carga (`e2e::carga`) diz *se* a tela ficou presa; esta régua diz
//! *onde*. Cada ponto de entrada da interface abre um [`trecho`] e, ao sair do
//! escopo, soma o tempo gasto nele:
//!
//! ```ignore
//! let _t = crate::regua::trecho("detalhe: colher");
//! ```
//!
//! 🔑 **Só existe nos testes.** Fora deles o [`Trecho`] é um tipo vazio e o
//! `Drop` não faz nada: o app de verdade não paga nem a leitura do relógio.
//! Não há profiler na máquina de todo mundo (`perf` pede privilégio), e a
//! pergunta "o que prende a tela durante a importação" tem de ter resposta
//! rodando `cargo test`.
//!
//! ⚠️ **Por thread**, e não global: os testes do GPUI rodam em paralelo, cada
//! um na sua thread, e uma régua compartilhada somaria o tempo de todos.

/// Um trecho sendo medido. Soma o tempo ao sair do escopo.
#[must_use = "o trecho mede até sair do escopo: guarde-o em `_t`"]
pub struct Trecho {
    #[cfg(test)]
    nome: &'static str,
    #[cfg(test)]
    inicio: std::time::Instant,
}

/// Começa a medir `nome` até o fim do escopo.
#[inline(always)]
pub fn trecho(nome: &'static str) -> Trecho {
    #[cfg(not(test))]
    let _ = nome;
    Trecho {
        #[cfg(test)]
        nome,
        #[cfg(test)]
        inicio: std::time::Instant::now(),
    }
}

impl Drop for Trecho {
    #[inline(always)]
    fn drop(&mut self) {
        #[cfg(test)]
        medidas::anotar(self.nome, self.inicio.elapsed());
    }
}

#[cfg(test)]
pub use medidas::{pior, relatorio, zerar};

#[cfg(test)]
mod medidas {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::time::Duration;

    /// `(vezes, total, pior)` por trecho.
    type Soma = (u32, Duration, Duration);

    thread_local! {
        static SOMAS: RefCell<BTreeMap<&'static str, Soma>> = const { RefCell::new(BTreeMap::new()) };
    }

    pub(super) fn anotar(nome: &'static str, gasto: Duration) {
        SOMAS.with(|somas| {
            let mut somas = somas.borrow_mut();
            let soma = somas.entry(nome).or_default();
            soma.0 += 1;
            soma.1 += gasto;
            soma.2 = soma.2.max(gasto);
        });
    }

    /// Esquece o que foi medido até aqui — o começo da parte que interessa.
    pub fn zerar() {
        SOMAS.with(|somas| somas.borrow_mut().clear());
    }

    /// O trecho que mais tempo prendeu a thread **de uma vez só**, e quanto.
    ///
    /// 🔑 É o que o operador sente: um quadro perdido é um bloco contínuo
    /// maior que 16,7 ms, e não a soma de muitos pequenos entre quadros.
    pub fn pior() -> Option<(&'static str, Duration)> {
        SOMAS.with(|somas| {
            somas
                .borrow()
                .iter()
                .map(|(nome, (_, _, pior))| (*nome, *pior))
                .max_by_key(|(_, pior)| *pior)
        })
    }

    /// Os trechos do mais caro para o mais barato, pelo total.
    ///
    /// ⚠️ Os trechos se aninham (o `render` da raiz contém o da tela da
    /// sessão): o total de um pai **inclui** o dos filhos, e somar a coluna
    /// contaria duas vezes.
    pub fn relatorio() -> String {
        SOMAS.with(|somas| {
            let mut linhas: Vec<(&'static str, Soma)> =
                somas.borrow().iter().map(|(n, s)| (*n, *s)).collect();
            linhas.sort_by_key(|(_, (_, total, _))| std::cmp::Reverse(*total));
            let mut texto = format!(
                "   {:<44} {:>6} {:>11} {:>10} {:>10}\n",
                "trecho", "vezes", "total", "média", "pior"
            );
            for (nome, (vezes, total, pior)) in linhas {
                texto.push_str(&format!(
                    "   {:<44} {:>6} {:>11.2?} {:>10.2?} {:>10.2?}\n",
                    nome,
                    vezes,
                    total,
                    total / vezes.max(1),
                    pior
                ));
            }
            texto
        })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn soma_por_trecho_e_guarda_o_pior() {
        zerar();
        for _ in 0..3 {
            let _t = trecho("a");
        }
        {
            let _t = trecho("b");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let texto = relatorio();
        let a = texto
            .lines()
            .find(|l| l.trim_start().starts_with("a "))
            .unwrap();
        assert!(a.contains(" 3 "), "três vezes: {a}");
        let primeira = texto.lines().nth(1).unwrap();
        assert!(
            primeira.trim_start().starts_with("b "),
            "o mais caro vem primeiro:\n{texto}"
        );
    }
}
