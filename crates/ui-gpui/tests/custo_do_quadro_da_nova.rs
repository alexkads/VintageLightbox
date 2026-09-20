//! 📏 Quanto custa **um quadro** da etapa 3 do assistente, em consultas ao
//! cache de prévias.
//!
//! Dono, 20/set/2026: *"não consigo digitar o título como se tivesse um bug no
//! input"* — faltando letras, com uma importação correndo por baixo (a bandeja
//! marcava 54 → 55 → 60 fotos esperando nota). Letra que some é a linha da
//! interface parada quando a tecla chega.
//!
//! `numeros_da_copia()` chama `quantas_previas()`, que percorre **todas** as
//! fotos do rascunho perguntando ao `PreviewManager` se cada uma tem miniatura
//! e prévia grande — duas consultas SQLite por foto, cada uma pegando o mesmo
//! `Mutex` que o trabalhador da importação está usando para **gravar**. E
//! `numeros_da_copia()` é chamado duas vezes por desenho (o cabeçalho e a área
//! das fotos).
//!
//! Este teste não abre janela: mede só o preço do lote de consultas que um
//! quadro paga.

use std::time::Instant;

use domain::services::PreviewType;
use infrastructure::cache::preview_manager::PreviewManager;

/// O que um quadro pergunta ao cache: 2 chamadas de `numeros_da_copia`, cada
/// uma percorrendo as N fotos e perguntando pelos dois tipos.
fn um_quadro(previews: &PreviewManager, ids: &[String]) -> usize {
    let mut previas = 0;
    for _ in 0..2 {
        previas = ids
            .iter()
            .filter(|id| {
                previews.tem(id, PreviewType::Thumbnail) || previews.tem(id, PreviewType::Large)
            })
            .count();
    }
    previas
}

#[test]
fn o_quadro_da_etapa_3_paga_quatro_consultas_por_foto() {
    let dir = tempfile::TempDir::new().expect("diretório temporário");
    let previews = PreviewManager::new_with_path(dir.path().to_path_buf());

    let imagem = image::DynamicImage::ImageRgba8(image::RgbaImage::new(320, 240));
    let ids: Vec<String> = (0..60).map(|n| format!("id-foto-{n}")).collect();
    for id in &ids {
        previews.save_preview(id, &imagem).expect("gravar a prévia");
    }

    // Aquece: a primeira volta paga a abertura do banco.
    um_quadro(&previews, &ids);

    let comeco = Instant::now();
    let quadros = 20;
    for _ in 0..quadros {
        um_quadro(&previews, &ids);
    }
    let por_quadro = comeco.elapsed() / quadros;

    println!(
        "60 fotos · {} consultas por quadro · {:?} por quadro",
        ids.len() * 4,
        por_quadro
    );

    // 16,6 ms é o orçamento de um quadro a 60 Hz. O que passa disso, a linha da
    // interface paga — e quem está digitando perde a tecla.
    assert!(
        por_quadro < std::time::Duration::from_millis(16),
        "um quadro da etapa 3 gasta {por_quadro:?} só perguntando ao cache de \
         prévias, com 60 fotos no rascunho: o orçamento de 60 Hz é 16 ms"
    );
}

/// O outro lado do mesmo quadro: antes, `preparar_miniaturas()` lia o cache na
/// linha da interface a **cada desenho**, e toda foto nova custava a leitura
/// inteira. Este teste mede as duas partes separadas, que é o que justifica
/// onde cada uma passou a rodar (ver `sessoes::nova::miniaturas`).
///
/// 🚨 A leitura é **117×** mais cara que a conversão. Era ela, e só ela, que
/// precisava sair da linha da interface.
#[test]
fn a_leitura_do_cache_e_que_custa_e_nao_a_conversao() {
    let dir = tempfile::TempDir::new().expect("diretório temporário");
    let previews = PreviewManager::new_with_path(dir.path().to_path_buf());

    // Uma foto de tamanho realista de prévia, e não um retângulo vazio.
    let mut imagem = image::RgbaImage::new(1280, 960);
    for (x, y, p) in imagem.enumerate_pixels_mut() {
        *p = image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255]);
    }
    let imagem = image::DynamicImage::ImageRgba8(imagem);

    let ids: Vec<String> = (0..10).map(|n| format!("id-leva-{n}")).collect();
    for id in &ids {
        previews.save_preview(id, &imagem).expect("gravar a prévia");
    }

    let mut lendo = std::time::Duration::ZERO;
    let mut convertendo = std::time::Duration::ZERO;
    for id in &ids {
        let comeco = Instant::now();
        let lida = previews
            .get_thumbnail(id)
            .or_else(|| previews.get_preview(id).map(|g| g.thumbnail(320, 320)));
        lendo += comeco.elapsed();

        let lida = lida.expect("a prévia foi gravada acima");
        let comeco = Instant::now();
        let _ = ui_gpui::imagem::para_gpui(lida);
        convertendo += comeco.elapsed();
    }

    println!(
        "leva de {} fotos · ler {lendo:?} ({:?}/foto) · converter {convertendo:?} ({:?}/foto)",
        ids.len(),
        lendo / ids.len() as u32,
        convertendo / ids.len() as u32,
    );

    // A leitura estoura o orçamento de um quadro a 60 Hz com folga — é por isso
    // que ela foi para a thread `miniaturas-da-nova`.
    assert!(
        lendo > std::time::Duration::from_millis(16),
        "se ler o cache ficou barato ({lendo:?} para {} fotos), a thread das \
         miniaturas perdeu a razão de existir — reavalie o módulo",
        ids.len()
    );

    // E a conversão cabe, com folga, no que sobrou: por isso ela continua na
    // linha da interface, em `Miniaturas::colher`.
    assert!(
        convertendo < std::time::Duration::from_millis(4),
        "converter {} fotos custou {convertendo:?}: se passar do quadro, ela \
         também precisa sair da linha da interface",
        ids.len()
    );
}
