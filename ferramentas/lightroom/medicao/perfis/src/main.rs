//! A ferramenta de medição da régua do Lightroom (`docs/REGUA-DO-LIGHTROOM.md`).
//! Um projeto à parte, fora do workspace do app (tem o próprio `[workspace]`).
//!
//! ```text
//! cargo run --release -- <comando> …
//! ```
//!
//! | comando | o quê |
//! |---|---|
//! | `dump <perfil.xmp>…` | as tabelas de um perfil criativo da Adobe (`crs:Table_<md5>`): tipo, tamanho, espaço |
//! | `aplicar <perfil> <entrada> <saída> <quantidade>` | a LUT 3D do perfil numa foto |
//! | `medir <nosso> <lightroom>` | a diferença geral e por faixa de tom |
//! | `radial <com> <sem>` | a queda de luz do centro ao canto (vinheta numa foto) |
//! | `perfil <foto>` / `denso <foto>…` | o perfil da vinheta numa foto cinza (11 pontos / em d) |
//! | `denso-csv <saída> <pasta>…` | os perfis densos de uma régua, num CSV |
//! | `curva <lightroom> <nosso>` | a curva de tom na rampa cinza |
//! | `rampa-csv <saída> <arquivo>…` | a rampa colorida (4 faixas × 256 degraus), num CSV |
//! | `analise-tom <csv>` | por canal, pela luminância ou "RGB Tone" — qual prevê o Lightroom |
//! | `cantos-csv <saída> <pasta>…` | o canto de cada quadrante (a vinheta inteira) |
//! | `arredondamento <csv> <tabelas.bin>` | ajusta a forma da vinheta no arredondamento negativo |
//!
//! 🔑 As tabelas da Adobe são **lidas** dos arquivos do Lightroom instalado,
//! na hora; nenhuma delas é copiada para cá.

use std::io::Read;

const ALFABETO: &[u8] =
    b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ.-:+=^!/*?`'|()[]{}@%$#";

fn base85(texto: &str) -> Vec<u8> {
    let mut valor = [255u8; 256];
    for (i, c) in ALFABETO.iter().enumerate() {
        valor[*c as usize] = i as u8;
    }
    let digitos: Vec<u32> = texto
        .bytes()
        .filter(|c| valor[*c as usize] != 255)
        .map(|c| valor[c as usize] as u32)
        .collect();
    let mut saida = Vec::new();
    for grupo in digitos.chunks(5) {
        let mut x: u64 = 0;
        let mut p: u64 = 1;
        for d in grupo {
            x += *d as u64 * p;
            p *= 85;
        }
        let bytes = (x as u32).to_le_bytes();
        let n = if grupo.len() == 5 { 4 } else { grupo.len() - 1 };
        saida.extend_from_slice(&bytes[..n]);
    }
    saida
}

fn tabela(texto: &str) -> Vec<u8> {
    let bruto = base85(texto);
    let tamanho = u32::from_le_bytes(bruto[0..4].try_into().unwrap()) as usize;
    let mut saida = Vec::new();
    flate2::read::ZlibDecoder::new(&bruto[4..])
        .read_to_end(&mut saida)
        .expect("zlib");
    assert_eq!(saida.len(), tamanho, "tamanho descomprimido");
    saida
}

struct Leitor<'a> {
    b: &'a [u8],
    i: usize,
}
impl Leitor<'_> {
    fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.b[self.i..self.i + 4].try_into().unwrap());
        self.i += 4;
        v
    }
    fn u16(&mut self) -> u16 {
        let v = u16::from_le_bytes(self.b[self.i..self.i + 2].try_into().unwrap());
        self.i += 2;
        v
    }
    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    fn f64(&mut self) -> f64 {
        let v = f64::from_le_bytes(self.b[self.i..self.i + 8].try_into().unwrap());
        self.i += 8;
        v
    }
    fn resto(&self) -> usize {
        self.b.len() - self.i
    }
}

fn tabelas_do_perfil(xmp: &str) -> Vec<(String, Vec<u8>)> {
    let mut v = Vec::new();
    let mut resto = xmp;
    while let Some(i) = resto.find("crs:Table_") {
        let depois = &resto[i + 10..];
        let fim = depois.find('=').unwrap();
        let id = depois[..fim].to_string();
        let aspas = &depois[fim + 2..];
        let fecho = aspas.find('"').unwrap();
        v.push((id, tabela(&aspas[..fecho])));
        resto = &aspas[fecho..];
    }
    v
}

fn atributo(xmp: &str, nome: &str) -> Option<String> {
    let chave = format!("crs:{nome}=\"");
    let depois = xmp.split(&chave).nth(1)?;
    Some(depois.split('"').next()?.to_string())
}

/// A tabela RGB 3D: amostras em u16 (0..65535) por [r][g][b].
struct Rgb {
    divisoes: usize,
    amostras: Vec<[f32; 3]>,
    primarias: u32,
    gama: u32,
    gamut: u32,
    min: f64,
    max: f64,
}

fn ler_rgb(b: &[u8], verboso: bool) -> Rgb {
    let mut l = Leitor { b, i: 0 };
    let tipo = l.u32();
    let versao = l.u32();
    let dimensoes = l.u32();
    let divisoes = l.u32() as usize;
    if verboso {
        println!("  tipo {tipo} versão {versao} dimensões {dimensoes} divisões {divisoes}");
    }
    assert_eq!(dimensoes, 3);
    let n = divisoes * divisoes * divisoes;
    let mut amostras = Vec::with_capacity(n);
    let nominal = |i: usize| ((i * 0xFFFF + (divisoes - 1) / 2) / (divisoes - 1)) as i32;
    let mut maior_delta = 0i32;
    for r in 0..divisoes {
        for g in 0..divisoes {
            for bb in 0..divisoes {
                let mut s = [0f32; 3];
                for (c, idx) in [r, g, bb].into_iter().enumerate() {
                    let delta = l.u16() as i16 as i32;
                    maior_delta = maior_delta.max(delta.abs());
                    let v = (nominal(idx) + delta) as u16;
                    s[c] = v as f32 / 65535.0;
                }
                amostras.push(s);
            }
        }
    }
    let primarias = l.u32();
    let gama = l.u32();
    let gamut = l.u32();
    let min = l.f64();
    let max = l.f64();
    if verboso {
        println!(
            "  maior delta {maior_delta}  primárias {primarias} gama {gama} gamut {gamut} min {min} max {max}  sobram {} bytes",
            l.resto()
        );
    }
    Rgb {
        divisoes,
        amostras,
        primarias,
        gama,
        gamut,
        min,
        max,
    }
}

fn dump(caminho: &str) {
    let xmp = std::fs::read_to_string(caminho).unwrap();
    println!("== {caminho}");
    println!(
        "  LookTable {:?}  RGBTable {:?}  RGBTableAmount {:?}",
        atributo(&xmp, "LookTable"),
        atributo(&xmp, "RGBTable"),
        atributo(&xmp, "RGBTableAmount")
    );
    for (id, b) in tabelas_do_perfil(&xmp) {
        let mut l = Leitor { b: &b, i: 0 };
        let tipo = l.u32();
        println!("  Table_{id}: {} bytes, tipo {tipo}", b.len());
        if tipo == 1 {
            ler_rgb(&b, true);
        } else {
            let versao = l.u32();
            let (h, s, v) = (l.u32(), l.u32(), l.u32());
            println!("  hue/sat/val {h}×{s}×{v} (versão {versao})");
            let n = (h * s * v) as usize;
            let mut maior = [0f32; 3];
            for _ in 0..n {
                let e = [l.f32(), l.f32(), l.f32()];
                maior[0] = maior[0].max(e[0].abs());
                maior[1] = maior[1].max((e[1] - 1.0).abs());
                maior[2] = maior[2].max((e[2] - 1.0).abs());
            }
            println!(
                "  maior |Δmatiz| {:.2}°  |Δsat| {:.3}  |Δval| {:.3}  sobram {} bytes",
                maior[0],
                maior[1],
                maior[2],
                l.resto()
            );
            let extra: Vec<u32> = (0..l.resto() / 4).map(|_| l.u32()).collect();
            println!("  resto como u32: {extra:?}");
        }
    }
}

fn srgb_para_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn linear_para_srgb(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

// sRGB linear <-> Adobe RGB (1998) linear, ambos D65.
const SRGB_PARA_ADOBE: [[f32; 3]; 3] = [
    [0.715_1, 0.284_9, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.041_2, 0.958_8],
];
const ADOBE_PARA_SRGB: [[f32; 3]; 3] = [
    [1.398_4, -0.398_4, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, -0.042_9, 1.042_9],
];

fn mul(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn gama(g: u32, v: f32, codificar: bool) -> f32 {
    let v = v.clamp(0.0, 1.0);
    let expoente = match g {
        0 => return v,
        1 => return if codificar { linear_para_srgb(v) } else { srgb_para_linear(v) },
        2 => 1.8,
        _ => 2.2,
    };
    if codificar {
        v.powf(1.0 / expoente)
    } else {
        v.powf(expoente)
    }
}

impl Rgb {
    /// Interpolação trilinear.
    fn amostrar(&self, p: [f32; 3]) -> [f32; 3] {
        let n = self.divisoes;
        let escala = (n - 1) as f32;
        let pos = p.map(|v| (v.clamp(0.0, 1.0) * escala).min(escala - 1e-4));
        let i = pos.map(|v| v.floor() as usize);
        let f = [pos[0] - i[0] as f32, pos[1] - i[1] as f32, pos[2] - i[2] as f32];
        let at = |r: usize, g: usize, b: usize| self.amostras[(r * n + g) * n + b];
        let mut s = [0f32; 3];
        for dr in 0..2 {
            for dg in 0..2 {
                for db in 0..2 {
                    let w = (if dr == 1 { f[0] } else { 1.0 - f[0] })
                        * (if dg == 1 { f[1] } else { 1.0 - f[1] })
                        * (if db == 1 { f[2] } else { 1.0 - f[2] });
                    let a = at(i[0] + dr, i[1] + dg, i[2] + db);
                    for c in 0..3 {
                        s[c] += w * a[c];
                    }
                }
            }
        }
        s
    }

    /// Um pixel sRGB (0..1, com gama) pela tabela, na quantidade.
    fn aplicar(&self, srgb: [f32; 3], quantidade: f32) -> [f32; 3] {
        assert!(self.primarias == 1, "só Adobe RGB por enquanto");
        let linear = srgb.map(srgb_para_linear);
        let adobe = mul(&SRGB_PARA_ADOBE, linear);
        let entrada = adobe.map(|v| gama(self.gama, v, true));
        let saida = self.amostrar(entrada);
        let misturado = [0, 1, 2].map(|c| entrada[c] + (saida[c] - entrada[c]) * quantidade);
        let adobe = misturado.map(|v| gama(self.gama, v, false));
        mul(&ADOBE_PARA_SRGB, adobe).map(linear_para_srgb)
    }
}

fn aplicar(perfil: &str, entrada: &str, saida: &str, quantidade: f32) {
    let xmp = std::fs::read_to_string(perfil).unwrap();
    let id = atributo(&xmp, "RGBTable").expect("perfil sem RGBTable");
    let rgb = tabelas_do_perfil(&xmp)
        .into_iter()
        .find(|(i, _)| *i == id)
        .map(|(_, b)| ler_rgb(&b, false))
        .unwrap();
    let _ = (rgb.gamut, rgb.min, rgb.max);
    let mut img = image::open(entrada).unwrap().to_rgb8();
    for p in img.pixels_mut() {
        let s = rgb.aplicar([p[0], p[1], p[2]].map(|v| v as f32 / 255.0), quantidade);
        *p = image::Rgb(s.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8));
    }
    img.save(saida).unwrap();
}

fn luma(p: &image::Rgb<u8>) -> f64 {
    0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64
}

fn medir(nosso: &str, lr: &str) {
    let lr = image::open(lr).unwrap().to_rgb8();
    let nosso = image::open(nosso).unwrap().to_rgb8();
    let nosso = image::imageops::resize(
        &nosso,
        lr.width(),
        lr.height(),
        image::imageops::FilterType::Triangle,
    );
    let mut soma = [0f64; 3];
    for (a, b) in nosso.pixels().zip(lr.pixels()) {
        for c in 0..3 {
            soma[c] += a[c].abs_diff(b[c]) as f64;
        }
    }
    let n = (lr.width() * lr.height()) as f64;
    println!(
        "|Δ| médio R G B {:.1} {:.1} {:.1}  (geral {:.1})",
        soma[0] / n,
        soma[1] / n,
        soma[2] / n,
        (soma[0] + soma[1] + soma[2]) / n / 3.0
    );
    println!(
        "{:9} {:>7} {:>8} {:>8} {:>8} {:>8}",
        "faixa", "pixels", "Δluma", "ΔR−B", "ΔG", "|Δ|"
    );
    for (nome, de, ate) in [
        ("pretos", 0.0, 32.0),
        ("sombras", 32.0, 96.0),
        ("médios", 96.0, 160.0),
        ("realces", 160.0, 224.0),
        ("brancos", 224.0, 256.0),
    ] {
        let (mut k, mut dl, mut dq, mut dv, mut ab) = (0u64, 0.0, 0.0, 0.0, 0.0);
        for (a, b) in nosso.pixels().zip(lr.pixels()) {
            let l = luma(b);
            if l < de || l >= ate {
                continue;
            }
            let q = |p: &image::Rgb<u8>| p[0] as f64 - p[2] as f64;
            let v = |p: &image::Rgb<u8>| p[1] as f64 - (p[0] as f64 + p[2] as f64) / 2.0;
            k += 1;
            dl += luma(a) - l;
            dq += q(a) - q(b);
            dv += v(a) - v(b);
            ab += (0..3).map(|c| a[c].abs_diff(b[c]) as f64).sum::<f64>() / 3.0;
        }
        if k > 0 {
            let m = k as f64;
            println!(
                "{nome:9} {:>6.1}% {:>8.1} {:>8.1} {:>8.1} {:>8.1}",
                m / n * 100.0,
                dl / m,
                dq / m,
                dv / m,
                ab / m
            );
        }
    }
}

/// A queda de luz do centro para a borda: luma(com) / luma(sem), por anel do
/// raio normalizado (0 = centro, 1 = canto), no referencial de `sem`.
fn radial(com: &str, sem: &str) -> Vec<f64> {
    let sem = image::open(sem).unwrap().to_rgb8();
    let com = image::imageops::resize(
        &image::open(com).unwrap().to_rgb8(),
        sem.width(),
        sem.height(),
        image::imageops::FilterType::Triangle,
    );
    let (w, h) = (sem.width() as f64, sem.height() as f64);
    let mut soma = [(0f64, 0f64); 10];
    for (x, y, b) in sem.enumerate_pixels() {
        let a = com.get_pixel(x, y);
        let dx = (x as f64 + 0.5 - w / 2.0) / (w / 2.0);
        let dy = (y as f64 + 0.5 - h / 2.0) / (h / 2.0);
        let r = ((dx * dx + dy * dy) / 2.0).sqrt().min(0.9999);
        let i = (r * 10.0) as usize;
        soma[i].0 += luma(a);
        soma[i].1 += luma(b);
    }
    soma.iter().map(|(a, b)| a / b.max(1.0)).collect()
}

/// O valor (luma, 0..255) do centro à borda em 11 pontos: na horizontal, na
/// vertical e na diagonal. Média de uma janela pequena, contra o ruído do JPEG.
fn perfil(caminho: &str) -> [[f64; 11]; 3] {
    let img = image::open(caminho).unwrap().to_rgb8();
    let (w, h) = (img.width() as f64, img.height() as f64);
    let media = |cx: f64, cy: f64| {
        let r = 3i64;
        let (mut s, mut n) = (0.0, 0.0);
        for dy in -r..=r {
            for dx in -r..=r {
                let x = (cx as i64 + dx).clamp(0, w as i64 - 1) as u32;
                let y = (cy as i64 + dy).clamp(0, h as i64 - 1) as u32;
                s += luma(img.get_pixel(x, y));
                n += 1.0;
            }
        }
        s / n
    };
    let mut saida = [[0.0; 11]; 3];
    for i in 0..=10 {
        let t = i as f64 / 10.0;
        let (cx, cy) = (w / 2.0, h / 2.0);
        saida[0][i] = media(cx + t * (w / 2.0 - 1.0), cy);
        saida[1][i] = media(cx, cy + t * (h / 2.0 - 1.0));
        saida[2][i] = media(cx + t * (w / 2.0 - 1.0), cy + t * (h / 2.0 - 1.0));
    }
    saida
}

/// Luma em função da distância elíptica d (0 = centro, 1 = meio da borda,
/// 1,414 = canto), de 0,05 em 0,05: pela horizontal até d = 1 e pela
/// diagonal dali em diante. Também devolve a vertical, para a forma.
fn denso(caminho: &str) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let img = image::open(caminho).unwrap().to_rgb8();
    let (w, h) = (img.width() as f64, img.height() as f64);
    let media = |cx: f64, cy: f64| {
        let (mut s, mut n) = (0.0, 0.0);
        for dy in -2i64..=2 {
            for dx in -2i64..=2 {
                let x = (cx as i64 + dx).clamp(0, w as i64 - 1) as u32;
                let y = (cy as i64 + dy).clamp(0, h as i64 - 1) as u32;
                s += luma(img.get_pixel(x, y));
                n += 1.0;
            }
        }
        s / n
    };
    let (cx, cy) = (w / 2.0, h / 2.0);
    let passos: Vec<f64> = (0..=28).map(|i| i as f64 * 0.05).collect();
    let horiz = passos
        .iter()
        .filter(|d| **d <= 1.0)
        .map(|d| media(cx + d * (cx - 1.0), cy))
        .collect();
    let vert = passos
        .iter()
        .filter(|d| **d <= 1.0)
        .map(|d| media(cx, cy + d * (cy - 1.0)))
        .collect();
    let diag = passos
        .iter()
        .map(|d| {
            let t = (d / std::f64::consts::SQRT_2).min(1.0);
            media(cx + t * (cx - 1.0), cy + t * (cy - 1.0))
        })
        .collect();
    (horiz, vert, diag)
}

/// A rampa de 256 degraus (da esquerda para a direita): a luma média do miolo
/// de cada degrau, longe das emendas e das bordas de cima e de baixo.
fn curva(caminho: &str) -> Vec<f64> {
    let img = image::open(caminho).unwrap().to_rgb8();
    let (w, h) = (img.width() as f64, img.height());
    (0..256)
        .map(|i| {
            let x0 = ((i as f64 + 0.3) * w / 256.0) as u32;
            let x1 = (((i as f64 + 0.7) * w / 256.0) as u32).max(x0 + 1);
            let (mut s, mut n) = (0.0, 0.0);
            for y in (h * 2 / 5)..(h * 3 / 5) {
                for x in x0..x1 {
                    s += luma(img.get_pixel(x, y));
                    n += 1.0;
                }
            }
            s / n
        })
        .collect()
}

/// A rampa colorida (4 faixas horizontais × 256 degraus): a média RGB do miolo
/// de cada degrau de cada faixa.
fn rampa_cor(caminho: &str) -> Vec<[[f64; 3]; 256]> {
    let img = image::open(caminho).unwrap().to_rgb8();
    let (w, h) = (img.width() as f64, img.height() as f64);
    (0..4)
        .map(|f| {
            let y0 = ((f as f64 + 0.3) * h / 4.0) as u32;
            let y1 = ((f as f64 + 0.7) * h / 4.0) as u32;
            let mut faixa = [[0.0; 3]; 256];
            for (i, saida) in faixa.iter_mut().enumerate() {
                let x0 = ((i as f64 + 0.3) * w / 256.0) as u32;
                let x1 = (((i as f64 + 0.7) * w / 256.0) as u32).max(x0 + 1);
                let mut s = [0.0; 3];
                let mut n = 0.0;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let p = img.get_pixel(x, y);
                        for c in 0..3 {
                            s[c] += p[c] as f64;
                        }
                        n += 1.0;
                    }
                }
                *saida = s.map(|v| v / n);
            }
            faixa
        })
        .collect()
}

type Rampa = std::collections::BTreeMap<String, Vec<[[f64; 3]; 256]>>;

fn ler_rampa(csv: &str) -> Rampa {
    let mut m: Rampa = Default::default();
    for l in std::fs::read_to_string(csv).unwrap().lines().skip(1) {
        let c: Vec<&str> = l.split(',').collect();
        let e = m.entry(c[0].to_string()).or_insert_with(|| vec![[[0.0; 3]; 256]; 4]);
        let (f, i): (usize, usize) = (c[1].parse().unwrap(), c[2].parse().unwrap());
        e[f][i] = [c[3].parse().unwrap(), c[4].parse().unwrap(), c[5].parse().unwrap()];
    }
    m
}

/// Por canal ou pela luminância? Para cada caso, a curva da faixa cinza
/// prevê as faixas coloridas dos dois jeitos; o erro médio diz qual é.
fn analise_tom(csv: &str) {
    let m = ler_rampa(csv);
    let neutro = m.iter().find(|(k, _)| k.contains("neutro")).map(|(_, v)| v.clone()).unwrap();
    // A curva do caso, como função do valor de entrada (degrau i do cinza).
    let interp = |c: &[[f64; 3]; 256], x: f64| {
        let x = x.clamp(0.0, 255.0);
        let i = (x.floor() as usize).min(254);
        let t = x - i as f64;
        c[i][1] * (1.0 - t) + c[i + 1][1] * t
    };
    // O "RGB Tone" do DNG SDK: a curva no maior e no menor canal, e o do meio
    // interpolado para guardar o matiz. Em `linear`, a interpolação é em luz
    // linear (os valores voltam ao sRGB no fim).
    let rgb_tone = |ent: [f64; 3], curva: &dyn Fn(f64) -> f64, linear: bool| {
        let para = |v: f64| if linear { srgb_para_linear((v / 255.0) as f32) as f64 } else { v };
        let de = |v: f64| if linear { linear_para_srgb(v as f32) as f64 * 255.0 } else { v };
        let mut idx = [0usize, 1, 2];
        idx.sort_by(|a, b| ent[*a].partial_cmp(&ent[*b]).unwrap());
        let (lo, mi, hi) = (idx[0], idx[1], idx[2]);
        let mut sai = [0.0; 3];
        sai[hi] = curva(ent[hi]);
        sai[lo] = curva(ent[lo]);
        let (a, b, c) = (para(ent[lo]), para(ent[mi]), para(ent[hi]));
        let (a2, c2) = (para(sai[lo]), para(sai[hi]));
        let t = if c - a > 1e-6 { (b - a) / (c - a) } else { 0.0 };
        sai[mi] = de(a2 + (c2 - a2) * t);
        sai
    };
    println!(
        "{:34} {:>8} {:>8} {:>8} {:>8}   (erro médio, níveis de 0 a 255)",
        "caso", "canal", "luma", "rgbtone", "rgbt-lin"
    );
    for (nome, faixas) in &m {
        let cinza = &faixas[0];
        let (mut e_canal, mut e_luma, mut n) = (0.0, 0.0, 0.0);
        let (mut e_rt, mut e_rtl) = (0.0, 0.0);
        for f in 1..4 {
            for i in 8..248 {
                // a entrada é o que o neutro do Lightroom devolveu nesse degrau
                let ent = neutro[f][i];
                let sai = faixas[f][i];
                let canal = ent.map(|v| interp(cinza, v));
                let l_ent = 0.2126 * ent[0] + 0.7152 * ent[1] + 0.0722 * ent[2];
                let l_sai = interp(cinza, l_ent);
                let k = if l_ent > 0.5 { l_sai / l_ent } else { 1.0 };
                let luma = ent.map(|v| (v * k).min(255.0));
                let curva = |v: f64| interp(cinza, v);
                let rt = rgb_tone(ent, &curva, false);
                let rtl = rgb_tone(ent, &curva, true);
                for c in 0..3 {
                    e_canal += (canal[c] - sai[c]).abs();
                    e_luma += (luma[c] - sai[c]).abs();
                    e_rt += (rt[c] - sai[c]).abs();
                    e_rtl += (rtl[c] - sai[c]).abs();
                }
                n += 3.0;
            }
        }
        println!(
            "{nome:34} {:8.2} {:8.2} {:8.2} {:8.2}",
            e_canal / n,
            e_luma / n,
            e_rt / n,
            e_rtl / n
        );
    }
}

/// Ajusta a forma da vinheta no arredondamento negativo: para cada valor
/// medido, a mistura elipse→caixa `a`, o expoente `n` da superelipse e a
/// compressão `k` da faixa (`ρ = 1 − (1 − d)·k`), contra os perfis do
/// Lightroom (horizontal, vertical e diagonal), numa foto 3:2.
fn ajustar_arredondamento(csv: &str, tabelas: &str) {
    let bruto = std::fs::read(tabelas).unwrap();
    let t: Vec<f32> = bruto.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    let linha = |l: usize, x: f64| {
        let p = x.clamp(0.0, 255.0);
        let i = (p.floor() as usize).min(254);
        let f = p - i as f64;
        t[l * 256 + i] as f64 * (1.0 - f) + t[l * 256 + i + 1] as f64 * f
    };
    // a máscara de m50/f50 (linha 164 + 4·9 + 4) em ρ; a força em −61·m
    let mascara = |rho: f64| linha(164 + 40, rho / 1.45 * 255.0);
    let forca = |a: f64| {
        let pos = ((a + 100.0) / 10.0).clamp(0.0, 20.0);
        let i = (pos.floor() as usize).min(19);
        let f = pos - i as f64;
        linha(122 + i, 128.0) * (1.0 - f) + linha(122 + i + 1, 128.0) * f
    };
    let (cx, cy) = (1.5f64, 1.0f64);
    let s = cx.min(cy);
    let prever = |px: f64, py: f64, a: f64, n: f64, k: f64| {
        let (rx, ry) = (px * cx, py * cy);
        let (ex, ey) = (rx / cx, ry / cy);
        let bx = ((rx - (cx - s)).max(0.0)) / s;
        let by = ((ry - (cy - s)).max(0.0)) / s;
        let (qx, qy) = (ex + (bx - ex) * a, ey + (by - ey) * a);
        let d = (qx.powf(n) + qy.powf(n)).powf(1.0 / n);
        let rho = 1.0 - (1.0 - d) * k;
        forca(-61.0 * mascara(rho.max(0.0)))
    };
    for l in std::fs::read_to_string(csv).unwrap().lines() {
        let c: Vec<&str> = l.split(',').collect();
        let Some(r) = c[0].split("arred").nth(1) else { continue };
        let r: f64 = r.parse().unwrap();
        let v: Vec<f64> = c[1..].iter().map(|x| x.parse().unwrap()).collect();
        let (h, vv, dg) = (&v[0..21], &v[21..42], &v[42..71]);
        let erro = |a: f64, n: f64, k: f64| {
            let mut e = 0.0;
            for i in 0..21 {
                let tt = i as f64 * 0.05;
                e += (prever(tt, 0.0, a, n, k) - h[i]).powi(2);
                e += (prever(0.0, tt, a, n, k) - vv[i]).powi(2);
            }
            for (i, valor) in dg.iter().enumerate() {
                let tt = (i as f64 * 0.05 / std::f64::consts::SQRT_2).min(1.0);
                e += (prever(tt, tt, a, n, k) - valor).powi(2);
            }
            (e / 71.0).sqrt()
        };
        let mut melhor = (f64::MAX, 0.0, 0.0, 0.0);
        for ia in (if std::env::var("A_FIXO").is_ok() { 20 } else { 0 })..=20 {
            let a = ia as f64 / 20.0;
            for in_ in 0..=40 {
                let n = 2.0 + in_ as f64 * 0.5;
                for ik in 0..=60 {
                    let k = 1.0 + ik as f64 * 0.15;
                    let e = erro(a, n, k);
                    if e < melhor.0 {
                        melhor = (e, a, n, k);
                    }
                }
            }
        }
        println!(
            "arredondamento {r:+5}: a={:.2} n={:.1} k={:.2}  erro {:.2} (r=0 com a=0,n=2,k=1: {:.2})",
            melhor.1,
            melhor.2,
            melhor.3,
            melhor.0,
            erro(0.0, 2.0, 1.0)
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args[0].as_str() {
        "dump" => args[1..].iter().for_each(|c| dump(c)),
        "aplicar" => aplicar(&args[1], &args[2], &args[3], args[4].parse().unwrap()),
        "medir" => medir(&args[1], &args[2]),
        // curva <lightroom> <nosso>: a saída nos degraus 0, 16, 32 … 255.
        "curva" => {
            let a = curva(&args[1]);
            let b = curva(&args[2]);
            let pontos = [0usize, 8, 16, 32, 48, 64, 96, 128, 160, 192, 224, 240, 255];
            let f = |c: &Vec<f64>| pontos.iter().map(|i| format!("{:5.0}", c[*i])).collect::<Vec<_>>().join(" ");
            println!("  LR    {}", f(&a));
            println!("  nosso {}", f(&b));
        }
        // denso-csv <saída.csv> <pasta>…: uma linha por arquivo, com a
        // horizontal (21 pontos, d até 1) e a diagonal (29, d até 1,4).
        "denso-csv" => {
            let mut linhas = Vec::new();
            for pasta in &args[2..] {
                let mut arquivos: Vec<_> = std::fs::read_dir(pasta)
                    .unwrap()
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|x| x == "jpg"))
                    .collect();
                arquivos.sort();
                for a in arquivos {
                    let (h, v, d) = denso(a.to_str().unwrap());
                    let rotulo = format!(
                        "{}/{}",
                        a.parent().unwrap().file_name().unwrap().to_string_lossy(),
                        a.file_stem().unwrap().to_string_lossy()
                    );
                    let nums: Vec<String> = h.iter().chain(v.iter()).chain(d.iter()).map(|x| format!("{x:.2}")).collect();
                    linhas.push(format!("{rotulo},{}", nums.join(",")));
                }
            }
            std::fs::write(&args[1], linhas.join("\n")).unwrap();
            println!("{} linhas", linhas.len());
        }
        // rampa-csv <saída.csv> <arquivo>…: caso,faixa,degrau,R,G,B
        "rampa-csv" => {
            let mut linhas = vec!["caso,faixa,degrau,r,g,b".to_string()];
            for a in &args[2..] {
                let nome = std::path::Path::new(a).file_stem().unwrap().to_string_lossy().to_string();
                for (f, faixa) in rampa_cor(a).iter().enumerate() {
                    for (i, p) in faixa.iter().enumerate() {
                        linhas.push(format!("{nome},{f},{i},{:.2},{:.2},{:.2}", p[0], p[1], p[2]));
                    }
                }
            }
            std::fs::write(&args[1], linhas.join("\n")).unwrap();
            println!("{} linhas", linhas.len() - 1);
        }
        "analise-tom" => analise_tom(&args[1]),
        "arredondamento" => ajustar_arredondamento(&args[1], &args[2]),
        // cantos-csv <saída.csv> <pasta>…: caso,foto,quadrante,R,G,B — o
        // canto de cada quadrante, onde a vinheta age por inteiro.
        "cantos-csv" => {
            let mut linhas = vec!["caso,foto,quadrante,r,g,b".to_string()];
            for pasta in &args[2..] {
                let foto = std::path::Path::new(pasta).file_name().unwrap().to_string_lossy().to_string();
                let mut arquivos: Vec<_> = std::fs::read_dir(pasta)
                    .unwrap()
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|x| x == "jpg"))
                    .collect();
                arquivos.sort();
                for a in arquivos {
                    let img = image::open(&a).unwrap().to_rgb8();
                    let (w, h) = (img.width(), img.height());
                    let caso = a.file_stem().unwrap().to_string_lossy().to_string();
                    for (q, (x0, y0)) in [(1, 1), (w - 5, 1), (1, h - 5), (w - 5, h - 5)].into_iter().enumerate() {
                        let mut s = [0.0f64; 3];
                        for y in y0..y0 + 4 {
                            for x in x0..x0 + 4 {
                                let p = img.get_pixel(x, y);
                                for c in 0..3 {
                                    s[c] += p[c] as f64 / 16.0;
                                }
                            }
                        }
                        linhas.push(format!("{caso},{foto},{q},{:.2},{:.2},{:.2}", s[0], s[1], s[2]));
                    }
                }
            }
            std::fs::write(&args[1], linhas.join("\n")).unwrap();
            println!("{} linhas", linhas.len() - 1);
        }
        "denso" => {
            for c in &args[1..] {
                let (h, v, d) = denso(c);
                let nome = std::path::Path::new(c).file_stem().unwrap().to_string_lossy();
                let f = |x: &Vec<f64>| x.iter().map(|v| format!("{v:5.1}")).collect::<Vec<_>>().join(" ");
                println!("{nome}\n  h {}\n  v {}\n  d {}", f(&h), f(&v), f(&d));
            }
        }
        "perfil" => {
            for (rotulo, linha) in ["horiz", "vert", "diag"].iter().zip(perfil(&args[1])) {
                println!(
                    "{rotulo:5} {}",
                    linha.iter().map(|v| format!("{v:5.1}")).collect::<Vec<_>>().join(" ")
                );
            }
        }
        "radial" => {
            let v = radial(&args[1], &args[2]);
            println!(
                "{}",
                v.iter()
                    .map(|x| format!("{x:.3}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        _ => panic!("dump <perfil.xmp>… | aplicar <perfil> <entrada> <saída> <quantidade> | medir <nosso> <lr>"),
    }
}
