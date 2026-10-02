//! O **RecordarFotos P&B** do app contra o do darktable — a régua do estilo das
//! fotos vendidas (estúdios de Canela e Gramado).
//!
//! ```text
//! cargo run --release -p infrastructure --example comparar_pb_darktable -- \
//!     --estilo "<RecordarFotos P&B.dtstyle>" --originais <pasta> --saida <pasta> \
//!     [--darktable-cli <exe>] [--preset <valores.json>] foto1 foto2 …
//! ```
//!
//! Para cada `foto` (o nome sem extensão em `--originais`):
//! 1. revela a referência no `darktable-cli` com o `.dtstyle`, se
//!    `<saida>/<foto>-dt.jpg` ainda não existe (`comum/darktable_cli.rs`);
//! 2. revela no motor do app, com a leitura do app (Adobe RGB nas fotos da
//!    câmera) e o RecordarFotos P&B do sistema (`use_cases::presets::
//!    RECORDARFOTOS_PB`) — ou os valores de `--preset`, um JSON do `ajustar_pb`;
//! 3. imprime ΔE2000, p95, ΔL* e a diferença de luma por faixa de tom;
//! 4. grava `<saida>/<foto>-darktable-x-app.jpg`, com legenda em cada lado.
//!
//! Abaixo de ΔE ~2 o olho não separa. Histórico e armadilhas:
//! `docs/REGUA-DO-LIGHTROOM.md`, seções 10–12.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::RgbImage;
use infrastructure::gpu_adjustments::{Ajustes, Motor, ParametrosLocais};

#[path = "comum/darktable_cli.rs"]
mod darktable_cli;
#[path = "comum/legenda.rs"]
mod legenda;
#[path = "comum/medidas.rs"]
mod medidas;

/// Os campos `{nome: valor}` sobre o neutro.
fn ajustes_de(campos: impl IntoIterator<Item = (String, f32)>) -> Ajustes {
    let mut v = Ajustes::default().como_vetor();
    for (campo, valor) in campos {
        let i = Ajustes::NOMES
            .iter()
            .position(|n| *n == campo)
            .unwrap_or_else(|| panic!("`{campo}` não é campo do motor"));
        v[i] = valor;
    }
    Ajustes::de_vetor(&v).unwrap()
}

fn do_json(caminho: &str) -> Ajustes {
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(caminho).expect("ler o json")).unwrap();
    ajustes_de(
        json.as_object()
            .expect("um objeto")
            .iter()
            .map(|(k, v)| (k.clone(), v.as_f64().unwrap() as f32)),
    )
}

fn reduzir(img: &RgbImage, lado: u32) -> RgbImage {
    let k = lado as f32 / img.width().max(img.height()) as f32;
    image::imageops::resize(
        img,
        (img.width() as f32 * k).round() as u32,
        (img.height() as f32 * k).round() as u32,
        image::imageops::FilterType::Triangle,
    )
}

/// A diferença média de luma (nosso − darktable) em 5 faixas de tom do
/// darktable, de 0 a 255.
fn faixas(nosso: &RgbImage, dt: &RgbImage) -> String {
    let luma = |p: &image::Rgb<u8>| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
    let mut f = [(0.0f64, 0.0f64); 5];
    for y in (0..dt.height()).step_by(2) {
        for x in (0..dt.width()).step_by(2) {
            let (a, b) = (nosso.get_pixel(x, y), dt.get_pixel(x, y));
            let i = ((luma(b) / 51.2) as usize).min(4);
            f[i].0 += luma(a) - luma(b);
            f[i].1 += 1.0;
        }
    }
    f.iter()
        .map(|(s, n)| {
            if *n > 0.0 {
                format!("{:+5.1}", s / n)
            } else {
                "    -".into()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn original(pasta: &Path, nome: &str) -> PathBuf {
    ["JPG", "jpg", "jpeg", "JPEG"]
        .iter()
        .map(|e| pasta.join(format!("{nome}.{e}")))
        .find(|p| p.exists())
        .unwrap_or_else(|| panic!("{nome}: original não achado em {}", pasta.display()))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let valor = |chave: &str| {
        args.windows(2)
            .find(|w| w[0] == chave)
            .map(|w| w[1].clone())
    };
    let estilo = PathBuf::from(valor("--estilo").expect("--estilo <.dtstyle>"));
    let originais = PathBuf::from(valor("--originais").expect("--originais <pasta>"));
    let saida = PathBuf::from(valor("--saida").expect("--saida <pasta>"));
    let cli = valor("--darktable-cli")
        .map(PathBuf::from)
        .unwrap_or_else(darktable_cli::achar);
    let (rotulo_app, ajustes) = match valor("--preset") {
        Some(json) => ("APP - valores de teste".to_string(), do_json(&json)),
        None => (
            "APP - RecordarFotos P&B".to_string(),
            ajustes_de(
                use_cases::presets::RECORDARFOTOS_PB
                    .iter()
                    .map(|(k, v)| (k.to_string(), *v)),
            ),
        ),
    };
    let com_valor = [
        "--estilo",
        "--originais",
        "--saida",
        "--darktable-cli",
        "--preset",
    ];
    let fotos: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            !a.starts_with("--") && (*i == 0 || !com_valor.contains(&args[i - 1].as_str()))
        })
        .map(|(_, a)| a)
        .collect();
    std::fs::create_dir_all(&saida).unwrap();

    let mut motor = Motor::abrir().expect("sem GPU");
    motor
        .definir_locais(&ParametrosLocais::default())
        .expect("locais vazios");
    println!(
        "{:34} {:>6} {:>6} {:>6}   luma por faixa, pretos → brancos",
        "foto", "ΔE", "p95", "ΔL*"
    );
    let mut soma = (0.0, 0);
    for foto in fotos {
        let arquivo = original(&originais, foto);
        let dt_jpg = saida.join(format!("{foto}-dt.jpg"));
        if !darktable_cli::revelar(&cli, &estilo, &arquivo, &dt_jpg, &saida.join("darktable")) {
            continue;
        }
        let dt = image::open(&dt_jpg)
            .expect("abrir a saída do darktable")
            .to_rgb8();
        let base = infrastructure::orientacao::abrir_de_pe(&arquivo).expect("abrir a foto");
        let rgba = base.to_rgba8();
        let (w, h) = rgba.dimensions();
        let nosso = motor
            .revelar(&Arc::new(rgba.into_raw()), w, h, &ajustes)
            .expect("o motor não revelou")
            .to_rgb8();
        assert_eq!(nosso.dimensions(), dt.dimensions(), "{foto}: tamanhos");
        let m = medidas::comparar(&nosso, &dt, 2);
        println!(
            "{foto:34} {:6.2} {:6.2} {:+6.2}   {}",
            m.media,
            m.p95,
            m.dl,
            faixas(&nosso, &dt)
        );
        soma = (soma.0 + m.media, soma.1 + 1);
        let lado = legenda::lado_a_lado(&[
            ("DARKTABLE (como foi vendido)", &reduzir(&dt, 1000)),
            (&rotulo_app, &reduzir(&nosso, 1000)),
        ]);
        lado.save(saida.join(format!("{foto}-darktable-x-app.jpg")))
            .unwrap();
    }
    if soma.1 > 0 {
        println!(
            "média: ΔE {:.2} em {} foto(s)",
            soma.0 / soma.1 as f64,
            soma.1
        );
    }
}
