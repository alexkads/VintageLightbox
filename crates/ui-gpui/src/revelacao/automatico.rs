//! O tom automático: ler a foto e escolher a exposição.
//!
//! É o "Auto" do painel Básico do Lightroom — e **não** um preset. Ele esteve na
//! lista de presets de sistema até 30/ago/2026 pedindo `exposure: Some(0.0)`,
//! que sobre o neutro `0.0` não mexe em nada: clicar não fazia efeito nenhum.
//! Preset é lista de números fixos, e nenhuma lista fixa serve para todas as
//! fotos. Este módulo olha a foto.
//!
//! ## O que ele mede
//!
//! Dois percentis do histograma da foto **crua** — `Aberta::bruta`, a imagem
//! como saiu do cache, antes de qualquer ajuste. Medir a foto que já está na
//! tela faria o segundo clique decidir sobre o resultado do primeiro, e o
//! automático andaria sozinho a cada toque.
//!
//! | Percentil | Para que serve |
//! |---|---|
//! | 50 (mediana) | onde está o meio-tom — é o que a exposição desloca |
//! | 99 | onde acabam as altas luzes — é o que não pode estourar |
//!
//! ## ⚠️ Por que ele mexe em dois ajustes, e não nos seis do Lightroom
//!
//! Porque a conta é a do shader, não a do Lightroom
//! (`image_adjustments.wgsl`), e ela é grossa: **"sombras" multiplica todo pixel
//! com luminância abaixo de 128**, não só as sombras. Numa foto lavada — a que
//! mais pede o automático — qualquer valor grande o bastante para levar o
//! primeiro percentil ao preto multiplicaria o meio-tom inteiro pelo mesmo
//! fator, e a foto sairia enterrada. "Brancos" e "pretos" têm portões ainda mais
//! estreitos (192 e 64): na foto lavada não há pixel nenhum dentro deles, e o
//! slider não faz nada.
//!
//! Sobram exposição — que é global, `pow(2, exposure)`, e faz exatamente o que
//! promete — e altas luzes, usada só para **tirar estouro**, nunca para clarear.
//!
//! 🔑 Escolher três números bonitos numa escala e entregá-los a uma conta que
//! não é aquela foi o defeito dos presets de sistema, consertado no mesmo dia em
//! que este módulo nasceu. Aqui os valores saem da conta que vai recebê-los.

use super::histograma::{Histograma, NIVEIS};

/// O que o tom automático decide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Escolha {
    pub exposure: f32,
    pub highlights: f32,
}

/// Onde a mediana deve ficar depois da exposição.
///
/// 118 e não 128: o meio-tom de uma foto bem exposta cai um pouco abaixo do meio
/// aritmético, e mirar no meio clareia demais o que já estava certo.
const ALVO_DO_MEIO_TOM: f32 = 118.0;

/// Acima disto a alta luz está no limite do arquivo, e o automático intervém.
const TETO_DAS_ALTAS: f32 = 250.0;

/// Para onde ele traz a alta luz estourada. Abaixo do teto, para o corte
/// aparecer.
const ALVO_DAS_ALTAS: f32 = 242.0;

/// Quantos pontos de exposição o automático pode pedir, para cada lado.
///
/// 🚨 **Sem este limite, foto quase preta pede `log2(118/1)`, quase 7 pontos** —
/// e sete pontos sobre ruído entregam ruído claro, não foto.
const EXPOSICAO_MAXIMA: f32 = 2.0;

/// O quanto ele pode fechar as altas luzes.
///
/// O portão do shader é 128: o que este valor multiplica é **toda** a metade
/// clara da foto, e não só o que estourou. -35 já recupera céu queimado; mais do
/// que isso escurece rosto para salvar nuvem.
const RECUO_MAXIMO_DAS_ALTAS: f32 = -35.0;

/// Lê o histograma da foto crua e escolhe.
///
/// A exposição é decidida primeiro, e as altas luzes **sobre o resultado dela** —
/// é a ordem do shader (exposição no bloco 1, altas luzes no 5). Decidir as duas
/// sobre a foto crua faria uma desmentir a outra: clarear dois pontos cria
/// estouro que a medição de antes não viu.
pub fn escolher(histograma: &Histograma) -> Escolha {
    let mediana = percentil(histograma, 0.50).max(1.0);
    let exposure = (ALVO_DO_MEIO_TOM / mediana)
        .log2()
        .clamp(-EXPOSICAO_MAXIMA, EXPOSICAO_MAXIMA);

    let altas = percentil(histograma, 0.99) * exposure.exp2();

    // `1 + valor*0.01` é o multiplicador do shader: para levar `altas` ao alvo, o
    // valor é `(alvo/altas - 1) * 100`. Só desce — o automático não inventa
    // detalhe onde a foto não tem.
    let highlights = if altas > TETO_DAS_ALTAS {
        ((ALVO_DAS_ALTAS / altas - 1.0) * 100.0).clamp(RECUO_MAXIMO_DAS_ALTAS, 0.0)
    } else {
        0.0
    };

    Escolha {
        // Duas casas na exposição e nenhuma nas altas luzes: são as casas que o
        // painel mostra (`CONTROLES`), e um valor que a barra não sabe exibir
        // vira um slider parado num número que não é o dele.
        exposure: (exposure * 100.0).round() / 100.0,
        highlights: highlights.round(),
    }
}

/// O nível abaixo do qual está a fração pedida dos pixels.
///
/// Os três canais somados: o histograma guarda um balde por canal, e o que
/// interessa aqui é como a foto se distribui entre claro e escuro — não qual cor
/// está onde.
fn percentil(histograma: &Histograma, fracao: f32) -> f32 {
    let contagem = |nivel: usize| {
        u64::from(histograma.vermelho[nivel])
            + u64::from(histograma.verde[nivel])
            + u64::from(histograma.azul[nivel])
    };

    let total: u64 = (0..NIVEIS).map(contagem).sum();
    if total == 0 {
        return 0.0;
    }

    let alvo = ((total as f64) * f64::from(fracao)).ceil().max(1.0) as u64;
    let mut acumulado = 0u64;

    for nivel in 0..NIVEIS {
        acumulado += contagem(nivel);
        if acumulado >= alvo {
            return nivel as f32;
        }
    }

    (NIVEIS - 1) as f32
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Um histograma com os pixels distribuídos pelos níveis pedidos, iguais nos
    /// três canais — cinza, que é o que interessa para medir tom.
    fn histograma(niveis: &[(usize, u32)]) -> Histograma {
        let mut canal = [0u32; NIVEIS];
        for &(nivel, quantos) in niveis {
            canal[nivel] += quantos;
        }
        Histograma {
            vermelho: canal,
            verde: canal,
            azul: canal,
            maximo: canal.iter().copied().max().unwrap_or(0).max(1),
        }
    }

    #[test]
    fn a_foto_ja_equilibrada_nao_e_mexida() {
        let escolha = escolher(&histograma(&[(118, 1_000)]));

        assert_eq!(escolha.exposure, 0.0, "mediana no alvo, exposição parada");
        assert_eq!(escolha.highlights, 0.0);
    }

    /// 🚨 Clicar duas vezes tem de dar o mesmo resultado.
    ///
    /// É por isso que a medida sai de `Aberta::bruta` e não do que está na tela:
    /// medindo o resultado do próprio ajuste, o segundo clique clarearia de novo
    /// uma foto que o primeiro já pôs no lugar.
    #[test]
    fn escolher_duas_vezes_do_mesmo_histograma_da_o_mesmo() {
        let escuro = histograma(&[(30, 1_000)]);

        assert_eq!(escolher(&escuro), escolher(&escuro));
    }

    #[test]
    fn a_foto_escura_ganha_exposicao() {
        let escolha = escolher(&histograma(&[(30, 1_000)]));

        // log2(118/30) = 1,98
        assert_eq!(escolha.exposure, 1.98);
    }

    #[test]
    fn a_foto_clara_perde_exposicao() {
        let escolha = escolher(&histograma(&[(200, 1_000)]));

        // log2(118/200) = -0,76
        assert_eq!(escolha.exposure, -0.76);
    }

    /// 🚨 Sete pontos de exposição sobre ruído entregam ruído claro.
    #[test]
    fn a_foto_quase_preta_para_no_limite() {
        let escolha = escolher(&histograma(&[(0, 1_000)]));

        assert_eq!(escolha.exposure, EXPOSICAO_MAXIMA);
    }

    #[test]
    fn o_histograma_vazio_nao_vira_nan() {
        let escolha = escolher(&histograma(&[]));

        assert!(escolha.exposure.is_finite() && escolha.highlights.is_finite());
    }

    /// O céu queimado desce; e desce **por causa da exposição**, que é o que a
    /// ordem das duas decisões existe para pegar.
    #[test]
    fn a_alta_luz_que_a_exposicao_vai_estourar_desce_antes() {
        // Meio-tom escuro (a exposição vai subir ~1 ponto) e 2% de céu em 200:
        // 200 × 2 passa de 250, e nada disso aparece medindo só a foto crua.
        let escolha = escolher(&histograma(&[(59, 980), (200, 20)]));

        assert!(escolha.exposure > 0.9, "a foto escura pede exposição");
        assert!(
            escolha.highlights < 0.0,
            "o céu ia para {} — as altas luzes tinham de recuar",
            200.0 * escolha.exposure.exp2()
        );
    }

    /// ⚠️ E o recuo tem teto: o portão do shader é 128, então este valor
    /// multiplica a metade clara inteira, não só o que estourou.
    #[test]
    fn o_recuo_das_altas_luzes_tem_teto() {
        let escolha = escolher(&histograma(&[(1, 950), (255, 50)]));

        assert_eq!(escolha.highlights, RECUO_MAXIMO_DAS_ALTAS);
    }

    #[test]
    fn a_foto_sem_estouro_nao_mexe_nas_altas_luzes() {
        let escolha = escolher(&histograma(&[(118, 990), (200, 10)]));

        assert_eq!(escolha.highlights, 0.0);
    }
}
