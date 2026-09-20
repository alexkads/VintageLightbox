//! O preço de venda online de uma foto, em regras puras.
//!
//! É o valor que a galeria do cliente mostra e que o pedido cobra — o
//! **contrário** da negociação do balcão ([`crate::negociacao`]), que registra
//! o que já aconteceu e não muda a compra. No banco é um campo só
//! (`preco_de_venda`, centavos): `None` = vale o preço da faixa.
//!
//! Positivo, sempre. Zero não é preço de venda — é cortesia, e cortesia é a
//! negociação. O backend recusa zero e negativo com `400`; a conferência aqui
//! poupa a ida ao servidor e, em lote, poupa N idas que voltariam iguais.

use crate::dinheiro;

pub fn ler(texto: &str) -> Result<i64, String> {
    let Some(preco) = dinheiro::ler_campo(texto) else {
        return Err("Valor inválido. Use o formato 19,90.".into());
    };
    if preco <= 0 {
        return Err("O preço de venda tem de ser maior que zero. Cortesia é negociação.".into());
    }
    Ok(preco)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_um_preco_valido() {
        assert_eq!(ler("19,90"), Ok(1990));
        assert_eq!(ler("R$ 1.999,00"), Ok(199_900));
    }

    #[test]
    fn zero_e_cortesia_e_nao_preco() {
        assert!(ler("0").unwrap_err().contains("Cortesia"));
        assert!(ler("-5,00").unwrap_err().contains("maior que zero"));
    }

    #[test]
    fn lixo_e_valor_invalido() {
        assert!(ler("abc").unwrap_err().contains("19,90"));
        assert!(ler("").is_err());
    }
}
