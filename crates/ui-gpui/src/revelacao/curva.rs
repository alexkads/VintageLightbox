//! A curva de tons: onde cada nível de entrada vai parar depois dos ajustes.
//!
//! ## 🚨 Até 17/ago/2026 ela ignorava os `tone_curve_*`
//!
//! `Ajustes` tem quatro campos com esse nome, o shader sempre os aplicou, e
//! **nenhum controle os escrevia** — nem aqui, nem no app de egui, cuja seção
//! "Tone Curve" desenhava um gráfico a partir dos ajustes do Básico e não tocava
//! nos parâmetros que levam o nome dela.
//!
//! Agora os quatro têm slider ([`super::controles`]), e **o gráfico os inclui**.
//! Ele voltaria a mentir se não incluísse: quatro controles logo abaixo de um
//! desenho que não se mexe quando eles se mexem é pior que não ter desenho.
//!
//! ## As duas metades desta conta têm precisões diferentes, e isso é deliberado
//!
//! | Parte | O que é |
//! |---|---|
//! | Básico (exposição, contraste, altas luzes, sombras, brancos, pretos) | **aproximação**, com as constantes do `tone_curve.rs` do legado: `0.1` por ponto de exposição, `0.05` nas altas luzes e sombras, `0.03` nos brancos e pretos |
//! | Curva de tons (as 4 zonas) | **a conta do shader, igual** — mesmos centros, mesma meia-largura, mesmo peso linear |
//!
//! ⚠️ **O Básico continua aproximado porque o shader trabalha em 0–255 com
//! limiares em 192 e 64**, e replicá-los aqui seria escrever a segunda
//! implementação daquela matemática — o defeito que a exportação tinha. A curva
//! de tons não tem esse problema: a conta dela é sobre luminância normalizada e
//! cabe inteira em quatro linhas, então aqui ela é exata.

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

        // As quatro zonas paramétricas, com a conta do shader.
        //
        // 🔑 **O shader mede a zona pela luminância JÁ ajustada** (`lum_final`,
        // depois de exposição, contraste e o resto), e não pela entrada. Usar a
        // entrada aqui faria o gráfico discordar da foto justamente quando os
        // dois grupos de controle estão em uso ao mesmo tempo — que é o caso
        // normal.
        for (valor_do_slider, centro) in [
            (ajustes.tone_curve_shadows, 0.125),
            (ajustes.tone_curve_darks, 0.375),
            (ajustes.tone_curve_lights, 0.625),
            (ajustes.tone_curve_highlights, 0.875),
        ] {
            if valor_do_slider == 0.0 {
                continue;
            }
            const MEIA_LARGURA: f32 = 0.25;
            let distancia = (valor - centro).abs();
            if distancia < MEIA_LARGURA {
                let peso = 1.0 - (distancia / MEIA_LARGURA);
                valor += valor * (valor_do_slider * 0.01 * peso);
            }
        }

        *ponto = valor.clamp(0.0, 1.0);
    }

    saida
}

// ------------------------------------------------------- a curva por ponto

/// Quantos pontos a curva por ponto tem, por canal.
pub const PONTOS_DA_CURVA: usize = 9;

/// A curva que devolve a foto como entrou: `y = x` nos nove x fixos.
pub fn curva_neutra() -> [f32; PONTOS_DA_CURVA] {
    std::array::from_fn(|i| i as f32 * 255.0 / 8.0)
}

/// Os quatro canais, na ordem em que a tela os oferece — `CANAIS_DA_CURVA`
/// do site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Canal {
    Rgb,
    Vermelho,
    Verde,
    Azul,
}

impl Canal {
    pub const TODOS: [Canal; 4] = [Canal::Rgb, Canal::Vermelho, Canal::Verde, Canal::Azul];

    pub fn rotulo(self) -> &'static str {
        match self {
            Canal::Rgb => "RGB",
            Canal::Vermelho => "Vermelho",
            Canal::Verde => "Verde",
            Canal::Azul => "Azul",
        }
    }

    /// A cor do traço: o tom 400 do Tailwind nos três de cor, como no site.
    /// `None` é o RGB, que segue a cor do texto do tema.
    pub fn cor(self) -> Option<u32> {
        match self {
            Canal::Rgb => None,
            Canal::Vermelho => Some(0xf87171),
            Canal::Verde => Some(0x4ade80),
            Canal::Azul => Some(0x60a5fa),
        }
    }

    /// As nove alturas deste canal.
    pub fn alturas(self, a: &Ajustes) -> [f32; PONTOS_DA_CURVA] {
        match self {
            Canal::Rgb => [
                a.curva_m0, a.curva_m1, a.curva_m2, a.curva_m3, a.curva_m4, a.curva_m5, a.curva_m6,
                a.curva_m7, a.curva_m8,
            ],
            Canal::Vermelho => [
                a.curva_r0, a.curva_r1, a.curva_r2, a.curva_r3, a.curva_r4, a.curva_r5, a.curva_r6,
                a.curva_r7, a.curva_r8,
            ],
            Canal::Verde => [
                a.curva_g0, a.curva_g1, a.curva_g2, a.curva_g3, a.curva_g4, a.curva_g5, a.curva_g6,
                a.curva_g7, a.curva_g8,
            ],
            Canal::Azul => [
                a.curva_b0, a.curva_b1, a.curva_b2, a.curva_b3, a.curva_b4, a.curva_b5, a.curva_b6,
                a.curva_b7, a.curva_b8,
            ],
        }
    }

    /// Escreve a altura de um ponto.
    pub fn definir(self, a: &mut Ajustes, ponto: usize, valor: f32) {
        let campo = match (self, ponto) {
            (Canal::Rgb, 0) => &mut a.curva_m0,
            (Canal::Rgb, 1) => &mut a.curva_m1,
            (Canal::Rgb, 2) => &mut a.curva_m2,
            (Canal::Rgb, 3) => &mut a.curva_m3,
            (Canal::Rgb, 4) => &mut a.curva_m4,
            (Canal::Rgb, 5) => &mut a.curva_m5,
            (Canal::Rgb, 6) => &mut a.curva_m6,
            (Canal::Rgb, 7) => &mut a.curva_m7,
            (Canal::Rgb, 8) => &mut a.curva_m8,
            (Canal::Vermelho, 0) => &mut a.curva_r0,
            (Canal::Vermelho, 1) => &mut a.curva_r1,
            (Canal::Vermelho, 2) => &mut a.curva_r2,
            (Canal::Vermelho, 3) => &mut a.curva_r3,
            (Canal::Vermelho, 4) => &mut a.curva_r4,
            (Canal::Vermelho, 5) => &mut a.curva_r5,
            (Canal::Vermelho, 6) => &mut a.curva_r6,
            (Canal::Vermelho, 7) => &mut a.curva_r7,
            (Canal::Vermelho, 8) => &mut a.curva_r8,
            (Canal::Verde, 0) => &mut a.curva_g0,
            (Canal::Verde, 1) => &mut a.curva_g1,
            (Canal::Verde, 2) => &mut a.curva_g2,
            (Canal::Verde, 3) => &mut a.curva_g3,
            (Canal::Verde, 4) => &mut a.curva_g4,
            (Canal::Verde, 5) => &mut a.curva_g5,
            (Canal::Verde, 6) => &mut a.curva_g6,
            (Canal::Verde, 7) => &mut a.curva_g7,
            (Canal::Verde, 8) => &mut a.curva_g8,
            (Canal::Azul, 0) => &mut a.curva_b0,
            (Canal::Azul, 1) => &mut a.curva_b1,
            (Canal::Azul, 2) => &mut a.curva_b2,
            (Canal::Azul, 3) => &mut a.curva_b3,
            (Canal::Azul, 4) => &mut a.curva_b4,
            (Canal::Azul, 5) => &mut a.curva_b5,
            (Canal::Azul, 6) => &mut a.curva_b6,
            (Canal::Azul, 7) => &mut a.curva_b7,
            (Canal::Azul, 8) => &mut a.curva_b8,
            _ => return,
        };
        *campo = valor;
    }
}

/// Esta curva devolve a foto como ela entrou? A tolerância do shader
/// (`curva_e_neutra`) e do site (`curvaEhNeutra`).
pub fn curva_eh_neutra(alturas: &[f32; PONTOS_DA_CURVA]) -> bool {
    alturas
        .iter()
        .zip(curva_neutra())
        .all(|(v, n)| (v - n).abs() < 0.01)
}

/// A altura da curva em `x` (0–255).
///
/// 🔑 **É a tradução linha a linha de `curva_por_ponto` do `corpo.wgsl`** —
/// Hermite monótono (Fritsch–Carlson), igual a `avaliarCurva` do site. Um
/// traço na tela diferente da conta do motor daria ao operador uma curva
/// olhando e outra no papel.
pub fn avaliar_curva(alturas: &[f32; PONTOS_DA_CURVA], x: f32) -> f32 {
    let ultimo = PONTOS_DA_CURVA - 1;
    let t0 = x.clamp(0.0, 255.0) / 255.0 * ultimo as f32;
    let i = (t0.floor() as usize).min(ultimo - 1);
    let t = t0 - i as f32;

    let y0 = alturas[i];
    let y1 = alturas[i + 1];
    let d = y1 - y0;
    let d_anterior = if i > 0 { y0 - alturas[i - 1] } else { d };
    let d_proximo = if i < ultimo - 1 {
        alturas[i + 2] - y1
    } else {
        d
    };

    let m0 = if d_anterior * d > 0.0 {
        2.0 * d_anterior * d / (d_anterior + d)
    } else {
        0.0
    };
    let m1 = if d * d_proximo > 0.0 {
        2.0 * d * d_proximo / (d + d_proximo)
    } else {
        0.0
    };

    let t2 = t * t;
    let t3 = t2 * t;
    let y = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
        + (t3 - 2.0 * t2 + t) * m0
        + (-2.0 * t3 + 3.0 * t2) * y1
        + (t3 - t2) * m1;
    y.clamp(0.0, 255.0)
}

#[cfg(test)]
mod curva_por_ponto {
    use super::*;

    #[test]
    fn o_neutro_e_a_identidade() {
        let neutra = curva_neutra();
        assert!(curva_eh_neutra(&neutra));
        for x in 0..=255 {
            let y = avaliar_curva(&neutra, x as f32);
            assert!((y - x as f32).abs() < 0.01, "{x} saiu {y}");
        }
        for canal in Canal::TODOS {
            assert_eq!(canal.alturas(&Ajustes::default()), neutra);
        }
    }

    /// 🚨 Monótono: a curva nunca sai do intervalo entre dois nós.
    #[test]
    fn nao_passa_por_fora_dos_nos() {
        let alturas = [0.0, 12.0, 40.0, 80.0, 81.0, 180.0, 215.0, 240.0, 255.0];
        for x in 0..=255 {
            let t0 = x as f32 / 255.0 * 8.0;
            let i = (t0.floor() as usize).min(7);
            let (a, b) = (alturas[i], alturas[i + 1]);
            let y = avaliar_curva(&alturas, x as f32);
            assert!(y >= a.min(b) - 1e-3 && y <= a.max(b) + 1e-3, "{x}: {y}");
        }
    }

    #[test]
    fn cada_canal_escreve_nos_seus_nove_campos() {
        for canal in Canal::TODOS {
            let mut a = Ajustes::default();
            for p in 0..PONTOS_DA_CURVA {
                canal.definir(&mut a, p, 100.0 + p as f32);
            }
            let esperado: [f32; 9] = std::array::from_fn(|p| 100.0 + p as f32);
            assert_eq!(canal.alturas(&a), esperado, "{}", canal.rotulo());
            for outro in Canal::TODOS.into_iter().filter(|c| *c != canal) {
                assert!(curva_eh_neutra(&outro.alturas(&a)));
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn perto(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// 🚨 **As quatro zonas movem o gráfico, cada uma na sua faixa.**
    ///
    /// Até 17/ago/2026 elas não moviam nada aqui: o desenho lia só o Básico. Com
    /// os sliders na tela, um gráfico parado logo acima deles seria pior que
    /// gráfico nenhum — ele afirmaria que o controle não faz efeito, bem no
    /// momento em que ele passou a fazer.
    #[test]
    fn cada_zona_da_curva_mexe_na_sua_faixa() {
        let neutra = curva(&Ajustes::default());

        /// Um caso: o nome, como aplicar, o ponto dentro da zona e um fora.
        struct Caso {
            nome: &'static str,
            aplicar: fn(&mut Ajustes),
            dentro: usize,
            fora: usize,
        }

        let casos = [
            Caso {
                nome: "sombras",
                aplicar: |a| a.tone_curve_shadows = 50.0,
                dentro: 12,
                fora: 87,
            },
            Caso {
                nome: "escuros",
                aplicar: |a| a.tone_curve_darks = 50.0,
                dentro: 37,
                fora: 87,
            },
            Caso {
                nome: "claros",
                aplicar: |a| a.tone_curve_lights = 50.0,
                dentro: 62,
                fora: 12,
            },
            Caso {
                nome: "altas luzes",
                aplicar: |a| a.tone_curve_highlights = 50.0,
                dentro: 87,
                fora: 12,
            },
        ];

        for Caso {
            nome,
            aplicar,
            dentro,
            fora,
        } in casos
        {
            let mut ajustes = Ajustes::default();
            aplicar(&mut ajustes);
            let curva_com = curva(&ajustes);

            assert!(
                curva_com[dentro] > neutra[dentro],
                "`{nome}` não levantou o ponto {dentro}, que é o centro da zona"
            );
            assert!(
                perto(curva_com[fora], neutra[fora]),
                "`{nome}` mexeu no ponto {fora}, que está fora da zona dela"
            );
        }
    }

    /// 🔑 **O gráfico usa a conta do shader, e não uma terceira aproximação.**
    ///
    /// Os mesmos centros, a mesma meia-largura e o mesmo peso linear. Este teste
    /// refaz a conta à mão para o dia em que alguém "simplificar" uma das duas
    /// pontas sem mexer na outra.
    #[test]
    fn a_zona_usa_a_conta_do_shader() {
        let ajustes = Ajustes {
            tone_curve_shadows: 40.0,
            ..Default::default()
        };
        let curva_com = curva(&ajustes);

        // O ponto 12 é entrada 0,12 — e no neutro o valor sai igual à entrada.
        let entrada = 12.0 / (PONTOS - 1) as f32;
        let distancia = (entrada - 0.125_f32).abs();
        let peso = 1.0 - (distancia / 0.25);
        let esperado = entrada + entrada * (40.0 * 0.01 * peso);

        assert!(
            perto(curva_com[12], esperado),
            "esperado {esperado}, veio {}",
            curva_com[12]
        );
    }

    /// ⚠️ **A zona é medida sobre o valor já ajustado, como no shader.**
    ///
    /// O shader calcula `lum_final` **depois** de exposição e contraste, e só
    /// então decide em qual zona o pixel cai. Medir pela entrada faria o gráfico
    /// discordar da foto quando os dois grupos de controle estão em uso ao mesmo
    /// tempo — que é o caso normal de quem revela.
    #[test]
    fn a_zona_segue_o_valor_ja_ajustado() {
        // Exposição alta empurra os tons escuros para dentro da zona dos claros.
        let ajustes = Ajustes {
            exposure: 4.0,
            tone_curve_lights: 60.0,
            ..Default::default()
        };
        let so_exposicao = curva(&Ajustes {
            exposure: 4.0,
            ..Default::default()
        });
        let com_zona = curva(&ajustes);

        // O ponto 20 entra em 0,20 e a exposição o leva a ~0,60 — dentro da
        // zona dos claros, cujo centro é 0,625.
        assert!(
            com_zona[20] > so_exposicao[20],
            "a zona dos claros tinha de alcançar um ponto que a exposição levou até lá"
        );
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
