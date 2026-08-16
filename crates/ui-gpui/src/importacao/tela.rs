//! O modal de importação: de onde vem, o que entra, e para onde vai.
//!
//! **Modal, e não tela.** A Biblioteca continua desenhada atrás — importar é uma
//! tarefa que começa e termina, não um lugar onde se fica. É a decisão que o
//! legado tomou ao reescrever a tela em 15/ago (`CurrentView::Import` deixou de
//! existir lá), e ela se preserva.
//!
//! ## A ordem das leituras, que é a regra conquistada da fase 3
//!
//! 1. **Varrer** devolve só caminhos — a grade aparece cheia na hora;
//! 2. **descrever** lê o cabeçalho EXIF e as células vão se completando;
//! 3. **conferir duplicatas** lê o arquivo inteiro para o hash, e chega por
//!    último.
//!
//! Emendar as três numa só faria um cartão de 2.000 RAWs travar a janela por
//! minutos antes de mostrar qualquer coisa.

use std::num::NonZeroUsize;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{
    actions, div, img, prelude::*, px, uniform_list, ClickEvent, Context, FocusHandle,
    SharedString, Task, Window,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::{ActiveTheme, Disableable, Selectable, Sizable};
use infrastructure::cache::preview_manager::PreviewManager;

use domain::value_objects::{ImportMode, OrganizationStrategy, RenamePattern};

use super::destino;
use super::estado::{aplicar, Estado, Ordem, Recado, Seguimento};
use super::explorador::{
    chave_de_miniatura, Andamento, Explorador, GeradorDeMiniaturas, Importador, SeletorDePasta,
};
use crate::biblioteca::miniaturas::{CacheDeMiniaturas, Miniatura};

actions!(
    importacao,
    [MarcarTudo, AlternarFoco, DescerFoco, SubirFoco, Confirmar]
);

/// O contexto de teclado do modal.
///
/// 🔑 **Nomeado, e não global.** As teclas daqui — `Enter`, `espaço`, `⌘A` — são
/// as mais disputadas que existem: sem contexto, elas roubariam a busca da
/// Biblioteca desenhada atrás e o `Enter` de qualquer campo de texto futuro.
const CONTEXTO: &str = "Importacao";

pub fn init(cx: &mut gpui::App) {
    cx.bind_keys([
        gpui::KeyBinding::new("cmd-a", MarcarTudo, Some(CONTEXTO)),
        gpui::KeyBinding::new("space", AlternarFoco, Some(CONTEXTO)),
        gpui::KeyBinding::new("down", DescerFoco, Some(CONTEXTO)),
        gpui::KeyBinding::new("up", SubirFoco, Some(CONTEXTO)),
        gpui::KeyBinding::new("enter", Confirmar, Some(CONTEXTO)),
    ]);
}

/// De quanto em quanto a tela pergunta se chegou recado.
///
/// Mais longo que o da Revelação (8ms) de propósito: ali o laço acompanha um
/// arrasto de slider, aqui espera disco. Acordar 120 vezes por segundo para
/// perguntar se um cartão terminou de ser lido é gasto sem contrapartida.
const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

pub struct Importacao {
    pub estado: Estado,
    explorador: Arc<dyn Explorador>,
    importador: Arc<dyn Importador>,
    seletor: Arc<dyn SeletorDePasta>,
    gerador: Arc<dyn GeradorDeMiniaturas>,
    previews: Arc<PreviewManager>,
    /// As miniaturas já convertidas. `Mutex` porque o closure do `uniform_list`
    /// recebe `&mut App`, e não `&mut self` — o mesmo arranjo da Biblioteca.
    cache: Arc<Mutex<CacheDeMiniaturas>>,
    /// De quem já foi pedida miniatura, para não pedir duas vezes.
    ///
    /// 🚨 Sem isto, **cada quadro** pediria de novo tudo o que está na tela: 60
    /// pedidos por segundo por célula visível, cada um abrindo o arquivo no
    /// cartão. É o tipo de laço que só aparece quando o cartão fica lento.
    pedidas: std::collections::HashSet<String>,
    /// Por onde os recados chegam. O `Sender` é clonado a cada pedido.
    recados: (Sender<Recado>, Receiver<Recado>),
    andamentos: (Sender<Andamento>, Receiver<Andamento>),
    /// O que a importação em curso já fez. `None` é "não há importação em curso".
    progresso: Option<Progresso>,
    /// Se há um laço de colheita rodando. Sem esta trava, cada pedido abriria um
    /// laço novo — o mesmo cuidado que a Revelação tem com a GPU.
    colhendo: bool,
    /// Se há um seletor de pasta aberto. É o que segura o laço de colheita
    /// enquanto não há leitura nenhuma em curso.
    esperando_escolha: bool,
    /// O foco do modal — sem ele, nenhuma das cinco teclas chega.
    foco: FocusHandle,
    _colheita: Option<Task<()>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Progresso {
    pub total: usize,
    pub feitos: usize,
    pub falhas: usize,
    pub pulados: usize,
    pub terminou: bool,
}

impl Importacao {
    pub fn nova(
        explorador: Arc<dyn Explorador>,
        importador: Arc<dyn Importador>,
        seletor: Arc<dyn SeletorDePasta>,
        gerador: Arc<dyn GeradorDeMiniaturas>,
        previews: Arc<PreviewManager>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            estado: Estado::default(),
            explorador,
            importador,
            seletor,
            gerador,
            previews,
            // Uma tela de linhas cabe bem em 128: a grade aqui é uma lista, e não
            // um mosaico como o da Biblioteca.
            cache: Arc::new(Mutex::new(CacheDeMiniaturas::nova(
                NonZeroUsize::new(128).expect("128 não é zero"),
            ))),
            pedidas: std::collections::HashSet::new(),
            recados: channel(),
            andamentos: channel(),
            progresso: None,
            colhendo: false,
            esperando_escolha: false,
            foco: cx.focus_handle(),
            _colheita: None,
        }
    }

    /// Põe o foco no modal. Chamado quando ele aparece — `track_focus` rastreia,
    /// não concede, e foi o que deixou o `Esc` da Revelação morto por dois
    /// commits.
    pub fn focar(&self, window: &mut Window) {
        window.focus(&self.foco);
    }

    fn ao_marcar_tudo(&mut self, _acao: &MarcarTudo, _window: &mut Window, cx: &mut Context<Self>) {
        let tudo = self.estado.marcados() == self.estado.candidatos.len();
        self.estado.marcar_todos(!tudo);
        cx.notify();
    }

    fn ao_alternar_foco(
        &mut self,
        _acao: &AlternarFoco,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.estado.alternar_o_foco();
        cx.notify();
    }

    fn ao_descer(&mut self, _acao: &DescerFoco, _window: &mut Window, cx: &mut Context<Self>) {
        self.estado.mover_foco(1);
        cx.notify();
    }

    fn ao_subir(&mut self, _acao: &SubirFoco, _window: &mut Window, cx: &mut Context<Self>) {
        self.estado.mover_foco(-1);
        cx.notify();
    }

    /// `Enter` importa — mas só quando há o que importar e nada em curso.
    ///
    /// ⚠️ A guarda é a mesma do botão, e precisa ser repetida porque tecla não
    /// passa por botão desligado: sem ela, `Enter` durante a importação começaria
    /// a segunda cópia do mesmo lote.
    fn ao_confirmar(&mut self, _acao: &Confirmar, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.importando() {
            self.importar(cx);
        }
    }

    /// Escolhe a origem e manda varrer.
    ///
    /// 🚨 **A origem é trocada antes do pedido**, e é o que permite recusar a
    /// resposta atrasada da anterior: o recado carrega a raiz de onde veio, e o
    /// estado compara com esta.
    pub fn abrir_origem(&mut self, raiz: String, cx: &mut Context<Self>) {
        self.comecar_varredura(raiz);
        self.acompanhar(cx);
        cx.notify();
    }

    /// Troca a origem e pede a varredura. Sem `cx` de propósito: também é chamada
    /// de dentro da colheita, onde o laço já está de pé.
    fn comecar_varredura(&mut self, raiz: String) {
        self.estado.esquecer_candidatos();
        // As miniaturas pedidas eram da listagem anterior. Guardá-las faria a
        // grade nova nunca pedir as dela, se algum caminho se repetisse.
        self.pedidas.clear();
        self.estado.origem = Some(raiz.clone());
        self.estado.varrendo = true;
        self.progresso = None;

        let subpastas = self.estado.opcoes.include_subfolders;
        self.explorador
            .varrer(raiz, subpastas, self.recados.0.clone());
    }

    /// Abre o seletor do sistema. A resposta chega como recado — sempre, mesmo
    /// que seja "desisti".
    pub fn escolher_origem(&mut self, cx: &mut Context<Self>) {
        self.esperando_escolha = true;
        self.seletor.escolher(self.recados.0.clone());
        // 🚨 O laço tem de estar de pé **antes** da resposta: o seletor é uma
        // janela do sistema e pode voltar a qualquer momento. Sem isto, a pasta
        // escolhida ficaria parada no canal até alguma outra coisa acordar a
        // colheita.
        self.acompanhar(cx);
        cx.notify();
    }

    /// Abre o seletor para a pasta de destino.
    pub fn escolher_destino(&mut self, cx: &mut Context<Self>) {
        self.esperando_escolha = true;
        self.seletor.escolher_destino(self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    /// Liga ou desliga "pular duplicatas".
    ///
    /// 🚨 **Ligar desmarca as duplicatas na hora.** Coerência com o que a grade
    /// mostra: se a importação vai pular, a marcação tem de dizer isso **antes**
    /// de o botão ser apertado — senão o rodapé promete 40 fotos e entram 32.
    pub fn pular_duplicatas(&mut self, pular: bool, cx: &mut Context<Self>) {
        self.estado.opcoes.skip_duplicates = pular;
        if pular {
            for candidato in self.estado.candidatos.iter_mut() {
                if candidato.duplicado {
                    candidato.marcado = false;
                }
            }
        }
        cx.notify();
    }

    /// Manda importar o que está marcado.
    pub fn importar(&mut self, cx: &mut Context<Self>) {
        let arquivos = self.estado.caminhos_marcados();
        if arquivos.is_empty() {
            return;
        }

        // `source_root` é o que o `PreserveStructure` usa para saber o que
        // preservar: sem ele, a organização por estrutura de origem não tem
        // origem de onde partir.
        let mut opcoes = self.estado.opcoes.clone();
        opcoes.source_root = self.estado.origem.clone();

        self.progresso = Some(Progresso {
            total: arquivos.len(),
            feitos: 0,
            falhas: 0,
            pulados: 0,
            terminou: false,
        });

        self.importador
            .importar(arquivos, opcoes, self.andamentos.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    pub fn progresso(&self) -> Option<&Progresso> {
        self.progresso.as_ref()
    }

    /// Liga o laço que drena os dois canais, se ainda não houver um.
    fn acompanhar(&mut self, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;

        self._colheita = Some(cx.spawn(async move |esta, cx| loop {
            cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
            let Ok(continua) = esta.update(cx, |tela, cx| tela.colher(cx)) else {
                break;
            };
            if !continua {
                break;
            }
        }));
    }

    /// Drena o que chegou. Devolve se vale continuar acordando.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;

        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            // A espera pelo seletor acaba com qualquer das duas respostas — a
            // pasta escolhida ou a desistência.
            if matches!(recado, Recado::OrigemEscolhida(_) | Recado::SemEscolha) {
                self.esperando_escolha = false;
            }

            if let Recado::MiniaturasProntas(caminhos) = &recado {
                // O cache guarda "ausente" para quem ainda não tinha miniatura;
                // sem esquecer isso, a foto recém-gerada só apareceria quando a
                // célula saísse e voltasse à tela.
                let mut cache = self.cache.lock().expect("o cache");
                for caminho in caminhos {
                    cache.esquecer(&chave_de_miniatura(caminho));
                }
            }

            match aplicar(&mut self.estado, recado) {
                Some(Seguimento::Detalhar(arquivos)) => {
                    self.explorador.detalhar(arquivos, self.recados.0.clone());
                }
                Some(Seguimento::Varrer(raiz)) => self.comecar_varredura(raiz),
                None => {}
            }
        }

        while let Ok(andamento) = self.andamentos.1.try_recv() {
            mudou = true;
            self.anotar(andamento);
        }

        if mudou {
            cx.notify();
        }

        // 🔑 O laço para quando não há mais nada a esperar. Um laço eterno
        // acordaria a cada 100ms pelo resto da sessão — e a tela de importação
        // costuma ficar aberta menos que isso importa, mas o modal fechado não
        // pode continuar cobrando relógio.
        let continua = self.esperando_escolha
            || self.estado.varrendo
            || self.estado.descrevendo
            || self.estado.conferindo_duplicatas
            || self.progresso.as_ref().is_some_and(|p| !p.terminou);
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    fn anotar(&mut self, andamento: Andamento) {
        let Some(progresso) = self.progresso.as_mut() else {
            return;
        };

        match andamento {
            Andamento::Comecou { total } => progresso.total = total,
            Andamento::Feito { .. } => progresso.feitos += 1,
            Andamento::Pulado { .. } => progresso.pulados += 1,
            Andamento::Falhou { caminho, erro } => {
                progresso.falhas += 1;
                // A falha de um arquivo não interrompe o lote, mas não pode
                // sumir: o aviso é o que diz que 39 de 40 entraram.
                self.estado.aviso = Some(format!("{caminho}: {erro}"));
            }
            Andamento::Terminou {
                sucesso,
                falhas,
                pulados,
            } => {
                progresso.feitos = sucesso;
                progresso.falhas = falhas;
                progresso.pulados = pulados;
                progresso.terminou = true;
            }
        }
    }

    fn cabecalho(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let origem: SharedString = self
            .estado
            .origem
            .clone()
            .unwrap_or_else(|| "Nenhuma pasta escolhida".into())
            .into();
        let subpastas = self.estado.opcoes.include_subfolders;

        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .pb(px(8.))
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("escolher-origem")
                    .label("Escolher pasta…")
                    .xsmall()
                    .on_click(cx.listener(|tela, _ev, _window, cx| {
                        tela.escolher_origem(cx);
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .truncate()
                    .text_color(cx.theme().muted_foreground)
                    .child(origem),
            )
            .child(
                Checkbox::new("subpastas")
                    .label("Incluir subpastas")
                    .checked(subpastas)
                    .on_click(cx.listener(|tela, marcado: &bool, _window, cx| {
                        tela.estado.opcoes.include_subfolders = *marcado;
                        // Trocar isto muda o que a origem tem: revarre, se há
                        // origem. Sem isso a caixa mentiria até alguém escolher
                        // outra pasta.
                        if let Some(raiz) = tela.estado.origem.clone() {
                            tela.abrir_origem(raiz, cx);
                        }
                        cx.notify();
                    })),
            )
    }

    fn barra_da_grade(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let so_novos = self.estado.so_novos;
        let ordem_atual = self.estado.ordem;
        let tudo_marcado = self.estado.marcados() == self.estado.candidatos.len()
            && !self.estado.candidatos.is_empty();

        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(4.))
            .py(px(6.))
            .child(
                Button::new("marcar-todos")
                    .label(if tudo_marcado {
                        "Desmarcar tudo"
                    } else {
                        "Marcar tudo"
                    })
                    .xsmall()
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.estado.marcar_todos(!tudo_marcado);
                        cx.notify();
                    })),
            )
            .child(
                Checkbox::new("so-novos")
                    .label("Só os novos")
                    .checked(so_novos)
                    .on_click(cx.listener(|tela, marcado: &bool, _window, cx| {
                        tela.estado.so_novos = *marcado;
                        cx.notify();
                    })),
            )
            .children(Ordem::TODAS.into_iter().map(|ordem| {
                let escolhida = ordem == ordem_atual;
                Button::new(SharedString::from(format!("ordem-{}", ordem.rotulo())))
                    .label(ordem.rotulo())
                    .xsmall()
                    .when(escolhida, |b| b.primary())
                    .selected(escolhida)
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.estado.ordem = ordem;
                        tela.estado.ordenar();
                        cx.notify();
                    }))
            }))
    }

    /// O lado "PARA": o que fazer com o arquivo, e onde ele vai parar.
    fn destino(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let modo = self.estado.opcoes.mode;
        let organizacao = self.estado.opcoes.organization;
        let renomeacao = self.estado.opcoes.rename_pattern.clone();
        let pular = self.estado.opcoes.skip_duplicates;
        let pasta: SharedString = self
            .estado
            .opcoes
            .destination
            .clone()
            .unwrap_or_else(|| "o catálogo".into())
            .into();

        let copia = modo != ImportMode::Add;

        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .w(px(240.))
            .pl(px(10.))
            .border_l_1()
            .border_color(cx.theme().border)
            .child(div().text_xs().child("Para"))
            .child(div().flex().gap(px(2.)).children(
                [ImportMode::Add, ImportMode::Copy, ImportMode::Move].map(|opcao| {
                    let escolhido = opcao == modo;
                    Button::new(SharedString::from(format!("modo-{}", opcao.label())))
                        .label(opcao.label())
                        .xsmall()
                        .when(escolhido, |b| b.primary())
                        .selected(escolhido)
                        .on_click(cx.listener(move |tela, _ev, _window, cx| {
                            tela.estado.opcoes.mode = opcao;
                            cx.notify();
                        }))
                }),
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(modo.description()),
            )
            // ⚠️ Destino, organização e renomeação **só aparecem quando copiam**.
            // No modo `Add` o arquivo fica onde está, e oferecer "organizar por
            // data" ali seria prometer uma arrumação que não vai acontecer.
            .when(copia, |painel| {
                painel
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .pt(px(6.))
                            .child(
                                Button::new("escolher-destino")
                                    .label("Destino…")
                                    .xsmall()
                                    .on_click(cx.listener(|tela, _ev, _window, cx| {
                                        tela.escolher_destino(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .truncate()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(pasta),
                            ),
                    )
                    .child(div().pt(px(4.)).text_xs().child("Organizar"))
                    .child(
                        div().flex().flex_wrap().gap(px(2.)).children(
                            [
                                OrganizationStrategy::ByDate,
                                OrganizationStrategy::PreserveStructure,
                                OrganizationStrategy::IntoOneFolder,
                            ]
                            .map(|opcao| {
                                let escolhida = opcao == organizacao;
                                Button::new(SharedString::from(format!("org-{}", opcao.label())))
                                    .label(opcao.label())
                                    .xsmall()
                                    .when(escolhida, |b| b.primary())
                                    .selected(escolhida)
                                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                                        tela.estado.opcoes.organization = opcao;
                                        cx.notify();
                                    }))
                            }),
                        ),
                    )
                    .child(div().pt(px(4.)).text_xs().child("Nomear"))
                    .child(div().flex().flex_wrap().gap(px(2.)).children(
                        [RenamePattern::Standard, RenamePattern::KeepOriginal].map(|opcao| {
                            let escolhida = opcao == renomeacao;
                            let rotulo = opcao.label();
                            Button::new(SharedString::from(format!("nome-{rotulo}")))
                                .label(rotulo)
                                .xsmall()
                                .when(escolhida, |b| b.primary())
                                .selected(escolhida)
                                .on_click(cx.listener(move |tela, _ev, _window, cx| {
                                    tela.estado.opcoes.rename_pattern = opcao.clone();
                                    cx.notify();
                                }))
                        }),
                    ))
            })
            // 🔑 A prévia é o que transforma quatro escolhas abstratas numa
            // decisão conferível antes de o botão ser apertado.
            .when_some(destino::previa(&self.estado), |painel, caminho| {
                painel.child(
                    div()
                        .flex()
                        .flex_col()
                        .pt(px(8.))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("A primeira foto vai para"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().primary)
                                .child(SharedString::from(caminho)),
                        ),
                )
            })
            .child(
                div().pt(px(8.)).child(
                    Checkbox::new("pular-duplicatas")
                        .label("Não importar duplicadas")
                        .checked(pular)
                        .on_click(cx.listener(|tela, marcado: &bool, _window, cx| {
                            tela.pular_duplicatas(*marcado, cx);
                        })),
                ),
            )
    }

    /// O lado da miniatura na célula.
    const LADO_DA_MINIATURA: f32 = 48.0;

    fn grade(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let visiveis = Arc::new(self.estado.visiveis());
        let candidatos = Arc::new(self.estado.candidatos.clone());
        let focado = self.estado.focado;
        let cache = self.cache.clone();
        let previews = self.previews.clone();
        let esta = cx.entity();

        let cor_apagada = cx.theme().muted_foreground;
        let cor_aviso = cx.theme().warning;
        let cor_foco = cx.theme().accent;

        uniform_list(
            "grade-de-importacao",
            visiveis.len(),
            move |faixa, _window, cx| {
                // 🔑 **É aqui que se sabe o que está na tela.** O `uniform_list`
                // só chama esta função para as linhas visíveis, e é o que
                // transforma "gerar 2.000 miniaturas" em "gerar as 20 que se
                // está olhando".
                let a_pedir: Vec<String> = faixa
                    .clone()
                    .filter_map(|posicao| visiveis.get(posicao))
                    .map(|indice| candidatos[*indice].caminho.clone())
                    .collect();
                esta.update(cx, |tela, _cx| tela.pedir_miniaturas(a_pedir));

                faixa
                    .filter_map(|posicao| visiveis.get(posicao).copied())
                    .map(|indice| {
                        let candidato = &candidatos[indice];
                        let duplicado = candidato.duplicado;
                        let marcado = candidato.marcado;
                        let em_foco = focado == Some(indice);

                        let miniatura = cache
                            .lock()
                            .expect("o cache")
                            .obter(&previews, &chave_de_miniatura(&candidato.caminho));

                        div()
                            .id(SharedString::from(format!("candidato-{indice}")))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(4.))
                            .py(px(2.))
                            .text_xs()
                            .when(em_foco, |linha| linha.bg(cor_foco))
                            .when(duplicado, |linha| linha.text_color(cor_apagada))
                            // Dentro do `uniform_list` o contexto é `&mut App`, e
                            // não `Context<Self>`: o clique volta à entidade pelo
                            // handle, como a grade da Biblioteca faz.
                            .on_click({
                                let esta = esta.clone();
                                move |evento: &ClickEvent, _window, cx| {
                                    esta.update(cx, |tela, cx| {
                                        if evento.modifiers().shift {
                                            let marcar = !tela.estado.candidatos[indice].marcado;
                                            tela.estado.marcar_ate(indice, marcar);
                                        } else {
                                            tela.estado.alternar(indice);
                                        }
                                        cx.notify();
                                    });
                                }
                            })
                            .child(
                                Checkbox::new(SharedString::from(format!("marca-{indice}")))
                                    .checked(marcado),
                            )
                            .child(
                                div()
                                    .w(px(Self::LADO_DA_MINIATURA))
                                    .h(px(Self::LADO_DA_MINIATURA))
                                    .flex_shrink_0()
                                    .bg(cor_foco)
                                    .when_some(
                                        match miniatura {
                                            Miniatura::Pronta(imagem) => Some(imagem),
                                            // Ausente é normal: a miniatura ainda
                                            // não foi gerada, e o retângulo vazio
                                            // é o que diz isso sem alarmar.
                                            Miniatura::Ausente => None,
                                        },
                                        |celula, imagem| celula.child(img(imagem).size_full()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .truncate()
                                    .child(SharedString::from(candidato.nome.clone())),
                            )
                            .child(
                                div()
                                    .w(px(110.))
                                    .truncate()
                                    .text_color(cor_apagada)
                                    .child(SharedString::from(candidato.camera.clone())),
                            )
                            .child(
                                div()
                                    .w(px(60.))
                                    .text_color(cor_apagada)
                                    .child(SharedString::from(tamanho_legivel(candidato.tamanho))),
                            )
                            .when(duplicado, |linha| {
                                linha.child(div().text_color(cor_aviso).child("já no catálogo"))
                            })
                            .into_any_element()
                    })
                    .collect()
            },
        )
        .flex_1()
        .h_full()
    }

    /// Pede as miniaturas que ainda não foram pedidas.
    fn pedir_miniaturas(&mut self, caminhos: Vec<String>) {
        let novos: Vec<String> = caminhos
            .into_iter()
            .filter(|caminho| self.pedidas.insert(caminho.clone()))
            .collect();

        if !novos.is_empty() {
            self.gerador.gerar(novos, self.recados.0.clone());
        }
    }

    fn rodape(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let marcados = self.estado.marcados();
        let bytes = tamanho_legivel(self.estado.bytes_marcados());
        let total = self.estado.candidatos.len();

        let resumo = match self.progresso.as_ref() {
            Some(progresso) if progresso.terminou => format!(
                "{} importadas · {} falharam · {} puladas",
                progresso.feitos, progresso.falhas, progresso.pulados
            ),
            Some(progresso) => format!("importando {} de {}…", progresso.feitos, progresso.total),
            None => format!("{marcados} de {total} marcadas · {bytes}"),
        };

        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .pt(px(8.))
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(resumo)),
            )
            .child(
                Button::new("importar")
                    .label(SharedString::from(format!("Importar {marcados}")))
                    .xsmall()
                    .primary()
                    .disabled(marcados == 0 || self.importando())
                    .on_click(cx.listener(|tela, _ev, _window, cx| {
                        tela.importar(cx);
                    })),
            )
    }

    /// Se há importação em curso — o botão fica desligado enquanto isso.
    ///
    /// ⚠️ Sem esta guarda, dois cliques seguidos importariam o lote duas vezes: a
    /// segunda passada acharia tudo duplicado e pularia, mas com `skip_duplicates`
    /// desligado ela copiaria cada arquivo de novo.
    pub fn importando(&self) -> bool {
        self.progresso.as_ref().is_some_and(|p| !p.terminou)
    }
}

impl Render for Importacao {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lendo = self.estado.varrendo || self.estado.descrevendo;

        div()
            .key_context(CONTEXTO)
            .track_focus(&self.foco)
            .on_action(cx.listener(Self::ao_marcar_tudo))
            .on_action(cx.listener(Self::ao_alternar_foco))
            .on_action(cx.listener(Self::ao_descer))
            .on_action(cx.listener(Self::ao_subir))
            .on_action(cx.listener(Self::ao_confirmar))
            .flex()
            .flex_col()
            .size_full()
            .p(px(12.))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.cabecalho(cx))
            .child(self.barra_da_grade(cx))
            .when(lendo, |tela| {
                tela.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(if self.estado.varrendo {
                            "Lendo a origem…"
                        } else {
                            "Lendo os metadados…"
                        }),
                )
            })
            .when_some(self.estado.aviso.clone(), |tela, aviso| {
                tela.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().warning)
                        .child(SharedString::from(aviso)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap(px(10.))
                    .child(self.grade(cx))
                    .child(self.destino(cx)),
            )
            .child(self.rodape(cx))
    }
}

/// Bytes no formato que a tela mostra.
fn tamanho_legivel(bytes: u64) -> String {
    const UNIDADES: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut valor = bytes as f64;
    let mut unidade = 0;

    while valor >= 1024.0 && unidade < UNIDADES.len() - 1 {
        valor /= 1024.0;
        unidade += 1;
    }

    if unidade == 0 {
        format!("{bytes} B")
    } else {
        format!("{valor:.1} {}", UNIDADES[unidade])
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use gpui::TestAppContext;

    use super::super::explorador::mentira::{
        ExploradorDeMentira, GeradorDeMentira, ImportadorDeMentira, SeletorDeMentira,
    };

    fn janela(
        cx: &mut TestAppContext,
        explorador: Arc<ExploradorDeMentira>,
        importador: Arc<ImportadorDeMentira>,
    ) -> gpui::WindowHandle<Importacao> {
        com_seletor(
            cx,
            explorador,
            importador,
            Arc::new(SeletorDeMentira::default()),
        )
    }

    fn com_seletor(
        cx: &mut TestAppContext,
        explorador: Arc<ExploradorDeMentira>,
        importador: Arc<ImportadorDeMentira>,
        seletor: Arc<SeletorDeMentira>,
    ) -> gpui::WindowHandle<Importacao> {
        cx.update(gpui_component::init);
        let dir = tempfile::TempDir::new().expect("diretório temporário");
        let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
        // O `TempDir` é solto de propósito: ele vive o teste inteiro dentro do
        // `PreviewManager`, e guardá-lo exigiria devolver duas coisas de cada
        // fábrica de janela.
        std::mem::forget(dir);

        cx.add_window(move |_window, cx| {
            Importacao::nova(
                explorador,
                importador,
                seletor,
                Arc::new(GeradorDeMentira::default()),
                previews,
                cx,
            )
        })
    }

    /// Deixa a colheita rodar até drenar o que já chegou.
    fn colher(cx: &mut TestAppContext, janela: &gpui::WindowHandle<Importacao>) {
        for _ in 0..10 {
            let _ = janela.update(cx, |tela, _window, cx| tela.colher(cx));
            cx.run_until_parked();
        }
    }

    #[test]
    fn tamanho_legivel_troca_de_unidade() {
        assert_eq!(tamanho_legivel(0), "0 B");
        assert_eq!(tamanho_legivel(512), "512 B");
        assert_eq!(tamanho_legivel(2048), "2.0 KB");
        assert_eq!(tamanho_legivel(5 * 1024 * 1024), "5.0 MB");
    }

    /// 🚨 A ordem das leituras: varrer, depois detalhar.
    ///
    /// A grade tem de aparecer cheia **antes** dos metadados. Se a tela esperasse
    /// o EXIF para mostrar a primeira célula, um cartão de 2.000 RAWs ficaria
    /// minutos em branco.
    #[gpui::test]
    fn varrer_enche_a_grade_antes_de_detalhar(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde(
            "/cartao",
            &["/cartao/a.NEF", "/cartao/b.NEF"],
        ));
        let janela = janela(
            cx,
            explorador.clone(),
            Arc::new(ImportadorDeMentira::default()),
        );

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_origem("/cartao".into(), cx);
                assert!(tela.estado.varrendo);
            })
            .expect("a janela deve estar aberta");

        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.estado.candidatos.len(), 2);
                assert!(tela.estado.candidatos.iter().all(|c| c.descrito));
                assert!(!tela.estado.varrendo && !tela.estado.descrevendo);
            })
            .expect("a janela deve estar aberta");

        assert_eq!(
            explorador.pedidos(),
            ["varrer:/cartao", "detalhar:2"],
            "detalhar só depois de varrer, e só o que a varredura achou"
        );
    }

    /// 🚨 A pasta escolhida no seletor cai direto na varredura.
    ///
    /// O seletor é uma janela do sistema e responde quando quiser — inclusive
    /// depois de a tela já ter parado de esperar qualquer outra coisa. É por isso
    /// que escolher liga o laço de colheita antes de abrir o diálogo.
    #[gpui::test]
    fn escolher_pasta_dispara_a_varredura(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde(
            "/escolhida",
            &["/escolhida/a.NEF"],
        ));
        let janela = com_seletor(
            cx,
            explorador.clone(),
            Arc::new(ImportadorDeMentira::default()),
            Arc::new(SeletorDeMentira::escolhe("/escolhida")),
        );

        janela
            .update(cx, |tela, _window, cx| {
                tela.escolher_origem(cx);
            })
            .expect("a janela deve estar aberta");

        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.estado.origem.as_deref(), Some("/escolhida"));
                assert_eq!(tela.estado.candidatos.len(), 1);
            })
            .expect("a janela deve estar aberta");

        assert_eq!(
            explorador.pedidos(),
            ["varrer:/escolhida", "detalhar:1"],
            "escolher pasta é o começo da mesma sequência"
        );
    }

    /// 🚨 Desistir do seletor solta o laço de colheita.
    ///
    /// Sem o recado de desistência, a tela esperaria para sempre uma pasta que
    /// nunca vem — e o laço acordaria a cada 100ms pelo resto da sessão, com o
    /// modal fechado e ninguém olhando.
    #[gpui::test]
    fn desistir_do_seletor_solta_o_laco(cx: &mut TestAppContext) {
        let janela = com_seletor(
            cx,
            Arc::new(ExploradorDeMentira::default()),
            Arc::new(ImportadorDeMentira::default()),
            Arc::new(SeletorDeMentira::default()),
        );

        janela
            .update(cx, |tela, _window, cx| {
                tela.escolher_origem(cx);
                assert!(tela.esperando_escolha);
            })
            .expect("a janela deve estar aberta");

        janela
            .update(cx, |tela, _window, cx| {
                assert!(!tela.colher(cx), "a desistência chegou e o laço para");
                assert!(!tela.esperando_escolha);
                assert!(tela.estado.origem.is_none(), "nada foi escolhido");
            })
            .expect("a janela deve estar aberta");
    }

    /// Trocar de origem esquece a listagem anterior antes de pedir a nova.
    #[gpui::test]
    fn trocar_de_origem_limpa_a_grade(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde("/a", &["/a/1.NEF"]));
        let janela = janela(cx, explorador, Arc::new(ImportadorDeMentira::default()));

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_origem("/a".into(), cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                assert_eq!(tela.estado.candidatos.len(), 1);

                tela.abrir_origem("/b".into(), cx);
                assert!(tela.estado.candidatos.is_empty(), "a grade some na hora");
                assert_eq!(tela.estado.origem.as_deref(), Some("/b"));
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 A miniatura de cada arquivo é pedida **uma vez só**.
    ///
    /// O `uniform_list` chama a função de renderização a cada quadro. Sem a
    /// lembrança do que já foi pedido, seriam 60 pedidos por segundo por célula
    /// visível — cada um abrindo o arquivo no cartão. É o tipo de laço que só
    /// aparece quando o cartão fica lento, e aí parece problema do cartão.
    #[gpui::test]
    fn a_miniatura_e_pedida_uma_vez_so(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde(
            "/cartao",
            &["/cartao/a.NEF", "/cartao/b.NEF"],
        ));
        let gerador = Arc::new(GeradorDeMentira::default());
        cx.update(gpui_component::init);

        let janela = cx.add_window({
            let gerador = gerador.clone();
            move |_window, cx| {
                let dir = tempfile::TempDir::new().expect("diretório temporário");
                let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
                std::mem::forget(dir);

                Importacao::nova(
                    explorador,
                    Arc::new(ImportadorDeMentira::default()),
                    Arc::new(SeletorDeMentira::default()),
                    gerador,
                    previews,
                    cx,
                )
            }
        });

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_origem("/cartao".into(), cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        // Três quadros pedindo as mesmas duas células.
        janela
            .update(cx, |tela, _window, _cx| {
                let visiveis: Vec<String> = tela
                    .estado
                    .candidatos
                    .iter()
                    .map(|c| c.caminho.clone())
                    .collect();
                for _ in 0..3 {
                    tela.pedir_miniaturas(visiveis.clone());
                }
            })
            .expect("a janela deve estar aberta");

        // ⚠️ A conta é sobre **caminhos repetidos**, e não sobre número de
        // pedidos: o próprio render já pediu os visíveis dele antes deste laço, e
        // contar chamadas mediria o desenho em vez da regra.
        let todos: Vec<String> = gerador.pedidos().into_iter().flatten().collect();
        let distintos: std::collections::HashSet<&String> = todos.iter().collect();

        assert_eq!(
            todos.len(),
            distintos.len(),
            "algum arquivo foi pedido duas vezes: {todos:?}"
        );
        assert!(distintos.len() <= 2, "só existem dois arquivos");
    }

    /// Trocar de origem esquece o que já foi pedido.
    ///
    /// 🔑 As miniaturas pedidas eram da listagem anterior. Guardá-las faria a
    /// grade nova nunca pedir as dela, se algum caminho se repetisse — o que
    /// acontece ao revarrer a mesma pasta com "incluir subpastas" trocado.
    #[gpui::test]
    fn trocar_de_origem_esquece_os_pedidos(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde("/cartao", &["/cartao/a.NEF"]));
        let janela = janela(cx, explorador, Arc::new(ImportadorDeMentira::default()));

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_origem("/cartao".into(), cx);
                tela.pedir_miniaturas(vec!["/cartao/a.NEF".into()]);
                assert_eq!(tela.pedidas.len(), 1);

                tela.abrir_origem("/cartao".into(), cx);
                assert!(tela.pedidas.is_empty());
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 As cinco teclas do modal chegam mesmo.
    ///
    /// `Enter`, `espaço`, `⌘A` e as setas são as teclas mais disputadas que
    /// existem, e uma ligação que não casa **não falha** — ela simplesmente não
    /// faz nada. Foi assim que o `Esc` da Revelação ficou dois commits morto, e
    /// por isso este teste aperta as teclas de verdade.
    #[gpui::test]
    fn as_teclas_do_modal_chegam(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde(
            "/cartao",
            &["/cartao/a.NEF", "/cartao/b.NEF", "/cartao/c.NEF"],
        ));
        let importador = Arc::new(ImportadorDeMentira::default());
        cx.update(gpui_component::init);
        cx.update(init);

        let janela = cx.add_window({
            let explorador = explorador.clone();
            let importador = importador.clone();
            move |_window, cx| {
                let dir = tempfile::TempDir::new().expect("diretório temporário");
                let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
                std::mem::forget(dir);

                Importacao::nova(
                    explorador,
                    importador,
                    Arc::new(SeletorDeMentira::default()),
                    Arc::new(GeradorDeMentira::default()),
                    previews,
                    cx,
                )
            }
        });

        janela
            .update(cx, |tela, window, cx| {
                tela.focar(window);
                tela.abrir_origem("/cartao".into(), cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);

        // ⌘A com tudo marcado desmarca tudo.
        visual.simulate_keystrokes("cmd-a");
        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.estado.marcados(), 0, "⌘A com tudo marcado desmarca");
            })
            .expect("a janela deve estar aberta");

        // Duas descidas e um espaço marcam a segunda célula.
        visual.simulate_keystrokes("down down space");
        janela
            .update(cx, |tela, _window, _cx| {
                assert_eq!(tela.estado.focado, Some(1), "a seta moveu o foco");
                assert!(tela.estado.candidatos[1].marcado, "o espaço marcou");
                assert_eq!(tela.estado.marcados(), 1);
            })
            .expect("a janela deve estar aberta");

        // E o Enter importa o que está marcado.
        visual.simulate_keystrokes("enter");
        assert_eq!(
            importador.importados()[0].0,
            ["/cartao/b.NEF"],
            "o Enter importa exatamente o que a grade mostra marcado"
        );
    }

    /// ⚠️ `Enter` durante a importação não começa a segunda cópia do mesmo lote.
    ///
    /// A guarda do botão não vale para a tecla — tecla não passa por botão
    /// desligado, e sem repetir a checagem o `Enter` copiaria tudo de novo.
    #[gpui::test]
    fn enter_durante_a_importacao_nao_repete_o_lote(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde("/cartao", &["/cartao/a.NEF"]));
        let importador = Arc::new(ImportadorDeMentira::default());
        let janela = janela(cx, explorador, importador.clone());

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_origem("/cartao".into(), cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, window, cx| {
                tela.importar(cx);
                // A importação está em curso: o Enter tem de ser ignorado.
                tela.ao_confirmar(&Confirmar, window, cx);
            })
            .expect("a janela deve estar aberta");

        assert_eq!(importador.importados().len(), 1, "um lote, e não dois");
    }

    /// 🚨 Shift+clique marca o intervalo a partir da âncora.
    #[gpui::test]
    fn shift_clique_marca_o_intervalo(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde(
            "/cartao",
            &[
                "/cartao/a.NEF",
                "/cartao/b.NEF",
                "/cartao/c.NEF",
                "/cartao/d.NEF",
            ],
        ));
        let janela = janela(cx, explorador, Arc::new(ImportadorDeMentira::default()));

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_origem("/cartao".into(), cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, _cx| {
                tela.estado.marcar_todos(false);
                // Clique comum na primeira: vira âncora.
                tela.estado.alternar(0);
                // Shift+clique na terceira: marca o intervalo.
                tela.estado.marcar_ate(2, true);

                assert_eq!(tela.estado.marcados(), 3);
                assert!(!tela.estado.candidatos[3].marcado, "a quarta fica fora");
            })
            .expect("a janela deve estar aberta");
    }

    /// 🚨 Ligar "não importar duplicadas" desmarca as duplicatas **na hora**.
    ///
    /// Coerência com o que a grade mostra: se a importação vai pular, a marcação
    /// tem de dizer isso antes de o botão ser apertado — senão o rodapé promete
    /// 40 fotos e entram 32, e a diferença só aparece no fim.
    #[gpui::test]
    fn pular_duplicatas_desmarca_na_hora(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde(
            "/cartao",
            &["/cartao/a.NEF", "/cartao/b.NEF"],
        ));
        let janela = janela(cx, explorador, Arc::new(ImportadorDeMentira::default()));

        janela
            .update(cx, |tela, _window, cx| {
                tela.estado.opcoes.skip_duplicates = false;
                tela.abrir_origem("/cartao".into(), cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                // Uma delas já está no catálogo.
                tela.estado.candidatos[0].duplicado = true;
                assert_eq!(tela.estado.marcados(), 2);

                tela.pular_duplicatas(true, cx);
                assert_eq!(tela.estado.marcados(), 1, "a duplicata saiu da marcação");

                // Desligar de volta **não** remarca: quem desmarcou à mão não
                // pode ter a escolha desfeita por uma caixa de opção.
                tela.pular_duplicatas(false, cx);
                assert_eq!(tela.estado.marcados(), 1);
            })
            .expect("a janela deve estar aberta");
    }

    /// A pasta de destino escolhida entra nas opções da importação.
    #[gpui::test]
    fn o_destino_escolhido_chega_a_importacao(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde("/cartao", &["/cartao/a.NEF"]));
        let importador = Arc::new(ImportadorDeMentira::default());
        let janela = com_seletor(
            cx,
            explorador,
            importador.clone(),
            Arc::new(SeletorDeMentira::escolhe("/HD/Fotos")),
        );

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_origem("/cartao".into(), cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                tela.escolher_destino(cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                assert_eq!(
                    tela.estado.opcoes.destination.as_deref(),
                    Some("/HD/Fotos"),
                    "e sem revarrer nada: trocar o destino não muda o que a origem tem"
                );
                assert_eq!(tela.estado.candidatos.len(), 1);
                tela.importar(cx);
            })
            .expect("a janela deve estar aberta");

        assert_eq!(
            importador.importados()[0].1.destination.as_deref(),
            Some("/HD/Fotos")
        );
    }

    /// 🚨 Importar leva só o que está marcado, e com a origem nas opções.
    ///
    /// `source_root` é o que o `PreserveStructure` usa para saber que estrutura
    /// preservar; sem ele, a organização por estrutura de origem não tem de onde
    /// partir e joga tudo numa pasta só.
    #[gpui::test]
    fn importar_leva_o_que_esta_marcado(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde(
            "/cartao",
            &["/cartao/a.NEF", "/cartao/b.NEF", "/cartao/c.NEF"],
        ));
        let importador = Arc::new(ImportadorDeMentira::default());
        let janela = janela(cx, explorador, importador.clone());

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_origem("/cartao".into(), cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                tela.estado.alternar(1); // desmarca a b.NEF
                tela.importar(cx);
            })
            .expect("a janela deve estar aberta");

        let importados = importador.importados();
        assert_eq!(importados.len(), 1);
        assert_eq!(
            importados[0].0,
            ["/cartao/a.NEF", "/cartao/c.NEF"],
            "a desmarcada fica"
        );
        assert_eq!(
            importados[0].1.source_root.as_deref(),
            Some("/cartao"),
            "a origem vai junto, para o PreserveStructure ter o que preservar"
        );
    }

    /// Sem nada marcado, importar não faz nada.
    #[gpui::test]
    fn importar_sem_marcacao_nao_faz_nada(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde("/cartao", &["/cartao/a.NEF"]));
        let importador = Arc::new(ImportadorDeMentira::default());
        let janela = janela(cx, explorador, importador.clone());

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_origem("/cartao".into(), cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                tela.estado.marcar_todos(false);
                tela.importar(cx);
                assert!(tela.progresso().is_none());
            })
            .expect("a janela deve estar aberta");

        assert!(importador.importados().is_empty());
    }

    /// 🚨 O botão fica desligado enquanto a importação corre.
    ///
    /// Dois cliques seguidos importariam o lote duas vezes — e com "pular
    /// duplicatas" desligado, a segunda passada copiaria cada arquivo de novo.
    #[gpui::test]
    fn nao_da_para_importar_duas_vezes_ao_mesmo_tempo(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde("/cartao", &["/cartao/a.NEF"]));
        let importador = Arc::new(ImportadorDeMentira::default());
        let janela = janela(cx, explorador, importador.clone());

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_origem("/cartao".into(), cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                tela.importar(cx);
                assert!(tela.importando(), "o botão fica desligado");
            })
            .expect("a janela deve estar aberta");

        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, _cx| {
                let progresso = tela.progresso().expect("há progresso");
                assert!(progresso.terminou);
                assert_eq!(progresso.feitos, 1);
                assert!(!tela.importando(), "e volta a ligar quando termina");
            })
            .expect("a janela deve estar aberta");
    }

    /// ⚠️ O laço de colheita para quando não há mais nada a esperar.
    ///
    /// Um laço eterno acordaria a cada 100ms pelo resto da sessão, com o modal
    /// fechado e ninguém olhando.
    #[gpui::test]
    fn a_colheita_para_quando_nao_ha_o_que_esperar(cx: &mut TestAppContext) {
        let explorador = Arc::new(ExploradorDeMentira::responde("/cartao", &["/cartao/a.NEF"]));
        let janela = janela(cx, explorador, Arc::new(ImportadorDeMentira::default()));

        janela
            .update(cx, |tela, _window, cx| {
                tela.abrir_origem("/cartao".into(), cx);
            })
            .expect("a janela deve estar aberta");
        colher(cx, &janela);

        janela
            .update(cx, |tela, _window, cx| {
                assert!(!tela.colher(cx), "não há mais o que esperar");
                assert!(!tela.colhendo);
            })
            .expect("a janela deve estar aberta");
    }
}
