//! 🔄 Girar a vista (R) — a ferramenta "Girar vista" do Photoshop: a foto
//! aparece girada no palco, e **nenhum pixel nem dimensão muda**. O documento,
//! a vista do `editor-core` e o histórico nem sabem que a tela está girada.
//!
//! 🔑 **O GPUI (0.3.7) não desenha imagem girada**: só o sprite monocromático
//! (ícone) tem `transformation`; a imagem (`PolychromeSprite`) não. Então, com a
//! vista girada, o palco é montado aqui, na CPU, em ladrilhos de
//! [`LADO`] pixels do dispositivo, amostrando a vista reduzida (e a lupa,
//! quando há) com a rotação inversa. Os ladrilhos ficam guardados: um gesto
//! de pincel refaz só os que caem sobre o que ele sujou, e mudar o zoom, o
//! giro ou o tamanho do palco refaz todos.
//!
//! Com o giro em zero nada disto roda — o palco usa os ladrilhos de sempre.
//!
//! O ângulo é em radianos, positivo no sentido horário da tela (o `y` desce),
//! em torno do centro do palco — como o do Photoshop.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use editor_core::vista::Vista as VistaDoEditor;
use editor_core::Retangulo;
use gpui_kit::RenderImage;
use image::RgbImage;

/// O lado dos ladrilhos do palco girado, em pixels do dispositivo.
pub const LADO: u32 = 256;

/// O ângulo de `⇧` + arrasto: de 15 em 15 graus, como no Photoshop.
pub const PASSO_COM_SHIFT: f32 = std::f32::consts::PI / 12.0;

/// A partir de quantos pixels do dispositivo por pixel da foto cada pixel
/// aparece nítido (sem interpolar) — o mesmo limiar do palco sem giro.
pub const NITIDO_A_PARTIR_DE: f32 = crate::revelacao::zoom::PIXELS_NITIDOS_A_PARTIR_DE;

/// O ângulo de volta a `(-π, π]`.
pub fn normalizar(angulo: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let mut a = angulo % tau;
    if a <= -std::f32::consts::PI {
        a += tau;
    } else if a > std::f32::consts::PI {
        a -= tau;
    }
    a
}

/// O ângulo preso ao múltiplo de 15° mais perto (⇧).
pub fn em_passos(angulo: f32) -> f32 {
    normalizar((angulo / PASSO_COM_SHIFT).round() * PASSO_COM_SHIFT)
}

/// Gira `p` em torno de `c`.
pub fn girar(p: (f32, f32), c: (f32, f32), angulo: f32) -> (f32, f32) {
    if angulo == 0.0 {
        return p;
    }
    let (s, co) = angulo.sin_cos();
    let (dx, dy) = (p.0 - c.0, p.1 - c.1);
    (c.0 + dx * co - dy * s, c.1 + dx * s + dy * co)
}

/// Onde a foto está no palco: a posição e a escala do zoom (sem giro), o
/// tamanho do palco e o giro em torno do centro dele. Pontos do palco.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visor {
    pub vx: f32,
    pub vy: f32,
    pub escala: f32,
    pub largura: f32,
    pub altura: f32,
    pub angulo: f32,
}

impl Visor {
    pub fn centro(&self) -> (f32, f32) {
        (self.largura / 2.0, self.altura / 2.0)
    }

    /// Um pixel da foto no palco.
    pub fn da_foto(&self, x: f32, y: f32) -> (f32, f32) {
        girar(
            (self.vx + x * self.escala, self.vy + y * self.escala),
            self.centro(),
            self.angulo,
        )
    }

    /// Um ponto do palco tirado o giro — o que o zoom e a mão entendem.
    pub fn sem_giro(&self, p: (f32, f32)) -> (f32, f32) {
        girar(p, self.centro(), -self.angulo)
    }

    /// Um deslocamento na tela (a mão, a roda) tirado o giro.
    pub fn deslocamento_sem_giro(&self, dx: f32, dy: f32) -> (f32, f32) {
        girar((dx, dy), (0.0, 0.0), -self.angulo)
    }

    /// Um ponto do palco na foto.
    pub fn na_foto(&self, p: (f32, f32)) -> (f32, f32) {
        let (x, y) = self.sem_giro(p);
        ((x - self.vx) / self.escala, (y - self.vy) / self.escala)
    }

    /// O pedaço da foto que o palco mostra (a caixa dos quatro cantos do palco
    /// levados à foto), em pixels da foto, sem limitar à foto.
    pub fn caixa_na_foto(&self) -> (f32, f32, f32, f32) {
        caixa(
            [
                (0.0, 0.0),
                (self.largura, 0.0),
                (0.0, self.altura),
                (self.largura, self.altura),
            ]
            .map(|p| self.na_foto(p)),
        )
    }

    /// A caixa no palco de um retângulo da foto.
    pub fn caixa_na_tela(&self, r: &Retangulo) -> (f32, f32, f32, f32) {
        let (x0, y0, x1, y1) = (r.x as f32, r.y as f32, r.direita() as f32, r.baixo() as f32);
        caixa([(x0, y0), (x1, y0), (x0, y1), (x1, y1)].map(|(x, y)| self.da_foto(x, y)))
    }
}

fn caixa(pontos: [(f32, f32); 4]) -> (f32, f32, f32, f32) {
    pontos.iter().fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(a, b, c, d), &(x, y)| (a.min(x), b.min(y), c.max(x), d.max(y)),
    )
}

/// O segmento `a–b` recortado ao retângulo `(x0, y0, x1, y1)` (Liang–Barsky).
/// `None` quando ele fica todo fora. O letreiro, ampliado, pode ter milhares
/// de pontos de comprimento; girado, cortar só as pontas o entortaria.
pub fn recortar(
    a: (f32, f32),
    b: (f32, f32),
    (x0, y0, x1, y1): (f32, f32, f32, f32),
) -> Option<((f32, f32), (f32, f32))> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-dx, a.0 - x0),
        (dx, x1 - a.0),
        (-dy, a.1 - y0),
        (dy, y1 - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((
        (a.0 + dx * t0, a.1 + dy * t0),
        (a.0 + dx * t1, a.1 + dy * t1),
    ))
}

/// Uma vista do `editor-core` para amostrar: a imagem reduzida, o pedaço da
/// foto que ela cobre e o fator.
#[derive(Clone, Copy)]
pub struct Fonte<'a> {
    pub imagem: &'a RgbImage,
    pub regiao: Retangulo,
    pub fator: u32,
}

impl<'a> Fonte<'a> {
    pub fn da_vista(v: &'a VistaDoEditor) -> Self {
        Self {
            imagem: v.imagem(),
            regiao: v.regiao(),
            fator: v.fator(),
        }
    }

    fn cobre(&self, x: f32, y: f32) -> bool {
        let r = &self.regiao;
        x >= r.x as f32 && y >= r.y as f32 && x < r.direita() as f32 && y < r.baixo() as f32
    }

    /// A cor no pixel da foto `(x, y)`: o pixel da vista que o contém
    /// (`nitido`) ou a média dos quatro em volta.
    fn cor(&self, x: f32, y: f32, nitido: bool) -> [f32; 3] {
        let (l, a) = (self.imagem.width() as i64, self.imagem.height() as i64);
        let f = self.fator as f32;
        let u = (x - self.regiao.x as f32) / f;
        let v = (y - self.regiao.y as f32) / f;
        let px = |i: i64, j: i64| {
            let p = self
                .imagem
                .get_pixel(i.clamp(0, l - 1) as u32, j.clamp(0, a - 1) as u32)
                .0;
            [p[0] as f32, p[1] as f32, p[2] as f32]
        };
        if nitido {
            return px(u.floor() as i64, v.floor() as i64);
        }
        let (u, v) = (u - 0.5, v - 0.5);
        let (i, j) = (u.floor(), v.floor());
        let (fx, fy) = (u - i, v - j);
        let (i, j) = (i as i64, j as i64);
        let (a, b, c, d) = (px(i, j), px(i + 1, j), px(i, j + 1), px(i + 1, j + 1));
        let mut saida = [0.0; 3];
        for k in 0..3 {
            let cima = a[k] + (b[k] - a[k]) * fx;
            let baixo = c[k] + (d[k] - c[k]) * fx;
            saida[k] = cima + (baixo - cima) * fy;
        }
        saida
    }
}

/// O que um ladrilho do palco girado precisa para ser montado.
#[derive(Clone, Copy)]
pub struct Montagem<'a> {
    pub visor: Visor,
    pub dpr: f32,
    /// A foto inteira (largura, altura), em pixels.
    pub foto: (u32, u32),
    pub vista: Fonte<'a>,
    pub lupa: Option<Fonte<'a>>,
}

impl Montagem<'_> {
    /// O tamanho do palco em pixels do dispositivo.
    pub fn tamanho(&self) -> (u32, u32) {
        (
            (self.visor.largura * self.dpr).ceil().max(1.0) as u32,
            (self.visor.altura * self.dpr).ceil().max(1.0) as u32,
        )
    }

    pub fn colunas_e_linhas(&self) -> (u32, u32) {
        let (l, a) = self.tamanho();
        (l.div_ceil(LADO), a.div_ceil(LADO))
    }

    /// O ladrilho `(lx, ly)` em BGRA: `(largura, altura, bytes)`. Fora da foto
    /// é transparente (o fundo do palco aparece), e a borda da foto tem um
    /// pixel de anti-aliasing — girada, ela seria uma escada.
    pub fn ladrilho(&self, (lx, ly): (u32, u32)) -> (u32, u32, Vec<u8>) {
        let (l, a) = self.tamanho();
        let (x0, y0) = (lx * LADO, ly * LADO);
        let (largura, altura) = ((l - x0).min(LADO), (a - y0).min(LADO));
        let mut bytes = vec![0u8; (largura * altura * 4) as usize];
        let por_pixel = self.visor.escala * self.dpr;
        let nitido = por_pixel >= NITIDO_A_PARTIR_DE;
        let (fl, fa) = (self.foto.0 as f32, self.foto.1 as f32);
        for j in 0..altura {
            for i in 0..largura {
                let tela = (
                    (x0 + i) as f32 / self.dpr + 0.5 / self.dpr,
                    (y0 + j) as f32 / self.dpr + 0.5 / self.dpr,
                );
                let (x, y) = self.visor.na_foto(tela);
                // A distância até a borda da foto, em pixels do dispositivo.
                let dentro = x.min(y).min(fl - x).min(fa - y) * por_pixel;
                let cobertura = (dentro + 0.5).clamp(0.0, 1.0);
                if cobertura <= 0.0 {
                    continue;
                }
                let (xs, ys) = (x.clamp(0.0, fl - 1e-3), y.clamp(0.0, fa - 1e-3));
                let fonte = match self.lupa {
                    Some(l) if l.cobre(xs, ys) => l,
                    _ => self.vista,
                };
                let c = fonte.cor(xs, ys, nitido);
                let k = ((j * largura + i) * 4) as usize;
                let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                bytes[k] = q(c[2]);
                bytes[k + 1] = q(c[1]);
                bytes[k + 2] = q(c[0]);
                bytes[k + 3] = q(cobertura * 255.0);
            }
        }
        (largura, altura, bytes)
    }

    /// Os ladrilhos do palco que um retângulo da foto toca.
    pub fn ladrilhos_sobre(&self, r: &Retangulo) -> Vec<(u32, u32)> {
        let (x0, y0, x1, y1) = self.visor.caixa_na_tela(r);
        let (c, l) = self.colunas_e_linhas();
        let em = |v: f32, n: u32| ((v * self.dpr).floor().max(0.0) as u32 / LADO).min(n);
        let (cx0, cy0) = (em(x0 - 1.0, c), em(y0 - 1.0, l));
        let (cx1, cy1) = (em(x1 + 1.0, c - 1), em(y1 + 1.0, l - 1));
        if x1 < 0.0 || y1 < 0.0 || cx0 >= c || cy0 >= l {
            return Vec::new();
        }
        let mut v = Vec::new();
        for ly in cy0..=cy1 {
            for lx in cx0..=cx1 {
                v.push((lx, ly));
            }
        }
        v
    }
}

/// Um ladrilho montado: largura, altura e os bytes em BGRA.
type LadrilhoBgra = (u32, u32, Vec<u8>);

/// O que identifica a montagem inteira: mudou, refaz todos os ladrilhos.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Chave {
    visor: Visor,
    dpr: f32,
    vista: (Retangulo, u32, (u32, u32)),
    lupa: Option<(Retangulo, u32)>,
}

/// Os ladrilhos do palco girado, guardados entre os quadros.
#[derive(Default)]
pub struct PalcoGirado {
    chave: Option<Chave>,
    pub ladrilhos: HashMap<(u32, u32), Arc<RenderImage>>,
    /// O que os gestos sujaram desde o último quadro, em pixels da foto.
    sujo: Vec<Retangulo>,
}

impl PalcoGirado {
    /// Um gesto sujou este pedaço da foto.
    pub fn sujar(&mut self, r: Retangulo) {
        if !r.vazio() {
            self.sujo.push(r);
        }
    }

    /// Esquece tudo (o giro voltou a zero).
    pub fn largar(&mut self) {
        self.chave = None;
        self.ladrilhos.clear();
        self.sujo.clear();
    }

    /// Deixa os ladrilhos em dia com a montagem; devolve quantos refez.
    pub fn atualizar(&mut self, m: &Montagem) -> usize {
        let chave = Chave {
            visor: m.visor,
            dpr: m.dpr,
            vista: (m.vista.regiao, m.vista.fator, m.vista.imagem.dimensions()),
            lupa: m.lupa.map(|l| (l.regiao, l.fator)),
        };
        let (c, l) = m.colunas_e_linhas();
        let refazer: BTreeSet<(u32, u32)> = if self.chave != Some(chave) {
            self.chave = Some(chave);
            self.ladrilhos.clear();
            self.sujo.clear();
            (0..l).flat_map(|y| (0..c).map(move |x| (x, y))).collect()
        } else {
            std::mem::take(&mut self.sujo)
                .iter()
                .flat_map(|r| m.ladrilhos_sobre(r))
                .collect()
        };
        if refazer.is_empty() {
            return 0;
        }
        let lista: Vec<(u32, u32)> = refazer.into_iter().collect();
        // Em faixas paralelas, como a vista do `editor-core`: o palco inteiro
        // de uma tela Retina são ~5 milhões de pixels.
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        let por = lista.len().div_ceil(threads).max(1);
        let feitos: Vec<((u32, u32), LadrilhoBgra)> = std::thread::scope(|escopo| {
            let tarefas: Vec<_> = lista
                .chunks(por)
                .map(|parte| {
                    escopo.spawn(move || {
                        parte
                            .iter()
                            .map(|&t| (t, m.ladrilho(t)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            tarefas
                .into_iter()
                .flat_map(|t| t.join().unwrap_or_default())
                .collect()
        });
        let n = feitos.len();
        for (t, (largura, altura, bytes)) in feitos {
            if let Some(imagem) = crate::imagem::de_bgra(largura, altura, bytes) {
                self.ladrilhos.insert(t, imagem);
            }
        }
        n
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn visor(angulo: f32) -> Visor {
        Visor {
            vx: 50.0,
            vy: 20.0,
            escala: 0.5,
            largura: 800.0,
            altura: 600.0,
            angulo,
        }
    }

    #[test]
    fn foto_e_tela_vao_e_voltam() {
        for angulo in [0.0, 0.3, -1.2, std::f32::consts::PI] {
            let v = visor(angulo);
            for (x, y) in [(0.0, 0.0), (123.0, 456.0), (1000.0, 10.0)] {
                let (sx, sy) = v.da_foto(x, y);
                let (bx, by) = v.na_foto((sx, sy));
                assert!(
                    (bx - x).abs() < 1e-2 && (by - y).abs() < 1e-2,
                    "{angulo} {x} {y}"
                );
            }
        }
    }

    #[test]
    fn o_centro_do_palco_fica_parado() {
        // Girar a vista não move o ponto da foto que está no centro do palco.
        let sem = visor(0.0).na_foto((400.0, 300.0));
        let com = visor(0.7).na_foto((400.0, 300.0));
        assert!((sem.0 - com.0).abs() < 1e-3 && (sem.1 - com.1).abs() < 1e-3);
    }

    #[test]
    fn noventa_graus_horario() {
        // A 90° no sentido horário, o que estava à direita do centro desce.
        let v = Visor {
            angulo: std::f32::consts::FRAC_PI_2,
            ..visor(0.0)
        };
        let (cx, cy) = v.centro();
        let direita = v.sem_giro((cx, cy + 100.0));
        assert!((direita.0 - (cx + 100.0)).abs() < 1e-3 && (direita.1 - cy).abs() < 1e-3);
    }

    #[test]
    fn passos_de_15_graus_e_normalizacao() {
        let grau = std::f32::consts::PI / 180.0;
        assert!((em_passos(22.0 * grau) - 15.0 * grau).abs() < 1e-5);
        assert!((em_passos(23.0 * grau) - 30.0 * grau).abs() < 1e-5);
        assert!((normalizar(370.0 * grau) - 10.0 * grau).abs() < 1e-4);
        assert!((normalizar(-190.0 * grau) - 170.0 * grau).abs() < 1e-4);
    }

    #[test]
    fn recortar_mantem_a_direcao() {
        let r = (0.0, 0.0, 100.0, 100.0);
        assert_eq!(
            recortar((-50.0, 50.0), (150.0, 50.0), r),
            Some(((0.0, 50.0), (100.0, 50.0)))
        );
        assert_eq!(recortar((-50.0, -50.0), (-10.0, 200.0), r), None);
        let ((ax, ay), (bx, by)) = recortar((-100.0, -100.0), (200.0, 200.0), r).unwrap();
        assert!(
            (ax - ay).abs() < 1e-4 && (bx - by).abs() < 1e-4,
            "a diagonal continua diagonal"
        );
    }

    #[test]
    fn o_ladrilho_girado_amostra_a_foto_e_nao_a_muda() {
        // Uma foto 200×100: metade esquerda vermelha, direita azul; vista 1:1.
        let imagem = RgbImage::from_fn(200, 100, |x, _| {
            if x < 100 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 0, 255])
            }
        });
        let copia = imagem.clone();
        let fonte = Fonte {
            imagem: &imagem,
            regiao: Retangulo::novo(0, 0, 200, 100),
            fator: 1,
        };
        // Palco 200×200 com a foto no meio, girada 180°: o vermelho vai para a
        // direita.
        let m = Montagem {
            visor: Visor {
                vx: 0.0,
                vy: 50.0,
                escala: 1.0,
                largura: 200.0,
                altura: 200.0,
                angulo: std::f32::consts::PI,
            },
            dpr: 1.0,
            foto: (200, 100),
            vista: fonte,
            lupa: None,
        };
        let (l, a, bytes) = m.ladrilho((0, 0));
        assert_eq!((l, a), (200, 200));
        let px = |x: u32, y: u32| {
            let k = ((y * l + x) * 4) as usize;
            [bytes[k + 2], bytes[k + 1], bytes[k], bytes[k + 3]]
        };
        assert_eq!(
            px(150, 100),
            [255, 0, 0, 255],
            "o vermelho foi para a direita"
        );
        assert_eq!(px(50, 100), [0, 0, 255, 255]);
        assert_eq!(px(100, 10)[3], 0, "fora da foto é transparente");
        assert_eq!(imagem, copia, "a foto não muda");
    }

    #[test]
    fn so_os_ladrilhos_sobre_o_que_sujou() {
        let imagem = RgbImage::new(4000, 3000);
        let m = Montagem {
            visor: Visor {
                vx: 0.0,
                vy: 0.0,
                escala: 0.25,
                largura: 1000.0,
                altura: 750.0,
                angulo: 0.4,
            },
            dpr: 2.0,
            foto: (4000, 3000),
            vista: Fonte {
                imagem: &imagem,
                regiao: Retangulo::novo(0, 0, 4000, 3000),
                fator: 1,
            },
            lupa: None,
        };
        let (c, l) = m.colunas_e_linhas();
        let poucos = m.ladrilhos_sobre(&Retangulo::novo(2000, 1500, 40, 40));
        assert!(!poucos.is_empty() && poucos.len() <= 4, "{poucos:?}");
        assert!(poucos.iter().all(|&(x, y)| x < c && y < l));
        assert!(m.ladrilhos_sobre(&Retangulo::novo(0, 0, 4000, 3000)).len() as u32 <= c * l);
    }
}
