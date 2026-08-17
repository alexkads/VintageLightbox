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
use super::informacoes::{estatisticas, estrelas};
use super::marcacao::{cor_ao_teclar, sinalizador_ao_teclar, Marca, Marcador};
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

/// Largura da coluna de informações, à direita.
///
/// É o painel `Metadata` do dock do legado, mais a parte legível do `Quick
/// Develop` — nota e cor da foto. Aqui ele é fixo, e lá é uma aba arrastável
/// (a decisão está registrada em docs/10-MIGRACAO-GPUI.md, fase 4).
const LADO_DAS_INFORMACOES: f32 = 240.0;

pub struct Biblioteca {
    /// `Arc` porque o closure do `uniform_list` é `'static` e precisa levar as
    /// fotos consigo — clonar o `Vec` a cada quadro seria copiar o acervo
    /// inteiro 60 vezes por segundo.
    fotos: Arc<Vec<PhotoViewModel>>,
    previews: Arc<PreviewManager>,
    /// Quem grava nota, cor e sinalizador — as treze teclas de triagem.
    marcador: Arc<dyn Marcador>,
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
    /// Índice **no acervo** da foto principal — a que a Revelação abre e a que
    /// o filmstrip centraliza.
    ///
    /// No acervo e não na lista filtrada: mudar de filtro não pode trocar qual
    /// foto está selecionada. Guardando a posição filtrada, escolher outra
    /// pasta manteria o índice e selecionaria uma foto diferente sem ninguém
    /// clicar em nada.
    selecionada: Option<usize>,
    /// Todas as selecionadas, incluindo a principal.
    ///
    /// 🔑 **`BTreeSet`, e não `HashSet` como o legado.** Lá a ordem de saída é a
    /// do hash, e ela **vaza para a folha de impressão**: entrar na Impressão com
    /// dez fotos selecionadas monta a coleção em ordem de hash, e a posição na
    /// lista é o que decide em qual célula cada foto cai. Ordenado, "as
    /// selecionadas" saem sempre na ordem do acervo.
    selecionadas: std::collections::BTreeSet<usize>,
    /// De onde o próximo `Shift+clique` mede o intervalo.
    ancora: Option<usize>,
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
    // 🚨 **As duas colunas entram na conta.** A grade fica entre elas, e contar
    // só uma faria `colunas_que_cabem` responder mais colunas do que cabem — o
    // sintoma é a última coluna cortada pela borda, que é o mesmo defeito que a
    // árvore de pastas causou quando entrou.
    f32::from(window.viewport_size().width) - MARGEM_LATERAL - LADO_DA_ARVORE - LADO_DAS_INFORMACOES
}

impl Biblioteca {
    pub fn nova(
        fotos: Vec<PhotoViewModel>,
        previews: Arc<PreviewManager>,
        marcador: Arc<dyn Marcador>,
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
            marcador,
            // Nasce do tamanho da janela padrão e se ajusta no primeiro
            // `render`, quando a janela de verdade já foi medida.
            cache: Arc::new(Mutex::new(CacheDeMiniaturas::nova(capacidade_para(6, 4)))),
            filtros: Filtros::default(),
            visiveis: Vec::new(),
            pastas: Vec::new(),
            selecionada: None,
            selecionadas: std::collections::BTreeSet::new(),
            ancora: None,
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

    /// A foto selecionada, para quem está de fora.
    ///
    /// Devolve uma cópia, e não uma referência: quem pergunta é a raiz, para
    /// levar a foto até a Revelação, e o que ela leva tem de continuar valendo
    /// depois que a grade mudar de filtro.
    pub fn foto_selecionada(&self) -> Option<PhotoViewModel> {
        self.selecionada.map(|i| self.fotos[i].clone())
    }

    /// Os ids das selecionadas, **na ordem do acervo**.
    ///
    /// É o que a Impressão recebe para montar a coleção — e a ordem importa,
    /// porque a posição na lista decide em qual célula da folha cada foto cai.
    /// No legado esta lista sai de um `HashSet`, em ordem de hash.
    pub fn ids_selecionados(&self) -> Vec<String> {
        self.selecionadas
            .iter()
            .map(|&i| self.fotos[i].id.clone())
            .collect()
    }

    /// As fotos que a grade está mostrando — já filtradas.
    ///
    /// É o que a Impressão leva ao ser aberta, e é o mesmo recorte que o legado
    /// usa lá (`filmstrip_filter.apply(&state.photos)`): "Todas" quer dizer
    /// todas as que se estava vendo, e não o acervo inteiro por trás do filtro.
    ///
    /// Devolve cópias pelo mesmo motivo de [`Self::foto_selecionada`]: quem
    /// recebe guarda, e o que guardou tem de continuar valendo depois que a
    /// grade mudar de filtro.
    pub fn fotos_visiveis(&self) -> Vec<PhotoViewModel> {
        self.visiveis
            .iter()
            .map(|&i| self.fotos[i].clone())
            .collect()
    }

    /// Põe o foco no campo de busca, para os testes da raiz.
    ///
    /// Existe por causa de um defeito real: quem busca uma foto deixa o foco no
    /// campo, e o campo some da tela ao entrar na Revelação — o caminho de foco
    /// fica apontando um elemento que não é mais renderizado, e as teclas da raiz
    /// param de chegar.
    #[cfg(test)]
    pub fn focar_busca(&self, window: &mut Window, cx: &mut gpui::App) {
        use gpui::Focusable;
        window.focus(&self.busca.read(cx).focus_handle(cx));
    }

    #[cfg(test)]
    pub fn texto_da_busca(&self, cx: &gpui::App) -> String {
        self.busca.read(cx).value().to_string()
    }

    /// Quantas fotos estão selecionadas.
    pub fn quantas_selecionadas(&self) -> usize {
        self.selecionadas.len()
    }

    /// Seleciona **uma só**, por índice no acervo — o clique sem modificador.
    ///
    /// `None` limpa tudo. Selecionar uma foto joga fora as outras, como no
    /// legado (`single_select`): clique sem modificador é "quero esta", e manter
    /// as anteriores faria a próxima tecla de nota cair em fotos que quem
    /// clicou já tinha esquecido.
    pub fn selecionar(&mut self, no_acervo: Option<usize>, cx: &mut Context<Self>) {
        self.selecionada = no_acervo;
        self.selecionadas = no_acervo.into_iter().collect();
        self.ancora = no_acervo;
        cx.notify();
    }

    /// `Cmd+clique`: põe ou tira uma foto da seleção, sem tocar nas outras.
    pub fn alternar_uma(&mut self, no_acervo: usize, cx: &mut Context<Self>) {
        if !self.selecionadas.remove(&no_acervo) {
            self.selecionadas.insert(no_acervo);
        }
        // A âncora e a principal andam com o último clique, mesmo quando ele
        // **tirou** a foto da seleção: é dali que o próximo `Shift+clique` mede.
        self.ancora = Some(no_acervo);
        self.selecionada = Some(no_acervo);
        cx.notify();
    }

    /// `Shift+clique`: acrescenta o intervalo entre a âncora e esta foto.
    ///
    /// 🚨 **O intervalo é contado na lista filtrada, e no legado é no acervo.**
    /// Lá o índice vem da grade (que enumera o filtrado) e o `select_range`
    /// indexa `state.photos` (o acervo inteiro): com qualquer filtro ligado,
    /// `Shift+clique` seleciona **outras fotos** — as que ocupam aquelas posições
    /// no acervo. Nada falha; a grade só marca células que ninguém apontou, e
    /// algumas das marcadas nem estão na tela. É a mesma armadilha que a grade da
    /// fase 1 encontrou (célula mostrando a foto errada com filtro ativo).
    pub fn selecionar_ate(&mut self, no_acervo: usize, cx: &mut Context<Self>) {
        let posicao_atual = self.visiveis.iter().position(|&i| i == no_acervo);
        let posicao_ancora = self
            .ancora
            .and_then(|ancora| self.visiveis.iter().position(|&i| i == ancora));

        match (posicao_atual, posicao_ancora) {
            (Some(atual), Some(ancora)) => {
                let (inicio, fim) = (atual.min(ancora), atual.max(ancora));
                for posicao in inicio..=fim {
                    self.selecionadas.insert(self.visiveis[posicao]);
                }
            }
            // Sem âncora — ou com a âncora fora do filtro atual — o Shift vale
            // como um clique comum sobre esta foto, que é o que o legado faz.
            _ => {
                self.selecionadas.insert(no_acervo);
                self.ancora = Some(no_acervo);
            }
        }

        self.selecionada = Some(no_acervo);
        cx.notify();
    }

    /// `Cmd+A`: tudo que está **na grade**.
    ///
    /// 🚨 **No legado é o acervo inteiro** (`select_all` percorre `state.photos`),
    /// ignorando o filtro — então `Cmd+A` com "★★★ ou mais" ligado seleciona
    /// também as de uma estrela, que não estão na tela, e a próxima tecla de nota
    /// cai em todas elas. O próprio legado se contradiz: o "Select All" do módulo
    /// de impressão usa a lista filtrada.
    pub fn selecionar_tudo(&mut self, cx: &mut Context<Self>) {
        self.selecionadas = self.visiveis.iter().copied().collect();
        // A principal continua sendo a que já era, se ela sobreviveu ao filtro;
        // senão, a primeira da grade. Sem isto, `Cmd+A` deixaria a Revelação sem
        // saber qual abrir.
        if self
            .selecionada
            .is_none_or(|i| !self.selecionadas.contains(&i))
        {
            self.selecionada = self.visiveis.first().copied();
        }
        self.ancora = self.selecionada;
        cx.notify();
    }

    /// `Cmd+D`: limpa a seleção inteira.
    pub fn limpar_selecao(&mut self, cx: &mut Context<Self>) {
        self.selecionar(None, cx);
    }

    /// As setas: um passo na lista **filtrada**, sem dar a volta.
    ///
    /// 🔑 **Anda na lista filtrada e guarda o índice do acervo** — os dois
    /// espaços de índice do arquivo, e a razão de `selecionada` ser no acervo.
    /// Andar sobre o acervo pularia para fotos que não estão na tela, e o
    /// sintoma seria a seleção sumindo da grade a cada seta.
    ///
    /// Sem seleção, a primeira seta escolhe a primeira (adiante) ou a última
    /// (atrás) — é o que o legado faz (`navigate_library`), e é o que permite
    /// começar a triagem sem tocar no ponteiro.
    ///
    /// ⚠️ **Não dá a volta**, também como no legado: chegar ao fim e continuar
    /// apertando fica no fim. Numa triagem longa, voltar ao começo sem aviso
    /// faria retrabalhar as primeiras sem perceber.
    pub fn andar(&mut self, passo: i32, cx: &mut Context<Self>) {
        if self.visiveis.is_empty() {
            return;
        }

        let posicao = self
            .selecionada
            .and_then(|no_acervo| self.visiveis.iter().position(|&i| i == no_acervo));

        let nova = match posicao {
            Some(atual) if passo > 0 => (atual + 1).min(self.visiveis.len() - 1),
            Some(atual) => atual.saturating_sub(1),
            None if passo > 0 => 0,
            None => self.visiveis.len() - 1,
        };

        self.selecionar(Some(self.visiveis[nova]), cx);
    }

    /// A nota das selecionadas — `0` a `5`, absoluta.
    pub fn dar_nota(&mut self, nota: i32, cx: &mut Context<Self>) {
        self.aplicar(Marca::Nota(nota), cx);
    }

    /// A cor das selecionadas — e a mesma cor de novo tira a cor.
    ///
    /// 🔑 **A decisão é do grupo inteiro**: só tira a cor se **todas** já
    /// estiverem com ela. É o que o legado faz (`all_already_have_color`), e é o
    /// que evita uma tecla deixar metade das fotos amarelas e a outra metade sem
    /// cor.
    pub fn dar_cor(&mut self, cor: &str, cx: &mut Context<Self>) {
        let todas_ja_tem = self.selecionadas.iter().all(|&i| {
            self.fotos[i]
                .color_label
                .as_deref()
                .is_some_and(|atual| atual.eq_ignore_ascii_case(cor))
        });

        let nova = cor_ao_teclar(todas_ja_tem.then_some(cor), cor);
        self.aplicar(Marca::Cor(nova), cx);
    }

    /// O sinalizador das selecionadas — `1`, `-1` ou `0`.
    ///
    /// 🚨 **A decisão também é do grupo, e no legado é foto a foto.** Lá o
    /// `handle_flag_shortcuts` calcula a alternância **dentro do laço**: com três
    /// fotos selecionadas e uma já escolhida, apertar `P` **desmarca aquela** e
    /// marca as outras duas — uma tecla, dois desfechos opostos no mesmo gesto, e
    /// nenhum jeito de prever qual sai. Aqui a regra é a mesma da cor: só
    /// desmarca se todas já estiverem com o sinalizador pedido.
    ///
    /// ⚠️ Com uma foto só — o caso comum — as duas regras dão o mesmo resultado.
    /// A divergência só aparece em lote, que é o que esta entrega trouxe.
    pub fn sinalizar(&mut self, pedido: i32, cx: &mut Context<Self>) {
        let todas_ja_tem = self
            .selecionadas
            .iter()
            .all(|&i| self.fotos[i].flag.unwrap_or(0) == pedido);

        let codigo = sinalizador_ao_teclar(todas_ja_tem.then_some(pedido), pedido);
        self.aplicar(Marca::Sinalizador(codigo), cx);
    }

    /// Escreve na foto que está na memória **e** manda gravar.
    ///
    /// 🔑 **A tela muda antes do banco responder**, como no legado ("optimistic
    /// update"). Numa triagem se aperta tecla mais rápido do que um `UPDATE`
    /// volta, e esperar faria a nota aparecer depois da foto seguinte já estar
    /// selecionada — o número certo na foto errada, do ponto de vista de quem
    /// olha.
    ///
    /// ⚠️ **E refiltra.** Com "★★★ ou mais" ligado, baixar uma foto para 1 tira
    /// ela da grade na hora: é o que o legado faz ao recarregar o acervo, e é o
    /// comportamento que se quer — a grade mostra o que passa no filtro, e a
    /// foto acabou de deixar de passar.
    fn aplicar(&mut self, marca: Marca, cx: &mut Context<Self>) {
        if self.selecionadas.is_empty() {
            return;
        }

        let fotos = Arc::make_mut(&mut self.fotos);
        for &no_acervo in &self.selecionadas {
            let foto = &mut fotos[no_acervo];

            match &marca {
                Marca::Nota(nota) => foto.rating = *nota,
                Marca::Cor(cor) => foto.color_label = cor.clone(),
                Marca::Sinalizador(codigo) => foto.flag = Some(*codigo),
            }

            self.marcador.marcar(foto.id.clone(), marca.clone());
        }

        self.refiltrar();
        cx.notify();
    }

    /// O clique sem modificador.
    ///
    /// Seleciona **só** esta — e clicar na que já era a única selecionada
    /// desmarca, que é como se desfaz sem procurar botão. ⚠️ Com várias
    /// selecionadas, clicar numa delas **não** desmarca tudo: encolhe a seleção
    /// para a que foi clicada. Desmarcar as cinco por engano ao tentar escolher
    /// uma delas é o desfecho que ninguém quer, e desfazer isso é reselecionar
    /// tudo de novo.
    pub fn alternar_selecao(&mut self, no_acervo: usize, cx: &mut Context<Self>) {
        let unica_e_esta = self.selecionadas.len() == 1 && self.selecionadas.contains(&no_acervo);
        let alvo = if unica_e_esta { None } else { Some(no_acervo) };
        self.selecionar(alvo, cx);
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

        // 🔑 **Quantas estão selecionadas só aparece a partir de duas.** Com uma
        // só, a moldura na grade já diz tudo; o número existe para quando a
        // seleção não cabe na tela — e uma tecla de nota vai cair em todas elas.
        let selecao: Option<SharedString> = (self.selecionadas.len() > 1)
            .then(|| format!("{} selecionadas", self.selecionadas.len()).into());

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
                    )
                    .children(selecao.map(|quantas| {
                        div()
                            .text_sm()
                            .text_color(cx.theme().primary)
                            .child(quantas)
                    })),
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

    /// O painel da direita: a foto selecionada e o retrato do acervo.
    ///
    /// Junta os dois painéis que no legado são abas do dock — `Metadata` (info da
    /// foto e estatísticas) e a parte legível do `Quick Develop` (nota e cor).
    ///
    /// 🚨 **O `Quick Develop` de lá promete mais do que faz**: ele desenha a
    /// fileira de cores com `ColorLabels::show(ui, &None, false)` — a cor da foto
    /// **não** é passada, e o `false` desliga o clique. A fileira aparece sempre
    /// vazia e não responde a nada, num painel que o nome diz ser para revelar
    /// rápido. Aqui a cor é a da foto, e continua sem clique: quem marca cor são
    /// as teclas `6`–`9`.
    fn painel_de_informacoes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let foto = self.selecionada.map(|i| &self.fotos[i]);
        let numeros = estatisticas(&self.fotos);
        let maior = numeros.por_nota.iter().copied().max().unwrap_or(0).max(1);

        div()
            .w(px(LADO_DAS_INFORMACOES))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(12.))
            .border_l_1()
            .border_color(cx.theme().border)
            .child(rotulo_do_grupo("foto", cx))
            .child(match foto {
                Some(foto) => div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .text_xs()
                    .child(
                        div()
                            .truncate()
                            .child(SharedString::from(foto.name.clone())),
                    )
                    .child(linha_de_dado("data", &foto.date, cx))
                    .child(linha_de_dado("câmera", &foto.camera, cx))
                    .child(linha_de_dado("exposição", &foto.exposure, cx))
                    .child(linha_de_dado("nota", &estrelas(foto.rating), cx))
                    .child(linha_de_dado(
                        "cor",
                        foto.color_label.as_deref().unwrap_or("—"),
                        cx,
                    ))
                    .into_any_element(),
                None => div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Nenhuma foto selecionada")
                    .into_any_element(),
            })
            .child(rotulo_do_grupo("acervo", cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    // Seis barras são seis `div`s, e isso é barato — o histograma
                    // da Revelação virou `canvas` porque lá são 768. A régua é o
                    // número de nós por quadro, não o desenho ser um gráfico.
                    .children((0..=5).rev().map(|nota: usize| {
                        let quantas = numeros.por_nota[nota];
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_xs()
                            .child(
                                div()
                                    .w(px(46.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(SharedString::from(if nota == 0 {
                                        "sem nota".to_string()
                                    } else {
                                        estrelas(nota as i32)
                                    })),
                            )
                            .child(
                                div()
                                    .h(px(8.))
                                    // A barra é proporcional à **maior** contagem,
                                    // e não ao total: com 2.000 fotos e 12 de
                                    // cinco estrelas, proporcional ao total todas
                                    // as barras seriam um fio.
                                    .w(px(120.0 * quantas as f32 / maior as f32))
                                    .min_w(px(1.))
                                    .rounded(px(2.))
                                    .bg(cx.theme().primary),
                            )
                            .child(
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(SharedString::from(quantas.to_string())),
                            )
                    })),
            )
            .child(rotulo_do_grupo("câmeras", cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .children(if numeros.cameras.is_empty() {
                        vec![div().child("nenhuma câmera conhecida").into_any_element()]
                    } else {
                        numeros
                            .cameras
                            .iter()
                            // ⚠️ **As cinco aparecem com nome.** O legado desenha
                            // cinco barras e escreve só três nomes embaixo — as
                            // duas últimas ficam anônimas num gráfico sem rótulo
                            // de eixo.
                            .map(|(nome, quantas)| {
                                div()
                                    .flex()
                                    .justify_between()
                                    .gap(px(6.))
                                    .child(div().truncate().child(SharedString::from(nome.clone())))
                                    .child(SharedString::from(quantas.to_string()))
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>()
                    }),
            )
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
                            // A faixa **seleciona**, não alterna: quem clica no
                            // filmstrip está navegando entre vizinhas, e
                            // desmarcar no meio disso esvaziaria a própria faixa.
                            this.selecionar(Some(no_acervo), cx);
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

/// Uma linha "rótulo: valor" do painel de informações.
///
/// Vazio vira travessão, e não linha em branco: o campo existir e estar vazio é
/// informação — o EXIF não trouxe aquilo — e uma linha em branco parece falha de
/// desenho.
fn linha_de_dado(rotulo: &'static str, valor: &str, cx: &App) -> impl IntoElement {
    let valor = if valor.trim().is_empty() {
        "—".to_string()
    } else {
        valor.to_string()
    };

    div()
        .flex()
        .justify_between()
        .gap(px(6.))
        .child(div().text_color(cx.theme().muted_foreground).child(rotulo))
        .child(div().truncate().child(SharedString::from(valor)))
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
    // A principal — a que a Revelação abre. Com uma foto só selecionada as duas
    // são a mesma; em lote, é a última clicada, e sem distingui-la ninguém sabe
    // qual das dez vai abrir ao apertar "Revelação".
    principal: bool,
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
        //
        // 🔑 **Duas forças de marca, e não uma.** A principal é a que a Revelação
        // abre; as outras da seleção compartilham as teclas de triagem. Marcando
        // as dez igual, apertar "Revelação" com dez selecionadas abre uma delas
        // sem que nada na tela tivesse dito qual.
        .border_color(match (selecionada, principal) {
            (_, true) => cx.theme().primary,
            (true, false) => cx.theme().primary.opacity(0.45),
            (false, false) => cx.theme().background,
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
        let principal = self.selecionada;
        // A seleção inteira vai para o closure `'static` do `uniform_list`.
        let selecionadas = Arc::new(self.selecionadas.clone());
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
                                                    selecionadas.contains(&no_acervo),
                                                    principal == Some(no_acervo),
                                                    // 🔑 Os modificadores vêm do
                                                    // `ClickEvent`, e é por isso que
                                                    // o clique fica na célula
                                                    // inteira: `Cmd` alterna uma,
                                                    // `Shift` estende o intervalo, e
                                                    // sem eles é seleção única.
                                                    move |evento: &gpui::ClickEvent,
                                                          _window,
                                                          cx: &mut gpui::App| {
                                                        let modificadores =
                                                            evento.modifiers();
                                                        eu.update(cx, |tela, cx| {
                                                            if modificadores.secondary() {
                                                                tela.alternar_uma(no_acervo, cx);
                                                            } else if modificadores.shift {
                                                                tela.selecionar_ate(no_acervo, cx);
                                                            } else {
                                                                tela.alternar_selecao(
                                                                    no_acervo, cx,
                                                                );
                                                            }
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
                    )
                    .child(self.painel_de_informacoes(cx)),
            )
            .child(self.filmstrip(cx))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use super::super::marcacao::mentira::MarcadorDeMentira;

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
    /// O marcador que não grava nada — quase todo teste daqui não tria.
    fn marcador() -> Arc<dyn Marcador> {
        Arc::new(MarcadorDeMentira::default())
    }

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

    /// Um acervo de dez, com nota crescente — dá para filtrar e ainda sobrar
    /// intervalo para o `Shift+clique` medir.
    fn acervo_grande() -> Vec<PhotoViewModel> {
        (0..10)
            .map(|i| PhotoViewModel {
                rating: i % 5,
                ..foto(&format!("{i:02}.jpg"))
            })
            .collect()
    }

    fn tela_com(
        cx: &mut TestAppContext,
        fotos: Vec<PhotoViewModel>,
    ) -> (
        gpui::WindowHandle<Biblioteca>,
        Arc<MarcadorDeMentira>,
        TempDir,
    ) {
        let (previews, dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let marcador = Arc::new(MarcadorDeMentira::default());
        let janela = cx.add_window({
            let marcador = marcador.clone();
            |window, cx| Biblioteca::nova(fotos, previews, marcador, window, cx)
        });
        (janela, marcador, dir)
    }

    /// 🚨 A grade desconta **as duas** colunas laterais.
    ///
    /// Contar só a das pastas faria `colunas_que_cabem` responder mais colunas do
    /// que cabem, e o sintoma é a última coluna cortada pela borda — o mesmo
    /// defeito que a árvore de pastas causou quando entrou. Sem este teste, o
    /// painel novo passaria despercebido até alguém abrir numa janela estreita.
    #[gpui::test]
    fn a_grade_desconta_as_duas_colunas_laterais(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());
        let visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_resize(gpui::size(px(1200.), px(800.)));

        janela
            .update(cx, |_tela, window, _cx| {
                let util = largura_util(window);
                assert!(
                    util <= 1200.0 - LADO_DA_ARVORE - LADO_DAS_INFORMACOES,
                    "a largura útil tem de caber entre as duas colunas: {util}"
                );
                assert!(util > 0.0);
            })
            .expect("a janela deve estar aberta");
    }

    /// `Cmd+clique` põe e tira uma foto sem tocar nas outras.
    #[gpui::test]
    fn cmd_clique_alterna_uma_so(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(0), cx);
                tela.alternar_uma(3, cx);
                tela.alternar_uma(7, cx);
                assert_eq!(tela.quantas_selecionadas(), 3);

                tela.alternar_uma(3, cx);
                assert_eq!(tela.quantas_selecionadas(), 2);
                assert_eq!(
                    tela.ids_selecionados(),
                    vec!["id-00.jpg".to_string(), "id-07.jpg".to_string()],
                    "e saem na ordem do acervo, não na de clique nem na do hash"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 `Shift+clique` conta o intervalo na **grade**, e o legado conta no
    /// acervo.
    ///
    /// Lá o índice vem da grade (que enumera o filtrado) e o `select_range`
    /// indexa `state.photos`: com filtro ligado, ele seleciona as fotos que
    /// ocupam aquelas posições no acervo — outras fotos, algumas nem visíveis.
    /// Nada falha; a grade marca células que ninguém apontou.
    #[gpui::test]
    fn shift_clique_estende_pelo_que_esta_na_grade(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, _window, cx| {
                // Só nota 4: as fotos 4 e 9 do acervo.
                // O filtro é escrito direto: quem o liga na tela é um botão da
                // barra, e o que este teste mede é o efeito dele na seleção.
                tela.filtros.nota_minima = NotaMinima(4);
                tela.refiltrar();
                assert_eq!(nomes_visiveis(tela), vec!["04.jpg", "09.jpg"]);

                tela.selecionar(Some(4), cx);
                tela.selecionar_ate(9, cx);

                assert_eq!(
                    tela.ids_selecionados(),
                    vec!["id-04.jpg".to_string(), "id-09.jpg".to_string()],
                    "as duas da grade — e não as seis do intervalo 4..=9 do acervo"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 `Cmd+A` seleciona o que está **na grade**, e o legado seleciona o
    /// acervo inteiro.
    ///
    /// Com filtro ligado, o `select_all` de lá marca também as que não estão na
    /// tela — e a tecla de nota seguinte cai em todas elas. O próprio legado se
    /// contradiz: o "Select All" do módulo de impressão respeita o filtro.
    #[gpui::test]
    fn selecionar_tudo_pega_so_o_que_esta_na_grade(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, _window, cx| {
                // O filtro é escrito direto: quem o liga na tela é um botão da
                // barra, e o que este teste mede é o efeito dele na seleção.
                tela.filtros.nota_minima = NotaMinima(4);
                tela.refiltrar();
                tela.selecionar_tudo(cx);

                assert_eq!(tela.quantas_selecionadas(), 2, "e não as dez do acervo");
                assert!(
                    tela.foto_selecionada().is_some(),
                    "e alguém continua sendo a principal, senão a Revelação não sabe o que abrir"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A tecla de nota vale para a seleção inteira — e grava uma vez por foto.
    #[gpui::test]
    fn a_nota_vale_para_todas_as_selecionadas(cx: &mut TestAppContext) {
        let (janela, marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(1), cx);
                tela.alternar_uma(2, cx);
                tela.alternar_uma(3, cx);

                tela.dar_nota(5, cx);

                for i in 1..=3 {
                    assert_eq!(tela.fotos[i].rating, 5);
                }
            })
            .expect("a janela deve estar aberta");

        let marcado = marcador.marcado();
        assert_eq!(marcado.len(), 3, "uma gravação por foto");
        assert!(marcado.iter().all(|(_, marca)| *marca == Marca::Nota(5)));
    }

    /// 🚨 O sinalizador decide pelo **grupo** — no legado ele decide foto a foto.
    ///
    /// Lá, com três selecionadas e uma já escolhida, `P` desmarca aquela e marca
    /// as outras duas: uma tecla, dois desfechos opostos no mesmo gesto. Aqui só
    /// desmarca quando **todas** já estão escolhidas.
    #[gpui::test]
    fn o_sinalizador_decide_pelo_grupo_inteiro(cx: &mut TestAppContext) {
        let mut fotos = acervo_grande();
        fotos[1].flag = Some(1);
        let (janela, _marcador, _dir) = tela_com(cx, fotos);

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(1), cx);
                tela.alternar_uma(2, cx);
                tela.alternar_uma(3, cx);

                tela.sinalizar(1, cx);
                assert_eq!(
                    (tela.fotos[1].flag, tela.fotos[2].flag, tela.fotos[3].flag),
                    (Some(1), Some(1), Some(1)),
                    "nem todas estavam escolhidas: a tecla escolhe as três"
                );

                // Agora todas estão — e a mesma tecla desmarca as três juntas.
                tela.sinalizar(1, cx);
                assert_eq!(
                    (tela.fotos[1].flag, tela.fotos[2].flag, tela.fotos[3].flag),
                    (Some(0), Some(0), Some(0))
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ Clicar numa das selecionadas **encolhe** a seleção para ela.
    ///
    /// Só desmarca quando ela já era a única. Desmarcar as cinco por engano ao
    /// tentar escolher uma delas é o desfecho que ninguém quer, e desfazer isso
    /// é reselecionar tudo de novo.
    #[gpui::test]
    fn clicar_numa_das_selecionadas_encolhe_em_vez_de_limpar(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(1), cx);
                tela.alternar_uma(2, cx);
                tela.alternar_uma(3, cx);

                tela.alternar_selecao(2, cx);
                assert_eq!(tela.quantas_selecionadas(), 1);
                assert_eq!(tela.ids_selecionados(), vec!["id-02.jpg".to_string()]);

                // E de novo na mesma, agora única: aí sim desmarca.
                tela.alternar_selecao(2, cx);
                assert_eq!(tela.quantas_selecionadas(), 0);
            })
            .expect("a janela deve estar aberta");
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

        let janela = cx.add_window(|window, cx| {
            Biblioteca::nova(acervo(), previews.clone(), marcador(), window, cx)
        });

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

        let janela = cx.add_window(|window, cx| {
            Biblioteca::nova(acervo(), previews.clone(), marcador(), window, cx)
        });

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
