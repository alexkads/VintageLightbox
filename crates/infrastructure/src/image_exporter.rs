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
use domain::value_objects::{CropSettings, ExportOptions, FilePath, Watermark, WatermarkPosition};
use domain::{DomainError, DomainResult};
use image::DynamicImage;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::gpu_adjustments::{ajustes_da_entidade, Ajustes, Motor};
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

    /// Revela **bytes de imagem** — a foto que não está no catálogo desta máquina.
    ///
    /// 🔑 É o `revelarIntegral` do editor do site: o original chega inteiro, os
    /// ajustes e o enquadramento vêm da tela, e o que sai é o JPEG que vai para
    /// a galeria. Sem `Photo` porque não há: a foto do storage não é do
    /// catálogo local, e forçá-la a virar uma entidade só para revelar criaria
    /// um registro que ninguém pediu.
    ///
    /// A ordem é a de [`Self::renderizar`] — revelar inteiro, enquadrar depois.
    pub fn renderizar_bytes(
        &self,
        bytes: &[u8],
        ajustes: &Ajustes,
        corte: &CropSettings,
        qualidade: u8,
    ) -> DomainResult<Vec<u8>> {
        let imagem = image::load_from_memory(bytes)
            .map_err(|e| DomainError::InfrastructureError(format!("o original não abriu: {e}")))?;
        let revelada = self.revelar(&imagem, ajustes)?;
        let saida = transformacao::aplicar(&revelada, corte, true);
        revelacao_core::jpeg::codificar(&saida, qualidade)
            .map_err(|e| DomainError::InfrastructureError(format!("o JPEG não saiu: {e}")))
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

/// Limita o lado maior, **sem nunca ampliar**.
///
/// ⚠️ **Ampliar é a resposta errada para toda pergunta que a galeria faz.** Pedir
/// 2048 px numa foto de 1200 devolveria 2048 px de nada — o mesmo detalhe
/// espalhado, com arquivo maior e nitidez menor. É o "Don't Enlarge" do
/// Lightroom, e aqui ele não é opção: é o comportamento.
fn redimensionar(imagem: DynamicImage, lado_maior: u32) -> DynamicImage {
    let (largura, altura) = (imagem.width(), imagem.height());
    let maior = largura.max(altura);
    if maior <= lado_maior {
        return imagem;
    }

    let fator = lado_maior as f32 / maior as f32;
    let nova_largura = ((largura as f32 * fator).round() as u32).max(1);
    let nova_altura = ((altura as f32 * fator).round() as u32).max(1);

    // Lanczos3 porque o destino é ver: reduzir com filtro rápido devolve serrilha
    // nas bordas, e a foto da galeria é justamente a que vai ser olhada de perto
    // por quem está decidindo se compra.
    imagem.resize_exact(
        nova_largura,
        nova_altura,
        image::imageops::FilterType::Lanczos3,
    )
}

/// Compõe a marca d'água por cima da foto.
///
/// 🚨 **Se a marca não puder ser aplicada, a exportação falha.** É a única falha
/// deste arquivo que não é técnica: exportar sem a marca uma foto que devia ir
/// marcada **entrega a foto que o cliente não comprou**. Um arquivo a menos é um
/// reexport; um arquivo sem marca na galeria não volta atrás.
fn aplicar_marca(base: &DynamicImage, marca: &Watermark) -> DomainResult<DynamicImage> {
    let caminho = marca.file().as_str()?;
    let logo = image::open(Path::new(&caminho)).map_err(|e| {
        DomainError::InfrastructureError(format!(
            "não foi possível abrir a marca d'água ({caminho}): {e}"
        ))
    })?;

    let (largura, altura) = (base.width(), base.height());
    let alvo_largura = ((largura as f32 * marca.scale()).round() as u32).max(1);
    let fator = alvo_largura as f32 / logo.width().max(1) as f32;
    let alvo_altura = ((logo.height() as f32 * fator).round() as u32).max(1);

    let logo = logo.resize_exact(
        alvo_largura.min(largura),
        alvo_altura.min(altura),
        image::imageops::FilterType::Lanczos3,
    );

    let margem = (largura.min(altura) as f32 * marca.margin()).round() as i64;
    let (lw, lh) = (logo.width() as i64, logo.height() as i64);
    let (bw, bh) = (largura as i64, altura as i64);
    let (x, y) = match marca.position() {
        WatermarkPosition::Center => ((bw - lw) / 2, (bh - lh) / 2),
        WatermarkPosition::TopLeft => (margem, margem),
        WatermarkPosition::TopRight => (bw - lw - margem, margem),
        WatermarkPosition::BottomLeft => (margem, bh - lh - margem),
        WatermarkPosition::BottomRight => (bw - lw - margem, bh - lh - margem),
    };

    // 🔑 A opacidade multiplica o alfa **que a marca já tem**, e não substitui.
    // Substituir faria o retângulo transparente em volta do logotipo virar um
    // véu cinza sobre a foto — o PNG tem alfa por um motivo.
    let mut logo = logo.to_rgba8();
    if marca.opacity() < 1.0 {
        for pixel in logo.pixels_mut() {
            pixel[3] = (pixel[3] as f32 * marca.opacity()).round() as u8;
        }
    }

    let mut saida = base.to_rgba8();
    image::imageops::overlay(&mut saida, &logo, x, y);
    Ok(DynamicImage::ImageRgba8(saida))
}

impl ImageExporterImpl {
    /// A foto pronta, **em memória** — revelada, enquadrada e com as opções
    /// aplicadas, sem passar pelo disco.
    ///
    /// 🔑 **Existe para a impressão usar o mesmo caminho.** A folha de papel
    /// precisa da foto do jeito que ela ficou, e não do arquivo cru: imprimir o
    /// original seria o mesmo defeito que a exportação tinha até hoje de manhã —
    /// a tela mostrando uma coisa e o resultado sendo outra.
    pub fn renderizar(&self, photo: &Photo, options: &ExportOptions) -> DomainResult<DynamicImage> {
        let input_path = photo.file_path().as_str()?;

        let img = image::open(Path::new(&input_path)).map_err(|e| {
            DomainError::InfrastructureError(format!("Failed to open source image: {}", e))
        })?;

        // 🔑 A ordem é a da tela: o shader devolve a foto inteira, e o
        // enquadramento vem depois (`tela.rs` faz `transformacao::aplicar` sobre
        // o que o processador devolveu). Inverter daria uma vinheta centrada no
        // quadro cortado em vez de no original.
        let revelada = self.revelar(&img, &ajustes_da_entidade(photo))?;
        let mut saida =
            transformacao::aplicar(&revelada, &transformacao::corte_da_entidade(photo), true);

        // ⚠️ Redimensionar **antes** da marca, e as duas coisas dependem disso:
        // reduzir depois reamostraria a marca junto (ela sai borrada, e é o
        // elemento mais fino da imagem), e o tamanho dela é uma fração do que se
        // vai ver — não do que se revelou.
        if let Some(lado_maior) = options.longest_edge() {
            saida = redimensionar(saida, lado_maior);
        }

        if let Some(marca) = options.watermark() {
            saida = aplicar_marca(&saida, marca)?;
        }

        Ok(saida)
    }
}

#[async_trait]
impl ImageExporter for ImageExporterImpl {
    async fn renderizar_jpeg(
        &self,
        photo: &Photo,
        options: &ExportOptions,
    ) -> DomainResult<Vec<u8>> {
        let saida = self.renderizar(photo, options)?;
        // 🔑 O codificador do `revelacao-core`: o mesmo que o navegador usa.
        // Dois codificadores dariam dois arquivos para a mesma foto revelada.
        revelacao_core::jpeg::codificar(&saida, options.quality())
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to encode JPEG: {}", e)))
    }

    /// 🔑 **Mesmo caminho, ajustes e corte no neutro.** Ele passa pelo shader
    /// como qualquer outro: o que sai daqui tem de ser byte a byte o que
    /// subiria se a foto nunca tivesse sido revelada — inclusive a marca
    /// d'água e o redimensionamento, que são do envio e não da edição.
    async fn renderizar_bruto_jpeg(
        &self,
        photo: &Photo,
        options: &ExportOptions,
    ) -> DomainResult<Option<Vec<u8>>> {
        // Nada revelado, nada a guardar: o próprio envio é o bruto.
        let corte = transformacao::corte_da_entidade(photo);
        if ajustes_da_entidade(photo) == Ajustes::default() && corte == CropSettings::default() {
            return Ok(None);
        }

        let input_path = photo.file_path().as_str()?;
        let img = image::open(Path::new(&input_path)).map_err(|e| {
            DomainError::InfrastructureError(format!("Failed to open source image: {}", e))
        })?;

        let revelada = self.revelar(&img, &Ajustes::default())?;
        let mut saida = transformacao::aplicar(&revelada, &CropSettings::default(), true);
        if let Some(lado_maior) = options.longest_edge() {
            saida = redimensionar(saida, lado_maior);
        }
        if let Some(marca) = options.watermark() {
            saida = aplicar_marca(&saida, marca)?;
        }
        revelacao_core::jpeg::codificar(&saida, options.quality())
            .map(Some)
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to encode JPEG: {}", e)))
    }

    async fn export(
        &self,
        photo: &Photo,
        output_path: &FilePath,
        options: &ExportOptions,
    ) -> DomainResult<()> {
        // 🔑 O mesmo caminho do pós-venda: o arquivo é o JPEG em memória gravado
        // no disco, e não uma segunda codificação. Dois codificadores dariam dois
        // arquivos diferentes para a mesma foto, e a exportação deixaria de ser
        // a prova do que o site recebe.
        let jpeg = self.renderizar_jpeg(photo, options).await?;

        let output_path_str = output_path.as_str()?;
        std::fs::write(Path::new(output_path_str), jpeg).map_err(|e| {
            DomainError::InfrastructureError(format!("Failed to create output file: {}", e))
        })?;

        Ok(())
    }
}
