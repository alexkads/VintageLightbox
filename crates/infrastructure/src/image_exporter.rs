//! A exportação: do arquivo original ao JPEG revelado.
//!
//! ## 🚨 O que este arquivo era, e por que mudou
//!
//! Ele tinha a **própria** implementação dos ajustes de revelação, na CPU, e ela
//! não era "o shader com menos campos" — era outra resposta para a mesma
//! pergunta. Medido em 17/ago/2026:
//!
//! | | a tela | o arquivo (antes) |
//! |---|---|---|
//! | quantos ajustes | 46 | **15** |
//! | ruído de luminância | bilateral, com `sigma_r` do valor | `img.blur(v * 0.02)`, gaussiano puro |
//! | ruído de cor | gaussiano no U/V | borra e restaura a luminância por razão |
//! | nitidez | máscara de desfoque **antes** dos tons | `imageops::unsharpen` **depois** deles |
//! | corte, giro, espelho | aplicados | **ignorados** |
//!
//! Quem revelava mexendo em HSL via o resultado na tela, exportava e recebia
//! outra imagem — sem erro, sem aviso. E o que faltava não era pouco: a curva de
//! tons inteira (4), o HSL inteiro nos 8 canais (24) e a lente (3).
//!
//! 🔑 **O conserto não foi acrescentar os 31 que faltavam.** Isso escreveria uma
//! terceira implementação da mesma matemática, para divergir de novo no próximo
//! ajuste. A exportação passa pelo **mesmo** `.wgsl` que desenha a tela
//! ([`crate::gpu_adjustments`]) e pelo **mesmo** enquadramento
//! ([`crate::transformacao`]) — um caminho, uma resposta.
//!
//! ⚠️ **E isso custa uma dependência nova: sem adaptador de GPU, não exporta.**
//! É deliberado. O caminho de CPU existia e dava outro resultado; mantê-lo como
//! reserva seria manter o defeito de pé, disfarçado de robustez. Falhar alto é o
//! que impede um lote inteiro de sair errado sem ninguém notar.

use async_trait::async_trait;
use domain::entities::Photo;
use domain::services::ImageExporter;
use domain::value_objects::FilePath;
use domain::{DomainError, DomainResult};
use image::DynamicImage;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::gpu_adjustments::{Ajustes, Motor};
use crate::transformacao;

/// O motor é aberto na primeira exportação e reaproveitado.
///
/// ⚠️ **Abrir custa dezenas de milissegundos** (`request_adapter` mais
/// `request_device`), e exportar uma seleção é uma chamada por foto: abrir um
/// motor por foto transformaria um custo fixo em custo por item.
#[derive(Default)]
pub struct ImageExporterImpl {
    motor: Mutex<Option<Arc<Mutex<Motor>>>>,
}

impl ImageExporterImpl {
    pub fn new() -> Self {
        Self::default()
    }

    fn motor(&self) -> DomainResult<Arc<Mutex<Motor>>> {
        let mut guarda = self
            .motor
            .lock()
            .map_err(|_| DomainError::InfrastructureError("motor de GPU envenenado".into()))?;

        if let Some(motor) = guarda.as_ref() {
            return Ok(motor.clone());
        }

        let motor = Motor::abrir().ok_or_else(|| {
            DomainError::InfrastructureError(
                "nenhum adaptador de GPU: a exportação usa o mesmo shader da tela, \
                 e sem ele o arquivo sairia diferente do que foi revelado"
                    .into(),
            )
        })?;

        let motor = Arc::new(Mutex::new(motor));
        *guarda = Some(motor.clone());
        Ok(motor)
    }

    /// Os 46 ajustes, no mesmo shader que desenha a Revelação.
    fn revelar(&self, imagem: &DynamicImage, ajustes: &Ajustes) -> DomainResult<DynamicImage> {
        // 🚨 RGBA de 8 bits é o que a textura de entrada espera
        // (`Rgba8Unorm`). Um `to_rgb8` aqui daria três canais para um formato de
        // quatro, e a foto sairia com as linhas deslocadas.
        let rgba = imagem.to_rgba8();
        let (largura, altura) = (rgba.width(), rgba.height());
        let pixels = Arc::new(rgba.into_raw());

        let motor = self.motor()?;
        let mut motor = motor
            .lock()
            .map_err(|_| DomainError::InfrastructureError("motor de GPU envenenado".into()))?;

        motor
            .revelar(&pixels, largura, altura, ajustes)
            .ok_or_else(|| {
                DomainError::InfrastructureError("a GPU não devolveu a imagem revelada".into())
            })
    }
}

#[async_trait]
impl ImageExporter for ImageExporterImpl {
    async fn export(&self, photo: &Photo, output_path: &FilePath) -> DomainResult<()> {
        let input_path = photo.file_path().as_str()?;

        let img = image::open(Path::new(&input_path)).map_err(|e| {
            DomainError::InfrastructureError(format!("Failed to open source image: {}", e))
        })?;

        // 🔑 A ordem é a da tela: o shader devolve a foto inteira, e o
        // enquadramento vem depois (`tela.rs` faz `transformacao::aplicar` sobre
        // o que o processador devolveu). Inverter daria uma vinheta centrada no
        // quadro cortado em vez de no original.
        let revelada = self.revelar(&img, &Ajustes::da_entidade(photo))?;
        let enquadrada =
            transformacao::aplicar(&revelada, &transformacao::corte_da_entidade(photo), true);

        let output_path_str = output_path.as_str()?;
        let rgb_img = enquadrada.to_rgb8();

        let file = std::fs::File::create(Path::new(output_path_str)).map_err(|e| {
            DomainError::InfrastructureError(format!("Failed to create output file: {}", e))
        })?;

        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, 90);
        encoder
            .encode(
                &rgb_img,
                rgb_img.width(),
                rgb_img.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| {
                DomainError::InfrastructureError(format!("Failed to encode JPEG: {}", e))
            })?;

        Ok(())
    }
}
