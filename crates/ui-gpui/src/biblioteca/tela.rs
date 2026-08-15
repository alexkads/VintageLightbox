//! A grade da Biblioteca desenhada.
//!
//! Junta as três peças: as fotos que o `LibraryController` traz do catálogo, as
//! contas de [`super::grade`] e as miniaturas de [`super::miniaturas`].

use std::sync::{Arc, Mutex};

use adapters::view_models::PhotoViewModel;
use gpui::{div, img, prelude::*, px, rgb, uniform_list, Context, SharedString, Window};
use infrastructure::cache::preview_manager::PreviewManager;

use super::filtros::{indices_visiveis, FiltroDeSinalizador, Filtros, NotaMinima};
use super::grade::{colunas_que_cabem, fotos_da_linha, linhas_necessarias};
use super::miniaturas::{capacidade_para, CacheDeMiniaturas, Miniatura};
use super::pastas::{pastas_do_acervo, Pasta};

/// Lado da miniatura, mais o espaçamento — a unidade que decide quantas colunas
/// cabem. Um número só, e não dois somados na hora de contar: separá-los faria
/// a conta de colunas e o desenho discordarem, e o sintoma é a última coluna
/// cortada pela borda.
const LADO_DO_ITEM: f32 = 180.0;
const ESPACAMENTO: f32 = 8.0;
const PASSO: f32 = LADO_DO_ITEM + ESPACAMENTO;

/// Largura da coluna de pastas.
const LADO_DA_ARVORE: f32 = 220.0;

pub struct Biblioteca {
    /// `Arc` porque o closure do `uniform_list` é `'static` e precisa levar as
    /// fotos consigo — clonar o `Vec` a cada quadro seria copiar o acervo
    /// inteiro 60 vezes por segundo.
    fotos: Arc<Vec<PhotoViewModel>>,
    previews: Arc<PreviewManager>,
    /// `Mutex` porque o closure recebe `&mut App`, e não `&mut self`: o cache
    /// precisa ser escrito de dentro dele.
    cache: Arc<Mutex<CacheDeMiniaturas>>,
    filtros: Filtros,
    /// Os índices do acervo que passam pelos filtros.
    ///
    /// Recalculado **quando o filtro muda**, e não a cada quadro: filtrar 2.000
    /// fotos 60 vezes por segundo é trabalho que ninguém pediu, e a resposta é
    /// sempre a mesma enquanto ninguém tocar na barra.
    visiveis: Vec<usize>,
    /// As pastas do acervo, calculadas **uma vez**.
    ///
    /// Elas saem dos caminhos das fotos, e as fotos não mudam enquanto a tela
    /// vive — recalcular a cada quadro seria varrer 2.000 caminhos 60 vezes por
    /// segundo para chegar sempre à mesma lista.
    pastas: Vec<Pasta>,
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
    f32::from(window.viewport_size().width) - MARGEM_LATERAL - LADO_DA_ARVORE
}

impl Biblioteca {
    pub fn nova(fotos: Vec<PhotoViewModel>, previews: Arc<PreviewManager>) -> Self {
        let mut tela = Self {
            fotos: Arc::new(fotos),
            previews,
            // Nasce do tamanho da janela padrão e se ajusta no primeiro
            // `render`, quando a janela de verdade já foi medida.
            cache: Arc::new(Mutex::new(CacheDeMiniaturas::nova(capacidade_para(6, 4)))),
            filtros: Filtros::default(),
            visiveis: Vec::new(),
            pastas: Vec::new(),
        };
        tela.pastas = pastas_do_acervo(&tela.fotos);
        // Nasce com a lista pronta: sem isto o primeiro quadro mostraria uma
        // grade vazia sobre um acervo cheio, e a tela só se corrigiria no
        // primeiro clique.
        tela.refiltrar();
        tela
    }

    /// Recalcula o que está visível. Chamado só quando um filtro muda.
    fn refiltrar(&mut self) {
        self.visiveis = indices_visiveis(&self.fotos, &self.filtros);
    }

    fn cabecalho(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.fotos.len();
        let mostradas = self.visiveis.len();

        // O contador diz **as duas coisas** quando há filtro. Mostrar só
        // "12 fotos" com 2.000 no acervo é um número parcial se apresentando
        // como total, e quem lê conclui que perdeu o acervo.
        let texto: SharedString = if total == 0 {
            "Nenhuma foto no catálogo — importe uma pasta pelo app de egui".into()
        } else if mostradas == total {
            format!("{total} fotos").into()
        } else {
            format!("{mostradas} de {total} fotos").into()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .border_b_1()
            .border_color(rgb(0x303030))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(div().text_lg().child("Biblioteca"))
                    .child(div().text_sm().text_color(rgb(0x9a9a9a)).child(texto)),
            )
            .child(self.barra_de_filtros(cx))
    }

    /// Notas, sinalizadores e o botão de limpar.
    fn barra_de_filtros(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let nota_atual = self.filtros.nota_minima.0;
        let sinalizador_atual = self.filtros.sinalizador;

        // Laços com `for`, e não `map` com `move`: `cx.listener` empresta o
        // `cx`, e um closure `move` o levaria embora — sobrando nada para o
        // segundo grupo de botões. O compilador chama isso de "captured
        // variable cannot escape", e a saída é coletar em `Vec` emprestando a
        // cada volta.
        let mut notas = Vec::new();
        for n in 0..=5u8 {
            notas.push(botao(
                format!("nota-{n}"),
                if n == 0 {
                    "todas".to_string()
                } else {
                    "★".repeat(n as usize)
                },
                nota_atual == n,
                cx.listener(
                    move |this: &mut Self,
                          _ev: &gpui::ClickEvent,
                          _window,
                          cx: &mut Context<Self>| {
                        this.filtros.nota_minima = NotaMinima(n);
                        this.refiltrar();
                        cx.notify();
                    },
                ),
            ));
        }

        let mut sinalizadores = Vec::new();
        for (qual, rotulo) in [
            (FiltroDeSinalizador::Qualquer, "todos"),
            (FiltroDeSinalizador::Escolhidas, "escolhidas"),
            (FiltroDeSinalizador::Rejeitadas, "rejeitadas"),
            (FiltroDeSinalizador::SemSinalizador, "sem marca"),
        ] {
            sinalizadores.push(botao(
                format!("sinal-{rotulo}"),
                rotulo.to_string(),
                sinalizador_atual == qual,
                cx.listener(
                    move |this: &mut Self,
                          _ev: &gpui::ClickEvent,
                          _window,
                          cx: &mut Context<Self>| {
                        this.filtros.sinalizador = qual;
                        this.refiltrar();
                        cx.notify();
                    },
                ),
            ));
        }

        // As cinco do domínio (`ColorLabel`), na grafia que o legado grava na
        // coluna — comparar com outra escrita não casaria com nada.
        let mut cores = vec![botao(
            "cor-todas".to_string(),
            "todas".to_string(),
            self.filtros.cor.is_none(),
            cx.listener(
                |this: &mut Self, _ev: &gpui::ClickEvent, _window, cx: &mut Context<Self>| {
                    this.filtros.cor = None;
                    this.refiltrar();
                    cx.notify();
                },
            ),
        )];
        for (valor, rotulo) in [
            ("Red", "vermelho"),
            ("Yellow", "amarelo"),
            ("Green", "verde"),
            ("Blue", "azul"),
            ("Purple", "roxo"),
        ] {
            let aceso = self.filtros.cor.as_deref() == Some(valor);
            cores.push(botao(
                format!("cor-{valor}"),
                rotulo.to_string(),
                aceso,
                cx.listener(
                    move |this: &mut Self,
                          _ev: &gpui::ClickEvent,
                          _window,
                          cx: &mut Context<Self>| {
                        this.filtros.cor = if aceso { None } else { Some(valor.to_string()) };
                        this.refiltrar();
                        cx.notify();
                    },
                ),
            ));
        }

        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(12.))
            .child(rotulo_do_grupo("nota mínima"))
            .child(div().flex().gap(px(4.)).children(notas))
            .child(rotulo_do_grupo("sinalizador"))
            .child(div().flex().gap(px(4.)).children(sinalizadores))
            .child(rotulo_do_grupo("cor"))
            .child(div().flex().gap(px(4.)).children(cores))
    }

    /// A coluna de pastas, à esquerda da grade.
    fn arvore_de_pastas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let escolhida = self.filtros.pasta.clone();

        let mut itens = vec![botao(
            "pasta-todas".to_string(),
            format!("Todas ({})", self.fotos.len()),
            escolhida.is_none(),
            cx.listener(
                |this: &mut Self, _ev: &gpui::ClickEvent, _window, cx: &mut Context<Self>| {
                    this.filtros.pasta = None;
                    this.refiltrar();
                    cx.notify();
                },
            ),
        )];

        for pasta in &self.pastas {
            let caminho = pasta.caminho.clone();
            let aceso = escolhida.as_deref() == Some(caminho.as_str());
            let alvo = caminho.clone();

            itens.push(botao(
                format!("pasta-{caminho}"),
                format!("{} ({})", pasta.nome, pasta.quantas),
                aceso,
                cx.listener(
                    move |this: &mut Self,
                          _ev: &gpui::ClickEvent,
                          _window,
                          cx: &mut Context<Self>| {
                        // Clicar de novo na pasta acesa desfaz a escolha: sem isso,
                        // a única saída seria achar o botão "Todas" no meio de uma
                        // lista comprida.
                        this.filtros.pasta = if this.filtros.pasta.as_deref() == Some(alvo.as_str())
                        {
                            None
                        } else {
                            Some(alvo.clone())
                        };
                        this.refiltrar();
                        cx.notify();
                    },
                ),
            ));
        }

        div()
            .id("arvore-de-pastas")
            .flex()
            .flex_col()
            .gap(px(4.))
            .w(px(LADO_DA_ARVORE))
            .h_full()
            .p(px(8.))
            .overflow_y_scroll()
            .border_r_1()
            .border_color(rgb(0x303030))
            .child(rotulo_do_grupo("pastas"))
            .children(itens)
    }
}

/// Rótulo cinza que nomeia um grupo de botões.
fn rotulo_do_grupo(texto: &'static str) -> impl IntoElement {
    div().text_xs().text_color(rgb(0x7a7a7a)).child(texto)
}

/// Um botão de filtro, aceso quando é o escolhido.
///
/// `id` próprio em cada um: o GPUI usa o id para saber que este é o mesmo
/// elemento entre quadros. Dois botões com o mesmo id trocariam de estado um
/// com o outro ao serem clicados.
///
/// Devolve `AnyElement`, e não `impl IntoElement`: **cada closure tem um tipo
/// concreto próprio**, então dois botões com ações diferentes são dois tipos
/// diferentes, e um `Vec` deles não compila. Apagar o tipo aqui é o que permite
/// montar a barra com um `push` por botão.
fn botao(
    id: String,
    texto: String,
    aceso: bool,
    ao_clicar: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    div()
        .id(SharedString::from(id))
        .px(px(8.))
        .py(px(3.))
        .rounded(px(3.))
        .text_xs()
        .cursor_pointer()
        .bg(if aceso { rgb(0x3a5a8a) } else { rgb(0x2a2a2a) })
        .text_color(if aceso { rgb(0xffffff) } else { rgb(0xb0b0b0) })
        .hover(|estilo| estilo.bg(if aceso { rgb(0x456ba0) } else { rgb(0x363636) }))
        .child(SharedString::from(texto))
        .on_click(ao_clicar)
        .into_any_element()
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colunas = colunas_que_cabem(largura_util(window), PASSO);
        // O total da grade é o **filtrado**, e não o acervo: são os índices em
        // `visiveis` que o `uniform_list` percorre.
        let total = self.visiveis.len();
        let linhas = linhas_necessarias(total, colunas);

        // O cache acompanha a janela: redimensionar para maior sem isto o
        // deixaria do tamanho da janela antiga, e a grade nova passaria a
        // descartar justamente o que está mostrando.
        self.cache
            .lock()
            .expect("o cache de miniaturas não deve estar envenenado")
            .ajustar_capacidade(capacidade_para(colunas, linhas_visiveis(window)));

        let fotos = self.fotos.clone();
        let visiveis = Arc::new(self.visiveis.clone());
        let previews = self.previews.clone();
        let cache = self.cache.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1b1b1b))
            .text_color(rgb(0xe6e6e6))
            .child(self.cabecalho(cx))
            // A partir daqui é uma linha: pastas à esquerda, grade à direita.
            // `min_h(0)` na linha e `flex_1` nos dois filhos — sem o `min_h`, o
            // conteúdo rolável empurra o pai e a rolagem nunca acontece.
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(self.arvore_de_pastas(cx))
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
                                            // Dois saltos: a linha dá a posição na lista
                                            // **filtrada**, e `visiveis` traduz para o
                                            // índice do acervo. Indexar `fotos` direto
                                            // mostraria a foto errada assim que houvesse
                                            // filtro — e a grade continuaria bonita, que
                                            // é o que torna esse defeito caro.
                                            .map(|posicao| {
                                                celula(&fotos[visiveis[posicao]], &previews, &cache)
                                            })
                                            .collect::<Vec<_>>(),
                                    )
                                })
                                .collect()
                        })
                        .flex_1()
                        .p(px(8.)),
                    ),
            )
    }
}
