//! Revela o original com a revelação que o Lightroom gravou no JPG exportado
//! e mede a nossa saída contra a dele.
//!
//! ```bash
//! cargo run --release -p infrastructure --example comparar_com_o_lightroom -- \
//!     <original.JPG> <exportado-pelo-lightroom.jpg> <pasta de saída> [modo]
//! ```
//!
//! 🔑 **O modo é a leitura do perfil criativo** (`<crs:Look>`). O XMP de uma
//! foto traz o perfil inteiro, com sliders próprios — o "B&W 01" é Contraste
//! +33, Realces −40, Sombras +45, uma curva —, aplicados na quantidade
//! (`crs:Amount`) e **por baixo** dos do operador:
//!
//! - `atual`: como `ler_xmp` lê hoje — o último `crs:Chave=` do arquivo vence,
//!   e o do perfil sobrescreve o do operador;
//! - `sem-look`: só os sliders do operador (o P&B do perfil fica);
//! - `somado`: operador + perfil × quantidade, e a curva do perfil antes da dele.
//!
//! 🔑 **A revelação vem do próprio JPG exportado**, e não da predefinição: é o
//! que o Lightroom de fato aplicou — o preset, a vinheta por cima e o que o
//! operador mexeu na foto. O caminho é o da exportação do app
//! (`ImageExporterImpl::renderizar_bytes`): base neutra, motor, enquadramento.
//!
//! Sai na pasta: `nosso.jpg`, `lado-a-lado.jpg` (Lightroom à esquerda) e
//! `diferenca-x4.png`. No terminal, a diferença geral e por faixa de tom do
//! Lightroom — onde a nossa foto está mais clara, mais quente ou mais verde.

use std::{fs, path::Path};

use domain::value_objects::CropSettings;
use image::{imageops::FilterType, RgbImage};
use infrastructure::{
    gpu_adjustments::{Ajustes, ParametrosLocais},
    image_exporter::ImageExporterImpl,
    lightroom::{self, Valor},
    revelacao_do_arquivo::{self, Origem},
};

fn luma(p: &image::Rgb<u8>) -> f64 {
    0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64
}

/// Diferença média por faixa de luminância **do Lightroom**: em cada faixa,
/// quanto a nossa está mais clara (luma), mais quente (R−B) e mais verde
/// (G − média de R e B). Positivo = a nossa tem mais.
fn por_faixa(nosso: &RgbImage, lr: &RgbImage) {
    const FAIXAS: [(&str, f64, f64); 5] = [
        ("pretos", 0.0, 32.0),
        ("sombras", 32.0, 96.0),
        ("médios", 96.0, 160.0),
        ("realces", 160.0, 224.0),
        ("brancos", 224.0, 256.0),
    ];
    println!(
        "{:9} {:>7} {:>8} {:>8} {:>8} {:>8}",
        "faixa", "pixels", "Δluma", "ΔR−B", "ΔG", "|Δ|"
    );
    for (nome, de, ate) in FAIXAS {
        let (mut n, mut dl, mut dq, mut dv, mut abs) = (0u64, 0.0, 0.0, 0.0, 0.0);
        for (a, b) in nosso.pixels().zip(lr.pixels()) {
            let l = luma(b);
            if l < de || l >= ate {
                continue;
            }
            let quente = |p: &image::Rgb<u8>| p[0] as f64 - p[2] as f64;
            let verde = |p: &image::Rgb<u8>| p[1] as f64 - (p[0] as f64 + p[2] as f64) / 2.0;
            n += 1;
            dl += luma(a) - l;
            dq += quente(a) - quente(b);
            dv += verde(a) - verde(b);
            abs += (0..3).map(|c| a[c].abs_diff(b[c]) as f64).sum::<f64>() / 3.0;
        }
        if n == 0 {
            continue;
        }
        let m = n as f64;
        let parte = n as f64 / (lr.width() * lr.height()) as f64 * 100.0;
        println!(
            "{nome:9} {parte:>6.1}% {:>8.1} {:>8.1} {:>8.1} {:>8.1}",
            dl / m,
            dq / m,
            dv / m,
            abs / m
        );
    }
}

fn geral(nosso: &RgbImage, lr: &RgbImage, saida: &Path) {
    let mut soma = [0f64; 3];
    let mut media = [[0f64; 3]; 2];
    let mut mapa = RgbImage::new(nosso.width(), nosso.height());
    for ((a, b), m) in nosso.pixels().zip(lr.pixels()).zip(mapa.pixels_mut()) {
        for c in 0..3 {
            soma[c] += a[c].abs_diff(b[c]) as f64;
            media[0][c] += a[c] as f64;
            media[1][c] += b[c] as f64;
        }
        let pior = (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap();
        let v = pior.saturating_mul(4);
        *m = image::Rgb([v, v, v]);
    }
    let n = (nosso.width() * nosso.height()) as f64;
    let f = |v: [f64; 3]| format!("{:6.1} {:6.1} {:6.1}", v[0] / n, v[1] / n, v[2] / n);
    println!("|Δ| médio R G B   {}", f(soma));
    println!("média nossa R G B {}", f(media[0]));
    println!("média LR    R G B {}", f(media[1]));
    mapa.save(saida.join("diferenca-x4.png")).unwrap();
}

/// O bloco `<crs:Look>…</crs:Look>` (o perfil criativo) e o XMP sem ele.
fn separar_o_look(texto: &str) -> (String, Option<String>) {
    let (Some(i), Some(k)) = (texto.find("<crs:Look>"), texto.find("</crs:Look>")) else {
        return (texto.to_string(), None);
    };
    let fim = k + "</crs:Look>".len();
    (
        format!("{}{}", &texto[..i], &texto[fim..]),
        Some(texto[i..fim].to_string()),
    )
}

fn pontos(itens: &[String]) -> Vec<(f32, f32)> {
    itens
        .iter()
        .filter_map(|i| {
            let (x, y) = i.split_once(',')?;
            Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .collect()
}

fn avaliar(p: &[(f32, f32)], x: f32) -> f32 {
    let j = p.iter().position(|(px, _)| *px >= x).unwrap_or(p.len() - 1);
    if j == 0 {
        return p[0].1;
    }
    let ((x0, y0), (x1, y1)) = (p[j - 1], p[j]);
    y0 + (y1 - y0) * (x - x0) / (x1 - x0).max(1e-6)
}

/// A revelação lida de um dos três jeitos — ver o uso.
fn ler(texto: &str, nome: &str, modo: &str) -> (Ajustes, Vec<String>) {
    let (fora, look) = separar_o_look(texto);
    let mut bruto = match modo {
        "atual" => lightroom::ler_xmp(texto, nome).unwrap(),
        _ => lightroom::ler_xmp(&fora, nome).unwrap(),
    };
    if modo != "atual" {
        if let Some(look) = look.as_deref().and_then(|l| lightroom::ler_xmp(l, nome)) {
            if look.ajustes.get("ConvertToGrayscale") == Some(&Valor::Booleano(true)) {
                bruto
                    .ajustes
                    .insert("ConvertToGrayscale".into(), Valor::Booleano(true));
            }
            if modo == "somado" {
                let quantidade = match look.ajustes.get("Amount") {
                    Some(Valor::Numero(n)) => *n,
                    _ => 1.0,
                };
                for (chave, valor) in &look.ajustes {
                    match valor {
                        Valor::Numero(n)
                            if chave.ends_with("2012")
                                || chave.starts_with("Clarity")
                                || chave.starts_with("Saturation") =>
                        {
                            let antes = match bruto.ajustes.get(chave) {
                                Some(Valor::Numero(v)) => *v,
                                _ => 0.0,
                            };
                            bruto
                                .ajustes
                                .insert(chave.clone(), Valor::Numero(antes + n * quantidade));
                        }
                        // A curva do perfil vem antes da do operador:
                        // y = operador(perfil(x)), com o perfil na quantidade.
                        Valor::Tabela(itens) if chave == "ToneCurvePV2012" => {
                            let perfil = pontos(itens);
                            let operador = match bruto.ajustes.get(chave) {
                                Some(Valor::Tabela(i)) => pontos(i),
                                _ => vec![(0.0, 0.0), (255.0, 255.0)],
                            };
                            let composta = (0..=15)
                                .map(|i| {
                                    let x = i as f32 * 17.0;
                                    let p = x + (avaliar(&perfil, x) - x) * quantidade;
                                    let y = avaliar(&operador, p.clamp(0.0, 255.0));
                                    format!("{x}, {}", y.round())
                                })
                                .collect();
                            bruto.ajustes.insert(chave.clone(), Valor::Tabela(composta));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    let traduzido = lightroom::traduzir(&bruto);
    let mut vetor = Ajustes::default().como_vetor();
    for (campo, valor) in traduzido.ajustes.iter() {
        if let Some(posicao) = Ajustes::NOMES.iter().position(|n| *n == campo) {
            vetor[posicao] = valor;
        }
    }
    // `VLB_FORCAR=shadows=40,tint=-5`: troca campos do motor depois da
    // tradução — para varrer um ajuste e ver quanto da diferença é dele.
    for par in std::env::var("VLB_FORCAR").unwrap_or_default().split(',') {
        let Some((campo, valor)) = par.split_once('=') else {
            continue;
        };
        let posicao = Ajustes::NOMES
            .iter()
            .position(|n| *n == campo.trim())
            .unwrap_or_else(|| panic!("`{campo}` não é campo do motor"));
        vetor[posicao] = valor.trim().parse().expect("número");
        println!("forçado: {campo}={valor}");
    }
    let mut lista: Vec<String> = traduzido
        .ajustes
        .iter()
        .filter(|(c, _)| !c.starts_with("curva_"))
        .map(|(c, v)| format!("{c}={v}"))
        .collect();
    lista.sort();
    println!("ajustes ({modo}): {}", lista.join(" "));
    (Ajustes::de_vetor(&vetor).unwrap(), traduzido.ignorados)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [original, exportado, saida, resto @ ..] = args.as_slice() else {
        panic!("uso: <original.JPG> <exportado-pelo-lightroom.jpg> <pasta de saída> [atual|sem-look|somado]")
    };
    let modo = resto.first().map(String::as_str).unwrap_or("atual");
    let saida = Path::new(saida).join(modo);
    let saida = saida.as_path();
    fs::create_dir_all(saida).unwrap();

    let bytes_lr = fs::read(exportado).expect("ler o exportado");
    let texto = raw_codec::xmp(&bytes_lr).expect("o exportado não traz XMP");
    let bytes = fs::read(original).expect("ler o original");
    // `orientacao_exif` lê TIFF (RAW, DNG); de um JPEG ela não sai, e sem ela
    // o corte não entra. O XMP do exportado a traz.
    let orientacao = raw_codec::orientacao_exif(&bytes).or_else(|| {
        let depois = texto.split("tiff:Orientation=\"").nth(1)?;
        depois.split('"').next()?.parse().ok()
    });
    let orientacao = orientacao.or(Some(1));
    let nome = Path::new(original).file_name().unwrap().to_string_lossy();
    // O corte não está no Look: sai igual dos três jeitos. Sem revelação
    // nenhuma (o "neutro" da régua), `de_xmp` devolve `None`: foto inteira.
    let corte = revelacao_do_arquivo::de_xmp(&texto, &nome, orientacao, Origem::Embutida)
        .map(|r| r.corte)
        .unwrap_or_default();
    let (ajustes, ignorados) = ler(&texto, &nome, modo);

    println!("orientação do original: {orientacao:?}");
    println!("ignorados: {ignorados:?}");
    let corte: &CropSettings = &corte;
    println!("corte: {corte:?}");

    let jpeg = ImageExporterImpl::new()
        .renderizar_bytes(&bytes, &ajustes, corte, &ParametrosLocais::default(), 95)
        .expect("revelar");
    fs::write(saida.join("nosso.jpg"), &jpeg).unwrap();

    let lr = image::load_from_memory(&bytes_lr).unwrap().to_rgb8();
    let mut nosso = image::load_from_memory(&jpeg).unwrap().to_rgb8();
    println!(
        "tamanho: nosso {}×{}, Lightroom {}×{}",
        nosso.width(),
        nosso.height(),
        lr.width(),
        lr.height()
    );
    if nosso.dimensions() != lr.dimensions() {
        nosso = image::imageops::resize(&nosso, lr.width(), lr.height(), FilterType::Lanczos3);
    }

    geral(&nosso, &lr, saida);
    por_faixa(&nosso, &lr);

    // Lado a lado, 1200 px de largura cada: Lightroom à esquerda.
    let lado = 1200;
    let alto = lado * lr.height() / lr.width();
    let reduzir = |i: &RgbImage| image::imageops::resize(i, lado, alto, FilterType::Triangle);
    let mut par = RgbImage::new(lado * 2, alto);
    image::imageops::replace(&mut par, &reduzir(&lr), 0, 0);
    image::imageops::replace(&mut par, &reduzir(&nosso), lado as i64, 0);
    par.save(saida.join("lado-a-lado.jpg")).unwrap();
}
