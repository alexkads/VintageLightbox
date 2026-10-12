//! O histograma: quantos pixels há em cada nível, por canal.
//!
//! É o instrumento que diz o que o olho não diz — se as altas luzes estouraram,
//! se as sombras entupiram. Numa tela de revelação ele vale mais que qualquer
//! slider, porque é a única coisa ali que não é opinião.
//!
//! ## ⚠️ Ele mede a foto **como ela está na tela**
//!
//! O legado calcula a partir da imagem já processada (`async_loader.rs`, depois
//! do `process_image`), e é o que faz sentido: um histograma da foto crua, com os
//! sliders todos mexidos, mediria uma imagem que ninguém está vendo. Aqui vale o
//! mesmo — ele é recalculado do resultado do shader.

use image::DynamicImage;

/// Quantos níveis o histograma tem. Um por valor possível de um canal de 8 bits.
pub const NIVEIS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Histograma {
    pub vermelho: [u32; NIVEIS],
    pub verde: [u32; NIVEIS],
    pub azul: [u32; NIVEIS],
    /// O maior valor entre os três canais — é por ele que a altura é normalizada.
    ///
    /// 🚨 **Nunca zero.** Numa imagem de lado zero (que não deveria existir, mas
    /// o cache já entregou coisa estranha antes) todos os baldes ficam vazios, e
    /// dividir por ele desenharia `NaN` de altura — que o GPUI aceita e não
    /// desenha, deixando um retângulo vazio sem explicação.
    pub maximo: u32,
}

/// Quantos pixels bastam para o desenho não mudar.
///
/// 🚨 **Isto roda uma vez por quadro enquanto o slider está sendo arrastado**, e
/// varrer os 4,4 Mpx de um preview de 2560px custava 4,46 ms — mais de um quarto
/// do orçamento de 16,7 ms, gasto num gráfico de 256 barras (medido em
/// 8/set/2026, `medir-revelacao`).
///
/// 🔑 **Um histograma é uma distribuição, e distribuição se estima.** Com 250 mil
/// amostras espalhadas pela imagem, cada uma das 256 barras recebe ~1.000 —
/// muito além do que 200 pixels de altura conseguem distinguir. O desenho é o
/// mesmo; o custo cai com o passo.
///
/// ⚠️ **O passo é sobre o total, não fixo**: numa imagem pequena ele vira 1 e
/// todos os pixels entram, que é o que os testes cobram e o que mantém a conta
/// exata onde ela é barata.
const AMOSTRAS: usize = 250_000;

/// A partir de que parte da foto num extremo o triângulo acende: 0,1 %.
/// Menos que isso é um reflexo ou um pixel de ruído, e não um estouro.
const ESTOURO: f32 = 0.001;

impl Histograma {
    pub fn da_imagem(imagem: &DynamicImage) -> Self {
        let mut vermelho = [0u32; NIVEIS];
        let mut verde = [0u32; NIVEIS];
        let mut azul = [0u32; NIVEIS];

        // 🔑 **`as_rgb8` antes de `to_rgb8`.** O resultado do shader já chega em
        // Rgb8, e `to_rgb8` num Rgb8 **clona os 13 MB** para devolver o que já
        // estava ali. Emprestar é o caminho normal; converter é a exceção.
        let convertida;
        let rgb = match imagem.as_rgb8() {
            Some(rgb) => rgb,
            None => {
                convertida = imagem.to_rgb8();
                &convertida
            }
        };

        // O alfa não entra em histograma de exposição — daí três canais, e não
        // quatro.
        let passo = (rgb.pixels().len() / AMOSTRAS).max(1);
        for pixel in rgb.pixels().step_by(passo) {
            vermelho[pixel[0] as usize] += 1;
            verde[pixel[1] as usize] += 1;
            azul[pixel[2] as usize] += 1;
        }

        let maximo = vermelho
            .iter()
            .chain(verde.iter())
            .chain(azul.iter())
            .copied()
            .max()
            .unwrap_or(0)
            .max(1);

        Self {
            vermelho,
            verde,
            azul,
            maximo,
        }
    }

    /// As alturas **para o desenho**, de 0 a 1, com o teto medido sem os dois
    /// extremos e um alisamento de três níveis.
    ///
    /// ⚠️ **O teto ignora o nível 0 e o 255.** Brancos estourados (ou um fundo
    /// preto) empilham num nível só, e normalizado por ele o resto do
    /// histograma virava uma linha rente ao chão. O Lightroom faz o mesmo: o
    /// pico da ponta é cortado no teto, e quem avisa do estouro é o triângulo
    /// do canto ([`Self::estouro_nas_sombras`], [`Self::estouro_nas_luzes`]).
    ///
    /// O alisamento tira o "pente" que a revelação em 8 bits deixa (níveis
    /// vazios entre níveis cheios depois de esticar o contraste).
    pub fn alturas_do_desenho(&self) -> Vec<(f32, f32, f32)> {
        let miolo = |canal: &[u32; NIVEIS]| canal[1..NIVEIS - 1].iter().copied().max().unwrap_or(0);
        let teto = miolo(&self.vermelho)
            .max(miolo(&self.verde))
            .max(miolo(&self.azul));
        let teto = if teto > 0 { teto } else { self.maximo } as f32;
        let alisado = |canal: &[u32; NIVEIS], i: usize| {
            let antes = canal[i.saturating_sub(1)] as f32;
            let depois = canal[(i + 1).min(NIVEIS - 1)] as f32;
            // Os extremos não se alisam: o estouro é um nível só, e espalhá-lo
            // o esconderia.
            let valor = if i == 0 || i == NIVEIS - 1 {
                canal[i] as f32
            } else {
                (antes + 2.0 * canal[i] as f32 + depois) / 4.0
            };
            (valor / teto).min(1.0)
        };
        (0..NIVEIS)
            .map(|i| {
                (
                    alisado(&self.vermelho, i),
                    alisado(&self.verde, i),
                    alisado(&self.azul, i),
                )
            })
            .collect()
    }

    /// A parte das amostras que algum canal tem num nível.
    fn parte_no_nivel(&self, nivel: usize) -> f32 {
        let total: u32 = self.vermelho.iter().sum();
        if total == 0 {
            return 0.0;
        }
        let la = self.vermelho[nivel]
            .max(self.verde[nivel])
            .max(self.azul[nivel]);
        la as f32 / total as f32
    }

    /// Há sombra entupida — algum canal com mais de [`ESTOURO`] da foto no
    /// preto puro? É o triângulo da esquerda do Lightroom.
    pub fn estouro_nas_sombras(&self) -> bool {
        self.parte_no_nivel(0) > ESTOURO
    }

    /// Há luz estourada — algum canal com mais de [`ESTOURO`] da foto no
    /// branco puro? É o triângulo da direita.
    pub fn estouro_nas_luzes(&self) -> bool {
        self.parte_no_nivel(NIVEIS - 1) > ESTOURO
    }

    /// A altura de cada nível, de 0 a 1, canal a canal.
    pub fn alturas(&self) -> impl Iterator<Item = (f32, f32, f32)> + '_ {
        (0..NIVEIS).map(move |i| {
            let maximo = self.maximo as f32;
            (
                self.vermelho[i] as f32 / maximo,
                self.verde[i] as f32 / maximo,
                self.azul[i] as f32 / maximo,
            )
        })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use image::{Rgba, RgbaImage};

    fn chapada(lado: u32, cor: [u8; 4]) -> DynamicImage {
        let mut img = RgbaImage::new(lado, lado);
        for pixel in img.pixels_mut() {
            *pixel = Rgba(cor);
        }
        DynamicImage::ImageRgba8(img)
    }

    /// Uma cor chapada põe todos os pixels num balde só, por canal.
    #[test]
    fn cor_chapada_enche_um_balde_por_canal() {
        let histograma = Histograma::da_imagem(&chapada(4, [10, 200, 255, 255]));

        assert_eq!(histograma.vermelho[10], 16);
        assert_eq!(histograma.verde[200], 16);
        assert_eq!(histograma.azul[255], 16);
        assert_eq!(histograma.maximo, 16);
        assert_eq!(
            histograma.vermelho[200], 0,
            "o balde do outro canal fica vazio"
        );
    }

    /// 🚨 O branco estourado não achata o resto: o teto do desenho é medido
    /// sem os extremos, o pico da ponta é cortado em 1 e o triângulo acende.
    #[test]
    fn o_estouro_nao_achata_o_desenho_e_acende_o_triangulo() {
        // 3/4 da foto no branco puro, 1/4 num cinza médio.
        let mut img = RgbaImage::new(4, 4);
        for (i, pixel) in img.pixels_mut().enumerate() {
            *pixel = if i < 12 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([120, 120, 120, 255])
            };
        }
        let histograma = Histograma::da_imagem(&DynamicImage::ImageRgba8(img));
        let desenho = histograma.alturas_do_desenho();

        assert_eq!(desenho.len(), NIVEIS);
        assert_eq!(desenho[255].0, 1.0, "o pico da ponta é cortado no teto");
        assert!(
            desenho[120].0 >= 0.5,
            "o cinza médio continua visível: {}",
            desenho[120].0
        );
        assert!(histograma.estouro_nas_luzes());
        assert!(!histograma.estouro_nas_sombras());
    }

    /// Uma foto sem nada nos extremos não acende triângulo nenhum.
    #[test]
    fn sem_extremo_nao_ha_estouro() {
        let histograma = Histograma::da_imagem(&chapada(4, [10, 200, 250, 255]));
        assert!(!histograma.estouro_nas_sombras());
        assert!(!histograma.estouro_nas_luzes());
    }

    /// 🚨 A altura é normalizada pelo **maior dos três canais**, e não por canal.
    ///
    /// Normalizar cada canal pelo próprio máximo faria uma foto azul-escura
    /// desenhar o vermelho tão alto quanto o azul — e o histograma passaria a
    /// mentir exatamente sobre o que ele existe para mostrar: qual canal domina.
    #[test]
    fn a_altura_e_normalizada_pelo_maior_dos_canais() {
        // Metade preta, metade vermelha: o nível 0 tem o dobro de verde/azul.
        let mut img = RgbaImage::new(4, 4);
        for (i, pixel) in img.pixels_mut().enumerate() {
            *pixel = if i < 8 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 0, 255])
            };
        }
        let histograma = Histograma::da_imagem(&DynamicImage::ImageRgba8(img));

        assert_eq!(
            histograma.maximo, 16,
            "verde e azul têm 16 pixels no nível 0"
        );

        let alturas: Vec<_> = histograma.alturas().collect();
        assert_eq!(alturas[0].0, 0.5, "o vermelho no nível 0 é metade");
        assert_eq!(alturas[0].1, 1.0, "o verde no nível 0 é o máximo");
        assert_eq!(alturas[255].0, 0.5, "e no 255 o vermelho é a outra metade");
    }

    /// ⚠️ Imagem vazia não gera divisão por zero.
    ///
    /// `maximo` mínimo é 1. Zero ali daria `NaN` de altura — que o GPUI aceita
    /// sem reclamar e simplesmente não desenha, deixando um retângulo vazio que
    /// ninguém saberia explicar.
    #[test]
    fn imagem_sem_pixel_nao_divide_por_zero() {
        let vazia = DynamicImage::ImageRgba8(RgbaImage::new(0, 0));
        let histograma = Histograma::da_imagem(&vazia);

        assert_eq!(histograma.maximo, 1);
        for (r, g, b) in histograma.alturas() {
            assert!(r.is_finite() && g.is_finite() && b.is_finite());
        }
    }

    /// São 256 níveis, nem um a mais.
    #[test]
    fn sao_duzentos_e_cinquenta_e_seis_niveis() {
        let histograma = Histograma::da_imagem(&chapada(2, [0, 0, 0, 255]));
        assert_eq!(histograma.alturas().count(), NIVEIS);
    }

    /// A soma de cada canal é o número de pixels — nenhum se perde no caminho.
    #[test]
    fn nenhum_pixel_se_perde() {
        let mut img = RgbaImage::new(8, 8);
        for (i, pixel) in img.pixels_mut().enumerate() {
            let v = (i * 4) as u8;
            *pixel = Rgba([v, 255 - v, v / 2, 255]);
        }
        let histograma = Histograma::da_imagem(&DynamicImage::ImageRgba8(img));

        for canal in [&histograma.vermelho, &histograma.verde, &histograma.azul] {
            assert_eq!(canal.iter().sum::<u32>(), 64);
        }
    }
}
