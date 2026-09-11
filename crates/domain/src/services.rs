//! Domain Services
//!
//! Serviços que não pertencem estritamente a uma entidade ou value object.

use crate::value_objects::{FilePath, PhotoMetadata};
use crate::DomainResult;
use async_trait::async_trait;

/// Serviço para extração de metadados de fotos
#[async_trait]
pub trait MetadataExtractor: Send + Sync {
    /// Extrai metadados do arquivo especificado
    fn extract(&self, path: &FilePath) -> DomainResult<PhotoMetadata>;
}

/// Serviço para geração de thumbnails
#[async_trait]
pub trait ThumbnailGenerator: Send + Sync {
    /// Gera um thumbnail para a imagem especificada
    /// Retorna os bytes da imagem (JPEG) redimensionada
    async fn generate(&self, path: &FilePath, max_size: u32) -> DomainResult<Vec<u8>>;

    /// Gera múltiplos thumbnails de uma vez, otimizando a leitura do arquivo
    /// Retorna os bytes de cada thumbnail na ordem solicitada
    async fn generate_set(&self, path: &FilePath, max_sizes: &[u32]) -> DomainResult<Vec<Vec<u8>>> {
        let mut results = Vec::new();
        for size in max_sizes {
            results.push(self.generate(path, *size).await?);
        }
        Ok(results)
    }
}

/// Representa uma imagem RAW decodificada
pub struct RawImage {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u16>, // Dados RAW geralmente são 12-14 bits, cabem em u16
    pub cpp: usize,     // Components per pixel (usually 1 for bayer)
}

/// Serviço para decodificação de arquivos RAW
#[async_trait]
pub trait RawDecoder: Send + Sync {
    /// Decodifica um arquivo RAW retornando os dados brutos
    fn decode(&self, path: &FilePath) -> DomainResult<RawImage>;
}

/// Serviço para exportação de imagens processadas
#[async_trait]
pub trait ImageExporter: Send + Sync {
    /// Exporta a foto aplicando as edições para o caminho de destino.
    ///
    /// 🔑 **As opções não são opcionais.** Elas carregam a marca d'água, que é a
    /// regra que separa entregar de mostrar: a foto comprada vai inteira, a que
    /// ficou para trás vai marcada. Um `export` sem opções deixaria "sem marca"
    /// como caminho mais curto — e o caminho mais curto é o que se pega no dia
    /// em que a atenção falta.
    async fn export(
        &self,
        photo: &crate::entities::Photo,
        output_path: &FilePath,
        options: &crate::value_objects::ExportOptions,
    ) -> DomainResult<()>;

    /// A mesma foto pronta, como bytes de JPEG — sem passar pelo disco.
    ///
    /// 🔑 **Existe para o pós-venda.** Subir uma galeria de trinta fotos por um
    /// arquivo temporário cada seria gravar e reler trinta vezes o que já está
    /// na memória; e o arquivo temporário é justamente o lugar onde a foto não
    /// comprada fica esquecida, legível, no disco de quem exportou.
    ///
    /// As mesmas opções de `export`, pela mesma razão: a marca d'água não é
    /// opcional por acidente.
    async fn renderizar_jpeg(
        &self,
        photo: &crate::entities::Photo,
        options: &crate::value_objects::ExportOptions,
    ) -> DomainResult<Vec<u8>>;

    /// A foto **sem revelação nenhuma**, como bytes de JPEG.
    ///
    /// 🚨 **É o arquivo de volta do "Zerar tudo".** O envio ao site renderiza
    /// com os ajustes do catálogo, então o que sobe já vem tratado; sem esta
    /// segunda cópia o servidor passa a tratar o revelado como se fosse o
    /// original, e a edição fica sem volta de lá (achado do dono, 11/set/2026).
    ///
    /// Sem ajustes **e sem corte**: enquadrar também é edição, e um bruto
    /// recortado devolveria metade do arrependimento.
    ///
    /// 🔑 **`None` quando a foto está no neutro**, e a decisão mora aqui de
    /// propósito: quem sabe ler os 53 campos da entidade é a implementação, e
    /// quem chama não deve reimplementar "isto está editado?" para escolher se
    /// pede. Nesse caso o próprio envio **é** o bruto — renderizar de novo
    /// seria uma geração de JPEG a mais para produzir o mesmo arquivo.
    async fn renderizar_bruto_jpeg(
        &self,
        photo: &crate::entities::Photo,
        options: &crate::value_objects::ExportOptions,
    ) -> DomainResult<Option<Vec<u8>>>;
}

pub mod pos_venda;

pub mod preview_storage;
pub use preview_storage::*;

pub mod file_organizer;
pub use file_organizer::FileOrganizer;

pub mod source_scanner;
pub use source_scanner::SourceScanner;
