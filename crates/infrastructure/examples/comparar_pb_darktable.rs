//! O `RecordarFotos P&B` do nosso motor contra o do `darktable-cli` 5.6.1, numa
//! foto da câmera, lendo o arquivo das duas formas: como sRGB (o que o app fez
//! até 2/out/2026) e pelo espaço que ele declara (o `R03` das Nikon: Adobe RGB).
//!
//! ```text
//! darktable-cli foto.jpg dt.jpg --style "RecordarFotos P&B" --core --configdir <pasta>
//! cargo run --release -p infrastructure --example comparar_pb_darktable -- \
//!     <foto.jpg> <dt.jpg> [<pasta para as nossas>]
//! ```
//!
//! 🔑 O `.dtstyle` do estúdio traz o próprio `colorin` com o perfil de entrada
//! **sRGB** fixo (`type = 1`): o darktable, com esse estilo, ignora o `R03`. A
//! pergunta que este exemplo responde é qual das duas leituras bate com ele.

use std::path::Path;

use image::{metadata::Orientation, DynamicImage, ImageDecoder, ImageReader, RgbImage};
use infrastructure::gpu_adjustments::{Ajustes, Motor, ParametrosLocais};

#[path = "comum/medidas.rs"]
mod medidas;

/// Os números do `docs/RecordarFotos P&B.dtstyle`, como na predefinição do
/// sistema (`use-cases/src/presets/list_presets.rs`).
fn estilo() -> Ajustes {
    Ajustes {
        dt_exposure_ativo: 1.0,
        dt_exposure_black: -0.001_900_002_4,
        dt_exposure_exposure: 0.162_999_87,
        dt_shadhi_ativo: 1.0,
        dt_shadhi_shadows: 65.380_005,
        dt_shadhi_highlights: -20.509_995,
        dt_monochrome_ativo: 1.0,
        dt_vignette_ativo: 1.0,
        dt_vignette_scale: 87.819_99,
        dt_vignette_falloff_scale: 45.51,
        dt_vignette_brightness: 0.999_999_9,
        dt_vignette_saturation: 0.146_999_96,
        dt_vignette_autoratio: 1.0,
        dt_vignette_shape: 0.479_999_96,
        dt_cb_ativo: 1.0,
        dt_cb_shadows_c: 0.174_699_98,
        dt_cb_shadows_h: 71.539_99,
        dt_cb_midtones_h: 73.849_99,
        dt_cb_highlights_y: 0.0449,
        dt_cb_highlights_c: 0.083_299_994,
        dt_cb_highlights_h: 71.539_99,
        dt_cb_saturation_highlights: 0.160_300_02,
        dt_cb_saturation_midtones: 0.134_599_92,
        dt_cb_brilliance_midtones: 0.147_400_02,
        ..Default::default()
    }
}

/// De pé, sem olhar o espaço de cor — a leitura antiga.
fn como_srgb(caminho: &Path) -> DynamicImage {
    let mut d = ImageReader::open(caminho)
        .unwrap()
        .with_guessed_format()
        .unwrap()
        .into_decoder()
        .unwrap();
    let o = d.orientation().unwrap_or(Orientation::NoTransforms);
    let mut foto = DynamicImage::from_decoder(d).unwrap();
    foto.apply_orientation(o);
    foto
}

/// ΔE2000 médio e p95, ΔL* médio (nossa − darktable) e a diferença média de
/// luma por faixa de tom do darktable (0–255).
fn medir(nosso: &RgbImage, dt: &RgbImage) -> String {
    let m = medidas::comparar(nosso, dt, 2);
    let luma = |p: &image::Rgb<u8>| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
    let mut faixas = [(0.0f64, 0.0f64); 5];
    for y in (0..dt.height()).step_by(2) {
        for x in (0..dt.width()).step_by(2) {
            let (a, b) = (nosso.get_pixel(x, y), dt.get_pixel(x, y));
            let f = ((luma(b) / 51.2) as usize).min(4);
            faixas[f].0 += luma(a) - luma(b);
            faixas[f].1 += 1.0;
        }
    }
    let por_faixa: Vec<String> = faixas
        .iter()
        .map(|(s, n)| {
            if *n > 0.0 {
                format!("{:+6.1}", s / n)
            } else {
                "     -".into()
            }
        })
        .collect();
    format!(
        "ΔE {:5.2}  p95 {:5.2}  ΔL* {:+5.2}  luma por faixa (pretos→brancos) {}",
        m.media,
        m.p95,
        m.dl,
        por_faixa.join(" ")
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [foto, dt, ..] = args.as_slice() else {
        panic!("uso: <foto.jpg> <dt.jpg> [<pasta>]");
    };
    let dt = image::open(dt)
        .expect("abrir a saída do darktable")
        .to_rgb8();

    let mut motor = Motor::abrir().expect("sem GPU");
    motor
        .definir_locais(&ParametrosLocais::default())
        .expect("locais vazios");
    let leituras = [
        ("como sRGB (antes)", como_srgb(Path::new(foto))),
        (
            "pelo espaço declarado",
            infrastructure::orientacao::abrir_de_pe(foto).expect("abrir a foto"),
        ),
    ];
    for (rotulo, base) in leituras {
        let rgba = base.to_rgba8();
        let (w, h) = rgba.dimensions();
        let nosso = motor
            .revelar(&std::sync::Arc::new(rgba.into_raw()), w, h, &estilo())
            .expect("o motor não revelou")
            .to_rgb8();
        assert_eq!(nosso.dimensions(), dt.dimensions(), "tamanhos diferentes");
        println!("{rotulo:24} {}", medir(&nosso, &dt));
        if let Some(pasta) = args.get(2) {
            let nome = Path::new(foto)
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let sufixo = if rotulo.starts_with("como") {
                "srgb"
            } else {
                "declarado"
            };
            nosso
                .save(Path::new(pasta).join(format!("{nome}-{sufixo}.png")))
                .unwrap();
        }
    }
}
