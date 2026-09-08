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

impl Histograma {
    pub fn da_imagem(imagem: &DynamicImage) -> Self {
        let mut vermelho = [0u32; NIVEIS];
        let mut verde = [0u32; NIVEIS];
        let mut azul = [0u32; NIVEIS];

        // `to_rgb8` e não `to_rgba8`: o alfa não entra em histograma de
        // exposição, e converter para três canais evita percorrer um quarto de
        // bytes à toa numa foto de 24 megapixels.
        for pixel in imagem.to_rgb8().pixels() {
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
