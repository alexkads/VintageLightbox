//! A folha de papel vira um arquivo.
//!
//! ## 🚨 O que este módulo substitui
//!
//! "Print" e "Export PDF" abriam um aviso de ***"coming soon"*** — nos dois apps.
//! Era o exemplo canônico do critério 5 do [objetivo](../../../docs/00-OBJETIVO.md):
//! um botão que anuncia o que não faz é pior que um botão ausente, porque ocupa
//! o lugar da funcionalidade e some do inventário mental de quem lê a tela.
//!
//! ## 🔑 Por que gerar PDF, e não falar com a impressora
//!
//! O PDF **é** o caminho de impressão, e não um desvio dele: no macOS, entregar
//! um PDF ao sistema abre o diálogo de impressão de verdade — com impressora,
//! bandeja, qualidade e "salvar como PDF" — que é a tela que quem imprime
//! conhece. Escrever um diálogo próprio seria construir uma versão pior de algo
//! que o sistema faz melhor, e que ainda por cima muda com a impressora.
//!
//! E o PDF é o que se manda para o laboratório, que é o outro destino real de
//! uma folha de contato.
//!
//! ## O corte deste arquivo
//!
//! [`posicoes`] decide **onde cada foto vai**, em milímetros, e é testável sem
//! escrever um byte de PDF. [`gerar`] pega essas posições e as escreve. O
//! defeito que dá para ter aqui é de coordenada, e ele mora todo na primeira.

use image::DynamicImage;
use printpdf::{Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, RawImage, XObjectTransform};

use super::pagina::{encaixar, Celula, Leiaute};

/// Onde uma foto vai parar no papel — já no sistema de coordenadas do PDF.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Posicao {
    /// Da **esquerda** do papel, em mm.
    pub x: f32,
    /// Da **base** do papel, em mm.
    ///
    /// 🚨 **É o eixo invertido em relação à tela.** [`Celula`] conta do topo,
    /// como todo leiaute de interface; o PDF conta da base. Trocar os dois não
    /// falha — ele imprime a folha de cabeça para baixo, e numa grade simétrica
    /// de fotos parecidas isso passa despercebido até o papel sair.
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
}

/// As posições de uma página, na ordem das fotos.
///
/// `aspectos` é largura ÷ altura de cada foto da página.
pub fn posicoes(leiaute: &Leiaute, aspectos: &[f32]) -> Vec<Posicao> {
    let (_, papel_altura) = leiaute.papel_mm();
    let celulas = leiaute.celulas();

    aspectos
        .iter()
        .zip(celulas.iter())
        .map(|(&aspecto, celula)| {
            let dentro = encaixar(celula, aspecto);
            do_topo_para_a_base(&dentro, papel_altura)
        })
        .collect()
}

fn do_topo_para_a_base(celula: &Celula, papel_altura: f32) -> Posicao {
    Posicao {
        x: celula.x,
        // O canto de baixo da foto, medido da base do papel.
        y: papel_altura - celula.y - celula.altura,
        largura: celula.largura,
        altura: celula.altura,
    }
}

/// A resolução com que as imagens entram no PDF.
///
/// ⚠️ **Não é a resolução do arquivo**: é a régua que converte pixel em
/// milímetro. A escala é calculada a partir dela para o tamanho pedido dar
/// exatamente o tamanho pedido — a qualidade quem decide é o pixel que a foto
/// tem.
const DPI: f32 = 300.0;

/// A folha inteira, em PDF.
///
/// ⚠️ **As fotos entram na ordem da coleção**, que é a ordem em que quem montou
/// a folha clicou — a mesma da prévia.
pub fn gerar(leiaute: &Leiaute, fotos: &[DynamicImage]) -> Result<Vec<u8>, String> {
    let (papel_largura, papel_altura) = leiaute.papel_mm();
    let por_pagina = leiaute.fotos_por_pagina().max(1);

    let mut documento = PdfDocument::new("VintageLightbox");
    let mut paginas = Vec::new();

    for bloco in fotos.chunks(por_pagina) {
        let aspectos: Vec<f32> = bloco
            .iter()
            .map(|foto| foto.width() as f32 / foto.height().max(1) as f32)
            .collect();
        let onde = posicoes(leiaute, &aspectos);

        let mut operacoes = Vec::new();
        for (foto, posicao) in bloco.iter().zip(onde.iter()) {
            if posicao.largura <= 0.0 || posicao.altura <= 0.0 {
                continue;
            }

            let rgb = foto.to_rgb8();
            let (px_largura, px_altura) = (rgb.width(), rgb.height());
            let imagem = RawImage {
                width: px_largura as usize,
                height: px_altura as usize,
                data_format: printpdf::RawImageFormat::RGB8,
                pixels: printpdf::RawImageData::U8(rgb.into_raw()),
                tag: Vec::new(),
            };
            let id = documento.add_image(&imagem);

            // O tamanho que a imagem teria sozinha, na régua acima.
            let natural_largura = px_largura as f32 / DPI * 25.4;
            let natural_altura = px_altura as f32 / DPI * 25.4;

            operacoes.push(Op::UseXobject {
                id,
                transform: XObjectTransform {
                    translate_x: Some(Mm(posicao.x).into()),
                    translate_y: Some(Mm(posicao.y).into()),
                    scale_x: Some(posicao.largura / natural_largura),
                    scale_y: Some(posicao.altura / natural_altura),
                    dpi: Some(DPI),
                    ..Default::default()
                },
            });
        }

        paginas.push(PdfPage::new(Mm(papel_largura), Mm(papel_altura), operacoes));
    }

    if paginas.is_empty() {
        return Err("nenhuma foto para imprimir".to_string());
    }

    let mut avisos = Vec::new();
    Ok(documento
        .with_pages(paginas)
        .save(&PdfSaveOptions::default(), &mut avisos))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::impressao::pagina::{Modelo, Orientacao, Papel};

    fn a4_com(modelo: Modelo) -> Leiaute {
        Leiaute {
            modelo,
            papel: Papel::A4,
            orientacao: Orientacao::Retrato,
            margem_mm: 10.0,
            espaco_mm: 0.0,
            colunas: 2,
            linhas: 2,
        }
    }

    /// A foto cabe dentro do papel.
    ///
    /// ⚠️ **Este teste NÃO pega o eixo invertido, e tentei.** Com uma foto só, a
    /// célula é centralizada e inverter o eixo devolve quase a mesma resposta —
    /// a quebra de propósito passou por ele. Quem pega é
    /// `a_primeira_foto_fica_acima_da_ultima`, que precisa de duas.
    ///
    /// 🔑 A lição é a mesma da marca d'água: um caso simétrico não distingue uma
    /// troca de sinal. O teste fica pelo que ele de fato prova.
    #[test]
    fn a_foto_cabe_no_papel() {
        let leiaute = a4_com(Modelo::Unica);
        let onde = posicoes(&leiaute, &[1.0]);

        assert_eq!(onde.len(), 1);
        let p = onde[0];
        let (largura_do_papel, altura_do_papel) = leiaute.papel_mm();

        assert!(p.y >= 0.0 && p.y + p.altura <= altura_do_papel);
        assert!(p.x >= 0.0 && p.x + p.largura <= largura_do_papel);
        assert!(p.largura > 0.0 && p.altura > 0.0);
    }

    /// 🔑 **A ordem das posições é a das fotos, de cima para baixo.**
    ///
    /// Numa grade 2×2, a primeira foto vai para o alto e a última para o pé. Se
    /// a conversão de eixo fosse feita depois da ordenação, a folha sairia com a
    /// sequência invertida — o que numa folha de contato numerada é o defeito
    /// que só aparece na conferência com o cliente.
    #[test]
    fn a_primeira_foto_fica_acima_da_ultima() {
        let leiaute = a4_com(Modelo::Grade2x2);
        let onde = posicoes(&leiaute, &[1.0, 1.0, 1.0, 1.0]);

        assert_eq!(onde.len(), 4);
        assert!(
            onde[0].y > onde[3].y,
            "a primeira foto tinha de ficar acima da última no papel"
        );
    }

    /// A foto fica dentro da célula dela, com o respiro da moldura.
    #[test]
    fn a_foto_nao_passa_da_margem() {
        let leiaute = a4_com(Modelo::Grade2x2);
        let (largura_do_papel, altura_do_papel) = leiaute.papel_mm();

        for p in posicoes(&leiaute, &[1.5, 1.5, 0.7, 0.7]) {
            assert!(p.x >= leiaute.margem_mm - 0.01, "passou da margem esquerda");
            assert!(
                p.x + p.largura <= largura_do_papel - leiaute.margem_mm + 0.01,
                "passou da margem direita"
            );
            assert!(p.y >= leiaute.margem_mm - 0.01, "passou da margem de baixo");
            assert!(
                p.y + p.altura <= altura_do_papel - leiaute.margem_mm + 0.01,
                "passou da margem de cima"
            );
        }
    }

    fn foto(largura: u32, altura: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::new(largura, altura))
    }

    /// ✅ **Sai um PDF de verdade, com uma página por bloco de fotos.**
    #[test]
    fn gera_um_pdf_com_as_paginas_certas() {
        let leiaute = a4_com(Modelo::Grade2x2);
        // Cinco fotos numa grade de quatro: duas páginas.
        let fotos: Vec<_> = (0..5).map(|_| foto(40, 30)).collect();

        let bytes = gerar(&leiaute, &fotos).expect("gerar o PDF");

        assert!(
            bytes.starts_with(b"%PDF"),
            "não é um PDF: começa com {:?}",
            &bytes[..8.min(bytes.len())]
        );
        assert!(bytes.len() > 1000, "PDF pequeno demais para ter imagem");

        let texto = String::from_utf8_lossy(&bytes);
        let paginas =
            texto.matches("/Type /Page\n").count() + texto.matches("/Type/Page\n").count();
        assert!(
            paginas >= 2 || texto.contains("/Count 2"),
            "esperava duas páginas para cinco fotos numa grade de quatro"
        );
    }

    /// ⚠️ **Sem foto não sai arquivo.** Um PDF de zero páginas é um arquivo que
    /// não abre em leitor nenhum, e ele apareceria no disco como se a exportação
    /// tivesse dado certo.
    #[test]
    fn sem_foto_nao_gera_arquivo() {
        assert!(gerar(&a4_com(Modelo::Grade2x2), &[]).is_err());
    }
}
