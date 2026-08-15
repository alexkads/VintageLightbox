//! Como a grade da Biblioteca se organiza — sem GPU, sem janela, sem GPUI.
//!
//! O `uniform_list` do GPUI virtualiza **linhas**: ele pergunta "renderize os
//! itens de 40 a 55" e só esses existem. Uma grade é isso com N fotos por
//! linha, e as contas de "quantas cabem" e "quais vão nesta linha" são o que
//! decide se 2.000 fotos rolam a 60fps ou engasgam.
//!
//! Elas moram aqui, fora do componente, porque é a parte que um teste alcança.

/// Quantas fotos cabem numa linha, dada a largura disponível.
///
/// `largura_do_item` já inclui o espaçamento — quem chama soma os dois antes,
/// para não haver duas opiniões sobre onde o `gap` entra.
///
/// **Nunca devolve zero.** Numa janela mais estreita que uma miniatura, a
/// resposta honesta é "uma por linha, com corte" — devolver zero faria
/// `total / colunas` dividir por zero, e a grade inteira desaparecer no
/// momento em que alguém arrasta a borda da janela para a esquerda.
pub fn colunas_que_cabem(largura_disponivel: f32, largura_do_item: f32) -> usize {
    if largura_do_item <= 0.0 || !largura_disponivel.is_finite() {
        return 1;
    }

    ((largura_disponivel / largura_do_item).floor() as usize).max(1)
}

/// Quantas linhas a grade tem.
///
/// Divisão para cima: 7 fotos em 3 colunas são 3 linhas, e não 2 — a última
/// fica pela metade. Arredondar para baixo esconderia a última linha inteira,
/// e o sintoma seria "as fotos mais recentes sumiram".
pub fn linhas_necessarias(total_de_fotos: usize, colunas: usize) -> usize {
    if colunas == 0 {
        return 0;
    }
    total_de_fotos.div_ceil(colunas)
}

/// A faixa de índices que a linha `indice` mostra.
///
/// Fechada no início, aberta no fim, como todo `Range` de Rust — e **recortada
/// pelo total**: a última linha costuma ser parcial, e pedir `fotos[24..27]`
/// num acervo de 25 é pânico, não tela vazia.
pub fn fotos_da_linha(indice: usize, colunas: usize, total: usize) -> std::ops::Range<usize> {
    let inicio = (indice * colunas).min(total);
    let fim = (inicio + colunas).min(total);
    inicio..fim
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cabem_quantas_couberem() {
        // 800px de largura, itens de 200px (miniatura + espaçamento) = 4.
        assert_eq!(colunas_que_cabem(800.0, 200.0), 4);
        // Sobra que não completa uma coluna não conta.
        assert_eq!(colunas_que_cabem(999.0, 200.0), 4);
    }

    #[test]
    fn janela_estreita_ainda_mostra_uma_coluna() {
        // O caso de arrastar a borda da janela até quase fechar. Zero colunas
        // faria `total / colunas` dividir por zero e a grade sumir.
        assert_eq!(colunas_que_cabem(50.0, 200.0), 1);
        assert_eq!(colunas_que_cabem(0.0, 200.0), 1);
    }

    #[test]
    fn largura_absurda_nao_derruba_a_conta() {
        // `f32::NAN` chega aqui quando o layout ainda não mediu o contêiner —
        // acontece no primeiro quadro. `NAN as usize` é 0 em Rust, e a grade
        // apareceria vazia por um quadro antes de se acertar.
        assert_eq!(colunas_que_cabem(f32::NAN, 200.0), 1);
        assert_eq!(colunas_que_cabem(f32::INFINITY, 200.0), 1);
        assert_eq!(colunas_que_cabem(800.0, 0.0), 1);
    }

    #[test]
    fn a_ultima_linha_parcial_conta_como_linha() {
        assert_eq!(linhas_necessarias(7, 3), 3);
        assert_eq!(linhas_necessarias(9, 3), 3);
        assert_eq!(linhas_necessarias(0, 3), 0);
        assert_eq!(linhas_necessarias(1, 3), 1);
    }

    #[test]
    fn cada_linha_pega_a_propria_fatia() {
        assert_eq!(fotos_da_linha(0, 4, 10), 0..4);
        assert_eq!(fotos_da_linha(1, 4, 10), 4..8);
    }

    #[test]
    fn a_ultima_linha_e_recortada_pelo_total() {
        // Sem o recorte isto seria `8..12` num acervo de 10 — e indexar
        // `fotos[8..12]` é pânico, não tela vazia.
        assert_eq!(fotos_da_linha(2, 4, 10), 8..10);
    }

    #[test]
    fn linha_alem_do_fim_e_vazia_e_nao_panico() {
        // O `uniform_list` pode pedir uma linha que deixou de existir entre a
        // medição e o desenho — quando o acervo encolhe por um filtro, por
        // exemplo.
        assert_eq!(fotos_da_linha(99, 4, 10), 10..10);
        assert!(fotos_da_linha(99, 4, 10).is_empty());
    }

    #[test]
    fn toda_foto_aparece_exatamente_uma_vez() {
        // A propriedade que importa: percorrer as linhas tem de reconstruir o
        // acervo inteiro, sem buraco e sem repetição. É o que uma grade
        // virtualizada mais facilmente erra, e o sintoma é a foto duplicada na
        // emenda entre duas linhas.
        for total in [0usize, 1, 7, 25, 2000] {
            for colunas in 1..=8 {
                let vistas: Vec<usize> = (0..linhas_necessarias(total, colunas))
                    .flat_map(|i| fotos_da_linha(i, colunas, total))
                    .collect();

                assert_eq!(
                    vistas,
                    (0..total).collect::<Vec<_>>(),
                    "total={total} colunas={colunas}"
                );
            }
        }
    }
}
