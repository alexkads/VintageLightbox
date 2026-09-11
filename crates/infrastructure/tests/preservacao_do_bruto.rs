//! A foto que sobe revelada tem de levar o arquivo de antes junto.
//!
//! 🚨 **O defeito que estes testes prendem custou dias para ser notado.** O
//! envio ao pós-venda renderiza **com os ajustes do catálogo**: o que chega ao
//! site já é a foto tratada. Sem uma segunda cópia, o servidor passa a tratar o
//! revelado como se fosse o original — e `restaurar_original` lá encontra o
//! campo do bruto vazio, conclui "nunca revelada" e devolve a própria foto
//! revelada, respondendo sucesso. "Zerar tudo" não faz nada e não avisa.
//!
//! O dono achou pelo sintoma, sem pista do lugar: *"já vi que teve edições que
//! não consegui voltar ao estado inicial"* (11/set/2026).
//!
//! 🔑 **Por que o teste mora aqui, e não no use case.** O use case só repassa o
//! que o exportador devolve — mockado, ele passaria com qualquer coisa. A
//! pergunta que importa é sobre **pixels**: o que sai de
//! [`ImageExporter::renderizar_bruto_jpeg`] é mesmo a foto sem tratamento, ou
//! alguém deixou um ajuste vazar? Isso só a GPU responde, e só comparando com o
//! arquivo de origem.

use domain::entities::Photo;
use domain::services::ImageExporter;
use domain::value_objects::{ExportOptions, FilePath};
use image::{DynamicImage, GenericImageView, RgbImage};
use infrastructure::image_exporter::ImageExporterImpl;

/// Uma foto de 64×64 com faixas de cor e uma rampa de brilho por coluna.
///
/// A rampa existe para haver o que exposição e contraste movam: numa imagem de
/// um tom só, "com ajuste" e "sem ajuste" podem sair iguais por acidente, e o
/// teste passaria sem provar nada.
fn foto_no_disco(dir: &tempfile::TempDir, nome: &str) -> (Photo, DynamicImage) {
    const CORES: [[u8; 3]; 4] = [[220, 40, 40], [40, 200, 60], [50, 80, 220], [230, 220, 40]];

    let mut imagem = RgbImage::new(64, 64);
    for (x, y, pixel) in imagem.enumerate_pixels_mut() {
        let cor = CORES[((y / 16) % 4) as usize];
        let escala = 0.4 + (x as f32 / 64.0) * 0.6;
        *pixel = image::Rgb([
            (cor[0] as f32 * escala) as u8,
            (cor[1] as f32 * escala) as u8,
            (cor[2] as f32 * escala) as u8,
        ]);
    }

    let caminho = dir.path().join(nome);
    imagem.save(&caminho).expect("gravar a foto de origem");
    let foto = Photo::new(FilePath::new(caminho.to_str().unwrap()).unwrap());
    (foto, DynamicImage::ImageRgb8(imagem))
}

/// Põe exposição, saturação e um corte pela metade — três edições de famílias
/// diferentes, para o teste não passar por uma delas ter sido esquecida.
fn com_revelacao(mut foto: Photo) -> Photo {
    #[rustfmt::skip]
    foto.set_edits(
        Some(1.4), None, None, None, None, None, None, None, None, None,
        Some(-80.0),
        None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None,
        None, None, None, None,
        None, None, None, None, None,
        None, None,
        Some(0.0), Some(0.0), Some(0.5), Some(0.5),
        None, None, None, None,
    )
    .expect("os ajustes do teste são válidos");
    foto
}

/// A distância média entre duas imagens do mesmo tamanho, por canal.
///
/// 🔑 **Média, e não "são iguais"**: o bruto passa pelo shader e pelo
/// codificador JPEG, então ele **não** sai byte a byte igual ao PNG de origem.
/// O que se pergunta é se ele está perto da origem (nenhum tratamento) ou longe
/// (tratado) — e a diferença entre os dois casos é de ordens de grandeza.
fn distancia(a: &DynamicImage, b: &DynamicImage) -> f64 {
    assert_eq!(a.dimensions(), b.dimensions(), "tamanhos diferentes");
    let (largura, altura) = a.dimensions();
    let mut soma = 0f64;
    for y in 0..altura {
        for x in 0..largura {
            let pa = a.get_pixel(x, y).0;
            let pb = b.get_pixel(x, y).0;
            for canal in 0..3 {
                soma += (pa[canal] as f64 - pb[canal] as f64).abs();
            }
        }
    }
    soma / (largura as f64 * altura as f64 * 3.0)
}

fn abrir(bytes: &[u8]) -> DynamicImage {
    image::load_from_memory(bytes).expect("o JPEG do bruto abre")
}

/// 🚨 **O bruto não leva os ajustes.** É o coração da proteção: se um ajuste
/// vazar para cá, o "arquivo original" guardado no servidor já vem tratado — e
/// "Zerar tudo" devolveria a foto revelada achando que devolveu o original.
/// O defeito seria invisível: o arquivo existe, tem o tamanho certo, abre.
#[tokio::test]
async fn o_bruto_sai_sem_os_ajustes_da_revelacao() {
    let dir = tempfile::tempdir().unwrap();
    let (origem, pixels_de_origem) = foto_no_disco(&dir, "origem.png");
    let foto = com_revelacao(origem);
    let exportador = ImageExporterImpl::new();

    let bruto = exportador
        .renderizar_bruto_jpeg(&foto, &ExportOptions::default())
        .await
        .expect("o bruto é renderizado")
        .expect("a foto tem revelação, então há bruto a guardar");
    let revelada = exportador
        .renderizar_jpeg(&foto, &ExportOptions::default())
        .await
        .expect("a revelada é renderizada");

    let perto = distancia(&abrir(&bruto), &pixels_de_origem);
    assert!(
        perto < 12.0,
        "o bruto devia ser a foto de origem, e está a {perto:.1} níveis dela"
    );

    // E a revelada tem de estar **longe**: sem isto, um teste que compara o
    // bruto com a origem passaria também num app que nunca revela nada.
    let revelada = abrir(&revelada);
    assert_ne!(
        revelada.dimensions(),
        pixels_de_origem.dimensions(),
        "a revelada leva o corte, então nem o tamanho bate"
    );
}

/// 🚨 **O corte também é edição, e também fica de fora.** Enquadrar é uma
/// decisão tão desfazível quanto mexer na exposição; um bruto já recortado
/// devolveria metade do arrependimento, e a metade que falta não volta nunca.
#[tokio::test]
async fn o_bruto_sai_sem_o_enquadramento() {
    let dir = tempfile::tempdir().unwrap();
    let (origem, pixels_de_origem) = foto_no_disco(&dir, "origem.png");
    let foto = com_revelacao(origem);

    let bruto = ImageExporterImpl::new()
        .renderizar_bruto_jpeg(&foto, &ExportOptions::default())
        .await
        .expect("o bruto é renderizado")
        .expect("a foto tem revelação");

    assert_eq!(
        abrir(&bruto).dimensions(),
        pixels_de_origem.dimensions(),
        "o corte era metade por metade: se ele vazou, o tamanho denuncia"
    );
}

/// ⚠️ **A foto no neutro não paga por um bruto.** O envio dela **é** o arquivo
/// original; renderizar de novo custaria uma geração de JPEG a mais para
/// produzir o mesmo arquivo, e o servidor guardaria duas cópias iguais.
///
/// 🔑 A decisão mora no exportador, e não em quem chama: quem sabe ler os 53
/// campos da entidade é ele, e espalhar "isto está editado?" pelos chamadores é
/// como um deles acaba perguntando diferente dos outros.
#[tokio::test]
async fn a_foto_no_neutro_nao_gera_bruto() {
    let dir = tempfile::tempdir().unwrap();
    let (foto, _) = foto_no_disco(&dir, "origem.png");

    let bruto = ImageExporterImpl::new()
        .renderizar_bruto_jpeg(&foto, &ExportOptions::default())
        .await
        .expect("não é erro: é ausência");

    assert!(
        bruto.is_none(),
        "sem revelação não há segunda cópia a guardar"
    );
}

/// 🚨 **Só o corte também conta como revelação.** Uma foto sem nenhum slider
/// mexido mas com enquadramento tem sim o que desfazer — e a conferência que
/// olhasse apenas os 53 ajustes diria "está no neutro" e deixaria o corte sem
/// volta.
#[tokio::test]
async fn so_o_corte_ja_pede_o_bruto() {
    let dir = tempfile::tempdir().unwrap();
    let (foto, pixels_de_origem) = foto_no_disco(&dir, "origem.png");
    let mut foto = foto;
    #[rustfmt::skip]
    foto.set_edits(
        None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None,
        None, None, None, None,
        None, None, None, None, None,
        None, None,
        Some(0.25), Some(0.25), Some(0.5), Some(0.5),
        None, None, None, None,
    )
    .unwrap();

    let bruto = ImageExporterImpl::new()
        .renderizar_bruto_jpeg(&foto, &ExportOptions::default())
        .await
        .expect("o bruto é renderizado")
        .expect("enquadrar é editar: há o que guardar");

    assert_eq!(
        abrir(&bruto).dimensions(),
        pixels_de_origem.dimensions(),
        "e o bruto guardado é a foto inteira"
    );
}
