//! A negociação do balcão, em regras puras — o `negociacao.ts` do site, portado.
//!
//! No banco são dois campos por foto: `preco_negociado` (centavos, `None` =
//! vale o preço da faixa) e `observacao_da_negociacao` (texto livre). A tela
//! apresenta isso como **um tipo** e o que cada tipo pede: cortesia não pede
//! preço (é zero), desconto pede o quanto entrou, site parceiro pede qual e o
//! cupom. O texto gravado segue um formato fixo (`"Cortesia — motivo"`,
//! `"TchêOfertas — cupom 123"`) para ser lido de volta ao editar e para virar
//! etiqueta curta no tile. Texto de antes do formato cai em "outro" e é
//! preservado como está.
//!
//! 🔒 **Nada aqui muda o preço de venda.** É registro do que já aconteceu; a
//! compra online continua cobrando a faixa (ou o preço fixado — ver
//! [`crate::preco_de_venda`]).

use crate::dinheiro;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Cortesia,
    Desconto,
    Parceiro,
    Outro,
}

impl Tipo {
    pub const TODOS: [Tipo; 4] = [Tipo::Cortesia, Tipo::Desconto, Tipo::Parceiro, Tipo::Outro];

    pub fn rotulo(self) -> &'static str {
        match self {
            Tipo::Cortesia => "Cortesia",
            Tipo::Desconto => "Desconto",
            Tipo::Parceiro => "Já paga em outro site",
            Tipo::Outro => "Outro",
        }
    }

    pub fn dica(self) -> &'static str {
        match self {
            Tipo::Cortesia => "Não cobrou nada por esta foto.",
            Tipo::Desconto => "Cobrou menos que o preço da faixa.",
            Tipo::Parceiro => "O cliente comprou por um site parceiro.",
            Tipo::Outro => "Qualquer outro acerto, em texto livre.",
        }
    }
}

/// Os sites em que o cliente compra antes e chega com o cupom.
pub const PARCEIROS: [&str; 3] = ["TchêOfertas", "LançadorDeOfertas", "ParksNet"];

const SEPARADOR: &str = " — ";

/// O que a tela edita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Negociacao {
    pub tipo: Tipo,
    /// Centavos. Cortesia é sempre 0; parceiro e outro podem ficar sem.
    pub preco: Option<i64>,
    pub parceiro: String,
    pub cupom: String,
    pub motivo: String,
}

impl Default for Negociacao {
    fn default() -> Self {
        Self {
            tipo: Tipo::Cortesia,
            preco: None,
            parceiro: PARCEIROS[0].to_string(),
            cupom: String::new(),
            motivo: String::new(),
        }
    }
}

/// O que vai para o banco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gravavel {
    pub preco_negociado: Option<i64>,
    pub observacao: Option<String>,
}

fn juntar<'a>(partes: impl IntoIterator<Item = Option<&'a str>>) -> String {
    partes
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(SEPARADOR)
}

/// Monta os dois campos a partir do que a tela pediu. Devolve erro em texto
/// quando falta o que o tipo exige — a tela mostra sem ir ao servidor.
pub fn montar(n: &Negociacao) -> Result<Gravavel, String> {
    match n.tipo {
        Tipo::Cortesia => {
            let obs = juntar([Some("Cortesia"), Some(&n.motivo)]);
            Ok(Gravavel {
                preco_negociado: Some(0),
                observacao: Some(if obs.is_empty() {
                    "Cortesia".into()
                } else {
                    obs
                }),
            })
        }
        Tipo::Desconto => match n.preco {
            None => Err("Informe quanto foi cobrado.".into()),
            Some(0) => Err("Cobrou zero? Então é cortesia.".into()),
            Some(preco) => {
                let obs = juntar([Some("Desconto"), Some(&n.motivo)]);
                Ok(Gravavel {
                    preco_negociado: Some(preco),
                    observacao: Some(if obs.is_empty() {
                        "Desconto".into()
                    } else {
                        obs
                    }),
                })
            }
        },
        Tipo::Parceiro => {
            let parceiro = n.parceiro.trim();
            if parceiro.is_empty() {
                return Err("Diga em qual site a foto foi paga.".into());
            }
            let cupom = if n.cupom.trim().is_empty() {
                String::new()
            } else {
                format!("cupom {}", n.cupom.trim())
            };
            let obs = juntar([Some(parceiro), Some(cupom.as_str()), Some(&n.motivo)]);
            Ok(Gravavel {
                preco_negociado: n.preco,
                observacao: Some(obs),
            })
        }
        Tipo::Outro => {
            let motivo = n.motivo.trim();
            if motivo.is_empty() && n.preco.is_none() {
                return Err("Escreva o que foi combinado, ou o valor cobrado.".into());
            }
            Ok(Gravavel {
                preco_negociado: n.preco,
                observacao: (!motivo.is_empty()).then(|| motivo.to_string()),
            })
        }
    }
}

/// Lê de volta o que está gravado, para a tela editar. Reconhece o formato que
/// [`montar`] escreve; o resto vira "outro" com o texto intacto.
pub fn interpretar(preco: Option<i64>, observacao: Option<&str>) -> Negociacao {
    let texto = observacao.unwrap_or("").trim();
    let mut partes = texto.split(SEPARADOR);
    let cabeca = partes.next().unwrap_or("");
    let resto: Vec<&str> = partes.collect();
    let cauda = resto.join(SEPARADOR);

    if preco == Some(0) || cabeca == "Cortesia" {
        return Negociacao {
            tipo: Tipo::Cortesia,
            preco: Some(0),
            motivo: if cabeca == "Cortesia" {
                cauda
            } else {
                texto.to_string()
            },
            ..Default::default()
        };
    }
    if cabeca == "Desconto" {
        return Negociacao {
            tipo: Tipo::Desconto,
            preco,
            motivo: cauda,
            ..Default::default()
        };
    }
    if let Some(parceiro) = PARCEIROS.iter().find(|p| **p == cabeca) {
        let partes: Vec<&str> = resto.iter().map(|r| r.trim()).collect();
        let indice_do_cupom = partes
            .iter()
            .position(|p| p.to_lowercase().starts_with("cupom "));
        let cupom = indice_do_cupom
            .map(|i| partes[i]["cupom ".len()..].to_string())
            .unwrap_or_default();
        let motivo = partes
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != indice_do_cupom)
            .map(|(_, p)| *p)
            .collect::<Vec<_>>()
            .join(SEPARADOR);
        return Negociacao {
            tipo: Tipo::Parceiro,
            preco,
            parceiro: (*parceiro).to_string(),
            cupom,
            motivo,
        };
    }
    Negociacao {
        tipo: Tipo::Outro,
        preco,
        motivo: texto.to_string(),
        ..Default::default()
    }
}

/// A etiqueta curta do tile: o que aconteceu, e quanto entrou quando isso
/// importa. `"Cortesia"`, `"Desconto · R$ 10,00"`, `"TchêOfertas · cupom 123"`.
pub struct Etiqueta {
    pub titulo: String,
    pub detalhe: Option<String>,
}

pub fn descrever(preco: Option<i64>, observacao: Option<&str>) -> Etiqueta {
    let n = interpretar(preco, observacao);
    let detalhe_do_motivo = || (!n.motivo.is_empty()).then(|| n.motivo.clone());
    match n.tipo {
        Tipo::Cortesia => Etiqueta {
            titulo: "Cortesia".into(),
            detalhe: detalhe_do_motivo(),
        },
        Tipo::Desconto => Etiqueta {
            titulo: format!(
                "Desconto · {}",
                n.preco.map_or("?".to_string(), dinheiro::formatar)
            ),
            detalhe: detalhe_do_motivo(),
        },
        Tipo::Parceiro => {
            let pagou = n
                .preco
                .map(|p| format!("pagou {} lá", dinheiro::formatar(p)));
            let detalhe = juntar([pagou.as_deref(), Some(&n.motivo)]);
            Etiqueta {
                titulo: if n.cupom.is_empty() {
                    n.parceiro.clone()
                } else {
                    format!("{} · cupom {}", n.parceiro, n.cupom)
                },
                detalhe: (!detalhe.is_empty()).then_some(detalhe),
            }
        }
        Tipo::Outro => Etiqueta {
            titulo: n.preco.map_or("Acerto".to_string(), |p| {
                format!("Cobrado {}", dinheiro::formatar(p))
            }),
            detalhe: detalhe_do_motivo(),
        },
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cortesia_grava_zero_e_o_motivo() {
        let g = montar(&Negociacao {
            tipo: Tipo::Cortesia,
            motivo: "aniversário".into(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(g.preco_negociado, Some(0));
        assert_eq!(g.observacao.as_deref(), Some("Cortesia — aniversário"));

        let sem_motivo = montar(&Negociacao::default()).unwrap();
        assert_eq!(sem_motivo.observacao.as_deref(), Some("Cortesia"));
    }

    #[test]
    fn desconto_exige_valor_e_zero_e_cortesia() {
        let sem = montar(&Negociacao {
            tipo: Tipo::Desconto,
            ..Default::default()
        });
        assert_eq!(sem.unwrap_err(), "Informe quanto foi cobrado.");

        let zero = montar(&Negociacao {
            tipo: Tipo::Desconto,
            preco: Some(0),
            ..Default::default()
        });
        assert_eq!(zero.unwrap_err(), "Cobrou zero? Então é cortesia.");

        let ok = montar(&Negociacao {
            tipo: Tipo::Desconto,
            preco: Some(1000),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(ok.preco_negociado, Some(1000));
        assert_eq!(ok.observacao.as_deref(), Some("Desconto"));
    }

    #[test]
    fn parceiro_grava_site_e_cupom_no_formato_que_se_le_de_volta() {
        let g = montar(&Negociacao {
            tipo: Tipo::Parceiro,
            preco: Some(1500),
            parceiro: "TchêOfertas".into(),
            cupom: "123".into(),
            motivo: "família".into(),
        })
        .unwrap();
        assert_eq!(
            g.observacao.as_deref(),
            Some("TchêOfertas — cupom 123 — família")
        );

        let lida = interpretar(g.preco_negociado, g.observacao.as_deref());
        assert_eq!(lida.tipo, Tipo::Parceiro);
        assert_eq!(lida.parceiro, "TchêOfertas");
        assert_eq!(lida.cupom, "123");
        assert_eq!(lida.motivo, "família");
        assert_eq!(lida.preco, Some(1500));
    }

    #[test]
    fn outro_exige_texto_ou_valor() {
        assert!(montar(&Negociacao {
            tipo: Tipo::Outro,
            ..Default::default()
        })
        .is_err());
        let g = montar(&Negociacao {
            tipo: Tipo::Outro,
            preco: Some(500),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(g.observacao, None);
    }

    /// Texto de antes do formato — ou digitado noutra tela — é "outro", intacto.
    #[test]
    fn texto_fora_do_formato_vira_outro_sem_perder_nada() {
        let n = interpretar(Some(2000), Some("acertado com a Maria"));
        assert_eq!(n.tipo, Tipo::Outro);
        assert_eq!(n.motivo, "acertado com a Maria");
        assert_eq!(n.preco, Some(2000));
    }

    #[test]
    fn zero_gravado_e_cortesia_mesmo_sem_texto() {
        let n = interpretar(Some(0), None);
        assert_eq!(n.tipo, Tipo::Cortesia);
        assert_eq!(n.preco, Some(0));
    }

    #[test]
    fn a_etiqueta_diz_o_que_aconteceu_e_quanto_entrou() {
        assert_eq!(
            descrever(Some(0), Some("Cortesia — festa")).titulo,
            "Cortesia"
        );
        assert_eq!(
            descrever(Some(1000), Some("Desconto")).titulo,
            "Desconto · R$ 10,00"
        );
        let parceiro = descrever(Some(1500), Some("ParksNet — cupom X9"));
        assert_eq!(parceiro.titulo, "ParksNet · cupom X9");
        assert_eq!(parceiro.detalhe.as_deref(), Some("pagou R$ 15,00 lá"));
        assert_eq!(descrever(None, Some("qualquer coisa")).titulo, "Acerto");
        assert_eq!(descrever(Some(700), Some("x")).titulo, "Cobrado R$ 7,00");
    }
}
