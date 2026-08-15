//! A ponte entre o `image` e o GPUI.
//!
//! É a fronteira técnica nº 1 da migração (docs/10-MIGRACAO-GPUI.md §3.1), e a
//! única sem a qual a Biblioteca não mostra miniatura nenhuma.
//!
//! O caminho fica **mais curto** que o de hoje:
//!
//! ```text
//! egui:  DynamicImage → ColorImage → egui::TextureHandle
//! gpui:  DynamicImage → RgbaImage → Frame → RenderImage
//! ```
//!
//! Some a conversão para `ColorImage`, que era cópia pura de bytes.

use std::sync::Arc;

use gpui::RenderImage;
// `Frame` vem do **crate `image`**, e não do `gpui`: lá ele é reexportado de
// forma privada. É a mesma peça que a `infrastructure` já usa, e é por isso que
// as duas versões do `image` precisam ser a mesma na árvore (§3.2).
use image::{DynamicImage, Frame};
use smallvec::SmallVec;

/// Converte a imagem decodificada no que o GPUI desenha.
///
/// # 🚨 O GPUI quer BGRA, e o crate `image` produz RGBA
///
/// `RenderImage` é documentado como *"a cached and processed image, in BGRA
/// format"*, e o renderizador Metal cria as texturas com `BGRA8Unorm`. O
/// `image::Frame`, por outro lado, carrega um `RgbaImage` — o tipo **não** diz
/// qual das duas ordens está lá dentro.
///
/// Entregar RGBA a quem espera BGRA não falha, não avisa e não quebra: troca
/// vermelho por azul em **toda** foto. Num programa de revelação, esse é o pior
/// desfecho possível — a imagem aparece, parece uma decisão de cor, e quem
/// olha conclui que o motor de cor está errado.
///
/// A troca aqui é a mesma que o próprio GPUI faz ao carregar imagem da área de
/// transferência (`platform.rs`, `frames_for_image`): `into_rgba8()` e depois
/// `swap(0, 2)` em cada pixel. Copiar a regra de quem já fala com essa API é
/// mais barato do que descobri-la olhando a foto ficar azul.
///
/// # Uma cópia, e não duas
///
/// `into_rgba8()` consome o `DynamicImage` e reaproveita o buffer quando ele já
/// é RGBA8 — que é o caso de toda miniatura vinda do cache, gravada em JPEG e
/// decodificada aqui. A troca de canais é feita **no lugar**, sem alocar de
/// novo: numa grade de 2.000 fotos, uma alocação a mais por miniatura é a
/// diferença entre rolar liso e engasgar.
pub fn para_gpui(imagem: DynamicImage) -> Arc<RenderImage> {
    let mut bytes = imagem.into_rgba8();

    // `chunks_exact_mut(4)` e não um laço por (x, y): o buffer é contíguo, e o
    // acesso por coordenada refaria a multiplicação a cada pixel.
    for pixel in bytes.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    Arc::new(RenderImage::new(SmallVec::from_elem(Frame::new(bytes), 1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::{Rgba, RgbaImage};

    /// Uma imagem 1×1 da cor pedida, em RGBA.
    fn um_pixel(r: u8, g: u8, b: u8, a: u8) -> DynamicImage {
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([r, g, b, a]));
        DynamicImage::ImageRgba8(img)
    }

    /// Os bytes que o GPUI vai mandar para a GPU.
    fn bytes_de(imagem: DynamicImage) -> Vec<u8> {
        let render = para_gpui(imagem);
        render
            .as_bytes(0)
            .expect("o quadro 0 tem de existir")
            .to_vec()
    }

    #[test]
    fn vermelho_puro_chega_como_bgra() {
        // Se este teste falhar com [255, 0, 0, 255], a troca sumiu — e é o
        // defeito que faz toda foto do acervo ficar azul.
        let bytes = bytes_de(um_pixel(255, 0, 0, 255));

        assert_eq!(
            bytes,
            vec![0, 0, 255, 255],
            "vermelho em RGBA vira B=0 G=0 R=255 em BGRA"
        );
    }

    #[test]
    fn azul_puro_chega_como_bgra() {
        let bytes = bytes_de(um_pixel(0, 0, 255, 255));

        assert_eq!(bytes, vec![255, 0, 0, 255]);
    }

    #[test]
    fn verde_e_alfa_nao_sao_tocados() {
        // A troca é só entre os canais 0 e 2. Verde no meio e alfa no fim
        // ficam onde estão — trocar quatro canais em vez de dois é o erro
        // gêmeo, e ele escurece a imagem em vez de mudar a cor.
        let bytes = bytes_de(um_pixel(10, 200, 30, 128));

        assert_eq!(bytes, vec![30, 200, 10, 128]);
    }

    #[test]
    fn imagem_rgb_sem_alfa_ganha_alfa_opaco() {
        // JPEG não tem alfa, e é o formato de toda miniatura do cache. O
        // `into_rgba8` preenche com 255 — se preenchesse com 0, a grade
        // inteira ficaria invisível.
        let rgb =
            DynamicImage::ImageRgb8(image::RgbImage::from_pixel(1, 1, image::Rgb([255, 0, 0])));

        assert_eq!(bytes_de(rgb), vec![0, 0, 255, 255]);
    }

    #[test]
    fn o_tamanho_sobrevive_a_conversao() {
        let render = para_gpui(DynamicImage::ImageRgba8(RgbaImage::new(320, 240)));
        let tamanho = render.size(0);

        assert_eq!(u32::from(tamanho.width), 320);
        assert_eq!(u32::from(tamanho.height), 240);
    }

    #[test]
    fn cada_imagem_tem_identidade_propria() {
        // O `RenderImage` compara por `id`, e é por ele que o GPUI decide se
        // pode reaproveitar a textura já enviada à GPU. Dois ids iguais para
        // fotos diferentes mostrariam a foto errada na grade.
        let a = para_gpui(um_pixel(1, 2, 3, 255));
        let b = para_gpui(um_pixel(1, 2, 3, 255));

        assert_ne!(a.id, b.id);
    }
}
