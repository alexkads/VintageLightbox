//! Como a grade da Biblioteca se organiza — **as contas moram no
//! `biblioteca-core`**, e este módulo só as apresenta ao GPUI.
//!
//! # O que mudou em 2026-09-05, e por quê
//!
//! Estas três funções eram escritas aqui e, com outros nomes e a mesma
//! finalidade, em `grade-layout.ts` no site — uma para a grade do painel do
//! pós-venda e a mesma para a galeria do cliente. Três cópias da mesma
//! aritmética, e elas **já tinham divergido**: a daqui cobrava respiro da última
//! coluna, e por isso mostrava uma coluna a menos em certas larguras de janela.
//!
//! É a armadilha nº 8 do projeto (duas listas para a mesma decisão) na forma que
//! menos aparece: nada quebra, a tela só fica um pouco pior de um lado só.
//!
//! ⚠️ **A correção muda o que se vê aqui**: numa janela de 999 px úteis, onde
//! cabiam 4 colunas de 180 + 8, agora cabem 5 — que é o número certo
//! (5 × 180 + 4 × 8 = 932). Nenhuma foto some nem se repete; a grade fica mais
//! cheia, e o teste que cobrava o número antigo virou o que cobra o novo.
//!
//! O `uniform_list` do GPUI continua virtualizando **linhas**: ele pergunta
//! "renderize os itens de 40 a 55" e só esses existem. As contas de "quantas
//! cabem" e "quais vão nesta linha" continuam sendo o que decide se 2.000 fotos
//! rolam a 60fps ou engasgam — só que agora são as mesmas do site.

pub use biblioteca_core::grade::{fotos_da_linha, linhas_necessarias};

/// Quantas fotos cabem numa linha.
///
/// ⚠️ **A assinatura mudou junto com a conta.** Antes recebia
/// `largura_do_item` já com o espaçamento somado, e era essa soma que escondia
/// o erro: com o respiro embutido, não havia como saber que a última coluna não
/// o usa. Agora o lado e o respiro entram separados, como no core.
pub fn colunas_que_cabem(largura_disponivel: f32, lado_do_item: f32, espacamento: f32) -> usize {
    biblioteca_core::grade::colunas_que_cabem(largura_disponivel, lado_do_item, espacamento)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O que o core garante está provado lá, com 67 testes. Aqui fica só o que
    /// é **desta** ponte: que a assinatura nova chega ao lugar certo, e que o
    /// caso de janela estreita — o que fazia a grade sumir — continua tratado.
    #[test]
    fn a_ponte_entrega_a_conta_do_core() {
        // 800 px, itens de 192 com 8 de respiro: cabem 4 (4×192 + 3×8 = 792).
        assert_eq!(colunas_que_cabem(800.0, 192.0, 8.0), 4);
    }

    #[test]
    fn a_ultima_coluna_nao_paga_respiro() {
        // 🚨 A conta antiga daqui respondia 4; o certo é 5.
        assert_eq!(colunas_que_cabem(999.0, 180.0, 8.0), 5);
    }

    #[test]
    fn janela_estreita_ainda_mostra_uma_coluna() {
        // O caso de arrastar a borda da janela até quase fechar. Zero colunas
        // faria `total / colunas` dividir por zero e a grade sumir.
        assert_eq!(colunas_que_cabem(50.0, 180.0, 8.0), 1);
        assert_eq!(colunas_que_cabem(0.0, 180.0, 8.0), 1);
    }

    #[test]
    fn largura_absurda_nao_derruba_a_conta() {
        // `f32::NAN` chega aqui quando o layout ainda não mediu o contêiner —
        // acontece no primeiro quadro, e `NAN as usize` é 0 em Rust.
        assert_eq!(colunas_que_cabem(f32::NAN, 180.0, 8.0), 1);
        assert_eq!(colunas_que_cabem(f32::INFINITY, 180.0, 8.0), 1);
        assert_eq!(colunas_que_cabem(800.0, 0.0, 8.0), 1);
    }
}
