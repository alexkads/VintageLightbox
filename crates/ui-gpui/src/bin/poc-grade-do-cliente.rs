//! 🧪 **POC** — a tela do cliente em **grade**, com o número de cada foto
//! embaixo (pedido do dono, 11/out/2026). Só a janela do cliente, com as fotos
//! de uma pasta: sem conta, sem sessão, sem API. É para olhar no monitor do
//! balcão e decidir o desenho — **não é o app**, e não muda nada nele.
//!
//! ```bash
//! cargo run --release -p ui-gpui --bin poc-grade-do-cliente -- ~/Pictures/ensaio
//! cargo run --release -p ui-gpui --bin poc-grade-do-cliente        # fotos de mentira
//! ```
//!
//! | tecla | faz |
//! |---|---|
//! | `2`…`8`, `+`, `-` | fotos por linha |
//! | setas, `PageUp`/`PageDown` | o foco do operador (a página acompanha) |
//! | `Q` | liga e desliga o QR — em coluna própria, nunca sobre as fotos |
//! | `B` | a foto em foco vira "Escolhida" (e volta) |
//! | `X` | rejeita a foto em foco: ela some e o número fica vago |
//! | `U` | traz as rejeitadas de volta |
//! | `N` | tamanho do número: pequeno, médio, grande |
//! | `G` | grade ↔ foto única |
//! | `F` | tela cheia |
//! | `Esc` | fecha |
//!
//! A conta (células, páginas, números, a coluna do QR) é a de
//! `biblioteca_core::grade_do_cliente`, que tem os testes.
//!
//! **Roteiro** (`VLB_ROTEIRO=<arquivo>` e `VLB_FOTOS=<pasta>`), para conferir
//! sem ninguém na frente da tela: `tecla q` · `tamanho 1920 1080` ·
//! `esperar 300` · `foto 01-grade` · `fim`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use biblioteca_core::grade_do_cliente::{self as conta, Arranjo, Item};
use gpui_kit::{
    div, img, prelude::*, px, size, Animation, AnimationExt, App, AppContext, Bounds, Context,
    FocusHandle, KeyDownEvent, Pixels, RenderImage, SharedString, Size, Window, WindowBounds,
    WindowOptions,
};

/// Quantas fotos a pasta empresta à POC, no máximo. É uma página de balcão
/// várias vezes; mais do que isso só atrasa a abertura.
const TETO_DE_FOTOS: usize = 120;

/// O maior lado da foto na memória: sobra para duas por linha num 4K.
const LADO_DA_FOTO: u32 = 1600;

/// A faixa do pé: qual página está no ar. Fica fora da conta das células.
const ALTURA_DO_PE: f32 = 30.;

/// A borda da foto em foco — a âmbar do Comparar.
const BORDA: f32 = 4.;

const AMBAR: u32 = 0xfbbf24;

/// O gosto do operador sobre o tamanho do número.
const ESCALAS_DO_NUMERO: [(f32, &str); 3] = [(0.75, "pequeno"), (1.0, "médio"), (1.4, "grande")];

struct Foto {
    imagem: Arc<RenderImage>,
    escolhida: bool,
    rejeitada: bool,
}

struct Poc {
    fotos: Vec<Foto>,
    /// A posição em foco **no recorte do operador** (com as rejeitadas).
    foco: usize,
    por_linha: usize,
    qr: bool,
    escala: usize,
    em_grade: bool,
    codigo_do_qr: Arc<Vec<Vec<bool>>>,
    foco_do_teclado: FocusHandle,
}

impl Poc {
    fn nova(fotos: Vec<Foto>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let foco_do_teclado = cx.focus_handle();
        window.focus(&foco_do_teclado, cx);
        Self {
            fotos,
            foco: 0,
            por_linha: conta::POR_LINHA_PADRAO,
            qr: false,
            escala: 1,
            em_grade: true,
            codigo_do_qr: Arc::new(qr_de_mentira()),
            foco_do_teclado,
        }
    }

    fn itens(&self) -> Vec<Item> {
        let oculta: Vec<bool> = self.fotos.iter().map(|f| f.rejeitada).collect();
        conta::itens(&oculta)
    }

    /// A área das fotos e o arranjo dela, para o tamanho de janela de agora.
    fn arranjo(&self, janela: Size<Pixels>) -> Arranjo {
        conta::arranjo(
            f32::from(janela.width),
            (f32::from(janela.height) - ALTURA_DO_PE).max(0.),
            self.por_linha,
            self.qr,
            ESCALAS_DO_NUMERO[self.escala].0,
        )
    }

    /// Anda o foco entre as fotos que o cliente vê.
    fn andar(&mut self, passo: isize) {
        let itens = self.itens();
        let Some(atual) = conta::indice_do_foco(&itens, self.foco) else {
            return;
        };
        let novo = (atual as isize + passo).clamp(0, itens.len() as isize - 1) as usize;
        self.foco = itens[novo].posicao;
    }

    fn tecla(&mut self, evento: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let por_pagina = self.arranjo(window.viewport_size()).por_pagina() as isize;
        let por_linha = self.por_linha as isize;
        match evento.keystroke.key.as_str() {
            "escape" => {
                window.remove_window();
                return;
            }
            "left" => self.andar(-1),
            "right" => self.andar(1),
            "up" => self.andar(-por_linha),
            "down" => self.andar(por_linha),
            "pageup" => self.andar(-por_pagina),
            "pagedown" => self.andar(por_pagina),
            "home" => self.andar(isize::MIN / 2),
            "end" => self.andar(isize::MAX / 2),
            "+" | "=" => self.por_linha = conta::por_linha_valido(self.por_linha + 1),
            "-" => self.por_linha = conta::por_linha_valido(self.por_linha.saturating_sub(1)),
            "q" => self.qr = !self.qr,
            "g" => self.em_grade = !self.em_grade,
            "n" => self.escala = (self.escala + 1) % ESCALAS_DO_NUMERO.len(),
            "f" => window.toggle_fullscreen(),
            "b" => {
                if let Some(foto) = self.fotos.get_mut(self.foco).filter(|f| !f.rejeitada) {
                    foto.escolhida = !foto.escolhida;
                }
            }
            "x" => {
                // A em foco some; o foco passa à seguinte que ainda se vê.
                let itens = self.itens();
                if let Some(i) = conta::indice_do_foco(&itens, self.foco) {
                    let posicao = itens[i].posicao;
                    self.fotos[posicao].rejeitada = true;
                    let vizinha = itens
                        .get(i + 1)
                        .or(i.checked_sub(1).and_then(|a| itens.get(a)));
                    if let Some(vizinha) = vizinha {
                        self.foco = vizinha.posicao;
                    }
                }
            }
            "u" => self.fotos.iter_mut().for_each(|f| f.rejeitada = false),
            numero => match numero.parse::<usize>() {
                Ok(n) if (conta::MENOS_POR_LINHA..=conta::MAIS_POR_LINHA).contains(&n) => {
                    self.por_linha = n
                }
                _ => return,
            },
        }
        cx.notify();
    }

    /// O estado numa linha — é o que o roteiro imprime para conferir.
    fn estado(&self, janela: Size<Pixels>) -> String {
        let arranjo = self.arranjo(janela);
        let itens = self.itens();
        let indice = conta::indice_do_foco(&itens, self.foco);
        let pagina = indice.map_or(0, |i| conta::pagina_de(i, arranjo.por_pagina()));
        let faixa = conta::faixa_da_pagina(pagina, arranjo.por_pagina(), itens.len());
        let numeros: Vec<usize> = itens[faixa].iter().map(|i| i.numero).collect();
        format!(
            "janela={}x{} por_linha={} linhas={} qr={} coluna_do_qr={} celula={:.0}x{:.0} letra={:.0} foco={:?} pagina={}/{} numeros={numeros:?}",
            f32::from(janela.width),
            f32::from(janela.height),
            arranjo.por_linha,
            arranjo.linhas,
            self.qr,
            arranjo.coluna_do_qr,
            arranjo.largura_da_celula,
            arranjo.altura_da_celula,
            arranjo.letra_do_numero,
            indice.map(|i| itens[i].numero),
            pagina + 1,
            conta::paginas(itens.len(), arranjo.por_pagina()),
        )
    }

    /// Uma célula: a foto inteira em cima, o número embaixo.
    fn celula(&self, item: Item, i: usize, em_foco: bool, a: &Arranjo) -> impl IntoElement {
        let foto = &self.fotos[item.posicao];
        let lugar = a.celula(i);
        // 🔑 A moldura nasce do tamanho da foto (`cabe_em`): o `Img` do GPUI
        // escreve a própria proporção, e `size_full` numa caixa de outra
        // proporção corta a foto — a armadilha da tela do cliente de 17/set.
        let caixa = size(
            px((lugar.w - 2. * BORDA).max(1.)),
            px((a.altura_da_foto - 2. * BORDA).max(1.)),
        );
        let base = ui_gpui::imagem::cabe_em(caixa, foto.imagem.size(0));
        let selo = (a.letra_do_numero * 0.5).clamp(11., 22.);

        div()
            .absolute()
            .left(px(lugar.x))
            .top(px(lugar.y))
            .w(px(lugar.w))
            .h(px(lugar.h))
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .h(px(a.altura_da_foto))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        // 🔑 A borda e o selo são **da foto**, e não da caixa:
                        // em volta da caixa, a foto em pé ganhava duas tarjas
                        // pretas dentro da moldura do foco.
                        div()
                            .relative()
                            .w(base.width + px(2. * BORDA))
                            .h(base.height + px(2. * BORDA))
                            .rounded(px(6.))
                            .border(px(BORDA))
                            .border_color(if em_foco {
                                gpui_kit::rgb(AMBAR).into()
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .child(img(foto.imagem.clone()).size_full())
                            .when(foto.escolhida, |quadro| {
                                quadro.child(
                                    div()
                                        .absolute()
                                        .top(px(6.))
                                        .right(px(6.))
                                        .px(px(selo * 0.6))
                                        .rounded(px(999.))
                                        .bg(gpui_kit::rgb(AMBAR))
                                        .text_size(px(selo))
                                        .text_color(gpui_kit::black())
                                        .child("Escolhida"),
                                )
                            }),
                    ),
            )
            // 🔑 A linha do número é reservada: existe do mesmo tamanho em
            // toda célula, e a foto nunca desce para cima dela.
            .child(
                div()
                    .w_full()
                    .h(px(a.altura_do_numero))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(a.letra_do_numero))
                    .line_height(px(a.altura_do_numero))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(if em_foco {
                        gpui_kit::rgb(AMBAR)
                    } else {
                        gpui_kit::rgb(0xf5f5f5)
                    })
                    .child(SharedString::from(item.numero.to_string())),
            )
    }

    /// O cartão do QR, do tamanho do da tela do cliente de hoje.
    fn cartao_do_qr(&self) -> impl IntoElement {
        div()
            .w(px(256.))
            .p(px(16.))
            .rounded(px(12.))
            .bg(gpui_kit::rgb(0x161616))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .child(div().p(px(8.)).bg(gpui_kit::white()).rounded(px(8.)).child(
                ui_gpui::sessoes::qr_do_bot::desenho_do_qr(self.codigo_do_qr.clone(), 208.),
            ))
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_sm()
                    .text_color(gpui_kit::rgb(0xf5f5f5))
                    .child("Aponte a câmera do celular para receber suas fotos"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(gpui_kit::rgb(0x9a9a9a))
                    .child("RF-POC234"),
            )
    }

    fn grade(&self, janela: Size<Pixels>) -> gpui_kit::AnyElement {
        let a = self.arranjo(janela);
        let itens = self.itens();
        let indice = conta::indice_do_foco(&itens, self.foco);
        let por_pagina = a.por_pagina();
        let pagina = indice.map_or(0, |i| conta::pagina_de(i, por_pagina));
        let faixa = conta::faixa_da_pagina(pagina, por_pagina, itens.len());
        let pe = match (
            itens.get(faixa.start),
            itens.get(faixa.end.saturating_sub(1)),
        ) {
            (Some(de), Some(ate)) if !faixa.is_empty() => format!(
                "Fotos {} a {}  ·  página {} de {}",
                de.numero,
                ate.numero,
                pagina + 1,
                conta::paginas(itens.len(), por_pagina)
            ),
            _ => "Nenhuma foto para mostrar".to_string(),
        };
        let celulas: Vec<_> = faixa
            .clone()
            .enumerate()
            .map(|(i, n)| {
                self.celula(itens[n], i, Some(n) == indice, &a)
                    .into_any_element()
            })
            .collect();

        div()
            .size_full()
            .flex()
            .child(
                div()
                    .w(px(a.largura_das_fotos))
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        // A página que entra aparece suave; o id com a página
                        // é o que reinicia a animação a cada troca.
                        div()
                            .relative()
                            .flex_1()
                            .w_full()
                            .children(celulas)
                            .with_animation(
                                ("pagina-da-grade", pagina),
                                Animation::new(Duration::from_millis(350)),
                                |pagina, delta| pagina.opacity(delta),
                            ),
                    )
                    .child(
                        div()
                            .h(px(ALTURA_DO_PE))
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .text_color(gpui_kit::rgb(0x8a8a8a))
                            .child(SharedString::from(pe)),
                    ),
            )
            // 🚨 O QR é **irmão** da área das fotos, e não uma camada por cima:
            // a conta das células já descontou esta coluna.
            .when(a.coluna_do_qr > 0., |tela| {
                tela.child(
                    div()
                        .w(px(a.coluna_do_qr))
                        .h_full()
                        .flex()
                        .items_end()
                        .justify_center()
                        .pb(px(24.))
                        .child(self.cartao_do_qr()),
                )
            })
            .into_any_element()
    }

    /// A tela de hoje, para comparar: uma foto, inteira, e o QR no canto.
    fn foto_unica(&self, janela: Size<Pixels>) -> gpui_kit::AnyElement {
        let itens = self.itens();
        let Some(indice) = conta::indice_do_foco(&itens, self.foco) else {
            return div().size_full().into_any_element();
        };
        let item = itens[indice];
        let foto = &self.fotos[item.posicao];
        let base = ui_gpui::imagem::cabe_em(janela, foto.imagem.size(0));
        div()
            .relative()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(base.width)
                    .h(base.height)
                    .child(img(foto.imagem.clone()).size_full()),
            )
            .child(
                div()
                    .absolute()
                    .left(px(20.))
                    .bottom(px(20.))
                    .px(px(14.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(gpui_kit::rgba(0x000000b4))
                    .text_size(px(28.))
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .text_color(gpui_kit::white())
                    .child(SharedString::from(format!(
                        "{}  /  {}",
                        item.numero,
                        self.fotos.len()
                    ))),
            )
            .when(self.qr, |tela| {
                tela.child(
                    div()
                        .absolute()
                        .right(px(24.))
                        .bottom(px(24.))
                        .child(self.cartao_do_qr()),
                )
            })
            .into_any_element()
    }
}

impl Render for Poc {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let janela = window.viewport_size();
        div()
            .track_focus(&self.foco_do_teclado)
            .on_key_down(
                cx.listener(|poc, evento: &KeyDownEvent, window, cx| poc.tecla(evento, window, cx)),
            )
            .size_full()
            .bg(gpui_kit::black())
            .child(if self.em_grade {
                self.grade(janela)
            } else {
                self.foto_unica(janela)
            })
    }
}

/// Um QR de mentira — só para o cartão ter o tamanho e o peso do de verdade.
fn qr_de_mentira() -> Vec<Vec<bool>> {
    const N: usize = 33;
    let mut semente: u32 = 0x1234_5678;
    let mut qr = vec![vec![false; N]; N];
    for linha in qr.iter_mut() {
        for modulo in linha.iter_mut() {
            semente = semente.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *modulo = (semente >> 16) & 1 == 0;
        }
    }
    // Os três olhos, que é o que faz parecer um QR.
    for (oy, ox) in [(0, 0), (0, N - 7), (N - 7, 0)] {
        for y in 0..7 {
            for x in 0..7 {
                let borda = y == 0 || y == 6 || x == 0 || x == 6;
                let miolo = (2..=4).contains(&y) && (2..=4).contains(&x);
                qr[oy + y][ox + x] = borda || miolo;
            }
        }
    }
    qr
}

/// As fotos de uma pasta (JPEG e PNG), em ordem de nome, reduzidas.
fn fotos_da_pasta(pasta: &Path) -> Vec<image::DynamicImage> {
    let mut arquivos: Vec<PathBuf> = std::fs::read_dir(pasta)
        .unwrap_or_else(|e| {
            eprintln!("a pasta {} não abriu: {e}", pasta.display());
            std::process::exit(1);
        })
        .filter_map(|entrada| entrada.ok().map(|e| e.path()))
        .filter(|caminho| {
            caminho
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| matches!(e.to_lowercase().as_str(), "jpg" | "jpeg" | "png"))
                .unwrap_or(false)
        })
        .collect();
    arquivos.sort();
    if arquivos.len() > TETO_DE_FOTOS {
        eprintln!(
            "🧪 a pasta tem {} fotos; a POC usa as {TETO_DE_FOTOS} primeiras",
            arquivos.len()
        );
        arquivos.truncate(TETO_DE_FOTOS);
    }
    // Decodificar é o que demora: uma fatia por núcleo.
    let nucleos = std::thread::available_parallelism().map_or(4, |n| n.get());
    let fatia = arquivos.len().div_ceil(nucleos).max(1);
    std::thread::scope(|escopo| {
        let tarefas: Vec<_> = arquivos
            .chunks(fatia)
            .map(|parte| {
                escopo.spawn(move || {
                    parte
                        .iter()
                        .filter_map(|caminho| match image::open(caminho) {
                            Ok(imagem) => Some(imagem.thumbnail(LADO_DA_FOTO, LADO_DA_FOTO)),
                            Err(e) => {
                                eprintln!("⚠️  {} não abriu: {e}", caminho.display());
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        tarefas
            .into_iter()
            .flat_map(|t| t.join().unwrap_or_default())
            .collect()
    })
}

/// Fotos de mentira: em pé e deitadas misturadas, cada uma de uma cor, com
/// uma moldura clara — se a foto for cortada, a moldura some de um lado.
fn fotos_de_mentira() -> Vec<image::DynamicImage> {
    (0..37u32)
        .map(|i| {
            let (largura, altura) = match i % 5 {
                0 | 3 => (600, 900),
                4 => (800, 800),
                _ => (900, 600),
            };
            let (r, g, b) = cor_da_roda(i as f32 * 47.0);
            let imagem = image::RgbImage::from_fn(largura, altura, |x, y| {
                let moldura = x < 12 || y < 12 || x >= largura - 12 || y >= altura - 12;
                if moldura {
                    return image::Rgb([235, 235, 235]);
                }
                let luz = 0.45 + 0.55 * (y as f32 / altura as f32);
                let cx = x as f32 - largura as f32 / 2.;
                let cy = y as f32 - altura as f32 / 2.;
                let disco = (cx * cx + cy * cy).sqrt() < largura.min(altura) as f32 * 0.28;
                let k = if disco { 1.25 } else { luz };
                image::Rgb([
                    (r * k).min(255.) as u8,
                    (g * k).min(255.) as u8,
                    (b * k).min(255.) as u8,
                ])
            });
            image::DynamicImage::ImageRgb8(imagem)
        })
        .collect()
}

fn cor_da_roda(graus: f32) -> (f32, f32, f32) {
    let h = (graus % 360.) / 60.;
    let x = 1. - (h % 2. - 1.).abs();
    let (r, g, b) = match h as u32 {
        0 => (1., x, 0.),
        1 => (x, 1., 0.),
        2 => (0., 1., x),
        3 => (0., x, 1.),
        4 => (x, 0., 1.),
        _ => (1., 0., x),
    };
    (60. + r * 150., 60. + g * 150., 60. + b * 150.)
}

fn main() {
    let pasta = std::env::args().nth(1).map(PathBuf::from);
    let cruas = match &pasta {
        Some(pasta) => fotos_da_pasta(pasta),
        None => fotos_de_mentira(),
    };
    if cruas.is_empty() {
        eprintln!("nenhuma foto JPEG ou PNG na pasta");
        std::process::exit(1);
    }
    eprintln!(
        "🧪 POC da grade do cliente: {} fotos. 2–8 ou +/- fotos por linha · setas o foco · Q o QR · B escolhida · X rejeita · U desfaz · N número · G foto única · F tela cheia · Esc fecha",
        cruas.len()
    );
    let roteiro = std::env::var_os("VLB_ROTEIRO").map(|caminho| {
        std::fs::read_to_string(&caminho).unwrap_or_else(|e| {
            eprintln!("o roteiro {} não abriu: {e}", Path::new(&caminho).display());
            std::process::exit(1);
        })
    });
    let pasta_das_fotos = std::env::var_os("VLB_FOTOS").map(PathBuf::from);

    gpui_kit::application()
        .with_assets(ui_gpui::recursos::Recursos)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            ui_gpui::imagem::coleta::ligar(cx);
            let fotos: Vec<Foto> = cruas
                .into_iter()
                .map(|imagem| Foto {
                    imagem: ui_gpui::imagem::para_gpui(imagem),
                    escolhida: false,
                    rejeitada: false,
                })
                .collect();

            // O tamanho da prévia da tela do cliente (e do pop-up do site).
            let limites = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            let mut poc = None;
            let janela = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(limites)),
                        titlebar: Some(gpui_kit::TitlebarOptions {
                            title: Some("POC — Tela do cliente em grade".into()),
                            ..Default::default()
                        }),
                        is_resizable: true,
                        window_decorations: ui_gpui::janela::decoracoes_ao_abrir(),
                        ..Default::default()
                    },
                    |window, cx| {
                        let entidade = cx.new(|cx| Poc::nova(fotos, window, cx));
                        poc = Some(entidade.clone());
                        entidade
                    },
                )
                .expect("abrir a janela da POC");
            let poc = poc.expect("a POC");
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            if let Some(texto) = roteiro {
                cx.spawn(async move |cx| {
                    let executor = cx.background_executor().clone();
                    let esperar = move |ms: u64| executor.timer(Duration::from_millis(ms));
                    esperar(600).await;
                    for linha in texto.lines() {
                        let linha = linha.split('#').next().unwrap_or_default().trim();
                        let mut partes = linha.split_whitespace();
                        let (Some(passo), resto) = (partes.next(), partes.collect::<Vec<_>>())
                        else {
                            continue;
                        };
                        match (passo, resto.as_slice()) {
                            ("esperar", [ms]) => esperar(ms.parse().unwrap_or(0)).await,
                            ("fim", _) => {
                                cx.update(|cx| cx.quit());
                                return;
                            }
                            ("tecla", [tecla]) => {
                                let tecla = gpui_kit::Keystroke::parse(tecla)
                                    .unwrap_or_else(|e| panic!("[roteiro] tecla {tecla}: {e}"));
                                // 🔑 Pela alça sem tipo: a tipada empresta a
                                // entidade da POC, e a tecla precisa dela.
                                let qualquer: gpui_kit::AnyWindowHandle = janela.into();
                                let _ = qualquer.update(cx, |_, window, cx| {
                                    window.dispatch_keystroke(tecla, cx)
                                });
                            }
                            ("tamanho", [largura, altura]) => {
                                let (l, a): (f32, f32) = (
                                    largura.parse().expect("[roteiro] largura"),
                                    altura.parse().expect("[roteiro] altura"),
                                );
                                let _ = janela
                                    .update(cx, |_, window, _| window.resize(size(px(l), px(a))));
                            }
                            ("foto", [nome]) => {
                                let destino = pasta_das_fotos
                                    .clone()
                                    .unwrap_or_else(|| PathBuf::from("."))
                                    .join(format!("{nome}.png"));
                                let _ = janela.update(cx, |poc, window, _| {
                                    eprintln!(
                                        "[estado] {nome}: {}",
                                        poc.estado(window.viewport_size())
                                    );
                                    match ui_gpui::depuracao::fotografar(window, &destino) {
                                        Ok(()) => eprintln!("[foto] {}", destino.display()),
                                        Err(e) => eprintln!("[foto] {}: {e}", destino.display()),
                                    }
                                });
                            }
                            _ => panic!("[roteiro] passo desconhecido: {linha}"),
                        }
                        esperar(150).await;
                    }
                })
                .detach();
            }
            drop(poc);
            cx.activate(true);
        });
}
