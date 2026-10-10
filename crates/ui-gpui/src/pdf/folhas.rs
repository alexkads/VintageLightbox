//! 📐 A coluna de páginas: onde cada folha fica, quais estão à vista e qual é
//! "a página atual". Conta pura — sem janela, sem imagem.

use std::ops::Range;

use visualizador_pdf::Folha;

/// O respiro em volta da coluna de páginas, em pixels.
pub const RESPIRO: f32 = 16.;
/// O espaço entre uma página e a seguinte — o mesmo do respiro: com a
/// página inteira no palco, a seguinte começa exatamente onde ele acaba (com
/// 12 sobrava um fio de 4 px dela no pé).
pub const ENTRE: f32 = 16.;
/// Os degraus do zoom. `1.0` é a página inteira dentro do palco.
pub const DEGRAUS: [f32; 6] = [0.5, 0.75, 1.0, 1.5, 2.0, 3.0];

/// O degrau seguinte (`+1`) ou o anterior (`-1`), parando nas pontas.
pub fn degrau(zoom: f32, passo: i32) -> f32 {
    let atual = DEGRAUS
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - zoom).abs().total_cmp(&(b.1 - zoom).abs()))
        .map(|(i, _)| i as i32)
        .unwrap_or(2);
    DEGRAUS[(atual + passo).clamp(0, DEGRAUS.len() as i32 - 1) as usize]
}

/// A largura de toda página na tela. No zoom 1 a página de proporção
/// `aspecto` **cabe inteira** no palco, com o respiro em volta: quem abre o
/// livro vê a capa toda, e não a metade de cima dela (a primeira versão
/// ajustava só à largura, e a página deitada passava do pé do palco).
pub fn largura_da_pagina(palco: (f32, f32), aspecto: f32, zoom: f32) -> f32 {
    let pela_largura = palco.0 - 2. * RESPIRO;
    let pela_altura = (palco.1 - 2. * RESPIRO) * aspecto.max(0.01);
    (pela_largura.min(pela_altura) * zoom).max(64.)
}

/// O topo e a altura de cada página dentro da coluna, com todas na mesma
/// largura (a altura sai da proporção de cada folha).
pub fn posicoes(folhas: &[Folha], largura: f32) -> Vec<(f32, f32)> {
    let mut topo = RESPIRO;
    folhas
        .iter()
        .map(|folha| {
            let altura = (largura / folha.aspecto().max(0.01)).max(1.);
            let lugar = (topo, altura);
            topo += altura + ENTRE;
            lugar
        })
        .collect()
}

/// A altura da coluna inteira, com o respiro de cima e o de baixo.
pub fn altura_total(posicoes: &[(f32, f32)]) -> f32 {
    posicoes
        .last()
        .map_or(2. * RESPIRO, |(topo, altura)| topo + altura + RESPIRO)
}

/// As páginas que tocam a janela `[rolado, rolado + altura_do_palco]`.
pub fn a_vista(posicoes: &[(f32, f32)], rolado: f32, altura_do_palco: f32) -> Range<usize> {
    let fim_da_janela = rolado + altura_do_palco;
    let primeira = posicoes.partition_point(|(topo, altura)| topo + altura < rolado);
    let depois = posicoes.partition_point(|(topo, _)| *topo <= fim_da_janela);
    primeira..depois.max(primeira)
}

/// A página atual: a que está no meio do palco (ou a última acima dele, se o
/// meio cair no espaço entre duas).
pub fn pagina_atual(posicoes: &[(f32, f32)], rolado: f32, altura_do_palco: f32) -> usize {
    let meio = rolado + altura_do_palco / 2.;
    posicoes
        .partition_point(|(topo, _)| *topo <= meio)
        .saturating_sub(1)
}

/// Quanto rolar para a página ficar no alto do palco, sem passar do fim.
pub fn rolar_ate(posicoes: &[(f32, f32)], pagina: usize, altura_do_palco: f32) -> f32 {
    let Some((topo, _)) = posicoes.get(pagina) else {
        return 0.;
    };
    let maximo = (altura_total(posicoes) - altura_do_palco).max(0.);
    (topo - RESPIRO).clamp(0., maximo)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Três páginas deitadas de 400 × 200 numa coluna de 400 de largura.
    fn tres() -> Vec<(f32, f32)> {
        let folha = Folha {
            largura: 800.,
            altura: 400.,
        };
        posicoes(&[folha; 3], 400.)
    }

    #[test]
    fn a_coluna_empilha_com_respiro_e_espaco() {
        let lugares = tres();
        assert_eq!(lugares, vec![(16., 200.), (232., 200.), (448., 200.)]);
        assert_eq!(altura_total(&lugares), 664.);
        assert_eq!(altura_total(&[]), 32.);
        // A folha em pé no meio de deitadas: mesma largura, altura própria.
        let mistas = posicoes(
            &[
                Folha {
                    largura: 800.,
                    altura: 400.,
                },
                Folha {
                    largura: 400.,
                    altura: 800.,
                },
            ],
            400.,
        );
        assert_eq!(mistas[1], (232., 800.));
    }

    #[test]
    fn a_vista_e_a_pagina_atual_seguem_a_rolagem() {
        let lugares = tres();
        // No alto, um palco de 300: a primeira inteira e o começo da segunda.
        assert_eq!(a_vista(&lugares, 0., 300.), 0..2);
        assert_eq!(pagina_atual(&lugares, 0., 300.), 0);
        // Rolado 250: a primeira saiu, a segunda e a terceira estão à vista.
        assert_eq!(a_vista(&lugares, 250., 300.), 1..3);
        assert_eq!(pagina_atual(&lugares, 250., 300.), 1);
        // No fim.
        assert_eq!(a_vista(&lugares, 364., 300.), 1..3);
        assert_eq!(pagina_atual(&lugares, 364., 300.), 2);
        // Sem página nenhuma, nada à vista e nada quebra.
        assert_eq!(a_vista(&[], 0., 300.), 0..0);
        assert_eq!(pagina_atual(&[], 0., 300.), 0);
    }

    #[test]
    fn rolar_ate_poe_a_pagina_no_alto_sem_passar_do_fim() {
        let lugares = tres();
        assert_eq!(rolar_ate(&lugares, 0, 300.), 0.);
        assert_eq!(rolar_ate(&lugares, 1, 300.), 216.);
        // A última não sobe até o alto: a coluna acaba antes.
        assert_eq!(rolar_ate(&lugares, 2, 300.), 364.);
        assert_eq!(rolar_ate(&lugares, 9, 300.), 0.);
        // E quem rola para a página está nela.
        assert_eq!(
            pagina_atual(&lugares, rolar_ate(&lugares, 1, 300.), 300.),
            1
        );
    }

    #[test]
    fn o_zoom_anda_por_degraus_e_para_nas_pontas() {
        assert_eq!(degrau(1.0, 1), 1.5);
        assert_eq!(degrau(1.0, -1), 0.75);
        assert_eq!(degrau(3.0, 1), 3.0);
        assert_eq!(degrau(0.5, -1), 0.5);
        // Um zoom fora dos degraus vai para o vizinho do mais próximo.
        assert_eq!(degrau(1.4, 1), 2.0);
        // Palco largo e baixo: quem manda é a altura — a página de 2:1 cabe
        // inteira em 332 de altura (300 sem o respiro) com 600 de largura.
        assert_eq!(largura_da_pagina((832., 332.), 2.0, 1.0), 600.);
        // Palco alto: quem manda é a largura.
        assert_eq!(largura_da_pagina((832., 900.), 2.0, 1.0), 800.);
        assert_eq!(largura_da_pagina((832., 900.), 2.0, 2.0), 1600.);
        assert_eq!(largura_da_pagina((10., 10.), 2.0, 1.0), 64.);
    }
}
