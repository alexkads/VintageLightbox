//! A grade da Biblioteca desenhada.
//!
//! Junta as três peças: as fotos que o `LibraryController` traz do catálogo, as
//! contas de [`super::grade`] e as miniaturas de [`super::miniaturas`].

use std::sync::{Arc, Mutex};

use adapters::view_models::PhotoViewModel;
use gpui::{
    div, img, prelude::*, px, uniform_list, App, Context, Entity, SharedString, Subscription,
    Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, Selectable, Sizable};
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
    /// Índice **no acervo** da foto selecionada.
    ///
    /// No acervo e não na lista filtrada: mudar de filtro não pode trocar qual
    /// foto está selecionada. Guardando a posição filtrada, escolher outra
    /// pasta manteria o índice e selecionaria uma foto diferente sem ninguém
    /// clicar em nada.
    selecionada: Option<usize>,
    /// O campo de busca.
    ///
    /// Entidade própria, e não um `String` no estado da tela: o `InputState` do
    /// `gpui-component` guarda o texto, o cursor, a seleção, o histórico de
    /// desfazer e o piscar do cursor. Espelhar isso num campo daqui seria
    /// reescrever o componente.
    busca: Entity<InputState>,
    /// 🚨 As assinaturas **têm de morar aqui**.
    ///
    /// `cx.subscribe` devolve uma `Subscription` que cancela a inscrição quando
    /// é descartada. Um `let _ = cx.subscribe(...)` compila, roda, e o campo de
    /// busca simplesmente não filtra nada — sem erro, sem aviso, digitando
    /// normalmente. Guardar é o que mantém a inscrição viva enquanto a tela
    /// existe.
    _assinaturas: Vec<Subscription>,
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
    pub fn nova(
        fotos: Vec<PhotoViewModel>,
        previews: Arc<PreviewManager>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let busca = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("buscar pelo nome do arquivo")
                // Esc limpa o campo. Sem isso, desfazer uma busca é apagar
                // caractere por caractere ou achar o `x` com o ponteiro — e
                // buscar é a coisa que mais se desfaz numa Biblioteca.
                .clean_on_escape()
        });

        // A tela não lê o campo a cada quadro: ela é avisada quando o texto
        // muda. Ler no `render` funcionaria e custaria uma comparação de string
        // por quadro para descobrir que nada mudou em 99% deles.
        let assinatura = cx.subscribe(&busca, |tela, campo, evento: &InputEvent, cx| {
            // Só `Change`. `Focus` e `Blur` também chegam aqui, e refiltrar 2.000
            // fotos porque alguém clicou no campo é trabalho que ninguém pediu.
            if matches!(evento, InputEvent::Change) {
                tela.filtros.busca = campo.read(cx).value().to_string();
                tela.refiltrar();
                cx.notify();
            }
        });

        let mut tela = Self {
            fotos: Arc::new(fotos),
            previews,
            // Nasce do tamanho da janela padrão e se ajusta no primeiro
            // `render`, quando a janela de verdade já foi medida.
            cache: Arc::new(Mutex::new(CacheDeMiniaturas::nova(capacidade_para(6, 4)))),
            filtros: Filtros::default(),
            visiveis: Vec::new(),
            pastas: Vec::new(),
            selecionada: None,
            busca,
            _assinaturas: vec![assinatura],
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
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(div().text_lg().child("Biblioteca"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(texto),
                    ),
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
            // A busca abre a barra porque é o filtro que responde à pergunta
            // mais específica — "onde está *esta* foto". Os outros respondem
            // "quais são as boas", que é uma varredura, não uma busca.
            //
            // A largura vem de um `div` em volta: o `Input` não implementa
            // `Styled`, então ele não tem `.w()`. Sem a moldura ele cresceria
            // até o fim da linha e empurraria os filtros para a linha de baixo.
            .child(
                div()
                    .w(px(240.))
                    .child(Input::new(&self.busca).cleanable(true)),
            )
            .child(rotulo_do_grupo("nota mínima", cx))
            .child(div().flex().gap(px(4.)).children(notas))
            .child(rotulo_do_grupo("sinalizador", cx))
            .child(div().flex().gap(px(4.)).children(sinalizadores))
            .child(rotulo_do_grupo("cor", cx))
            .child(div().flex().gap(px(4.)).children(cores))
    }

    /// A faixa de miniaturas do rodapé.
    ///
    /// Mostra a **vizinhança da selecionada**, e não o acervo inteiro: o
    /// filmstrip existe para responder "o que vem antes e depois desta", que é
    /// a pergunta de quem está escolhendo entre fotos parecidas. Uma faixa com
    /// 2.000 itens responderia a mesma coisa que a grade, com menos espaço.
    ///
    /// Sem seleção ele mostra o começo da lista — é o que dá para dizer antes
    /// de alguém escolher alguma coisa.
    fn filmstrip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        /// Quantas de cada lado. Ímpar de propósito: a selecionada fica no meio.
        const VIZINHAS: usize = 7;

        let posicao_atual = self
            .selecionada
            .and_then(|no_acervo| self.visiveis.iter().position(|&i| i == no_acervo));

        let inicio = posicao_atual
            .map(|p| p.saturating_sub(VIZINHAS))
            .unwrap_or(0);
        let fim = (inicio + VIZINHAS * 2 + 1).min(self.visiveis.len());

        let mut itens = Vec::new();
        for posicao in inicio..fim {
            let no_acervo = self.visiveis[posicao];
            let foto = &self.fotos[no_acervo];
            let e_a_selecionada = self.selecionada == Some(no_acervo);

            itens.push(
                div()
                    .id(SharedString::from(format!("faixa-{}", foto.id)))
                    .w(px(56.))
                    .h(px(56.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .border_1()
                    .border_color(if e_a_selecionada {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    })
                    .bg(cx.theme().muted)
                    .child(
                        match self
                            .cache
                            .lock()
                            .expect("o cache de miniaturas não deve estar envenenado")
                            .obter(&self.previews, &foto.id)
                        {
                            Miniatura::Pronta(imagem) => {
                                img(imagem).max_w(px(52.)).max_h(px(52.)).into_any_element()
                            }
                            Miniatura::Ausente => div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("—")
                                .into_any_element(),
                        },
                    )
                    .on_click(cx.listener(
                        move |this: &mut Self,
                              _ev: &gpui::ClickEvent,
                              _window,
                              cx: &mut Context<Self>| {
                            this.selecionada = Some(no_acervo);
                            cx.notify();
                        },
                    ))
                    .into_any_element(),
            );
        }

        let legenda: SharedString = match self.selecionada {
            Some(i) => format!("selecionada: {}", self.fotos[i].name).into(),
            None => "nenhuma foto selecionada".into(),
        };

        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .p(px(8.))
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .w(px(200.))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(legenda),
            )
            .child(div().flex().gap(px(4.)).children(itens))
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
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(rotulo_do_grupo("pastas", cx))
            .children(itens)
    }
}

/// Rótulo cinza que nomeia um grupo de botões.
fn rotulo_do_grupo(texto: &'static str, cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(texto)
}

/// Um botão de filtro, aceso quando é o escolhido.
///
/// `id` próprio em cada um: o GPUI usa o id para saber que este é o mesmo
/// elemento entre quadros. Dois botões com o mesmo id trocariam de estado um
/// com o outro ao serem clicados.
///
/// 🔑 **Devolve `Button`, e não `AnyElement`** — e essa é a diferença que o
/// `gpui-component` fez aparecer. A versão à mão tinha de apagar o tipo porque
/// cada closure é um tipo concreto próprio, e um `Vec` com dois botões de ações
/// diferentes não compilava. O `Button` guarda o handler num `Rc<dyn Fn>`: todos
/// os botões passam a ser **o mesmo tipo**, e a lista volta a ser uma lista.
fn botao(
    id: String,
    texto: String,
    aceso: bool,
    ao_clicar: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> Button {
    Button::new(SharedString::from(id))
        .label(SharedString::from(texto))
        .xsmall()
        // O aceso vira `primary`, e não só `selected`. `selected` num botão
        // secundário é `#3a3a3a` sobre `#2d2d2d` — dois cinzas a 5% de distância
        // um do outro, o que numa barra de 15 botões é o mesmo que não marcar
        // nenhum. Qual filtro está ligado é a informação mais importante da
        // barra: sem ela, a grade filtrada parece um acervo que encolheu.
        .when(aceso, |b| b.primary())
        .selected(aceso)
        .on_click(ao_clicar)
}

/// Uma célula da grade: a miniatura, ou o lugar dela.
fn celula(
    foto: &PhotoViewModel,
    previews: &PreviewManager,
    cache: &Mutex<CacheDeMiniaturas>,
    selecionada: bool,
    ao_clicar: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    cx: &App,
) -> gpui::AnyElement {
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
        .bg(cx.theme().muted)
        .rounded(cx.theme().radius);

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
                .text_color(cx.theme().muted_foreground)
                .child("sem preview"),
        ),
    };

    div()
        .id(SharedString::from(format!("celula-{}", foto.id)))
        .flex()
        .flex_col()
        .gap(px(4.))
        .w(px(LADO_DO_ITEM))
        .p(px(2.))
        .rounded(cx.theme().radius)
        .cursor_pointer()
        // A moldura da seleção é **borda**, e não fundo: fundo colorido atrás
        // de uma foto muda como a foto é percebida, e num programa de revelação
        // isso é mentir sobre a cor. Pela mesma razão a borda é fina.
        .border_1()
        // A borda existe sempre, e some no fundo quando não está selecionada:
        // criá-la só na selecionada deslocaria a foto em 1px ao clicar.
        .border_color(if selecionada {
            cx.theme().primary
        } else {
            cx.theme().background
        })
        .child(conteudo)
        .child(
            div()
                .text_xs()
                .text_color(if selecionada {
                    cx.theme().foreground
                } else {
                    cx.theme().muted_foreground
                })
                .truncate()
                .child(SharedString::from(foto.name.clone())),
        )
        .on_click(ao_clicar)
        .into_any_element()
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
        let selecionada = self.selecionada;
        // O closure do `uniform_list` é `'static` e recebe `&mut App`, não
        // `&mut self` — para escrever no estado a partir dele, é preciso levar
        // uma referência à entidade e pedir a ela que se atualize.
        let eu = cx.entity();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
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
                        uniform_list("grade-da-biblioteca", linhas, move |faixa, _window, cx| {
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
                                                let no_acervo = visiveis[posicao];
                                                let eu = eu.clone();
                                                celula(
                                                    &fotos[no_acervo],
                                                    &previews,
                                                    &cache,
                                                    selecionada == Some(no_acervo),
                                                    move |_ev, _window, cx| {
                                                        eu.update(cx, |tela, cx| {
                                                            // Clicar na já
                                                            // selecionada
                                                            // desmarca: é como
                                                            // se desfaz sem
                                                            // procurar botão.
                                                            tela.selecionada = if tela.selecionada
                                                                == Some(no_acervo)
                                                            {
                                                                None
                                                            } else {
                                                                Some(no_acervo)
                                                            };
                                                            cx.notify();
                                                        });
                                                    },
                                                    cx,
                                                )
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
            .child(self.filmstrip(cx))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui::TestAppContext;
    use tempfile::TempDir;

    /// O primeiro teste de tela do projeto em `TestAppContext`.
    ///
    /// É o substituto que o plano escolheu para os 146 testes de UI que morrem
    /// na fase 5 (docs/10-MIGRACAO-GPUI.md §6): não é snapshot visual — dirige a
    /// janela e afirma sobre o **estado**. E é justamente o estado que a solda
    /// entre componente e tela pode perder sem que nada falhe.
    ///
    /// ⚠️ Nunca `PreviewManager::new()` num teste: aquele resolve
    /// `AppPaths::preview_cache_dir()` e escreve na biblioteca de fotos de quem
    /// rodar a suíte — o defeito que a fase 0 encontrou.
    fn previews_descartaveis() -> (Arc<PreviewManager>, TempDir) {
        let dir = TempDir::new().expect("criar diretório temporário");
        (
            Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf())),
            dir,
        )
    }

    fn foto(nome: &str) -> PhotoViewModel {
        PhotoViewModel {
            id: format!("id-{nome}"),
            name: nome.to_string(),
            path: format!("/fotos/{nome}"),
            ..Default::default()
        }
    }

    fn acervo() -> Vec<PhotoViewModel> {
        vec![
            foto("DSC_001.NEF"),
            foto("DSC_002.NEF"),
            foto("retrato.jpg"),
        ]
    }

    fn nomes_visiveis(tela: &Biblioteca) -> Vec<String> {
        tela.visiveis
            .iter()
            .map(|&i| tela.fotos[i].name.clone())
            .collect()
    }

    /// 🚨 Digitar na busca filtra a grade.
    ///
    /// Parece óbvio, e é exatamente por isso que precisa de teste: o caminho
    /// inteiro depende de uma `Subscription` guardada num campo da struct. Um
    /// `let _ = cx.subscribe(...)` cancela a inscrição na hora, compila, roda, e
    /// o campo aceita texto normalmente **sem filtrar nada** — sem erro, sem
    /// aviso, sem sintoma além de "a busca não funciona".
    ///
    /// São ~50 controles com essa mesma forma de solda na Revelação. Este teste
    /// é o molde deles.
    #[gpui::test]
    fn digitar_na_busca_filtra_a_grade(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        // O `InputState` lê estado global que só o `init` cria — sem esta linha
        // o teste morre antes da primeira asserção.
        cx.update(gpui_component::init);

        let janela =
            cx.add_window(|window, cx| Biblioteca::nova(acervo(), previews.clone(), window, cx));

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(
                    nomes_visiveis(tela).len(),
                    3,
                    "sem busca, a grade mostra o acervo inteiro"
                );
            })
            .expect("a janela deve estar aberta");

        janela
            .update(cx, |tela, window, cx| {
                tela.busca
                    .update(cx, |campo, cx| campo.set_value("RETRATO", window, cx));
            })
            .expect("a janela deve estar aberta");
        // O `cx.emit` do componente enfileira um efeito; ele só chega ao
        // assinante quando a fila drena. Sem isto o teste leria o estado de
        // antes da digitação e passaria por engano.
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                // Maiúsculas de propósito: a busca não diferencia caixa, e é o
                // `filtros.rs` que garante isso. O que se confere aqui é que o
                // texto **chegou** até ele.
                assert_eq!(nomes_visiveis(tela), vec!["retrato.jpg"]);
            })
            .expect("a janela deve estar aberta");
    }

    /// Apagar a busca devolve o acervo inteiro.
    ///
    /// A ida sem a volta deixaria passar uma inscrição que dispara uma vez só —
    /// e o sintoma seria uma grade que nunca mais destrava.
    ///
    /// ⚠️ A asserção do meio não é decoração. Sem ela este teste **passa com a
    /// inscrição cancelada**: se nada nunca filtra, a grade tem as três fotos no
    /// fim, que é exatamente o que ele cobra. Foi o que apareceu ao quebrar o
    /// código de propósito para conferir se o teste falhava — o primeiro falhou,
    /// este não. Teste de volta precisa provar que houve ida.
    #[gpui::test]
    fn limpar_a_busca_devolve_o_acervo(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);

        let janela =
            cx.add_window(|window, cx| Biblioteca::nova(acervo(), previews.clone(), window, cx));

        janela
            .update(cx, |tela, window, cx| {
                tela.busca
                    .update(cx, |campo, cx| campo.set_value("retrato", window, cx));
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(
                    nomes_visiveis(tela),
                    vec!["retrato.jpg"],
                    "sem a ida, a volta não prova nada"
                );
            })
            .expect("a janela deve estar aberta");

        janela
            .update(cx, |tela, window, cx| {
                tela.busca
                    .update(cx, |campo, cx| campo.set_value("", window, cx));
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(nomes_visiveis(tela).len(), 3);
            })
            .expect("a janela deve estar aberta");
    }
}
