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
    let mut exposicoes_gravadas = Vec::new();

    for i in 0..quantas {
        // UUID, e não `medicao-00001`: `PhotoId::from_string` recusa qualquer
        // outra coisa, e o `find_all` do repositório derruba a **leitura
        // inteira** num id inválido — não a linha. Foi o que este semeador
        // descobriu na primeira tentativa, e a recusa está certa: id que não
        // casa com o formato é erro, e não vizinho mais próximo.
        let id = uuid::Uuid::new_v4().to_string();

        // Uma em cada cinco nasce **já revelada**, e é a única forma de conferir
        // que a Revelação carrega o que está no banco: num acervo em que nenhuma
        // foto tem ajuste gravado, "abre no neutro" e "não lê o banco" são a
        // mesma tela. Só campos que chegam ao shader — exposição, contraste e
        // saturação; semear HSL/matiz mostraria um número no painel e nada na
        // foto (docs/10-MIGRACAO-GPUI.md, §"a tela também não aplica 46").
        let revelada = i % 5 == 0;
        let (exposicao, contraste, saturacao) = if revelada {
            let passo = (i / 5 % 7) as f32;
            (
                Some(-1.5 + passo * 0.5),
                Some(0.8 + passo * 0.1),
                Some(-0.3 + passo * 0.1),
            )
        } else {
            (None, None, None)
        };
        exposicoes_gravadas.extend(exposicao);

        sqlx::query(
            "INSERT INTO photos (id, file_path, rating, color_label, flag, is_edited, imported_at, modified_at, metadata,
                                 edit_exposure, edit_contrast, edit_saturation)
             VALUES (?1, ?2, ?3, ?6, ?7, ?8, ?4, ?4, ?5, ?9, ?10, ?11)",
        )
        .bind(&id)
        // Espalhadas por pastas, senao a arvore lateral nasce com um item so
        // e nao ha o que conferir nela.
        .bind(format!("/medicao/{}/DSC_{i:05}.NEF", 2020 + (i % 6)))
        // Notas de 0 a 5 espalhadas, para os filtros da fase 1d terem o que
        // filtrar quando chegarem.
        .bind((i % 6) as i64)
        .bind(&agora)
        .bind(r#"{"camera_model":"Medição","width":320,"height":240}"#)
        // Uma em cada seis fica sem cor, e o resto gira pelas cinco do dominio.
        .bind(match i % 6 {
            0 => None,
            n => Some(["Red", "Yellow", "Green", "Blue", "Purple"][n - 1]),
        })
        // 1 = escolhida, -1 = rejeitada, NULL = sem marca — a convencao do
        // Lightroom, que e a que esta na coluna.
        .bind(match i % 3 {
            0 => Some(1i64),
            1 => Some(-1i64),
            _ => None,
        })
        .bind(revelada as i64)
        .bind(exposicao)
        .bind(contraste)
        .bind(saturacao)
        .execute(&pool)
        .await
        .expect("inserir a foto");

        let sintetica = foto_sintetica(i);
        previews
            .save_thumbnail(&id, &sintetica)
            .expect("gravar a miniatura");
        // O preview "Large" também, e não só a miniatura: é dele que a Revelação
        // tira a imagem grande. Sem isto o catálogo sintético abriria a
        // Revelação em "não tem preview no cache" — e a queda para a miniatura
        // esconderia justamente o caminho que se quer conferir.
        previews
            .save_preview(&id, &sintetica)
            .expect("gravar o preview");

        if (i + 1) % 250 == 0 {
            println!("  {} de {quantas}", i + 1);
        }
    }

    println!("✅ {quantas} fotos, {quantas} miniaturas e {quantas} previews gravados.");

    conferir_a_volta(&pool, &exposicoes_gravadas).await;

    println!("   VLB_CATALOG={} cargo run -p ui-gpui", catalogo.display());
}

/// Relê pelo caminho do app e confere que a revelação gravada volta inteira.
///
/// 🔑 **Gravar não é ler.** São 46 colunas atravessando quatro etapas —
/// `row_to_photo`, a entidade `Photo`, o `PhotoViewModel` do controller e
/// [`ui_gpui::revelacao::persistencia::da_foto`] — e cada uma delas engole campo
/// desconhecido em silêncio: o repositório faz `.unwrap_or(None)`, e um campo que
/// se perca no meio vira "esta foto nunca foi revelada". Nenhum teste unitário
/// cobre a cadeia toda, porque nenhum deles tem banco.
///
/// Por isso a conferência mora aqui: quem semeia sabe o que gravou.
///
/// 🚨 **E foi ela que achou o quarto lugar onde mora o neutro.** A primeira
/// versão contava as fotos com `da_foto(foto) != Ajustes::default()` e encontrou
/// **todas** — não as que tinham revelação. O motivo está no schema:
/// `014_add_hsl_lens_fields.sql` cria `edit_lens_vignette_midpoint` com
/// `DEFAULT 50.0`, então **toda** foto importada nasce com esse campo preenchido.
/// Não é revelação, é o padrão da coluna.
///
/// Os dois apps leem o mesmo 50 e mostram o mesmo slider, então não há
/// divergência de paridade. O que fica é: contar "tem revelação" comparando com o
/// neutro **não funciona neste banco**, e a otimização de não pedir revelação no
/// neutro (`tela.rs`) quase nunca dispara com foto de verdade — o legado também
/// pede sempre, então o comportamento é o mesmo dele.
async fn conferir_a_volta(pool: &sqlx::SqlitePool, exposicoes_gravadas: &[f32]) {
    use std::sync::Arc;
    use ui_gpui::revelacao::persistencia;

    let repositorio = Arc::new(infrastructure::PhotoRepositoryImpl::new(pool.clone()));
    let biblioteca = adapters::controllers::LibraryController::new(repositorio);
    let fotos = biblioteca
        .get_all_photos()
        .await
        .expect("reler as fotos pelo mesmo caminho do app");

    // A exposição, e não "difere do neutro": ela é `REAL` sem `DEFAULT` no
    // schema, então `Some` ali significa que alguém gravou.
    let mut lidas: Vec<f32> = fotos
        .iter()
        .filter(|foto| foto.edit_exposure.is_some())
        .map(|foto| persistencia::da_foto(foto).exposure)
        .collect();
    lidas.sort_by(f32::total_cmp);

    let mut esperadas = exposicoes_gravadas.to_vec();
    esperadas.sort_by(f32::total_cmp);

    if lidas != esperadas {
        // Os valores, e não só a contagem: o modo de falha mais provável é a
        // exposição voltar como **zero** — que é o que acontece quando `da_foto`
        // deixa de copiar o campo. Aí as duas listas têm o mesmo tamanho, e uma
        // mensagem que só contasse diria "2 e 2" e não ajudaria ninguém.
        eprintln!(
            "❌ a revelação se perde entre o banco e a tela — a Revelação abriria o arquivo cru.\n   \
             gravadas: {esperadas:?}\n   lidas:    {lidas:?}"
        );
        std::process::exit(1);
    }

    println!(
        "✅ {} delas voltam com a revelação que foi gravada, lida pelo caminho do app.",
        lidas.len()
    );
}
