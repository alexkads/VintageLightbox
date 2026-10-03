//! Os sons de fábrica, **sintetizados em código**.
//!
//! 🔑 Sem arquivo e sem licença: cada som é uma soma de senoides com envelope
//! (ataque curto, queda exponencial), e sai igual no Mac, no Windows e no
//! Linux. O ataque de 5 ms existe para o som não começar com um estalo.

use std::f32::consts::TAU;

/// A taxa em que os sons de fábrica são gerados.
pub const TAXA: u32 = 44_100;

/// O pico de todo som de fábrica — o volume do operador vem por cima.
const PICO: f32 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Embutido {
    Sino,
    Campainha,
    DoisToques,
    Descida,
    Alerta,
    Suave,
    Moeda,
}

impl Embutido {
    pub const TODOS: [Embutido; 7] = [
        Embutido::Sino,
        Embutido::Campainha,
        Embutido::DoisToques,
        Embutido::Descida,
        Embutido::Alerta,
        Embutido::Suave,
        Embutido::Moeda,
    ];

    pub fn chave(self) -> &'static str {
        match self {
            Embutido::Sino => "sino",
            Embutido::Campainha => "campainha",
            Embutido::DoisToques => "dois_toques",
            Embutido::Descida => "descida",
            Embutido::Alerta => "alerta",
            Embutido::Suave => "suave",
            Embutido::Moeda => "moeda",
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Embutido::Sino => "Sino",
            Embutido::Campainha => "Campainha",
            Embutido::DoisToques => "Dois toques",
            Embutido::Descida => "Descida",
            Embutido::Alerta => "Alerta",
            Embutido::Suave => "Suave",
            Embutido::Moeda => "Moeda",
        }
    }

    pub fn da_chave(chave: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|e| e.chave() == chave)
    }

    /// As amostras, mono, em [`TAXA`].
    pub fn amostras(self) -> Vec<f32> {
        let mut som = match self {
            // Um sino agudo, com o parcial inarmônico que dá o "metal".
            Embutido::Sino => nota(1318.5, 1.2, 3.5, &[(1.0, 1.0), (2.76, 0.35), (5.4, 0.12)]),
            // Dim-dom: a terça maior descendo.
            Embutido::Campainha => juntar(&[
                nota(659.25, 0.45, 4.0, &[(1.0, 1.0), (2.0, 0.25)]),
                nota(523.25, 0.8, 3.0, &[(1.0, 1.0), (2.0, 0.25)]),
            ]),
            Embutido::DoisToques => juntar(&[
                nota(880.0, 0.12, 12.0, &[(1.0, 1.0), (2.0, 0.2)]),
                silencio(0.08),
                nota(880.0, 0.25, 8.0, &[(1.0, 1.0), (2.0, 0.2)]),
            ]),
            // Três notas descendo — o "desfeito".
            Embutido::Descida => juntar(&[
                nota(784.0, 0.14, 9.0, &[(1.0, 1.0), (2.0, 0.2)]),
                nota(587.33, 0.14, 9.0, &[(1.0, 1.0), (2.0, 0.2)]),
                nota(440.0, 0.4, 5.0, &[(1.0, 1.0), (2.0, 0.2)]),
            ]),
            // Três bipes ásperos (ímpares somados): é o que pede atenção.
            Embutido::Alerta => juntar(&[
                nota(988.0, 0.09, 6.0, &[(1.0, 1.0), (3.0, 0.3), (5.0, 0.15)]),
                silencio(0.06),
                nota(988.0, 0.09, 6.0, &[(1.0, 1.0), (3.0, 0.3), (5.0, 0.15)]),
                silencio(0.06),
                nota(988.0, 0.09, 6.0, &[(1.0, 1.0), (3.0, 0.3), (5.0, 0.15)]),
            ]),
            // O acorde maior, macio: deu certo, sem pressa.
            Embutido::Suave => misturar(&[
                nota(523.25, 0.9, 3.0, &[(1.0, 1.0)]),
                nota(659.25, 0.9, 3.0, &[(1.0, 0.8)]),
                nota(783.99, 0.9, 3.0, &[(1.0, 0.6)]),
            ]),
            Embutido::Moeda => juntar(&[
                nota(987.77, 0.08, 4.0, &[(1.0, 1.0), (2.0, 0.3)]),
                nota(1318.5, 0.45, 5.0, &[(1.0, 1.0), (2.0, 0.3)]),
            ]),
        };
        normalizar(&mut som);
        som
    }
}

/// Uma nota: a fundamental e os parciais `(múltiplo, peso)`, com ataque de
/// 5 ms e queda exponencial de constante `queda` por segundo.
fn nota(frequencia: f32, duracao: f32, queda: f32, parciais: &[(f32, f32)]) -> Vec<f32> {
    let total = (duracao * TAXA as f32) as usize;
    let ataque = (0.005 * TAXA as f32) as usize;
    // Os últimos 10 ms vão a zero, para a nota não terminar num degrau.
    let fim = (0.010 * TAXA as f32) as usize;
    (0..total)
        .map(|i| {
            let t = i as f32 / TAXA as f32;
            let onda: f32 = parciais
                .iter()
                .map(|(multiplo, peso)| peso * (TAU * frequencia * multiplo * t).sin())
                .sum();
            let subida = (i as f32 / ataque as f32).min(1.0);
            let descida = ((total - i) as f32 / fim as f32).min(1.0);
            onda * (-queda * t).exp() * subida * descida
        })
        .collect()
}

fn silencio(duracao: f32) -> Vec<f32> {
    vec![0.0; (duracao * TAXA as f32) as usize]
}

fn juntar(partes: &[Vec<f32>]) -> Vec<f32> {
    partes.concat()
}

fn misturar(partes: &[Vec<f32>]) -> Vec<f32> {
    let tamanho = partes.iter().map(Vec::len).max().unwrap_or(0);
    (0..tamanho)
        .map(|i| partes.iter().filter_map(|p| p.get(i)).sum())
        .collect()
}

fn normalizar(som: &mut [f32]) {
    let maior = som.iter().fold(0.0f32, |m, a| m.max(a.abs()));
    if maior > 0.0 {
        for amostra in som.iter_mut() {
            *amostra *= PICO / maior;
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Todo som de fábrica toca alguma coisa, curto, sem estourar e sem
    /// estalo no começo nem no fim.
    #[test]
    fn todo_som_de_fabrica_e_curto_e_limpo() {
        for som in Embutido::TODOS {
            let amostras = som.amostras();
            let segundos = amostras.len() as f32 / TAXA as f32;
            assert!((0.2..2.0).contains(&segundos), "{som:?} dura {segundos} s");
            assert!(
                amostras.iter().all(|a| a.is_finite() && a.abs() <= 1.0),
                "{som:?} sai da faixa"
            );
            let pico = amostras.iter().fold(0.0f32, |m, a| m.max(a.abs()));
            assert!((pico - PICO).abs() < 1e-3, "{som:?} com pico {pico}");
            assert!(amostras[0].abs() < 1e-3, "{som:?} começa com estalo");
            assert!(
                amostras.last().unwrap().abs() < 0.01,
                "{som:?} termina num degrau"
            );
        }
    }

    #[test]
    fn a_chave_volta_ao_som() {
        for som in Embutido::TODOS {
            assert_eq!(Embutido::da_chave(som.chave()), Some(som));
        }
        assert_eq!(Embutido::da_chave("meu:x.wav"), None);
    }
}
