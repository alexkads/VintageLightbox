//! A grade da Biblioteca desenhada.
//!
//! Junta as três peças: as fotos que o `LibraryController` traz do catálogo, as
//! contas de [`super::grade`] e as miniaturas de [`super::miniaturas`].

use std::sync::{Arc, Mutex};

use adapters::view_models::PhotoViewModel;
use gpui::{
    div, img, prelude::*, px, uniform_list, AnyElement, App, Context, Entity, SharedString,
    Subscription, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::dock::{register_panel, DockArea, DockEvent, DockItem, PanelView};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, Selectable, Sizable};
use infrastructure::cache::preview_manager::PreviewManager;

use super::arranjo;
use super::colecoes;
use biblioteca_core::selecao::{Modificadores, Selecao};

use super::filtros::{indices_visiveis, FiltroDeCompra, FiltroDeSinalizador, Filtros, NotaMinima};
use super::grade::{colunas_que_cabem, fotos_da_linha, linhas_necessarias};
use super::informacoes::{estatisticas, estrelas};
use super::marcacao::{cor_ao_teclar, sinalizador_ao_teclar, Marca, Marcador};
use super::miniaturas::{capacidade_para, CacheDeMiniaturas, Miniatura};
use super::paineis::{PainelDaBiblioteca, Qual};
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

/// Quanto se espera antes de gravar o arranjo, em milissegundos.
///
/// Os mesmos 500 ms da gravação de ajustes da Revelação — arrastar uma divisória
/// emite dezenas de eventos por segundo, e sem a espera cada um seria um arquivo
/// escrito.
const ESPERA_DO_ARRANJO_MS: u64 = 500;

/// Altura da faixa de miniaturas do rodapé.
const ALTURA_DO_FILMSTRIP: f32 = 84.0;

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
    /// As coleções: a lista lateral, e qual está aberta.
    colecoes: colecoes::Estado,
    /// O ensaio que a grade está mostrando. `None` é o catálogo inteiro.
    ///
    /// 🚨 **É o que faz a sessão ser o lugar de trabalho.** Logado, é dentro do
    /// ensaio que se revela e se escolhe com o cliente — e a grade tem de ser a
    /// dele, não a de todos os clientes juntos. Sem este recorte, a triagem de
    /// um casamento aconteceria no meio das fotos de outros três.
    ///
    /// `None` continua existindo, e é o modo offline: sem conta não há ensaio, e
    /// o app volta a ser o catálogo local de sempre.
    sessao: Option<String>,
    /// Uma frase para quem está olhando a grade.
    ///
    /// 🔑 **Quem escreve aqui é a raiz**, contando o que aconteceu com o site: a
    /// Biblioteca não fala com ele, mas é onde quem classificou está olhando.
    /// Silêncio depois de classificar 200 fotos é o pior desfecho possível.
    aviso: Option<SharedString>,
    /// O campo do nome da coleção nova.
    nome_da_colecao: Entity<InputState>,
    /// Quantas fotos a confirmação de apagar está segurando.
    ///
    /// 🚨 **Apagar leva a revelação junto**, e é por isso que pergunta. Os 46
    /// ajustes moram na linha da foto; tirá-la do catálogo joga fora o trabalho
    /// de revelação dela, e reimportar devolve o arquivo — não a revelação.
    confirmando_apagar: Option<usize>,
    porta_de_colecoes: Arc<dyn colecoes::Colecoes>,
    recados_de_colecao: (
        std::sync::mpsc::Sender<colecoes::Recado>,
        std::sync::mpsc::Receiver<colecoes::Recado>,
    ),
    colhendo_colecoes: bool,
    _colheita_de_colecoes: Option<gpui::Task<()>>,
    /// As fotos que vão para a coleção assim que ela nascer.
    ///
    /// 🔑 A criação é assíncrona e o id só existe do outro lado; guardar a
    /// seleção aqui é o que permite "criar coleção com estas 40" ser um gesto
    /// só. Sem isso, quem cria precisa selecionar de novo o que já tinha
    /// selecionado.
    fotos_para_a_nova_colecao: Vec<String>,
    /// As pastas do acervo, calculadas **uma vez**.
    ///
    /// Elas saem dos caminhos das fotos, e as fotos não mudam enquanto a tela
    /// vive — recalcular a cada quadro seria varrer 2.000 caminhos 60 vezes por
    /// segundo para chegar sempre à mesma lista.
    pastas: Vec<Pasta>,
    /// Quem está marcado e onde está o cursor — **a mesma `Selecao` da grade do
    /// site** (`biblioteca_core::selecao`).
    ///
    /// 🔑 **É por posição na grade filtrada, e não por índice do acervo.** Foi a
    /// troca de 6/set/2026: as regras de clique, Shift, Ctrl e teclado moravam
    /// aqui em quatro métodos próprios e no site em outros — mesmo gesto, outro
    /// resultado. Quem sai de `/dashboard/sessoes-fotograficas` e abre este app
    /// não pode ter de reaprender o Shift.
    ///
    /// ⚠️ **O preço é a tradução nas bordas**: a grade, o filmstrip e as marcas
    /// falam em índice do acervo, e [`Self::visiveis`] é o dicionário entre os
    /// dois. Trocar de filtro embaralha as posições — por isso
    /// [`Self::refiltrar`] reconstrói a seleção **por id**, e não por número.
    selecao: Selecao,
    /// O dock: quem arruma os quatro painéis, e quem os deixa ser arrastados.
    ///
    /// `Option` porque ele nasce **depois** do resto: os painéis precisam de uma
    /// referência fraca a esta entidade, e ela só existe quando o construtor
    /// termina. Montá-lo aqui dentro daria um `WeakEntity` de algo que ainda não
    /// foi entregue ao `cx`.
    dock: Option<Entity<DockArea>>,
    /// A gravação adiada do arranjo. Guardada porque **descartá-la cancela** —
    /// é o que faz um arrasto inteiro de divisória virar uma escrita só.
    _arranjo: Option<gpui::Task<()>>,
    /// Em qual arquivo o arranjo é gravado. Definido ao montar o dock.
    arranjo_em: std::path::PathBuf,
    /// Quantas colunas a grade tem. `None` é **automático** — quantas couberem
    /// na janela.
    ///
    /// 🔑 **As duas coisas existem por decisão do dono, em 17/ago.** O legado só
    /// tem o número fixo (`state.grid_columns`, 1 a 5, sem automático) e a grade
    /// nova só tinha o automático. Manter os dois é o único arranjo que responde
    /// aos dois casos — "aproveite a janela toda" e "quero ver estas quatro
    /// grandes" — e é **feature nova**, que a regra §7.1 só permite assim: com
    /// decisão de dono registrada.
    colunas_escolhidas: Option<u8>,
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
        porta_de_colecoes: Arc<dyn colecoes::Colecoes>,
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

        let nome_da_colecao =
            cx.new(|cx| InputState::new(window, cx).placeholder("nome da coleção"));

        let mut tela = Self {
            fotos: Arc::new(fotos),
            previews,
            marcador,
            // Nasce do tamanho da janela padrão e se ajusta no primeiro
            // `render`, quando a janela de verdade já foi medida.
            cache: Arc::new(Mutex::new(CacheDeMiniaturas::nova(capacidade_para(6, 4)))),
            filtros: Filtros::default(),
            colecoes: colecoes::Estado::default(),
            sessao: None,
            aviso: None,
            nome_da_colecao,
            confirmando_apagar: None,
            porta_de_colecoes,
            recados_de_colecao: std::sync::mpsc::channel(),
            colhendo_colecoes: false,
            _colheita_de_colecoes: None,
            fotos_para_a_nova_colecao: Vec::new(),
            visiveis: Vec::new(),
            pastas: Vec::new(),
            selecao: Selecao::nova(),
            colunas_escolhidas: None,
            dock: None,
            _arranjo: None,
            arranjo_em: std::path::PathBuf::new(),
            busca,
            _assinaturas: vec![assinatura],
        };
        tela.pastas = pastas_do_acervo(&tela.fotos);
        // A lista lateral é pedida na abertura: sem isso ela só apareceria
        // depois de alguém criar uma coleção, e um painel vazio num catálogo que
        // tem coleções é indistinguível de um catálogo que não tem nenhuma.
        tela.porta_de_colecoes
            .listar(tela.recados_de_colecao.0.clone());
        // Nasce com a lista pronta: sem isto o primeiro quadro mostraria uma
        // grade vazia sobre um acervo cheio, e a tela só se corrigiria no
        // primeiro clique.
        tela.refiltrar();
        tela
    }

    /// O acervo foi relido: troca a lista inteira e reconstrói o que dependia
    /// dela.
    ///
    /// 🚨 **É o que faltava para a importação aparecer.** As fotos entravam no
    /// banco e a grade continuava com a lista lida antes da janela existir — o
    /// modal contava "65 importadas" sobre um acervo que não mudava. Nada
    /// falhava, e a leitura de quem usa é que a importação não funciona.
    ///
    /// 🔑 **A seleção é preservada por id, e não por índice.** Importar
    /// acrescenta fotos e a ordenação do acervo não é a de chegada: guardar a
    /// posição faria a foto principal virar outra — e a Revelação abriria a
    /// errada depois de qualquer importação.
    ///
    /// ⚠️ **As pastas são recalculadas.** Importar cria pasta nova
    /// (`2026/08/17/`), e uma árvore montada sobre o acervo velho não a teria —
    /// a foto estaria na grade e sem caminho até ela pela árvore.
    pub fn trocar_acervo(&mut self, fotos: Vec<PhotoViewModel>, cx: &mut Context<Self>) {
        let principal = self.foto_selecionada().map(|foto| foto.id);
        let outras: std::collections::HashSet<String> = self
            .ids_selecionados()
            .into_iter()
            .collect::<std::collections::HashSet<_>>();

        self.fotos = Arc::new(fotos);
        self.pastas = pastas_do_acervo(&self.fotos);

        self.refiltrar();
        // A escolha volta **por id**: as posições do acervo antigo não querem
        // dizer nada no novo. O que sumiu do catálogo simplesmente não volta.
        self.remarcar_por_id(&outras, principal.as_deref());
        cx.notify();
    }

    /// Monta o dock com o arranjo padrão.
    ///
    /// 🚨 **Só pode ser chamado depois de a entidade existir** — os painéis
    /// guardam uma referência fraca a ela, e dentro do construtor ela ainda não
    /// foi entregue ao `cx`. Por isso é um segundo passo, feito por quem cria a
    /// Biblioteca.
    ///
    /// O arranjo é o do legado (`create_library_layout`): pastas à esquerda,
    /// grade no meio com o filmstrip embaixo, informações à direita.
    /// Monta o dock com o arranjo padrão, e restaura o salvo por cima.
    ///
    /// 🚨 **Só pode ser chamado depois de a entidade existir** — os painéis
    /// guardam uma referência fraca a ela, e dentro do construtor ela ainda não
    /// foi entregue ao `cx`. Por isso é um segundo passo, feito por quem cria a
    /// Biblioteca:
    ///
    /// ```ignore
    /// biblioteca.update(cx, |tela, cx| {
    ///     let eu = cx.entity();
    ///     tela.montar_o_dock(&eu, window, cx);
    /// });
    /// ```
    ///
    /// O arranjo é o do legado (`create_library_layout`): pastas à esquerda,
    /// grade no meio com o filmstrip embaixo, informações à direita.
    pub fn montar_o_dock(
        &mut self,
        eu: &Entity<Self>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.montar_o_dock_em(eu, arranjo::caminho("biblioteca"), window, cx);
    }

    /// O mesmo, com o arquivo de arranjo escolhido.
    ///
    /// 🚨 **Existe por causa do teste.** O caminho padrão é `AppPaths` — o
    /// catálogo de verdade —, e um teste que exercitasse a gravação por ele
    /// escreveria no catálogo de quem roda a suíte. É o mesmo defeito que a fase
    /// 0 encontrou (`PreviewManager::new()` num teste de UI), e a defesa é a
    /// mesma: o caminho entra por parâmetro.
    pub fn montar_o_dock_em(
        &mut self,
        eu: &Entity<Self>,
        arquivo: std::path::PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let arquivo_para_ler = arquivo.clone();
        self.arranjo_em = arquivo;
        let dock = cx.new(|cx| DockArea::new("biblioteca", Some(arranjo::VERSAO), window, cx));
        let fraca = dock.downgrade();

        // 🚨 **Restaurar um arranjo salvo passa por aqui.** O `DockArea` guarda
        // só o **nome** de cada painel; quem sabe construí-lo de volta é este
        // registro global. Sem ele, um leiaute gravado volta como
        // `InvalidPanel` — um retângulo com o nome escrito dentro, no lugar da
        // grade. E isso não falha: o app abre, a tela está lá, e o que sumiu
        // foram as fotos.
        for qual in [
            Qual::Pastas,
            Qual::Grade,
            Qual::Informacoes,
            Qual::Filmstrip,
        ] {
            let acervo = eu.downgrade();
            register_panel(
                cx,
                qual.nome(),
                move |_dock, _estado, _info, _window, cx| {
                    let acervo = acervo.clone();
                    Box::new(cx.new(|cx| PainelDaBiblioteca::novo(qual, acervo, cx)))
                },
            );
        }

        let painel = |qual: Qual, eu: &Entity<Self>, cx: &mut gpui::App| {
            let acervo = eu.downgrade();
            let entidade = cx.new(|cx| PainelDaBiblioteca::novo(qual, acervo, cx));
            std::sync::Arc::new(entidade) as std::sync::Arc<dyn PanelView>
        };

        let pastas = DockItem::tabs(vec![painel(Qual::Pastas, eu, cx)], &fraca, window, cx);
        let grade = DockItem::tabs(vec![painel(Qual::Grade, eu, cx)], &fraca, window, cx);
        let filmstrip = DockItem::tabs(vec![painel(Qual::Filmstrip, eu, cx)], &fraca, window, cx);
        let informacoes =
            DockItem::tabs(vec![painel(Qual::Informacoes, eu, cx)], &fraca, window, cx);

        // O meio é uma coluna: a grade em cima, o filmstrip embaixo. As duas
        // laterais nascem com a largura que elas tinham fixas — quem arrastar
        // depois muda, e é esse o ponto.
        let meio = DockItem::split_with_sizes(
            gpui::Axis::Vertical,
            vec![grade, filmstrip],
            vec![None, Some(px(ALTURA_DO_FILMSTRIP))],
            &fraca,
            window,
            cx,
        );

        let centro = DockItem::split_with_sizes(
            gpui::Axis::Horizontal,
            vec![pastas, meio, informacoes],
            vec![
                Some(px(LADO_DA_ARVORE)),
                None,
                Some(px(LADO_DAS_INFORMACOES)),
            ],
            &fraca,
            window,
            cx,
        );

        dock.update(cx, |area, cx| {
            area.set_center(centro, window, cx);

            // O arranjo salvo entra **por cima** do padrão. Montar o padrão
            // antes não é desperdício: é o que garante uma tela inteira mesmo
            // quando o arquivo não existe, está corrompido, ou é de outra
            // versão — os três casos em que `ler_de` devolve `None`.
            if let Some(salvo) = arranjo::ler_de(&arquivo_para_ler) {
                if let Err(erro) = area.load(salvo, window, cx) {
                    eprintln!("⚠️  Arranjo salvo não pôde ser restaurado: {erro}");
                }
            }
        });

        // ⚠️ **A gravação é adiada.** O próprio `gpui-component` avisa que
        // `LayoutChanged` "may be emitted too frequently" — um arrasto de
        // divisória emite dezenas por segundo, e cada uma seria um arquivo
        // escrito. É a mesma espera da gravação de ajustes da Revelação, e pelo
        // mesmo motivo: guardar a `Task` faz cada evento novo **adiar** em vez
        // de enfileirar mais uma escrita.
        let assinatura = cx.subscribe_in(
            &dock,
            window,
            |tela: &mut Self, area, evento: &DockEvent, _window, cx| {
                if !matches!(evento, DockEvent::LayoutChanged) {
                    return;
                }
                let area = area.clone();
                tela._arranjo = Some(cx.spawn(async move |tela, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(ESPERA_DO_ARRANJO_MS))
                        .await;
                    let _ = tela.update(cx, |tela, cx| {
                        arranjo::gravar_em(&tela.arranjo_em, &area.read(cx).dump(cx));
                    });
                }));
            },
        );

        self._assinaturas.push(assinatura);
        self.dock = Some(dock);
    }

    /// Recalcula o que está visível. Chamado só quando um filtro muda.
    /// Recorta a grade a um ensaio. `None` volta ao catálogo inteiro.
    pub fn escopar_na_sessao(&mut self, sessao: Option<String>, cx: &mut Context<Self>) {
        if self.sessao == sessao {
            return;
        }
        self.sessao = sessao;
        self.refiltrar();
        // 🔑 Trocar de ensaio é trocar o conteúdo da grade: a seleção fala em
        // posição, e mantê-la apontaria fotos de outro cliente.
        self.selecao = Selecao::nova();
        cx.notify();
    }

    pub fn sessao(&self) -> Option<&str> {
        self.sessao.as_deref()
    }

    fn refiltrar(&mut self) {
        // 🚨 Lido **antes** de `visiveis` mudar: a `Selecao` fala em posições da
        // grade, e a grade está prestes a deixar de ser a mesma.
        let marcadas = self.marcadas_no_acervo();
        let principal = self.principal_no_acervo();

        self.visiveis = indices_visiveis(&self.fotos, &self.filtros);
        // 🚨 **O ensaio recorta antes de tudo.** Ele não é mais um filtro da
        // barra: é o assunto da tela. Uma foto de outro cliente aparecendo aqui
        // não é ruído, é a triagem errada.
        if let Some(sessao) = self.sessao.clone() {
            let fotos = self.fotos.clone();
            self.visiveis
                .retain(|&i| fotos[i].sessao_id.as_deref() == Some(sessao.as_str()));
        }
        // 🔑 A coleção filtra **por cima** dos outros filtros, e não no lugar
        // deles: abrir o ensaio de um cliente e então pedir ★★★★ dentro dele é
        // a pergunta normal — "quais das dele valem a pena". Substituir faria a
        // coleção desligar nota, cor e busca sem dizer.
        if self.colecoes.aberta().is_some() {
            let fotos = self.fotos.clone();
            let colecoes = &self.colecoes;
            self.visiveis.retain(|&i| colecoes.aceita(&fotos[i].id));
        }
        self.sanear_selecao(marcadas, principal);
    }

    pub fn colecoes(&self) -> &colecoes::Estado {
        &self.colecoes
    }

    /// Abre (ou fecha) uma coleção, e pede os ids quando precisa.
    pub fn abrir_colecao(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.colecoes.abrir(id) {
            self.porta_de_colecoes
                .fotos(id.to_string(), self.recados_de_colecao.0.clone());
            self.acompanhar_colecoes(cx);
        }
        self.refiltrar();
        cx.notify();
    }

    /// Cria uma coleção com as fotos selecionadas dentro.
    ///
    /// 🔑 **Criar e povoar são um gesto só.** Uma coleção nova e vazia obriga a
    /// selecionar de novo o que já estava selecionado — e é exatamente com uma
    /// seleção na mão que alguém decide criar o ensaio de um cliente.
    pub fn criar_colecao(&mut self, nome: String, cx: &mut Context<Self>) {
        if nome.trim().is_empty() {
            return;
        }
        self.fotos_para_a_nova_colecao = self.ids_selecionados();
        self.porta_de_colecoes
            .criar(nome, self.recados_de_colecao.0.clone());
        self.acompanhar_colecoes(cx);
        cx.notify();
    }

    /// Acrescenta a seleção à coleção aberta.
    pub fn acrescentar_a_colecao_aberta(&mut self, cx: &mut Context<Self>) {
        let Some(colecao) = self.colecoes.aberta().map(str::to_string) else {
            return;
        };
        let fotos = self.ids_selecionados();
        if fotos.is_empty() {
            return;
        }
        self.porta_de_colecoes
            .acrescentar(colecao, fotos, self.recados_de_colecao.0.clone());
        self.acompanhar_colecoes(cx);
    }

    /// Tira a seleção da coleção aberta.
    pub fn remover_da_colecao_aberta(&mut self, cx: &mut Context<Self>) {
        let Some(colecao) = self.colecoes.aberta().map(str::to_string) else {
            return;
        };
        let fotos = self.ids_selecionados();
        if fotos.is_empty() {
            return;
        }
        self.porta_de_colecoes
            .remover(colecao, fotos, self.recados_de_colecao.0.clone());
        self.acompanhar_colecoes(cx);
    }

    /// Liga o laço que drena os recados das coleções, se ainda não houver um.
    fn acompanhar_colecoes(&mut self, cx: &mut Context<Self>) {
        if self.colhendo_colecoes {
            return;
        }
        self.colhendo_colecoes = true;

        self._colheita_de_colecoes = Some(cx.spawn(async move |esta, cx| {
            // ⚠️ Um teto, e não um laço eterno: coleção é gesto pontual, e o
            // laço não pode continuar cobrando relógio pelo resto da sessão.
            for _ in 0..100 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                let Ok(continua) = esta.update(cx, |tela, cx| tela.colher_colecoes(cx)) else {
                    return;
                };
                if !continua {
                    return;
                }
            }
        }));
    }

    /// Drena os recados. Devolve se vale continuar acordando.
    pub fn colher_colecoes(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        let mut pendente = false;

        while let Ok(recado) = self.recados_de_colecao.1.try_recv() {
            mudou = true;
            match recado {
                colecoes::Recado::Listadas(lista) => self.colecoes.lista = lista,
                colecoes::Recado::Fotos { colecao, ids } => {
                    if self.colecoes.receber_fotos(&colecao, ids) {
                        self.refiltrar();
                    }
                }
                colecoes::Recado::Criada(nova) => {
                    let id = nova.id.clone();
                    self.colecoes.lista.push(nova);
                    let fotos = std::mem::take(&mut self.fotos_para_a_nova_colecao);
                    if !fotos.is_empty() {
                        self.porta_de_colecoes.acrescentar(
                            id,
                            fotos,
                            self.recados_de_colecao.0.clone(),
                        );
                        pendente = true;
                    }
                }
                colecoes::Recado::Mudou { colecao, .. } => {
                    // A contagem da lista lateral e os ids da aberta mudaram —
                    // os dois são relidos, e não recalculados aqui: o banco é
                    // quem sabe o que ficou lá dentro.
                    self.porta_de_colecoes
                        .listar(self.recados_de_colecao.0.clone());
                    if self.colecoes.aberta() == Some(colecao.as_str()) {
                        self.porta_de_colecoes
                            .fotos(colecao, self.recados_de_colecao.0.clone());
                    }
                    pendente = true;
                }
                colecoes::Recado::Falhou(erro) => self.colecoes.aviso = Some(erro),
            }
        }

        if mudou {
            cx.notify();
        }
        if !pendente && !mudou {
            self.colhendo_colecoes = false;
            return false;
        }
        true
    }

    /// 🚨 A seleção não pode apontar para fora da grade.
    ///
    /// Sem isto, dar nota 1 numa foto com o filtro em "★★★ ou mais" tira ela da
    /// grade e **mantém ela selecionada**: o painel de informações continua
    /// mostrando-a, as teclas seguintes caem nela, e "Revelação" abre uma foto
    /// que não está na tela. Nada falha — é a seleção apontando para o invisível.
    ///
    /// O legado tem a mesma defesa (`sanitize_develop_selection`), com uma
    /// diferença: lá a nova escolhida é sempre **a primeira da lista filtrada**.
    /// ⚠️ **Aqui é a seguinte**, porque numa triagem de 800 fotos voltar ao começo
    /// a cada foto rejeitada faz perder o lugar — e perder o lugar é o que a
    /// triagem inteira existe para não fazer.
    fn sanear_selecao(&mut self, marcadas: Vec<usize>, principal: Option<usize>) {
        let visiveis: std::collections::HashSet<usize> = self.visiveis.iter().copied().collect();

        self.selecao = Selecao::nova();
        for no_acervo in marcadas.iter().copied().filter(|i| visiveis.contains(i)) {
            if let Some(posicao) = self.posicao_de(no_acervo) {
                self.selecao.marcar(posicao);
            }
        }

        let Some(atual) = principal else {
            return;
        };
        if visiveis.contains(&atual) {
            self.selecao.focar(self.posicao_de(atual));
            return;
        }

        // Sobrou alguém da seleção? Então a principal passa a ser a primeira
        // delas — a seleção manda mais que a posição. Senão, anda para a
        // seguinte que ainda está na grade.
        let nova = self
            .selecao
            .marcadas()
            .next()
            .and_then(|posicao| self.no_acervo(posicao))
            .or_else(|| {
                self.visiveis
                    .iter()
                    .copied()
                    .find(|&i| i > atual)
                    .or_else(|| self.visiveis.last().copied())
            });

        if let Some(no_acervo) = nova {
            if let Some(posicao) = self.posicao_de(no_acervo) {
                self.selecao.marcar(posicao);
                self.selecao.focar(Some(posicao));
            }
        }
    }

    /// A posição na grade filtrada de uma foto do acervo. `None` se ela não
    /// passa no filtro de agora.
    fn posicao_de(&self, no_acervo: usize) -> Option<usize> {
        self.visiveis.iter().position(|&i| i == no_acervo)
    }

    /// O caminho de volta: a foto do acervo que está naquela posição da grade.
    fn no_acervo(&self, posicao: usize) -> Option<usize> {
        self.visiveis.get(posicao).copied()
    }

    /// As marcadas, traduzidas para o acervo e **na ordem da grade**.
    ///
    /// A ordem importa: é ela que decide em qual célula da folha de impressão
    /// cada foto cai. No legado esta lista sai de um `HashSet`, em ordem de hash.
    fn marcadas_no_acervo(&self) -> Vec<usize> {
        self.selecao
            .marcadas()
            .filter_map(|posicao| self.no_acervo(posicao))
            .collect()
    }

    /// A foto do cursor, no acervo.
    fn principal_no_acervo(&self) -> Option<usize> {
        self.selecao.foco().and_then(|p| self.no_acervo(p))
    }

    /// Refaz a escolha a partir de ids, depois de o acervo inteiro mudar.
    fn remarcar_por_id(
        &mut self,
        marcadas: &std::collections::HashSet<String>,
        principal: Option<&str>,
    ) {
        self.selecao = Selecao::nova();
        for (posicao, &no_acervo) in self.visiveis.iter().enumerate() {
            if marcadas.contains(&self.fotos[no_acervo].id) {
                self.selecao.marcar(posicao);
            }
        }
        if let Some(id) = principal {
            let onde = self.visiveis.iter().position(|&i| self.fotos[i].id == id);
            self.selecao.focar(onde);
        }
    }

    /// A foto selecionada, para quem está de fora.
    ///
    /// Devolve uma cópia, e não uma referência: quem pergunta é a raiz, para
    /// levar a foto até a Revelação, e o que ela leva tem de continuar valendo
    /// depois que a grade mudar de filtro.
    pub fn foto_selecionada(&self) -> Option<PhotoViewModel> {
        self.principal_no_acervo().map(|i| self.fotos[i].clone())
    }

    /// Os ids das selecionadas, **na ordem do acervo**.
    ///
    /// É o que a Impressão recebe para montar a coleção — e a ordem importa,
    /// porque a posição na lista decide em qual célula da folha cada foto cai.
    /// No legado esta lista sai de um `HashSet`, em ordem de hash.
    pub fn ids_selecionados(&self) -> Vec<String> {
        self.marcadas_no_acervo()
            .into_iter()
            .map(|i| self.fotos[i].id.clone())
            .collect()
    }

    /// Esquece a miniatura de uma foto: a próxima pintura relê do cache.
    ///
    /// 🚨 O cache guarda **"ausente"** para quem ainda não tinha miniatura — é o
    /// que impede pedir de novo a cada quadro. Sem esquecer, a foto que acabou
    /// de chegar do site só apareceria quando a célula saísse e voltasse à tela.
    pub fn esquecer_miniatura(&mut self, chave: &str, cx: &mut Context<Self>) {
        self.cache.lock().expect("o cache").esquecer(chave);
        cx.notify();
    }

    /// O acervo inteiro que esta tela conhece, sem filtro nenhum.
    ///
    /// É o que a raiz precisa para **recompor** a grade: as locais que já estão
    /// aqui, mais as que o site respondeu.
    pub fn todas_as_fotos(&self) -> Vec<PhotoViewModel> {
        self.fotos.as_ref().clone()
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

    /// As fotos da seleção múltipla, **na ordem do acervo**.
    ///
    /// Vazio quando ninguém marcou nada com `Cmd`/`Shift` — e é o que faz a
    /// exportação cair em [`Self::fotos_visiveis`] nesse caso, em vez de exportar
    /// a única foto principal quando a pessoa filtrou 300 e não marcou nenhuma.
    pub fn fotos_selecionadas(&self) -> Vec<PhotoViewModel> {
        self.marcadas_no_acervo()
            .into_iter()
            .map(|i| self.fotos[i].clone())
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

    /// O filtro por nota mínima — o passo 4 do fluxo: **filtro as classificadas**.
    ///
    /// `0` é "todas". A grade recorta na hora; a seleção que sair do recorte é
    /// reconstruída por id, não por posição.
    pub fn filtrar_por_nota(&mut self, minima: u8, cx: &mut Context<Self>) {
        self.filtros.nota_minima = NotaMinima(minima);
        self.refiltrar();
        cx.notify();
    }

    pub fn nota_minima(&self) -> u8 {
        self.filtros.nota_minima.0
    }

    /// Põe um aviso na tela. É por aqui que a raiz conta o que aconteceu com o
    /// site — a Biblioteca não fala com ele, mas é onde quem classificou está
    /// olhando.
    pub fn avisar(&mut self, texto: String, cx: &mut Context<Self>) {
        self.aviso = Some(texto.into());
        cx.notify();
    }

    pub fn aviso(&self) -> Option<&SharedString> {
        self.aviso.as_ref()
    }

    pub fn limpar_aviso(&mut self, cx: &mut Context<Self>) {
        if self.aviso.take().is_some() {
            cx.notify();
        }
    }

    /// Quantas fotos estão selecionadas.
    /// Pede a confirmação de apagar a seleção.
    ///
    /// ⚠️ **Não apaga nada ainda.** A tecla `Delete` fica ao lado das treze de
    /// triagem, que se apertam às centenas numa sessão; sem o passo de
    /// confirmação, um dedo fora do lugar tira fotos do catálogo em lote.
    pub fn pedir_para_apagar(&mut self, cx: &mut Context<Self>) {
        let quantas = if self.selecao.vazia() {
            usize::from(self.selecao.foco().is_some())
        } else {
            self.selecao.quantas()
        };
        if quantas == 0 {
            return;
        }
        self.confirmando_apagar = Some(quantas);
        cx.notify();
    }

    pub fn cancelar_apagar(&mut self, cx: &mut Context<Self>) {
        self.confirmando_apagar = None;
        cx.notify();
    }

    pub fn confirmando_apagar(&self) -> Option<usize> {
        self.confirmando_apagar
    }

    /// Tira as fotos selecionadas do catálogo.
    ///
    /// 🔑 **O acervo em memória é atualizado na hora**, e não relido: apagar 40
    /// fotos e esperar a releitura deixaria a grade mostrando o que já não
    /// existe — e clicar numa delas abriria a Revelação numa foto sem linha no
    /// banco. É o *optimistic update* que a triagem já usa.
    ///
    /// ⚠️ **O arquivo continua no disco.** É o "Remove from Catalog" do
    /// Lightroom, e a tela diz isso na confirmação: apagar do disco é operação
    /// de outra natureza, que ninguém deve tomar por engano com a tecla `Delete`.
    pub fn apagar_confirmado(&mut self, cx: &mut Context<Self>) {
        self.confirmando_apagar = None;

        let alvos: Vec<usize> = if self.selecao.vazia() {
            self.principal_no_acervo().into_iter().collect()
        } else {
            self.marcadas_no_acervo()
        };
        if alvos.is_empty() {
            return;
        }

        let ids: Vec<String> = alvos.iter().map(|&i| self.fotos[i].id.clone()).collect();
        for id in &ids {
            self.marcador.apagar(id.clone());
        }

        let apagados: std::collections::HashSet<&String> = ids.iter().collect();
        let restantes: Vec<PhotoViewModel> = self
            .fotos
            .iter()
            .filter(|foto| !apagados.contains(&foto.id))
            .cloned()
            .collect();

        self.fotos = Arc::new(restantes);
        self.pastas = pastas_do_acervo(&self.fotos);
        // 🚨 A seleção apontava para posições de uma grade que não existe mais.
        // Mantê-la faria a próxima tecla de triagem cair em fotos que ninguém
        // escolheu.
        self.selecao = Selecao::nova();
        self.refiltrar();
        cx.notify();
    }

    /// Quantas fotos o acervo tem — o número que a releitura muda.
    pub fn quantas_fotos(&self) -> usize {
        self.fotos.len()
    }

    /// Quantas a grade está mostrando — depois de todos os filtros.
    pub fn quantas_visiveis(&self) -> usize {
        self.visiveis.len()
    }

    pub fn quantas_selecionadas(&self) -> usize {
        self.selecao.quantas()
    }

    /// Seleciona **uma só**, por índice no acervo — o clique sem modificador.
    ///
    /// `None` limpa tudo. Selecionar uma foto joga fora as outras, como no
    /// legado (`single_select`): clique sem modificador é "quero esta", e manter
    /// as anteriores faria a próxima tecla de nota cair em fotos que quem
    /// clicou já tinha esquecido.
    pub fn selecionar(&mut self, no_acervo: Option<usize>, cx: &mut Context<Self>) {
        match no_acervo.and_then(|i| self.posicao_de(i)) {
            Some(posicao) => self
                .selecao
                .clicar(posicao, false, Modificadores::default()),
            None => self.selecao.limpar_tudo(),
        }
        cx.notify();
    }

    /// `Cmd+clique`: põe ou tira uma foto da seleção, sem tocar nas outras.
    pub fn alternar_uma(&mut self, no_acervo: usize, cx: &mut Context<Self>) {
        let Some(posicao) = self.posicao_de(no_acervo) else {
            return;
        };
        // A âncora e o cursor andam com o último clique, mesmo quando ele
        // **tirou** a foto da seleção: é dali que o próximo `Shift+clique` mede.
        // Quem decide isso é o core, e é a mesma decisão da grade do site.
        self.selecao.clicar(
            posicao,
            false,
            Modificadores {
                aditivo: true,
                faixa: false,
            },
        );
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
        let Some(posicao) = self.posicao_de(no_acervo) else {
            return;
        };
        self.selecao.clicar(
            posicao,
            false,
            Modificadores {
                aditivo: false,
                faixa: true,
            },
        );
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
        self.selecao.marcar_todas(self.visiveis.len());
        // O cursor continua onde estava; se não havia nenhum, vai para a
        // primeira da grade. Sem isto, `Cmd+A` deixaria a Revelação sem saber
        // qual abrir.
        self.selecao.focar_primeira_se_vazio(self.visiveis.len());
        cx.notify();
    }

    /// `Cmd+D`: desmarca tudo.
    ///
    /// ⚠️ **O cursor fica onde está**, como no site (`Selecao::desmarcar`). Até
    /// 6/set/2026 o desktop limpava o foco junto, e a tecla seguinte de triagem
    /// não tinha em quem cair.
    pub fn limpar_selecao(&mut self, cx: &mut Context<Self>) {
        self.selecao.desmarcar();
        cx.notify();
    }

    /// Quantas colunas a grade desenha agora.
    ///
    /// ⚠️ **O automático é o padrão**, e não um modo escondido: numa janela
    /// larga ele é a resposta certa, e é a que o app dá antes de alguém escolher
    /// qualquer coisa.
    fn colunas(&self, window: &Window) -> usize {
        match self.colunas_escolhidas {
            Some(quantas) => quantas.clamp(1, 5) as usize,
            None => colunas_que_cabem(largura_util(window), LADO_DO_ITEM, ESPACAMENTO),
        }
    }

    pub fn colunas_escolhidas(&self) -> Option<u8> {
        self.colunas_escolhidas
    }

    /// `None` volta para o automático.
    pub fn escolher_colunas(&mut self, quantas: Option<u8>, cx: &mut Context<Self>) {
        self.colunas_escolhidas = quantas;
        cx.notify();
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

        let posicao = self.selecao.foco();

        let nova = match posicao {
            Some(atual) if passo > 0 => (atual + 1).min(self.visiveis.len() - 1),
            Some(atual) => atual.saturating_sub(1),
            None if passo > 0 => 0,
            None => self.visiveis.len() - 1,
        };

        self.selecao.clicar(nova, false, Modificadores::default());
        cx.notify();
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
        let todas_ja_tem = self.marcadas_no_acervo().into_iter().all(|i| {
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
            .marcadas_no_acervo()
            .into_iter()
            .all(|i| self.fotos[i].flag.unwrap_or(0) == pedido);

        let codigo = sinalizador_ao_teclar(todas_ja_tem.then_some(pedido), pedido);
        self.aplicar(Marca::Sinalizador(codigo), cx);
    }

    /// Levada no balcão — a tecla `B`, alternando pelo grupo como `P`.
    ///
    /// 🔑 Só desmarca quando **todas** as selecionadas já estão levadas: com
    /// três selecionadas e uma já marcada, `B` marca as três — é a mesma regra
    /// do sinalizador, pelo mesmo motivo (uma tecla, um desfecho).
    pub fn marcar_comprada(&mut self, cx: &mut Context<Self>) {
        let todas_ja_levadas = self
            .marcadas_no_acervo()
            .into_iter()
            .all(|i| self.fotos[i].comprada);
        self.aplicar(Marca::Comprada(!todas_ja_levadas), cx);
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
        let alvos = self.marcadas_no_acervo();
        if alvos.is_empty() {
            return;
        }

        // 🚨 A travessia do zero é lida **antes** da escrita, e só para a nota.
        //
        // No fluxo do dono é a classificação que autoriza a foto a subir para o
        // site, e zerá-la é tirá-la de lá. Depois de escrever, o "antes" já não
        // existe — e sem ele não dá para saber quem atravessou, só quem está de
        // que lado agora.
        let travessia = match &marca {
            Marca::Nota(nova) => Some(travessia_do_zero(&self.fotos, &alvos, *nova)),
            _ => None,
        };

        let fotos = Arc::make_mut(&mut self.fotos);
        for no_acervo in alvos {
            let foto = &mut fotos[no_acervo];

            match &marca {
                Marca::Nota(nota) => foto.rating = *nota,
                Marca::Cor(cor) => foto.color_label = cor.clone(),
                Marca::Sinalizador(codigo) => foto.flag = Some(*codigo),
                Marca::Comprada(sim) => foto.comprada = *sim,
            }

            self.marcador.marcar(foto.id.clone(), marca.clone());
        }

        self.refiltrar();
        if let Some(classificou) = travessia {
            if !classificou.vazio() {
                cx.emit(classificou);
            }
        }
        cx.notify();
    }

    /// O clique sem modificador: seleciona **só** esta.
    ///
    /// 🚨 **Mudou em 6/set/2026, e mudou de propósito.** Até então clicar na
    /// única foto selecionada a **desmarcava**, e a grade do site não faz isso —
    /// mesmo gesto, dois resultados, dependendo do app em que a pessoa está. A
    /// regra que sobrou é a do site, que é a referência: clique simples marca a
    /// foto clicada e só ela. Para desmarcar sem clicar em outra há `Cmd+D`.
    pub fn alternar_selecao(&mut self, no_acervo: usize, cx: &mut Context<Self>) {
        self.selecionar(Some(no_acervo), cx);
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
        let selecao: Option<SharedString> = (self.selecao.quantas() > 1)
            .then(|| format!("{} selecionadas", self.selecao.quantas()).into());

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

        // Automático primeiro: é o padrão, e ler "auto 1 2 3 4 5" diz na ordem
        // o que a barra faz — a alternativa (5 4 3 2 1 auto) esconde o padrão no
        // fim de uma fileira de números.
        let escolhidas = self.colunas_escolhidas;
        let mut colunas = vec![botao(
            "colunas-auto".to_string(),
            "auto".to_string(),
            escolhidas.is_none(),
            cx.listener(
                move |this: &mut Self, _ev: &gpui::ClickEvent, _window, cx: &mut Context<Self>| {
                    this.escolher_colunas(None, cx);
                },
            ),
        )];
        for quantas in 1..=5u8 {
            colunas.push(botao(
                format!("colunas-{quantas}"),
                quantas.to_string(),
                escolhidas == Some(quantas),
                cx.listener(
                    move |this: &mut Self,
                          _ev: &gpui::ClickEvent,
                          _window,
                          cx: &mut Context<Self>| {
                        this.escolher_colunas(Some(quantas), cx);
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

        // O que o cliente decidiu no balcão — o filtro que separa o que o
        // pós-venda libera do que ele põe à venda.
        let compra_atual = self.filtros.compra;
        let mut compras = Vec::new();
        for (qual, rotulo) in [
            (FiltroDeCompra::Qualquer, "todas"),
            (FiltroDeCompra::LevadasNoBalcao, "levadas"),
            (FiltroDeCompra::ParaVenda, "à venda"),
        ] {
            compras.push(botao(
                format!("compra-{rotulo}"),
                rotulo.to_string(),
                compra_atual == qual,
                cx.listener(
                    move |this: &mut Self,
                          _ev: &gpui::ClickEvent,
                          _window,
                          cx: &mut Context<Self>| {
                        this.filtros.compra = qual;
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
            .child(rotulo_do_grupo("balcão", cx))
            .child(div().flex().gap(px(4.)).children(compras))
            .child(rotulo_do_grupo("cor", cx))
            .child(div().flex().gap(px(4.)).children(cores))
            .child(rotulo_do_grupo("colunas", cx))
            .child(div().flex().gap(px(4.)).children(colunas))
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
        let foto = self.principal_no_acervo().map(|i| &self.fotos[i]);
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
                    .child(self.estrelas_clicaveis(foto.rating, cx))
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

    /// As cinco estrelas, clicáveis — a nota sem passar pelo teclado.
    ///
    /// 🔑 **Clicar vale para a seleção inteira**, como as teclas `0`–`5`: é a
    /// mesma ação, e ter o clique valendo só para uma faria a mesma nota
    /// significar duas coisas conforme de onde veio.
    ///
    /// ⚠️ **Clicar na estrela que já está acesa não apaga a nota** — é o que o
    /// legado faz (`if new_rating != *rating`), e tirar a nota continua sendo a
    /// tecla `0`. Alternar aqui seria feature nova, e ela tem um custo real:
    /// numa triagem, clicar duas vezes por engano na terceira estrela apagaria a
    /// nota em vez de confirmá-la.
    fn estrelas_clicaveis(&self, nota: i32, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .justify_between()
            .gap(px(6.))
            .text_xs()
            .child(div().text_color(cx.theme().muted_foreground).child("nota"))
            .child(div().flex().gap(px(2.)).children((1..=5).map(|estrela| {
                let acesa = estrela <= nota;
                div()
                    .id(SharedString::from(format!("estrela-{estrela}")))
                    .cursor_pointer()
                    .text_color(if acesa {
                        cx.theme().primary
                    } else {
                        cx.theme().muted_foreground.opacity(0.4)
                    })
                    .child("★")
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.dar_nota(estrela, cx);
                    }))
            })))
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

        // O cursor já é posição na grade — é a mesma conta que o filmstrip
        // precisa, e não há mais tradução de ida e volta aqui.
        let posicao_atual = self.selecao.foco();
        let principal = self.principal_no_acervo();

        let inicio = posicao_atual
            .map(|p| p.saturating_sub(VIZINHAS))
            .unwrap_or(0);
        let fim = (inicio + VIZINHAS * 2 + 1).min(self.visiveis.len());

        let mut itens = Vec::new();
        for posicao in inicio..fim {
            let no_acervo = self.visiveis[posicao];
            let foto = &self.fotos[no_acervo];
            let e_a_selecionada = principal == Some(no_acervo);

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

        let legenda: SharedString = match principal {
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
                .flex()
                .items_center()
                .gap(px(4.))
                // O selo de "levada no balcão" fica ao lado do nome, e não sobre
                // a foto: sobre a foto ele cobriria justamente o que se está
                // avaliando, e é a mesma razão de a seleção ser borda e não fundo.
                .when(foto.comprada, |linha| {
                    linha.child(
                        div()
                            .text_xs()
                            .px(px(4.))
                            .rounded(px(3.))
                            .bg(cx.theme().primary)
                            .text_color(cx.theme().primary_foreground)
                            .child("levada"),
                    )
                })
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
                ),
        )
        .on_click(ao_clicar)
        .into_any_element()
}

/// Quais fotos atravessaram o zero da classificação — e para que lado.
///
/// 🔑 **É o gatilho do passo 3 do fluxo do dono**, e o motivo de a Biblioteca
/// não falar com o site: ela **notifica** que a classificação mudou de lado, e
/// quem sabe o que fazer com isso é a raiz, que tem a sessão e a galeria aberta.
/// Assim a grade continua funcionando offline, sem saber que existe um site.
///
/// ⚠️ **A travessia, e não o estado.** Passar de 3 para 4 estrelas não é evento:
/// a foto já estava no site e continua. O que importa é 0 → nota (sobe) e
/// nota → 0 (sai).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Classificou {
    /// Ganharam nota: sobem para a galeria aberta.
    pub subiram: Vec<String>,
    /// Ficaram sem nota: saem do storage.
    pub sairam: Vec<String>,
}

impl Classificou {
    pub fn vazio(&self) -> bool {
        self.subiram.is_empty() && self.sairam.is_empty()
    }
}

impl gpui::EventEmitter<Classificou> for Biblioteca {}

/// Quem atravessou o zero, comparando o que está no acervo com a nota nova.
fn travessia_do_zero(fotos: &[PhotoViewModel], alvos: &[usize], nova: i32) -> Classificou {
    let mut classificou = Classificou::default();
    for &no_acervo in alvos {
        let foto = &fotos[no_acervo];
        let tinha = foto.rating >= 1;
        let tera = nova >= 1;
        if tinha == tera {
            continue;
        }
        if tera {
            classificou.subiram.push(foto.id.clone());
        } else {
            classificou.sairam.push(foto.id.clone());
        }
    }
    classificou
}

impl Render for Biblioteca {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.barra(cx))
            .when_some(self.aviso.clone(), |tela, aviso| {
                tela.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .px(px(10.))
                        .py(px(4.))
                        .text_xs()
                        .text_color(cx.theme().warning)
                        .child(aviso)
                        .child(
                            Button::new("biblioteca-fechar-aviso")
                                .label("ok")
                                .xsmall()
                                .ghost()
                                .on_click(
                                    cx.listener(|tela, _ev, _window, cx| tela.limpar_aviso(cx)),
                                ),
                        ),
                )
            })
            .child(
                // `min_h(0)`: sem ele o conteúdo rolável de dentro do dock
                // empurra o pai e a rolagem nunca acontece.
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .children(self.dock.clone()),
            )
    }
}

impl Biblioteca {
    /// A grade — o painel central do dock.
    ///
    /// 🔑 **Os quatro painéis continuam sendo métodos daqui**, e não views com
    /// estado próprio. É o que fez o dock caber num commit: os `cx.listener`
    /// deste arquivo esperam `Context<Biblioteca>`, e movê-los para dentro de
    /// uma view nova trocaria **todos** eles por `entidade.update(...)` — 600
    /// linhas reescritas para mudar de lugar, com os testes por baixo.
    pub fn painel_da_grade(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colunas = self.colunas(window);
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
        let principal = self.principal_no_acervo();
        // A seleção inteira vai para o closure `'static` do `uniform_list`, já
        // traduzida para índices do acervo — é a linguagem da célula.
        let selecionadas: Arc<std::collections::BTreeSet<usize>> =
            Arc::new(self.marcadas_no_acervo().into_iter().collect());
        // O closure do `uniform_list` é `'static` e recebe `&mut App`, não
        // `&mut self` — para escrever no estado a partir dele, é preciso levar
        // uma referência à entidade e pedir a ela que se atualize.
        let eu = cx.entity();

        // O `uniform_list` só chama o closure para as linhas visíveis. É o que
        // faz 2.000 fotos custarem o mesmo que 20 na hora de desenhar — a
        // diferença entre rolar liso e engasgar.
        uniform_list("grade-da-biblioteca", linhas, move |faixa, _window, cx| {
            faixa
                .map(|indice| {
                    let desta_linha = fotos_da_linha(indice, colunas, total);

                    div().flex().gap(px(ESPACAMENTO)).p(px(4.)).children(
                        desta_linha
                            // Dois saltos: a linha dá a posição na lista
                            // **filtrada**, e `visiveis` traduz para o índice do
                            // acervo. Indexar `fotos` direto mostraria a foto
                            // errada assim que houvesse filtro — e a grade
                            // continuaria bonita, que é o que torna esse defeito
                            // caro.
                            .map(|posicao| {
                                let no_acervo = visiveis[posicao];
                                let eu = eu.clone();
                                celula(
                                    &fotos[no_acervo],
                                    &previews,
                                    &cache,
                                    selecionadas.contains(&no_acervo),
                                    principal == Some(no_acervo),
                                    // 🔑 Os modificadores vêm do `ClickEvent`, e é
                                    // por isso que o clique fica na célula
                                    // inteira: `Cmd` alterna uma, `Shift` estende
                                    // o intervalo, e sem eles é seleção única.
                                    move |evento: &gpui::ClickEvent,
                                          _window,
                                          cx: &mut gpui::App| {
                                        let modificadores = evento.modifiers();
                                        eu.update(cx, |tela, cx| {
                                            if modificadores.secondary() {
                                                tela.alternar_uma(no_acervo, cx);
                                            } else if modificadores.shift {
                                                tela.selecionar_ate(no_acervo, cx);
                                            } else {
                                                tela.alternar_selecao(no_acervo, cx);
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
        .size_full()
        .p(px(8.))
        .into_any_element()
    }

    /// A árvore de pastas e os filtros — o painel da esquerda.
    /// A coluna da esquerda: pastas em cima, coleções embaixo.
    ///
    /// 🔑 **As duas no mesmo painel, como no Lightroom.** Não é economia de
    /// espaço: um painel novo no dock não apareceria para quem já tem arranjo
    /// gravado — o JSON do disco descreve os quatro que existiam, e restaurá-lo
    /// deixaria as coleções invisíveis exatamente para quem mais usou o app.
    pub fn painel_das_pastas(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .child(self.arvore_de_pastas(cx)),
            )
            .child(self.lista_de_colecoes(cx))
            .into_any_element()
    }

    /// A lista de coleções, e o campo que cria uma.
    fn lista_de_colecoes(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let aberta = self.colecoes.aberta().map(str::to_string);
        let tem_selecao = !self.selecao.vazia();
        let quantas_selecionadas = self.selecao.quantas();

        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .border_t_1()
            .border_color(cx.theme().border)
            .pt(px(6.))
            .max_h(px(220.))
            .child(
                div()
                    .px(px(8.))
                    .pb(px(4.))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Coleções ({})", self.colecoes.lista.len())),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .overflow_hidden()
                    .children(
                        self.colecoes
                            .lista
                            .iter()
                            .map(|colecao| {
                                let id = colecao.id.clone();
                                let esta_aberta = aberta.as_deref() == Some(id.as_str());
                                Button::new(SharedString::from(format!("colecao-{id}")))
                                    .label(format!("{} ({})", colecao.name, colecao.photo_count))
                                    .xsmall()
                                    .when(esta_aberta, |b| b.primary())
                                    .selected(esta_aberta)
                                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                                        tela.abrir_colecao(&id, cx);
                                    }))
                            })
                            .collect::<Vec<_>>(),
                    ),
            )
            .when(self.colecoes.lista.is_empty(), |raiz| {
                raiz.child(
                    div()
                        .px(px(8.))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        // ⚠️ Um painel vazio sem esta linha é indistinguível de
                        // um painel quebrado — e a saída fica escrita junto.
                        .child("nenhuma ainda · selecione fotos e crie a primeira"),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .px(px(6.))
                    .pt(px(4.))
                    .child(div().flex_1().child(self.nome_da_colecao.clone()))
                    .child(
                        Button::new("colecao-criar")
                            .label("Criar")
                            .xsmall()
                            .on_click(cx.listener(|tela, _ev, window, cx| {
                                let nome = tela.nome_da_colecao.read(cx).value().to_string();
                                tela.criar_colecao(nome, cx);
                                tela.nome_da_colecao.update(cx, |campo, cx| {
                                    campo.set_value("", window, cx);
                                });
                            })),
                    ),
            )
            .when(aberta.is_some() && tem_selecao, |raiz| {
                raiz.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .px(px(6.))
                        .pt(px(2.))
                        .child(
                            Button::new("colecao-acrescentar")
                                .label(format!("+ {quantas_selecionadas}"))
                                .xsmall()
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.acrescentar_a_colecao_aberta(cx);
                                })),
                        )
                        .child(
                            Button::new("colecao-remover")
                                .label(format!("− {quantas_selecionadas}"))
                                .xsmall()
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    tela.remover_da_colecao_aberta(cx);
                                })),
                        ),
                )
            })
    }

    /// A foto e o acervo em números — o painel da direita.
    pub fn painel_das_informacoes(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.painel_de_informacoes(cx).into_any_element()
    }

    /// A faixa de miniaturas — o painel de baixo.
    pub fn painel_do_filmstrip(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.filmstrip(cx).into_any_element()
    }

    /// A barra de cima: busca, filtros e colunas.
    ///
    /// ⚠️ **Não é painel do dock**, e no legado também não: ela é a barra da
    /// janela, e um dock que pudesse fechá-la deixaria a Biblioteca sem busca e
    /// sem filtro, com "Reset Layout" como única volta.
    pub fn barra(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.cabecalho(cx).into_any_element()
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

    /// Sem coleção nenhuma — o estado de quem nunca criou uma.
    fn colecoes_vazias() -> Arc<dyn colecoes::Colecoes> {
        Arc::new(colecoes::mentira::ColecoesDeMentira::default())
    }

    fn tela_com_colecoes(
        cx: &mut TestAppContext,
        porta: Arc<colecoes::mentira::ColecoesDeMentira>,
    ) -> gpui::WindowHandle<Biblioteca> {
        let (previews, dir) = previews_descartaveis();
        std::mem::forget(dir);
        cx.update(gpui_component::init);
        cx.add_window(move |window, cx| {
            Biblioteca::nova(acervo(), previews, marcador(), porta, window, cx)
        })
    }

    /// 🚨 **Apagar pede confirmação, e a tecla sozinha não apaga.**
    ///
    /// `Delete` fica ao lado das treze teclas de triagem, que se apertam às
    /// centenas numa sessão de 800 fotos. Sem o passo de confirmação, um dedo
    /// fora do lugar tira fotos do catálogo em lote — e leva a revelação delas
    /// junto, porque os 46 ajustes moram na linha da foto.
    #[gpui::test]
    fn a_tecla_de_apagar_sozinha_nao_apaga(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let marcador = Arc::new(MarcadorDeMentira::default());

        let janela = cx.add_window({
            let previews = previews.clone();
            let marcador = marcador.clone();
            |window, cx| {
                Biblioteca::nova(acervo(), previews, marcador, colecoes_vazias(), window, cx)
            }
        });

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(0), cx);
                tela.pedir_para_apagar(cx);

                assert_eq!(tela.confirmando_apagar(), Some(1), "tinha de perguntar");
                assert_eq!(tela.quantas_fotos(), 3, "e não apagar nada ainda");
            })
            .expect("a janela deve estar aberta");

        assert!(
            marcador.apagados().is_empty(),
            "a tecla sozinha mandou apagar — não pode"
        );
    }

    /// ⚠️ **Cancelar não apaga nada**, e é o caminho que mais se usa num aviso.
    #[gpui::test]
    fn cancelar_o_aviso_nao_apaga(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let marcador = Arc::new(MarcadorDeMentira::default());

        let janela = cx.add_window({
            let previews = previews.clone();
            let marcador = marcador.clone();
            |window, cx| {
                Biblioteca::nova(acervo(), previews, marcador, colecoes_vazias(), window, cx)
            }
        });

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(0), cx);
                tela.pedir_para_apagar(cx);
                tela.cancelar_apagar(cx);
                assert!(tela.confirmando_apagar().is_none());
                assert_eq!(tela.quantas_fotos(), 3);
            })
            .expect("a janela deve estar aberta");

        assert!(marcador.apagados().is_empty());
    }

    /// ✅ **Confirmado, apaga a seleção inteira e a grade some com elas na hora.**
    ///
    /// 🔑 A grade é atualizada em memória, e não relida: apagar 40 fotos e
    /// esperar a releitura deixaria a grade mostrando o que já não existe — e
    /// clicar numa delas abriria a Revelação numa foto sem linha no banco.
    #[gpui::test]
    fn apagar_confirmado_tira_a_selecao_do_catalogo_e_da_grade(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let marcador = Arc::new(MarcadorDeMentira::default());

        let janela = cx.add_window({
            let previews = previews.clone();
            let marcador = marcador.clone();
            |window, cx| {
                Biblioteca::nova(acervo(), previews, marcador, colecoes_vazias(), window, cx)
            }
        });

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(0), cx);
                tela.alternar_uma(1, cx);
                tela.pedir_para_apagar(cx);
                assert_eq!(tela.confirmando_apagar(), Some(2));

                tela.apagar_confirmado(cx);

                assert_eq!(tela.quantas_fotos(), 1, "sobrou uma das três");
                assert_eq!(nomes_visiveis(tela), ["retrato.jpg"]);
                assert_eq!(
                    tela.quantas_selecionadas(),
                    0,
                    "a seleção apontava para posições do acervo antigo"
                );
            })
            .expect("a janela deve estar aberta");

        let mut apagados = marcador.apagados();
        apagados.sort();
        assert_eq!(apagados, ["id-DSC_001.NEF", "id-DSC_002.NEF"]);
    }

    /// 🚨 **Abrir uma coleção filtra a grade; fechar devolve o acervo.**
    ///
    /// É a funcionalidade inteira do ponto de vista de quem usa: o ensaio de um
    /// cliente é uma coleção, e "ver só o dele" é abrir. O backend, o
    /// repositório e os três use cases estavam prontos e testados há meses —
    /// **faltava o controller e faltava a tela**, e sem os dois nada disso era
    /// alcançável por um clique.
    #[gpui::test]
    fn abrir_uma_colecao_filtra_a_grade(cx: &mut TestAppContext) {
        let porta = Arc::new(colecoes::mentira::ColecoesDeMentira::com(vec![
            adapters::controllers::CollectionViewModel {
                id: "ensaio-1".into(),
                name: "Casamento Ana e João".into(),
                photo_count: 1,
            },
        ]));
        // O acervo de teste tem duas fotos; a coleção tem uma.
        porta.responde_fotos("ensaio-1", &["id-DSC_001.NEF"]);

        let janela = tela_com_colecoes(cx, porta);

        // A lista lateral é pedida na abertura.
        janela
            .update(cx, |tela, _window, cx| {
                tela.colher_colecoes(cx);
                assert_eq!(tela.colecoes().lista.len(), 1);
                assert_eq!(tela.quantas_visiveis(), 3, "a grade começa com o acervo");

                tela.abrir_colecao("ensaio-1", cx);
                tela.colher_colecoes(cx);

                assert_eq!(
                    tela.quantas_visiveis(),
                    1,
                    "abrir a coleção tinha de deixar só a foto dela na grade"
                );
                assert_eq!(nomes_visiveis(tela), ["DSC_001.NEF"]);
                assert_eq!(
                    tela.colecoes().nome_da_aberta(),
                    Some("Casamento Ana e João")
                );

                // Clicar de novo na mesma fecha.
                tela.abrir_colecao("ensaio-1", cx);
                assert_eq!(tela.quantas_visiveis(), 3, "fechar devolve o acervo");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🔑 **Criar uma coleção leva a seleção junto** — é um gesto só.
    ///
    /// Uma coleção nova e vazia obrigaria a selecionar de novo o que já estava
    /// selecionado, e é exatamente com uma seleção na mão que alguém decide
    /// criar o ensaio de um cliente.
    #[gpui::test]
    fn criar_colecao_leva_a_selecao_junto(cx: &mut TestAppContext) {
        let porta = Arc::new(colecoes::mentira::ColecoesDeMentira::default());
        let janela = tela_com_colecoes(cx, porta.clone());

        janela
            .update(cx, |tela, _window, cx| {
                tela.alternar_uma(0, cx);
                tela.alternar_uma(1, cx);
                assert_eq!(tela.quantas_selecionadas(), 2);

                tela.criar_colecao("Cliente X".into(), cx);
                // Criar responde na hora; a colheita encadeia o acrescentar.
                tela.colher_colecoes(cx);
                tela.colher_colecoes(cx);
            })
            .expect("a janela deve estar aberta");

        assert_eq!(
            porta.criadas.lock().expect("as criadas").clone(),
            ["Cliente X"]
        );
        let acrescentados = porta
            .acrescentados
            .lock()
            .expect("os acrescentados")
            .clone();
        assert_eq!(
            acrescentados.len(),
            1,
            "um lote, e não uma chamada por foto"
        );
        assert_eq!(
            acrescentados[0].1.len(),
            2,
            "as duas selecionadas tinham de ir para a coleção nova"
        );
    }

    /// ⚠️ **Nome vazio não cria coleção.**
    ///
    /// Uma coleção sem nome aparece na lista como um botão em branco, e a única
    /// forma de descobrir o que tem dentro é abrir — num painel que existe para
    /// dizer o que tem onde.
    #[gpui::test]
    fn nome_vazio_nao_cria_colecao(cx: &mut TestAppContext) {
        let porta = Arc::new(colecoes::mentira::ColecoesDeMentira::default());
        let janela = tela_com_colecoes(cx, porta.clone());

        janela
            .update(cx, |tela, _window, cx| {
                tela.criar_colecao("   ".into(), cx);
                tela.colher_colecoes(cx);
            })
            .expect("a janela deve estar aberta");

        assert!(porta.criadas.lock().expect("as criadas").is_empty());
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
            |window, cx| Biblioteca::nova(fotos, previews, marcador, colecoes_vazias(), window, cx)
        });

        // 🚨 **O dock é montado aqui também**, e não só no `app.rs`. Sem esta
        // linha os testes desenhavam uma Biblioteca **sem painel nenhum** — uma
        // tela que o app nunca tem, e onde qualquer defeito de dock passaria
        // despercebido. Foi o primeiro teste a tocar no dock que acusou isso.
        // 🚨 **Num arquivo descartável**, e não no caminho de verdade: montar lê
        // o arranjo salvo, e um teste que lesse o do catálogo real passaria a
        // depender da tela que o fotógrafo arrumou ontem — além de gravar nela.
        let arquivo = std::env::temp_dir().join(format!(
            "vlb-teste-arranjo-biblioteca-{}.json",
            std::process::id()
        ));
        janela
            .update(cx, |tela, window, cx| {
                let eu = cx.entity();
                tela.montar_o_dock_em(&eu, arquivo.clone(), window, cx);
            })
            .expect("a janela deve estar aberta");
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

    /// 🚨 Mexer no arranjo **grava o arquivo** — depois da espera, e uma vez só.
    ///
    /// A gravação é o único jeito de a arrumação sobreviver a fechar o app, e ela
    /// mora numa `Task` adiada: sem este teste, ela poderia nunca disparar (a
    /// inscrição descartada, o evento não sendo `LayoutChanged`, a espera nunca
    /// terminando) e nada avisaria — o arquivo simplesmente não apareceria, e só
    /// na abertura seguinte alguém notaria que a tela voltou ao padrão.
    #[gpui::test]
    fn mexer_no_arranjo_grava_o_arquivo(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());
        let pasta = TempDir::new().expect("diretório temporário");
        let arquivo = pasta.path().join("arranjo.json");

        janela
            .update(cx, |tela, window, cx| {
                let eu = cx.entity();
                tela.montar_o_dock_em(&eu, arquivo.clone(), window, cx);
            })
            .expect("a janela deve estar aberta");

        assert!(!arquivo.exists(), "montar não grava: só mexer grava");

        // O que um arrasto de divisória emite.
        janela
            .update(cx, |tela, _window, cx| {
                let dock = tela.dock.as_ref().expect("o dock foi montado").clone();
                dock.update(cx, |_area, cx| {
                    cx.emit(gpui_component::dock::DockEvent::LayoutChanged);
                });
            })
            .expect("a janela deve estar aberta");
        cx.run_until_parked();

        assert!(
            !arquivo.exists(),
            "e a gravação espera meio segundo antes de escrever"
        );

        cx.executor()
            .advance_clock(std::time::Duration::from_millis(ESPERA_DO_ARRANJO_MS * 2));
        cx.run_until_parked();

        assert!(
            arquivo.exists(),
            "passada a espera, o arranjo está no disco"
        );
        assert!(
            arranjo::ler_de(&arquivo).is_some(),
            "e volta a ser lido — versão certa, JSON válido"
        );
    }

    /// 🚨 O arranjo gravado volta como **os quatro painéis**, e não como caixas
    /// vazias.
    ///
    /// O `DockArea` guarda só o **nome** de cada painel; quem sabe reconstruí-lo
    /// é o registro global (`register_panel`). Sem esse registro, restaurar um
    /// leiaute salvo devolve `InvalidPanel` — um retângulo escrito *"The
    /// `biblioteca:grade` panel type is not registered"* no lugar da grade. E
    /// isso não falha em lugar nenhum: o app abre, a tela está lá, e o que sumiu
    /// foram as fotos.
    ///
    /// 🔑 **A conferência é pelos painéis vivos, e não pelo retrato.** O
    /// `InvalidPanel::dump` devolve **o estado antigo**, com o nome original
    /// dentro: um teste que gravasse, carregasse e comparasse os dois retratos
    /// passaria com o registro faltando inteiro. Conferido — foi o primeiro
    /// jeito que escrevi, e ele passava com o `register_panel` removido.
    #[gpui::test]
    fn o_arranjo_gravado_volta_com_os_quatro_paineis(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());

        /// Os nomes dos painéis **vivos** dentro do dock.
        fn vivos(item: &gpui_component::dock::DockItem, cx: &gpui::App) -> Vec<&'static str> {
            use gpui_component::dock::DockItem;
            match item {
                DockItem::Tabs { items, .. } => {
                    items.iter().map(|view| view.panel_name(cx)).collect()
                }
                DockItem::Split { items, .. } => {
                    items.iter().flat_map(|filho| vivos(filho, cx)).collect()
                }
                DockItem::Panel { view, .. } => vec![view.panel_name(cx)],
                DockItem::Tiles { .. } => Vec::new(),
            }
        }

        let retrato = janela
            .update(cx, |tela, _window, cx| {
                let dock = tela.dock.as_ref().expect("o dock foi montado");
                assert_eq!(
                    vivos(dock.read(cx).items(), cx),
                    vec![
                        "biblioteca:pastas",
                        "biblioteca:grade",
                        "biblioteca:filmstrip",
                        "biblioteca:informacoes"
                    ],
                    "o arranjo padrão tem os quatro"
                );
                dock.read(cx).dump(cx)
            })
            .expect("a janela deve estar aberta");

        // A volta: restaurar o que foi gravado e olhar o que ficou vivo.
        janela
            .update(cx, |tela, window, cx| {
                let dock = tela.dock.as_ref().expect("o dock foi montado").clone();
                dock.update(cx, |area, cx| {
                    area.load(retrato, window, cx).expect("restaurar o arranjo");
                });

                assert_eq!(
                    vivos(dock.read(cx).items(), cx),
                    vec![
                        "biblioteca:pastas",
                        "biblioteca:grade",
                        "biblioteca:filmstrip",
                        "biblioteca:informacoes"
                    ],
                    "restaurar tem de reconstruir os painéis, e não InvalidPanel"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 As colunas: automático por padrão, e fixo quando alguém escolhe.
    ///
    /// As duas coisas convivem por decisão do dono (17/ago) — o legado só tem o
    /// número fixo e a grade nova só tinha o automático. O que este teste prende
    /// é que a escolha **ganha da janela**: sem isso o botão acende, o número
    /// muda na barra e a grade continua com as colunas que cabem.
    #[gpui::test]
    fn a_escolha_de_colunas_ganha_da_largura_da_janela(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());
        let visual = gpui::VisualTestContext::from_window(janela.into(), cx);
        visual.simulate_resize(gpui::size(px(1600.), px(900.)));

        janela
            .update(cx, |tela, window, cx| {
                let automatico = tela.colunas(window);
                assert!(
                    automatico > 2,
                    "numa janela de 1600px cabe mais que duas colunas: {automatico}"
                );

                tela.escolher_colunas(Some(2), cx);
                assert_eq!(tela.colunas(window), 2);

                // E o automático volta.
                tela.escolher_colunas(None, cx);
                assert_eq!(tela.colunas(window), automatico);
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ Fora da faixa de 1 a 5, a escolha é presa — como no legado.
    #[gpui::test]
    fn a_escolha_de_colunas_fica_entre_uma_e_cinco(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, window, cx| {
                tela.escolher_colunas(Some(0), cx);
                assert_eq!(tela.colunas(window), 1, "zero coluna não desenha nada");

                tela.escolher_colunas(Some(9), cx);
                assert_eq!(tela.colunas(window), 5);
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Clicar numa estrela dá a nota — e vale para a seleção inteira.
    ///
    /// É a mesma ação das teclas `0`–`5`, e ter o clique valendo só para uma
    /// faria a mesma nota significar duas coisas conforme de onde veio.
    #[gpui::test]
    fn clicar_na_estrela_da_a_nota_a_selecao_inteira(cx: &mut TestAppContext) {
        let (janela, marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(1), cx);
                tela.alternar_uma(2, cx);

                // O que o clique na terceira estrela chama.
                tela.dar_nota(3, cx);

                assert_eq!(tela.fotos[1].rating, 3);
                assert_eq!(tela.fotos[2].rating, 3);
            })
            .expect("a janela deve estar aberta");

        assert_eq!(marcador.marcado().len(), 2, "uma gravação por foto");
    }

    /// 🚨 Rejeitar a foto selecionada com o filtro ligado **anda** para a
    /// seguinte.
    ///
    /// Sem isto a seleção fica apontando para uma foto que saiu da grade: o
    /// painel continua mostrando-a, as teclas seguintes caem nela, e "Revelação"
    /// abre uma foto que não está na tela. É a defesa que o legado chama de
    /// `sanitize_develop_selection` — ⚠️ com uma diferença: lá a nova escolhida é
    /// sempre a **primeira** da lista filtrada, e numa triagem de 800 fotos isso
    /// devolve quem tria ao começo a cada foto rejeitada.
    #[gpui::test]
    fn a_selecao_anda_quando_o_filtro_tira_a_foto_da_grade(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, _window, cx| {
                // Notas: 0,1,2,3,4,0,1,2,3,4 — com "★★★ ou mais" ficam 3,4,8,9.
                tela.filtros.nota_minima = NotaMinima(3);
                tela.refiltrar();
                assert_eq!(
                    nomes_visiveis(tela),
                    vec!["03.jpg", "04.jpg", "08.jpg", "09.jpg"]
                );

                tela.selecionar(Some(3), cx);
                // Rejeitar: a foto 3 sai da grade na hora.
                tela.dar_nota(1, cx);

                assert_eq!(
                    tela.foto_selecionada().map(|f| f.name),
                    Some("04.jpg".to_string()),
                    "a seleção tem de andar para a seguinte que ainda está na grade"
                );
                assert_eq!(tela.quantas_selecionadas(), 1);
            })
            .expect("a janela deve estar aberta");
    }

    /// E quando nada mais passa no filtro, a seleção some — em vez de apontar
    /// para uma grade vazia.
    #[gpui::test]
    fn sem_nenhuma_foto_na_grade_a_selecao_e_limpa(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(4), cx);
                tela.filtros.nota_minima = NotaMinima(5);
                tela.refiltrar();
                assert!(
                    nomes_visiveis(tela).is_empty(),
                    "nenhuma tem cinco estrelas"
                );

                assert!(tela.foto_selecionada().is_none());
                assert_eq!(tela.quantas_selecionadas(), 0);
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ Quando o filtro aperta, quem manda é a seleção que sobrou.
    ///
    /// Com três selecionadas e a principal saindo da grade, a principal passa a
    /// ser a primeira das que ficaram — e não uma foto de fora da seleção. Andar
    /// para fora dela faria a tecla seguinte cair onde ninguém escolheu.
    #[gpui::test]
    fn ao_apertar_o_filtro_a_principal_passa_a_ser_a_primeira_que_sobrou(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, _window, cx| {
                // Notas 0..4 repetidas: os índices 3, 4, 8 e 9 têm 3, 4, 3 e 4.
                tela.filtros.nota_minima = NotaMinima(3);
                tela.refiltrar();

                tela.selecionar(Some(4), cx);
                tela.alternar_uma(9, cx);
                tela.alternar_uma(3, cx);
                assert_eq!(tela.quantas_selecionadas(), 3);
                assert_eq!(
                    tela.foto_selecionada().map(|f| f.name),
                    Some("03.jpg".into())
                );

                // O filtro aperta: a 3 (nota 3) sai; a 4 e a 9 (nota 4) ficam.
                tela.filtros.nota_minima = NotaMinima(4);
                tela.refiltrar();

                assert_eq!(
                    tela.quantas_selecionadas(),
                    2,
                    "a que saiu da grade sai da seleção"
                );
                assert_eq!(
                    tela.foto_selecionada().map(|f| f.name),
                    Some("04.jpg".to_string()),
                    "a principal passa a ser a primeira das que sobraram"
                );
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

    /// `B` decide pelo grupo, como `P`: marca as três se alguma falta, e só
    /// desmarca quando todas já estão levadas.
    #[gpui::test]
    fn levada_no_balcao_decide_pelo_grupo_inteiro(cx: &mut TestAppContext) {
        let mut fotos = acervo_grande();
        fotos[1].comprada = true;
        let (janela, marcador, _dir) = tela_com(cx, fotos);

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(1), cx);
                tela.alternar_uma(2, cx);
                tela.alternar_uma(3, cx);

                tela.marcar_comprada(cx);
                assert!(tela.fotos[1].comprada && tela.fotos[2].comprada && tela.fotos[3].comprada);

                tela.marcar_comprada(cx);
                assert!(
                    !tela.fotos[1].comprada && !tela.fotos[2].comprada && !tela.fotos[3].comprada
                );
            })
            .expect("a janela deve estar aberta");

        let marcado = marcador.marcado();
        assert_eq!(marcado.len(), 6, "três marcadas, depois três desmarcadas");
        assert!(marcado[..3]
            .iter()
            .all(|(_, m)| *m == Marca::Comprada(true)));
        assert!(marcado[3..]
            .iter()
            .all(|(_, m)| *m == Marca::Comprada(false)));
    }

    /// 🚨 Clique simples encolhe a seleção para a foto clicada — e **não**
    /// desmarca quando ela já era a única.
    ///
    /// Até 6/set/2026 o desktop desmarcava nesse segundo caso, e a grade do
    /// site nunca fez isso: mesmo gesto, dois resultados conforme o app. Quem
    /// sai de `/dashboard/sessoes-fotograficas` e abre este app não pode ter de
    /// reaprender o clique. A regra que sobrou é a do site, porque o site é a
    /// referência; para desmarcar sem clicar em outra foto existe `Cmd+D`.
    #[gpui::test]
    fn clicar_numa_das_selecionadas_encolhe_para_ela(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(1), cx);
                tela.alternar_uma(2, cx);
                tela.alternar_uma(3, cx);

                tela.alternar_selecao(2, cx);
                assert_eq!(tela.quantas_selecionadas(), 1);
                assert_eq!(tela.ids_selecionados(), vec!["id-02.jpg".to_string()]);

                // E de novo na mesma: continua sendo ela, como no site.
                tela.alternar_selecao(2, cx);
                assert_eq!(tela.quantas_selecionadas(), 1);
                assert_eq!(tela.ids_selecionados(), vec!["id-02.jpg".to_string()]);

                // Quem desmarca é o `Cmd+D` — e o cursor fica onde estava.
                tela.limpar_selecao(cx);
                assert_eq!(tela.quantas_selecionadas(), 0);
                assert_eq!(
                    tela.foto_selecionada().map(|f| f.id),
                    Some("id-02.jpg".to_string()),
                    "desmarcar não é perder o lugar"
                );
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 `Shift+clique` **substitui** pela faixa, e não acrescenta a ela.
    ///
    /// A outra divergência de 6/set/2026: o desktop somava o intervalo ao que
    /// já estava marcado, e o site troca pela faixa entre a âncora e o destino.
    /// Com quatro fotos espalhadas já marcadas, o mesmo gesto dava 4+n de um
    /// lado e n do outro.
    #[gpui::test]
    fn shift_clique_substitui_pela_faixa_como_no_site(cx: &mut TestAppContext) {
        let (janela, _marcador, _dir) = tela_com(cx, acervo_grande());

        janela
            .update(cx, |tela, _window, cx| {
                tela.selecionar(Some(0), cx);
                tela.alternar_uma(5, cx);
                assert_eq!(tela.quantas_selecionadas(), 2);

                // A âncora é o último clique — a foto 5. Estender até a 7 dá
                // três fotos, e não cinco.
                tela.selecionar_ate(7, cx);
                assert_eq!(
                    tela.ids_selecionados(),
                    vec![
                        "id-05.jpg".to_string(),
                        "id-06.jpg".to_string(),
                        "id-07.jpg".to_string()
                    ],
                    "a faixa é a seleção inteira, e mede da âncora"
                );
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
            Biblioteca::nova(
                acervo(),
                previews.clone(),
                marcador(),
                colecoes_vazias(),
                window,
                cx,
            )
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
            Biblioteca::nova(
                acervo(),
                previews.clone(),
                marcador(),
                colecoes_vazias(),
                window,
                cx,
            )
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
