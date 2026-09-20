//! O estresse do app: volume, repetição e desordem, com números medidos e
//! limites afirmados.
//!
//! # Por que este arquivo existe
//!
//! *"Finalmente teste de estresse!"* (dono, 2026-09-17). Os testes de unidade
//! conferem **um** gesto; os de fluxo, **uma** passagem. Nenhum deles pergunta o
//! que acontece com dez mil fotos, com mil eventos de slider no mesmo gesto, ou
//! com trezentas respostas do site chegando fora de ordem e com falhas no meio —
//! que é exatamente o dia de um casamento. A meta do dono é **uma sessão de 30
//! fotos em ~5 min**, da câmera ao link, e lentidão é defeito.
//!
//! # Onde está cada parte
//!
//! - aqui: as regras puras (corte e zoom) com milhares de entradas aleatórias,
//!   a Biblioteca com acervo grande, o lote do "Salvar na galeria" com recados
//!   embaralhados, o cupom do caixa com centenas de itens e o motor em 24 MP;
//! - `revelacao/tela/estresse.rs`: a tira, os sliders, o Enquadrar e o zoom —
//!   o que só se alcança de dentro da tela;
//! - `caixa/estresse_do_lote.rs`: a negociação em lote, de quatro em quatro.
//!
//! # Como rodar
//!
//! ```bash
//! cargo test -p ui-gpui --lib estresse -- --nocapture            # os leves
//! cargo test -p ui-gpui --lib estresse -- --ignored --nocapture  # os pesados
//! ```
//!
//! 🚨 **Nada daqui sai da máquina.** Só portas de mentira e catálogos em pasta
//! temporária — nunca a API de produção, nunca a sessão guardada.
//!
//! ⚠️ **Os números são do perfil `test`**, e não do `--release`: servem para
//! achar o que cresce errado (quadrático, vazamento, laço preso), e os limites
//! afirmados são folgados de propósito. Medida de fluidez fina é nos binários
//! `medir-*`, em `--release`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use adapters::view_models::PhotoViewModel;
use gpui::TestAppContext;
use image::{DynamicImage, Rgba, RgbaImage};
use infrastructure::cache::preview_manager::PreviewManager;
use tempfile::TempDir;

use crate::app::{Aplicativo, Portas, Tela};
use crate::atualizacao::porta::mentira::AtualizadorDeMentira;
use crate::biblioteca::acervo::mentira::AcervoDeMentira;
use crate::biblioteca::colecoes::mentira::ColecoesDeMentira;
use crate::biblioteca::marcacao::mentira::MarcadorDeMentira;
use crate::exportacao::porta::mentira::ExportadorDeMentira;
use crate::importacao::explorador::mentira::{
    ExploradorDeMentira, GeradorDeMentira, ImportadorDeMentira, SeletorDeMentira,
};
use crate::impressao::porta::mentira::FolhaDeMentira;
use crate::pos_venda::porta::mentira::PublicadorDeMentira;
use crate::pos_venda::porta::Recado;
use crate::revelacao::corte::{self, Alca, Retangulo, ANGULO_MAXIMO, PROPORCOES};
use crate::revelacao::lightroom::mentira::EscolhaDeMentira;
use crate::revelacao::persistencia::mentira::GravadorDeMentira;
use crate::revelacao::presets::mentira::GuardaDeMentira;
use crate::revelacao::reposicao::mentira::RepositorDeMentira;
use crate::revelacao::zoom::{self, Cena, EstadoDoZoom, Medidas, Nivel, Ponto};
use crate::segundo_plano::vigia::{AoFechar, Vigia};

// ───────────────────────────────────────────────────────── as ferramentas

/// Um gerador congruencial: determinístico, sem crate nova, e o bastante para
/// espalhar entradas. A mesma semente dá a mesma sequência em toda máquina —
/// uma falha aqui se reproduz com o número que o teste imprime.
pub(crate) struct Lcg(u64);

impl Lcg {
    pub(crate) fn novo(semente: u64) -> Self {
        Self(semente ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub(crate) fn proximo(&mut self) -> u64 {
        // As constantes do MMIX (Knuth).
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    /// Um número em `[0, 1)`.
    pub(crate) fn unitario(&mut self) -> f32 {
        (self.proximo() % 1_000_000) as f32 / 1_000_000.
    }

    /// Um número em `[de, ate)`.
    pub(crate) fn entre(&mut self, de: f32, ate: f32) -> f32 {
        de + (ate - de) * self.unitario()
    }

    /// Um inteiro em `[0, n)`.
    pub(crate) fn ate(&mut self, n: usize) -> usize {
        (self.proximo() % n.max(1) as u64) as usize
    }

    pub(crate) fn embaralhar<T>(&mut self, itens: &mut [T]) {
        for i in (1..itens.len()).rev() {
            let j = self.ate(i + 1);
            itens.swap(i, j);
        }
    }
}

/// A memória residente do processo, em MB — o `ps` do sistema, sem crate.
///
/// `None` onde o `ps` não responde: a medida é informativa, e os limites de
/// memória deste arquivo são afirmados sobre o que o app **guarda** (tamanho de
/// cache), que não depende do sistema.
pub(crate) fn memoria_em_mb() -> Option<f64> {
    let saida = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let kb: f64 = String::from_utf8_lossy(&saida.stdout).trim().parse().ok()?;
    Some(kb / 1024.)
}

/// Imprime uma linha da régua: `✅`/`🚨`, o rótulo e o número contra o limite.
pub(crate) fn relatar(rotulo: &str, medido: Duration, limite: Duration) {
    let ms = medido.as_secs_f64() * 1000.;
    let teto = limite.as_secs_f64() * 1000.;
    println!(
        "{} {rotulo}: {ms:.2} ms (limite {teto:.0} ms)",
        if medido <= limite { "✅" } else { "🚨" }
    );
}

/// Mede e afirma: o tempo de `f` não passa de `limite`.
pub(crate) fn cronometrar<R>(rotulo: &str, limite: Duration, f: impl FnOnce() -> R) -> R {
    let inicio = Instant::now();
    let r = f();
    let gasto = inicio.elapsed();
    relatar(rotulo, gasto, limite);
    assert!(
        gasto <= limite,
        "{rotulo}: {gasto:?} passou do limite de {limite:?}"
    );
    r
}

fn finito(v: f32) -> bool {
    v.is_finite()
}

// ─────────────────────────────────────── as regras puras, em milhares de casos

const CASOS: usize = 20_000;

/// Um espaço de foto que existe: de uma miniatura de 64 px a um bruto de 60 MP,
/// em pé e deitado, até o panorama de 4:1.
fn espaco_aleatorio(g: &mut Lcg) -> (f32, f32) {
    let maior = g.entre(64., 9600.).round();
    let razao = g.entre(1., 4.);
    let menor = (maior / razao).round().max(64.);
    if g.ate(2) == 0 {
        (maior, menor)
    } else {
        (menor, maior)
    }
}

fn retangulo_dentro(g: &mut Lcg, espaco: (f32, f32)) -> Retangulo {
    let w = g.entre(1., espaco.0);
    let h = g.entre(1., espaco.1);
    Retangulo {
        x: g.entre(0., espaco.0 - w),
        y: g.entre(0., espaco.1 - h),
        w,
        h,
    }
}

fn alca_aleatoria(g: &mut Lcg) -> Option<Alca> {
    let i = g.ate(Alca::TODAS.len() + 1);
    Alca::TODAS.get(i).copied()
}

fn sem_nan(r: &Retangulo) -> bool {
    finito(r.x) && finito(r.y) && finito(r.w) && finito(r.h)
}

/// Um retângulo **degenerado**: um lado de um pixel, que é o piso do
/// arredondamento e não um corte que alguém faria.
fn degenerado(r: &Retangulo) -> bool {
    r.w <= 1. || r.h <= 1.
}

/// 🚨 **O arrasto do Enquadrar nunca vira NaN, nunca some e não sai da foto.**
///
/// Vinte mil arrastos com alça, proporção e deltas aleatórios — inclusive
/// deltas que atravessam a foto inteira, que é o que o ponteiro faz quando sai
/// da janela no meio do gesto.
///
/// ⚠️ **Defeito conhecido, o mesmo do site** (`corte.ts`, `arrastar`): com a
/// proporção travada, o retângulo que bate no lado mínimo sai da conta com o
/// lado menor abaixo do mínimo, e o `max(minimo)` do fim o **cresce para fora
/// da foto**, a partir da âncora — até `minimo` pixels além da borda, e com a
/// proporção quebrada. O `CropSettings` corta o excesso ao gravar, então o que
/// se perde é a forma (3:2 vira ~1,37 no pior caso), e não a foto. O teste
/// conta os casos e prende o tamanho do excesso; o conserto, para valer, é nos
/// dois lados de uma vez (o mínimo aplicado ao lado **menor**, antes da âncora).
#[test]
fn estresse_o_arrasto_do_corte_fica_dentro_do_espaco() {
    let mut g = Lcg::novo(17);
    let inicio = Instant::now();
    let mut estouros = 0usize;
    for caso in 0..CASOS {
        let espaco = espaco_aleatorio(&mut g);
        let inicial = retangulo_dentro(&mut g, espaco);
        let alca = alca_aleatoria(&mut g);
        let proporcao = alca.and(PROPORCOES[g.ate(PROPORCOES.len())].1);
        let dx = g.entre(-3. * espaco.0, 3. * espaco.0);
        let dy = g.entre(-3. * espaco.1, 3. * espaco.1);

        let r = corte::arrastar_em_pixels(inicial, alca, dx, dy, espaco, proporcao);
        let contexto = format!(
            "caso {caso}: espaço {espaco:?}, inicial {inicial:?}, alça {alca:?}, \
             proporção {proporcao:?}, delta ({dx}, {dy}) → {r:?}"
        );
        let minimo = (0.01 * espaco.0.min(espaco.1)).round().max(1.);

        assert!(sem_nan(&r), "NaN no retângulo — {contexto}");
        assert!(r.w >= 1. && r.h >= 1., "o retângulo sumiu — {contexto}");
        assert!(
            r.x >= 0. && r.y >= 0.,
            "saiu pela esquerda/topo — {contexto}"
        );
        let excesso = (r.x + r.w - espaco.0).max(r.y + r.h - espaco.1);
        if proporcao.is_none() {
            // Arredondado para pixel inteiro: meio pixel de folga, e só isso.
            assert!(excesso <= 0.5, "saiu pela direita/base — {contexto}");
        } else {
            assert!(excesso <= minimo + 0.5, "saiu além do mínimo — {contexto}");
            if excesso > 0.5 {
                estouros += 1;
            }
        }

        // A proporção travada vale para toda alça — a menos que o retângulo
        // tenha batido no lado mínimo, onde a foto manda.
        if let Some(p) = proporcao {
            if r.w > minimo + 1. && r.h > minimo + 1. {
                // Um pixel de arredondamento em cada lado.
                let folga = p * (1. / r.h + 1. / r.w) * 1.5 + 1e-3;
                assert!(
                    (r.w / r.h - p).abs() <= folga,
                    "a proporção {p} virou {} — {contexto}",
                    r.w / r.h
                );
            }
        }
    }
    relatar(
        &format!("{CASOS} arrastos de corte"),
        inicio.elapsed(),
        Duration::from_millis(2000),
    );
    println!(
        "⚠️ {estouros} de {CASOS} arrastos com proporção passaram da borda pelo lado mínimo (defeito conhecido, igual ao do site)"
    );
    assert!(
        estouros * 20 < CASOS,
        "o estouro deixou de ser caso de borda"
    );
}

/// 🚨 **O endireitar sempre cabe na foto girada, e com a mesma forma.**
///
/// É a conta que roda a cada 0,1° do transferidor: bissecção de 40 passos sobre
/// quatro cantos. Um NaN aqui vira um retângulo invisível; um canto de fora vira
/// a faixa preta que o "zoom do endireitar" existe para esconder.
///
/// ⚠️ **Defeito conhecido, o mesmo do site** (`encolherParaCaber`): quando o
/// centro pedido está **dentro** da foto girada mas colado na borda, nenhum
/// retângulo em volta dele cabe, a bissecção devolve escala zero e o
/// arredondamento entrega um retângulo de 1 px — que também não cabe. O
/// teste conta esses casos (degenerados) e confere todo o resto.
#[test]
fn estresse_o_endireitar_cabe_e_mantem_a_forma() {
    let mut g = Lcg::novo(23);
    let inicio = Instant::now();
    let mut degenerados = 0usize;
    for caso in 0..CASOS {
        let espaco = espaco_aleatorio(&mut g);
        let pedido = retangulo_dentro(&mut g, espaco);
        let angulo = g.entre(-ANGULO_MAXIMO, ANGULO_MAXIMO);
        let r = corte::encolher_para_caber(pedido, espaco, angulo);
        let contexto =
            format!("caso {caso}: espaço {espaco:?}, pedido {pedido:?}, ângulo {angulo} → {r:?}");

        assert!(sem_nan(&r), "NaN — {contexto}");
        assert!(r.w >= 1. && r.h >= 1., "sumiu — {contexto}");
        assert!(r.x >= 0. && r.y >= 0., "fora do espaço — {contexto}");
        assert!(
            r.x + r.w <= espaco.0 + 0.5 && r.y + r.h <= espaco.1 + 0.5,
            "fora do espaço — {contexto}"
        );
        // Nunca cresce além do que foi pedido (um pixel de arredondamento).
        assert!(
            r.w <= pedido.w + 1. && r.h <= pedido.h + 1.,
            "cresceu além do pedido — {contexto}"
        );
        if degenerado(&r) {
            degenerados += 1;
            continue;
        }
        assert!(
            corte::cabe_na_foto_girada(r, espaco, angulo),
            "canto vazio — {contexto}"
        );
        // A forma: dois pixels de arredondamento, relativos ao menor lado.
        if r.w > 8. && r.h > 8. {
            let antes = pedido.w / pedido.h;
            let depois = r.w / r.h;
            let folga = antes * (2. / r.w + 2. / r.h) + 1e-3;
            assert!(
                (antes - depois).abs() <= folga,
                "a forma {antes} virou {depois} — {contexto}"
            );
        }

        // O caminho inteiro do slider de ângulo, pelo `CropSettings`.
        let base = corte::com_retangulo(&corte::foto_inteira(), pedido, espaco);
        let reto = corte::endireitar(&base, angulo, espaco, Some(pedido));
        for v in [
            reto.crop_x(),
            reto.crop_y(),
            reto.crop_width(),
            reto.crop_height(),
            reto.angle(),
        ] {
            assert!(finito(v), "NaN no CropSettings — {contexto}");
        }
        assert!(reto.crop_x() + reto.crop_width() <= 1. + 1e-4, "{contexto}");
        assert!(
            reto.crop_y() + reto.crop_height() <= 1. + 1e-4,
            "{contexto}"
        );
    }
    relatar(
        &format!("{CASOS} endireitamentos"),
        inicio.elapsed(),
        Duration::from_millis(3000),
    );
    println!("⚠️ {degenerados} de {CASOS} endireitamentos degeneraram em 1 px (defeito conhecido, igual ao do site)");
    assert!(
        degenerados * 50 < CASOS,
        "o degenerado deixou de ser caso de borda"
    );
}

/// 🚨 **Trocar a proporção no meio preserva a forma pedida e cabe na foto.**
#[test]
fn estresse_a_proporcao_no_centro() {
    let mut g = Lcg::novo(29);
    let mut degenerados = 0usize;
    for caso in 0..CASOS {
        let espaco = espaco_aleatorio(&mut g);
        let pedido = retangulo_dentro(&mut g, espaco);
        let angulo = if g.ate(2) == 0 {
            0.
        } else {
            g.entre(-ANGULO_MAXIMO, ANGULO_MAXIMO)
        };
        let p = PROPORCOES[1 + g.ate(PROPORCOES.len() - 1)]
            .1
            .expect("travada");
        let r = corte::com_proporcao_no_centro(pedido, p, espaco, angulo);
        let contexto = format!(
            "caso {caso}: espaço {espaco:?}, pedido {pedido:?}, p {p}, ângulo {angulo} → {r:?}"
        );
        assert!(sem_nan(&r), "{contexto}");
        assert!(r.w >= 1. && r.h >= 1., "{contexto}");
        if degenerado(&r) {
            assert!(angulo != 0., "sem ângulo não há degenerado — {contexto}");
            degenerados += 1;
            continue;
        }
        assert!(corte::cabe_na_foto_girada(r, espaco, angulo), "{contexto}");
        if r.w > 8. && r.h > 8. {
            let folga = p * (2. / r.w + 2. / r.h) + 1e-3;
            assert!((r.w / r.h - p).abs() <= folga, "{contexto}");
        }
    }
    println!("⚠️ {degenerados} de {CASOS} trocas de proporção degeneraram em 1 px");
    assert!(degenerados * 50 < CASOS);
}

fn cena_aleatoria(g: &mut Lcg) -> Cena {
    Cena {
        janela: Medidas {
            largura: g.entre(1., 9600.).round().max(1.),
            altura: g.entre(1., 9600.).round().max(1.),
        },
        area: Medidas {
            largura: g.entre(2., 3840.),
            altura: g.entre(2., 2160.),
        },
        dpr: [1., 1.5, 2., 3.][g.ate(4)],
        fator_do_bruto: if g.ate(3) == 0 { 1. } else { g.entre(1., 8.) },
    }
}

/// 🚨 **Mil gestos de zoom seguidos, em cenas aleatórias: nada escapa.**
///
/// ⌘=, ⌘−, a roda, a caixa, as páginas, Home e End — o centro fica dentro da
/// foto, a vista fica finita, o retângulo do navegador fica no quadrado, e o
/// `⌘=` nunca diminui (nem o `⌘−` aumenta).
#[test]
fn estresse_o_zoom_nunca_escapa() {
    let mut g = Lcg::novo(31);
    let inicio = Instant::now();
    let mut gestos = 0usize;
    for cena_n in 0..400 {
        let c = cena_aleatoria(&mut g);
        let mut z = EstadoDoZoom::default();
        for passo in 0..250 {
            gestos += 1;
            let v = zoom::vista_do_zoom(z, &c);
            let contexto = format!("cena {cena_n} passo {passo}: {c:?} {z:?} → {v:?}");
            assert!(
                finito(v.escala) && finito(v.x) && finito(v.y),
                "vista com NaN — {contexto}"
            );
            assert!(v.escala > 0., "escala não positiva — {contexto}");
            assert!(
                (0.0..=1.0).contains(&v.centro.x) && (0.0..=1.0).contains(&v.centro.y),
                "centro fora da foto — {contexto}"
            );
            let r = zoom::retangulo_visivel(&v, &c);
            for (nome, valor) in [("x", r.x), ("y", r.y), ("w", r.w), ("h", r.h)] {
                assert!(
                    finito(valor) && (-1e-3..=1. + 1e-3).contains(&valor),
                    "navegador com {nome} = {valor} — {contexto}"
                );
            }
            assert!(
                r.x + r.w <= 1. + 1e-3 && r.y + r.h <= 1. + 1e-3,
                "{contexto}"
            );
            assert!(!zoom::rotulo_do_nivel(z.nivel).is_empty());
            let _ = zoom::precisa_do_bruto(&z, &v, &c);

            z = match g.ate(9) {
                0 | 1 => {
                    let direcao = if g.ate(2) == 0 { 1 } else { -1 };
                    let nivel = zoom::proxima_parada(v.escala, direcao, &c);
                    let nova = zoom::escala_do_nivel(nivel, &c);
                    if direcao > 0 {
                        assert!(nova >= v.escala * (1. - 1e-3), "⌘= diminuiu — {contexto}");
                    } else {
                        assert!(nova <= v.escala * (1. + 1e-3), "⌘− aumentou — {contexto}");
                    }
                    EstadoDoZoom {
                        nivel,
                        centro: v.centro,
                    }
                }
                2 => {
                    // A roda (ou a pinça), em torno de um ponto da área.
                    let dy = g.entre(-400., 400.);
                    let escala = zoom::limitar_escala(v.escala * zoom::fator_da_roda(dy), &c);
                    let ponto = Ponto {
                        x: g.entre(0., c.area.largura),
                        y: g.entre(0., c.area.altura),
                    };
                    EstadoDoZoom {
                        nivel: zoom::nivel_da_escala(escala, &c),
                        centro: zoom::centro_em_torno_de(&v, escala, ponto, &c),
                    }
                }
                3 => {
                    // A mão: o centro pode ser pedido fora, e é o `posicionar`
                    // que o traz de volta.
                    let centro = zoom::centro_arrastado(
                        &v,
                        g.entre(-5000., 5000.),
                        g.entre(-5000., 5000.),
                        &c,
                    );
                    EstadoDoZoom { centro, ..z }
                }
                4 => {
                    let a = Ponto {
                        x: g.entre(-100., c.area.largura + 100.),
                        y: g.entre(-100., c.area.altura + 100.),
                    };
                    let b = Ponto {
                        x: g.entre(-100., c.area.largura + 100.),
                        y: g.entre(-100., c.area.altura + 100.),
                    };
                    zoom::zoom_da_caixa(&v, a, b, &c).unwrap_or(z)
                }
                5 => EstadoDoZoom {
                    centro: zoom::centro_paginado(&v, if g.ate(2) == 0 { 1 } else { -1 }, &c),
                    ..z
                },
                6 => EstadoDoZoom {
                    centro: [Ponto { x: 0., y: 0. }, Ponto { x: 1., y: 1. }][g.ate(2)],
                    ..z
                },
                7 => EstadoDoZoom {
                    nivel: Nivel::Razao(zoom::RAZOES[g.ate(zoom::RAZOES.len())]),
                    ..z
                },
                _ => EstadoDoZoom {
                    nivel: [Nivel::Encaixar, Nivel::Preencher][g.ate(2)],
                    ..z
                },
            };
        }
    }
    relatar(
        &format!("{gestos} gestos de zoom"),
        inicio.elapsed(),
        Duration::from_millis(3000),
    );
}

/// 🔑 **O `PgDn` percorre a foto ampliada inteira e volta ao começo.**
///
/// Em Z, uma tela por vez: com a foto em 16:1, são centenas de páginas, e o
/// laço tem de terminar — um passo que não anda deixaria o operador preso numa
/// coluna.
#[test]
fn estresse_as_paginas_percorrem_a_foto_inteira() {
    let mut g = Lcg::novo(37);
    for _ in 0..200 {
        let c = cena_aleatoria(&mut g);
        let nivel = Nivel::Razao(zoom::RAZOES[g.ate(zoom::RAZOES.len())]);
        let mut z = EstadoDoZoom {
            nivel,
            centro: Ponto { x: 0., y: 0. },
        };
        let v = zoom::vista_do_zoom(z, &c);
        let r = zoom::retangulo_visivel(&v, &c);
        let paginas = ((1. / r.w).ceil() * (1. / r.h).ceil()) as usize;
        // 16:1 num bruto de 60 MP são centenas de milhares de telas: a conta é
        // a mesma, e o laço só confere as que cabem num teste.
        if paginas > 20_000 {
            continue;
        }
        // Da primeira página, `paginas` passos voltam à primeira.
        let mut voltou_em = None;
        for passo in 1..=paginas + 2 {
            let v = zoom::vista_do_zoom(z, &c);
            z.centro = zoom::centro_paginado(&v, 1, &c);
            let depois = zoom::retangulo_visivel(&zoom::vista_do_zoom(z, &c), &c);
            if depois.x < 1e-3 && depois.y < 1e-3 {
                voltou_em = Some(passo);
                break;
            }
        }
        assert!(
            voltou_em.is_some(),
            "{paginas} páginas e o PgDn não voltou ao começo — {c:?} {nivel:?}"
        );
    }
}

// ─────────────────────────────────────────────────── o app, com portas de mentira

fn portas(publicador: Arc<PublicadorDeMentira>, gravador: Arc<GravadorDeMentira>) -> Portas {
    portas_com(publicador, gravador, Arc::new(AcervoDeMentira::default()))
}

/// O mesmo, com o acervo de fora — é por ele que o estresse conta **quantas
/// releituras** um lote provoca.
fn portas_com(
    publicador: Arc<PublicadorDeMentira>,
    gravador: Arc<GravadorDeMentira>,
    acervo: Arc<AcervoDeMentira>,
) -> Portas {
    Portas {
        gravador,
        acervo,
        exportador: Arc::new(ExportadorDeMentira::default()),
        publicador,
        colecoes: Arc::new(ColecoesDeMentira::default()),
        folha: Arc::new(FolhaDeMentira::default()),
        marcador: Arc::new(MarcadorDeMentira::default()),
        gerador: Arc::new(GeradorDeMentira::default()),
        repositor: Arc::new(RepositorDeMentira::default()),
        guarda_de_presets: Arc::new(GuardaDeMentira::default()),
        escolha_de_presets: Arc::new(EscolhaDeMentira::default()),
        explorador: Arc::new(ExploradorDeMentira::default()),
        importador: Arc::new(ImportadorDeMentira::default()),
        seletor: Arc::new(SeletorDeMentira::default()),
        seletor_de_fotos: Arc::new(crate::sessoes::arquivos::mentira::SeletorDeMentira::default()),
        atualizador: Arc::new(AtualizadorDeMentira::default()),
        acervo_de_arquivos: Arc::new(
            crate::backup::porta::mentira::AcervoDeArquivosDeMentira::default(),
        ),
        escolha_do_backup: Arc::new(crate::backup::escolha::mentira::EscolhaDeMentira::default()),
    }
}

fn cinza(lado: u32) -> DynamicImage {
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        lado,
        lado,
        Rgba([120, 120, 120, 255]),
    ))
}

/// Uma foto do ensaio `g1`, já no site quando `no_site`.
fn foto(i: usize, no_site: bool) -> PhotoViewModel {
    PhotoViewModel {
        id: format!("id-{i:05}"),
        name: format!("DSC_{i:05}.jpg"),
        path: format!("/fotos/DSC_{i:05}.jpg"),
        sessao_id: Some("g1".into()),
        pos_venda_foto_id: no_site.then(|| format!("remota-{i:05}")),
        // Um quinto sem nota, o resto espalhado: é o que os recortes cortam.
        rating: (i % 6) as i32,
        ..Default::default()
    }
}

fn previews_descartaveis() -> (Arc<PreviewManager>, TempDir) {
    let dir = TempDir::new().expect("diretório temporário");
    (
        Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf())),
        dir,
    )
}

/// 🚨 **A Biblioteca com 10.000 fotos: abrir, recortar, marcar, andar,
/// classificar tudo de uma vez — e o ensaio inteiro subindo.**
///
/// 🔄 **O gesto mais caro mudou de dono em 2026-09-20.** Era a classificação em
/// massa: cada foto que ganhava nota virava um pedido ao site. Com C20 quem
/// manda as dez mil é **entrar na sessão**, e a nota não fala com a rede —
/// então o cenário prende as duas coisas: classificar 10.000 não faz pedido
/// nenhum, e o ensaio de 10.000 sobe de três em três, com o contador que prende
/// o G9 voltando a zero.
#[gpui::test]
fn estresse_a_biblioteca_com_dez_mil_fotos(cx: &mut TestAppContext) {
    const N: usize = 10_000;
    let (previews, _dir) = previews_descartaveis();
    cx.update(gpui_component::init);
    let publicador = Arc::new(PublicadorDeMentira::default());
    let fotos: Vec<PhotoViewModel> = (0..N).map(|i| foto(i, false)).collect();
    let memoria_antes = memoria_em_mb();

    // O acervo de mentira conta as releituras: é por ele que este cenário
    // afirma que o lote não trava a Biblioteca.
    let acervo = Arc::new(AcervoDeMentira::default());
    let janela = cronometrar(
        "abrir o app com 10.000 fotos",
        Duration::from_secs(5),
        || {
            cx.add_window({
                let publicador = publicador.clone();
                let acervo = acervo.clone();
                move |window, cx| {
                    Aplicativo::ja_dentro(
                        fotos,
                        previews,
                        Vec::new(),
                        portas_com(publicador, Arc::new(GravadorDeMentira::default()), acervo),
                        window,
                        cx,
                    )
                }
            })
        },
    );
    cx.run_until_parked();

    janela
        .update(cx, |app, _window, cx| {
            app.tela = Tela::Biblioteca;
            assert_eq!(app.biblioteca.read(cx).quantas_fotos(), N);

            cronometrar(
                "recortar ★★★ ou mais",
                Duration::from_millis(500),
                || {
                    app.na_biblioteca(cx, |tela, cx| tela.filtrar_por_nota(3, cx));
                },
            );
            let visiveis = app.biblioteca.read(cx).quantas_visiveis();
            assert_eq!(visiveis, (0..N).filter(|i| i % 6 >= 3).count());

            cronometrar("tirar o recorte", Duration::from_millis(500), || {
                app.na_biblioteca(cx, |tela, cx| tela.filtrar_por_nota(0, cx));
            });

            cronometrar(
                "andar 10.000 fotos pela seta",
                Duration::from_secs(5),
                || {
                    app.na_biblioteca(cx, |tela, cx| {
                        for _ in 0..N + 5 {
                            tela.andar(1, cx);
                        }
                    });
                },
            );
            assert_eq!(
                app.biblioteca.read(cx).posicao_da_selecao().map(|p| p.0),
                Some(N),
                "a seta para no fim, sem dar a volta (a posição conta de 1)"
            );
            cronometrar(
                "voltar 10.000 fotos pela seta",
                Duration::from_secs(5),
                || {
                    app.na_biblioteca(cx, |tela, cx| {
                        for _ in 0..N + 5 {
                            tela.andar(-1, cx);
                        }
                    });
                },
            );

            cronometrar("⌘A em 10.000", Duration::from_millis(500), || {
                app.na_biblioteca(cx, |tela, cx| tela.selecionar_tudo(cx));
            });
            assert_eq!(app.biblioteca.read(cx).quantas_selecionadas(), N);

            cronometrar(
                "ler as 10.000 selecionadas",
                Duration::from_millis(500),
                || {
                    assert_eq!(app.biblioteca.read(cx).ids_selecionados().len(), N);
                },
            );

            // A nota em todas: curadoria, e nada mais (C22).
            cronometrar(
                "dar ★★★★ a 10.000 de uma vez",
                Duration::from_secs(5),
                || {
                    app.na_biblioteca(cx, |tela, cx| tela.dar_nota(4, cx));
                },
            );
        })
        .expect("a janela aberta");
    assert!(
        publicador.subidas().is_empty(),
        "🔄 classificar não fala com a rede desde 2026-09-20 (C22)"
    );

    // 📤 **Agora o gesto caro: entrar na sessão com 10.000 fotos no disco.**
    cronometrar(
        "entrar na sessão de 10.000",
        Duration::from_secs(5),
        || {
            janela
                .update(cx, |app, _window, cx| app.entrar_na_sessao("g1".into(), cx))
                .expect("a janela aberta");
        },
    );

    // 🚨 **Três no ar, e não 10.000** (dono, 18/set/2026: *"essa sessão tinha 200
    // fotos e deu erro"*). Cada foto no ar é um original decodificado — 96 MB em
    // RAM —, e o lote inteiro de uma vez levava a máquina do balcão junto. A
    // conta da espera continua sendo o lote todo: todas as respostas virão.
    assert_eq!(
        publicador.subidas().len(),
        crate::envios::EM_VOO,
        "só as primeiras saem; as outras esperam vaga"
    );

    // O site respondeu na hora (a mentira não demora): a próxima colheita zera —
    // e é ela que vai abrindo as vagas até a última.
    cronometrar("colher as respostas", Duration::from_secs(5), || {
        for _ in 0..40 {
            cx.executor().advance_clock(Duration::from_millis(300));
            cx.run_until_parked();
        }
    });
    assert_eq!(
        publicador.subidas().len(),
        N,
        "ao fim, subiu o ensaio inteiro — classificado ou não"
    );

    // 🚨 **A Biblioteca continua operável durante o lote** (dono, 18/set/2026:
    // *"funcionou, mas o usuário não consegue operar a biblioteca durante a
    // atualização das fotos"*). Reler o acervo é varrer o catálogo e refazer a
    // grade **na thread que desenha**; uma releitura por resposta são 1.667
    // varreduras de 10.000 fotos. Agrupadas, são poucas.
    let releituras = *acervo.pedidos.lock().expect("os pedidos");
    assert!(
        releituras < N / 10,
        "{releituras} releituras para {N} respostas — a grade está sendo refeita foto a foto"
    );
    janela
        .update(cx, |app, _window, cx| {
            assert_eq!(app.sincronias_pendentes(), 0, "o contador voltou a zero");
            assert_eq!(app.retrato_do_segundo_plano(cx).subindo, 0);
        })
        .expect("a janela aberta");

    // E desenhar a grade inteira: só as linhas visíveis custam.
    let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
    let tamanho = gpui::size(gpui::px(1600.), gpui::px(1000.));
    visual.draw(gpui::Point::default(), tamanho, |_w, _cx| gpui::Empty);
    let quadros = 20;
    let inicio = Instant::now();
    for _ in 0..quadros {
        janela
            .update(&mut visual, |_app, _w, cx| cx.notify())
            .expect("a janela aberta");
        visual.draw(gpui::Point::default(), tamanho, |_w, _cx| gpui::Empty);
    }
    let por_quadro = inicio.elapsed() / quadros;
    relatar(
        "um quadro da Biblioteca com 10.000",
        por_quadro,
        Duration::from_millis(100),
    );
    assert!(por_quadro <= Duration::from_millis(100));

    if let (Some(antes), Some(depois)) = (memoria_antes, memoria_em_mb()) {
        println!("ℹ️ memória: {antes:.0} MB → {depois:.0} MB");
    }
}

/// Monta o app dentro da Revelação com `n` fotos do site, todas marcadas —
/// o "Sincronizar N" já feito.
fn revelacao_com_lote(
    cx: &mut TestAppContext,
    n: usize,
    publicador: Arc<PublicadorDeMentira>,
) -> (gpui::WindowHandle<Aplicativo>, TempDir) {
    revelacao_com_lote_e_acervo(cx, n, publicador, Arc::new(AcervoDeMentira::default()))
}

/// O mesmo, com o catálogo por parâmetro.
///
/// 🔑 **O catálogo importa quando a frase precisa do nome do arquivo**: a raiz
/// relê o acervo ao longo do lote, e um catálogo vazio deixa a Biblioteca sem
/// nenhuma foto — é de lá que sai o nome que a recusa mostra ao operador.
fn revelacao_com_lote_e_acervo(
    cx: &mut TestAppContext,
    n: usize,
    publicador: Arc<PublicadorDeMentira>,
    acervo: Arc<AcervoDeMentira>,
) -> (gpui::WindowHandle<Aplicativo>, TempDir) {
    use crate::revelacao::sincronizacao::Escolha;

    let (previews, dir) = previews_descartaveis();
    // Só a aberta precisa de pixels: as outras recebem a receita pelo banco.
    previews
        .save_preview("id-00000", &cinza(16))
        .expect("gravar preview");
    cx.update(gpui_component::init);
    let fotos: Vec<PhotoViewModel> = (0..n).map(|i| foto(i, true)).collect();
    let janela = cx.add_window(move |window, cx| {
        Aplicativo::ja_dentro(
            fotos,
            previews,
            Vec::new(),
            portas_com(publicador, Arc::new(GravadorDeMentira::default()), acervo),
            window,
            cx,
        )
    });
    janela
        .update(cx, |app, window, cx| {
            app.tela = Tela::Biblioteca;
            app.biblioteca
                .update(cx, |tela, cx| tela.selecionar(Some(0), cx));
            app.revelar(window, cx);
            assert_eq!(app.tela(), Tela::Revelacao);
            app.revelacao.update(cx, |tela, cx| {
                tela.aplicar_para_teste(0, 1.0, cx);
                tela.marcar_todas(cx);
                tela.definir_escolha_da_sincronizacao(Escolha::default());
            });
            cronometrar(
                &format!("Sincronizar {n}"),
                Duration::from_millis(100 + n as u64 * 2),
                || app.sincronizar_revelacao(window, cx),
            );
        })
        .expect("a janela aberta");
    (janela, dir)
}

/// O que as quatro vistas do envio dizem agora: o contador, o "Salvando k/N",
/// o canto dos envios e a bandeja. **Elas não podem discordar.**
fn conferir_os_contadores(
    app: &Aplicativo,
    cx: &gpui::App,
    esperadas: usize,
    total: usize,
    respondidas: usize,
) {
    assert_eq!(app.sincronias_pendentes(), esperadas, "o contador");
    let retrato = app.retrato_do_segundo_plano(cx);
    assert_eq!(retrato.subindo, esperadas, "a bandeja");
    assert_eq!(retrato.ha_envio_pendente(), esperadas > 0, "o G9");
    // 🔑 **O terceiro número saiu do botão e foi para o lote** (dono,
    // 18/set/2026): o editor fecha no clique, e quem conta as respostas é
    // `lote_no_ar` — é ele que decide quando avisar, uma vez só.
    let lote = app.lote_no_ar_para_teste();
    if respondidas < total {
        assert_eq!(
            lote.map(|(t, feitas, _)| (feitas, t)),
            Some((respondidas, total)),
            "o lote ainda no ar"
        );
    } else {
        assert_eq!(lote, None, "o lote acabou");
    }
}

/// 🚨 **"Salvar na galeria" com 300 fotos, respostas embaralhadas e falhas.**
///
/// O cenário do casamento: o lote inteiro sobe de uma vez, as respostas voltam
/// na ordem que a rede quiser, uma em cada dez falha, e o operador fecha a
/// janela no meio. Os quatro números que falam do envio — o contador da raiz, o
/// "Salvando k/N" do botão, a bandeja e a decisão do G9 — têm de andar juntos,
/// nunca negativos, e zerar no fim; e a janela escondida tem de poder sair.
#[gpui::test]
fn estresse_salvar_trezentas_na_galeria_com_desordem_e_falhas(cx: &mut TestAppContext) {
    const N: usize = 300;
    let publicador = Arc::new(PublicadorDeMentira {
        demorada: true,
        ..Default::default()
    });
    let (janela, _dir) = revelacao_com_lote(cx, N, publicador.clone());

    janela
        .update(cx, |app, window, cx| {
            cronometrar(
                "Salvar na galeria (300)",
                Duration::from_millis(500),
                || app.salvar_na_galeria(window, cx),
            );
            conferir_os_contadores(app, cx, N, N, 0);
            // Um segundo clique no meio do lote não manda nada de novo.
            app.salvar_na_galeria(window, cx);
            conferir_os_contadores(app, cx, N, N, 0);
        })
        .expect("a janela aberta");
    // 🚨 **Três em voo, e não trezentas** (dono, 18/set/2026: a aplicação
    // estourava a memória ao salvar em segundo plano). Cada foto no ar é um
    // original baixado e decodificado — 96 MB em RAM —, e o lote inteiro de uma
    // vez levava a máquina do balcão junto. O contador de espera continua sendo
    // o lote inteiro: todas as respostas virão, uma vaga de cada vez.
    assert_eq!(
        publicador.reveladas().len(),
        crate::envios::EM_VOO,
        "só as primeiras saem; as outras esperam vaga"
    );

    // O operador fecha a janela com o lote no ar: G9.
    let mut vigia = Vigia::default();
    assert_eq!(vigia.ao_fechar(true), AoFechar::Esconder);

    // O primeiro quadro da Revelação custa segundos no perfil de teste (fontes,
    // tema, a GPU abrindo): aquecido antes, para a régua medir a colheita.
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.run_until_parked();

    let mut g = Lcg::novo(41);
    let mut ja_falharam: std::collections::HashSet<String> = std::collections::HashSet::new();
    let inicio = Instant::now();
    let mut voltas = 0u32;
    let mut respondidas = 0;
    let mut falhas = 0;
    let mut i = 0usize;
    while respondidas < N {
        voltas += 1;
        // 🔑 **As respostas aparecem aos poucos**, porque o despacho é aos
        // poucos: o que está guardado agora é o que está no ar.
        let mut recados: Vec<_> =
            std::mem::take(&mut *publicador.guardados.lock().expect("guardados"));
        assert!(
            recados.len() <= crate::envios::EM_VOO,
            "mais de {} fotos no ar: {}",
            crate::envios::EM_VOO,
            recados.len()
        );
        g.embaralhar(&mut recados);
        for (_, recado) in recados.iter_mut() {
            // 🚨 **Uma em cada dez leva um 502 na primeira tentativa** — e só
            // na primeira: é a rede do balcão oscilando, não o site recusando.
            // A esteira tem de trazer essa foto de volta sozinha (dono,
            // 18/set/2026: *"essa rotina precisa ser um tanque de guerra!"*).
            if i % 10 == 3 {
                if let Recado::RevelacaoSalva { foto_no_site } = recado {
                    let alvo = foto_no_site.clone();
                    if ja_falharam.insert(alvo.clone()) {
                        *recado = Recado::EnvioFalhou {
                            alvo,
                            frase: format!("502 na foto {i}"),
                        };
                        falhas += 1;
                        // A tentativa que falhou não fecha a foto: ela volta
                        // para a fila, e a resposta boa vem numa volta adiante.
                        respondidas -= 1;
                    }
                }
            }
            i += 1;
        }
        let lote = recados.len();
        for (canal, recado) in recados.drain(..) {
            canal.send(recado).expect("o canal da raiz");
        }
        respondidas += lote;
        // Metade das vezes quem colhe é o laço de espera (o relógio andando);
        // na outra metade, a colheita é chamada no meio do caminho.
        if g.ate(2) == 0 {
            cx.executor().advance_clock(Duration::from_millis(150));
            cx.run_until_parked();
        } else {
            janela
                .update(cx, |app, _window, cx| {
                    app.colher_sincronia(cx);
                })
                .expect("a janela aberta");
        }
        janela
            .update(cx, |app, _window, cx| {
                // ⚠️ **Com repetição, a conta do lote não é a das respostas**:
                // a tentativa que falhou respondeu e voltou para a fila, e a
                // foto ainda deve uma resposta. O que continua valendo é o
                // sentido: ainda há envio pendente até a última subir.
                assert!(
                    !vigia.deve_sair(app.retrato_do_segundo_plano(cx).ha_envio_pendente())
                        || respondidas == N
                );
            })
            .expect("a janela aberta");
    }
    let gasto = inicio.elapsed();
    relatar(
        &format!("colher 300 respostas embaralhadas ({voltas} voltas, com o quadro de cada uma)"),
        gasto,
        Duration::from_secs(3),
    );
    assert!(gasto <= Duration::from_secs(3));

    // Recados que ninguém pediu (um eco atrasado) não fazem o contador dar a
    // volta.
    let canal = janela
        .update(cx, |app, _window, _cx| app.canal_da_sincronia_para_teste())
        .expect("a janela aberta");
    for _ in 0..50 {
        canal.send(Recado::Sincronizou).expect("o canal");
    }
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();

    janela
        .update(cx, |app, _window, cx| {
            conferir_os_contadores(app, cx, 0, N, N);
            let retrato = app.retrato_do_segundo_plano(cx);
            // 🚨 **Nenhuma recusa**: as trinta que levaram 502 voltaram
            // sozinhas e subiram na segunda tentativa. Antes de a esteira
            // repetir, cada uma dessas era uma foto perdida com um aviso no
            // canto.
            assert_eq!(
                retrato.recusadas, 0,
                "a repetição tinha de ter salvado as {falhas} que falharam uma vez"
            );
            assert!(falhas > 20, "o cenário precisa ter falhado: {falhas}");
            assert_eq!(
                publicador.reveladas().len(),
                N + falhas,
                "cada falha custa uma tentativa a mais, e nenhuma foto sobe \
                 duas vezes de graça"
            );
            assert!(
                vigia.deve_sair(retrato.ha_envio_pendente()),
                "G9: a fila esvaziou, o app sai"
            );
            // 🔑 **A tela saiu no clique** (o lote sobe em segundo plano), e o
            // que não subiu continua na fila: é a receita no depósito que
            // protege o trabalho, não a tela parada.
            assert_eq!(app.tela(), Tela::Sessao);
            assert!(app.revelacao.read(cx).ha_o_que_salvar());
        })
        .expect("a janela aberta");
    println!(
        "ℹ️ a galeria foi relida {} vez(es) para {} respostas",
        publicador.abertas().len(),
        N - falhas
    );
}

/// 🚨 **O tanque de guerra: 400 fotos, rede oscilando e um site que às vezes
/// nunca aceita.**
///
/// Dono, 18/set/2026: *"essa rotina precisa ser um tanque de guerra!"*. O
/// cenário junta os três desfechos que o balcão vê num sábado:
///
/// - **a maioria sobe de primeira**;
/// - **uma em oito leva um erro de momento** (o 502, o Wi-Fi que oscilou) e tem
///   de subir sozinha na tentativa seguinte, sem ninguém apertar nada;
/// - **uma em cinquenta é recusa de verdade** (a foto foi apagada no site) e
///   nunca vai passar: depois de [`TENTATIVAS`] a esteira desiste — e o
///   operador tem de **saber qual arquivo** ficou para trás.
///
/// O que ele prende, além dos números: nenhuma foto sobe duas vezes de graça,
/// o teto de [`EM_VOO`] vale o tempo todo (é ele que segura a memória), e os
/// contadores voltam a zero mesmo com recuo e repetição no meio.
#[gpui::test]
fn estresse_o_lote_sobrevive_a_rede_ruim(cx: &mut TestAppContext) {
    const N: usize = 400;
    let publicador = Arc::new(PublicadorDeMentira {
        demorada: true,
        ..Default::default()
    });
    // A rede do balcão, programada: quantas vezes cada foto ainda vai falhar.
    let mut intermitentes: Vec<String> = Vec::new();
    let mut perdidas: Vec<String> = Vec::new();
    {
        let mut falhas = publicador.falhas_por_foto.lock().expect("as falhas");
        for i in 0..N {
            let alvo = format!("remota-{i:05}");
            if i.is_multiple_of(50) {
                falhas.insert(alvo.clone(), u8::MAX);
                perdidas.push(alvo);
            } else if i.is_multiple_of(8) {
                falhas.insert(alvo.clone(), 1);
                intermitentes.push(alvo);
            }
        }
    }
    let acervo = Arc::new(AcervoDeMentira {
        fotos: std::sync::Mutex::new((0..N).map(|i| foto(i, true)).collect()),
        ..Default::default()
    });
    let (janela, _dir) = revelacao_com_lote_e_acervo(cx, N, publicador.clone(), acervo);

    janela
        .update(cx, |app, window, cx| app.salvar_na_galeria(window, cx))
        .expect("a janela aberta");

    // A rede responde, embaralhada, até a esteira parar.
    let mut g = Lcg::novo(77);
    let inicio = Instant::now();
    let mut voltas = 0;
    loop {
        voltas += 1;
        assert!(voltas < 1_000, "o lote não terminou em 1.000 voltas");
        let mut recados: Vec<_> =
            std::mem::take(&mut *publicador.guardados.lock().expect("guardados"));
        assert!(
            recados.len() <= crate::envios::EM_VOO,
            "🚨 {} fotos no ar: o teto é {} — é ele que segura a memória",
            recados.len(),
            crate::envios::EM_VOO
        );
        g.embaralhar(&mut recados);
        for (canal, recado) in recados.drain(..) {
            canal.send(recado).expect("o canal da raiz");
        }
        // O relógio anda o bastante para os recuos das repetições vencerem.
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.run_until_parked();

        let acabou = janela
            .update(cx, |app, _window, cx| {
                app.colher_sincronia(cx);
                app.sincronias_pendentes() == 0
                    && publicador.guardados.lock().expect("guardados").is_empty()
            })
            .expect("a janela aberta");
        if acabou {
            break;
        }
    }
    relatar(
        &format!("{N} fotos com rede ruim ({voltas} voltas)"),
        inicio.elapsed(),
        Duration::from_secs(20),
    );

    // 🔑 **A conta das tentativas, foto a foto.** É ela que mostra que a
    // repetição existe e que não há tentativa a mais escondida.
    let mut tentativas: std::collections::HashMap<String, usize> = Default::default();
    for (alvo, _, _) in publicador.reveladas() {
        *tentativas.entry(alvo).or_default() += 1;
    }
    for alvo in &perdidas {
        assert_eq!(
            tentativas.get(alvo).copied(),
            Some(crate::envios::TENTATIVAS as usize),
            "a foto que nunca passa é tentada {} vezes, e só",
            crate::envios::TENTATIVAS
        );
    }
    for alvo in &intermitentes {
        assert_eq!(
            tentativas.get(alvo).copied(),
            Some(2),
            "a foto do 502 sobe na segunda — sem ninguém apertar nada"
        );
    }
    let normais = N - perdidas.len() - intermitentes.len();
    assert_eq!(
        tentativas.len(),
        N,
        "toda foto do lote foi ao site pelo menos uma vez"
    );
    assert_eq!(
        publicador.reveladas().len(),
        normais + intermitentes.len() * 2 + perdidas.len() * crate::envios::TENTATIVAS as usize,
        "nenhuma tentativa a mais, nenhuma a menos"
    );

    janela
        .update(cx, |app, _window, cx| {
            // 🚨 **O que ficou para trás tem nome.** Um id não diz ao operador
            // qual foto repetir, e é ele quem vai repetir.
            let recusas = app.recusas_para_teste();
            assert_eq!(
                recusas.len(),
                perdidas.len(),
                "uma recusa por foto que nunca passou: {recusas:?}"
            );
            for i in (0..N).step_by(50) {
                let nome = format!("DSC_{i:05}.jpg");
                assert!(
                    recusas.iter().any(|r| r.starts_with(&nome)),
                    "a recusa de {nome} tinha de estar no canto: {recusas:?}"
                );
            }
            // E as contas voltam a zero, com repetição e recuo no meio.
            assert_eq!(app.sincronias_pendentes(), 0, "o contador zerou");
            let retrato = app.retrato_do_segundo_plano(cx);
            assert_eq!(retrato.subindo, 0, "a bandeja esvaziou");
            assert_eq!(
                retrato.recusadas,
                perdidas.len(),
                "o canto guarda as {perdidas:?}"
            );
            assert!(
                !retrato.ha_envio_pendente(),
                "G9: o app pode sair — não há envio no ar"
            );
        })
        .expect("a janela aberta");
}

/// 🚨 **A resposta que demora mais que a espera não pode prender o contador.**
///
/// O `reqwest` da API espera até 180 s por pedido, e salvar uma foto são três
/// deles (baixar o original, pedir o bilhete, subir o JPEG). O laço de espera
/// desistia em 30 s por resposta: a que chegasse depois ficava no canal sem
/// ninguém para lê-la — o botão preso em "Salvando 0/1", o salvar recusando
/// qualquer clique novo, e o G9 segurando a janela escondida para sempre.
#[gpui::test]
fn estresse_a_resposta_atrasada_ainda_zera_o_contador(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDeMentira {
        demorada: true,
        ..Default::default()
    });
    let (janela, _dir) = revelacao_com_lote(cx, 2, publicador.clone());
    janela
        .update(cx, |app, window, cx| {
            app.salvar_na_galeria(window, cx);
            conferir_os_contadores(app, cx, 2, 2, 0);
        })
        .expect("a janela aberta");

    // A rede leva dois minutos — dentro dos 180 s do cliente HTTP.
    cx.executor().advance_clock(Duration::from_secs(120));
    cx.run_until_parked();
    publicador.responder();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();

    janela
        .update(cx, |app, _window, cx| {
            conferir_os_contadores(app, cx, 0, 2, 2);
            assert_eq!(app.tela(), Tela::Sessao, "o lote acabou bem: a tela sai");
        })
        .expect("a janela aberta");
}

// ─────────────────────────────────────────────── a grade da sessão, a tela principal

fn foto_do_site(i: usize) -> domain::services::pos_venda::FotoDaGaleria {
    use domain::services::pos_venda::EstadoDaFotoNoSite;
    domain::services::pos_venda::FotoDaGaleria {
        id: format!("remota-{i:05}"),
        arquivo: format!("DSC_{i:05}.jpg"),
        estado: match i % 10 {
            0 => EstadoDaFotoNoSite::Comprada,
            1 | 2 => EstadoDaFotoNoSite::LevadaNoBalcao,
            _ => EstadoDaFotoNoSite::Disponivel,
        },
        ordem: i as i32,
        preco_negociado: None,
        observacao_da_negociacao: None,
        apagada: i.is_multiple_of(97),
        nota: Some(1 + (i % 5) as u8),
        produto_efetivo: "p1".into(),
        preco_de_venda: None,
        pedido_id: None,
        downloads: 0,
        revelada: false,
        ajustes: None,
        ..Default::default()
    }
}

/// A tela da sessão com `n` fotos, já aberta e com as miniaturas chegadas.
fn sessao_com(
    cx: &mut TestAppContext,
    n: usize,
) -> (
    gpui::WindowHandle<Aplicativo>,
    Arc<PublicadorDeMentira>,
    TempDir,
    Duration,
) {
    use domain::services::pos_venda::GaleriaDoPainel;

    let (previews, dir) = previews_descartaveis();
    cx.update(gpui_component::init);
    let publicador = Arc::new(PublicadorDeMentira {
        galerias: std::sync::Mutex::new(vec![GaleriaDoPainel {
            id: "g1".into(),
            titulo: "Casamento".into(),
            email: Some("noivos@x.com".into()),
            whatsapp: None,
            produto_id: "p1".into(),
            user_id: None,
            criada_em_iso: "2026-09-17".into(),
            expira_em: None,
            fotos: Default::default(),
            totais: None,
            ..Default::default()
        }]),
        fotos_da_sessao: std::sync::Mutex::new((0..n).map(foto_do_site).collect()),
        ..Default::default()
    });
    let janela = cx.add_window({
        let publicador = publicador.clone();
        move |window, cx| {
            Aplicativo::novo(
                Vec::new(),
                previews,
                Vec::new(),
                portas(publicador, Arc::new(GravadorDeMentira::default())),
                window,
                cx,
            )
        }
    });
    let inicio = Instant::now();
    janela
        .update(cx, |app, _window, cx| {
            app.entrar_na_conta(
                domain::services::pos_venda::Sessao {
                    access_token: "tok".into(),
                    refresh_token: "ref".into(),
                    access_vence_em: i64::MAX,
                    refresh_vence_em: i64::MAX,
                },
                cx,
            );
            app.entrar_na_sessao("g1".into(), cx);
            assert_eq!(app.tela(), Tela::Sessao);
        })
        .expect("a janela aberta");
    // A galeria e as miniaturas chegam pelo laço de 100 ms da tela.
    for _ in 0..10 {
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.run_until_parked();
    }
    let abrir = inicio.elapsed();
    (janela, publicador, dir, abrir)
}

/// Um quadro da sessão, medido: a **mediana** de `quadros` desenhos, depois de
/// um de aquecimento.
///
/// ⚠️ Mediana, e não média: a suíte roda com a máquina ocupada (compilação ao
/// lado), e um quadro atropelado pelo sistema não diz nada sobre a grade.
fn quadro_da_sessao(
    cx: &mut TestAppContext,
    janela: gpui::WindowHandle<Aplicativo>,
    quadros: usize,
) -> Duration {
    let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
    let tamanho = gpui::size(gpui::px(1600.), gpui::px(1000.));
    visual.draw(gpui::Point::default(), tamanho, |_w, _cx| gpui::Empty);
    let mut tempos: Vec<Duration> = (0..quadros)
        .map(|_| {
            let inicio = Instant::now();
            janela
                .update(&mut visual, |app, _w, cx| {
                    app.detalhe.update(cx, |_d, cx| cx.notify())
                })
                .expect("a janela aberta");
            visual.draw(gpui::Point::default(), tamanho, |_w, _cx| gpui::Empty);
            inicio.elapsed()
        })
        .collect();
    tempos.sort();
    tempos[tempos.len() / 2]
}

/// O teto de miniaturas na memória na janela de teste (1920 × 1080, zoom
/// padrão) — **o mesmo para 300 e para 10.000 fotos**. É a grade à vista mais
/// uma tela de cada lado, e a tira com o pedaço que ela desenha; medido em
/// 17/set/2026: 110. Antes o cache guardava uma por foto, sem teto.
const TETO_DE_MINIATURAS: usize = 160;

/// 🚨 **A grade da sessão — a tela em que o app abre — com 300, 2.000 e
/// 10.000 fotos.**
///
/// Até 17/set/2026 a grade e a tira montavam o recorte **inteiro** a cada
/// quadro, e o cache de miniaturas crescia até ele: 76 ms por quadro com 300
/// fotos e 537 ms com 2.000. Agora a grade monta só as linhas à vista, a
/// tira desenha só o pedaço à vista, e o cache tem teto. O teste afirma o
/// orçamento de 60 fps e o teto — no começo e depois de andar até o fim.
///
/// ⚠️ **O tempo só vale sozinho**: com a suíte inteira em paralelo o quadro
/// passa do orçamento (medido: 30 ms). Por isso há duas entradas: a da suíte,
/// com 300 fotos, confere pedidos, contadores e o teto de memória sem afirmar
/// tempo; a pesada (`--ignored`) acrescenta 2.000 e 10.000 e o tempo.
#[gpui::test]
fn a_grade_da_sessao_tem_teto_e_contas_certas(cx: &mut TestAppContext) {
    percorrer_a_sessao(cx, &[300], false);
}

#[gpui::test]
#[ignore = "pesado: 10.000 fotos e tempo de quadro afirmado — rode com --ignored --nocapture"]
fn estresse_a_grade_da_sessao(cx: &mut TestAppContext) {
    percorrer_a_sessao(cx, &[300, 2_000, 10_000], true);
}

fn percorrer_a_sessao(cx: &mut TestAppContext, tamanhos: &[usize], afirmar_tempo: bool) {
    for &n in tamanhos {
        let (janela, publicador, _dir, abrir) = sessao_com(cx, n);
        relatar(
            &format!("abrir a sessão de {n} (galeria + miniaturas)"),
            abrir,
            Duration::from_secs(60),
        );
        let visiveis = janela
            .update(cx, |app, _window, cx| {
                let detalhe = app.detalhe.read(cx);
                assert!(detalhe.aberta().is_some(), "a galeria abriu");
                detalhe.total_visivel()
            })
            .expect("a janela aberta");
        // "Todas" mostra as apagadas também — desbotadas, como no site.
        assert_eq!(visiveis, n);

        // Um quadro com tudo carregado.
        let orcamento = Duration::from_micros(16_700);
        let quadro = quadro_da_sessao(cx, janela, 21);
        relatar(
            &format!("um quadro da sessão de {n} (orçamento de 60 fps: 16,7 ms)"),
            quadro,
            orcamento,
        );
        assert!(
            !afirmar_tempo || quadro <= orcamento,
            "um quadro de {quadro:?} com {n} fotos"
        );
        let guardadas = janela
            .update(cx, |app, _w, cx| {
                app.detalhe.read(cx).miniaturas_na_memoria()
            })
            .expect("a janela aberta");
        println!("ℹ️ sessão de {n}: {guardadas} miniatura(s) na memória");
        assert!(
            guardadas <= TETO_DE_MINIATURAS,
            "{guardadas} miniaturas na memória com {n} fotos"
        );

        janela
            .update(cx, |app, _window, cx| {
                app.detalhe.update(cx, |d, cx| {
                    cronometrar(
                        &format!("andar {n} fotos pela seta na sessão"),
                        Duration::from_millis(500),
                        || {
                            for _ in 0..n + 3 {
                                d.andar(1, cx);
                            }
                        },
                    );
                    assert_eq!(d.posicao_em_foco(), Some(visiveis - 1), "para no fim");
                });
            })
            .expect("a janela aberta");

        // No fim da sessão: a grade e a tira seguiram o foco, e o quadro e a
        // memória continuam os mesmos do começo.
        let quadro = quadro_da_sessao(cx, janela, 21);
        relatar(
            &format!("um quadro no fim da sessão de {n}, com o painel"),
            quadro,
            orcamento,
        );
        assert!(
            !afirmar_tempo || quadro <= orcamento,
            "um quadro de {quadro:?} no fim de {n}"
        );
        janela
            .update(cx, |app, _window, cx| {
                let guardadas = app.detalhe.read(cx).miniaturas_na_memoria();
                assert!(
                    guardadas <= TETO_DE_MINIATURAS,
                    "{guardadas} miniaturas na memória no fim de {n}"
                );
                app.detalhe.update(cx, |d, cx| {
                    cronometrar(&format!("⌘A em {n}"), Duration::from_millis(200), || {
                        d.selecionar_tudo(cx)
                    });
                    assert_eq!(d.quantas_marcadas(), visiveis);
                    cronometrar(
                        &format!("dar ★★★★ a {n} de uma vez"),
                        Duration::from_millis(500),
                        || d.dar_nota(4, cx),
                    );
                });
            })
            .expect("a janela aberta");
        let editaveis = (0..n).filter(|i| i % 97 != 0 && i % 10 != 0).count();
        assert_eq!(
            publicador.negociadas().len(),
            editaveis,
            "um pedido por foto editável"
        );

        // As respostas voltam; o contador da tela volta a zero e aceita outra rodada.
        for _ in 0..5 {
            cx.executor().advance_clock(Duration::from_millis(250));
            cx.run_until_parked();
        }
        janela
            .update(cx, |app, _window, cx| {
                app.detalhe.update(cx, |d, cx| {
                    d.selecionar_tudo(cx);
                    d.marcar_como(
                        domain::services::pos_venda::EstadoNoBalcao::LevadaNoBalcao,
                        cx,
                    );
                })
            })
            .expect("a janela aberta");
        assert_eq!(
            publicador.negociadas().len(),
            2 * editaveis,
            "a segunda rodada saiu: o contador de mudanças voltou a zero"
        );
        cx.executor().advance_clock(Duration::from_secs(2));
        cx.run_until_parked();
        println!(
            "ℹ️ sessão de {n}: a galeria foi relida {} vez(es)",
            publicador.abertas().len()
        );
    }
}

/// 🚨 **A galeria continua sendo a tela do cliente enquanto o lote sobe.**
///
/// Dono, 18/set/2026: *"da forma que ficou eu não tenho a galeria liberada para
/// ir mostrando as fotos para o cliente e isso deixa a UX muito ruim. Mas na
/// WEB eu consigo!"*
///
/// O cenário é o balcão de verdade: 300 fotos na galeria, o operador com vinte
/// marcadas e o cliente ao lado olhando, e **duzentas** revelações voltando do
/// site enquanto isso. Cada resposta manda a raiz reler a galeria — e é aí que
/// a tela do operador se perde, ou não.
///
/// O que este cenário afirma, e que a releitura por `entrar` quebrava:
///
/// 1. **a grade nunca fica vazia** — `entrar` zera `do_site` e só repõe quando
///    a resposta chega, e nesse vão o cliente vê a tela em branco;
/// 2. **as miniaturas não são baixadas de novo** — `entrar` esvazia o cache, e
///    a foto some da célula até o download voltar;
/// 3. **a seleção do operador sobrevive às duzentas** — era `limpar_tudo` a
///    cada resposta;
/// 4. **as releituras são poucas** — agrupadas pelo respiro, e não uma por foto.
#[gpui::test]
fn estresse_a_galeria_de_pe_durante_duzentas_revelacoes(cx: &mut TestAppContext) {
    const N: usize = 300;
    const LOTE: usize = 200;

    let (janela, publicador, _dir, _abrir) = sessao_com(cx, N);
    let aberturas_antes = publicador.abertas().len();
    // A abertura pede uma miniatura por foto **que tem miniatura**: a grade
    // mostra as apagadas desbotadas, sem pedir arquivo nenhum.
    let na_abertura = publicador.miniaturas_pedidas();
    let miniaturas_antes = na_abertura.len();
    let com_miniatura: std::collections::HashSet<String> = na_abertura.into_iter().collect();
    assert!(
        miniaturas_antes > N * 9 / 10,
        "{miniaturas_antes} miniaturas"
    );

    // O operador está com o cliente: vinte fotos marcadas e uma em foco.
    let (todas, marcadas) = janela
        .update(cx, |app, _window, cx| {
            let todas = app.detalhe.read(cx).ids_visiveis();
            let marcadas: Vec<String> = todas.iter().take(20).cloned().collect();
            app.detalhe.update(cx, |d, cx| {
                d.marcar_ids(&marcadas, cx);
            });
            (todas, marcadas)
        })
        .expect("a janela aberta");
    assert_eq!(todas.len(), N, "a galeria abriu inteira");

    // E o lote sobe: duzentas revelações confirmadas pelo site, uma a uma,
    // como a rede as traz.
    let canal = janela
        .update(cx, |app, _window, _cx| app.canal_da_sincronia_para_teste())
        .expect("a janela aberta");
    let mut menor_grade = usize::MAX;
    let mut menor_cache = usize::MAX;
    let inicio = Instant::now();
    for (i, id) in todas.iter().take(LOTE).enumerate() {
        canal
            .send(Recado::RevelacaoSalva {
                foto_no_site: id.clone(),
            })
            .expect("o canal aberto");
        janela
            .update(cx, |app, _window, cx| {
                app.colher_sincronia(cx);
                let d = app.detalhe.read(cx);
                menor_grade = menor_grade.min(d.total_visivel());
                menor_cache = menor_cache.min(d.miniaturas_na_memoria());
            })
            .expect("a janela aberta");
        // A cada vinte, o relógio anda: é quando a releitura agrupada dispara e
        // a galeria volta do site — o momento exato em que a tela piscava.
        if i.is_multiple_of(20) {
            cx.executor().advance_clock(Duration::from_millis(500));
            cx.run_until_parked();
            janela
                .update(cx, |app, _window, cx| {
                    let d = app.detalhe.read(cx);
                    menor_grade = menor_grade.min(d.total_visivel());
                    menor_cache = menor_cache.min(d.miniaturas_na_memoria());
                })
                .expect("a janela aberta");
        }
    }
    relatar(
        &format!("{LOTE} revelações confirmadas com a galeria de {N} aberta"),
        inicio.elapsed(),
        Duration::from_secs(20),
    );

    // A poeira baixa: a última releitura agrupada chega.
    for _ in 0..8 {
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.run_until_parked();
    }

    // 1 e 2 — o que o cliente via.
    assert_eq!(
        menor_grade, N,
        "a grade esvaziou no meio do lote: o cliente viu a tela em branco"
    );
    assert!(
        menor_cache > 0,
        "as miniaturas sumiram da memória no meio do lote"
    );
    // 🚨 **Uma re-descida por foto que subiu, e nenhuma a mais** (dono,
    // 18/set/2026: *"não travou, mas não fez a atualização das miniaturas"*).
    // A miniatura guardada é a de antes do envio: se ninguém a pedir de novo, a
    // célula continua mostrando o "antes" — e se a galeria inteira for pedida a
    // cada releitura, volta o custo do qual se saiu.
    let ultimas: Vec<String> = publicador.miniaturas_pedidas().split_off(miniaturas_antes);
    let rebaixadas = ultimas.len();
    let esperadas: Vec<String> = todas
        .iter()
        .take(LOTE)
        .filter(|id| com_miniatura.contains(*id))
        .cloned()
        .collect();
    assert_eq!(
        ultimas, esperadas,
        "as miniaturas pedidas de novo têm de ser exatamente as {LOTE} que \
         subiram, na ordem em que o site respondeu"
    );

    // 3 e 4 — o que o operador tinha na mão, e o preço da atualização.
    let releituras = publicador.abertas().len() - aberturas_antes;
    println!(
        "ℹ️ {LOTE} respostas → {releituras} releitura(s) da galeria, \
         {rebaixadas} miniatura(s) baixada(s) de novo"
    );
    assert!(
        releituras <= LOTE / 10,
        "{releituras} releituras para {LOTE} respostas — a galeria está sendo \
         reaberta foto a foto"
    );
    janela
        .update(cx, |app, _window, cx| {
            let d = app.detalhe.read(cx);
            assert_eq!(d.total_visivel(), N, "a galeria continua inteira no fim");
            assert_eq!(
                d.marcadas(),
                marcadas,
                "a seleção do operador sobreviveu às {LOTE} respostas"
            );
        })
        .expect("a janela aberta");

    // E o quadro, no fim do lote: a tela que o cliente está olhando.
    let quadro = quadro_da_sessao(cx, janela, 21);
    relatar(
        "um quadro da galeria logo depois do lote",
        quadro,
        Duration::from_millis(100),
    );
}

// ─────────────────────────────────────────────────────────────── o caixa

/// 🚨 **Um cupom de 800 fotos, com toda negociação que existe.**
///
/// O cupom é recalculado a cada releitura da galeria. Aqui, a conta: soma
/// fechando, ordem estável, e nada quadrático.
#[test]
fn estresse_o_cupom_com_oitocentas_fotos() {
    use biblioteca_core::acervo::Estado;
    use biblioteca_core::caixa::{self, Faixa, FotoDoCupom};

    const N: usize = 800;
    let mut g = Lcg::novo(43);
    let observacoes = [
        None,
        Some("Cortesia".to_string()),
        Some("Desconto — aniversário".to_string()),
        Some("Parceiro — TchêOfertas — cupom ABC123".to_string()),
        Some("Outro — acerto de balcão".to_string()),
    ];
    let fotos: Vec<FotoDoCupom> = (0..N)
        .map(|i| {
            let obs = observacoes[g.ate(observacoes.len())].clone();
            FotoDoCupom {
                id: format!("f{i}"),
                ordem: (N - i) as i64 % 97,
                arquivo: format!("DSC_{i}.jpg"),
                estado: if i.is_multiple_of(10) {
                    Estado::Disponivel
                } else {
                    Estado::LevadaNoBalcao
                },
                apagada: i.is_multiple_of(50),
                nota: Some(3),
                sem_marcacao: false,
                produto_efetivo: if i.is_multiple_of(2) { "p1" } else { "p2" }.into(),
                produto_id: None,
                preco_negociado: obs.as_ref().map(|_| g.ate(5000) as i64),
                observacao: obs,
            }
        })
        .collect();
    let vendidas = (0..N)
        .filter(|i| i.is_multiple_of(7))
        .map(|i| format!("f{i}"))
        .collect();
    let faixa = |f: &FotoDoCupom| Faixa {
        nome: f.produto_efetivo.clone(),
        preco: if f.produto_efetivo == "p1" {
            3000
        } else {
            6000
        },
    };

    let cupom = cronometrar(
        "fechar o cupom de 800 (×100)",
        Duration::from_secs(2),
        || {
            let mut ultimo = None;
            for _ in 0..100 {
                ultimo = Some(caixa::fechar_caixa(&fotos, faixa, &vendidas));
            }
            ultimo.expect("cem cupons")
        },
    );

    assert!(cupom.itens.len() > 500, "{} itens", cupom.itens.len());
    let subtotal: i64 = cupom.itens.iter().map(|i| i.cheio).sum();
    let total: i64 = cupom.itens.iter().map(|i| i.cobrado).sum();
    assert_eq!(cupom.subtotal, subtotal);
    assert_eq!(cupom.total, total);
    assert_eq!(
        cupom.subtotal,
        cupom.total + cupom.descontos + cupom.pago_em_parceiro,
        "a conta do cupom fecha"
    );
    assert!(cupom
        .itens
        .windows(2)
        .all(|par| (par[0].ordem, &par[0].foto_id) <= (par[1].ordem, &par[1].foto_id)));
    assert!(cupom.itens.iter().all(|i| i.cobrado >= 0));

    // E o pagamento, lançado em centenas de parcelas.
    let parcelas: Vec<_> = (0..300)
        .map(|i| caixa::PagamentoLancado {
            forma: caixa::FormaDePagamento::TODAS[i % caixa::FormaDePagamento::TODAS.len()],
            valor: 1 + (i as i64 % 17),
            detalhe: String::new(),
        })
        .collect();
    let conta = caixa::conta_do_pagamento(cupom.total, &parcelas);
    assert!(conta.falta >= 0 && conta.troco >= 0 && conta.excedente_fora_do_dinheiro >= 0);
}

// ────────────────────────────────────────────────── o motor, em tamanho de câmera

/// Uma foto de 24 MP (6000 × 4000) com gradiente, para o motor não pular nada.
fn vinte_e_quatro_mp() -> (Arc<Vec<u8>>, u32, u32) {
    let (l, a) = (6000u32, 4000u32);
    let mut pixels = Vec::with_capacity((l * a * 4) as usize);
    for y in 0..a {
        for x in 0..l {
            pixels.extend_from_slice(&[
                (x % 256) as u8,
                (y % 256) as u8,
                ((x + y) % 256) as u8,
                255,
            ]);
        }
    }
    (Arc::new(pixels), l, a)
}

/// 🚨 **Enquadrar e medir 24 MP na CPU** — o que a tela faz a cada troca de
/// ângulo com o bruto na tela, e o que a exportação faz em toda foto.
#[test]
#[ignore = "pesado: 24 MP na CPU — rode com --ignored --nocapture"]
fn estresse_enquadrar_vinte_e_quatro_mp_na_cpu() {
    use crate::revelacao::histograma::Histograma;
    use domain::value_objects::CropSettings;
    use infrastructure::transformacao;

    let (pixels, l, a) = vinte_e_quatro_mp();
    let imagem = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(l, a, pixels.as_ref().clone()).expect("24 MP"),
    );
    let reto = CropSettings::new(0.1, 0.1, 0.8, 0.8, 0, 0., false, false);
    let torto = CropSettings::new(0.1, 0.1, 0.8, 0.8, 1, 7.3, true, false);

    let saida = cronometrar(
        "recortar 24 MP (sem ângulo)",
        Duration::from_secs(3),
        || transformacao::aplicar(&imagem, &reto, true),
    );
    assert_eq!((saida.width(), saida.height()), (4800, 3200));
    let saida = cronometrar(
        "girar, espelhar e endireitar 24 MP",
        Duration::from_secs(10),
        || transformacao::aplicar(&imagem, &torto, true),
    );
    assert!(saida.width() > 0 && saida.height() > 0);
    cronometrar("histograma de 24 MP", Duration::from_secs(2), || {
        Histograma::da_imagem(&saida)
    });
}

/// 🚨 **O motor de verdade em 24 MP, trinta vezes** — a sessão de 30 fotos da
/// meta do dono, cada uma revelada em resolução cheia para subir.
///
/// ⚠️ Precisa de GPU. Sem adaptador o teste avisa e sai: não há caminho de CPU,
/// de propósito (`processador.rs`).
#[test]
#[ignore = "pesado e com GPU — rode com --ignored --nocapture"]
fn estresse_o_motor_em_vinte_e_quatro_mp() {
    use infrastructure::gpu_adjustments::{Ajustes, Motor};

    let Some(mut motor) = Motor::abrir() else {
        println!("⚠️ sem GPU nesta máquina: o motor não abriu");
        return;
    };
    println!("ℹ️ motor: {}", motor.backend());
    let (pixels, l, a) = vinte_e_quatro_mp();
    let ajustes = Ajustes {
        exposure: 0.7,
        contrast: 1.2,
        highlights: -40.,
        shadows: 30.,
        temperature: 12.,
        ..Ajustes::default()
    };

    let primeira = cronometrar(
        "revelar 24 MP (a primeira, com upload)",
        Duration::from_secs(20),
        || {
            motor
                .revelar(&pixels, l, a, &ajustes)
                .expect("a GPU respondeu")
        },
    );
    assert_eq!((primeira.width(), primeira.height()), (l, a));

    let mut tempos = Vec::new();
    for i in 0..30 {
        let mut a_vez = ajustes;
        a_vez.exposure = -1. + i as f32 * 0.07;
        let inicio = Instant::now();
        let revelada = motor
            .revelar(&pixels, l, a, &a_vez)
            .expect("a GPU respondeu");
        tempos.push(inicio.elapsed());
        assert_eq!(revelada.width(), l);
    }
    tempos.sort();
    let mediana = tempos[tempos.len() / 2];
    let pior = *tempos.last().expect("trinta");
    relatar(
        "revelar 24 MP (mediana de 30)",
        mediana,
        Duration::from_secs(5),
    );
    relatar("revelar 24 MP (pior de 30)", pior, Duration::from_secs(10));
    let total: Duration = tempos.iter().sum();
    println!("ℹ️ 30 revelações em 24 MP: {:.1} s", total.as_secs_f64());
    assert!(mediana <= Duration::from_secs(5));
    if let Some(mb) = memoria_em_mb() {
        println!("ℹ️ memória depois de 30 revelações: {mb:.0} MB");
    }
}
