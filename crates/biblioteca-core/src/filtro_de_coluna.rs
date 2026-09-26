//! O filtro **por coluna** da lista de sessões — o que um DataGrid põe numa
//! linha sob o cabeçalho. É o porte do `filtro-de-coluna.ts` do site.
//!
//! # Um operador por tipo, e só o que faz sentido
//!
//! 🔑 **Texto pergunta "contém"; número pergunta "quanto".** Um campo de texto
//! em "Levadas" faria quem digita `3` receber `3`, `13` e `30`.
//!
//! | tipo | o que o operador escolhe |
//! |---|---|
//! | texto | contém, sem acento e sem caixa |
//! | número, dinheiro | `≥`, `≤` ou `=` |
//! | data | de / até (inclusive nos dois lados) |
//! | escolha | uma situação; nenhuma = todas |
//!
//! # 🚨 O que falta não passa em filtro nenhum
//!
//! A sessão sem `totais` não rendeu zero — ela **não informa**. Num filtro
//! `balcão ≥ 0` ela apareceria como se tivesse rendido zero. Enquanto o filtro
//! da coluna estiver ligado, ela fica de fora.
//!
//! # 💵 Dinheiro se digita em reais
//!
//! O operador escreve `100` pensando em R$ 100,00, e a coluna guarda
//! centavos. Comparar o número digitado com os centavos — como o site fazia
//! até 24/set/2026 — punha em "Balcão ≥ 100" quem vendeu R$ 1,00.

use crate::sessoes::{dia_da_criacao, normalizar, SessaoFotografica, Situacao};

/// As colunas da lista que o desktop desenha, na ordem da tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Coluna {
    Galeria,
    Contato,
    Situacao,
    Levadas,
    AVenda,
    Compradas,
    Balcao,
    Caixa,
    PosVenda,
    Criada,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Texto,
    Numero,
    Dinheiro,
    Data,
    Escolha,
}

impl Coluna {
    pub const TODAS: [Coluna; 10] = [
        Coluna::Galeria,
        Coluna::Contato,
        Coluna::Situacao,
        Coluna::Levadas,
        Coluna::AVenda,
        Coluna::Compradas,
        Coluna::Balcao,
        Coluna::Caixa,
        Coluna::PosVenda,
        Coluna::Criada,
    ];

    pub fn tipo(self) -> Tipo {
        match self {
            Coluna::Galeria | Coluna::Contato => Tipo::Texto,
            Coluna::Situacao => Tipo::Escolha,
            Coluna::Levadas | Coluna::AVenda | Coluna::Compradas => Tipo::Numero,
            Coluna::Balcao | Coluna::Caixa | Coluna::PosVenda => Tipo::Dinheiro,
            Coluna::Criada => Tipo::Data,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Operador {
    #[default]
    MaiorIgual,
    MenorIgual,
    Igual,
}

impl Operador {
    pub const TODOS: [Operador; 3] = [Operador::MaiorIgual, Operador::MenorIgual, Operador::Igual];

    pub fn simbolo(self) -> &'static str {
        match self {
            Operador::MaiorIgual => "≥",
            Operador::MenorIgual => "≤",
            Operador::Igual => "=",
        }
    }

    fn compara(self, valor: f64, alvo: f64) -> bool {
        match self {
            Operador::MaiorIgual => valor >= alvo,
            Operador::MenorIgual => valor <= alvo,
            // Dinheiro chega arredondado em centavos: o `=` compara inteiros.
            Operador::Igual => (valor - alvo).abs() < 0.5,
        }
    }
}

/// Um filtro **ligado**. O vazio não existe aqui: quem monta a lista deixa de
/// fora a coluna cujo campo está em branco.
#[derive(Debug, Clone, PartialEq)]
pub enum FiltroDeColuna {
    Texto(String),
    /// Em reais nas colunas de dinheiro; em unidades nas outras.
    Numero {
        operador: Operador,
        valor: f64,
    },
    /// Dias `YYYY-MM-DD`; `None` é a ponta aberta.
    Data {
        de: Option<String>,
        ate: Option<String>,
    },
    Situacao(Situacao),
}

/// Um número como se digita no balcão: `19,90`, `1.234,50`, `R$ 100`.
pub fn ler_numero(texto: &str) -> Option<f64> {
    let limpo: String = texto
        .trim()
        .trim_start_matches("R$")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if limpo.is_empty() {
        return None;
    }
    // Com vírgula, ela é a dos centavos e os pontos são de milhar.
    let pronto = if limpo.contains(',') {
        limpo.replace('.', "").replace(',', ".")
    } else {
        limpo
    };
    pronto.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Um dia como se digita: `24/09/2026` ou `2026-09-24`. Vira `2026-09-24`.
pub fn ler_dia(texto: &str) -> Option<String> {
    let texto = texto.trim();
    let partes: Vec<&str> = texto.split(['/', '-']).collect();
    let [a, b, c] = partes.as_slice() else {
        return None;
    };
    let numero = |p: &str| p.parse::<u32>().ok();
    let (ano, mes, dia) = if texto.contains('/') {
        (numero(c)?, numero(b)?, numero(a)?)
    } else {
        (numero(a)?, numero(b)?, numero(c)?)
    };
    ((1000..=9999).contains(&ano) && (1..=12).contains(&mes) && (1..=31).contains(&dia))
        .then(|| format!("{ano:04}-{mes:02}-{dia:02}"))
}

/// O valor que a coluna compara. `None` = a sessão não informa.
enum Valor<'a> {
    Texto(String),
    Numero(f64),
    Dia(&'a str),
    Situacao(Situacao),
}

fn valor(coluna: Coluna, sessao: &SessaoFotografica, agora: i64) -> Option<Valor<'_>> {
    let centavos = |c: i64| Valor::Numero(c as f64);
    Some(match coluna {
        Coluna::Galeria => Valor::Texto(normalizar(&sessao.titulo)),
        Coluna::Contato => Valor::Texto(normalizar(
            sessao.email.as_deref().or(sessao.whatsapp.as_deref())?,
        )),
        Coluna::Situacao => Valor::Situacao(sessao.situacao(agora)),
        Coluna::Levadas => Valor::Numero(sessao.fotos.levadas_no_balcao.into()),
        Coluna::AVenda => Valor::Numero(sessao.fotos.disponiveis.into()),
        Coluna::Compradas => Valor::Numero(sessao.fotos.compradas.into()),
        Coluna::Balcao => centavos(sessao.totais?.balcao),
        Coluna::PosVenda => centavos(sessao.totais?.pos_venda),
        Coluna::Caixa => centavos(sessao.caixa.as_ref()?.liquido_centavos),
        Coluna::Criada => Valor::Dia(dia_da_criacao(&sessao.criada_em_iso)),
    })
}

/// A sessão passa por este filtro desta coluna?
pub fn passa(
    coluna: Coluna,
    filtro: &FiltroDeColuna,
    sessao: &SessaoFotografica,
    agora: i64,
) -> bool {
    // 🚨 Ver a nota do topo: o que não informa não passa em filtro de coluna.
    let Some(valor) = valor(coluna, sessao, agora) else {
        return false;
    };
    match (filtro, valor) {
        (FiltroDeColuna::Texto(procura), Valor::Texto(texto)) => {
            texto.contains(&normalizar(procura))
        }
        (
            FiltroDeColuna::Numero {
                operador,
                valor: alvo,
            },
            Valor::Numero(n),
        ) => {
            let alvo = if coluna.tipo() == Tipo::Dinheiro {
                (alvo * 100.0).round()
            } else {
                *alvo
            };
            operador.compara(n, alvo)
        }
        (FiltroDeColuna::Data { de, ate }, Valor::Dia(dia)) => {
            // `YYYY-MM-DD` comparado como texto é comparado como data.
            de.as_deref().is_none_or(|de| dia >= de) && ate.as_deref().is_none_or(|ate| dia <= ate)
        }
        (FiltroDeColuna::Situacao(querida), Valor::Situacao(s)) => *querida == s,
        // Filtro de um tipo numa coluna de outro: quem montou errou, e a
        // coluna não filtra — esconder a lista inteira seria pior.
        _ => true,
    }
}

/// Aplica os filtros de coluna. **Todos** precisam passar (`E`, não `OU`):
/// "à venda ≥ 10" e "criada em setembro" é procurar o que satisfaz os dois.
pub fn filtrar_por_coluna<'a>(
    sessoes: Vec<&'a SessaoFotografica>,
    filtros: &[(Coluna, FiltroDeColuna)],
    agora: i64,
) -> Vec<&'a SessaoFotografica> {
    if filtros.is_empty() {
        return sessoes;
    }
    sessoes
        .into_iter()
        .filter(|s| filtros.iter().all(|(c, f)| passa(*c, f, s, agora)))
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::sessoes::{ContagemDeFotos, PagoNoCaixa, Totais};

    fn sessao(titulo: &str) -> SessaoFotografica {
        SessaoFotografica {
            id: titulo.into(),
            titulo: titulo.into(),
            email: None,
            whatsapp: None,
            criada_em_iso: "2026-09-10T12:00:00Z".into(),
            expira_em: None,
            user_id: None,
            fotos: ContagemDeFotos {
                levadas_no_balcao: 3,
                disponiveis: 13,
                compradas: 0,
                apagadas: 0,
            },
            totais: None,
            caixa: None,
            sem_estudio: false,
        }
    }

    fn titulos(v: &[&SessaoFotografica]) -> Vec<String> {
        v.iter().map(|s| s.titulo.clone()).collect()
    }

    #[test]
    fn texto_acha_sem_acento_e_sem_caixa() {
        let lista = [sessao("Ávila e João"), sessao("Maria")];
        let achadas = filtrar_por_coluna(
            lista.iter().collect(),
            &[(Coluna::Galeria, FiltroDeColuna::Texto("joao".into()))],
            0,
        );
        assert_eq!(titulos(&achadas), ["Ávila e João"]);
    }

    #[test]
    fn numero_pergunta_quanto_e_nao_contem() {
        let lista = [sessao("três")];
        let igual = |valor| {
            filtrar_por_coluna(
                lista.iter().collect(),
                &[(
                    Coluna::Levadas,
                    FiltroDeColuna::Numero {
                        operador: Operador::Igual,
                        valor,
                    },
                )],
                0,
            )
            .len()
        };
        assert_eq!(igual(3.0), 1);
        assert_eq!(igual(13.0), 0, "3 não é 13, mesmo contido nele");
    }

    #[test]
    fn dinheiro_se_digita_em_reais() {
        let mut vendeu = sessao("vendeu");
        vendeu.totais = Some(Totais {
            balcao: 42_500,
            pos_venda: 0,
        });
        let mut pouco = sessao("pouco");
        pouco.totais = Some(Totais {
            balcao: 150,
            pos_venda: 0,
        });
        let lista = [vendeu, pouco];
        let achadas = filtrar_por_coluna(
            lista.iter().collect(),
            &[(
                Coluna::Balcao,
                FiltroDeColuna::Numero {
                    operador: Operador::MaiorIgual,
                    valor: 100.0,
                },
            )],
            0,
        );
        assert_eq!(titulos(&achadas), ["vendeu"], "R$ 1,50 não é ≥ R$ 100");
    }

    #[test]
    fn quem_nao_informa_nao_passa() {
        let sem_totais = sessao("sem totais");
        let mut com_caixa = sessao("com caixa");
        com_caixa.caixa = Some(PagoNoCaixa {
            vendas: 0,
            bruto_centavos: 0,
            estornado_centavos: 0,
            liquido_centavos: 0,
            ..PagoNoCaixa::default()
        });
        let lista = [sem_totais, com_caixa];
        let zero_ou_mais = |coluna| {
            titulos(&filtrar_por_coluna(
                lista.iter().collect(),
                &[(
                    coluna,
                    FiltroDeColuna::Numero {
                        operador: Operador::MaiorIgual,
                        valor: 0.0,
                    },
                )],
                0,
            ))
        };
        assert!(zero_ou_mais(Coluna::Balcao).is_empty());
        assert_eq!(zero_ou_mais(Coluna::Caixa), ["com caixa"]);
    }

    #[test]
    fn data_e_inclusiva_nas_duas_pontas_e_os_filtros_somam() {
        let mut outra = sessao("outra");
        outra.criada_em_iso = "2026-09-20T09:00:00Z".into();
        let lista = [sessao("dia 10"), outra];
        let achadas = filtrar_por_coluna(
            lista.iter().collect(),
            &[
                (
                    Coluna::Criada,
                    FiltroDeColuna::Data {
                        de: Some("2026-09-10".into()),
                        ate: None,
                    },
                ),
                (Coluna::Galeria, FiltroDeColuna::Texto("dia".into())),
            ],
            0,
        );
        assert_eq!(titulos(&achadas), ["dia 10"]);
    }

    #[test]
    fn situacao_escolhe_uma() {
        let mut sem_fotos = sessao("vazia");
        sem_fotos.fotos = ContagemDeFotos::default();
        let lista = [sessao("com fotos"), sem_fotos];
        let achadas = filtrar_por_coluna(
            lista.iter().collect(),
            &[(
                Coluna::Situacao,
                FiltroDeColuna::Situacao(Situacao::SemFotos),
            )],
            0,
        );
        assert_eq!(titulos(&achadas), ["vazia"]);
    }

    #[test]
    fn le_numero_e_dia_como_se_digita() {
        assert_eq!(ler_numero("19,90"), Some(19.9));
        assert_eq!(ler_numero("R$ 1.234,50"), Some(1234.5));
        assert_eq!(ler_numero("100"), Some(100.0));
        assert_eq!(ler_numero(""), None);
        assert_eq!(ler_numero("abc"), None);
        assert_eq!(ler_dia("24/09/2026").as_deref(), Some("2026-09-24"));
        assert_eq!(ler_dia("2026-9-4").as_deref(), Some("2026-09-04"));
        assert_eq!(ler_dia("24/09"), None);
        assert_eq!(ler_dia("32/01/2026"), None);
    }
}
