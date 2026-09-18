//! O seletor de período da lista de sessões — **em português**.
//!
//! # Por que não é o `DatePicker` do `gpui-component`
//!
//! 🚨 **Ele não fala português, e não há como ensiná-lo de fora** (dono,
//! 18/set/2026: *"ainda está exibindo os meses e semanas em inglês"*, e *"no
//! `ui-gpui` não tem seleção de idioma i18n, mas o padrão é português"*). O
//! calendário de lá resolve os nomes por `rust_i18n::t!`, e o dicionário
//! (`locales/ui.yml`: `en`, `zh-CN`, `zh-HK`, `it`) é **compilado dentro do
//! crate**, numa `Lazy` estática que nenhum outro crate alcança. Trocar o
//! idioma exigiria um fork.
//!
//! O que sobra é pequeno e é nosso: um mês desenhado numa grade de sete
//! colunas. O que ele **não** tem — seleção de hora, múltiplos meses, presets
//! genéricos — é o que esta tela não usa.
//!
//! # O desenho é o do `DatePicker` do shadcn
//!
//! Um gatilho com o rótulo do período, e um popover com os atalhos em cima e o
//! mês embaixo. Os mesmos sete atalhos do site (`filtro-de-periodo.tsx`), a
//! mesma regra de um clique só (o primeiro dia já filtra) e o mesmo "Tudo".

use biblioteca_core::sessoes::FaixaDeDatas;
use chrono::{Datelike, NaiveDate};
use gpui::{div, prelude::*, px, SharedString};
use gpui_component::{h_flex, v_flex, ActiveTheme};

/// Os meses, como o Brasil os escreve.
const MESES: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];

/// 🔑 **A semana começa no domingo**, como no calendário brasileiro (e como no
/// `Calendar` do shadcn com `locale={ptBR}`… que também começa no domingo).
const DIAS: [&str; 7] = ["D", "S", "T", "Q", "Q", "S", "S"];

/// O que o seletor guarda enquanto está aberto.
#[derive(Debug, Clone)]
pub struct EstadoDoPeriodo {
    /// O mês desenhado agora — sempre o dia 1 dele.
    pub mes: NaiveDate,
    /// O primeiro clique de um intervalo, à espera do segundo.
    pub comecando: Option<NaiveDate>,
}

impl EstadoDoPeriodo {
    pub fn no_mes_de(dia: NaiveDate) -> Self {
        Self {
            mes: primeiro_do_mes(dia),
            comecando: None,
        }
    }

    pub fn andar_mes(&mut self, passo: i32) {
        self.mes = somar_meses(self.mes, passo);
    }
}

/// O rótulo do gatilho — o que se lê sem abrir nada.
///
/// 🔑 **"Hoje" e "Ontem" por extenso**: são os dois estados em que a tela mais
/// fica, e ver "18/09" ali faria parecer um filtro escolhido por alguém.
pub fn rotulo(faixa: Option<&FaixaDeDatas>, hoje: NaiveDate) -> String {
    let Some(faixa) = faixa else {
        return "Todo o período".to_string();
    };
    let (de, ate) = faixa.em_ordem();
    if de == ate {
        let hoje_iso = iso(hoje);
        if de == hoje_iso {
            return "Hoje".to_string();
        }
        if de == iso(hoje.pred_opt().unwrap_or(hoje)) {
            return "Ontem".to_string();
        }
        return curto(de, hoje);
    }
    format!("{} – {}", curto(de, hoje), curto(ate, hoje))
}

/// `2026-09-18` → `18/09` (ou `18/09/2025`, quando o ano não é o de agora).
///
/// 🇧🇷 Dia antes do mês, sempre. O ano só entra quando ele muda a leitura —
/// senão a data vira uma barra de números que ninguém lê.
fn curto(data_iso: &str, hoje: NaiveDate) -> String {
    let mut partes = data_iso.split('-');
    let (Some(ano), Some(mes), Some(dia)) = (partes.next(), partes.next(), partes.next()) else {
        return data_iso.to_string();
    };
    if ano == hoje.year().to_string() {
        format!("{dia}/{mes}")
    } else {
        format!("{dia}/{mes}/{ano}")
    }
}

/// O título do mês: `setembro 2026`.
pub fn titulo_do_mes(mes: NaiveDate) -> String {
    format!("{} {}", MESES[(mes.month() - 1) as usize], mes.year())
}

/// Os dias que a grade desenha: o mês inteiro, precedido pelos vazios até o
/// primeiro domingo.
///
/// 🔑 **Vazios em vez dos dias do mês anterior**: eles não são clicáveis nesta
/// tela, e desenhá-los apagados só convida ao clique que não faz nada.
pub fn dias_da_grade(mes: NaiveDate) -> Vec<Option<NaiveDate>> {
    let primeiro = primeiro_do_mes(mes);
    // `num_days_from_sunday` é 0 no domingo — a coluna 0 da grade.
    let vazios = primeiro.weekday().num_days_from_sunday() as usize;
    let dias_no_mes = dias_no_mes(primeiro);
    let mut grade: Vec<Option<NaiveDate>> = vec![None; vazios];
    for dia in 1..=dias_no_mes {
        grade.push(primeiro.with_day(dia));
    }
    grade
}

/// O clique num dia: começa um intervalo, ou o fecha.
///
/// ⚠️ **O primeiro clique já filtra** (um dia só), como no site: esperar o
/// segundo deixaria a lista parada no meio do gesto.
pub fn clicar_no_dia(estado: &mut EstadoDoPeriodo, dia: NaiveDate) -> FaixaDeDatas {
    match estado.comecando.take() {
        // Fecha o intervalo — em ordem, venha o segundo clique antes ou depois.
        Some(inicio) => {
            let (de, ate) = if inicio <= dia {
                (inicio, dia)
            } else {
                (dia, inicio)
            };
            FaixaDeDatas {
                de: iso(de),
                ate: iso(ate),
            }
        }
        None => {
            estado.comecando = Some(dia);
            FaixaDeDatas::no_dia(iso(dia))
        }
    }
}

/// Este dia está dentro do que já foi escolhido?
pub fn dentro(faixa: Option<&FaixaDeDatas>, dia: NaiveDate) -> bool {
    faixa.is_some_and(|f| f.contem(&iso(dia)))
}

/// Os sete atalhos, na ordem em que o balcão pergunta.
///
/// ⚠️ **Trimestre, semestre e ano são corridos** (os últimos 90, 180 e 365
/// dias): a pergunta de quem fecha caixa é "quanto entrou até aqui", e não "o
/// que aconteceu no trimestre civil".
pub fn atalhos(hoje: NaiveDate) -> Vec<(&'static str, Option<FaixaDeDatas>)> {
    let ha = |dias: i64| hoje - chrono::Duration::days(dias);
    let faixa = |de: NaiveDate, ate: NaiveDate| {
        Some(FaixaDeDatas {
            de: iso(de),
            ate: iso(ate),
        })
    };
    vec![
        ("Hoje", faixa(hoje, hoje)),
        ("Ontem", faixa(ha(1), ha(1))),
        ("7 dias", faixa(ha(6), hoje)),
        ("30 dias", faixa(ha(29), hoje)),
        ("Trimestre", faixa(ha(89), hoje)),
        ("Semestre", faixa(ha(179), hoje)),
        ("Ano", faixa(ha(364), hoje)),
        ("Tudo", None),
    ]
}

/// A grade do mês, desenhada. `escolher` recebe o dia clicado.
pub fn calendario<T: 'static>(
    estado: &EstadoDoPeriodo,
    faixa: Option<&FaixaDeDatas>,
    hoje: NaiveDate,
    cx: &mut gpui::Context<T>,
    escolher: impl Fn(&mut T, NaiveDate, &mut gpui::Window, &mut gpui::Context<T>) + Clone + 'static,
) -> gpui::AnyElement {
    let tema = cx.theme().clone();
    let lado = px(30.);
    let semana = h_flex().gap(px(2.)).children(DIAS.iter().map(|d| {
        div()
            .w(lado)
            .text_xs()
            .text_color(tema.muted_foreground)
            .flex()
            .justify_center()
            .child(*d)
    }));

    let mut linhas: Vec<gpui::AnyElement> = Vec::new();
    for semana_de_dias in dias_da_grade(estado.mes).chunks(7) {
        let mut linha = h_flex().gap(px(2.));
        for dia in semana_de_dias {
            linha = match dia {
                None => linha.child(div().w(lado).h(lado)),
                Some(dia) => {
                    let dia = *dia;
                    let escolhido = dentro(faixa, dia);
                    let e_hoje = dia == hoje;
                    let escolher = escolher.clone();
                    linha.child(
                        div()
                            .id(SharedString::from(format!("dia-{dia}")))
                            .w(lado)
                            .h(lado)
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.))
                            .text_sm()
                            .cursor_pointer()
                            .when(escolhido, |d| {
                                d.bg(tema.primary).text_color(tema.primary_foreground)
                            })
                            .when(!escolhido && e_hoje, |d| {
                                // 🔑 Hoje tem borda, e não fundo: o fundo é da
                                // escolha, e os dois juntos fariam parecer que
                                // hoje está escolhido sempre.
                                d.border_1().border_color(tema.primary)
                            })
                            .when(!escolhido, |d| {
                                let acento = tema.accent;
                                d.hover(move |s| s.bg(acento))
                            })
                            .child(dia.day().to_string())
                            .on_click(cx.listener(move |tela, _ev, window, cx| {
                                escolher(tela, dia, window, cx)
                            })),
                    )
                }
            };
        }
        linhas.push(linha.into_any_element());
    }

    v_flex()
        .gap(px(4.))
        .child(semana)
        .children(linhas)
        .into_any_element()
}

fn iso(dia: NaiveDate) -> String {
    dia.format("%Y-%m-%d").to_string()
}

fn primeiro_do_mes(dia: NaiveDate) -> NaiveDate {
    dia.with_day(1).unwrap_or(dia)
}

fn dias_no_mes(primeiro: NaiveDate) -> u32 {
    let proximo = somar_meses(primeiro, 1);
    proximo.signed_duration_since(primeiro).num_days() as u32
}

/// Soma meses sem estourar o fim do mês — sempre no dia 1.
fn somar_meses(mes: NaiveDate, passo: i32) -> NaiveDate {
    let total = mes.year() * 12 + mes.month0() as i32 + passo;
    let (ano, mes0) = (total.div_euclid(12), total.rem_euclid(12) as u32);
    NaiveDate::from_ymd_opt(ano, mes0 + 1, 1).unwrap_or(mes)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn dia(iso: &str) -> NaiveDate {
        NaiveDate::parse_from_str(iso, "%Y-%m-%d").expect("data")
    }

    /// 🇧🇷 O rótulo fala como o balcão.
    #[test]
    fn o_rotulo_diz_hoje_ontem_e_a_data_em_dia_mes() {
        let hoje = dia("2026-09-18");

        assert_eq!(rotulo(None, hoje), "Todo o período");
        assert_eq!(
            rotulo(Some(&FaixaDeDatas::no_dia("2026-09-18")), hoje),
            "Hoje"
        );
        assert_eq!(
            rotulo(Some(&FaixaDeDatas::no_dia("2026-09-17")), hoje),
            "Ontem"
        );
        assert_eq!(
            rotulo(Some(&FaixaDeDatas::no_dia("2026-09-12")), hoje),
            "12/09"
        );
        assert_eq!(
            rotulo(Some(&FaixaDeDatas::no_dia("2025-12-31")), hoje),
            "31/12/2025",
            "o ano entra quando não é o de agora"
        );
        assert_eq!(
            rotulo(
                Some(&FaixaDeDatas {
                    de: "2026-09-12".into(),
                    ate: "2026-09-18".into()
                }),
                hoje
            ),
            "12/09 – 18/09"
        );
    }

    /// 🇧🇷 O mês em português, com o ano ao lado.
    #[test]
    fn o_titulo_do_mes_e_em_portugues() {
        assert_eq!(titulo_do_mes(dia("2026-09-01")), "setembro 2026");
        assert_eq!(titulo_do_mes(dia("2026-03-15")), "março 2026");
    }

    /// 🚨 A grade começa no domingo e tem um dia por dia do mês.
    #[test]
    fn a_grade_alinha_o_mes_no_domingo() {
        // 1º de setembro de 2026 é uma terça — dois vazios antes.
        let grade = dias_da_grade(dia("2026-09-10"));

        assert_eq!(grade[0], None);
        assert_eq!(grade[1], None);
        assert_eq!(grade[2], Some(dia("2026-09-01")));
        assert_eq!(grade.iter().filter(|d| d.is_some()).count(), 30);
        assert_eq!(grade.last().copied().flatten(), Some(dia("2026-09-30")));
    }

    /// Fevereiro bissexto — o mês mais fácil de errar.
    #[test]
    fn fevereiro_de_ano_bissexto_tem_29() {
        let grade = dias_da_grade(dia("2028-02-05"));

        assert_eq!(grade.iter().filter(|d| d.is_some()).count(), 29);
    }

    /// ⚠️ O primeiro clique já filtra; o segundo fecha o intervalo — em ordem,
    /// mesmo clicando de trás para a frente.
    #[test]
    fn dois_cliques_fecham_o_intervalo_em_ordem() {
        let mut estado = EstadoDoPeriodo::no_mes_de(dia("2026-09-18"));

        let primeiro = clicar_no_dia(&mut estado, dia("2026-09-18"));
        assert_eq!(primeiro, FaixaDeDatas::no_dia("2026-09-18"));
        assert!(estado.comecando.is_some(), "espera o segundo clique");

        let segundo = clicar_no_dia(&mut estado, dia("2026-09-12"));
        assert_eq!(
            segundo,
            FaixaDeDatas {
                de: "2026-09-12".into(),
                ate: "2026-09-18".into()
            },
            "clicar de trás para a frente dá o mesmo intervalo"
        );
        assert!(estado.comecando.is_none(), "o gesto terminou");
    }

    /// A navegação de mês atravessa o ano sem estourar o dia.
    #[test]
    fn andar_de_mes_atravessa_o_ano() {
        let mut estado = EstadoDoPeriodo::no_mes_de(dia("2026-01-31"));

        estado.andar_mes(-1);
        assert_eq!(estado.mes, dia("2025-12-01"));
        estado.andar_mes(2);
        assert_eq!(estado.mes, dia("2026-02-01"));
    }

    /// Os oito atalhos, com as janelas corridas.
    #[test]
    fn os_atalhos_sao_os_do_balcao() {
        let hoje = dia("2026-09-18");
        let atalhos = atalhos(hoje);

        let nomes: Vec<&str> = atalhos.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            nomes,
            vec![
                "Hoje",
                "Ontem",
                "7 dias",
                "30 dias",
                "Trimestre",
                "Semestre",
                "Ano",
                "Tudo"
            ]
        );
        assert_eq!(
            atalhos[2].1,
            Some(FaixaDeDatas {
                de: "2026-09-12".into(),
                ate: "2026-09-18".into()
            }),
            "7 dias conta hoje"
        );
        assert_eq!(atalhos[7].1, None, "Tudo é o arquivo inteiro");
    }
}
