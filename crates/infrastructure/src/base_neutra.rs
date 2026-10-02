//! A base neutra: o bruto decodificado, de pé, sem nenhum efeito — a imagem que
//! o motor de revelação recebe e a base do editor em camadas (C28,
//! `docs/editor-em-camadas/02-CONTRATO.md`).
//!
//! 🚨 **Uma função só para as duas perguntas.** A prévia de um RAW local sai da
//! LibRaw (`thumbnail_generator.rs`), mas a exportação local e o zoom em
//! resolução cheia abriam o mesmo arquivo pelo `image::ImageReader`, que não
//! decodifica RAW: o editor pintaria sobre uma imagem e a Revelação revelaria
//! outra. Daqui em diante os três passam por aqui.
//!
//! | Origem | Decodificação | Cor | Bits | Dimensões |
//! |---|---|---|---|---|
//! | RAW (`raw_processing::is_raw_file`) | LibRaw, parâmetros de saída padrão (sRGB, curva BT.709 0,45/4,5, luz do dia da câmera, clareamento automático) | sRGB | 8 | a imagem processada inteira, já de pé (a LibRaw aplica o `flip`) |
//! | JPEG/WebP/PNG, e os bytes do bruto do site | `image` + etiquetas EXIF e ICC (`orientacao`) | **o espaço que o arquivo declara**, convertido para sRGB (`foto_codec::espaco_de_cor`) — Adobe RGB nas fotos da câmera | 8 | as do arquivo, de pé |
//!
//! 🚨 **Até 2/out/2026 o JPEG era lido como sRGB, com etiqueta ou sem.** As
//! câmeras do estúdio gravam Adobe RGB (o `R03` do EXIF), e toda foto saía
//! mais apagada e mais fria que no Lightroom (ΔE2000 3,08 no neutro; lida
//! certo, 1,27). O dono decidiu uma leitura só, a do arquivo
//! (`docs/REGUA-DO-LIGHTROOM.md`, seção 10). ⚠️ O `RecordarFotos P&B` vinha do
//! darktable lendo sRGB (o `colorin` do `.dtstyle` fixa sRGB): sobre a leitura
//! certa, os pretos dele descem 6 a 8 níveis até ele ser refeito com os
//! controles do motor.

use std::path::Path;

use image::DynamicImage;

use crate::orientacao;
use crate::raw_processing;

/// A base neutra de um arquivo do disco.
pub fn base_neutra(caminho: &Path) -> Result<DynamicImage, String> {
    let texto = caminho.to_string_lossy();
    if raw_processing::is_raw_file(&texto) {
        return raw_processing::load_raw_as_dynamic_image(&texto);
    }
    orientacao::abrir_de_pe(caminho).map_err(|e| e.to_string())
}

/// A base neutra dos bytes de um bruto (o do site, que chega pela rede).
///
/// ⚠️ Sem nome de arquivo não há extensão: o formato é adivinhado pelo
/// conteúdo, e o que o `image` não reconhece é tentado como RAW.
pub fn base_neutra_de_bytes(bytes: &[u8]) -> Result<DynamicImage, String> {
    match orientacao::decodificar_de_pe(bytes) {
        Ok(imagem) => Ok(imagem),
        Err(erro_do_image) => raw_processing::load_raw_from_bytes(bytes)
            .map_err(|erro_do_raw| format!("{erro_do_image}; como RAW: {erro_do_raw}")),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_jpeg_sai_de_pe_pelos_dois_caminhos() {
        let dir = tempfile::tempdir().unwrap();
        let caminho = dir.path().join("f.png");
        let imagem = DynamicImage::ImageRgb8(image::RgbImage::from_fn(6, 4, |x, y| {
            image::Rgb([x as u8 * 10, y as u8 * 10, 7])
        }));
        imagem.save(&caminho).unwrap();
        let do_disco = base_neutra(&caminho).unwrap().to_rgb8();
        let dos_bytes = base_neutra_de_bytes(&std::fs::read(&caminho).unwrap())
            .unwrap()
            .to_rgb8();
        assert_eq!(do_disco.as_raw(), imagem.to_rgb8().as_raw());
        assert_eq!(dos_bytes.as_raw(), do_disco.as_raw());
    }

    #[test]
    fn bytes_que_nao_sao_foto_dao_erro_e_nao_panico() {
        assert!(base_neutra_de_bytes(b"nada").is_err());
    }
}
