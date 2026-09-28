//! A revelação local chega ao arquivo: exportar aplica as máscaras, o bruto
//! continua sendo o bruto, e o que sobe como `ajustes` continua plano.
//!
//! ⚠️ Precisa de GPU, como toda exportação (`ImageExporterImpl`).

use domain::entities::Photo;
use domain::services::ImageExporter;
use domain::value_objects::{ExportOptions, FilePath};
use image::{DynamicImage, GenericImageView, RgbImage};
use infrastructure::gpu_adjustments::ParametrosLocais;
use infrastructure::image_exporter::ImageExporterImpl;
use revelacao_core::locais::*;

fn foto_no_disco(dir: &tempfile::TempDir) -> Photo {
    let mut imagem = RgbImage::new(96, 64);
    for (x, y, pixel) in imagem.enumerate_pixels_mut() {
        *pixel = image::Rgb([(40 + x) as u8, (60 + y) as u8, 90]);
    }
    let caminho = dir.path().join("origem.png");
    imagem.save(&caminho).unwrap();
    Photo::new(FilePath::new(caminho.to_str().unwrap()).unwrap())
}

/// Uma máscara grande no lado esquerdo, com +2 EV.
fn parametros(pontos: usize) -> ParametrosLocais {
    ParametrosLocais {
        camadas: vec![Camada {
            ajustes: AjustesLocais { exposicao_ev: 2.0 },
            componentes: vec![Componente {
                modo: Modo::Somar,
                forma: Forma::Pincel(BrushStroke {
                    raio: 0.15,
                    feather: 0.3,
                    opacidade: 1.0,
                    pontos: (0..pontos)
                        .map(|i| [0.2, i as f32 / pontos.max(1) as f32, 1.0])
                        .collect(),
                }),
            }],
            invertida: false,
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn media_da_coluna(img: &DynamicImage, x: u32) -> f32 {
    (0..img.height())
        .map(|y| img.get_pixel(x, y)[0] as f32)
        .sum::<f32>()
        / img.height() as f32
}

#[test]
fn exportar_aplica_a_mascara_so_onde_ela_esta() {
    let dir = tempfile::tempdir().unwrap();
    let sem = foto_no_disco(&dir);
    let mut com = sem.clone();
    com.definir_locais(parametros(20).em_json());

    let exportador = ImageExporterImpl::new();
    let opcoes = ExportOptions::default();
    let a = exportador.renderizar(&sem, &opcoes).unwrap();
    let b = exportador.renderizar(&com, &opcoes).unwrap();

    let x_mascara = (0.2 * 96.0) as u32;
    assert!(
        media_da_coluna(&b, x_mascara) > media_da_coluna(&a, x_mascara) + 30.0,
        "a máscara clareia onde foi pintada"
    );
    assert_eq!(media_da_coluna(&b, 90), media_da_coluna(&a, 90), "e só lá");
}

/// 🚨 Revelação ilegível não exporta calada sem as máscaras.
#[test]
fn parametros_locais_ilegiveis_faz_a_exportacao_falhar() {
    let dir = tempfile::tempdir().unwrap();
    let mut foto = foto_no_disco(&dir);
    foto.definir_locais(Some(r#"{"versao": 99}"#.into()));
    let erro = ImageExporterImpl::new()
        .renderizar(&foto, &ExportOptions::default())
        .unwrap_err();
    assert!(erro.to_string().contains("revelação local"), "{erro}");
}

/// 🚨 Foto só com máscara (sliders no neutro) tem bruto a guardar: o arquivo que
/// sobe é o mascarado, e sem o bruto o site o trataria como original.
#[tokio::test]
async fn foto_so_com_mascara_sobe_o_bruto_junto() {
    let dir = tempfile::tempdir().unwrap();
    let mut foto = foto_no_disco(&dir);
    let exportador = ImageExporterImpl::new();
    let opcoes = ExportOptions::default();
    assert!(exportador
        .renderizar_bruto_jpeg(&foto, &opcoes)
        .await
        .unwrap()
        .is_none());

    foto.definir_locais(parametros(4).em_json());
    assert!(
        exportador
            .renderizar_bruto_jpeg(&foto, &opcoes)
            .await
            .unwrap()
            .is_some(),
        "com máscara, o bruto é outro arquivo"
    );
}

/// 🚨 O contrato com a API do site (`conferir_forma` no backend): `ajustes` é um
/// objeto **plano de números**, com no máximo 16 KiB. A revelação local não entra
/// nele — com um stroke de 4000 pontos, o objeto continua o mesmo.
#[test]
fn os_parametros_que_sobe_continua_plana_com_mascara() {
    let dir = tempfile::tempdir().unwrap();
    let mut foto = foto_no_disco(&dir);
    #[rustfmt::skip]
    foto.set_edits(
        Some(0.5), None, None, None, None, None, None, None, None, None, None,
        None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None,
        None, None, None,
        None, None, None, None,
        None, None, None, None, None,
        None, None,
        None, None, None, None, None, None, None, None,
    )
    .unwrap();
    let exportador = ImageExporterImpl::new();
    let sem_mascara = exportador.parametros_para_o_site(&foto).unwrap();

    foto.definir_locais(parametros(4000).em_json());
    assert!(
        foto.locais().unwrap().len() > 16 * 1024,
        "a revelação local é grande"
    );
    let com_mascara = exportador.parametros_para_o_site(&foto).unwrap();

    assert_eq!(com_mascara, sem_mascara);
    let objeto = com_mascara.as_object().unwrap();
    assert!(objeto
        .values()
        .all(|v| v.as_f64().is_some_and(f64::is_finite)));
    assert!(com_mascara.to_string().len() <= 16 * 1024);
}
