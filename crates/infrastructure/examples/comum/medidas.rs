//! As medidas de cor dos CLIs de comparação, num lugar só e em todos os núcleos.
//!
//! 🚨 **O CIEDE2000 é a parte cara**: numa thread só, uma rodada inteira do
//! `comparar_em_lote` levava ~1 h e um ajuste do `ajustar_pb`, mais de 1 h. A
//! conta é por pixel e independente, então se divide em faixas de linhas, uma
//! por núcleo (`std::thread::scope`, sem dependência nova).
//!
//! Incluído por `#[path = "comum/medidas.rs"] mod medidas;` — a pasta não tem
//! `main.rs`, e o cargo não a toma por exemplo.

#![allow(dead_code)]

use image::RgbImage;
use palette::{color_difference::Ciede2000, IntoColor, Lab, Srgb};

/// O Lab (D65) de um pixel sRGB de 8 bits.
pub fn lab(p: &image::Rgb<u8>) -> Lab {
    Srgb::new(p[0], p[1], p[2])
        .into_format::<f32>()
        .into_linear()
        .into_color()
}

/// O Lab de uma imagem inteira, na amostra (`passo` nos dois eixos).
pub fn labs(img: &RgbImage, passo: u32) -> Vec<Lab> {
    paralelo(img.height().div_ceil(passo), |y| {
        (0..img.width())
            .step_by(passo as usize)
            .map(|x| lab(img.get_pixel(x, y * passo)))
            .collect()
    })
}

/// O que se mede entre a nossa e a referência.
pub struct Medida {
    /// ΔE2000 médio e o percentil 95 (abaixo de ~2 não se nota).
    pub media: f64,
    pub p95: f64,
    /// ΔL* médio, nossa − referência.
    pub dl: f64,
    /// Croma nossa ÷ referência (1 = a mesma saturação).
    pub croma: f64,
}

/// A nossa contra a referência já em Lab (`labs` com o mesmo `passo`).
pub fn contra(nosso: &RgbImage, alvo: &[Lab], passo: u32) -> Medida {
    let largura = nosso.width().div_ceil(passo) as usize;
    // Por linha da amostra: (os ΔE, soma do ΔL, croma nossa, croma alvo).
    let linhas = paralelo(nosso.height().div_ceil(passo), |y| {
        let (mut des, mut dl, mut cn, mut ca) = (Vec::new(), 0.0, 0.0, 0.0);
        for (i, x) in (0..nosso.width()).step_by(passo as usize).enumerate() {
            let a = lab(nosso.get_pixel(x, y * passo));
            let b = alvo[y as usize * largura + i];
            des.push(a.difference(b) as f64);
            dl += (a.l - b.l) as f64;
            cn += a.a.hypot(a.b) as f64;
            ca += b.a.hypot(b.b) as f64;
        }
        vec![(des, dl, cn, ca)]
    });
    let mut des = Vec::new();
    let (mut dl, mut cn, mut ca) = (0.0, 0.0, 0.0);
    for (d, l, n, a) in linhas {
        des.extend(d);
        dl += l;
        cn += n;
        ca += a;
    }
    let n = des.len() as f64;
    let media = des.iter().sum::<f64>() / n;
    let k = ((n * 0.95) as usize).min(des.len() - 1);
    let p95 = *des.select_nth_unstable_by(k, |a, b| a.total_cmp(b)).1;
    Medida {
        media,
        p95,
        dl: dl / n,
        croma: cn / ca.max(1e-6),
    }
}

/// A nossa contra a referência, as duas em sRGB.
pub fn comparar(nosso: &RgbImage, referencia: &RgbImage, passo: u32) -> Medida {
    contra(nosso, &labs(referencia, passo), passo)
}

/// `f(linha)` para cada linha de `0..linhas`, em todos os núcleos, na ordem.
fn paralelo<T: Send>(linhas: u32, f: impl Fn(u32) -> Vec<T> + Sync) -> Vec<T> {
    let nucleos = std::thread::available_parallelism().map_or(4, |n| n.get()) as u32;
    let faixa = linhas.div_ceil(nucleos).max(1);
    std::thread::scope(|s| {
        let tarefas: Vec<_> = (0..linhas)
            .step_by(faixa as usize)
            .map(|inicio| {
                let f = &f;
                s.spawn(move || {
                    (inicio..(inicio + faixa).min(linhas))
                        .flat_map(f)
                        .collect::<Vec<T>>()
                })
            })
            .collect();
        tarefas
            .into_iter()
            .flat_map(|t| t.join().expect("thread da medida"))
            .collect()
    })
}
