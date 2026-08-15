//! As miniaturas da grade, carregadas sob demanda.
//!
//! O cache de preview é um SQLite com JPEG dentro (`PreviewManager`). Cada
//! miniatura custa uma consulta, um decode e a conversão para BGRA — e a grade
//! tem 2.000 delas.
//!
//! **Carregar tudo na abertura é o que engasga**: 2.000 decodes antes do
//! primeiro quadro. O `uniform_list` só pede as linhas visíveis, então aqui
//! também só se carrega o que foi pedido, e o que já veio fica guardado.

use std::num::NonZeroUsize;
use std::sync::Arc;

use gpui::RenderImage;
use infrastructure::cache::preview_manager::PreviewManager;
use lru::LruCache;

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

/// Quantas telas de miniaturas ficam na memória além da visível.
///
/// Uma só bastaria para desenhar, e faria toda rolagem de volta redecodificar o
/// que acabou de sair. Três é uma tela para cada lado mais a atual: cobre o
/// vaivém curto, que é como se procura foto, sem guardar o acervo inteiro.
const TELAS_GUARDADAS: usize = 3;

/// O menor cache que ainda funciona.
///
/// Se a capacidade calculada ficasse abaixo do que **está na tela**, cada
/// quadro descartaria miniatura que o quadro seguinte pede de volta — um cache
/// que só custa. Acontece com a janela minúscula, onde a conta dá quase zero.
const MINIMO_DE_MINIATURAS: usize = 32;

/// Guarda as miniaturas já convertidas, descartando as mais antigas.
///
/// # Por que LRU, e por que a capacidade não é um número escrito à mão
///
/// Uma miniatura de 320px em BGRA ocupa ~300 KB. Com um mapa sem descarte, as
/// 2.000 do acervo de referência dariam ~600 MB — o consumo crescia com o
/// quanto se rolava, e não com o que estava na tela.
///
/// A capacidade vem de [`capacidade_para`], que a deriva do que **cabe na
/// janela**: quem abre em tela cheia guarda mais que quem abre numa janela
/// pequena, e é o certo nos dois casos. Um número fixo aqui seria grande demais
/// para o notebook e pequeno demais para o monitor de 27".
pub struct CacheDeMiniaturas {
    carregadas: LruCache<String, Miniatura>,
}

/// Quantas miniaturas guardar, dado o que cabe na tela.
///
/// `colunas × linhas_visiveis` é o que está à vista; o resto é folga para a
/// rolagem. O piso existe para a janela minúscula não produzir um cache que
/// descarta o que acabou de mostrar.
pub fn capacidade_para(colunas: usize, linhas_visiveis: usize) -> NonZeroUsize {
    let na_tela = colunas.saturating_mul(linhas_visiveis);
    let desejada = na_tela
        .saturating_mul(TELAS_GUARDADAS)
        .max(MINIMO_DE_MINIATURAS);

    NonZeroUsize::new(desejada).expect("o mínimo garante que não é zero")
}

impl CacheDeMiniaturas {
    pub fn nova(capacidade: NonZeroUsize) -> Self {
        Self {
            carregadas: LruCache::new(capacidade),
        }
    }

    /// Ajusta o tamanho quando a janela muda.
    ///
    /// Redimensionar para maior sem isto deixaria o cache do tamanho da janela
    /// antiga, e a grade nova passaria a descartar o que mostra.
    ///
    /// **Só cresce dentro do quadro; encolher descarta na hora** — é o que o
    /// `lru` faz, e é o comportamento certo: quem diminuiu a janela quer a
    /// memória de volta.
    pub fn ajustar_capacidade(&mut self, capacidade: NonZeroUsize) {
        if self.carregadas.cap() != capacidade {
            self.carregadas.resize(capacidade);
        }
    }

    /// A miniatura desta foto, carregando do cache na primeira vez.
    ///
    /// **Ausência também é guardada.** Sem isso, uma foto sem preview seria
    /// consultada de novo a cada quadro em que estivesse visível — 60 consultas
    /// por segundo por foto, exatamente durante a importação, que é quando o
    /// disco já está ocupado gerando os previews que faltam.
    ///
    /// `get` e não `peek`: ler **é** usar, e é o que move a miniatura para o
    /// fim da fila de descarte. Com `peek`, o que está na tela agora seria a
    /// primeira coisa a ser jogada fora.
    pub fn obter(&mut self, previews: &PreviewManager, id_da_foto: &str) -> Miniatura {
        if let Some(ja_carregada) = self.carregadas.get(id_da_foto) {
            return ja_carregada.clone();
        }

        let miniatura = match previews.get_thumbnail(id_da_foto) {
            Some(imagem) => Miniatura::Pronta(para_gpui(imagem)),
            None => Miniatura::Ausente,
        };

        self.carregadas
            .put(id_da_foto.to_string(), miniatura.clone());
        miniatura
    }

    /// Quantas miniaturas estão na memória agora.
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

        let mut cache = CacheDeMiniaturas::nova(capacidade_para(4, 4));

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
        let mut cache = CacheDeMiniaturas::nova(capacidade_para(4, 4));

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

        let mut cache = CacheDeMiniaturas::nova(capacidade_para(4, 4));
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
        let mut cache = CacheDeMiniaturas::nova(capacidade_para(4, 4));

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

        let mut cache = CacheDeMiniaturas::nova(capacidade_para(4, 4));
        let Miniatura::Pronta(render) = cache.obter(&previews, "foto-1") else {
            panic!("a miniatura tinha de estar pronta");
        };

        let bytes = render.as_bytes(0).expect("o quadro 0 tem de existir");
        // JPEG é com perda, então o vermelho volta perto de 255, não exato.
        assert!(bytes[0] < 40, "canal B tem de ser baixo, veio {}", bytes[0]);
        assert!(bytes[2] > 200, "canal R tem de ser alto, veio {}", bytes[2]);
    }

    #[test]
    fn a_capacidade_cresce_com_o_que_cabe_na_tela() {
        // Quem abre em tela cheia guarda mais que quem abre numa janela
        // pequena, e é o certo nos dois casos. Um número fixo seria grande
        // demais para o notebook e pequeno demais para o monitor de 27".
        let pequena = capacidade_para(3, 3);
        let grande = capacidade_para(8, 6);

        assert!(
            grande > pequena,
            "{grande} tinha de ser maior que {pequena}"
        );
    }

    #[test]
    fn janela_minuscula_ainda_guarda_o_que_mostra() {
        // Com capacidade abaixo do que está na tela, cada quadro descartaria a
        // miniatura que o quadro seguinte pede de volta — um cache que só
        // custa. O piso existe para isso.
        let minima = capacidade_para(1, 1);

        assert!(usize::from(minima) >= MINIMO_DE_MINIATURAS);
    }

    #[test]
    fn a_capacidade_cobre_mais_de_uma_tela() {
        // Guardar só o visível faria toda rolagem de volta redecodificar o que
        // acabou de sair — e procurar foto é justamente vaivém curto.
        let na_tela = 5 * 4;
        let capacidade = usize::from(capacidade_para(5, 4));

        assert!(
            capacidade >= na_tela * 2,
            "capacidade {capacidade} tem de cobrir mais de uma tela de {na_tela}"
        );
    }

    #[test]
    fn o_cache_nao_cresce_alem_da_capacidade() {
        // O defeito que este LRU vem consertar: com um mapa, rolar 2.000 fotos
        // guardava as 2.000 — ~600 MB, crescendo com o quanto se rolava e não
        // com o que estava na tela.
        let (previews, _dir) = cache_descartavel();
        let capacidade = NonZeroUsize::new(10).unwrap();
        let mut cache = CacheDeMiniaturas::nova(capacidade);

        for i in 0..100 {
            cache.obter(&previews, &format!("foto-{i}"));
        }

        assert_eq!(cache.quantas_na_memoria(), 10);
    }

    #[test]
    fn quem_foi_lido_por_ultimo_sobrevive() {
        // `get` e não `peek`: ler é usar. Com `peek`, o que está na tela agora
        // seria a primeira coisa descartada.
        let (previews, _dir) = cache_descartavel();
        previews
            .save_thumbnail("veterana", &foto_vermelha())
            .expect("gravar miniatura");

        let mut cache = CacheDeMiniaturas::nova(NonZeroUsize::new(3).unwrap());
        cache.obter(&previews, "veterana");

        for i in 0..2 {
            cache.obter(&previews, &format!("passageira-{i}"));
        }
        // Relê a veterana: ela volta para o fim da fila de descarte.
        cache.obter(&previews, "veterana");
        // Mais duas empurram para fora as duas passageiras, não ela.
        for i in 2..4 {
            cache.obter(&previews, &format!("passageira-{i}"));
        }

        assert!(
            matches!(cache.obter(&previews, "veterana"), Miniatura::Pronta(_)),
            "a miniatura relida tinha de ter sobrevivido ao descarte"
        );
    }

    #[test]
    fn encolher_a_janela_devolve_memoria() {
        let (previews, _dir) = cache_descartavel();
        let mut cache = CacheDeMiniaturas::nova(NonZeroUsize::new(50).unwrap());

        for i in 0..50 {
            cache.obter(&previews, &format!("foto-{i}"));
        }
        assert_eq!(cache.quantas_na_memoria(), 50);

        // Quem diminuiu a janela quer a memória de volta, e não daqui a pouco.
        cache.ajustar_capacidade(NonZeroUsize::new(5).unwrap());

        assert_eq!(cache.quantas_na_memoria(), 5);
    }
}
