//! O zoom da revelação — a conta, separada de quem desenha e de quem ouve o
//! mouse. É o `zoom.ts` do site, com os mesmos nomes e as mesmas paradas.
//!
//! *"Estou sentindo falta de zoom no modo revelação na web, mas precisa ser
//! poderoso igual no Lightroom"* (dono, 2026-09-14, no site; aqui pela regra de
//! paridade, 2026-09-17: *"Não tem a tela de zoom e revelação"*).
//!
//! # As unidades
//!
//! - **pixels da janela**: a foto que o palco mostra (já recortada);
//! - **pontos da área**: onde está o mouse, com a origem no canto da área;
//! - **razão**: pixels do dispositivo por pixel da foto — o "1:1" do
//!   Lightroom. O nível guardado é a razão, e não a escala: é o que deixa andar
//!   pela tira com a seta e continuar em 1:1.
//!
//! ⚠️ **O `fator_do_bruto` aqui é 1.** O desktop revela o preview do cache, e
//! não tem o bruto em resolução cheia para trocar na hora do 1:1 como o site.
//! Acima do 1:1 o que aparece é a foto ampliada, como no site antes de o bruto
//! carregar.

/// Um ponto, em pontos da área ou normalizado (0–1), conforme o uso.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Ponto {
    pub x: f32,
    pub y: f32,
}

impl Ponto {
    pub const CENTRO: Ponto = Ponto { x: 0.5, y: 0.5 };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Medidas {
    pub largura: f32,
    pub altura: f32,
}

/// `Encaixar` e `Preencher` dependem da área; a razão é fixa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Nivel {
    Encaixar,
    Preencher,
    Razao(f32),
}

/// O que a conta precisa saber da tela e da foto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cena {
    /// A foto que o palco mostra, em pixels.
    pub janela: Medidas,
    /// A área útil do palco, em pontos.
    pub area: Medidas,
    /// Pixels do dispositivo por ponto.
    pub dpr: f32,
    /// Quantos pixels do bruto cabem num pixel da janela (≥ 1).
    pub fator_do_bruto: f32,
}

/// Onde a janela está na área: a escala e o canto dela, e o centro já trazido
/// para dentro dos limites.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vista {
    pub escala: f32,
    pub x: f32,
    pub y: f32,
    pub centro: Ponto,
}

/// O retângulo visível, normalizado — o do navegador.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retangulo {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// As razões do Lightroom Classic, com 1:8 e 16:1 nas pontas.
pub const RAZOES: [f32; 11] = [
    1. / 8.,
    1. / 4.,
    1. / 3.,
    1. / 2.,
    1.,
    2.,
    3.,
    4.,
    8.,
    11.,
    16.,
];
const RAZAO_MINIMA: f32 = RAZOES[0];
const RAZAO_MAXIMA: f32 = RAZOES[RAZOES.len() - 1];
const QUASE: f32 = 0.005;

fn perto(a: f32, b: f32) -> bool {
    (a - b).abs() <= a.max(b) * QUASE
}

pub fn escala_de_encaixar(c: &Cena) -> f32 {
    (c.area.largura / c.janela.largura).min(c.area.altura / c.janela.altura)
}

pub fn escala_de_preencher(c: &Cena) -> f32 {
    (c.area.largura / c.janela.largura).max(c.area.altura / c.janela.altura)
}

pub fn escala_da_razao(razao: f32, c: &Cena) -> f32 {
    razao * c.fator_do_bruto / c.dpr
}

pub fn razao_da_escala(escala: f32, c: &Cena) -> f32 {
    escala * c.dpr / c.fator_do_bruto
}

/// O de baixo nunca impede o encaixe, e o de cima nunca impede o preencher.
pub fn limitar_escala(escala: f32, c: &Cena) -> f32 {
    let minima = escala_de_encaixar(c).min(escala_da_razao(RAZAO_MINIMA, c));
    let maxima = escala_de_preencher(c).max(escala_da_razao(RAZAO_MAXIMA, c));
    escala.max(minima).min(maxima)
}

pub fn escala_do_nivel(nivel: Nivel, c: &Cena) -> f32 {
    match nivel {
        Nivel::Encaixar => escala_de_encaixar(c),
        Nivel::Preencher => escala_de_preencher(c),
        Nivel::Razao(r) => limitar_escala(escala_da_razao(r, c), c),
    }
}

/// O nível que uma escala qualquer representa. Encostou no encaixe, é encaixe.
pub fn nivel_da_escala(escala: f32, c: &Cena) -> Nivel {
    if perto(escala, escala_de_encaixar(c)) {
        return Nivel::Encaixar;
    }
    if perto(escala, escala_de_preencher(c)) {
        return Nivel::Preencher;
    }
    let razao = razao_da_escala(escala, c);
    Nivel::Razao(
        RAZOES
            .iter()
            .copied()
            .find(|r| perto(*r, razao))
            .unwrap_or(razao),
    )
}

/// "Encaixar", "1:1", "3:1", "1:4", ou a porcentagem entre paradas.
pub fn rotulo_do_nivel(nivel: Nivel) -> String {
    match nivel {
        Nivel::Encaixar => "Encaixar".into(),
        Nivel::Preencher => "Preencher".into(),
        Nivel::Razao(r) if r >= 1. && perto(r, r.round()) => format!("{}:1", r.round() as i32),
        Nivel::Razao(r) if r < 1. && perto(1. / r, (1. / r).round()) => {
            format!("1:{}", (1. / r).round() as i32)
        }
        Nivel::Razao(r) => format!("{}%", (r * 100.).round() as i32),
    }
}

/// Põe a janela na área, numa escala e em torno de um centro. Em cada eixo:
/// se a foto cabe, fica centrada; se não, a borda nunca descola da borda.
pub fn posicionar(escala: f32, centro: Ponto, c: &Cena) -> Vista {
    let eixo = |janela: f32, area: f32, pedido: f32| {
        let tamanho = janela * escala;
        if tamanho <= area {
            return ((area - tamanho) / 2., 0.5);
        }
        let pos = (area / 2. - pedido * tamanho).max(area - tamanho).min(0.);
        (pos, (area / 2. - pos) / tamanho)
    };
    let (x, cx) = eixo(c.janela.largura, c.area.largura, centro.x);
    let (y, cy) = eixo(c.janela.altura, c.area.altura, centro.y);
    Vista {
        escala,
        x,
        y,
        centro: Ponto { x: cx, y: cy },
    }
}

/// O estado guardado: o nível e o centro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EstadoDoZoom {
    pub nivel: Nivel,
    pub centro: Ponto,
}

impl Default for EstadoDoZoom {
    fn default() -> Self {
        Self {
            nivel: Nivel::Encaixar,
            centro: Ponto::CENTRO,
        }
    }
}

pub fn vista_do_zoom(z: EstadoDoZoom, c: &Cena) -> Vista {
    posicionar(escala_do_nivel(z.nivel, c), z.centro, c)
}

/// Há foto fora da área? É o que torna o arrastar possível.
pub fn passa_da_area(v: &Vista, c: &Cena) -> bool {
    c.janela.largura * v.escala > c.area.largura + 0.5
        || c.janela.altura * v.escala > c.area.altura + 0.5
}

/// A tela precisa do bruto em resolução cheia? (`precisaDoBruto` do site)
///
/// Ampliar a cópia de trabalho além de um pixel dela por pixel do dispositivo
/// é borrão, e não zoom. O encaixe fica de fora mesmo quando passaria da conta:
/// carregar o bruto para ele deixaria todo slider pesado por nada.
pub fn precisa_do_bruto(z: &EstadoDoZoom, v: &Vista, c: &Cena) -> bool {
    if z.nivel == Nivel::Encaixar || c.fator_do_bruto <= 1.05 {
        return false;
    }
    v.escala * c.dpr > 1.05
}

/// O centro que mantém o ponto sob o cursor no lugar ao mudar de escala.
pub fn centro_em_torno_de(v: &Vista, nova_escala: f32, ponto: Ponto, c: &Cena) -> Ponto {
    let na_janela_x = (ponto.x - v.x) / v.escala;
    let na_janela_y = (ponto.y - v.y) / v.escala;
    let novo_x = ponto.x - na_janela_x * nova_escala;
    let novo_y = ponto.y - na_janela_y * nova_escala;
    Ponto {
        x: (c.area.largura / 2. - novo_x) / (c.janela.largura * nova_escala),
        y: (c.area.altura / 2. - novo_y) / (c.janela.altura * nova_escala),
    }
}

/// O centro depois de a foto andar `dx, dy` pontos com a mão.
pub fn centro_arrastado(v: &Vista, dx: f32, dy: f32, c: &Cena) -> Ponto {
    Ponto {
        x: (c.area.largura / 2. - (v.x + dx)) / (c.janela.largura * v.escala),
        y: (c.area.altura / 2. - (v.y + dy)) / (c.janela.altura * v.escala),
    }
}

/// A próxima parada do `⌘=` (`direcao` 1) ou do `⌘−` (-1), na ordem da escala
/// que cada uma dá nesta foto e nesta tela.
pub fn proxima_parada(escala_atual: f32, direcao: i32, c: &Cena) -> Nivel {
    let mut paradas: Vec<(Nivel, f32)> = vec![
        (Nivel::Encaixar, escala_de_encaixar(c)),
        (Nivel::Preencher, escala_de_preencher(c)),
    ];
    paradas.extend(
        RAZOES
            .iter()
            .map(|r| (Nivel::Razao(*r), escala_da_razao(*r, c))),
    );
    paradas.retain(|(_, e)| *e == limitar_escala(*e, c));
    paradas.sort_by(|a, b| a.1.total_cmp(&b.1));
    if direcao > 0 {
        return paradas
            .iter()
            .find(|(_, e)| *e > escala_atual * (1. + QUASE))
            .or(paradas.last())
            .map(|p| p.0)
            .unwrap_or(Nivel::Encaixar);
    }
    paradas
        .iter()
        .rev()
        .find(|(_, e)| *e < escala_atual * (1. - QUASE))
        .or(paradas.first())
        .map(|p| p.0)
        .unwrap_or(Nivel::Encaixar)
}

/// O pedaço da janela que está na tela, normalizado.
pub fn retangulo_visivel(v: &Vista, c: &Cena) -> Retangulo {
    let largura = c.janela.largura * v.escala;
    let altura = c.janela.altura * v.escala;
    let x = (-v.x / largura).max(0.);
    let y = (-v.y / altura).max(0.);
    Retangulo {
        x,
        y,
        w: (c.area.largura / largura).min(1.).min(1. - x),
        h: (c.area.altura / altura).min(1.).min(1. - y),
    }
}

/// `⌘` + arrastar: a caixa marcada passa a encher a área. Caixa pequena
/// demais (um clique que tremeu) não vira zoom.
pub fn zoom_da_caixa(v: &Vista, a: Ponto, b: Ponto, c: &Cena) -> Option<EstadoDoZoom> {
    let w = (b.x - a.x).abs();
    let h = (b.y - a.y).abs();
    if w < 8. || h < 8. {
        return None;
    }
    let esquerda = (a.x.min(b.x) - v.x) / v.escala;
    let topo = (a.y.min(b.y) - v.y) / v.escala;
    let (jw, jh) = (w / v.escala, h / v.escala);
    let escala = limitar_escala((c.area.largura / jw).min(c.area.altura / jh), c);
    Some(EstadoDoZoom {
        nivel: nivel_da_escala(escala, c),
        centro: Ponto {
            x: (esquerda + jw / 2.) / c.janela.largura,
            y: (topo + jh / 2.) / c.janela.altura,
        },
    })
}

/// `Page Down`/`Page Up`: percorre a foto ampliada em Z, uma tela por vez.
pub fn centro_paginado(v: &Vista, direcao: i32, c: &Cena) -> Ponto {
    let r = retangulo_visivel(v, c);
    let (ultima_x, ultima_y) = (1. - r.w, 1. - r.h);
    let fim = |a: f32, b: f32| a >= b - 1e-3;
    let (mut x, mut y) = (r.x, r.y);
    if direcao > 0 {
        if !fim(y, ultima_y) {
            y = (y + r.h).min(ultima_y);
        } else if !fim(x, ultima_x) {
            x = (x + r.w).min(ultima_x);
            y = 0.;
        } else {
            x = 0.;
            y = 0.;
        }
    } else if y > 1e-3 {
        y = (y - r.h).max(0.);
    } else if x > 1e-3 {
        x = (x - r.w).max(0.);
        y = ultima_y;
    } else {
        x = ultima_x;
        y = ultima_y;
    }
    Ponto {
        x: x + r.w / 2.,
        y: y + r.h / 2.,
    }
}

/// O fator da roda com `⌘`/`Ctrl`/`⌥` (e da pinça): contínuo no trackpad, um
/// degrau fixo na roda do mouse.
pub fn fator_da_roda(dy: f32) -> f32 {
    if dy.abs() < 40. {
        (-dy * 0.01).exp()
    } else if dy < 0. {
        1.25
    } else {
        0.8
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_bruto_so_entra_alem_da_copia_e_fora_do_encaixe() {
        let mut c = cena();
        c.fator_do_bruto = 3.;
        c.dpr = 2.;
        let z = |nivel| EstadoDoZoom {
            nivel,
            centro: Ponto { x: 0.5, y: 0.5 },
        };
        let v = |escala| Vista {
            escala,
            x: 0.,
            y: 0.,
            centro: Ponto { x: 0.5, y: 0.5 },
        };
        assert!(!precisa_do_bruto(&z(Nivel::Encaixar), &v(2.), &c));
        assert!(precisa_do_bruto(&z(Nivel::Razao(1.)), &v(1.5), &c));
        assert!(!precisa_do_bruto(&z(Nivel::Razao(0.25)), &v(0.5), &c));
        c.fator_do_bruto = 1.;
        assert!(!precisa_do_bruto(&z(Nivel::Razao(1.)), &v(1.5), &c));
    }

    fn cena() -> Cena {
        Cena {
            janela: Medidas {
                largura: 2000.,
                altura: 1000.,
            },
            area: Medidas {
                largura: 1000.,
                altura: 800.,
            },
            dpr: 2.,
            fator_do_bruto: 1.,
        }
    }

    #[test]
    fn encaixar_e_preencher_seguem_a_area() {
        let c = cena();
        assert_eq!(escala_de_encaixar(&c), 0.5);
        assert_eq!(escala_de_preencher(&c), 0.8);
        // 1:1 numa tela Retina é meio ponto por pixel.
        assert_eq!(escala_do_nivel(Nivel::Razao(1.), &c), 0.5);
        assert_eq!(nivel_da_escala(0.5, &c), Nivel::Encaixar);
        assert_eq!(nivel_da_escala(1.0, &c), Nivel::Razao(2.));
    }

    #[test]
    fn os_rotulos_sao_os_do_site() {
        assert_eq!(rotulo_do_nivel(Nivel::Razao(1.)), "1:1");
        assert_eq!(rotulo_do_nivel(Nivel::Razao(0.25)), "1:4");
        assert_eq!(rotulo_do_nivel(Nivel::Razao(3.)), "3:1");
        assert_eq!(rotulo_do_nivel(Nivel::Razao(1.5)), "150%");
        assert_eq!(rotulo_do_nivel(Nivel::Preencher), "Preencher");
    }

    #[test]
    fn a_foto_que_cabe_fica_centrada_e_a_que_nao_cabe_nao_descola() {
        let c = cena();
        let v = posicionar(0.5, Ponto { x: 0.9, y: 0.1 }, &c);
        assert_eq!((v.x, v.y), (0., 150.));
        let v = posicionar(2., Ponto { x: 0.0, y: 1.0 }, &c);
        assert_eq!(v.x, 0., "o canto esquerdo encosta na borda");
        assert_eq!(v.y, 800. - 2000., "o de baixo também");
        assert!(passa_da_area(&v, &c));
    }

    #[test]
    fn ampliar_em_torno_do_cursor_mantem_o_ponto() {
        let c = cena();
        let v = posicionar(1., Ponto::CENTRO, &c);
        let ponto = Ponto { x: 300., y: 200. };
        let centro = centro_em_torno_de(&v, 2., ponto, &c);
        let nova = posicionar(2., centro, &c);
        let antes = ((ponto.x - v.x) / v.escala, (ponto.y - v.y) / v.escala);
        let depois = (
            (ponto.x - nova.x) / nova.escala,
            (ponto.y - nova.y) / nova.escala,
        );
        assert!((antes.0 - depois.0).abs() < 0.01 && (antes.1 - depois.1).abs() < 0.01);
    }

    #[test]
    fn as_paradas_andam_em_ordem_de_escala() {
        let c = cena();
        assert_eq!(proxima_parada(0.5, 1, &c), Nivel::Preencher);
        assert_eq!(proxima_parada(0.8, 1, &c), Nivel::Razao(2.));
        // Nesta cena 1:1 e Encaixar dão a mesma escala (0,5); descendo, o site
        // pega a última da lista ordenada, que é a razão.
        assert_eq!(proxima_parada(0.8, -1, &c), Nivel::Razao(1.));
        assert_eq!(proxima_parada(0.5, -1, &c), Nivel::Razao(0.5));
    }

    #[test]
    fn o_navegador_mostra_o_pedaco_visivel() {
        let c = cena();
        let v = posicionar(1., Ponto::CENTRO, &c);
        let r = retangulo_visivel(&v, &c);
        assert!((r.w - 0.5).abs() < 1e-4 && (r.h - 0.8).abs() < 1e-4);
        assert!((r.x - 0.25).abs() < 1e-4 && (r.y - 0.1).abs() < 1e-4);
    }

    #[test]
    fn a_caixa_pequena_nao_amplia_e_a_grande_enche_a_area() {
        let c = cena();
        let v = posicionar(0.5, Ponto::CENTRO, &c);
        assert!(
            zoom_da_caixa(&v, Ponto { x: 10., y: 10. }, Ponto { x: 14., y: 30. }, &c).is_none()
        );
        let z =
            zoom_da_caixa(&v, Ponto { x: 0., y: 150. }, Ponto { x: 500., y: 400. }, &c).unwrap();
        assert_eq!(z.nivel, Nivel::Razao(2.));
    }

    #[test]
    fn a_paginacao_percorre_em_z() {
        let c = cena();
        let v = posicionar(2., Ponto { x: 0., y: 0. }, &c);
        let proximo = centro_paginado(&v, 1, &c);
        assert!(proximo.y > 0.4, "desce uma tela");
        assert!(proximo.x < 0.3, "na mesma coluna");
    }
}
