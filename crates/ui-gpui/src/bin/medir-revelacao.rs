//! Quanto custa **entrar na Revelação e andar uma foto**, por parte.
//!
//! # Por que isto existe
//!
//! Em 8/set/2026 a Revelação passou a abrir de verdade as fotos que só estão no
//! disco — antes elas viravam foto do site, não achavam nada no cache e a tela
//! ficava preta. O trabalho que não acontecia passou a acontecer, e o relato
//! foi imediato: *"ficou muito mais lento"*.
//!
//! Duas leituras cabem nessa frase, e elas pedem decisões opostas: ou a tela
//! agora paga o preço normal de mostrar uma foto de 2560px (e o lento é o
//! esperado), ou alguma coisa nova custa caro por tecla. Dívida sem número não
//! dá para priorizar — este binário põe o número em cada parte.
//!
//! ```bash
//! cargo run --release -p ui-gpui --bin medir-revelacao
//! ```
//!
//! ⚠️ **Só vale em `--release`**: em `debug` decodificar JPEG custa 56× mais, e
//! a fase 1 quase condenou o framework medindo no perfil errado.

use std::sync::Arc;
use std::time::Instant;

use domain::services::PreviewType;
use infrastructure::cache::preview_manager::PreviewManager;
use infrastructure::paths::AppPaths;

/// O orçamento de um quadro a 60fps.
const QUADRO_MS: f64 = 16.7;

#[tokio::main]
async fn main() {
    if cfg!(debug_assertions) {
        eprintln!("⚠️  perfil `debug`: os números não valem. Use --release.\n");
    }

    let previews = Arc::new(PreviewManager::new());
    let pool = infrastructure::create_pool(&AppPaths::main_db_path().to_string_lossy())
        .await
        .expect("abrir o banco");
    let repo = infrastructure::PhotoRepositoryImpl::new(pool);

    let fotos = {
        use domain::repositories::PhotoRepository;
        repo.find_all().await.expect("ler as fotos")
    };
    println!("catálogo: {} fotos\n", fotos.len());

    // A maior sessão do catálogo é a que se revela em série — é ela que a tira
    // mostra, e é sobre ela que a conta importa.
    let mut por_sessao: std::collections::HashMap<String, Vec<String>> = Default::default();
    for foto in &fotos {
        let chave = foto.sessao().unwrap_or("(sem ensaio)").to_string();
        por_sessao
            .entry(chave)
            .or_default()
            .push(foto.id().to_string());
    }
    let (ensaio, ids) = por_sessao
        .into_iter()
        .max_by_key(|(_, ids)| ids.len())
        .expect("ao menos um ensaio");
    println!("maior ensaio: {ensaio} — {} fotos\n", ids.len());

    // 1 · A varredura da reposição: duas perguntas por foto, na thread da
    //     interface, a cada troca de foto.
    let inicio = Instant::now();
    let mut faltando = 0;
    for id in &ids {
        if !(previews.tem(id, PreviewType::Large) && previews.tem(id, PreviewType::Thumbnail)) {
            faltando += 1;
        }
    }
    let varredura = inicio.elapsed().as_secs_f64() * 1000.0;
    veredito(
        "varredura da reposição (por tecla)",
        varredura,
        &format!("{} perguntas, {faltando} faltando", ids.len() * 2),
    );

    // 2 · O que `mostrar` faz por foto: ler o preview grande do cache e
    //     convertê-lo para os pixels que sobem à GPU.
    let alvo = &ids[0];
    let inicio = Instant::now();
    let imagem = previews.get_preview(alvo).expect("o preview do cache");
    let leitura = inicio.elapsed().as_secs_f64() * 1000.0;

    let inicio = Instant::now();
    let rgba = imagem.to_rgba8();
    let conversao = inicio.elapsed().as_secs_f64() * 1000.0;
    let (l, a) = (rgba.width(), rgba.height());

    veredito(
        "ler o preview do cache (1ª vez, decodifica)",
        leitura,
        &format!("{l}x{a}"),
    );
    veredito(
        "to_rgba8 (o que vai para a GPU)",
        conversao,
        &format!("{:.1} MB", (l as f64 * a as f64 * 4.0) / 1e6),
    );

    // A segunda leitura mede o cache em memória (L1), que é o caminho da seta
    // de volta.
    let inicio = Instant::now();
    let _ = previews.get_preview(alvo);
    veredito(
        "ler o preview de novo (memória)",
        inicio.elapsed().as_secs_f64() * 1000.0,
        "é o que a seta de volta paga",
    );

    // 3 · A tira: uma miniatura por foto, na primeira montagem.
    let inicio = Instant::now();
    let mut lidas = 0;
    for id in ids.iter().take(50) {
        if previews.get_thumbnail(id).is_some() {
            lidas += 1;
        }
    }
    veredito(
        "miniaturas da tira (1ª montagem)",
        inicio.elapsed().as_secs_f64() * 1000.0,
        &format!("{lidas} lidas"),
    );

    // 4 · A sequência de verdade: o palco lê o preview, a tira lê as miniaturas,
    //     e a seta volta ao palco. É aqui que se vê se a tira despeja o preview.
    println!("\n--- a sequência real de entrar na Revelação e andar ---");
    let _ = previews.get_preview(alvo);
    let inicio = Instant::now();
    for id in ids.iter().take(50) {
        let _ = previews.get_thumbnail(id);
    }
    let tira = inicio.elapsed().as_secs_f64() * 1000.0;

    let inicio = Instant::now();
    let _ = previews.get_preview(alvo);
    let depois_da_tira = inicio.elapsed().as_secs_f64() * 1000.0;

    veredito("a tira lê 50 miniaturas", tira, "uma vez, ao montar");
    veredito(
        "o palco relê o preview DEPOIS da tira",
        depois_da_tira,
        if depois_da_tira > 10.0 {
            "🔥 a tira despejou o preview da memória — cada seta redecodifica"
        } else {
            "o preview sobreviveu na memória"
        },
    );

    println!("\nO orçamento de um quadro a 60fps é {QUADRO_MS} ms.");
}

fn veredito(nome: &str, ms: f64, detalhe: &str) {
    let marca = if ms > QUADRO_MS { "🚨" } else { "✅" };
    println!("{marca} {nome}: {ms:.2} ms  ({detalhe})");
}
