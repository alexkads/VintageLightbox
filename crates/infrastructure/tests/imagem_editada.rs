//! A exportação parte da imagem editada, e o bruto continua o bruto
//! (`docs/editor-em-camadas/02-CONTRATO.md`, C28–C34).
//!
//! 🔑 **Três perguntas, cada uma com arquivo de verdade no disco:**
//! 1. a foto exportada tem o traço do editor, com a revelação por cima (C32);
//! 2. o "bruto" que sobe ao site **não** tem o traço (C2, C34);
//! 3. o JPEG da exportação e o do pós-venda (bytes da imagem editada) são o
//!    mesmo arquivo — um caminho, uma resposta.

use std::path::PathBuf;
use std::sync::Arc;

use domain::entities::Photo;
use domain::services::ImageExporter;
use domain::value_objects::{ExportOptions, FilePath};
use editor_core::projeto::{DiscoReal, Projeto};
use editor_core::{BaseRef, Documento, Historico, Sessao};
use image::RgbImage;
use infrastructure::gpu_adjustments::{Ajustes, ParametrosLocais};
use infrastructure::image_exporter::ImageExporterImpl;

struct Campos {
    saturation: Option<f32>,
    hsl_red_sat: Option<f32>,
    crop_x: Option<f32>,
    crop_y: Option<f32>,
    crop_width: Option<f32>,
    crop_height: Option<f32>,
}

fn com_campos(mut foto: Photo, campos: Campos) -> Photo {
    #[rustfmt::skip]
    foto.set_edits(
        None, None, None, None, None, None, None, None, None, None,
        campos.saturation,
        None, None, None, None,
        campos.hsl_red_sat,
        None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None,
        None, None, None, None,
        None, None, None, None, None,
        None, None,
        campos.crop_x, campos.crop_y, campos.crop_width, campos.crop_height,
        None, None, None, None,
    )
    .expect("os ajustes do teste são válidos");
    foto
}

/// Uma foto cinza-azulada de 120×80 no disco, e a edição dela com um traço
/// vermelho no meio, salva como projeto.
fn foto_editada(dir: &tempfile::TempDir) -> (Photo, PathBuf, RgbImage) {
    let base = RgbImage::from_fn(120, 80, |x, y| {
        image::Rgb([60 + (x / 4) as u8, 90, 120 + (y / 4) as u8])
    });
    let caminho = dir.path().join("bruto.png");
    base.save(&caminho).unwrap();
    let foto = Photo::new(FilePath::new(caminho.to_str().unwrap()).unwrap());

    let base = Arc::new(
        infrastructure::base_neutra::base_neutra(&caminho)
            .unwrap()
            .to_rgb8(),
    );
    let doc = Documento::novo(BaseRef::da_imagem(&base));
    let mut sessao = Sessao::nova(base.clone(), doc, Historico::novo(), 100);
    sessao.pincel.cor = [230, 20, 20];
    sessao.pincel.raio = 6.0;
    sessao.pincel.dureza = 1.0;
    sessao.apertar(20.0, 40.0);
    sessao.arrastar(100.0, 40.0);
    sessao.soltar();
    let (doc, hist) = sessao.instantaneo();
    let projeto = Projeto::novo(dir.path().join("edicao"), Arc::new(DiscoReal));
    let versao = projeto
        .salvar("e1", &base, &doc, &hist, 1)
        .unwrap()
        .versao
        .expect("com traço há imagem editada");
    (foto, versao.arquivo, (*base).clone())
}

fn saturada(foto: Photo) -> Photo {
    com_campos(
        foto,
        Campos {
            saturation: Some(30.0),
            hsl_red_sat: None,
            crop_x: None,
            crop_y: None,
            crop_width: None,
            crop_height: None,
        },
    )
}

fn exportador_com(foto: &Photo, editada: &std::path::Path) -> ImageExporterImpl {
    let id = foto.id();
    let editada = editada.to_path_buf();
    ImageExporterImpl::new().com_editadas(Arc::new(move |p: &Photo| {
        (p.id() == id).then(|| editada.clone())
    }))
}

fn vermelho(p: [u8; 3]) -> bool {
    p[0] > 150 && p[1] < 80 && p[2] < 80
}

#[tokio::test]
async fn a_exportacao_parte_da_imagem_editada_e_o_bruto_continua_o_bruto() {
    let dir = tempfile::tempdir().unwrap();
    let (foto, editada, base) = foto_editada(&dir);
    let foto = saturada(foto);
    let exportador = exportador_com(&foto, &editada);

    // 1. A revelada tem o traço (e a revelação: saturação sobre ele).
    let revelada = exportador
        .renderizar(&foto, &ExportOptions::default())
        .unwrap()
        .to_rgb8();
    assert_eq!((revelada.width(), revelada.height()), (120, 80));
    assert!(
        vermelho(revelada.get_pixel(60, 40).0),
        "{:?}",
        revelada.get_pixel(60, 40)
    );
    assert!(
        !vermelho(revelada.get_pixel(60, 10).0),
        "fora do traço é a foto"
    );

    // Sem o editor ligado, a mesma foto sai sem traço: a diferença é só a fonte.
    let sem_editor = ImageExporterImpl::new()
        .renderizar(&foto, &ExportOptions::default())
        .unwrap()
        .to_rgb8();
    assert!(!vermelho(sem_editor.get_pixel(60, 40).0));

    // 2. O "bruto" que sobe ao site é o bruto: sem o traço.
    let bruto = exportador
        .renderizar_bruto_jpeg(&foto, &ExportOptions::default())
        .await
        .unwrap()
        .expect("com revelação, sobe o bruto à parte");
    let bruto = image::load_from_memory(&bruto).unwrap().to_rgb8();
    assert!(!vermelho(bruto.get_pixel(60, 40).0));
    let p = base.get_pixel(60, 40).0;
    let q = bruto.get_pixel(60, 40).0;
    assert!(
        (0..3).all(|i| (p[i] as i32 - q[i] as i32).abs() <= 6),
        "{p:?} × {q:?}"
    );

    // O arquivo do bruto no disco não mudou (C2).
    let no_disco = image::open(foto.file_path().as_str().unwrap())
        .unwrap()
        .to_rgb8();
    assert_eq!(no_disco.as_raw(), base.as_raw());
}

#[tokio::test]
async fn exportacao_e_pos_venda_saem_o_mesmo_arquivo() {
    let dir = tempfile::tempdir().unwrap();
    let (foto, editada, _) = foto_editada(&dir);
    let foto = saturada(foto);
    let exportador = exportador_com(&foto, &editada);

    let exportado = exportador
        .renderizar_jpeg(&foto, &ExportOptions::default().with_quality(92))
        .await
        .unwrap();
    // O pós-venda revela os **bytes** da imagem editada (C32), com a mesma revelação.
    let ajustes = Ajustes {
        saturation: 30.0,
        ..Default::default()
    };
    let do_pos_venda = exportador
        .renderizar_bytes(
            &std::fs::read(&editada).unwrap(),
            &ajustes,
            &Default::default(),
            &ParametrosLocais::default(),
            92,
        )
        .unwrap();
    assert_eq!(exportado, do_pos_venda);
}
