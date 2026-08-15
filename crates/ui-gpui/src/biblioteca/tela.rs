//! A grade da Biblioteca desenhada.
//!
//! Junta as três peças: as fotos que o `LibraryController` traz do catálogo, as
//! contas de [`super::grade`] e as miniaturas de [`super::miniaturas`].

use std::sync::{Arc, Mutex};

use adapters::view_models::PhotoViewModel;
use gpui::{div, img, prelude::*, px, rgb, uniform_list, Context, SharedString, Window};
use infrastructure::cache::preview_manager::PreviewManager;

use super::grade::{colunas_que_cabem, fotos_da_linha, linhas_necessarias};
use super::miniaturas::{capacidade_para, CacheDeMiniaturas, Miniatura};

/// Lado da miniatura, mais o espaçamento — a unidade que decide quantas colunas
/// cabem. Um número só, e não dois somados na hora de contar: separá-los faria
/// a conta de colunas e o desenho discordarem, e o sintoma é a última coluna
/// cortada pela borda.
const LADO_DO_ITEM: f32 = 180.0;
const ESPACAMENTO: f32 = 8.0;
const PASSO: f32 = LADO_DO_ITEM + ESPACAMENTO;

pub struct Biblioteca {
    /// `Arc` porque o closure do `uniform_list` é `'static` e precisa levar as
    /// fotos consigo — clonar o `Vec` a cada quadro seria copiar o acervo
    /// inteiro 60 vezes por segundo.
    fotos: Arc<Vec<PhotoViewModel>>,
    previews: Arc<PreviewManager>,
    /// `Mutex` porque o closure recebe `&mut App`, e não `&mut self`: o cache
    /// precisa ser escrito de dentro dele.
    cache: Arc<Mutex<CacheDeMiniaturas>>,
}

/// Quantas linhas cabem na altura da janela.
///
/// Serve para dimensionar o cache de miniaturas, e não para desenhar — quem
/// decide o que desenhar é o `uniform_list`. Guardar menos que uma tela faria
/// cada quadro descartar o que o seguinte pede de volta.
fn linhas_visiveis(window: &Window) -> usize {
    const ALTURA_DO_CABECALHO: f32 = 56.0;
    let util = f32::from(window.viewport_size().height) - ALTURA_DO_CABECALHO;
    ((util / PASSO).ceil() as usize).max(1)
}

/// O que sobra para a grade depois das margens laterais.
///
/// Vem de `window.viewport_size()`, e não da medição do contêiner: a grade
/// ocupa a janela inteira menos um padding conhecido, e ler a janela dá a
/// resposta **antes** do primeiro desenho. Medir o filho exigiria um quadro
/// com o número errado para descobrir o certo — e é nesse quadro que
/// `colunas_que_cabem` receberia zero.
///
/// Quando a Biblioteca ganhar a árvore de pastas ao lado, esta conta passa a
/// descontar a largura dela — e é por isso que ela é uma função, e não uma
/// leitura solta no meio do `render`.
fn largura_util(window: &Window) -> f32 {
    const MARGEM_LATERAL: f32 = 16.0;
    f32::from(window.viewport_size().width) - MARGEM_LATERAL
}

impl Biblioteca {
    pub fn nova(fotos: Vec<PhotoViewModel>, previews: Arc<PreviewManager>) -> Self {
        Self {
            fotos: Arc::new(fotos),
            previews,
            // Nasce do tamanho da janela padrão e se ajusta no primeiro
            // `render`, quando a janela de verdade já foi medida.
            cache: Arc::new(Mutex::new(CacheDeMiniaturas::nova(capacidade_para(6, 4)))),
        }
    }

    fn cabecalho(&self) -> impl IntoElement {
        let total = self.fotos.len();
        let texto: SharedString = if total == 0 {
            "Nenhuma foto no catálogo — importe uma pasta pelo app de egui".into()
        } else {
            format!("{total} fotos").into()
        };

        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .p(px(12.))
            .border_b_1()
            .border_color(rgb(0x303030))
            .child(div().text_lg().child("Biblioteca"))
            .child(div().text_sm().text_color(rgb(0x9a9a9a)).child(texto))
    }
}

/// Uma célula da grade: a miniatura, ou o lugar dela.
fn celula(
    foto: &PhotoViewModel,
    previews: &PreviewManager,
    cache: &Mutex<CacheDeMiniaturas>,
) -> impl IntoElement {
    let miniatura = cache
        .lock()
        .expect("o cache de miniaturas não deve estar envenenado")
        .obter(previews, &foto.id);

    let moldura = div()
        .w(px(LADO_DO_ITEM))
        .h(px(LADO_DO_ITEM))
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(0x232323))
        .rounded(px(3.));

    let conteudo = match miniatura {
        // ⚠️ `object_fit` de conter, e não de cobrir: a moldura é quadrada e o
        // acervo não é. `cover` recortaria a foto — e num estúdio de retrato o
        // recorte centralizado tira a cabeça primeiro. É a mesma decisão que a
        // capa do blog do outro projeto custou caro para aprender.
        Miniatura::Pronta(imagem) => {
            moldura.child(img(imagem).max_w(px(LADO_DO_ITEM)).max_h(px(LADO_DO_ITEM)))
        }
        Miniatura::Ausente => moldura.child(
            div()
                .text_xs()
                .text_color(rgb(0x6a6a6a))
                .child("sem preview"),
        ),
    };

    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .w(px(LADO_DO_ITEM))
        .child(conteudo)
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x9a9a9a))
                .truncate()
                .child(SharedString::from(foto.name.clone())),
        )
}

impl Render for Biblioteca {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colunas = colunas_que_cabem(largura_util(window), PASSO);
        let total = self.fotos.len();
        let linhas = linhas_necessarias(total, colunas);

        // O cache acompanha a janela: redimensionar para maior sem isto o
        // deixaria do tamanho da janela antiga, e a grade nova passaria a
        // descartar justamente o que está mostrando.
        self.cache
            .lock()
            .expect("o cache de miniaturas não deve estar envenenado")
            .ajustar_capacidade(capacidade_para(colunas, linhas_visiveis(window)));

        let fotos = self.fotos.clone();
        let previews = self.previews.clone();
        let cache = self.cache.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1b1b1b))
            .text_color(rgb(0xe6e6e6))
            .child(self.cabecalho())
            .child(
                // O `uniform_list` só chama o closure para as linhas visíveis.
                // É o que faz 2.000 fotos custarem o mesmo que 20 na hora de
                // desenhar — a diferença entre rolar liso e engasgar.
                uniform_list("grade-da-biblioteca", linhas, move |faixa, _window, _cx| {
                    faixa
                        .map(|indice| {
                            let desta_linha = fotos_da_linha(indice, colunas, total);

                            div().flex().gap(px(ESPACAMENTO)).p(px(4.)).children(
                                desta_linha
                                    .map(|i| celula(&fotos[i], &previews, &cache))
                                    .collect::<Vec<_>>(),
                            )
                        })
                        .collect()
                })
                .flex_1()
                .p(px(8.)),
            )
    }
}
