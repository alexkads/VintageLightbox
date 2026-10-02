//! Lado a lado com legenda escrita em cada quadro, para quem olha saber quem é
//! quem sem ler o nome do arquivo (dono, 2/out/2026: "não estou conseguindo
//! entender quem é quem").
//!
//! A fonte é a do sistema (Segoe UI/Arial no Windows, Helvetica/Arial no Mac,
//! DejaVu no Linux). Sem nenhuma delas, o quadro sai sem legenda — a imagem
//! continua valendo.

#![allow(dead_code)]

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use image::{Rgb, RgbImage};

const FONTES: [&str; 6] = [
    "C:/Windows/Fonts/segoeuib.ttf",
    "C:/Windows/Fonts/arialbd.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "/Library/Fonts/Arial Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
];

fn fonte() -> Option<FontVec> {
    FONTES
        .iter()
        .find_map(|c| FontVec::try_from_vec(std::fs::read(c).ok()?).ok())
}

/// Escreve `texto` em `img` a partir de (x, y), na altura `px`, na cor dada.
pub fn escrever(img: &mut RgbImage, texto: &str, x: f32, y: f32, px: f32, cor: [u8; 3]) {
    let Some(fonte) = fonte() else {
        return;
    };
    let escala = fonte.as_scaled(PxScale::from(px));
    let mut caneta = x;
    let base = y + escala.ascent();
    let mut anterior = None;
    for c in texto.chars() {
        let id = escala.glyph_id(c);
        if let Some(a) = anterior {
            caneta += escala.kern(a, id);
        }
        let glifo = id.with_scale_and_position(PxScale::from(px), ab_glyph::point(caneta, base));
        caneta += escala.h_advance(id);
        anterior = Some(id);
        if let Some(contorno) = fonte.outline_glyph(glifo) {
            let caixa = contorno.px_bounds();
            contorno.draw(|gx, gy, cobertura| {
                let (px_x, px_y) = (
                    caixa.min.x as i32 + gx as i32,
                    caixa.min.y as i32 + gy as i32,
                );
                if px_x < 0 || px_y < 0 || px_x >= img.width() as i32 || px_y >= img.height() as i32
                {
                    return;
                }
                let p = img.get_pixel_mut(px_x as u32, px_y as u32);
                for i in 0..3 {
                    p[i] = (p[i] as f32 * (1.0 - cobertura) + cor[i] as f32 * cobertura) as u8;
                }
            });
        }
    }
}

/// Os quadros lado a lado, com uma faixa branca em cima e a legenda de cada um.
/// O primeiro rótulo sai em preto; os demais, em vermelho-escuro.
pub fn lado_a_lado(quadros: &[(&str, &RgbImage)]) -> RgbImage {
    let (w, h) = quadros[0].1.dimensions();
    let faixa = (w / 22).max(28);
    let n = quadros.len() as u32;
    let mut saida = RgbImage::from_pixel(n * w + (n - 1) * 12, h + faixa, Rgb([255; 3]));
    for (i, (rotulo, img)) in quadros.iter().enumerate() {
        let x = i as u32 * (w + 12);
        image::imageops::replace(&mut saida, *img, x as i64, faixa as i64);
        let cor = if i == 0 { [0, 0, 0] } else { [140, 20, 20] };
        escrever(
            &mut saida,
            rotulo,
            x as f32 + 6.0,
            faixa as f32 * 0.12,
            faixa as f32 * 0.72,
            cor,
        );
    }
    saida
}
