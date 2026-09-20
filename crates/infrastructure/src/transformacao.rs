//! Do pixel revelado ao pixel exibido, visto daqui: a leitura da entidade e a
//! ponte entre `CropSettings` (do `domain`) e [`Corte`] (do `revelacao-core`).
//!
//! 🔑 **A matemática saiu deste crate em 2026-09-04** para
//! [`revelacao_core::transformacao`], junto com o motor, para o navegador
//! enquadrar a foto pelo mesmo código que o desktop. O que ficou aqui é o que
//! só este crate pode fazer: ler a entidade e converter o tipo do `domain`. A
//! ordem (espelhos → giro de 90° → endireitamento → recorte), a folga do caminho
//! rápido e os testes de geometria estão lá.

use image::DynamicImage;

use domain::entities::Photo;
use domain::value_objects::CropSettings;
pub use revelacao_core::Corte;

/// O enquadramento gravado na foto, ou a foto inteira quando não há nenhum.
///
/// 🔑 **Mora aqui, junto de quem o aplica, e não em cada chamador.** A tela e a
/// exportação precisam do **mesmo** retângulo: dois lugares lendo os oito campos
/// com oito `unwrap_or` cada é a forma conhecida de um deles ficar com o padrão
/// errado — e o sintoma seria o arquivo exportado com um enquadramento que a
/// tela nunca mostrou.
pub fn corte_da_entidade(foto: &Photo) -> CropSettings {
    CropSettings::new(
        foto.edit_crop_x().unwrap_or(0.0),
        foto.edit_crop_y().unwrap_or(0.0),
        foto.edit_crop_width().unwrap_or(1.0),
        foto.edit_crop_height().unwrap_or(1.0),
        foto.edit_crop_rotation().unwrap_or(0),
        foto.edit_crop_angle().unwrap_or(0.0),
        foto.edit_crop_flip_h().unwrap_or(false),
        foto.edit_crop_flip_v().unwrap_or(false),
    )
}

/// O `CropSettings` do `domain` como o [`Corte`] do motor: os mesmos oito
/// campos, e o mesmo `clamp` dos dois lados — a conversão não move nada.
pub fn corte(settings: &CropSettings) -> Corte {
    Corte::novo(
        settings.crop_x(),
        settings.crop_y(),
        settings.crop_width(),
        settings.crop_height(),
        settings.rotation_90(),
        settings.angle(),
        settings.flip_horizontal(),
        settings.flip_vertical(),
    )
}

/// A foto pronta para a tela — ver [`revelacao_core::transformacao::aplicar`].
///
/// `recortar` é `false` no modo de corte: lá a foto aparece inteira (girada e
/// endireitada) com o retângulo desenhado por cima, senão não haveria o que
/// arrastar.
pub fn aplicar(imagem: &DynamicImage, settings: &CropSettings, recortar: bool) -> DynamicImage {
    revelacao_core::transformacao::aplicar(imagem, &corte(settings), recortar)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// 🔑 A conversão preserva os oito campos — inclusive depois do `clamp`.
    ///
    /// Os dois tipos limitam os valores na construção; se um deles limitasse
    /// diferente, a tela (que passa por `CropSettings`) e o navegador (que
    /// constrói `Corte` direto) enquadrariam a mesma foto de dois jeitos.
    #[test]
    fn a_conversao_nao_move_nenhum_campo() {
        let settings = CropSettings::new(0.2, 0.1, 0.9, 0.5, 1, 12.5, true, false);
        let c = corte(&settings);
        assert_eq!(c.x(), settings.crop_x());
        assert_eq!(c.y(), settings.crop_y());
        assert_eq!(c.largura(), settings.crop_width());
        assert_eq!(c.altura(), settings.crop_height());
        assert_eq!(c.giro_90(), settings.rotation_90());
        assert_eq!(c.angulo(), settings.angle());
        assert_eq!(c.espelho_h(), settings.flip_horizontal());
        assert_eq!(c.espelho_v(), settings.flip_vertical());

        // Fora da faixa: os dois lados limitam igual.
        let estourado = CropSettings::new(-1.0, 2.0, 5.0, 0.0, 7, 90.0, false, true);
        let c = corte(&estourado);
        assert_eq!(c.x(), estourado.crop_x());
        assert_eq!(c.largura(), estourado.crop_width());
        assert_eq!(c.altura(), estourado.crop_height());
        assert_eq!(c.giro_90(), estourado.rotation_90());
        assert_eq!(c.angulo(), estourado.angle());
    }

    /// A foto atravessa igual pelo caminho do `domain`: é o que `ui-gpui` e o
    /// exportador chamam, e o que os testes de geometria do core já provam por
    /// dentro.
    #[test]
    fn sem_nada_a_foto_atravessa_igual() {
        let mut img = image::RgbaImage::new(8, 8);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([(x * 30) as u8, (y * 30) as u8, 7, 255]);
        }
        let entrada = DynamicImage::ImageRgba8(img);
        let inteiro = CropSettings::new(0.0, 0.0, 1.0, 1.0, 0, 0.0, false, false);
        let saida = aplicar(&entrada, &inteiro, true);
        assert_eq!(saida.to_rgba8().into_raw(), entrada.to_rgba8().into_raw());

        let metade = CropSettings::new(0.5, 0.0, 0.5, 0.5, 0, 0.0, false, false);
        assert_eq!(aplicar(&entrada, &metade, true).width(), 4);
    }
}
