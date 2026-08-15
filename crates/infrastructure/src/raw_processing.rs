use async_trait::async_trait;
use domain::{
    services::{RawDecoder, RawImage},
    value_objects::FilePath,
    DomainError, DomainResult,
};

/// Implementação do RawDecoder usando a crate `rawloader`
pub struct RawDecoderImpl;

impl RawDecoderImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RawDecoderImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RawDecoder for RawDecoderImpl {
    fn decode(&self, path: &FilePath) -> DomainResult<RawImage> {
        let path_str = path
            .as_ref()
            .to_str()
            .ok_or_else(|| DomainError::InvalidOperation("Invalid path encoding".to_string()))?;

        // rawloader::decode_file returns Result<RawImage, RAWError>
        let raw_image = rawloader::decode_file(path_str).map_err(|e| {
            DomainError::InfrastructureError(format!("Failed to decode RAW file: {}", e))
        })?;

        // Convert rawloader::RawImage to domain::RawImage
        let data = match raw_image.data {
            rawloader::RawImageData::Integer(v) => v,
            _ => {
                return Err(DomainError::InfrastructureError(
                    "Unsupported RAW data format".to_string(),
                ))
            }
        };

        Ok(RawImage {
            width: raw_image.width,
            height: raw_image.height,
            data,
            cpp: raw_image.cpp,
        })
    }
}

/// Verifica se um arquivo é RAW baseado na extensão
pub fn is_raw_file(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    matches!(
        ext.as_deref(),
        Some(
            "nef"
                | "cr2"
                | "cr3"
                | "arw"
                | "dng"
                | "orf"
                | "raw"
                | "rw2"
                | "raf"
                | "pef"
                | "srw"
                | "x3f"
        )
    )
}

/// Carrega um arquivo RAW e converte para DynamicImage RGB
///
/// Esta função usa LibRaw (via rsraw) que já faz:
/// 1. Demosaic de alta qualidade
/// 2. White balance automático
/// 3. Color correction
/// 4. Retorna DynamicImage::ImageRgb8
pub fn load_raw_as_dynamic_image(path: &str) -> Result<image::DynamicImage, String> {
    use rsraw::{RawImage, BIT_DEPTH_8};

    // 1. Read file to buffer
    let file_data = std::fs::read(path).map_err(|e| format!("Failed to read RAW file: {}", e))?;

    // 2. Open RAW image with LibRaw
    let mut raw =
        RawImage::open(&file_data).map_err(|e| format!("LibRaw failed to open: {:?}", e))?;

    // 3. Unpack the raw data
    raw.unpack()
        .map_err(|e| format!("LibRaw unpack failed: {:?}", e))?;

    // 4. Get dimensions before processing
    let width = raw.width();
    let height = raw.height();

    // 5. Process with LibRaw (demosaic, white balance, color correction)
    let processed = raw
        .process::<BIT_DEPTH_8>()
        .map_err(|e| format!("LibRaw process failed: {:?}", e))?;

    // 6. Get RGB data from processed image
    // ProcessedImage<8> implements Deref<Target=[u8]>
    let rgb_data: Vec<u8> = processed.to_vec();

    // 7. Create RGB image
    // LibRaw returns RGB data, 3 bytes per pixel
    let img_buffer = image::RgbImage::from_raw(width, height, rgb_data)
        .ok_or_else(|| "Failed to create image buffer from LibRaw data".to_string())?;

    Ok(image::DynamicImage::ImageRgb8(img_buffer))
}

/// Extrai o preview JPEG embutido no arquivo RAW (muito mais rápido que raw decoding)
///
/// Câmeras gravam mais de um preview no mesmo arquivo (de 160px a resolução cheia).
/// `min_height` diz o menor tamanho que serve para quem chamou: devolve o **menor**
/// preview que ainda atende, e só cai no maior disponível se nenhum atender. Pegar
/// sempre o maior jogaria fora o ganho de velocidade — decodificar um JPEG 6000×4000
/// para gerar um thumbnail de 320px custa mais do que o preview certo.
///
/// Só previews em JPEG são devolvidos: os outros formatos do LibRaw (bitmap cru,
/// H.265) não passam por `image::load_from_memory`, que é o que o chamador faz.
///
/// Não chama `unpack()` de propósito — o preview embutido sai direto do arquivo,
/// sem demosaic. É esse o caminho rápido.
pub fn extract_embedded_preview(path: &str, min_height: u32) -> Option<Vec<u8>> {
    use rsraw::{RawImage, ThumbFormat};

    let file_data = std::fs::read(path).ok()?;
    let mut raw = RawImage::open(&file_data).ok()?;

    let mut jpegs: Vec<_> = raw
        .extract_thumbs()
        .ok()?
        .into_iter()
        .filter(|t| t.format == ThumbFormat::Jpeg && !t.data.is_empty())
        .collect();

    if jpegs.is_empty() {
        return None;
    }

    // `extract_thumbs` já devolve ordenado por altura crescente, mas o filtro acima
    // não garante ordem se a fonte mudar — ordenar aqui é barato e torna a escolha
    // independente disso.
    jpegs.sort_by_key(|t| t.height);

    let escolhido = jpegs
        .iter()
        .position(|t| t.height >= min_height)
        .unwrap_or(jpegs.len() - 1);

    Some(jpegs.swap_remove(escolhido).data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_decode_invalid_file() {
        let decoder = RawDecoderImpl::new();
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Not a RAW file").unwrap();

        let path = FilePath::new(file.path().to_str().unwrap()).unwrap();
        let result = decoder.decode(&path);

        // `assert!(true)` no braço certo e `assert!(false)` no outro diziam a
        // coisa certa de um jeito que o compilador não conferia. `matches!`
        // afirma o mesmo e ainda mostra o que veio quando falha.
        //
        // Sobre o `.err()`: `RawImage` não implementa `Debug`, então o
        // `Result` inteiro não é formatável — e o caso de sucesso não tem o que
        // relatar aqui além de "não deveria ter dado certo".
        let erro = result.err();
        assert!(
            matches!(erro, Some(DomainError::InfrastructureError(_))),
            "Expected InfrastructureError, got {erro:?}"
        );
    }

    // Note: Testing successful decoding requires a real RAW file,
    // which we don't have in the repo. This would be covered by integration tests
    // with test assets in a real environment.
}
