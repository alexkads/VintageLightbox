//! A lista de sessões fotográficas — situação, busca, contagens e o gráfico.
//!
//! # Por que isto é conta, e não tela
//!
//! A lista do painel (`/dashboard/sessoes-fotograficas`) e a do desktop
//! respondem à mesma pergunta com o mesmo operador na frente: *"cadê a galeria
//! da Maria?"*. Escritas duas vezes, elas divergem — e a divergência aqui não
//! aparece como erro, aparece como **a busca achando num app e não no outro**.
//!
//! Estas regras nasceram em TypeScript (`apresentacao.ts`, com 215 linhas de
//! teste ao lado) e foram portadas para cá em 6/set/2026, reescritas e não
//! copiadas, para que os dois lados passem a ler daqui.
//!
//! # A ordem da situação não é arbitrária
//!
//! É a ordem em que ela importa para quem opera: **vencida vence tudo** (não
//! adianta subir foto numa galeria que expirou), depois "sem fotos" (o próximo
//! passo é subir), e só então se o cliente já entrou ou ainda não.
//!
//! # Sem dependência nenhuma, inclusive de data
//!
//! Este crate compila para `wasm32` porque não puxa nada — e data seria a
//! primeira tentação. `expira_em` chega como **segundos desde a época** e a
//! comparação é aritmética; o agrupamento do gráfico lê a data ISO como texto e
//! conta os dias com o algoritmo civil, que cabe em dez linhas.

use std::collections::BTreeMap;

/// O que a lista diz de cada sessão, num olhar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Situacao {
    SemFotos,
    AguardandoCliente,
    AbertaPeloCliente,
    Vencida,
}

impl Situacao {
    /// Na ordem em que a barra de filtros as mostra.
    pub const TODAS: [Situacao; 4] = [
        Situacao::SemFotos,
        Situacao::AguardandoCliente,
        Situacao::AbertaPeloCliente,
        Situacao::Vencida,
    ];

    pub fn rotulo(self) -> &'static str {
        match self {
            Situacao::SemFotos => "Sem fotos",
            Situacao::AguardandoCliente => "Aguardando o cliente",
            Situacao::AbertaPeloCliente => "Cliente já abriu",
            Situacao::Vencida => "Vencida",
        }
    }

    /// O nome que atravessa a fronteira (JSON do site, wasm, teste).
    pub fn como_texto(self) -> &'static str {
        match self {
            Situacao::SemFotos => "sem_fotos",
            Situacao::AguardandoCliente => "aguardando_cliente",
            Situacao::AbertaPeloCliente => "aberta_pelo_cliente",
            Situacao::Vencida => "vencida",
        }
    }
}

/// Quantas fotos a sessão tem, por estado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContagemDeFotos {
    pub levadas_no_balcao: u32,
    pub disponiveis: u32,
    pub compradas: u32,
    pub apagadas: u32,
}

impl ContagemDeFotos {
    /// As que ainda existem. ⚠️ A apagada **não** conta: o arquivo sumiu pela
    /// retenção, e uma galeria só com apagadas não tem o que mostrar ao cliente.
    pub fn vivas(&self) -> u32 {
        self.levadas_no_balcao + self.disponiveis + self.compradas
    }
}

/// Quanto a sessão rendeu, por porta, em centavos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totais {
    pub balcao: i64,
    pub pos_venda: i64,
}

/// Uma sessão fotográfica como a lista precisa vê-la.
///
/// 🔑 O nome é `SessaoFotografica` e não `Sessao` de propósito: no desktop já
/// existe uma `Sessao` que é o **login** do operador, e as duas juntas num
/// mesmo `use` seriam duas coisas sem relação com o mesmo nome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessaoFotografica {
    pub id: String,
    pub titulo: String,
    pub email: Option<String>,
    pub whatsapp: Option<String>,
    /// `"2026-09-03"`, no fuso do estúdio — o carimbo do eixo do gráfico.
    pub criada_em_iso: String,
    /// Segundos desde a época. `None` = não expira.
    pub expira_em: Option<i64>,
    /// Preenchido quando o cliente já criou conta pelo link.
    pub user_id: Option<String>,
    pub fotos: ContagemDeFotos,
    /// `None` na sessão vinda de uma API anterior ao campo — e isso é dito na
    /// soma, em vez de virar zero calado.
    pub totais: Option<Totais>,
}

impl SessaoFotografica {
    /// A situação, na ordem em que ela importa para quem opera.
    pub fn situacao(&self, agora: i64) -> Situacao {
        if self.expira_em.is_some_and(|quando| quando < agora) {
            return Situacao::Vencida;
        }
        if self.fotos.vivas() == 0 {
            return Situacao::SemFotos;
        }
        if self.user_id.is_some() {
            return Situacao::AbertaPeloCliente;
        }
        Situacao::AguardandoCliente
    }
}

/// O que a busca procura.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Criterio {
    pub busca: String,
    /// `None` é "todas".
    pub situacao: Option<Situacao>,
}

/// Minúsculas e sem acento: "Joao" acha "João" e vice-versa.
///
/// ⚠️ **A tabela é do português, e é de propósito.** Normalizar Unicode inteiro
/// exigiria uma tabela de decomposição — dezenas de KB no `.wasm` que o
/// navegador baixa — para acertar alfabetos que este balcão não atende. O que
/// não estiver aqui passa direto, em minúsculas.
pub fn normalizar(texto: &str) -> String {
    texto
        .trim()
        .chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ý' | 'ÿ' => 'y',
            outro => outro,
        })
        .collect()
}

fn so_digitos(texto: &str) -> String {
    texto.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Quantos dígitos a busca precisa ter para valer como telefone.
///
/// Menos que isto casaria com quase todo número da lista, e o operador que
/// digita "12" procurando um título veria as galerias erradas aparecerem.
const DIGITOS_PARA_VALER_TELEFONE: usize = 3;

/// Busca por título, e-mail ou WhatsApp, e filtro por situação.
///
/// O WhatsApp compara **só dígitos**: quem digita `99999` acha
/// `(47) 99999-8888` — a máscara é apresentação, não dado.
pub fn filtrar<'a>(
    sessoes: &'a [SessaoFotografica],
    criterio: &Criterio,
    agora: i64,
) -> Vec<&'a SessaoFotografica> {
    let termo = normalizar(&criterio.busca);
    let digitos = so_digitos(&criterio.busca);

    sessoes
        .iter()
        .filter(|sessao| {
            if criterio
                .situacao
                .is_some_and(|querida| sessao.situacao(agora) != querida)
            {
                return false;
            }
            if termo.is_empty() {
                return true;
            }
            if normalizar(&sessao.titulo).contains(&termo) {
                return true;
            }
            if sessao
                .email
                .as_deref()
                .is_some_and(|email| normalizar(email).contains(&termo))
            {
                return true;
            }
            digitos.len() >= DIGITOS_PARA_VALER_TELEFONE
                && sessao
                    .whatsapp
                    .as_deref()
                    .is_some_and(|zap| so_digitos(zap).contains(&digitos))
        })
        .collect()
}

/// Quantas sessões há em cada situação, para os contadores dos filtros.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Contagens {
    pub todas: usize,
    pub sem_fotos: usize,
    pub aguardando_cliente: usize,
    pub aberta_pelo_cliente: usize,
    pub vencida: usize,
}

impl Contagens {
    pub fn de(&self, situacao: Situacao) -> usize {
        match situacao {
            Situacao::SemFotos => self.sem_fotos,
            Situacao::AguardandoCliente => self.aguardando_cliente,
            Situacao::AbertaPeloCliente => self.aberta_pelo_cliente,
            Situacao::Vencida => self.vencida,
        }
    }
}

pub fn contar_por_situacao(sessoes: &[SessaoFotografica], agora: i64) -> Contagens {
    let mut contagens = Contagens {
        todas: sessoes.len(),
        ..Contagens::default()
    };
    for sessao in sessoes {
        match sessao.situacao(agora) {
            Situacao::SemFotos => contagens.sem_fotos += 1,
            Situacao::AguardandoCliente => contagens.aguardando_cliente += 1,
            Situacao::AbertaPeloCliente => contagens.aberta_pelo_cliente += 1,
            Situacao::Vencida => contagens.vencida += 1,
        }
    }
    contagens
}

/// A soma do **recorte visível**, e o que ela não pôde somar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Soma {
    pub balcao: i64,
    pub pos_venda: i64,
    /// Quantas sessões entraram sem totais — de uma API anterior ao campo.
    ///
    /// 🔑 **Contadas, e não silenciadas.** Sem isto o rodapé anunciaria um total
    /// menor que o real sem nada dizendo por quê.
    pub sem_totais: usize,
}

/// ⚠️ **É a soma do que está na tela.** Com busca ou filtro ativo o número é do
/// recorte, e a tela precisa dizer isso: número parcial que se apresenta como
/// total é a mesma armadilha que o contador da grade evita do outro lado.
pub fn somar_totais(sessoes: &[&SessaoFotografica]) -> Soma {
    let mut soma = Soma::default();
    for sessao in sessoes {
        match sessao.totais {
            Some(totais) => {
                soma.balcao += totais.balcao;
                soma.pos_venda += totais.pos_venda;
            }
            None => soma.sem_totais += 1,
        }
    }
    soma
}

/// Um degrau do gráfico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Periodo {
    /// `"2026-09-03"` por dia, `"2026-09"` por mês.
    pub chave: String,
    pub balcao: i64,
    pub pos_venda: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Passo {
    Dia,
    Mes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serie {
    pub periodos: Vec<Periodo>,
    pub passo: Passo,
}

/// Acima disto o eixo por dia vira uma tarja; abaixo, o mês esconde a forma.
const DIAS_ATE_AGRUPAR_POR_MES: i64 = 45;

/// O que entrou por período — **pelo dia em que a sessão foi criada**, que é o
/// único carimbo de tempo que a lista tem.
///
/// ⚠️ **Não é faturamento por data de pagamento.** Uma foto comprada hoje numa
/// galeria de dois meses atrás entra no degrau de dois meses atrás. É a leitura
/// honesta do que a lista sabe, e a tela tem de dizer isso: chamar de "vendas do
/// mês" seria um número que responde outra pergunta.
///
/// 🔑 **Período sem sessão nenhuma não vira degrau vazio.** O eixo é categórico
/// — o que aconteceu —, e inventar zeros para dias sem atendimento desenharia
/// uma queda que não houve.
pub fn agrupar_por_periodo(sessoes: &[&SessaoFotografica]) -> Serie {
    if sessoes.is_empty() {
        return Serie {
            periodos: Vec::new(),
            passo: Passo::Dia,
        };
    }

    let mut dias: Vec<&str> = sessoes.iter().map(|s| s.criada_em_iso.as_str()).collect();
    dias.sort_unstable();
    let vao = match (dia_civil(dias[0]), dia_civil(dias[dias.len() - 1])) {
        (Some(primeiro), Some(ultimo)) => ultimo - primeiro,
        // Data ilegível não decide o passo: por dia é o que preserva a forma.
        _ => 0,
    };
    let passo = if vao > DIAS_ATE_AGRUPAR_POR_MES {
        Passo::Mes
    } else {
        Passo::Dia
    };

    // `BTreeMap` porque a saída é ordenada por chave, e a chave ISO ordena como
    // texto exatamente como ordena no tempo.
    let mut por_chave: BTreeMap<String, Periodo> = BTreeMap::new();
    for sessao in sessoes {
        let chave = match passo {
            Passo::Mes => sessao
                .criada_em_iso
                .get(..7)
                .unwrap_or_default()
                .to_string(),
            Passo::Dia => sessao.criada_em_iso.clone(),
        };
        let degrau = por_chave.entry(chave.clone()).or_insert(Periodo {
            chave,
            balcao: 0,
            pos_venda: 0,
        });
        if let Some(totais) = sessao.totais {
            degrau.balcao += totais.balcao;
            degrau.pos_venda += totais.pos_venda;
        }
    }

    Serie {
        periodos: por_chave.into_values().collect(),
        passo,
    }
}

/// O número do dia desde uma época qualquer, a partir de `"AAAA-MM-DD"`.
///
/// 🔑 É o algoritmo civil de Howard Hinnant, e ele está aqui por uma razão só:
/// medir o **vão** entre duas datas sem trazer um crate de calendário para um
/// crate que precisa continuar sem dependência nenhuma. Não faz fuso, não faz
/// hora, não valida mês 13 — mede distância entre dois dias.
fn dia_civil(iso: &str) -> Option<i64> {
    let mut partes = iso.split('-');
    let ano: i64 = partes.next()?.parse().ok()?;
    let mes: i64 = partes.next()?.parse().ok()?;
    let dia: i64 = partes.next()?.get(..2)?.parse().ok()?;

    let ano = if mes <= 2 { ano - 1 } else { ano };
    let era = if ano >= 0 { ano } else { ano - 399 } / 400;
    let ano_da_era = ano - era * 400;
    let dia_do_ano = (153 * (if mes > 2 { mes - 3 } else { mes + 9 }) + 2) / 5 + dia - 1;
    let dia_da_era = ano_da_era * 365 + ano_da_era / 4 - ano_da_era / 100 + dia_do_ano;
    Some(era * 146_097 + dia_da_era - 719_468)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 12h de 3/set/2026, em segundos — o "agora" de todos os testes.
    const AGORA: i64 = 1_788_609_600;
    const UM_DIA: i64 = 86_400;

    fn sessao(titulo: &str) -> SessaoFotografica {
        SessaoFotografica {
            id: titulo.to_string(),
            titulo: titulo.to_string(),
            email: None,
            whatsapp: None,
            criada_em_iso: "2026-09-03".into(),
            expira_em: None,
            user_id: None,
            fotos: ContagemDeFotos {
                disponiveis: 1,
                ..ContagemDeFotos::default()
            },
            totais: None,
        }
    }

    /// 🚨 A ordem da situação é a ordem em que ela importa para quem opera.
    #[test]
    fn vencida_vence_tudo_e_sem_foto_vem_antes_do_cliente() {
        let com_cliente_e_sem_foto = SessaoFotografica {
            user_id: Some("u1".into()),
            fotos: ContagemDeFotos::default(),
            ..sessao("a")
        };
        assert_eq!(
            com_cliente_e_sem_foto.situacao(AGORA),
            Situacao::SemFotos,
            "sem foto viva não há o que o cliente tenha aberto"
        );

        assert_eq!(sessao("b").situacao(AGORA), Situacao::AguardandoCliente);

        let aberta = SessaoFotografica {
            user_id: Some("u1".into()),
            ..sessao("c")
        };
        assert_eq!(aberta.situacao(AGORA), Situacao::AbertaPeloCliente);

        let vencida = SessaoFotografica {
            expira_em: Some(AGORA - UM_DIA),
            user_id: Some("u1".into()),
            ..sessao("d")
        };
        assert_eq!(
            vencida.situacao(AGORA),
            Situacao::Vencida,
            "não adianta subir foto nem esperar o cliente"
        );

        let no_prazo = SessaoFotografica {
            expira_em: Some(AGORA + UM_DIA),
            ..sessao("e")
        };
        assert_eq!(no_prazo.situacao(AGORA), Situacao::AguardandoCliente);
    }

    /// ⚠️ A apagada não conta como viva: o arquivo sumiu pela retenção.
    #[test]
    fn so_apagadas_e_o_mesmo_que_sem_fotos() {
        let so_apagadas = SessaoFotografica {
            fotos: ContagemDeFotos {
                apagadas: 12,
                ..ContagemDeFotos::default()
            },
            ..sessao("a")
        };
        assert_eq!(so_apagadas.situacao(AGORA), Situacao::SemFotos);
    }

    #[test]
    fn normalizar_tira_acento_e_caixa() {
        assert_eq!(normalizar("  JOÃO Gonçalves "), "joao goncalves");
        assert_eq!(normalizar("Ângela"), "angela");
        assert_eq!(normalizar("Müller"), "muller");
    }

    #[test]
    fn a_busca_acha_por_titulo_email_e_telefone() {
        let sessoes = vec![
            SessaoFotografica {
                email: Some("joao@exemplo.com".into()),
                whatsapp: Some("(47) 99999-8888".into()),
                ..sessao("Ensaio do João")
            },
            SessaoFotografica {
                email: Some("maria@outro.com".into()),
                ..sessao("Casamento da Maria")
            },
        ];

        let todas = filtrar(&sessoes, &Criterio::default(), AGORA);
        assert_eq!(todas.len(), 2, "sem critério devolve tudo");

        let por_titulo = filtrar(
            &sessoes,
            &Criterio {
                busca: "joao".into(),
                situacao: None,
            },
            AGORA,
        );
        assert_eq!(por_titulo.len(), 1, "'joao' acha 'João'");
        assert_eq!(por_titulo[0].titulo, "Ensaio do João");

        let por_email = filtrar(
            &sessoes,
            &Criterio {
                busca: "outro.com".into(),
                situacao: None,
            },
            AGORA,
        );
        assert_eq!(por_email.len(), 1);

        let por_telefone = filtrar(
            &sessoes,
            &Criterio {
                busca: "99999".into(),
                situacao: None,
            },
            AGORA,
        );
        assert_eq!(
            por_telefone.len(),
            1,
            "a máscara é apresentação: compara só dígitos"
        );
    }

    /// 🚨 Dois dígitos soltos não acham telefone — seriam todos.
    #[test]
    fn poucos_digitos_nao_valem_como_telefone() {
        let sessoes = vec![SessaoFotografica {
            whatsapp: Some("(47) 99999-8888".into()),
            ..sessao("Ensaio")
        }];
        let achadas = filtrar(
            &sessoes,
            &Criterio {
                busca: "47".into(),
                situacao: None,
            },
            AGORA,
        );
        assert!(achadas.is_empty(), "'47' não pode achar todo mundo");
    }

    #[test]
    fn o_filtro_de_situacao_corta_antes_da_busca() {
        let sessoes = vec![
            sessao("Ensaio do João"),
            SessaoFotografica {
                fotos: ContagemDeFotos::default(),
                ..sessao("Ensaio da Joana")
            },
        ];
        let achadas = filtrar(
            &sessoes,
            &Criterio {
                busca: "ensaio".into(),
                situacao: Some(Situacao::SemFotos),
            },
            AGORA,
        );
        assert_eq!(achadas.len(), 1);
        assert_eq!(achadas[0].titulo, "Ensaio da Joana");
    }

    #[test]
    fn as_contagens_batem_com_o_total() {
        let sessoes = vec![
            sessao("a"),
            SessaoFotografica {
                fotos: ContagemDeFotos::default(),
                ..sessao("b")
            },
            SessaoFotografica {
                user_id: Some("u".into()),
                ..sessao("c")
            },
            SessaoFotografica {
                expira_em: Some(AGORA - UM_DIA),
                ..sessao("d")
            },
        ];
        let contagens = contar_por_situacao(&sessoes, AGORA);
        assert_eq!(contagens.todas, 4);
        assert_eq!(contagens.de(Situacao::AguardandoCliente), 1);
        assert_eq!(contagens.de(Situacao::SemFotos), 1);
        assert_eq!(contagens.de(Situacao::AbertaPeloCliente), 1);
        assert_eq!(contagens.de(Situacao::Vencida), 1);
    }

    /// 🔑 A sessão sem totais é **contada à parte**, e não somada como zero.
    #[test]
    fn somar_separa_as_portas_e_conta_quem_nao_tem_totais() {
        let com = SessaoFotografica {
            totais: Some(Totais {
                balcao: 5_000,
                pos_venda: 1_990,
            }),
            ..sessao("a")
        };
        let sem = sessao("b");
        let lista = vec![&com, &sem];

        let soma = somar_totais(&lista);
        assert_eq!(soma.balcao, 5_000);
        assert_eq!(soma.pos_venda, 1_990);
        assert_eq!(soma.sem_totais, 1);

        assert_eq!(somar_totais(&[]), Soma::default(), "lista vazia é zero");
    }

    #[test]
    fn o_grafico_agrupa_por_dia_quando_a_historia_e_curta() {
        let a = SessaoFotografica {
            criada_em_iso: "2026-09-01".into(),
            totais: Some(Totais {
                balcao: 100,
                pos_venda: 0,
            }),
            ..sessao("a")
        };
        let b = SessaoFotografica {
            criada_em_iso: "2026-09-01".into(),
            totais: Some(Totais {
                balcao: 50,
                pos_venda: 20,
            }),
            ..sessao("b")
        };
        let c = SessaoFotografica {
            criada_em_iso: "2026-09-05".into(),
            totais: Some(Totais {
                balcao: 0,
                pos_venda: 7,
            }),
            ..sessao("c")
        };

        // Fora de ordem de propósito: o degrau sai cronológico assim mesmo.
        let serie = agrupar_por_periodo(&[&c, &a, &b]);
        assert_eq!(serie.passo, Passo::Dia);
        assert_eq!(serie.periodos.len(), 2, "dia sem atendimento não vira zero");
        assert_eq!(serie.periodos[0].chave, "2026-09-01");
        assert_eq!(serie.periodos[0].balcao, 150);
        assert_eq!(serie.periodos[0].pos_venda, 20);
        assert_eq!(serie.periodos[1].chave, "2026-09-05");
    }

    /// Com mais de 45 dias de história o eixo por dia vira tarja.
    #[test]
    fn o_grafico_passa_a_agrupar_por_mes_quando_o_intervalo_cresce() {
        let antiga = SessaoFotografica {
            criada_em_iso: "2026-06-10".into(),
            ..sessao("antiga")
        };
        let nova = SessaoFotografica {
            criada_em_iso: "2026-09-05".into(),
            ..sessao("nova")
        };
        let serie = agrupar_por_periodo(&[&antiga, &nova]);
        assert_eq!(serie.passo, Passo::Mes);
        assert_eq!(serie.periodos[0].chave, "2026-06");
        assert_eq!(serie.periodos[1].chave, "2026-09");
    }

    #[test]
    fn sem_sessao_nenhuma_o_grafico_nao_tem_degrau() {
        let serie = agrupar_por_periodo(&[]);
        assert!(serie.periodos.is_empty());
        assert_eq!(serie.passo, Passo::Dia);
    }

    /// O algoritmo civil mede o vão, e é só para isso que ele está aqui.
    #[test]
    fn o_dia_civil_mede_a_distancia_entre_datas() {
        let um = dia_civil("2026-09-01").expect("data boa");
        let outro = dia_civil("2026-09-05").expect("data boa");
        assert_eq!(outro - um, 4);

        // Ano bissexto, e a virada de fevereiro.
        let fim_de_fevereiro = dia_civil("2024-02-28").expect("data boa");
        let primeiro_de_marco = dia_civil("2024-03-01").expect("data boa");
        assert_eq!(primeiro_de_marco - fim_de_fevereiro, 2, "2024 tem 29/02");

        assert_eq!(dia_civil("nada disso"), None);
    }
}
