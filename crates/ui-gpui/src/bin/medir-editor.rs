//! Quanto custa o editor em camadas numa foto grande, por parte
//! (`docs/editor-em-camadas/04-PLANO-ETAPA-1.md`, "Medidas").
//!
//! ```bash
//! cargo run --release -p ui-gpui --bin medir-editor              # 6000×4000 sintética
//! cargo run --release -p ui-gpui --bin medir-editor -- foto.NEF  # uma foto de verdade
//! ```
//!
//! O que sai, cada um com o orçamento ao lado:
//!
//! 1. **abrir** — a base neutra (LibRaw para RAW) e a vista reduzida;
//! 2. **pincel** — cada evento de ponteiro: o carimbo em resolução cheia, a
//!    vista refeita onde sujou e os ladrilhos convertidos para BGRA (o que a
//!    janela entrega à GPU), p50/p95/máx;
//! 3. **composição** da imagem editada inteira;
//! 4. **salvar** (tiles + PNG + manifesto) e **reabrir** o projeto;
//! 5. **a Revelação depois de salvar** — a cópia de trabalho feita da PNG e a
//!    revelação dela no motor de verdade (wgpu), com uma receita não neutra;
//! 6. o **pico de memória** do processo.
//!
//! ⚠️ **Só vale em `--release`** — ver `CLAUDE.md`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use editor_core::projeto::{DiscoReal, Projeto};
use editor_core::{BaseRef, Documento, Ferramenta, Historico, Sessao};
use image::{DynamicImage, RgbImage};

const QUADRO_MS: f64 = 16.7;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

struct Memoria {
    sistema: sysinfo::System,
    pid: sysinfo::Pid,
    pico: u64,
}

impl Memoria {
    fn nova() -> Self {
        Self {
            sistema: sysinfo::System::new(),
            pid: sysinfo::get_current_pid().expect("o próprio pid"),
            pico: 0,
        }
    }

    fn medir(&mut self) -> u64 {
        self.sistema
            .refresh_processes(sysinfo::ProcessesToUpdate::Some(&[self.pid]), true);
        let agora = self
            .sistema
            .process(self.pid)
            .map(|p| p.memory())
            .unwrap_or(0);
        self.pico = self.pico.max(agora);
        agora
    }
}

fn percentil(amostras: &mut [f64], p: f64) -> f64 {
    amostras.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let i = ((amostras.len() as f64 - 1.0) * p).round() as usize;
    amostras[i]
}

fn linha(nome: &str, valor_ms: f64, orcamento: Option<f64>) {
    let marca = match orcamento {
        Some(o) if valor_ms <= o => format!("  ✅ (≤ {o} ms)"),
        Some(o) => format!("  ⚠️  (> {o} ms)"),
        None => String::new(),
    };
    println!("  {nome:<46} {valor_ms:>9.2} ms{marca}");
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("⚠️  perfil `debug`: os números não valem. Use --release.\n");
    }
    let mut memoria = Memoria::nova();
    let inicial = memoria.medir();

    // 1. A base.
    let t = Instant::now();
    let base = match std::env::args().nth(1) {
        Some(caminho) => infrastructure::base_neutra::base_neutra(std::path::Path::new(&caminho))
            .expect("a foto abriu")
            .to_rgb8(),
        None => RgbImage::from_fn(6000, 4000, |x, y| {
            image::Rgb([(x / 24) as u8, (y / 16) as u8, ((x + y) / 40) as u8])
        }),
    };
    let decodificar = t.elapsed();
    let base = Arc::new(base);
    println!(
        "foto: {}×{} ({:.1} MP), {} — sistema: {} {}\n",
        base.width(),
        base.height(),
        base.width() as f64 * base.height() as f64 / 1e6,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::env::consts::OS,
        std::env::consts::ARCH,
    );

    let t = Instant::now();
    let doc = Documento::novo(BaseRef::da_imagem(&base));
    let impressao = t.elapsed();
    let t = Instant::now();
    let mut sessao = Sessao::nova(base.clone(), doc, Historico::novo(), 2048);
    let vista = t.elapsed();
    sessao.vista_mut().levar_os_sujos();
    println!("abrir");
    linha("decodificar a base", ms(decodificar), None);
    linha("impressão digital da base (SHA-256)", ms(impressao), None);
    linha("vista reduzida inicial (2048 px)", ms(vista), None);
    memoria.medir();

    // 2. O pincel: um traço em zigue-zague, evento a evento, como o ponteiro.
    println!("\npincel (por evento de ponteiro: carimbo + vista + ladrilhos BGRA)");
    for (raio, rotulo) in [(40.0, "raio 40 px"), (200.0, "raio 200 px")] {
        sessao.pincel.raio = raio;
        sessao.pincel.dureza = 0.6;
        sessao.pincel.ferramenta = Ferramenta::Pincel;
        let mut amostras = Vec::new();
        let (l, a) = (base.width() as f32, base.height() as f32);
        sessao.apertar(l * 0.1, a * 0.2);
        let eventos = 600;
        for k in 0..eventos {
            let f = k as f32 / eventos as f32;
            // ~6 px da foto por evento: um arrasto rápido a 120 Hz.
            let x = l * (0.1 + 0.8 * f);
            let y = a * (0.2 + 0.6 * (f * 12.0).sin().abs());
            let t = Instant::now();
            sessao.arrastar(x, y);
            let sujos = sessao.vista_mut().levar_os_sujos();
            for ladrilho in sujos {
                let _ = sessao.vista().ladrilho_bgra(ladrilho);
            }
            amostras.push(ms(t.elapsed()));
        }
        let t = Instant::now();
        sessao.soltar();
        let soltar = t.elapsed();
        let p50 = percentil(&mut amostras, 0.5);
        let p95 = percentil(&mut amostras, 0.95);
        let maximo = percentil(&mut amostras, 1.0);
        linha(&format!("{rotulo}: p50"), p50, Some(8.0));
        linha(&format!("{rotulo}: p95"), p95, Some(QUADRO_MS));
        linha(&format!("{rotulo}: máx"), maximo, None);
        linha(
            &format!("{rotulo}: soltar (passo do desfazer)"),
            ms(soltar),
            None,
        );
    }
    let t = Instant::now();
    sessao.desfazer();
    linha(
        "desfazer o traço grande",
        ms(t.elapsed()),
        Some(QUADRO_MS * 3.0),
    );
    sessao.refazer();
    memoria.medir();

    // 3. Composição inteira.
    println!("\ncomposição");
    let t = Instant::now();
    let composta = sessao.compor();
    linha("imagem editada inteira", ms(t.elapsed()), None);
    drop(composta);

    // 4. Salvar e reabrir.
    println!("\nsalvar e reabrir");
    let pasta = tempfile::tempdir().expect("pasta temporária");
    let projeto = Projeto::novo(pasta.path().join("e"), Arc::new(DiscoReal));
    let (doc, hist) = sessao.instantaneo();
    let t = Instant::now();
    let salvo = projeto
        .salvar("medida", &base, &doc, &hist, 1)
        .expect("salvar");
    let salvar = t.elapsed();
    linha("salvar (tiles + PNG + manifesto)", ms(salvar), None);
    let versao = salvo.versao.expect("com traço há versão");
    let tamanho = std::fs::metadata(&versao.arquivo)
        .map(|m| m.len())
        .unwrap_or(0);
    println!(
        "  {:<46} {:>9.1} MB ({} tiles)",
        "PNG da imagem editada",
        tamanho as f64 / 1e6,
        doc.camadas[0].pixels.quantos()
    );
    // Um segundo salvamento com um retoque pequeno: só os tiles novos.
    sessao.pincel.raio = 20.0;
    sessao.apertar(100.0, 100.0);
    sessao.soltar();
    let (doc2, hist2) = sessao.instantaneo();
    let t = Instant::now();
    projeto
        .salvar("medida", &base, &doc2, &hist2, 2)
        .expect("salvar");
    linha(
        "salvar de novo depois de um retoque pequeno",
        ms(t.elapsed()),
        None,
    );
    let t = Instant::now();
    let reaberto = Projeto::novo(pasta.path().join("e"), Arc::new(DiscoReal))
        .abrir(&base)
        .expect("reabrir")
        .expect("há projeto");
    linha(
        "reabrir (manifesto + tiles + histórico)",
        ms(t.elapsed()),
        None,
    );
    assert_eq!(reaberto.documento, doc2, "o projeto voltou intacto");
    memoria.medir();

    // 5. A Revelação depois de salvar: cópia de trabalho e motor.
    println!("\na Revelação depois de salvar");
    let previews = infrastructure::cache::preview_manager::PreviewManager::new_with_path(
        pasta.path().join("cache"),
    );
    let t = Instant::now();
    let copia = ui_gpui::revelacao::fonte::copia_da_versao(&previews, &versao, "medida")
        .expect("a cópia de trabalho");
    let fazer_a_copia = t.elapsed();
    linha(
        "cópia de trabalho da PNG (decodificar + 2560 px)",
        ms(fazer_a_copia),
        None,
    );
    let rgba = DynamicImage::ImageRgb8(copia.to_rgb8()).to_rgba8();
    let (l, a) = (rgba.width(), rgba.height());
    let pixels = Arc::new(rgba.into_raw());
    match infrastructure::gpu_adjustments::Motor::abrir() {
        Some(mut motor) => {
            let ajustes = infrastructure::gpu_adjustments::Ajustes {
                exposure: 0.4,
                contrast: 20.0,
                saturation: 15.0,
                ..Default::default()
            };
            let _ = motor.revelar(&pixels, l, a, &ajustes); // aquece o motor
            let pixels_novos = Arc::new((*pixels).clone()); // textura nova, como na troca
            let t = Instant::now();
            let revelada = motor.revelar(&pixels_novos, l, a, &ajustes);
            let revelar = t.elapsed();
            assert!(revelada.is_some());
            linha("revelar a cópia nova no motor (GPU)", ms(revelar), None);
            linha(
                "salvar → Revelação atualizada (cópia + motor)",
                ms(fazer_a_copia + revelar),
                Some(500.0),
            );
        }
        None => println!("  (sem adaptador de GPU: a revelação não foi medida)"),
    }

    let final_ = memoria.medir();
    println!("\nmemória do processo");
    println!("  {:<46} {:>9.0} MB", "no início", inicial as f64 / 1e6);
    println!("  {:<46} {:>9.0} MB", "no fim", final_ as f64 / 1e6);
    println!(
        "  {:<46} {:>9.0} MB  (alvo < 700 MB)",
        "pico medido",
        memoria.pico as f64 / 1e6
    );
}
