//! A altura da tira — arrastável, e guardada ao lado do catálogo.
//!
//! # Por que a altura é o zoom
//!
//! 🔑 Pedido do dono, 2026-09-05, para a tela da web: *"resizable vertical nas
//! tiras, dê zoom nas fotos"*. São a mesma coisa: a tira é uma faixa de
//! miniaturas, e a única dimensão livre dela é a altura — puxar a borda para
//! cima aumenta a miniatura, que é o zoom. Um controle separado seria um segundo
//! jeito de dizer o mesmo, com os dois discordando no dia em que a janela
//! mudasse de tamanho.
//!
//! É o porte de `altura-da-tira.ts` do site, com os mesmos números — se os dois
//! divergirem, a mesma tira terá dois tamanhos mínimos em duas telas que o
//! operador usa no mesmo dia.
//!
//! # Onde fica gravada
//!
//! Ao lado do catálogo, como o arranjo dos painéis ([`crate::biblioteca::arranjo`])
//! e pela mesma razão: rodar contra um catálogo de medição não pode mexer na
//! arrumação de quem trabalha. Falhar ao ler ou gravar **nunca** interrompe —
//! o pior desfecho é a tira voltar ao tamanho padrão.

use std::path::PathBuf;

use infrastructure::paths::AppPaths;

/// O mínimo cabe a miniatura e a linha de atalhos.
pub const ALTURA_MINIMA: f32 = 84.0;
/// O máximo é metade de uma tela de 1080: a tira é a navegação, a foto é o
/// assunto.
pub const ALTURA_MAXIMA: f32 = 420.0;
/// O padrão, o mesmo do site.
pub const ALTURA_PADRAO: f32 = 104.0;
/// O que sobra para a miniatura depois da linha de cima e das margens.
pub const ENFEITE_DA_TIRA: f32 = 34.0;
/// A proporção da miniatura da tira — paisagem, como no site.
pub const PROPORCAO: f32 = 1.35;

fn caminho(qual: &str) -> PathBuf {
    AppPaths::catalog_root().join(format!("tira-{qual}.txt"))
}

/// Limita ao que cabe — a mesma conta do arrasto e da leitura.
///
/// ⚠️ **Só o NaN volta ao padrão; o infinito é limitado.** É o que
/// `limitarAltura` do site faz, e a diferença importa: um `NaN` não tem lado
/// para o qual limitar (`clamp` o devolveria intacto, e a tira ficaria com
/// altura `NaN`, que não desenha), enquanto `+∞` quer dizer claramente "o
/// máximo". Tratar os dois como "não sei" faria um arrasto muito rápido
/// devolver a tira ao tamanho de fábrica em vez de encostá-la no teto.
pub fn limitar(valor: f32) -> f32 {
    if valor.is_nan() {
        return ALTURA_PADRAO;
    }
    valor.round().clamp(ALTURA_MINIMA, ALTURA_MAXIMA)
}

/// O lado da miniatura, dada a altura da tira.
pub fn lado_da_miniatura(altura: f32) -> f32 {
    (altura - ENFEITE_DA_TIRA).max(40.0)
}

/// A altura guardada, ou o padrão.
pub fn guardada(qual: &str) -> f32 {
    std::fs::read_to_string(caminho(qual))
        .ok()
        .and_then(|t| t.trim().parse::<f32>().ok())
        .map(limitar)
        .unwrap_or(ALTURA_PADRAO)
}

/// Grava. Não poder lembrar não pode impedir de arrastar.
pub fn guardar(qual: &str, altura: f32) {
    let destino = caminho(qual);
    if let Some(pasta) = destino.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    let _ = std::fs::write(destino, format!("{:.0}", limitar(altura)));
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_altura_fica_entre_o_minimo_e_o_maximo() {
        assert_eq!(limitar(10.0), ALTURA_MINIMA);
        assert_eq!(limitar(9_000.0), ALTURA_MAXIMA);
        assert_eq!(limitar(150.4), 150.0);
    }

    #[test]
    fn altura_invalida_volta_ao_padrao() {
        assert_eq!(limitar(f32::NAN), ALTURA_PADRAO);
        assert_eq!(limitar(f32::INFINITY), ALTURA_MAXIMA);
    }

    /// 🔑 A miniatura nunca some, mesmo na altura mínima.
    #[test]
    fn a_miniatura_tem_piso() {
        assert!(lado_da_miniatura(ALTURA_MINIMA) >= 40.0);
        assert!(lado_da_miniatura(ALTURA_MAXIMA) > lado_da_miniatura(ALTURA_MINIMA));
    }
}
