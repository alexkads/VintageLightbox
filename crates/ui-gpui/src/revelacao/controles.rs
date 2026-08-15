//! Quais sliders existem, e o que cada um move.
//!
//! Uma tabela, e não 11 blocos de interface iguais. São ~46 ajustes no total, e
//! escrever cada um à mão é como o `crates/ui` ficou com 290 linhas para *um*
//! slider — a tabela é o que faz o painel de HSL (24 controles) caber numa
//! linha por canal quando chegar a vez dele.

use super::processador::Ajustes;

/// Um controle: o rótulo, a faixa e por onde ele escreve.
pub struct Definicao {
    pub rotulo: &'static str,
    pub minimo: f32,
    pub maximo: f32,
    /// Quantas casas mostrar ao lado do rótulo — é o que o app de egui faz com
    /// `{:+.2}` na exposição e `{:.2}` no contraste.
    pub casas: usize,
    /// Se o valor merece sinal explícito. Exposição `+0,30` diz "clareou";
    /// contraste `1,30` não é "mais 1,30", é um multiplicador.
    pub com_sinal: bool,
    pub aplicar: fn(&mut Ajustes, f32),
    pub ler: fn(&Ajustes) -> f32,
}

impl Definicao {
    /// Onde o slider nasce.
    ///
    /// 🔑 Vem de [`Ajustes::default`], e **não** de um número escrito aqui. Os
    /// neutros não são todos zero (contraste é 1.0), e ter dois lugares
    /// dizendo qual é o neutro é ter um deles errado mais cedo ou mais tarde —
    /// com o sintoma de a foto abrir alterada com o slider no meio.
    pub fn neutro(&self) -> f32 {
        (self.ler)(&Ajustes::default())
    }

    pub fn formatar(&self, valor: f32) -> String {
        if self.com_sinal {
            format!("{:+.*}", self.casas, valor)
        } else {
            format!("{:.*}", self.casas, valor)
        }
    }
}

/// O painel Básico.
///
/// ⚠️ **As faixas são as do `crates/ui`**, lidas de
/// `docking/dock_viewer.rs` uma a uma. Não são arredondamentos bonitos: o
/// contraste vai de 0 a 2 porque é multiplicador, a temperatura de -10 a 10
/// porque é a escala do shader, e os quatro tonais de -100 a 100. Mudar
/// qualquer uma faria o mesmo arrasto dar resultado diferente nos dois apps —
/// que é exatamente o que a paridade da fase 2 mede.
pub const BASICOS: &[Definicao] = &[
    Definicao {
        rotulo: "Exposição",
        minimo: -5.0,
        maximo: 5.0,
        casas: 2,
        com_sinal: true,
        aplicar: |a, v| a.exposure = v,
        ler: |a| a.exposure,
    },
    Definicao {
        rotulo: "Contraste",
        minimo: 0.0,
        maximo: 2.0,
        casas: 2,
        com_sinal: false,
        aplicar: |a, v| a.contrast = v,
        ler: |a| a.contrast,
    },
    Definicao {
        rotulo: "Temperatura",
        minimo: -10.0,
        maximo: 10.0,
        casas: 1,
        com_sinal: true,
        aplicar: |a, v| a.temperature = v,
        ler: |a| a.temperature,
    },
    Definicao {
        rotulo: "Matiz",
        minimo: -10.0,
        maximo: 10.0,
        casas: 1,
        com_sinal: true,
        aplicar: |a, v| a.tint = v,
        ler: |a| a.tint,
    },
    Definicao {
        rotulo: "Altas luzes",
        minimo: -100.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: true,
        aplicar: |a, v| a.highlights = v,
        ler: |a| a.highlights,
    },
    Definicao {
        rotulo: "Sombras",
        minimo: -100.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: true,
        aplicar: |a, v| a.shadows = v,
        ler: |a| a.shadows,
    },
    Definicao {
        rotulo: "Brancos",
        minimo: -100.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: true,
        aplicar: |a, v| a.whites = v,
        ler: |a| a.whites,
    },
    Definicao {
        rotulo: "Pretos",
        minimo: -100.0,
        maximo: 100.0,
        casas: 0,
        com_sinal: true,
        aplicar: |a, v| a.blacks = v,
        ler: |a| a.blacks,
    },
    Definicao {
        rotulo: "Textura",
        minimo: -1.0,
        maximo: 1.0,
        casas: 2,
        com_sinal: true,
        aplicar: |a, v| a.clarity = v,
        ler: |a| a.clarity,
    },
    Definicao {
        rotulo: "Intensidade",
        minimo: -1.0,
        maximo: 1.0,
        casas: 2,
        com_sinal: true,
        aplicar: |a, v| a.vibrance = v,
        ler: |a| a.vibrance,
    },
    Definicao {
        rotulo: "Saturação",
        minimo: -1.0,
        maximo: 1.0,
        casas: 2,
        com_sinal: true,
        aplicar: |a, v| a.saturation = v,
        ler: |a| a.saturation,
    },
];

#[cfg(test)]
mod testes {
    use super::*;

    /// 🚨 Todo controle nasce no neutro **e** dentro da própria faixa.
    ///
    /// O contraste é o caso que pega: neutro `1.0` numa faixa de `0..2`. Se
    /// alguém trocar a faixa para `-1..1` copiando a dos vizinhos, o slider
    /// nasceria grudado no fim da barra e a foto abriria com contraste no
    /// máximo — sem erro, e parecendo escolha de quem desenhou a tela.
    #[test]
    fn todo_neutro_cabe_na_faixa() {
        for def in BASICOS {
            let neutro = def.neutro();
            assert!(
                neutro >= def.minimo && neutro <= def.maximo,
                "`{}` nasce em {neutro}, fora de {}..{}",
                def.rotulo,
                def.minimo,
                def.maximo
            );
        }
    }

    /// A faixa é uma faixa: mínimo abaixo do máximo.
    #[test]
    fn nenhuma_faixa_esta_invertida() {
        for def in BASICOS {
            assert!(
                def.minimo < def.maximo,
                "`{}` tem faixa invertida",
                def.rotulo
            );
        }
    }

    /// 🔑 Cada controle escreve num campo **diferente**.
    ///
    /// Dois `aplicar` apontando para o mesmo campo é o erro de copiar-e-colar
    /// desta tabela, e ele não falha: um slider simplesmente não faz nada e o
    /// outro passa a responder por dois. Com 46 ajustes pela frente, a chance
    /// de acontecer é alta e a de perceber olhando é baixa.
    #[test]
    fn cada_controle_move_um_campo_proprio() {
        for (i, def) in BASICOS.iter().enumerate() {
            let mut ajustes = Ajustes::default();
            // Um valor que certamente difere do neutro, dentro da faixa.
            let marca = def.minimo + (def.maximo - def.minimo) * 0.25;
            (def.aplicar)(&mut ajustes, marca);

            for (j, outro) in BASICOS.iter().enumerate() {
                if i == j {
                    assert_eq!(
                        (def.ler)(&ajustes),
                        marca,
                        "`{}` não lê de volta o que escreveu",
                        def.rotulo
                    );
                } else {
                    assert_eq!(
                        (outro.ler)(&ajustes),
                        outro.neutro(),
                        "mexer em `{}` mexeu em `{}` — os dois escrevem no mesmo campo",
                        def.rotulo,
                        outro.rotulo
                    );
                }
            }
        }
    }
}
