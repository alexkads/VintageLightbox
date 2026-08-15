//! As miniaturas da grade, carregadas sob demanda.
//!
//! O cache de preview é um SQLite com JPEG dentro (`PreviewManager`). Cada
//! miniatura custa uma consulta, um decode e a conversão para BGRA — e a grade
//! tem 2.000 delas.
//!
//! **Carregar tudo na abertura é o que engasga**: 2.000 decodes antes do
//! primeiro quadro. O `uniform_list` só pede as linhas visíveis, então aqui
//! também só se carrega o que foi pedido, e o que já veio fica guardado.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::RenderImage;
use infrastructure::cache::preview_manager::PreviewManager;

use crate::imagem::para_gpui;

/// O que a grade sabe sobre uma miniatura.
#[derive(Clone)]
pub enum Miniatura {
    /// Veio do cache e está pronta para desenhar.
    Pronta(Arc<RenderImage>),
    /// O cache não tem essa foto — a grade desenha um retângulo no lugar.
    ///
    /// É um estado normal, e não um erro: quem acabou de importar ainda não
    /// tem preview gerado. Tratar como falha faria a grade piscar avisos
    /// durante toda a primeira importação.
    Ausente,
}

/// Guarda as miniaturas já convertidas, por id de foto.
///
/// # Por que um mapa simples, e não LRU
///
/// Uma miniatura de 320px em BGRA ocupa ~400 KB, e o acervo de referência tem
/// 2.000 fotos — o teto é ~800 MB, o que **não** cabe. O `crates/ui` resolve
/// isso com `lru`, e aqui vai ser preciso também.
///
/// Fica um mapa nesta entrega de propósito: a política de descarte depende de
/// quantas linhas o `uniform_list` mantém vivas fora da tela, e isso se mede
/// com a grade rodando sobre o catálogo real — que é o critério de saída da
/// fase 1. Escolher o tamanho do LRU antes dessa medição seria chutar o número
/// que a medição existe para dar.
///
/// ⚠️ **Enquanto isso, o consumo cresce com o quanto se rola.** Não é para ir
/// para a fase 2 assim.
#[derive(Default)]
pub struct CacheDeMiniaturas {
    carregadas: HashMap<String, Miniatura>,
}

impl CacheDeMiniaturas {
    pub fn novo() -> Self {
        Self::default()
    }

    /// A miniatura desta foto, carregando do cache na primeira vez.
    ///
    /// **Ausência também é guardada.** Sem isso, uma foto sem preview seria
    /// consultada de novo a cada quadro em que estivesse visível — 60 consultas
    /// por segundo por foto, exatamente durante a importação, que é quando o
    /// disco já está ocupado gerando os previews que faltam.
    pub fn obter(&mut self, previews: &PreviewManager, id_da_foto: &str) -> Miniatura {
        if let Some(ja_carregada) = self.carregadas.get(id_da_foto) {
            return ja_carregada.clone();
        }

        let miniatura = match previews.get_thumbnail(id_da_foto) {
            Some(imagem) => Miniatura::Pronta(para_gpui(imagem)),
            None => Miniatura::Ausente,
        };

        self.carregadas
            .insert(id_da_foto.to_string(), miniatura.clone());
        miniatura
    }

    /// Quantas miniaturas estão na memória — o número que a fase 1 vai medir
    /// para decidir o tamanho do LRU.
    pub fn quantas_na_memoria(&self) -> usize {
        self.carregadas.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use image::{DynamicImage, Rgba, RgbaImage};
    use tempfile::TempDir;

    /// Um `PreviewManager` sobre catálogo descartável.
    ///
    /// Nunca `PreviewManager::new()` num teste: aquele resolve
    /// `AppPaths::preview_cache_dir()` e escreve na biblioteca de fotos de quem
    /// rodar a suíte — foi o defeito que a fase 0 encontrou.
    fn cache_descartavel() -> (PreviewManager, TempDir) {
        let dir = TempDir::new().expect("criar diretório temporário");
        (PreviewManager::new_with_path(dir.path().to_path_buf()), dir)
    }

    fn foto_vermelha() -> DynamicImage {
        let mut img = RgbaImage::new(4, 4);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([255, 0, 0, 255]);
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn foto_com_preview_no_cache_vira_miniatura_pronta() {
        let (previews, _dir) = cache_descartavel();
        previews
            .save_thumbnail("foto-1", &foto_vermelha())
            .expect("gravar miniatura");

        let mut cache = CacheDeMiniaturas::novo();

        assert!(matches!(
            cache.obter(&previews, "foto-1"),
            Miniatura::Pronta(_)
        ));
    }

    #[test]
    fn foto_sem_preview_e_ausente_e_nao_erro() {
        // Quem acabou de importar ainda não tem preview gerado. É estado
        // normal — tratar como falha faria a grade piscar avisos durante toda
        // a primeira importação.
        let (previews, _dir) = cache_descartavel();
        let mut cache = CacheDeMiniaturas::novo();

        assert!(matches!(
            cache.obter(&previews, "nunca-vista"),
            Miniatura::Ausente
        ));
    }

    #[test]
    fn a_segunda_leitura_vem_da_memoria() {
        let (previews, _dir) = cache_descartavel();
        previews
            .save_thumbnail("foto-1", &foto_vermelha())
            .expect("gravar miniatura");

        let mut cache = CacheDeMiniaturas::novo();
        let Miniatura::Pronta(primeira) = cache.obter(&previews, "foto-1") else {
            panic!("a primeira leitura tinha de achar a miniatura");
        };
        let Miniatura::Pronta(segunda) = cache.obter(&previews, "foto-1") else {
            panic!("a segunda leitura tinha de achar a miniatura");
        };

        // Mesmo `id` significa a **mesma** textura: o GPUI compara `RenderImage`
        // por id para decidir se pode reaproveitar o que já mandou para a GPU.
        // Ids diferentes fariam cada quadro reenviar a grade inteira.
        assert_eq!(primeira.id, segunda.id);
        assert_eq!(cache.quantas_na_memoria(), 1);
    }

    #[test]
    fn a_ausencia_tambem_e_guardada() {
        // Sem isto, uma foto sem preview seria consultada 60 vezes por segundo
        // enquanto estivesse visível — justo durante a importação, quando o
        // disco já está ocupado gerando os previews que faltam.
        let (previews, _dir) = cache_descartavel();
        let mut cache = CacheDeMiniaturas::novo();

        cache.obter(&previews, "nunca-vista");
        cache.obter(&previews, "nunca-vista");

        assert_eq!(cache.quantas_na_memoria(), 1);
    }

    #[test]
    fn a_miniatura_chega_em_bgra() {
        // A ponta a ponta: JPEG gravado no cache, decodificado e convertido.
        // Vermelho tem de sair como B=0 G=0 R=255 — se sair [255, 0, 0], a
        // grade inteira aparece azul.
        let (previews, _dir) = cache_descartavel();
        previews
            .save_thumbnail("foto-1", &foto_vermelha())
            .expect("gravar miniatura");

        let mut cache = CacheDeMiniaturas::novo();
        let Miniatura::Pronta(render) = cache.obter(&previews, "foto-1") else {
            panic!("a miniatura tinha de estar pronta");
        };

        let bytes = render.as_bytes(0).expect("o quadro 0 tem de existir");
        // JPEG é com perda, então o vermelho volta perto de 255, não exato.
        assert!(bytes[0] < 40, "canal B tem de ser baixo, veio {}", bytes[0]);
        assert!(bytes[2] > 200, "canal R tem de ser alto, veio {}", bytes[2]);
    }
}
