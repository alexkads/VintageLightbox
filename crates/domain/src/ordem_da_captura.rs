//! A ordem em que o ensaio foi fotografado — a regra, num lugar só.
//!
//! # Por que existe
//!
//! 🚨 **"Tem que ser tudo na ordem da fotografia"** (dono, 20/set/2026). A
//! ordem do ensaio era decidida em quatro lugares diferentes, e cada um
//! respondia uma coisa:
//!
//! - a grade de importação ordenava pela data crua, onde *sem data* é menor que
//!   qualquer data — o arquivo sem EXIF abria o ensaio;
//! - o catálogo listava por `imported_at DESC` — a ordem da fotografia,
//!   invertida, e com as importações concorrentes embaralhadas entre si;
//! - o nome do arquivo comparava byte a byte, onde `IMG_10` vem antes de
//!   `IMG_9`;
//! - e a rajada não tinha desempate nenhum, porque o subsegundo não era lido.
//!
//! As quatro respostas agora saem daqui. A mesma regra, escrita em TypeScript,
//! está em `ordem-da-captura.ts`, no site — e as duas dizem o mesmo:
//! **com captura primeiro, na ordem do disparo; sem captura no fim, por nome; e
//! as duas metades não se misturam.**
//!
//! A chave de captura em si é de quem a lê: [`crate::value_objects::PhotoMetadata::chave_de_captura`].

use std::cmp::Ordering;

/// Compara dois nomes de arquivo como a câmera os conta.
///
/// 🚨 **`IMG_9` vem antes de `IMG_10`.** A comparação de texto do Rust é byte a
/// byte, e nela `'1' < '9'`: um cartão que passou de `IMG_9` para `IMG_10`
/// apareceria com a décima foto no meio das unidades. Como este é o desempate
/// de toda ordenação — e a ordem inteira de quem não tem EXIF —, ele tem de ler
/// os números como números.
///
/// Os blocos de dígitos comparam por comprimento e depois por texto, sem virar
/// inteiro: um nome com trinta algarismos estouraria qualquer `u64`, e o que se
/// quer aqui não é o valor, é a ordem.
pub fn comparar_nome(a: &str, b: &str) -> Ordering {
    fn tomar_numero(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
        let mut digitos = String::new();
        while let Some(c) = it.peek().copied() {
            if !c.is_ascii_digit() {
                break;
            }
            digitos.push(c);
            it.next();
        }
        // Zeros à esquerda não mudam o valor: `007` e `7` são a mesma contagem,
        // de duas câmeras configuradas diferente.
        let sem_zeros = digitos.trim_start_matches('0');
        if sem_zeros.is_empty() {
            "0".to_string()
        } else {
            sem_zeros.to_string()
        }
    }

    let (mut ca, mut cb) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ca.peek().copied(), cb.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (na, nb) = (tomar_numero(&mut ca), tomar_numero(&mut cb));
                match na.len().cmp(&nb.len()).then_with(|| na.cmp(&nb)) {
                    Ordering::Equal => {}
                    outro => return outro,
                }
            }
            (Some(x), Some(y)) => {
                ca.next();
                cb.next();
                match x
                    .to_lowercase()
                    .cmp(y.to_lowercase())
                    .then_with(|| x.cmp(&y))
                {
                    Ordering::Equal => {}
                    outro => return outro,
                }
            }
        }
    }
}

/// Compara duas fotos pela ordem em que foram fotografadas.
///
/// `captura` é a chave do disparo (vazia quando não há) e `nome` é o do arquivo.
///
/// 🔑 **Quem não tem captura vai para o fim, e as duas metades não se
/// misturam.** Chutar uma data para quem não tem — a do arquivo, que é a da
/// *cópia* — o enfiaria num ponto qualquer da sequência; no fim, por nome, é
/// previsível, e é onde quem procura vai olhar.
pub fn comparar_captura(a: (&str, &str), b: (&str, &str)) -> Ordering {
    let (captura_a, nome_a) = a;
    let (captura_b, nome_b) = b;
    captura_a
        .is_empty()
        .cmp(&captura_b.is_empty())
        .then_with(|| captura_a.cmp(captura_b))
        .then_with(|| comparar_nome(nome_a, nome_b))
}

/// Põe uma lista na ordem da fotografia, dada a chave de captura e o nome de
/// cada item.
///
/// A ordenação é **estável**: empate de captura cai no nome, e empate de nome
/// mantém a ordem em que já estava.
pub fn ordenar_pela_captura<T, F>(itens: &mut [T], extrair: F)
where
    F: Fn(&T) -> (String, String),
{
    itens.sort_by(|a, b| {
        let (ca, na) = extrair(a);
        let (cb, nb) = extrair(b);
        comparar_captura((&ca, &na), (&cb, &nb))
    });
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_numero_compara_como_numero() {
        assert_eq!(comparar_nome("IMG_9.NEF", "IMG_10.NEF"), Ordering::Less);
        assert_eq!(
            comparar_nome("IMG_100.NEF", "IMG_99.NEF"),
            Ordering::Greater
        );
        assert_eq!(comparar_nome("IMG_0009.NEF", "IMG_9.NEF"), Ordering::Equal);
        assert_eq!(comparar_nome("DSC_2.jpg", "dsc_10.jpg"), Ordering::Less);
        assert_eq!(comparar_nome("a.jpg", "a.jpg"), Ordering::Equal);
        assert_eq!(comparar_nome("a.jpg", "a.jpeg"), Ordering::Greater);
    }

    /// Um nome com mais algarismos do que cabe num inteiro não pode entrar em
    /// pânico nem mentir na comparação.
    #[test]
    fn numero_gigante_nao_estoura() {
        let grande = format!("IMG_{}.jpg", "9".repeat(40));
        let maior = format!("IMG_{}.jpg", "9".repeat(41));
        assert_eq!(comparar_nome(&grande, &maior), Ordering::Less);
    }

    #[test]
    fn com_captura_vem_antes_de_sem_captura() {
        let mut fotos = vec![
            ("", "escaneada.png"),
            ("2026:09:20 12:00:00.000", "IMG_2.jpg"),
            ("", "antiga.png"),
            ("2026:09:20 09:00:00.000", "IMG_1.jpg"),
        ];
        ordenar_pela_captura(&mut fotos, |f| (f.0.to_string(), f.1.to_string()));
        assert_eq!(
            fotos.iter().map(|f| f.1).collect::<Vec<_>>(),
            ["IMG_1.jpg", "IMG_2.jpg", "antiga.png", "escaneada.png"]
        );
    }

    /// 🚨 O contador que virou: por nome o ensaio novo abriria a lista; por
    /// disparo ele fecha, que é onde ele foi fotografado.
    #[test]
    fn o_contador_que_virou_nao_reordena_o_ensaio() {
        let mut fotos = vec![
            ("2026:09:20 09:00:02.000", "IMG_0001.jpg"),
            ("2026:09:20 09:00:00.000", "IMG_9998.jpg"),
            ("2026:09:20 09:00:01.000", "IMG_9999.jpg"),
        ];
        ordenar_pela_captura(&mut fotos, |f| (f.0.to_string(), f.1.to_string()));
        assert_eq!(
            fotos.iter().map(|f| f.1).collect::<Vec<_>>(),
            ["IMG_9998.jpg", "IMG_9999.jpg", "IMG_0001.jpg"]
        );
    }

    #[test]
    fn a_ordenacao_e_estavel_entre_chamadas() {
        let mut fotos = vec![
            ("2026:09:20 09:00:00.000", "b.jpg"),
            ("2026:09:20 09:00:00.000", "a.jpg"),
            ("", "z.png"),
        ];
        ordenar_pela_captura(&mut fotos, |f| (f.0.to_string(), f.1.to_string()));
        let primeira: Vec<_> = fotos.iter().map(|f| f.1).collect();
        ordenar_pela_captura(&mut fotos, |f| (f.0.to_string(), f.1.to_string()));
        assert_eq!(fotos.iter().map(|f| f.1).collect::<Vec<_>>(), primeira);
        assert_eq!(primeira, ["a.jpg", "b.jpg", "z.png"]);
    }
}
