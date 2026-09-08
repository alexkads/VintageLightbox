//! Reais, em centavos — a leitura e a escrita que a tela do balcão faz.
//!
//! É o `money.ts` do site, portado: o preço aparece em `pt-BR` **fixo**, e não
//! no idioma de quem lê, porque é em reais que ele está no app do banco, no
//! código PIX e na fatura do MercadoPago. `en-US` produziria `R$1,999.90` para
//! o mesmo valor — a moeda é a mesma, o separador troca de papel, e quem
//! confere a tela contra o banco vê dois números para a mesma cobrança.
//!
//! ⚠️ **Sem ponto flutuante no caminho do dinheiro.** `ler_campo` converte o
//! texto para centavos com aritmética de inteiros: `19,90` vira `1990` porque
//! são os dígitos, e não porque `19.9 * 100` deu perto de `1990`.

/// `1999` → `"R$ 19,99"`, `199990` → `"R$ 1.999,90"`, `-100` → `"-R$ 1,00"`.
pub fn formatar(centavos: i64) -> String {
    let sinal = if centavos < 0 { "-" } else { "" };
    let absoluto = centavos.unsigned_abs();
    format!(
        "{sinal}R$ {}",
        com_separadores(absoluto / 100, absoluto % 100)
    )
}

/// Reais que o gateway devolve como decimal (`299.0`), formatados do mesmo
/// jeito. É a única entrada em ponto flutuante, e ela nasce fora daqui.
pub fn formatar_reais(valor: f64) -> String {
    let centavos = (valor * 100.0).round() as i64;
    formatar(centavos)
}

/// O texto que vai **dentro de um campo**: `19990` → `"199,90"`.
///
/// Inverso exato de [`ler_campo`], e por isso sem `R$` e sem separador de
/// milhar: o que sai daqui volta por lá inalterado ao salvar.
pub fn formatar_campo(centavos: i64) -> String {
    let sinal = if centavos < 0 { "-" } else { "" };
    let absoluto = centavos.unsigned_abs();
    format!("{sinal}{},{:02}", absoluto / 100, absoluto % 100)
}

/// O que o operador digitou, em centavos — `None` para o que não é dinheiro.
///
/// Aceita `19,90`, `R$ 19,90`, `1.999,90`, `1999.90` e `1.999` (que é mil
/// novecentos e noventa e nove reais: ninguém escreve "1.999" para dizer um
/// real e noventa e nove centavos). Vazio é `None`, e não zero: o campo vazio
/// do preço de venda significa "volta à faixa", que é outra coisa.
pub fn ler_campo(texto: &str) -> Option<i64> {
    let limpo = texto.trim();
    let limpo = limpo
        .strip_prefix("R$")
        .or_else(|| limpo.strip_prefix("r$"))
        .map(str::trim_start)
        .unwrap_or(limpo);
    if limpo.is_empty() {
        return None;
    }

    let (negativo, sem_sinal) = match limpo.strip_prefix('-') {
        Some(resto) => (true, resto),
        None => (false, limpo),
    };

    let normalizado: String = if sem_sinal.contains(',') {
        sem_sinal.replace('.', "").replacen(',', ".", 1)
    } else if so_grupos_de_tres(sem_sinal) {
        sem_sinal.replace('.', "")
    } else {
        sem_sinal.to_string()
    };

    let (inteiros, decimais) = match normalizado.split_once('.') {
        Some((i, d)) => (i, d),
        None => (normalizado.as_str(), ""),
    };
    if inteiros.is_empty() || !inteiros.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !decimais.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }

    let reais: i64 = inteiros.parse().ok()?;
    let mut digitos = decimais.bytes().map(|b| i64::from(b - b'0'));
    let d1 = digitos.next().unwrap_or(0);
    let d2 = digitos.next().unwrap_or(0);
    // O terceiro dígito arredonda o segundo, como o `Math.round` fazia.
    let arredonda = digitos.next().is_some_and(|d| d >= 5);
    let centavos = reais.checked_mul(100)? + d1 * 10 + d2 + i64::from(arredonda);
    Some(if negativo { -centavos } else { centavos })
}

/// `1.999`, `199.900`, `1.234.567`: só grupos de exatamente três.
fn so_grupos_de_tres(texto: &str) -> bool {
    let mut partes = texto.split('.');
    let Some(primeira) = partes.next() else {
        return false;
    };
    let primeira_ok =
        (1..=3).contains(&primeira.len()) && primeira.bytes().all(|b| b.is_ascii_digit());
    let mut houve_grupo = false;
    for grupo in partes {
        houve_grupo = true;
        if grupo.len() != 3 || !grupo.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    primeira_ok && houve_grupo
}

fn com_separadores(reais: u64, centavos: u64) -> String {
    let digitos = reais.to_string();
    let mut saida = String::with_capacity(digitos.len() + digitos.len() / 3 + 3);
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i).is_multiple_of(3) {
            saida.push('.');
        }
        saida.push(c);
    }
    format!("{saida},{centavos:02}")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn formata_em_reais_com_separador_brasileiro() {
        assert_eq!(formatar(1999), "R$ 19,99");
        assert_eq!(formatar(199_990), "R$ 1.999,90");
        assert_eq!(formatar(123_456_700), "R$ 1.234.567,00");
        assert_eq!(formatar(0), "R$ 0,00");
        assert_eq!(formatar(-100), "-R$ 1,00");
    }

    #[test]
    fn o_campo_nao_leva_moeda_nem_milhar() {
        assert_eq!(formatar_campo(19_990), "199,90");
        assert_eq!(formatar_campo(5), "0,05");
        assert_eq!(formatar_campo(-250), "-2,50");
    }

    #[test]
    fn le_o_que_o_operador_digita() {
        assert_eq!(ler_campo("19,90"), Some(1990));
        assert_eq!(ler_campo("R$ 19,90"), Some(1990));
        assert_eq!(ler_campo("1.999,90"), Some(199_990));
        assert_eq!(ler_campo("1999.90"), Some(199_990));
        assert_eq!(ler_campo("199.9"), Some(19_990));
        assert_eq!(ler_campo("25"), Some(2500));
        assert_eq!(ler_campo("-2,50"), Some(-250));
    }

    /// Ninguém escreve "1.999" para dizer um real e noventa e nove centavos.
    #[test]
    fn ponto_em_grupos_de_tres_e_milhar() {
        assert_eq!(ler_campo("1.999"), Some(199_900));
        assert_eq!(ler_campo("199.900"), Some(19_990_000));
        assert_eq!(ler_campo("1.234.567"), Some(123_456_700));
    }

    #[test]
    fn vazio_e_ausencia_e_lixo_e_nada() {
        assert_eq!(ler_campo(""), None);
        assert_eq!(ler_campo("   "), None);
        assert_eq!(ler_campo("abc"), None);
        assert_eq!(ler_campo("1,2,3"), None);
        assert_eq!(ler_campo("12.34.5"), None);
    }

    #[test]
    fn o_terceiro_decimal_arredonda() {
        assert_eq!(ler_campo("1,995"), Some(200));
        assert_eq!(ler_campo("1,994"), Some(199));
    }

    /// A ida e volta do campo tem de ser exata: preço é onde ela custa dinheiro.
    #[test]
    fn campo_e_leitura_sao_inversos() {
        for centavos in [0, 5, 99, 100, 1990, 19_990, 199_990, 123_456_789] {
            assert_eq!(ler_campo(&formatar_campo(centavos)), Some(centavos));
        }
    }

    #[test]
    fn reais_do_gateway() {
        assert_eq!(formatar_reais(299.0), "R$ 299,00");
        assert_eq!(formatar_reais(2.99), "R$ 2,99");
    }
}
