//! Quanto custa desenhar **um quadro** da grade da sessão.
//!
//! # Por que isto existe
//!
//! `Detalhe::celula` e `Detalhe::tira` chamavam `get_preview` + `para_gpui`
//! **por foto, dentro do render** — sem nada guardado entre um quadro e o
//! seguinte, ao contrário da grade da Biblioteca, que tem o `CacheDeMiniaturas`
//! exatamente para isso.
//!
//! Duas coisas faziam esse caminho custar o que custava:
//!
//! 1. as fotos da sessão (`site:…`) só tinham o preview **grande** gravado
//!    (`Recado::Miniatura` chamava `save_preview`, nunca `save_thumbnail`), então
//!    uma célula de 160px carregava uma imagem de 640px;
//! 2. o L1 do `PreviewManager` guarda **15** imagens — ele foi feito para a
//!    Revelação, que vai e volta entre fotos vizinhas (`docs/08-CACHE-ARCHITECTURE.md`).
//!    Uma grade que varre 25 numa sequência acerta **zero**: cada quadro
//!    redecodifica todas.
//!
//! ```bash
//! CAT="$HOME/Pictures/VintageLightbox/VintageLightbox Catalog"
//! sqlite3 "$CAT/Previews.lrdata/preview_cache.db" \
//!   "select photo_id from previews where photo_id like 'site:%' and type=1" \
//!   | cargo run --release -p ui-gpui --bin medir-grade-da-sessao
//! ```

use std::num::NonZeroUsize;
use std::time::Instant;

use infrastructure::cache::preview_manager::PreviewManager;
use infrastructure::paths::AppPaths;
use ui_gpui::biblioteca::miniaturas::CacheDeMiniaturas;
use ui_gpui::imagem::para_gpui;

/// O orçamento de um quadro a 60fps.
const ORCAMENTO_MS: f64 = 16.7;

/// Quantos quadros medir de cada caminho.
const QUADROS: usize = 3;

/// O lado da miniatura da sessão — o mesmo `LADO_DA_MINIATURA` de `detalhe.rs`.
const LADO_DA_MINIATURA: u32 = 320;

/// Reduz sem ampliar, como o `reduzir` de `detalhe.rs`.
fn reduzir(imagem: &image::DynamicImage, lado: u32) -> image::DynamicImage {
    use image::GenericImageView;
    let (largura, altura) = imagem.dimensions();
    if largura <= lado && altura <= lado {
        return imagem.clone();
    }
    imagem.thumbnail(lado, lado)
}

fn main() {
    let quantas: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(25);

    let previews = PreviewManager::new_with_path(AppPaths::preview_cache_dir());

    let chaves: Vec<String> = std::io::stdin()
        .lines()
        .map_while(Result::ok)
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .take(quantas)
        .collect();

    if chaves.is_empty() {
        eprintln!("❌ nenhuma chave na entrada padrão.");
        return;
    }

    println!(
        "\nperfil: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    println!("células: {}\n", chaves.len());

    // ── Antes: o que `celula` fazia, por foto, dentro do render ──────────────
    println!("antes — get_preview + para_gpui dentro do render:");
    let antes = medir(QUADROS, |_| {
        for chave in &chaves {
            if let Some(imagem) = previews
                .get_preview(chave)
                .or_else(|| previews.get_thumbnail(chave))
            {
                std::hint::black_box(para_gpui(imagem));
            }
        }
    });

    // ── Depois: carrega uma vez, o quadro só lê ──────────────────────────────
    println!("\ndepois — CacheDeMiniaturas, carregado uma vez por quadro:");
    let mut cache =
        CacheDeMiniaturas::nova(NonZeroUsize::new(chaves.len().max(64)).expect("não é zero"));
    let depois = medir(QUADROS, |_| {
        // O mesmo que `Detalhe::preparar_miniaturas` faz: gera a miniatura que
        // falta **uma vez** (e a grava), depois só lê.
        for chave in &chaves {
            if cache.espiar(chave).is_some() {
                continue;
            }
            if previews.get_thumbnail(chave).is_none() {
                if let Some(grande) = previews.get_preview(chave) {
                    let _ = previews.save_thumbnail(chave, &reduzir(&grande, LADO_DA_MINIATURA));
                }
            }
            cache.obter(&previews, chave);
        }
        // E o que o quadro faz de verdade: uma leitura por célula, mais a tira.
        for chave in &chaves {
            std::hint::black_box(cache.espiar(chave));
            std::hint::black_box(cache.espiar(chave));
        }
    });

    let prontas = chaves
        .iter()
        .filter(|c| {
            matches!(
                cache.espiar(c),
                Some(ui_gpui::biblioteca::miniaturas::Miniatura::Pronta(_))
            )
        })
        .count();
    println!("  miniaturas prontas: {prontas} de {}", chaves.len());
    assert!(prontas > 0, "medida inválida: nenhuma miniatura carregou");

    let (a, d) = (menor(&antes), menor(&depois));
    println!(
        "\n  melhor quadro antes:  {a:8.1} ms  ({:.0}× o orçamento)",
        a / ORCAMENTO_MS
    );
    println!(
        "  melhor quadro depois: {d:8.1} ms  ({:.0}% do orçamento)",
        d / ORCAMENTO_MS * 100.0
    );
    // Sem razão entre os dois: o "depois" não é um quadro mais rápido, é um
    // quadro que **não faz o trabalho** — ele só lê o que já está em memória.
    // Dividir por ~0 daria um número grande e sem significado.
    println!("\n  ✅ o custo por quadro sai do caminho: {a:.0} ms viram uma leitura de memória.");
    println!(
        "  ⚠️  O primeiro quadro custa {:.0} ms — é a geração das miniaturas que\n      faltavam, uma vez por foto, e ela fica gravada no cache de previews.",
        depois.first().copied().unwrap_or(0.0)
    );
    println!("  🔑 E a tira de baixo pagava o mesmo do 'antes' de novo, por quadro.");
}

fn medir(quadros: usize, mut quadro: impl FnMut(usize)) -> Vec<f64> {
    let mut medidas = Vec::with_capacity(quadros);
    for n in 0..quadros {
        let comeco = Instant::now();
        quadro(n);
        let ms = comeco.elapsed().as_secs_f64() * 1000.0;
        println!("  quadro {}: {ms:9.2} ms", n + 1);
        medidas.push(ms);
    }
    medidas
}

fn menor(medidas: &[f64]) -> f64 {
    medidas.iter().cloned().fold(f64::INFINITY, f64::min)
}
