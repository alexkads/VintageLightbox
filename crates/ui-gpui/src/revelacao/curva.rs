//! A curva de tons: onde cada nível de entrada vai parar depois dos ajustes.
//!
//! ## 🚨 Ela **não** é a curva de tons dos `tone_curve_*`
//!
//! `Ajustes` tem quatro campos com esse nome e o shader os aplica, mas nenhum
//! controle os escreve — em nenhum dos dois apps
//! ([`super::controles`] explica por quê). O que esta curva desenha é outra
//! coisa: uma **leitura** de exposição, contraste, altas luzes, sombras, brancos
//! e pretos, mostrada como gráfico.
//!
//! Ou seja: a seção "Tone Curve" do legado tem esse nome e não toca nos
//! parâmetros que levam o nome dela. Portar a curva é portar o gráfico, e nada
//! além disso — dar-lhe pontos arrastáveis seria feature nova (§7.1).
//!
//! ## ⚠️ E ela é uma aproximação, não o shader
//!
//! A conta aqui é a do `tone_curve.rs` do legado, copiada nas constantes: `0.1`
//! por ponto de exposição, `0.05` nas altas luzes e sombras, `0.03` nos brancos e
//! pretos. **O shader faz outra conta** (`image_adjustments.wgsl` trabalha em
//! 0–255 e tem limiares em 192 e 64). O gráfico é uma ilustração do sentido de
//! cada ajuste, e não uma previsão do pixel — copiar a aproximação de lá é o que
//! mantém os dois apps mostrando o mesmo desenho.

use super::processador::Ajustes;

/// Quantos pontos a curva tem. Os mesmos 101 do legado (`0..=100`).
pub const PONTOS: usize = 101;

/// Para cada entrada de 0 a 1, a saída depois dos ajustes.
pub fn curva(ajustes: &Ajustes) -> [f32; PONTOS] {
    let mut saida = [0.0; PONTOS];

    for (i, ponto) in saida.iter_mut().enumerate() {
        let entrada = i as f32 / (PONTOS - 1) as f32;
        let mut valor = entrada;

        // Exposição levanta a curva inteira.
        valor += ajustes.exposure * 0.1;

        // Contraste gira em torno do meio.
        valor = (valor - 0.5) * ajustes.contrast + 0.5;

        // Altas luzes e sombras pegam metade cada uma; brancos e pretos, um
        // quarto. O peso cresce conforme se afasta do meio, e é o que dá à curva
        // a forma de S.
        if entrada > 0.5 {
            valor += ajustes.highlights * 0.05 * ((entrada - 0.5) * 2.0);
        } else {
            valor += ajustes.shadows * 0.05 * ((0.5 - entrada) * 2.0);
        }
        if entrada > 0.75 {
            valor += ajustes.whites * 0.03 * ((entrada - 0.75) * 4.0);
        } else if entrada < 0.25 {
            valor += ajustes.blacks * 0.03 * ((0.25 - entrada) * 4.0);
        }

        *ponto = valor.clamp(0.0, 1.0);
    }

    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    fn perto(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// 🚨 No neutro a curva é a diagonal — entrada igual a saída.
    ///
    /// É o teste que pega a conta de contraste escrita errada: o legado usa
    /// `1.0 + (contrast - 1.0)`, que é o próprio `contrast`. Quem lê aquela linha
    /// e "simplifica" para `1.0 + contrast` levanta a curva inteira no neutro, e
    /// o gráfico passa a mostrar uma foto contrastada que ninguém pediu.
    #[test]
    fn no_neutro_a_curva_e_a_diagonal() {
        let curva = curva(&Ajustes::default());

        for (i, valor) in curva.iter().enumerate() {
            let entrada = i as f32 / (PONTOS - 1) as f32;
            assert!(
                perto(*valor, entrada),
                "no ponto {i} a curva devia valer {entrada} e vale {valor}"
            );
        }
    }

    /// Exposição levanta a curva inteira.
    #[test]
    fn a_exposicao_levanta_tudo() {
        let curva = curva(&Ajustes {
            exposure: 1.0,
            ..Default::default()
        });

        assert!(perto(curva[0], 0.1), "o preto sobe 0,1");
        assert!(perto(curva[50], 0.6));
        assert!(perto(curva[100], 1.0), "o branco já estava no teto");
    }

    /// ⚠️ Contraste gira em torno do meio: o meio não se move.
    #[test]
    fn o_contraste_gira_em_torno_do_meio() {
        let curva = curva(&Ajustes {
            contrast: 1.5,
            ..Default::default()
        });

        assert!(perto(curva[50], 0.5), "o ponto do meio fica parado");
        assert!(curva[25] < 0.25, "abaixo do meio escurece");
        assert!(curva[75] > 0.75, "acima do meio clareia");
    }

    /// As altas luzes só mexem na metade de cima; as sombras, na de baixo.
    #[test]
    fn cada_faixa_mexe_so_na_sua_metade() {
        let altas = curva(&Ajustes {
            highlights: 50.0,
            ..Default::default()
        });
        let sombras = curva(&Ajustes {
            shadows: 50.0,
            ..Default::default()
        });
        let neutra = curva(&Ajustes::default());

        assert!(
            perto(altas[25], neutra[25]),
            "altas luzes não tocam o quarto de baixo"
        );
        assert!(altas[75] > neutra[75]);
        assert!(
            perto(sombras[75], neutra[75]),
            "sombras não tocam o quarto de cima"
        );
        assert!(sombras[25] > neutra[25]);
    }

    /// Brancos e pretos só mexem nos extremos.
    #[test]
    fn brancos_e_pretos_ficam_nos_extremos() {
        let neutra = curva(&Ajustes::default());
        let brancos = curva(&Ajustes {
            whites: 50.0,
            ..Default::default()
        });
        let pretos = curva(&Ajustes {
            blacks: -50.0,
            ..Default::default()
        });

        assert!(
            perto(brancos[50], neutra[50]),
            "o meio não sente os brancos"
        );
        assert!(brancos[90] > neutra[90]);
        assert!(perto(pretos[50], neutra[50]));
        assert!(pretos[10] < neutra[10]);
    }

    /// A curva nunca sai de 0..1 — o gráfico tem moldura fixa.
    #[test]
    fn a_curva_fica_dentro_da_moldura() {
        let extremo = curva(&Ajustes {
            exposure: 5.0,
            contrast: 2.0,
            highlights: 100.0,
            shadows: 100.0,
            whites: 100.0,
            blacks: 100.0,
            ..Default::default()
        });

        for valor in extremo {
            assert!((0.0..=1.0).contains(&valor), "{valor} saiu da moldura");
        }
    }

    /// São 101 pontos, como no legado.
    #[test]
    fn sao_cento_e_um_pontos() {
        assert_eq!(curva(&Ajustes::default()).len(), 101);
    }
}
