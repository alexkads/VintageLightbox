//! A comparação com o Lightroom em lote: muitas exportações, um processo só.
//!
//! ```bash
//! # Tudo: toda régua debaixo da pasta, nos dois processos, com o resumo — e
//! # o que mudou desde a última rodada.
//! cargo run --release -p infrastructure --example comparar_em_lote -- \
//!     <saída.csv> --reguas "<pasta Comparar Presets>" --processos 0,1 [--base <saída anterior.csv>]
//!
//! cargo run --release -p infrastructure --example comparar_em_lote -- \
//!     <saída.csv> --regua "<pasta de uma régua>" [--regua …] [--processos 0,1] \
//!     [--filtro Contrast,Shadows] [--forcar shadows=10;tint=-2] [--imagens <pasta>]
//!
//! cargo run --release -p infrastructure --example comparar_em_lote -- \
//!     <saída.csv> --lista <casos.csv> [--processos 0,1] [--imagens <pasta>]
//! ```
//!
//! 🔑 **O resumo é o que pega regressão.** Além da linha por caso, sai
//! `<saída>.resumo.csv` e uma tabela no terminal: cada caso agrupado nas fotos
//! (separando JPG de RAW), a média em cada processo e a diferença do 1 para o
//! 0, **o que piorou primeiro**. Com `--base`, uma coluna a mais: quanto cada
//! caso mudou desde aquela rodada — é o que se roda depois de mexer no motor.
//!
//! O `comparar_com_o_lightroom` abre um processo, a GPU e o original de 24 MP
//! **por caso**; numa régua de 300 casos isso é uma hora. Este abre o motor uma
//! vez, decodifica cada original uma vez, e revela já no tamanho da exportação
//! do Lightroom (o lado maior de 2048 da régua), com a escala do original
//! informada ao motor — os ajustes de vizinhança medem a mesma fração da cena.
//!
//! - `--regua <pasta>`: a pasta `saida=` de um pedido do plug-in
//!   (`ferramentas/lightroom/`). Cada subpasta é uma foto, com os casos dentro
//!   (`NN-caso.jpg`), e o original em `originais/` com o mesmo nome.
//! - `--lista <csv>`: `rotulo,original,exportado,forcar`, uma linha por caso.
//! - `--processos`: roda cada caso em cada processo (padrão: o que a tradução
//!   der). `--forcar` troca campos do motor depois da tradução, como o
//!   `VLB_FORCAR` do comparador. `--filtro` fica só com os casos cujo nome
//!   contém um dos pedaços.
//! - `--imagens <pasta>`: grava o lado a lado (Lightroom à esquerda) de cada caso.
//!
//! O original abre pela base neutra do app, que já lê o espaço de cor do
//! arquivo (`foto_codec::espaco_de_cor`: as fotos da câmera são Adobe RGB). Até
//! 2/out/2026 havia uma opção `--adobe` para isso; hoje ela converteria duas vezes.
//!
//! A saída tem, por caso, a diferença média (0–255) e, em cada faixa de tom do
//! Lightroom, quanto a nossa está mais clara, mais quente (R−B) e mais verde.
//!
//! ⚠️ **O caminho é o do motor, e não o da exportação do app**: revelar na
//! escala da exportação é o que o torna rápido. O comparador de um caso
//! (`comparar_com_o_lightroom`) continua sendo o da exportação, em tamanho
//! cheio.

use std::{
    collections::HashMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

#[path = "comum/medidas.rs"]
mod medidas;

use domain::value_objects::CropSettings;
use image::{imageops::FilterType, DynamicImage, RgbImage};
use infrastructure::{
    base_neutra,
    gpu_adjustments::{Ajustes, Motor, ParametrosLocais},
    lightroom,
    revelacao_do_arquivo::{self, Origem},
    transformacao,
};

struct Caso {
    rotulo: String,
    original: PathBuf,
    exportado: PathBuf,
    forcar: Vec<(String, f32)>,
}

fn pares(texto: &str) -> Vec<(String, f32)> {
    texto
        .split([';', ','])
        .filter_map(|p| {
            let (c, v) = p.split_once('=')?;
            Some((c.trim().to_string(), v.trim().parse().ok()?))
        })
        .collect()
}

/// Os casos de uma régua: `<pasta>/<foto>/NN-caso.jpg`, com o original em
/// `<pasta>/originais/<foto>.*`.
fn casos_da_regua(pasta: &Path) -> Vec<Caso> {
    let originais: Vec<PathBuf> = fs::read_dir(pasta.join("originais"))
        .expect("a régua tem a pasta originais")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    let mut casos = Vec::new();
    let mut fotos: Vec<PathBuf> = fs::read_dir(pasta)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n != "originais"))
        .collect();
    fotos.sort();
    for foto in fotos {
        let nome = foto.file_name().unwrap().to_string_lossy().to_string();
        let Some(original) = originais
            .iter()
            .find(|o| o.file_stem().is_some_and(|s| s.to_string_lossy() == nome))
        else {
            eprintln!("sem original para {nome}");
            continue;
        };
        let mut arquivos: Vec<PathBuf> = fs::read_dir(&foto)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("jpg")))
            .collect();
        arquivos.sort();
        for exportado in arquivos {
            let caso = exportado.file_stem().unwrap().to_string_lossy();
            let caso = caso.split_once('-').map(|(_, r)| r).unwrap_or(&caso);
            casos.push(Caso {
                rotulo: format!("{nome}/{caso}"),
                original: original.clone(),
                exportado,
                forcar: Vec::new(),
            });
        }
    }
    casos
}

fn casos_da_lista(csv: &Path) -> Vec<Caso> {
    fs::read_to_string(csv)
        .expect("ler a lista")
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let c: Vec<&str> = l.splitn(4, ',').collect();
            Caso {
                rotulo: c[0].to_string(),
                original: PathBuf::from(c[1]),
                exportado: PathBuf::from(c[2]),
                forcar: c.get(3).map(|f| pares(f)).unwrap_or_default(),
            }
        })
        .collect()
}

/// A revelação do XMP do exportado, nos termos do motor: os ajustes e o corte.
fn revelacao(exportado: &[u8], original: &Path) -> (Ajustes, CropSettings) {
    let texto = raw_codec::xmp(exportado).expect("o exportado não traz XMP");
    let nome = original.file_name().unwrap().to_string_lossy();
    let bytes_do_original = fs::read(original).unwrap_or_default();
    let orientacao = raw_codec::orientacao_exif(&bytes_do_original)
        .or_else(|| {
            let depois = texto.split("tiff:Orientation=\"").nth(1)?;
            depois.split('"').next()?.parse().ok()
        })
        .or(Some(1));
    let corte = revelacao_do_arquivo::de_xmp(&texto, &nome, orientacao, Origem::Embutida)
        .map(|r| r.corte)
        .unwrap_or_default();
    let bruto = lightroom::ler_xmp(&texto, &nome).expect("XMP do Lightroom");
    let traduzido = lightroom::traduzir(&bruto);
    let mut vetor = Ajustes::default().como_vetor();
    for (campo, valor) in traduzido.ajustes.iter() {
        if let Some(i) = Ajustes::NOMES.iter().position(|n| *n == campo) {
            vetor[i] = valor;
        }
    }
    (Ajustes::de_vetor(&vetor).unwrap(), corte)
}

fn forcar(ajustes: &Ajustes, campos: &[(String, f32)]) -> Ajustes {
    let mut v = ajustes.como_vetor();
    for (campo, valor) in campos {
        let i = Ajustes::NOMES
            .iter()
            .position(|n| n == campo)
            .unwrap_or_else(|| panic!("`{campo}` não é campo do motor"));
        v[i] = *valor;
    }
    Ajustes::de_vetor(&v).unwrap()
}

fn luma(p: &image::Rgb<u8>) -> f64 {
    0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64
}

const FAIXAS: [(&str, f64, f64); 5] = [
    ("pretos", 0.0, 32.0),
    ("sombras", 32.0, 96.0),
    ("medios", 96.0, 160.0),
    ("realces", 160.0, 224.0),
    ("brancos", 224.0, 256.0),
];

/// `geral` e, por faixa, `luma,quente,verde` — vazio onde a faixa não tem pixel.
/// As medidas que o olho entende, em Lab (`palette`): `ΔE2000` médio e o
/// percentil 95 (abaixo de ~2 não se nota), `ΔL*` (nossa − Lightroom, só a
/// luminosidade), a razão de croma (nossa ÷ Lightroom: 1 = a mesma saturação,
/// menos = mais lavada) e o SSIM da luminância (1 = a mesma estrutura — é o
/// que Remover névoa, Claridade e Textura mexem).
///
/// Um pixel em cada quatro (passo 2 nos dois eixos): a média não muda, e a
/// conta do CIEDE2000 é a parte cara.
fn perceptual(nosso: &RgbImage, lr: &RgbImage) -> (f64, f64, f64, f64, f64) {
    let m = medidas::comparar(nosso, lr, 2);
    (m.media, m.p95, m.dl, m.croma, ssim(nosso, lr))
}

/// O SSIM da luminância, em janelas 8×8 de passo 4, numa redução a 512 px.
fn ssim(nosso: &RgbImage, lr: &RgbImage) -> f64 {
    let lado = 512u32.min(lr.width());
    let alto = (lado * lr.height() / lr.width()).max(8);
    let cinza = |i: &RgbImage| -> Vec<f64> {
        image::imageops::resize(i, lado, alto, FilterType::Triangle)
            .pixels()
            .map(luma)
            .collect()
    };
    let (a, b) = (cinza(nosso), cinza(lr));
    let (w, h) = (lado as usize, alto as usize);
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    let (mut soma, mut n) = (0.0, 0.0);
    for y0 in (0..h.saturating_sub(8)).step_by(4) {
        for x0 in (0..w.saturating_sub(8)).step_by(4) {
            let (mut ma, mut mb) = (0.0, 0.0);
            for y in y0..y0 + 8 {
                for x in x0..x0 + 8 {
                    ma += a[y * w + x];
                    mb += b[y * w + x];
                }
            }
            ma /= 64.0;
            mb /= 64.0;
            let (mut va, mut vb, mut cov) = (0.0, 0.0, 0.0);
            for y in y0..y0 + 8 {
                for x in x0..x0 + 8 {
                    let (da, db) = (a[y * w + x] - ma, b[y * w + x] - mb);
                    va += da * da;
                    vb += db * db;
                    cov += da * db;
                }
            }
            va /= 63.0;
            vb /= 63.0;
            cov /= 63.0;
            soma += ((2.0 * ma * mb + c1) * (2.0 * cov + c2))
                / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1.0;
        }
    }
    if n > 0.0 {
        soma / n
    } else {
        1.0
    }
}

fn medir(nosso: &RgbImage, lr: &RgbImage) -> String {
    let mut soma = 0.0;
    for (a, b) in nosso.pixels().zip(lr.pixels()) {
        soma += (0..3).map(|c| a[c].abs_diff(b[c]) as f64).sum::<f64>() / 3.0;
    }
    let total = (lr.width() * lr.height()) as f64;
    let mut linha = format!("{:.2}", soma / total);
    for (_, de, ate) in FAIXAS {
        let (mut n, mut dl, mut dq, mut dv) = (0.0, 0.0, 0.0, 0.0);
        for (a, b) in nosso.pixels().zip(lr.pixels()) {
            let l = luma(b);
            if l < de || l >= ate {
                continue;
            }
            let q = |p: &image::Rgb<u8>| p[0] as f64 - p[2] as f64;
            let v = |p: &image::Rgb<u8>| p[1] as f64 - (p[0] as f64 + p[2] as f64) / 2.0;
            n += 1.0;
            dl += luma(a) - l;
            dq += q(a) - q(b);
            dv += v(a) - v(b);
        }
        if n > 0.0 {
            let _ = write!(linha, ",{:.2},{:.2},{:.2}", dl / n, dq / n, dv / n);
        } else {
            linha.push_str(",,,");
        }
    }
    linha
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let saida = PathBuf::from(args.first().expect("o CSV de saída"));
    let valor = |chave: &str| {
        args.iter()
            .position(|a| a == chave)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    // `--regua` pode vir mais de uma vez; `--reguas` acha toda régua debaixo
    // de uma pasta (a que tem `originais/`).
    let mut reguas: Vec<PathBuf> = args
        .windows(2)
        .filter(|w| w[0] == "--regua")
        .map(|w| PathBuf::from(&w[1]))
        .collect();
    if let Some(raiz) = valor("--reguas") {
        let mut achadas: Vec<PathBuf> = fs::read_dir(&raiz)
            .expect("ler a pasta das réguas")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.join("originais").is_dir())
            .collect();
        achadas.sort();
        reguas.extend(achadas);
    }
    let mut casos: Vec<Caso> = if !reguas.is_empty() {
        reguas
            .iter()
            .flat_map(|r| {
                let nome = r.file_name().unwrap().to_string_lossy().to_string();
                casos_da_regua(r).into_iter().map(move |mut c| {
                    c.rotulo = format!("{nome}/{}", c.rotulo);
                    c
                })
            })
            .collect()
    } else if let Some(lista) = valor("--lista") {
        casos_da_lista(Path::new(&lista))
    } else {
        panic!("--regua <pasta>, --reguas <raiz> ou --lista <csv>")
    };
    if let Some(filtro) = valor("--filtro") {
        let pedacos: Vec<&str> = filtro.split(',').collect();
        casos.retain(|c| pedacos.iter().any(|p| c.rotulo.contains(p)));
    }
    let processos: Vec<Option<f32>> = match valor("--processos") {
        Some(p) => p
            .split(',')
            .map(|x| Some(x.trim().parse().unwrap()))
            .collect(),
        None => vec![None],
    };
    let forcar_todos = valor("--forcar").map(|f| pares(&f)).unwrap_or_default();
    let imagens = valor("--imagens").map(PathBuf::from);
    if let Some(p) = &imagens {
        fs::create_dir_all(p).unwrap();
    }

    let mut motor = Motor::abrir().expect("sem GPU");
    eprintln!(
        "motor: {} — {} casos × {} processo(s)",
        motor.backend(),
        casos.len(),
        processos.len()
    );
    motor
        .definir_locais(&ParametrosLocais::default())
        .expect("locais vazios");

    let mut cabecalho = String::from("rotulo,tipo,processo,geral");
    for (f, _, _) in FAIXAS {
        let _ = write!(cabecalho, ",{f}_luma,{f}_quente,{f}_verde");
    }
    cabecalho.push_str(",de2000,de2000_p95,dl,croma,ssim");
    let mut linhas = vec![cabecalho];
    let mut resultados: Vec<Resultado> = Vec::new();
    // Os originais decodificados, e as reduções deles por tamanho.
    let mut decodificados: HashMap<PathBuf, DynamicImage> = HashMap::new();
    let mut reduzidos: HashMap<(PathBuf, u32, u32), Arc<Vec<u8>>> = HashMap::new();

    for (i, caso) in casos.iter().enumerate() {
        let bytes_lr = fs::read(&caso.exportado).expect("ler o exportado");
        let lr = image::load_from_memory(&bytes_lr).unwrap().to_rgb8();
        let (ajustes, corte) = revelacao(&bytes_lr, &caso.original);
        // Um original por vez: os casos vêm agrupados por foto, e guardar
        // todos (24 MP cada, e os RAW) esgotava a memória numa rodada inteira.
        if !decodificados.contains_key(&caso.original) {
            decodificados.clear();
            reduzidos.clear();
        }
        let original = decodificados
            .entry(caso.original.clone())
            .or_insert_with(|| base_neutra::base_neutra(&caso.original).expect("abrir o original"));
        let (w, h) = (original.width() as f32, original.height() as f32);
        // A escala que deixa o recorte no tamanho do exportado.
        let recorte = (
            w * corte.crop_width().max(0.01),
            h * corte.crop_height().max(0.01),
        );
        let escala = (lr.width().max(lr.height()) as f32 / recorte.0.max(recorte.1)).min(1.0);
        let (rw, rh) = (
            ((w * escala).round() as u32).max(1),
            ((h * escala).round() as u32).max(1),
        );
        let pixels = reduzidos
            .entry((caso.original.clone(), rw, rh))
            .or_insert_with(|| {
                Arc::new(
                    original
                        .resize_exact(rw, rh, FilterType::Triangle)
                        .to_rgba8()
                        .into_raw(),
                )
            })
            .clone();

        for processo in &processos {
            let mut a = forcar(&forcar(&ajustes, &forcar_todos), &caso.forcar);
            if let Some(p) = processo {
                a.processo = *p;
            }
            motor.definir_escala_do_original(escala);
            motor.definir_corte(&transformacao::corte(&corte));
            let revelada = motor
                .revelar(&pixels, rw, rh, &a)
                .expect("o motor não revelou");
            let nosso = transformacao::aplicar(&revelada, &corte, true).to_rgb8();
            let nosso =
                image::imageops::resize(&nosso, lr.width(), lr.height(), FilterType::Triangle);
            let rotulo_p = match processo {
                Some(p) => format!("{} p{}", caso.rotulo, p),
                None => caso.rotulo.clone(),
            };
            let medida = medir(&nosso, &lr);
            let geral: f64 = medida.split(',').next().unwrap().parse().unwrap();
            let (de, de95, dl, croma, ssim) = perceptual(&nosso, &lr);
            resultados.push(Resultado {
                caso: caso_sem_foto(&caso.rotulo),
                tipo: tipo(&caso.original),
                processo: processo.unwrap_or(-1.0),
                geral,
                de,
            });
            linhas.push(format!(
                "{},{},{},{},{de:.2},{de95:.2},{dl:.2},{croma:.3},{ssim:.4}",
                caso.rotulo.replace(',', ";"),
                tipo(&caso.original),
                processo.map(|p| p.to_string()).unwrap_or_default(),
                medida
            ));
            if let Some(pasta) = &imagens {
                let lado = 1200u32.min(lr.width());
                let alto = lado * lr.height() / lr.width();
                let mut par = RgbImage::new(lado * 2, alto);
                image::imageops::replace(
                    &mut par,
                    &image::imageops::resize(&lr, lado, alto, FilterType::Triangle),
                    0,
                    0,
                );
                image::imageops::replace(
                    &mut par,
                    &image::imageops::resize(&nosso, lado, alto, FilterType::Triangle),
                    lado as i64,
                    0,
                );
                let arquivo = rotulo_p.replace(['/', '\\', ' ', ':'], "_");
                par.save(pasta.join(format!("{arquivo}.jpg"))).unwrap();
            }
        }
        eprintln!("{}/{} {}", i + 1, casos.len(), caso.rotulo);
        // Grava a cada caso: uma régua longa interrompida não perde o que fez.
        fs::write(&saida, linhas.join("\n") + "\n").unwrap();
    }
    println!("{} linhas em {}", linhas.len() - 1, saida.display());
    resumir(
        &resultados,
        &saida,
        valor("--base").map(PathBuf::from).as_deref(),
    );
}

struct Resultado {
    caso: String,
    tipo: &'static str,
    processo: f32,
    geral: f64,
    de: f64,
}

/// `JPG` ou `RAW`, pela extensão do original: no RAW a base já difere.
fn tipo(original: &Path) -> &'static str {
    let nome = original.to_string_lossy();
    if raw_codec::eh_raw(&nome) {
        "RAW"
    } else {
        "JPG"
    }
}

/// `regua/foto/caso` → `regua/caso`: o mesmo caso em fotos diferentes.
fn caso_sem_foto(rotulo: &str) -> String {
    let partes: Vec<&str> = rotulo.split('/').collect();
    match partes.as_slice() {
        [regua, _foto, caso] => format!("{regua}/{caso}"),
        [_foto, caso] => caso.to_string(),
        _ => rotulo.to_string(),
    }
}

/// A média de cada caso (nas fotos, por tipo) em cada processo; o que piorou
/// primeiro. Com `base`, quanto cada caso mudou desde aquela rodada.
fn resumir(resultados: &[Resultado], saida: &Path, base: Option<&Path>) {
    use std::collections::BTreeMap;
    /// Por processo: (soma do geral, soma do ΔE, n).
    type Somas = BTreeMap<String, (f64, f64, f64)>;
    // (caso, tipo) → processo → somas
    let mut grupos: BTreeMap<(String, &str), Somas> = BTreeMap::new();
    for r in resultados {
        let p = if r.processo < 0.0 {
            "-".to_string()
        } else {
            format!("{}", r.processo)
        };
        let e = grupos
            .entry((r.caso.clone(), r.tipo))
            .or_default()
            .entry(p)
            .or_insert((0.0, 0.0, 0.0));
        e.0 += r.geral;
        e.1 += r.de;
        e.2 += 1.0;
    }
    let media = |m: &BTreeMap<String, (f64, f64, f64)>, p: &str| m.get(p).map(|(s, _, n)| s / n);
    let media_de = |m: &BTreeMap<String, (f64, f64, f64)>, p: &str| m.get(p).map(|(_, d, n)| d / n);
    // A rodada anterior: (caso, tipo, processo) → média.
    let anterior: HashMap<(String, String, String), f64> = base
        .map(|b| ler_resumo(&b.with_extension("resumo.csv")))
        .unwrap_or_default();

    let mut linhas: Vec<(f64, String)> = Vec::new();
    let mut csv =
        vec!["caso,tipo,processo0,processo1,diferenca,base_processo1,mudou,de0,de1".to_string()];
    let (mut melhorou, mut piorou) = (0, 0);
    for ((caso, tipo), m) in &grupos {
        let p0 = media(m, "0");
        let p1 = media(m, "1").or_else(|| media(m, "-"));
        let dif = match (p0, p1) {
            (Some(a), Some(b)) => Some(b - a),
            _ => None,
        };
        if let Some(d) = dif {
            if d < -0.5 {
                melhorou += 1;
            } else if d > 0.5 {
                piorou += 1;
            }
        }
        let antes = anterior
            .get(&(caso.clone(), tipo.to_string(), "1".to_string()))
            .copied();
        let mudou = match (antes, p1) {
            (Some(a), Some(b)) => Some(b - a),
            _ => None,
        };
        let de0 = media_de(m, "0");
        let de1 = media_de(m, "1").or_else(|| media_de(m, "-"));
        let f = |v: Option<f64>| v.map(|x| format!("{x:.1}")).unwrap_or_default();
        csv.push(format!(
            "{},{tipo},{},{},{},{},{},{},{}",
            caso.replace(',', ";"),
            f(p0),
            f(p1),
            f(dif),
            f(antes),
            f(mudou),
            f(de0),
            f(de1)
        ));
        linhas.push((
            dif.unwrap_or(0.0),
            format!(
                "{:<58} {tipo}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}",
                caso.chars().take(58).collect::<String>(),
                f(p0),
                f(p1),
                f(dif),
                f(mudou),
                f(de0),
                f(de1)
            ),
        ));
    }
    let arquivo = saida.with_extension("resumo.csv");
    fs::write(&arquivo, csv.join("\n") + "\n").unwrap();
    linhas.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!(
        "\n{:<58} tipo  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}  {:>6}",
        "caso (o que piorou primeiro)", "proc0", "proc1", "1−0", "mudou", "ΔE 0", "ΔE 1"
    );
    for (_, l) in &linhas {
        println!("{l}");
    }
    println!(
        "\n{} casos: {melhorou} melhoraram no processo 1, {piorou} pioraram — resumo em {}",
        grupos.len(),
        arquivo.display()
    );
}

fn ler_resumo(arquivo: &Path) -> HashMap<(String, String, String), f64> {
    let mut m = HashMap::new();
    let Ok(texto) = fs::read_to_string(arquivo) else {
        eprintln!("sem resumo anterior em {}", arquivo.display());
        return m;
    };
    for l in texto.lines().skip(1) {
        let c: Vec<&str> = l.split(',').collect();
        if c.len() >= 4 {
            for (p, col) in [("0", 2), ("1", 3)] {
                if let Ok(v) = c[col].parse::<f64>() {
                    m.insert((c[0].to_string(), c[1].to_string(), p.to_string()), v);
                }
            }
        }
    }
    m
}
