//! Redimensionar e converter para WebP — **antes de subir**.
//!
//! É o par nativo de `backup/imagem.ts` do site, com **as mesmas três regras**,
//! porque o mesmo arquivo arrastado nas duas interfaces tem de virar o mesmo
//! objeto no R2 (FLUXO_UNICO_DAS_TRES_INTERFACES: a web é a referência):
//!
//! 1. **Nunca aumenta.** Uma miniatura de 300px não vira 2560.
//! 2. **Converte só o que dá para decodificar.** Num estúdio, metade do que se
//!    arrasta é RAW; aqui o `image` até abre alguns, mas o site não — e um
//!    acervo em que o `.NEF` tem WebP ao lado numa interface e não na outra é
//!    divergência, que é o que este projeto não pode ter.
//! 3. **Se o WebP não for menor, ele não existe.**
//!
//! 🔑 **A conta do tamanho é por `maior lado`**, e não por uma caixa
//! largura×altura: a caixa `1920×1080` trata a foto em pé pior que a deitada —
//! uma de 3000×4000 viraria 810×1080, metade da resolução da mesma foto
//! deitada. É o mesmo cuidado que o corte da tira já cobrou uma vez.

use std::path::Path;

use image::{DynamicImage, ImageFormat};

/// O que fazer com as imagens de um envio — o espelho de `Conversao` do site.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conversao {
    /// O maior lado, em pixels. `0` mantém o tamanho.
    pub maior_lado: u32,
    /// De 0 a 1.
    pub qualidade: f32,
}

impl Default for Conversao {
    fn default() -> Self {
        Self {
            maior_lado: 2560,
            qualidade: 0.85,
        }
    }
}

/// As extensões que o **site** converte, porque é ele quem manda.
///
/// Lista fechada, e pela extensão e não pelo tipo MIME: aqui não há navegador
/// para adivinhar tipo, e o arrasto traz caminho de arquivo.
const CONVERSIVEIS: [&str; 6] = ["jpg", "jpeg", "png", "gif", "bmp", "webp"];

/// Se vale a pena tentar converter este arquivo.
pub fn eh_conversivel(caminho: &Path) -> bool {
    caminho
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| CONVERSIVEIS.contains(&e.as_str()))
}

/// As medidas que cabem na caixa — **sem nunca aumentar**.
pub fn medidas_que_cabem(largura: u32, altura: u32, maior_lado: u32) -> (u32, u32) {
    let maior = largura.max(altura);
    if maior_lado == 0 || maior == 0 || maior <= maior_lado {
        return (largura, altura);
    }
    let escala = f64::from(maior_lado) / f64::from(maior);
    (
        // Nenhum lado chega a zero: uma faixa de 4000×1 encolhida daria altura
        // 0, e uma imagem de altura 0 não existe.
        ((f64::from(largura) * escala).round() as u32).max(1),
        ((f64::from(altura) * escala).round() as u32).max(1),
    )
}

/// O nome do irmão convertido: `DSC_01.JPG` → `DSC_01.webp`.
///
/// 🔑 **Irmão, e não substituto** — os dois ficam guardados (decisão do dono,
/// 2026-09-18).
pub fn nome_convertido(nome: &str) -> String {
    match nome.rfind('.') {
        Some(ponto) if ponto > 0 => format!("{}.webp", &nome[..ponto]),
        _ => format!("{nome}.webp"),
    }
}

/// O que a conversão produziu.
#[derive(Debug, Clone, PartialEq)]
pub struct Convertido {
    pub nome: String,
    pub bytes: Vec<u8>,
}

/// Converte, ou devolve `None` quando não vale a pena (ou não dá).
///
/// `None` é resposta normal, e não falha: RAW, PDF, um PNG pequeno que ficaria
/// maior. Quem chama sobe só o original.
pub fn converter_para_webp(nome: &str, bruto: &[u8], conversao: Conversao) -> Option<Convertido> {
    if !eh_conversivel(Path::new(nome)) {
        return None;
    }
    // De pé: o WebP não leva a etiqueta de girar do EXIF — sem aplicá-la, a
    // foto em retrato ficaria deitada no backup (`infrastructure::orientacao`).
    let imagem = infrastructure::orientacao::decodificar_de_pe(bruto).ok()?;
    let (largura, altura) =
        medidas_que_cabem(imagem.width(), imagem.height(), conversao.maior_lado);

    let ajustada = if (largura, altura) == (imagem.width(), imagem.height()) {
        imagem
    } else {
        // `Lanczos3`: a redução grande com filtro barato serrilha cabelo e
        // tecido, que é justamente o que um estúdio de retrato olha.
        imagem.resize_exact(largura, altura, image::imageops::FilterType::Lanczos3)
    };

    let bytes = codificar_webp(&ajustada)?;
    // A regra 3.
    if bytes.len() >= bruto.len() {
        return None;
    }
    Some(Convertido {
        nome: nome_convertido(nome),
        bytes,
    })
}

/// WebP pelo `image`.
///
/// ⚠️ **O encoder do `image` só faz WebP sem perda**, e por isso a qualidade
/// pedida não entra na conta aqui — ela existe na [`Conversao`] porque o site a
/// usa, e tirá-la faria as duas telas terem opções diferentes. O tamanho ainda
/// cai pelo redimensionamento, e a regra 3 descarta o que não encolheu; quando
/// a versão com perda existir no `image`, é este o ponto que muda.
fn codificar_webp(imagem: &DynamicImage) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    imagem
        .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::WebP)
        .ok()?;
    (!bytes.is_empty()).then_some(bytes)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nunca_aumenta() {
        assert_eq!(medidas_que_cabem(300, 200, 2560), (300, 200));
    }

    /// 🔑 A foto em pé e a deitada perdem o mesmo tanto — a conta é por maior
    /// lado, e não por uma caixa.
    #[test]
    fn o_retrato_e_a_paisagem_perdem_o_mesmo() {
        assert_eq!(medidas_que_cabem(4000, 3000, 2000), (2000, 1500));
        assert_eq!(medidas_que_cabem(3000, 4000, 2000), (1500, 2000));
    }

    #[test]
    fn maior_lado_zero_mantem_o_tamanho() {
        assert_eq!(medidas_que_cabem(4000, 3000, 0), (4000, 3000));
    }

    #[test]
    fn nenhum_lado_chega_a_zero() {
        assert_eq!(medidas_que_cabem(4000, 1, 100).1, 1);
    }

    #[test]
    fn o_convertido_e_irmao_do_original() {
        assert_eq!(nome_convertido("DSC_01.NEF"), "DSC_01.webp");
        assert_eq!(nome_convertido("sem-extensao"), "sem-extensao.webp");
        assert_eq!(nome_convertido(".oculto"), ".oculto.webp");
    }

    /// 🚨 A lista é a do **site**: o RAW não vira WebP em interface nenhuma, ou
    /// o mesmo arrasto produziria acervos diferentes.
    #[test]
    fn o_raw_nao_e_conversivel() {
        for raw in ["DSC_01.NEF", "IMG.CR2", "a.ARW", "b.dng", "c.pdf"] {
            assert!(!eh_conversivel(Path::new(raw)), "{raw}");
        }
        for imagem in ["a.JPG", "b.jpeg", "c.png", "d.WebP"] {
            assert!(eh_conversivel(Path::new(imagem)), "{imagem}");
        }
    }

    #[test]
    fn o_que_nao_e_imagem_nao_vira_webp() {
        assert!(converter_para_webp("nota.pdf", b"%PDF-1.4", Conversao::default()).is_none());
    }

    /// Uma foto de verdade encolhe e vira WebP com o nome irmão.
    #[test]
    fn a_foto_grande_encolhe_e_vira_webp() {
        let grande = DynamicImage::ImageRgb8(image::RgbImage::from_fn(1200, 900, |x, y| {
            // Ruído suave: uma imagem chapada comprime tanto que a regra 3
            // descartaria o convertido e o teste não provaria nada.
            image::Rgb([(x % 251) as u8, (y % 253) as u8, ((x + y) % 241) as u8])
        }));
        let mut bruto = Vec::new();
        grande
            .write_to(&mut std::io::Cursor::new(&mut bruto), ImageFormat::Png)
            .expect("png de teste");

        let convertido = converter_para_webp(
            "ensaio.png",
            &bruto,
            Conversao {
                maior_lado: 300,
                qualidade: 0.85,
            },
        )
        .expect("deveria converter");

        assert_eq!(convertido.nome, "ensaio.webp");
        assert!(convertido.bytes.len() < bruto.len());
        let lida = image::load_from_memory(&convertido.bytes).expect("webp legível");
        assert_eq!((lida.width(), lida.height()), (300, 225));
    }

    /// 🚨 **A regra 3, no caso que ela existe para pegar**: um WebP que já está
    /// no tamanho não ganha um irmão WebP do mesmo tamanho.
    ///
    /// Sem ela, reenviar uma pasta que já foi convertida dobraria o acervo com
    /// cópias byte a byte — e o R2 é cobrado por byte guardado.
    #[test]
    fn o_convertido_que_nao_encolhe_nao_existe() {
        let imagem = DynamicImage::ImageRgb8(image::RgbImage::from_fn(200, 150, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 253) as u8, ((x + y) % 241) as u8])
        }));
        let mut ja_webp = Vec::new();
        imagem
            .write_to(&mut std::io::Cursor::new(&mut ja_webp), ImageFormat::WebP)
            .expect("webp de teste");

        // Sem redimensionar: o que sai é do mesmo tamanho do que entrou.
        let conversao = Conversao {
            maior_lado: 0,
            qualidade: 0.85,
        };
        assert!(converter_para_webp("capa.webp", &ja_webp, conversao).is_none());
    }
}
