//! 🛡️ O arquivo de origem no disco não é escrito por caminho nenhum do app.
//!
//! # Por que este teste existe
//!
//! Pedido do dono em 2026-09-11, depois de encontrar edições que não voltavam
//! ao estado inicial: *"quero uma coisa à prova de falhas, como se fosse um
//! guardião aonde nada e nem ninguém consiga quebrar essa segurança"*.
//!
//! No servidor e no navegador o bruto é uma **cópia** que o sistema guarda, e
//! por isso lá a proteção é uma trava: um `CHECK`, um gatilho, um campo privado.
//! Aqui no desktop não existe cópia — o bruto **é** o arquivo do fotógrafo, na
//! pasta dele. Não há o que travar; o que há é uma invariante estrutural: a
//! revelação é paramétrica, e nenhum gesto do app grava por cima da origem.
//!
//! 🔑 **Invariante sem teste é intenção.** Ela vale hoje porque cada caminho foi
//! escrito assim, um de cada vez — e basta um `save` com o caminho errado, num
//! gesto novo daqui a três meses, para ela deixar de valer sem que nada
//! reclame. O sintoma apareceria semanas depois, na forma pior: "a foto original
//! não é mais a que eu importei".
//!
//! # O que ele faz
//!
//! Tira a impressão digital do arquivo, exerce os gestos que mexem em pixels —
//! revelar, exportar, renderizar o bruto, imprimir — e cobra o **mesmo** hash no
//! fim. Byte a byte, e não "parece igual": qualquer escrita muda o SHA-256.

use domain::entities::Photo;
use domain::services::ImageExporter;
use domain::value_objects::{ExportOptions, FilePath};
use image::RgbImage;
use infrastructure::content_hash::calculate_file_hash;
use infrastructure::image_exporter::ImageExporterImpl;

fn foto_no_disco(dir: &tempfile::TempDir) -> (Photo, std::path::PathBuf) {
    let mut imagem = RgbImage::new(48, 48);
    for (x, y, pixel) in imagem.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x * 5) as u8, (y * 5) as u8, 120]);
    }
    let caminho = dir.path().join("DSC_0001.png");
    imagem.save(&caminho).expect("gravar a foto de origem");
    let foto = Photo::new(FilePath::new(caminho.to_str().unwrap()).unwrap());
    (foto, caminho)
}

/// Exposição, saturação e um corte — três famílias de edição, para o teste não
/// passar por acaso quando só uma delas estiver sendo aplicada.
fn com_revelacao(mut foto: Photo) -> Photo {
    #[rustfmt::skip]
    foto.set_edits(
        Some(1.2), None, None, None, None, None, None, None, None, None,
        Some(-60.0),
        None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None,
        None, None, None, None,
        None, None, None, None, None,
        None, None,
        Some(0.1), Some(0.1), Some(0.6), Some(0.6),
        None, None, None, None,
    )
    .expect("os ajustes do teste são válidos");
    foto
}

/// 🚨 **O gesto que mais perto chega de escrever é a exportação** — e ela
/// escreve **noutro** arquivo. Se um dia alguém trocar o destino pelo caminho de
/// origem (um "salvar por cima" que pareça conveniente), é aqui que se descobre.
#[tokio::test]
async fn revelar_e_exportar_nao_tocam_no_arquivo_de_origem() {
    let dir = tempfile::tempdir().unwrap();
    let (foto, caminho) = foto_no_disco(&dir);
    let foto = com_revelacao(foto);
    let antes = calculate_file_hash(&caminho).expect("a digital de origem");

    let exportador = ImageExporterImpl::new();
    let opcoes = ExportOptions::default();

    // 1. Revelar para bytes — o caminho do envio ao pós-venda.
    exportador
        .renderizar_jpeg(&foto, &opcoes)
        .await
        .expect("revela");

    // 2. Renderizar o bruto — o que sobe ao lado, para o "Zerar tudo" ter volta.
    exportador
        .renderizar_bruto_jpeg(&foto, &opcoes)
        .await
        .expect("renderiza o bruto");

    // 3. Exportar para disco — o gesto que escreve de verdade.
    let saida = dir.path().join("exportada.jpg");
    exportador
        .export(
            &foto,
            &FilePath::new(saida.to_str().unwrap()).unwrap(),
            &opcoes,
        )
        .await
        .expect("exporta");

    assert!(saida.exists(), "a exportação escreveu onde devia");
    assert_eq!(
        calculate_file_hash(&caminho).expect("a digital de agora"),
        antes,
        "o arquivo de origem mudou — algum caminho do app escreveu por cima dele"
    );
}

/// ⚠️ **E exportar com o mesmo nome, em outra pasta, também não.** É o caso que
/// mais se parece com "salvar por cima" sem ser: o nome bate, a pasta não, e um
/// `join` errado em algum ponto do caminho resolveria para a origem.
#[tokio::test]
async fn exportar_com_o_mesmo_nome_noutra_pasta_nao_toca_na_origem() {
    let dir = tempfile::tempdir().unwrap();
    let saidas = tempfile::tempdir().unwrap();
    let (foto, caminho) = foto_no_disco(&dir);
    let foto = com_revelacao(foto);
    let antes = calculate_file_hash(&caminho).expect("a digital de origem");

    let saida = saidas.path().join("DSC_0001.png");
    ImageExporterImpl::new()
        .export(
            &foto,
            &FilePath::new(saida.to_str().unwrap()).unwrap(),
            &ExportOptions::default(),
        )
        .await
        .expect("exporta");

    assert_eq!(
        calculate_file_hash(&caminho).expect("a digital de agora"),
        antes,
        "o arquivo de origem mudou ao exportar com o mesmo nome noutra pasta"
    );
}

/// 🔑 **A prova de que o teste sabe reprovar.** Sem ela, um `calculate_file_hash`
/// que devolvesse o mesmo valor para tudo — um caminho errado engolido, um
/// retorno constante — faria os dois casos acima passarem para sempre, inclusive
/// no dia em que o app começasse a escrever por cima da origem.
#[test]
fn a_digital_muda_quando_o_arquivo_muda() {
    let dir = tempfile::tempdir().unwrap();
    let (_, caminho) = foto_no_disco(&dir);
    let antes = calculate_file_hash(&caminho).unwrap();

    let mut outra = RgbImage::new(48, 48);
    for pixel in outra.pixels_mut() {
        *pixel = image::Rgb([9, 9, 9]);
    }
    outra.save(&caminho).expect("sobrescrever de propósito");

    assert_ne!(
        calculate_file_hash(&caminho).unwrap(),
        antes,
        "a digital não mudou depois de o arquivo ser reescrito: ela não serve de guardiã"
    );
}
