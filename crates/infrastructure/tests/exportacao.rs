//! O arquivo exportado tem de ser o que a tela mostra.
//!
//! 🚨 **Até 17/ago/2026 ele não era**, e nada falhava. `ImageExporterImpl` tinha
//! a própria implementação dos ajustes, na CPU, com **15** dos 46: a curva de
//! tons inteira, o HSL nos 8 canais e a lente eram descartados em silêncio, e o
//! corte era ignorado. Quem revelava mexendo em HSL via um resultado na tela e
//! recebia outro no disco.
//!
//! 🔑 **Nenhum teste unitário alcança isto.** O defeito mora na junção — qual
//! caminho a exportação toma —, e não dentro de nenhuma função. Estes testes
//! gravam arquivo de verdade e leem de volta, que é a única pergunta que
//! importa: *o que está no disco é o que estava na tela?*

use std::sync::Arc;

use domain::entities::Photo;
use domain::services::ImageExporter;
use domain::value_objects::FilePath;
use image::{DynamicImage, GenericImageView, RgbImage};
use infrastructure::gpu_adjustments::{Ajustes, Motor};
use infrastructure::image_exporter::ImageExporterImpl;
use infrastructure::transformacao;

/// Os 54 campos de `set_edits`, com nome, para o teste não depender de acertar a
/// ordem de 54 argumentos posicionais.
///
/// ⚠️ **A dívida que este struct contorna é a que o STATUS registra**: a cadeia
/// de revelação viaja como parâmetro solto. Aqui ela é local ao teste — trocar
/// dois `None` de lugar numa lista de 54 compila e mede a coisa errada.
#[derive(Default, Clone, Copy)]
struct Campos {
    saturation: Option<f32>,
    hsl_red_sat: Option<f32>,
    crop_x: Option<f32>,
    crop_y: Option<f32>,
    crop_width: Option<f32>,
    crop_height: Option<f32>,
}

impl Campos {
    fn ajustes(&self) -> Ajustes {
        Ajustes {
            saturation: self.saturation.unwrap_or(0.0),
            hsl_red_sat: self.hsl_red_sat.unwrap_or(0.0),
            ..Default::default()
        }
    }
}

/// Uma foto de 64×64 com as oito cores do HSL, gravada em disco.
fn foto_no_disco(dir: &tempfile::TempDir, nome: &str) -> (Photo, DynamicImage) {
    const CORES: [[u8; 3]; 8] = [
        [220, 40, 40],
        [230, 140, 30],
        [230, 220, 40],
        [40, 200, 60],
        [40, 210, 200],
        [50, 80, 220],
        [140, 50, 210],
        [220, 50, 180],
    ];

    let mut imagem = RgbImage::new(64, 64);
    for (x, y, pixel) in imagem.enumerate_pixels_mut() {
        let cor = CORES[((y / 8) % 8) as usize];
        // Uma rampa por coluna, para haver o que exposição e contraste movam.
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
        campos.crop_x, campos.crop_y, campos.crop_width, campos.crop_height,
        None, None, None, None,
    )
    .expect("os ajustes do teste são válidos");
    foto
}

async fn exportar(foto: &Photo, destino: &std::path::Path) -> DynamicImage {
    let exportador = ImageExporterImpl::new();
    exportador
        .export(foto, &FilePath::new(destino.to_str().unwrap()).unwrap())
        .await
        .expect("exportar");
    image::open(destino).expect("reabrir o arquivo exportado")
}

/// 🚨 **A exportação ignorava o corte.** O fotógrafo enquadrava, via o corte no
/// viewer e nas miniaturas, exportava e recebia a imagem inteira.
///
/// A dimensão é o jeito mais barato de perguntar isso, e não dá para passar por
/// acaso: metade da largura e metade da altura é uma resposta só.
#[tokio::test]
async fn o_arquivo_sai_com_o_enquadramento_da_tela() {
    let dir = tempfile::tempdir().unwrap();
    let (foto, _) = foto_no_disco(&dir, "origem.png");
    let foto = com_campos(
        foto,
        Campos {
            crop_x: Some(0.25),
            crop_y: Some(0.25),
            crop_width: Some(0.5),
            crop_height: Some(0.5),
            ..Default::default()
        },
    );

    let saida = dir.path().join("cortada.jpg");
    let exportada = exportar(&foto, &saida).await;

    assert_eq!(
        exportada.dimensions(),
        (32, 32),
        "a foto é 64×64 e o corte pede metade em cada eixo — \
         64×64 aqui significa que a exportação voltou a ignorar o enquadramento"
    );
}

/// 🚨 **O HSL não chegava ao arquivo.** São 24 dos 46 ajustes — saturação, matiz
/// e luminância nos 8 canais —, e o exportador não tinha nenhum deles.
///
/// O teste pede o extremo num canal só (vermelho) para a diferença ser grande o
/// bastante para atravessar o JPEG de qualidade 90 sem depender de tolerância
/// fina.
#[tokio::test]
async fn o_hsl_chega_ao_arquivo() {
    let dir = tempfile::tempdir().unwrap();
    let (origem, _) = foto_no_disco(&dir, "origem.png");

    let neutra = exportar(&origem.clone(), &dir.path().join("neutra.jpg")).await;

    let com_hsl = com_campos(
        origem,
        Campos {
            hsl_red_sat: Some(-100.0),
            ..Default::default()
        },
    );
    let alterada = exportar(&com_hsl, &dir.path().join("com-hsl.jpg")).await;

    assert_ne!(
        neutra.to_rgb8().into_raw(),
        alterada.to_rgb8().into_raw(),
        "tirar toda a saturação do vermelho não mudou o arquivo — \
         o HSL voltou a ser descartado na exportação"
    );
}

/// O arquivo é o que a tela mostra, pixel a pixel.
///
/// 🔑 **A referência é montada aqui pelo caminho da tela** — o mesmo `Motor` e a
/// mesma `transformacao` que a Revelação usa — e não por uma segunda conta. É
/// justamente ter duas contas para a mesma pergunta que produziu o defeito que
/// este teste fecha.
///
/// ⚠️ **A tolerância é do JPEG, e não do motor.** Qualidade 90 mexe em cada canal
/// alguns níveis; o defeito que isto pega é da ordem de dezenas. Uma comparação
/// exata falharia por compressão e esconderia a pergunta de verdade.
#[tokio::test]
async fn o_arquivo_e_o_que_a_tela_mostra() {
    let dir = tempfile::tempdir().unwrap();
    let (foto, original) = foto_no_disco(&dir, "origem.png");
    let campos = Campos {
        saturation: Some(-0.5),
        hsl_red_sat: Some(-80.0),
        crop_x: Some(0.1),
        crop_y: Some(0.1),
        crop_width: Some(0.8),
        crop_height: Some(0.8),
    };
    let foto = com_campos(foto, campos);

    let exportada = exportar(&foto, &dir.path().join("saida.jpg")).await;

    // A tela: o motor, e depois o enquadramento — nessa ordem.
    let mut motor = Motor::abrir().expect("nenhum adaptador de GPU");
    let rgba = original.to_rgba8();
    let (largura, altura) = (rgba.width(), rgba.height());
    let revelada = motor
        .revelar(
            &Arc::new(rgba.into_raw()),
            largura,
            altura,
            &campos.ajustes(),
        )
        .expect("o motor não devolveu imagem");
    let na_tela = transformacao::aplicar(&revelada, &transformacao::corte_da_entidade(&foto), true);

    assert_eq!(
        exportada.dimensions(),
        na_tela.dimensions(),
        "o arquivo e a tela discordam do enquadramento"
    );

    let (do_arquivo, da_tela) = (exportada.to_rgb8(), na_tela.to_rgb8());
    let pior = do_arquivo
        .as_raw()
        .iter()
        .zip(da_tela.as_raw())
        .map(|(a, b)| a.abs_diff(*b) as u32)
        .max()
        .expect("a imagem não é vazia");

    assert!(
        pior <= 12,
        "o arquivo diverge da tela em até {pior} níveis por canal — \
         o JPEG de qualidade 90 explica uns poucos, não isto"
    );
}
