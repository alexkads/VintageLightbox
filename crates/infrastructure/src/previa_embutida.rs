//! A prévia que a câmera grava **dentro** do próprio JPEG.
//!
//! 🚨 **A janela de escolher as fotos do cartão decodificava a foto inteira para
//! desenhar uma célula** (dono, 27/set/2026, com o cartão da NIKON D3100 no
//! leitor: *"ainda não está muito legal"*). Cada miniatura custava ler 6 MB do
//! cartão e abrir 14 MP, em série — 48 fotos deixavam a janela 7 a 17 s cheia
//! de quadros vazios, e um cartão de 300 fotos, minutos.
//!
//! 🔑 **A câmera já fez esse trabalho.** A D3100 guarda, antes dos pixels, uma
//! prévia de 570×375 no MakerNote (e a de 160×120 do EXIF); Sony e Fuji gravam
//! a do MPF, maior ainda. Ela mora nos primeiros KB do arquivo: ler 256 KB do
//! cartão em vez de 6 MB, e decodificar 0,2 MP em vez de 14, é o que o
//! Lightroom chama de "prévias incorporadas" na importação.
//!
//! ⚠️ **Só serve para ver, nunca para revelar.** Quem importa continua lendo o
//! arquivo inteiro: a prévia é o que a câmera achou bom para a tela dela, com o
//! processamento dela.

use std::io::{Cursor, Read};
use std::path::Path;

use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};

/// Quanto do começo do arquivo se lê à procura da prévia.
///
/// O EXIF (onde mora o MakerNote) cabe num segmento APP1 de no máximo 64 KB, e
/// o MPF vem logo depois dele. 256 KB cobre os dois com folga, e ainda é 4% de
/// uma foto de 6 MB.
const CABECA: u64 = 256 * 1024;

/// A proporção da prévia pode diferir um pouco da foto — a D3100 grava
/// 570×375 (1,52) para 4608×3072 (1,50). Mais do que isto é outro
/// enquadramento (uma prévia 16:9 com tarjas, um recorte), e não serve.
const FOLGA_DA_PROPORCAO: f32 = 0.05;

/// A maior prévia embutida no JPEG em `caminho`, **de pé**, se o lado maior
/// dela tiver ao menos `lado_minimo`.
///
/// `None` quando o arquivo não é JPEG, não tem prévia do tamanho pedido, ou a
/// que tem não é a mesma foto — e aí quem chama decodifica o arquivo inteiro,
/// que é o que fazia antes.
pub fn da_camera(caminho: &Path, lado_minimo: u32) -> Option<DynamicImage> {
    let mut cabeca = Vec::new();
    std::fs::File::open(caminho)
        .ok()?
        .take(CABECA)
        .read_to_end(&mut cabeca)
        .ok()?;
    da_cabeca(&cabeca, lado_minimo)
}

/// O mesmo que [`da_camera`], sobre os primeiros bytes já lidos.
pub fn da_cabeca(cabeca: &[u8], lado_minimo: u32) -> Option<DynamicImage> {
    if !cabeca.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    // O cabeçalho da foto principal diz o tamanho e a orientação dela — a
    // prévia não tem EXIF próprio, e herda os dois.
    let mut principal = ImageReader::with_format(Cursor::new(cabeca), ImageFormat::Jpeg)
        .into_decoder()
        .ok()?;
    let orientacao = principal.orientation().unwrap_or(Orientation::NoTransforms);
    let (largura, altura) = principal.dimensions();
    if largura == 0 || altura == 0 {
        return None;
    }
    let proporcao = largura as f32 / altura as f32;

    // 🔑 Um `FF D8 FF` depois do começo só pode ser outro JPEG: no dado
    // comprimido todo `FF` vem seguido de `00` ou de um `RSTn`, nunca de `D8`.
    let mut melhor: Option<(usize, u32, u32)> = None;
    for inicio in
        (2..cabeca.len().saturating_sub(3)).filter(|&i| cabeca[i..i + 3] == [0xFF, 0xD8, 0xFF])
    {
        let Ok((l, a)) =
            ImageReader::with_format(Cursor::new(&cabeca[inicio..]), ImageFormat::Jpeg)
                .into_dimensions()
        else {
            continue;
        };
        if l.max(a) < lado_minimo || a == 0 {
            continue;
        }
        if ((l as f32 / a as f32) - proporcao).abs() > FOLGA_DA_PROPORCAO {
            continue;
        }
        if melhor.is_none_or(|(_, ml, ma)| l * a > ml * ma) {
            melhor = Some((inicio, l, a));
        }
    }

    let (inicio, _, _) = melhor?;
    // Uma prévia cortada pelos 256 KB não decodifica, e cai no caminho longo.
    let mut previa =
        image::load_from_memory_with_format(&cabeca[inicio..], ImageFormat::Jpeg).ok()?;
    previa.apply_orientation(orientacao);
    Some(previa)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn jpeg(largura: u32, altura: u32, cinza: u8) -> Vec<u8> {
        let imagem = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            largura,
            altura,
            image::Rgb([cinza, cinza, cinza]),
        ));
        let mut bytes = Vec::new();
        imagem
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .expect("codificar");
        bytes
    }

    fn segmento(marcador: u8, conteudo: &[u8]) -> Vec<u8> {
        let mut s = vec![0xFF, marcador];
        s.extend_from_slice(&((conteudo.len() + 2) as u16).to_be_bytes());
        s.extend_from_slice(conteudo);
        s
    }

    /// Um APP1 com um TIFF mínimo: `Orientation` e nada mais.
    fn exif_com_orientacao(valor: u16) -> Vec<u8> {
        let mut tiff = vec![b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0];
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&valor.to_le_bytes());
        tiff.extend_from_slice(&[0, 0]);
        tiff.extend_from_slice(&0u32.to_le_bytes());
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(&tiff);
        segmento(0xE1, &app1)
    }

    /// A foto da câmera: SOI, EXIF, as prévias num APP2 (como o MakerNote as
    /// carrega dentro do APP1) e depois a foto principal.
    fn da_camera_com(orientacao: u16, previas: &[Vec<u8>], principal: (u32, u32)) -> Vec<u8> {
        let mut foto = vec![0xFF, 0xD8];
        foto.extend(exif_com_orientacao(orientacao));
        for previa in previas {
            foto.extend(segmento(0xE2, previa));
        }
        foto.extend_from_slice(&jpeg(principal.0, principal.1, 200)[2..]);
        foto
    }

    #[test]
    fn acha_a_maior_previa_do_mesmo_enquadramento() {
        let foto = da_camera_com(1, &[jpeg(160, 120, 10), jpeg(600, 400, 90)], (1200, 800));

        let previa = da_cabeca(&foto, 400).expect("a prévia de 600 serve");

        assert_eq!((previa.width(), previa.height()), (600, 400));
        assert!(
            previa.to_rgb8().get_pixel(10, 10)[0] < 120,
            "é a prévia, e não a foto"
        );
    }

    /// 🚨 A foto em pé da D3100 é gravada deitada, com `Orientation = 6`; a
    /// prévia dentro dela também, e sem etiqueta própria.
    #[test]
    fn a_previa_da_foto_em_pe_sai_de_pe() {
        let foto = da_camera_com(6, &[jpeg(600, 400, 90)], (1200, 800));

        let previa = da_cabeca(&foto, 400).expect("abre");

        assert_eq!((previa.width(), previa.height()), (400, 600));
    }

    #[test]
    fn previa_pequena_demais_fica_para_o_caminho_longo() {
        let foto = da_camera_com(1, &[jpeg(160, 120, 10)], (1200, 800));

        assert!(da_cabeca(&foto, 400).is_none());
    }

    /// Uma prévia 16:9 de uma foto 3:2 é outro enquadramento.
    #[test]
    fn previa_de_outra_proporcao_nao_serve() {
        let foto = da_camera_com(1, &[jpeg(640, 360, 90)], (1200, 800));

        assert!(da_cabeca(&foto, 400).is_none());
    }

    #[test]
    fn o_que_nao_e_jpeg_nao_tem_previa() {
        assert!(da_cabeca(b"\x89PNG\r\n\x1a\n", 100).is_none());
    }
}
