//! 🎞️ O balanço de branco do painel Básico: o conta-gotas e o "Automático".
//!
//! O motor não tem balanço em Kelvin — a temperatura e o colorir são
//! deslocamentos somados aos canais (o passo 3 e 4 de `corpo.wgsl`), como o
//! balanço relativo que o Lightroom dá a um JPEG. Então "neutralizar uma cor" é
//! resolver essa conta ao contrário: que temperatura e que colorir levam este
//! pixel ao cinza.
//!
//! ⚠️ **A amostra é da foto crua** (`Aberta::bruta`), com a exposição e o
//! contraste de agora aplicados — os dois passos que vêm antes da temperatura no
//! shader. Medir a foto da tela faria o segundo clique decidir sobre o resultado
//! do primeiro, e numa foto em P&B não haveria cor nenhuma para medir.

use image::{DynamicImage, GenericImageView};

use super::processador::Ajustes;

/// O limite dos dois campos no motor (±10, que a tela mostra como ±100).
const LIMITE: f32 = 10.0;

/// A temperatura e o colorir (na escala do motor) que levam `cor` — RGB 0–255
/// da foto crua — ao cinza, com a exposição e o contraste de `ajustes`.
///
/// 🔑 **A conta sai do shader, e fecha nos dois ramos do colorir.** A
/// temperatura soma `10·T` ao vermelho e tira do azul: o par se encontra na
/// média deles quando `T = (b − r) / 20`. O colorir mexe o verde contra o
/// vermelho e o azul juntos, `5·t` para cada lado, nos dois sinais — e por isso
/// `t = (g − m) / 10` serve para o magenta e para o verde.
pub fn neutralizar(cor: [f32; 3], ajustes: &Ajustes) -> (f32, f32) {
    let fator = 2f32.powf(ajustes.exposure);
    let [r, g, b] = cor.map(|c| (c * fator - 128.0) * ajustes.contrast + 128.0);
    let temperatura = (b - r) / 20.0;
    let meio = (r + b) / 2.0;
    let colorir = (g - meio) / 10.0;
    (
        temperatura.clamp(-LIMITE, LIMITE),
        colorir.clamp(-LIMITE, LIMITE),
    )
}

/// A média de um quadrado de `lado` pixels em volta de `ponto` (0–1 da foto
/// inteira) — o que o conta-gotas lê. Um pixel só seria ruído do sensor.
pub fn amostra(foto: &DynamicImage, ponto: [f32; 2], lado: u32) -> Option<[f32; 3]> {
    let (w, h) = foto.dimensions();
    if w == 0 || h == 0 || !(0.0..=1.0).contains(&ponto[0]) || !(0.0..=1.0).contains(&ponto[1]) {
        return None;
    }
    let cx = ((ponto[0] * w as f32) as u32).min(w - 1);
    let cy = ((ponto[1] * h as f32) as u32).min(h - 1);
    let meio = lado / 2;
    let mut soma = [0f32; 3];
    let mut n = 0f32;
    for y in cy.saturating_sub(meio)..=(cy + meio).min(h - 1) {
        for x in cx.saturating_sub(meio)..=(cx + meio).min(w - 1) {
            let p = foto.get_pixel(x, y);
            for c in 0..3 {
                soma[c] += p[c] as f32;
            }
            n += 1.0;
        }
    }
    Some(soma.map(|s| s / n))
}

/// A cor média da foto, sem o que está estourado ou no preto — o "mundo
/// cinza" do balanço automático: a média de uma cena comum é neutra, e o que
/// sobra de cor nela é o tom da luz.
///
/// ⚠️ É uma aproximação declarada: uma foto que é toda de uma cor (um campo de
/// grama, um pôr do sol) puxa a média para o lado dela, e o automático a
/// "corrige" demais — o mesmo limite do automático de qualquer câmera.
pub fn media_neutra(foto: &DynamicImage) -> Option<[f32; 3]> {
    let (w, h) = foto.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    // ~65 mil amostras bastam para a média, e a foto inteira levaria segundos.
    let passo = ((w as u64 * h as u64 / 65_536) as f64).sqrt().max(1.0) as u32;
    let mut soma = [0f64; 3];
    let mut n = 0f64;
    for y in (0..h).step_by(passo as usize) {
        for x in (0..w).step_by(passo as usize) {
            let p = foto.get_pixel(x, y);
            let maior = p[0].max(p[1]).max(p[2]);
            let menor = p[0].min(p[1]).min(p[2]);
            if maior >= 250 || menor <= 5 {
                continue;
            }
            for c in 0..3 {
                soma[c] += p[c] as f64;
            }
            n += 1.0;
        }
    }
    (n > 0.0).then(|| soma.map(|s| (s / n) as f32))
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Os passos 1 a 4 do shader, sobre um pixel: exposição, contraste,
    /// temperatura e colorir.
    fn shader(cor: [f32; 3], a: &Ajustes) -> [f32; 3] {
        let f = 2f32.powf(a.exposure);
        let [mut r, mut g, mut b] = cor.map(|c| (c * f - 128.0) * a.contrast + 128.0);
        r += a.temperature * 10.0;
        b -= a.temperature * 10.0;
        if a.tint > 0.0 {
            r += a.tint * 5.0;
            b += a.tint * 5.0;
            g -= a.tint * 5.0;
        } else {
            g -= a.tint * 5.0;
            r += a.tint * 5.0;
            b += a.tint * 5.0;
        }
        [r, g, b]
    }

    /// 🔑 **O conta-gotas leva a cor ao cinza** — com luz quente e com luz
    /// fria, puxando para o verde e para o magenta, e com exposição e
    /// contraste mexidos.
    #[test]
    fn o_conta_gotas_leva_a_cor_ao_cinza() {
        for cor in [
            [150.0, 120.0, 90.0],
            [90.0, 110.0, 150.0],
            [110.0, 140.0, 105.0],
            [140.0, 100.0, 135.0],
        ] {
            for (exposicao, contraste) in [(0.0, 1.0), (0.4, 1.2), (-0.3, 0.8)] {
                let mut a = Ajustes {
                    exposure: exposicao,
                    contrast: contraste,
                    ..Ajustes::default()
                };
                (a.temperature, a.tint) = neutralizar(cor, &a);
                let [r, g, b] = shader(cor, &a);
                assert!(
                    (r - g).abs() < 0.01 && (g - b).abs() < 0.01,
                    "{cor:?} com {exposicao}/{contraste} saiu {r} {g} {b}"
                );
            }
        }
    }

    /// O cinza já é cinza: nada a corrigir.
    #[test]
    fn o_cinza_fica_como_esta() {
        assert_eq!(neutralizar([128.0; 3], &Ajustes::default()), (0.0, 0.0));
    }

    /// A amostra é a média do quadrado, e fora da foto não há amostra.
    #[test]
    fn a_amostra_e_a_media_do_quadrado() {
        let mut foto = image::RgbImage::from_pixel(10, 10, image::Rgb([100, 100, 100]));
        foto.put_pixel(5, 5, image::Rgb([190, 100, 10]));
        let foto = DynamicImage::ImageRgb8(foto);
        assert_eq!(amostra(&foto, [0.55, 0.55], 1), Some([190.0, 100.0, 10.0]));
        assert_eq!(amostra(&foto, [0.55, 0.55], 3), Some([110.0, 100.0, 90.0]));
        assert_eq!(amostra(&foto, [1.5, 0.5], 3), None);
    }

    /// A média ignora o estouro e o preto, que não dizem nada da luz.
    #[test]
    fn a_media_ignora_estouro_e_preto() {
        let mut foto = image::RgbImage::from_pixel(4, 4, image::Rgb([120, 110, 90]));
        foto.put_pixel(0, 0, image::Rgb([255, 255, 255]));
        foto.put_pixel(1, 0, image::Rgb([0, 0, 0]));
        let media = media_neutra(&DynamicImage::ImageRgb8(foto)).unwrap();
        assert_eq!(media, [120.0, 110.0, 90.0]);
    }
}
