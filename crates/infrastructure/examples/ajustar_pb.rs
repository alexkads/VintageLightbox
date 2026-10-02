//! Acha os controles do motor que reproduzem o `RecordarFotos P&B` do darktable.
//!
//! 🔑 **Um motor só** (dono, 2/out/2026): o estilo vinha do darktable e tinha
//! módulos próprios, em RGB linear Rec.2020, lendo a foto como sRGB (o `colorin`
//! do `.dtstyle` fixa sRGB). Com a foto lida pelo espaço que ela declara (Adobe
//! RGB nas da câmera), o estilo passa a ser feito com os controles de sempre —
//! P&B, tom, viragem, vinheta — e este exemplo procura os valores que deixam o
//! resultado como o do darktable de hoje.
//!
//! ```text
//! darktable-cli foto.JPG pb.xmp foto-dt.jpg --hq true --core --configdir <pasta>
//! cargo run --release -p infrastructure --example ajustar_pb -- \
//!     <pasta dos originais> <pasta das saídas do darktable> <saida.json> \
//!     foto1 foto2 … @validacao1 @validacao2
//! ```
//!
//! Cada `foto` é o nome sem extensão: `<originais>/foto.JPG` e
//! `<darktable>/foto-dt.jpg`. As marcadas com `@` ficam fora do ajuste e só são
//! medidas no fim. A busca é uma descida coordenada: cada controle anda um
//! passo para cima e para baixo; o que baixa o ΔE2000 médio fica, e o passo de
//! quem não melhorou cai pela metade.

use std::{fs, path::Path, sync::Arc};

use image::{imageops::FilterType, DynamicImage};
use infrastructure::gpu_adjustments::{Ajustes, Motor, ParametrosLocais};
use palette::{color_difference::Ciede2000, Lab};

#[path = "comum/legenda.rs"]
mod legenda;
#[path = "comum/medidas.rs"]
mod medidas;

/// O lado maior em que se ajusta (e o passo da amostra): o bastante para a
/// vinheta e as sombras locais, e rápido para mil revelações.
const LADO: u32 = 768;
const PASSO_DA_AMOSTRA: u32 = 3;

/// (campo, início, mínimo, máximo, passo inicial).
const CONTROLES: [(&str, f32, f32, f32, f32); 33] = [
    // A vinheta da Lente vem antes da viragem: a borda clareada ganha o sépia,
    // como no darktable. A pós-corte vem depois e clareia para o branco neutro.
    ("lens_vignette_amount", 0.0, -100.0, 100.0, 8.0),
    ("lens_vignette_midpoint", 50.0, 0.0, 100.0, 8.0),
    ("texture", 0.0, -100.0, 100.0, 8.0),
    ("pcv_highlights", 0.0, 0.0, 100.0, 8.0),
    ("exposure", 0.16, -1.5, 1.5, 0.1),
    ("contrast", 1.0, 0.5, 1.6, 0.05),
    ("highlights", -20.0, -100.0, 100.0, 8.0),
    ("shadows", 40.0, -100.0, 100.0, 8.0),
    ("whites", 0.0, -100.0, 100.0, 8.0),
    ("blacks", 0.0, -100.0, 100.0, 8.0),
    ("clarity", 0.0, -1.0, 1.0, 0.1),
    ("tone_curve_shadows", 0.0, -100.0, 100.0, 8.0),
    ("tone_curve_darks", 0.0, -100.0, 100.0, 8.0),
    ("tone_curve_lights", 0.0, -100.0, 100.0, 8.0),
    ("tone_curve_highlights", 0.0, -100.0, 100.0, 8.0),
    ("bw_red", 0.0, -100.0, 100.0, 10.0),
    ("bw_orange", 0.0, -100.0, 100.0, 10.0),
    ("bw_yellow", 0.0, -100.0, 100.0, 10.0),
    ("bw_green", 0.0, -100.0, 100.0, 10.0),
    ("bw_aqua", 0.0, -100.0, 100.0, 10.0),
    ("bw_blue", 0.0, -100.0, 100.0, 10.0),
    ("bw_purple", 0.0, -100.0, 100.0, 10.0),
    ("bw_magenta", 0.0, -100.0, 100.0, 10.0),
    ("split_shadow_hue", 45.0, 0.0, 360.0, 10.0),
    ("split_shadow_sat", 15.0, 0.0, 100.0, 4.0),
    ("split_highlight_hue", 50.0, 0.0, 360.0, 10.0),
    ("split_highlight_sat", 8.0, 0.0, 100.0, 4.0),
    ("split_balance", 0.0, -100.0, 100.0, 10.0),
    ("pcv_amount", 25.0, -100.0, 100.0, 8.0),
    ("pcv_midpoint", 50.0, 0.0, 100.0, 8.0),
    ("pcv_feather", 50.0, 30.0, 100.0, 8.0),
    ("pcv_roundness", 0.0, -100.0, 100.0, 10.0),
    ("dehaze", 0.0, -100.0, 100.0, 8.0),
];

struct Foto {
    nome: String,
    validacao: bool,
    pixels: Arc<Vec<u8>>,
    largura: u32,
    altura: u32,
    escala: f32,
    /// O Lab do darktable, já na amostra.
    alvo: Vec<Lab>,
    /// O darktable reduzido, para o lado a lado do `VLB_SO_MEDIR`.
    darktable: image::RgbImage,
}

fn reduzir(img: &DynamicImage, lado: u32) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let k = lado as f32 / w.max(h) as f32;
    img.resize_exact(
        (w as f32 * k).round() as u32,
        (h as f32 * k).round() as u32,
        FilterType::Triangle,
    )
}

/// O estilo da vinheta é uma escolha (0 realces, 1 cores, 2 sobreposição), e
/// não um número para descer: vem de `VLB_PCV_STYLE`, uma rodada por estilo.
fn estilo_da_vinheta() -> f32 {
    std::env::var("VLB_PCV_STYLE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0)
}

fn ajustes(valores: &[f32]) -> Ajustes {
    let mut v = Ajustes {
        bw_ativo: 1.0,
        processo: 1.0,
        pcv_style: estilo_da_vinheta(),
        ..Default::default()
    }
    .como_vetor();
    for ((campo, ..), valor) in CONTROLES.iter().zip(valores) {
        let i = Ajustes::NOMES.iter().position(|n| n == campo).unwrap();
        v[i] = *valor;
    }
    Ajustes::de_vetor(&v).unwrap()
}

/// (ΔE2000 médio, p95) de uma foto com estes valores.
fn medir(motor: &mut Motor, foto: &Foto, valores: &[f32]) -> (f64, f64) {
    motor.definir_escala_do_original(foto.escala);
    let nosso = motor
        .revelar(&foto.pixels, foto.largura, foto.altura, &ajustes(valores))
        .expect("o motor não revelou")
        .to_rgb8();
    let m = medidas::contra(&nosso, &foto.alvo, PASSO_DA_AMOSTRA);
    (m.media, m.p95)
}

fn custo(motor: &mut Motor, fotos: &[Foto], valores: &[f32]) -> f64 {
    let ajuste: Vec<&Foto> = fotos.iter().filter(|f| !f.validacao).collect();
    ajuste
        .iter()
        .map(|f| medir(motor, f, valores).0)
        .sum::<f64>()
        / ajuste.len() as f64
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [originais, darktable, saida, nomes @ ..] = args.as_slice() else {
        panic!("uso: <originais> <darktable> <saida.json> foto… @validacao…");
    };
    let mut fotos = Vec::new();
    for nome in nomes {
        let (validacao, nome) = match nome.strip_prefix('@') {
            Some(n) => (true, n.to_string()),
            None => (false, nome.clone()),
        };
        let original = Path::new(originais).join(format!("{nome}.JPG"));
        let base = infrastructure::orientacao::abrir_de_pe(&original).expect("abrir o original");
        let reduzida = reduzir(&base, LADO).to_rgba8();
        let dt = image::open(Path::new(darktable).join(format!("{nome}-dt.jpg")))
            .expect("abrir a saída do darktable");
        let dt = reduzir(&dt, LADO).to_rgb8();
        assert_eq!(dt.dimensions(), reduzida.dimensions(), "{nome}: tamanhos");
        fotos.push(Foto {
            escala: LADO as f32 / base.width().max(base.height()) as f32,
            largura: reduzida.width(),
            altura: reduzida.height(),
            pixels: Arc::new(reduzida.into_raw()),
            alvo: medidas::labs(&dt, PASSO_DA_AMOSTRA),
            darktable: dt,
            nome,
            validacao,
        });
    }

    let mut motor = Motor::abrir().expect("sem GPU");
    motor
        .definir_locais(&ParametrosLocais::default())
        .expect("locais vazios");

    let mut valores: Vec<f32> = CONTROLES.iter().map(|c| c.1).collect();
    // `VLB_SO_MEDIR=1`: parte do `saida.json` de uma rodada anterior, não
    // procura nada, e grava ao lado dele, por foto, a nossa, a do darktable e
    // o mapa do ΔE2000 (preto 0, branco 20 ou mais).
    let so_medir = std::env::var("VLB_SO_MEDIR").is_ok();
    if so_medir {
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(saida).expect("ler o json")).unwrap();
        for (valor, (campo, ..)) in valores.iter_mut().zip(CONTROLES) {
            *valor = json[campo].as_f64().unwrap_or(*valor as f64) as f32;
        }
        let pasta = Path::new(saida).parent().unwrap();
        for foto in &fotos {
            motor.definir_escala_do_original(foto.escala);
            let nosso = motor
                .revelar(&foto.pixels, foto.largura, foto.altura, &ajustes(&valores))
                .unwrap()
                .to_rgb8();
            let mapa = image::GrayImage::from_fn(nosso.width(), nosso.height(), |x, y| {
                let p = PASSO_DA_AMOSTRA;
                let alvo = foto.alvo[((y / p) * nosso.width().div_ceil(p) + x / p) as usize];
                let d = medidas::lab(nosso.get_pixel(x, y)).difference(alvo);
                image::Luma([(d / 20.0 * 255.0).min(255.0) as u8])
            });
            nosso
                .save(pasta.join(format!("{}-nosso.png", foto.nome)))
                .unwrap();
            mapa.save(pasta.join(format!("{}-erro.png", foto.nome)))
                .unwrap();
            // O lado a lado, com legenda: o darktable à esquerda, a nossa à direita.
            let lado = legenda::lado_a_lado(&[
                ("DARKTABLE (referência)", &foto.darktable),
                ("APP - valores do ajuste", &nosso),
            ]);
            lado.save(pasta.join(format!("{}-lado-a-lado.png", foto.nome)))
                .unwrap();
        }
    }
    // `VLB_INICIO=<json>`: parte de uma rodada anterior, com um quarto do passo.
    let mut passos: Vec<f32> = CONTROLES.iter().map(|c| c.4).collect();
    if let Ok(inicio) = std::env::var("VLB_INICIO") {
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(inicio).expect("ler o início")).unwrap();
        for ((valor, passo), (campo, ..)) in valores.iter_mut().zip(&mut passos).zip(CONTROLES) {
            // Quem não está no json começa do início, com o passo inteiro.
            if let Some(v) = json[campo].as_f64() {
                *valor = v as f32;
                *passo /= 4.0;
            }
        }
    }
    let mut melhor = custo(&mut motor, &fotos, &valores);
    eprintln!("início: ΔE {melhor:.3}");
    for rodada in 1..=if so_medir { 0 } else { 40 } {
        let antes = melhor;
        for i in 0..CONTROLES.len() {
            let (_, _, minimo, maximo, inicial) = CONTROLES[i];
            if passos[i] < inicial / 64.0 {
                continue;
            }
            let mut melhorou = false;
            for sinal in [1.0, -1.0] {
                let mut tentativa = valores.clone();
                tentativa[i] = (valores[i] + sinal * passos[i]).clamp(minimo, maximo);
                if tentativa[i] == valores[i] {
                    continue;
                }
                let c = custo(&mut motor, &fotos, &tentativa);
                if c < melhor - 1e-4 {
                    melhor = c;
                    valores = tentativa;
                    melhorou = true;
                    break;
                }
            }
            if !melhorou {
                passos[i] *= 0.5;
            }
        }
        eprintln!("rodada {rodada:2}: ΔE {melhor:.3}");
        if antes - melhor < 1e-3 && passos.iter().zip(&CONTROLES).all(|(p, c)| *p < c.4 / 16.0) {
            break;
        }
    }

    println!("\nfoto                              ΔE2000   p95");
    for foto in &fotos {
        let (m, p) = medir(&mut motor, foto, &valores);
        let marca = if foto.validacao { " (validação)" } else { "" };
        println!("{:32} {m:6.2} {p:6.2}{marca}", foto.nome);
    }
    let mut json = format!(
        "{{\n  \"bw_ativo\": 1.0,\n  \"processo\": 1.0,\n  \"pcv_style\": {}",
        estilo_da_vinheta()
    );
    for ((campo, inicio, ..), valor) in CONTROLES.iter().zip(&valores) {
        println!("{campo:24} {valor:8.3}   (começou em {inicio})");
        json.push_str(&format!(",\n  \"{campo}\": {valor}"));
    }
    json.push_str("\n}\n");
    fs::write(saida, json).expect("gravar o json");
}
