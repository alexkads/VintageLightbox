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
use domain::value_objects::{ExportOptions, FilePath, Watermark, WatermarkPosition};
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
    exportar_com(foto, destino, &ExportOptions::default()).await
}

async fn exportar_com(
    foto: &Photo,
    destino: &std::path::Path,
    opcoes: &ExportOptions,
) -> DynamicImage {
    let exportador = ImageExporterImpl::new();
    exportador
        .export(
            foto,
            &FilePath::new(destino.to_str().unwrap()).unwrap(),
            opcoes,
        )
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

/// Um PNG de marca d'água: um quadrado vermelho opaco no meio de transparência.
///
/// O contorno transparente é o ponto: é ele que separa "compor a marca" de
/// "colar um retângulo por cima da foto".
fn marca_no_disco(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let mut logo = image::RgbaImage::new(32, 32);
    for (x, y, pixel) in logo.enumerate_pixels_mut() {
        let dentro = (8..24).contains(&x) && (8..24).contains(&y);
        *pixel = if dentro {
            image::Rgba([255, 0, 0, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        };
    }
    let caminho = dir.path().join("marca.png");
    logo.save(&caminho).expect("gravar a marca");
    caminho
}

/// 🚨 **A marca d'água chega ao arquivo.**
///
/// É a funcionalidade que separa *entregar* de *mostrar*: a foto que o cliente
/// comprou vai inteira, a que ficou para trás vai marcada. Sem este caminho não
/// existe upsell — existe distribuição.
#[tokio::test]
async fn a_marca_dagua_chega_ao_arquivo() {
    let dir = tempfile::tempdir().unwrap();
    let (foto, _) = foto_no_disco(&dir, "origem.png");
    let marca = marca_no_disco(&dir);

    let sem = exportar(&foto, &dir.path().join("sem.jpg")).await;
    let com = exportar_com(
        &foto,
        &dir.path().join("com.jpg"),
        // ⚠️ Opacidade **abaixo de 1.0** de propósito: com 1.0 o código nem entra
        // no ajuste de alfa, e o teste passaria sem nunca exercitá-lo. Foi assim
        // que uma quebra de propósito não falhou.
        &ExportOptions::default().with_watermark(Watermark::new(
            FilePath::new(marca.to_str().unwrap()).unwrap(),
            WatermarkPosition::Center,
            0.5,
            0.5,
        )),
    )
    .await;

    assert_eq!(
        sem.dimensions(),
        com.dimensions(),
        "a marca não pode mudar o tamanho da foto"
    );

    let (a, b) = (sem.to_rgb8(), com.to_rgb8());
    let centro_mudou = a.get_pixel(32, 32) != b.get_pixel(32, 32);
    assert!(centro_mudou, "o centro não recebeu a marca");

    // 🔑 **E o canto tem de continuar igual.** Duas coisas dependem disto, e as
    // duas somem em silêncio: a composição respeitar o alfa do PNG (senão a
    // marca chega como um retângulo opaco sobre a foto), e a opacidade
    // **multiplicar** o alfa em vez de substituí-lo (senão o contorno
    // transparente vira um véu por cima de tudo). O teste do centro passaria
    // nos dois casos.
    // A marca é 32×32 centrada numa foto 64×64, então ela ocupa de (16,16) a
    // (48,48), com o vermelho no miolo. O pixel (18,18) está **dentro** do
    // retângulo da marca e **fora** do desenho dela.
    //
    // ⚠️ Testar o canto (2,2) não serve, e essa foi a primeira versão: ele fica
    // fora do retângulo, então prova só que a composição não vaza. As duas
    // quebras de propósito passaram por ele.
    assert_eq!(
        a.get_pixel(18, 18),
        b.get_pixel(18, 18),
        "o contorno transparente da marca alterou a foto — o alfa foi ignorado"
    );
}

/// ⚠️ **Sem marca d'água legível a exportação falha, em vez de sair limpa.**
///
/// É a única falha do exportador que não é técnica. Um logotipo apagado, movido
/// ou com o caminho errado produziria — em silêncio — exatamente o arquivo que
/// não pode existir: a foto não comprada, legível, na galeria.
#[tokio::test]
async fn marca_dagua_ilegivel_derruba_a_exportacao_em_vez_de_sair_sem_ela() {
    let dir = tempfile::tempdir().unwrap();
    let (foto, _) = foto_no_disco(&dir, "origem.png");

    let exportador = ImageExporterImpl::new();
    let destino = dir.path().join("saida.jpg");
    let resultado = exportador
        .export(
            &foto,
            &FilePath::new(destino.to_str().unwrap()).unwrap(),
            &ExportOptions::default().with_watermark(Watermark::new(
                FilePath::new("/nao/existe/marca.png").unwrap(),
                WatermarkPosition::Center,
                0.5,
                1.0,
            )),
        )
        .await;

    assert!(
        resultado.is_err(),
        "devia falhar, e não exportar sem a marca"
    );
    assert!(
        !destino.exists(),
        "o arquivo sem marca não pode existir nem por um instante"
    );
}

/// ⚠️ **Reduzir limita o lado maior e mantém a proporção.**
#[tokio::test]
async fn reduzir_limita_o_lado_maior() {
    let dir = tempfile::tempdir().unwrap();
    let (foto, _) = foto_no_disco(&dir, "origem.png");

    let reduzida = exportar_com(
        &foto,
        &dir.path().join("reduzida.jpg"),
        &ExportOptions::default().with_longest_edge(32),
    )
    .await;

    assert_eq!(reduzida.dimensions(), (32, 32), "a foto é 64×64 quadrada");
}

/// 🚨 **Reduzir nunca amplia.**
///
/// Pedir 2048 px numa foto de 64 devolveria 2048 px de nada: o mesmo detalhe
/// espalhado, com arquivo maior e nitidez menor. É o "Don't Enlarge" do
/// Lightroom, e aqui não é opção — é o comportamento.
#[tokio::test]
async fn reduzir_nunca_amplia() {
    let dir = tempfile::tempdir().unwrap();
    let (foto, _) = foto_no_disco(&dir, "origem.png");

    let pedida_maior = exportar_com(
        &foto,
        &dir.path().join("maior.jpg"),
        &ExportOptions::default().with_longest_edge(2048),
    )
    .await;

    assert_eq!(pedida_maior.dimensions(), (64, 64));
}
