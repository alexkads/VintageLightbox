//! Semeia um catálogo de medição, com fotos e miniaturas de verdade.
//!
//! # Por que isto existe
//!
//! O critério de saída da fase 1 é **navegar 2.000 fotos a 60fps**
//! (docs/10-MIGRACAO-GPUI.md §5). Sem um acervo desse tamanho não há o que
//! medir — e o catálogo real desta máquina está vazio desde que foi recriado
//! limpo, na decisão das migrations 16–19.
//!
//! Esperar o dono importar 2.000 fotos para só então descobrir que a grade
//! engasga é justamente a ordem errada: a fase 1 existe para essa resposta vir
//! **antes** de cinco meses de reescrita.
//!
//! ```bash
//! cargo run -p ui-gpui --bin semear-catalogo -- 2000
//! VLB_CATALOG=/tmp/catalogo-de-medicao cargo run -p ui-gpui
//! ```
//!
//! ⚠️ **Ele recusa escrever num catálogo que já tem fotos.** O caminho vem de
//! `VLB_CATALOG` como qualquer outro, e um erro de digitação apontaria para a
//! biblioteca real — encher o acervo de alguém com 2.000 quadrados coloridos
//! não é o tipo de engano que se desfaz com um comando.

use std::path::PathBuf;

use image::{DynamicImage, Rgba, RgbaImage};
use infrastructure::cache::preview_manager::PreviewManager;
use infrastructure::paths::AppPaths;

/// Uma foto sintética: quadrado de cor sólida com uma faixa clara no topo.
///
/// A faixa existe para a conferência visual ser possível: numa grade de cores
/// chapadas não dá para ver se as miniaturas estão sendo trocadas de lugar
/// durante a rolagem, e é exatamente esse o defeito que a virtualização
/// introduz.
fn foto_sintetica(indice: usize) -> DynamicImage {
    let (largura, altura) = (320u32, 240u32);
    let mut img = RgbaImage::new(largura, altura);

    let matiz = (indice * 37 % 360) as f32;
    let (r, g, b) = matiz_para_rgb(matiz);

    for y in 0..altura {
        for x in 0..largura {
            // Faixa clara nos 40px do topo, com um degrau a cada 32px: dá para
            // contar as fotos passando durante a rolagem.
            let pixel = if y < 40 && (x / 32) % 2 == 0 {
                Rgba([240, 240, 240, 255])
            } else {
                Rgba([r, g, b, 255])
            };
            img.put_pixel(x, y, pixel);
        }
    }

    DynamicImage::ImageRgba8(img)
}

/// HSV com saturação e valor cheios — só para gerar cores distinguíveis.
fn matiz_para_rgb(matiz: f32) -> (u8, u8, u8) {
    let setor = (matiz / 60.0).floor() as i32 % 6;
    let f = (matiz / 60.0) - (matiz / 60.0).floor();
    let q = (255.0 * (1.0 - f)) as u8;
    let t = (255.0 * f) as u8;

    match setor {
        0 => (255, t, 0),
        1 => (q, 255, 0),
        2 => (0, 255, t),
        3 => (0, q, 255),
        4 => (t, 0, 255),
        _ => (255, 0, q),
    }
}

#[tokio::main]
async fn main() {
    let quantas: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(2000);

    let catalogo: PathBuf = AppPaths::catalog_root();
    println!("catálogo: {}", catalogo.display());

    if !catalogo.exists() {
        std::fs::create_dir_all(&catalogo).expect("criar o catálogo");
    }

    let db_path = AppPaths::main_db_path();
    let url = format!("sqlite:{}?mode=rwc", db_path.to_string_lossy());
    let pool = infrastructure::create_pool(&url)
        .await
        .expect("abrir o banco");
    infrastructure::run_migrations(&pool)
        .await
        .expect("rodar as migrations");

    let ja_tem: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM photos")
        .fetch_one(&pool)
        .await
        .expect("contar as fotos");

    // A trava que impede o erro de digitação de virar estrago: o `VLB_CATALOG`
    // é texto livre, e o padrão dele é a biblioteca real.
    if ja_tem > 0 {
        eprintln!(
            "❌ este catálogo já tem {ja_tem} fotos — semear aqui misturaria dados sintéticos \
             com os de verdade.\n   Aponte VLB_CATALOG para um diretório vazio."
        );
        std::process::exit(1);
    }

    let previews = PreviewManager::new();
    let agora = chrono::Utc::now().to_rfc3339();

    for i in 0..quantas {
        // UUID, e não `medicao-00001`: `PhotoId::from_string` recusa qualquer
        // outra coisa, e o `find_all` do repositório derruba a **leitura
        // inteira** num id inválido — não a linha. Foi o que este semeador
        // descobriu na primeira tentativa, e a recusa está certa: id que não
        // casa com o formato é erro, e não vizinho mais próximo.
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO photos (id, file_path, rating, is_edited, imported_at, modified_at, metadata)
             VALUES (?1, ?2, ?3, 0, ?4, ?4, ?5)",
        )
        .bind(&id)
        .bind(format!("/medicao/DSC_{i:05}.NEF"))
        // Notas de 0 a 5 espalhadas, para os filtros da fase 1d terem o que
        // filtrar quando chegarem.
        .bind((i % 6) as i64)
        .bind(&agora)
        .bind(r#"{"camera_model":"Medição","width":320,"height":240}"#)
        .execute(&pool)
        .await
        .expect("inserir a foto");

        previews
            .save_thumbnail(&id, &foto_sintetica(i))
            .expect("gravar a miniatura");

        if (i + 1) % 250 == 0 {
            println!("  {} de {quantas}", i + 1);
        }
    }

    println!("✅ {quantas} fotos e {quantas} miniaturas gravadas.");
    println!("   VLB_CATALOG={} cargo run -p ui-gpui", catalogo.display());
}
