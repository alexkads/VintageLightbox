//! "Do cartão ou pasta…": o menu dos cartões montados, o "Escolher pasta…" e
//! a janela de escolher as fotos da pasta — um componente só, usado pela etapa
//! 2 do assistente de nova sessão e pelo modal "Importar fotos" de dentro da
//! sessão.
//!
//! 🔑 **Um componente, e não dois desenhos parecidos** (dono, 22/set/2026:
//! *"seria bom ser um único componente para reaproveitamento"*). Até esse dia
//! o menu e a janela moravam dentro de `NovaSessao`, e a sessão não tinha
//! nenhum dos dois. O componente não sabe importar: quando o operador confirma,
//! ele **emite** [`EventoDaOrigem::Escolhidas`] com os caminhos marcados, e
//! cada tela os leva pelo caminho dela — o rascunho no assistente, o
//! `enviar_arquivos` na sessão.
//!
//! 🪟 **Dois pedaços, desenhados por quem usa**: o [`OrigemDasFotos::botao`]
//! vai onde a tela quer o botão, e o [`OrigemDasFotos::dialogo`] em qualquer
//! lugar da árvore: é o `Dialog` do gpui-kit, que se desenha adiado e
//! ancorado no canto da janela — por cima do caixa flutuante também.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon};
use gpui_kit::{
    div, img, prelude::*, px, AnyElement, ClickEvent, Context, Div, Entity, EventEmitter,
    FocusHandle, FontWeight, RenderImage, SharedString, Subscription, Task, Window,
};
use infrastructure::cache::preview_manager::PreviewManager;

use crate::estilo;
use crate::importacao::estado::{Descricao, Recado};
use crate::importacao::explorador::{Explorador, GeradorDeMiniaturas, SeletorDePasta};
use crate::recursos::Icone;

const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(100);

/// As portas que o componente usa — as mesmas da importação.
#[derive(Clone)]
pub struct PortasDaOrigem {
    /// Acha os cartões montados, varre a pasta e lê os metadados.
    pub explorador: Arc<dyn Explorador>,
    /// A janela **do sistema** para escolher a pasta.
    pub seletor_de_pasta: Arc<dyn SeletorDePasta>,
    /// Gera miniaturas dos arquivos que ainda estão na câmera ou na pasta.
    pub gerador: Arc<dyn GeradorDeMiniaturas>,
    /// Onde o gerador grava as miniaturas — e de onde a janela as lê.
    pub previews: Arc<PreviewManager>,
}

/// O que o componente conta a quem o usa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventoDaOrigem {
    /// A janela de escolher as fotos abriu, ainda lendo a pasta. Quem mostrava
    /// o botão dentro de outro modal fecha o dele; quem tinha um aviso na tela
    /// o tira.
    Lendo,
    /// "Importar N fotos": os caminhos marcados, na ordem da pasta.
    Escolhidas(Vec<String>),
    /// Algo para dizer ao operador — cada tela mostra no aviso dela.
    Aviso { texto: String, erro: bool },
}

/// Os cartões montados, para o menu "Do cartão ou pasta…".
struct MenuDaOrigem {
    procurando: bool,
    cartoes: Vec<(String, String)>,
}

/// Fotos encontradas numa pasta, antes da cópia.
///
/// A pasta é uma origem, não uma ordem para importar tudo: o fotógrafo pode
/// fazer uma triagem e trazer só parte do ensaio.
struct SelecaoDaPasta {
    raiz: String,
    fotos: Vec<(String, bool)>,
    /// Primeiro item de um intervalo feito com Shift+clique.
    ancora: Option<usize>,
    /// A varredura ainda não voltou: a janela já está aberta, sem fotos.
    lendo: bool,
}

pub struct OrigemDasFotos {
    portas: PortasDaOrigem,
    menu: Option<MenuDaOrigem>,
    selecao: Option<SelecaoDaPasta>,
    metadados: HashMap<String, Descricao>,
    /// Miniaturas dos arquivos que ainda estão na origem.
    miniaturas: HashMap<String, Arc<RenderImage>>,
    gerando_miniaturas: bool,
    maximizada: bool,
    minimizada: bool,
    zoom: Entity<SliderState>,
    zoom_valor: f32,
    /// A janela do sistema está aberta: a resposta chega pelo canal.
    escolhendo: bool,
    /// Uma varredura de cartão/pasta está no ar: o resultado chega pelo canal
    /// e só a colheita o mostra, então ela não pode dormir antes.
    varrendo: bool,
    origens: (Sender<Recado>, Receiver<Recado>),
    /// O foco da janela de escolher — ver o comentário em `desenhar_dialogo`.
    foco: FocusHandle,
    /// Quem tinha o foco antes de a janela abrir, para recebê-lo de volta.
    devolver_foco: Option<FocusHandle>,
    colhendo: bool,
    _colheita: Option<Task<()>>,
    _assinaturas: Vec<Subscription>,
}

impl EventEmitter<EventoDaOrigem> for OrigemDasFotos {}

impl OrigemDasFotos {
    pub fn nova(portas: PortasDaOrigem, cx: &mut Context<Self>) -> Self {
        let zoom = cx.new(|_| {
            SliderState::new()
                .min(0.7)
                .max(1.5)
                .step(0.05)
                .default_value(1.0)
        });
        let assinatura = cx.subscribe(&zoom, |origem, _, evento: &SliderEvent, cx| {
            // O `Release` (novo no gpui-kit 0.6) chega depois do último `Change`
            // com o mesmo valor: tratá-lo gravaria duas vezes.
            let SliderEvent::Change(valor) = evento else {
                return;
            };
            origem.zoom_valor = valor.start();
            cx.notify();
        });
        Self {
            portas,
            menu: None,
            selecao: None,
            metadados: HashMap::new(),
            miniaturas: HashMap::new(),
            gerando_miniaturas: false,
            maximizada: false,
            minimizada: false,
            zoom,
            zoom_valor: 1.0,
            escolhendo: false,
            varrendo: false,
            origens: channel(),
            foco: cx.focus_handle(),
            devolver_foco: None,
            colhendo: false,
            _colheita: None,
            _assinaturas: vec![assinatura],
        }
    }

    /// Se a janela de escolher as fotos está aberta.
    pub fn escolhendo_fotos(&self) -> bool {
        self.selecao.is_some()
    }

    /// Se o menu dos cartões está aberto.
    pub fn menu_aberto(&self) -> bool {
        self.menu.is_some()
    }

    /// "Do cartão ou pasta…": abre o menu e procura os cartões. O segundo
    /// clique fecha.
    pub fn abrir_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            cx.notify();
            return;
        }
        self.menu = Some(MenuDaOrigem {
            procurando: true,
            cartoes: Vec::new(),
        });
        self.portas.explorador.origens(self.origens.0.clone());
        self.acompanhar(window, cx);
        cx.notify();
    }

    /// Fecha o menu; devolve se ele estava aberto (o `Esc` da tela).
    pub fn fechar_menu(&mut self, cx: &mut Context<Self>) -> bool {
        let estava = self.menu.take().is_some();
        if estava {
            cx.notify();
        }
        estava
    }

    /// Abre a janela na hora, vazia e "lendo": a varredura de um cartão lento
    /// não deixa a tela muda até terminar.
    fn abrir_selecao_lendo(&mut self, raiz: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.selecao.is_none() {
            self.devolver_foco = window.focused(cx);
        }
        window.focus(&self.foco, cx);
        self.miniaturas.clear();
        self.metadados.clear();
        self.gerando_miniaturas = false;
        self.maximizada = false;
        self.minimizada = false;
        self.varrendo = true;
        self.selecao = Some(SelecaoDaPasta {
            raiz,
            fotos: Vec::new(),
            ancora: None,
            lendo: true,
        });
        cx.emit(EventoDaOrigem::Lendo);
    }

    /// A janela fechou: o foco volta a quem o tinha.
    fn fechar_selecao(&mut self, window: &mut Window, cx: &mut gpui_kit::App) {
        self.selecao = None;
        self.varrendo = false;
        self.gerando_miniaturas = false;
        self.minimizada = false;
        if let Some(foco) = self.devolver_foco.take() {
            window.focus(&foco, cx);
        }
    }

    pub fn ler_cartao(&mut self, caminho: String, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.abrir_selecao_lendo(caminho.clone(), window, cx);
        self.portas
            .explorador
            .varrer(caminho, true, self.origens.0.clone());
        self.acompanhar(window, cx);
        cx.notify();
    }

    pub fn escolher_pasta(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.selecao = None;
        self.gerando_miniaturas = false;
        self.escolhendo = true;
        self.portas
            .seletor_de_pasta
            .escolher(self.origens.0.clone(), cx);
        self.acompanhar(window, cx);
        cx.notify();
    }

    pub fn marcar_foto(&mut self, indice: usize, shift: bool, cx: &mut Context<Self>) {
        if let Some(selecao) = self.selecao.as_mut() {
            let Some((_, atual)) = selecao.fotos.get(indice) else {
                return;
            };
            let marcado = !*atual;
            if shift {
                if let Some(ancora) = selecao.ancora {
                    let (inicio, fim) = if ancora <= indice {
                        (ancora, indice)
                    } else {
                        (indice, ancora)
                    };
                    for (_, item) in &mut selecao.fotos[inicio..=fim] {
                        *item = marcado;
                    }
                } else if let Some((_, atual)) = selecao.fotos.get_mut(indice) {
                    *atual = marcado;
                }
            } else if let Some((_, atual)) = selecao.fotos.get_mut(indice) {
                *atual = marcado;
                selecao.ancora = Some(indice);
            }
        }
        cx.notify();
    }

    pub fn marcar_todas(&mut self, marcado: bool, cx: &mut Context<Self>) {
        if let Some(selecao) = self.selecao.as_mut() {
            for (_, atual) in &mut selecao.fotos {
                *atual = marcado;
            }
            selecao.ancora = None;
        }
        cx.notify();
    }

    /// Alterna a seleção total da janela (`⌘A` no macOS, `Ctrl+A` nos demais
    /// sistemas), como a tela de importação principal já faz.
    pub fn alternar_todas(&mut self, cx: &mut Context<Self>) {
        let todas = self
            .selecao
            .as_ref()
            .is_some_and(|selecao| selecao.fotos.iter().all(|(_, marcado)| *marcado));
        self.marcar_todas(!todas, cx);
    }

    pub fn cancelar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fechar_selecao(window, cx);
        cx.notify();
    }

    pub fn alternar_maximizacao(&mut self, cx: &mut Context<Self>) {
        self.maximizada = !self.maximizada;
        self.minimizada = false;
        cx.notify();
    }

    pub fn alternar_minimizacao(&mut self, cx: &mut Context<Self>) {
        self.minimizada = !self.minimizada;
        cx.notify();
    }

    /// "Importar N fotos": fecha a janela e entrega os caminhos marcados.
    pub fn importar_selecao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selecao.as_ref().is_some_and(|s| s.lendo) {
            return;
        }
        let Some(selecao) = self.selecao.take() else {
            return;
        };
        let fotos: Vec<String> = selecao
            .fotos
            .into_iter()
            .filter_map(|(caminho, marcado)| marcado.then_some(caminho))
            .collect();
        self.fechar_selecao(window, cx);
        cx.notify();
        if !fotos.is_empty() {
            cx.emit(EventoDaOrigem::Escolhidas(fotos));
        }
    }

    fn avisar(&mut self, texto: String, erro: bool, cx: &mut Context<Self>) {
        cx.emit(EventoDaOrigem::Aviso { texto, erro });
    }

    // ── Colheita ─────────────────────────────────────────────────────────

    fn acompanhar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;
        self._colheita = Some(cx.spawn_in(window, async move |origem, cx| loop {
            cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
            let continua = origem
                .update_in(cx, |origem, window, cx| origem.colher(window, cx))
                .unwrap_or(false);
            if !continua {
                break;
            }
        }));
    }

    /// Drena o canal. Devolve se vale continuar acordando.
    pub fn colher(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(recado) = self.origens.1.try_recv() {
            mudou = true;
            match recado {
                Recado::Origens { cartoes, .. } => {
                    if let Some(menu) = self.menu.as_mut() {
                        menu.procurando = false;
                        menu.cartoes = cartoes.into_iter().map(|o| (o.nome, o.caminho)).collect();
                    }
                }
                Recado::OrigemEscolhida(pasta) => {
                    self.menu = None;
                    self.selecao = None;
                    self.escolhendo = false;
                    self.abrir_selecao_lendo(pasta.clone(), window, cx);
                    self.portas
                        .explorador
                        .varrer(pasta, true, self.origens.0.clone());
                }
                Recado::SemEscolha => self.escolhendo = false,
                Recado::Descritos(descricoes) => {
                    for descricao in descricoes {
                        self.metadados.insert(descricao.caminho.clone(), descricao);
                    }
                }
                Recado::MiniaturasProntas(caminhos) => {
                    self.gerando_miniaturas = false;
                    for caminho in caminhos {
                        if let Some(imagem) = self.portas.previews.get_thumbnail(
                            &crate::importacao::explorador::chave_de_miniatura(&caminho),
                        ) {
                            self.miniaturas
                                .insert(caminho, crate::imagem::para_gpui(imagem));
                        }
                    }
                }
                Recado::Varrido { raiz, arquivos } => {
                    self.varrendo = false;
                    // Cancelado (ou trocado por outra pasta) enquanto lia: o
                    // resultado velho não reabre nada.
                    if !self
                        .selecao
                        .as_ref()
                        .is_some_and(|s| s.lendo && s.raiz == raiz)
                    {
                        continue;
                    }
                    let nome = std::path::Path::new(&raiz)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| raiz.clone());
                    let fotos: Vec<String> = arquivos
                        .into_iter()
                        .filter(|a| {
                            crate::sessoes::arquivos::so_as_fotos(&[PathBuf::from(a)]).len() == 1
                        })
                        .collect();
                    if fotos.is_empty() {
                        self.fechar_selecao(window, cx);
                        self.avisar(format!("Nenhuma foto em {nome}."), false, cx);
                    } else {
                        self.gerando_miniaturas = true;
                        self.portas
                            .gerador
                            .gerar(fotos.clone(), self.origens.0.clone());
                        self.portas
                            .explorador
                            .detalhar(fotos.clone(), self.origens.0.clone());
                        self.selecao = Some(SelecaoDaPasta {
                            raiz,
                            fotos: fotos.into_iter().map(|caminho| (caminho, true)).collect(),
                            ancora: None,
                            lendo: false,
                        });
                    }
                }
                Recado::Falhou(erro) => {
                    self.escolhendo = false;
                    self.varrendo = false;
                    if self.selecao.as_ref().is_some_and(|s| s.lendo) {
                        self.fechar_selecao(window, cx);
                    }
                    self.avisar(erro, true, cx);
                }
                _ => {}
            }
        }
        if mudou {
            cx.notify();
        }

        let continua = self.escolhendo
            || self.varrendo
            || self.gerando_miniaturas
            // Os metadados chegam depois da varredura, e só a colheita os põe
            // na janela: enquanto ela está aberta, alguém tem de acordar.
            || self.selecao.is_some()
            || self.menu.as_ref().is_some_and(|m| m.procurando);
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    // ── Desenho ──────────────────────────────────────────────────────────

    /// A janela de escolher as fotos, no `Dialog` do gpui-kit — `None` quando
    /// fechada. Só o "Cancelar" e o "Importar" a fecham, como antes: o véu e o
    /// `Esc` não descartam uma seleção feita foto a foto.
    pub fn dialogo(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let selecao = self.selecao.as_ref()?;
        let largura = self.largura_do_dialogo(window);
        let miolo = self
            .desenhar_dialogo(selecao, window, cx)
            .into_any_element();
        crate::dialogo::desenhar_conteudo(
            Some(miolo),
            None,
            crate::dialogo::Jeito::sem_saida(largura),
            |origem, window, cx| origem.cancelar(window, cx),
            window,
            cx,
        )
    }

    /// 88% da janela até 820 px; maximizada, 94% até 1440.
    fn largura_do_dialogo(&self, window: &Window) -> f32 {
        let largura_janela = f32::from(window.viewport_size().width);
        if self.maximizada {
            (largura_janela * 0.94).min(1440.)
        } else {
            (largura_janela * 0.88).min(820.)
        }
    }

    /// "Do cartão ou pasta…", com o menu dos cartões e o "Escolher pasta…".
    pub fn botao(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        div()
            .relative()
            .child(
                estilo::botao_contorno("origem-botao", cx)
                    .debug_selector(|| "origem-botao".into())
                    .child(Icon::new(Icone::HardDrive).size(px(16.)))
                    .child("Do cartão ou pasta…")
                    .on_click(cx.listener(|origem, _, window, cx| origem.abrir_menu(window, cx))),
            )
            .when_some(self.menu.as_ref(), |c, menu| {
                c.child(
                    v_flex()
                        .absolute()
                        // A área das fotos fica dentro de uma coluna com
                        // `overflow_hidden`. Abrir para baixo fazia o menu
                        // ser cortado pela borda inferior dessa etapa.
                        .bottom(px(36.))
                        .left_0()
                        .min_w(px(240.))
                        .p(px(4.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(tema.border)
                        .bg(tema.popover)
                        .shadow_lg()
                        .child(
                            div()
                                .px(px(8.))
                                .py(px(6.))
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tema.muted_foreground)
                                .child("Cartões conectados"),
                        )
                        .when(menu.procurando, |m| {
                            m.child(div().px(px(8.)).py(px(6.)).text_sm().child("Procurando…"))
                        })
                        .when(!menu.procurando && menu.cartoes.is_empty(), |m| {
                            m.child(
                                div()
                                    .px(px(8.))
                                    .py(px(6.))
                                    .text_sm()
                                    .text_color(tema.muted_foreground)
                                    .child("Nenhum cartão encontrado"),
                            )
                        })
                        .children(menu.cartoes.iter().enumerate().map(|(i, (nome, caminho))| {
                            let caminho = caminho.clone();
                            h_flex()
                                .id(SharedString::from(format!("origem-cartao-{i}")))
                                .gap(px(8.))
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(px(4.))
                                .text_sm()
                                .cursor_pointer()
                                .hover(|h| h.bg(tema.accent))
                                .child(Icon::new(Icone::HardDrive).size(px(16.)))
                                .child(nome.clone())
                                .on_click(cx.listener(move |origem, _, window, cx| {
                                    cx.stop_propagation();
                                    origem.ler_cartao(caminho.clone(), window, cx)
                                }))
                        }))
                        .child(div().my(px(4.)).h(px(1.)).bg(tema.border))
                        .child(
                            h_flex()
                                .id("origem-escolher-pasta")
                                .debug_selector(|| "origem-escolher-pasta".into())
                                .gap(px(8.))
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(px(4.))
                                .text_sm()
                                .cursor_pointer()
                                .hover(|h| h.bg(tema.accent))
                                .child(Icon::new(Icone::FolderInput).size(px(16.)))
                                .child("Escolher pasta…")
                                .on_click(cx.listener(|origem, _, window, cx| {
                                    cx.stop_propagation();
                                    origem.escolher_pasta(window, cx)
                                })),
                        ),
                )
            })
            .into_any_element()
    }

    fn desenhar_dialogo(
        &self,
        selecao: &SelecaoDaPasta,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let tema = cx.theme().clone();
        let selecionadas = selecao.fotos.iter().filter(|(_, marcado)| *marcado).count();
        let todas = selecionadas == selecao.fotos.len();
        let minimizada = self.minimizada;
        let maximizada = self.maximizada;
        let zoom = self.zoom_valor;
        let largura_modal = self.largura_do_dialogo(window);
        let altura_janela = window.viewport_size().height;
        let colunas = ((largura_modal - 64.) / (178. * zoom))
            .floor()
            .clamp(2., 8.) as u16;
        let nome_da_pasta = std::path::Path::new(&selecao.raiz)
            .file_name()
            .map(|nome| nome.to_string_lossy().to_string())
            .unwrap_or_else(|| selecao.raiz.clone());

        // 🪟 Véu, caixa e canto são do `Dialog` do gpui-kit ([`Self::dialogo`]);
        // a altura é a de antes (82% da janela, 94% maximizada), menos o
        // respiro de 16 da caixa.
        v_flex()
                .max_h(altura_janela * if maximizada { 0.94 } else { 0.82 } - px(32.))
                .gap(px(14.))
                // 🔑 **O foco é do diálogo**, e os atalhos de marcar também —
                // nas duas telas. `Cmd/Ctrl+A` e `Cmd/Ctrl+D` chegam como ação
                // (a raiz as liga a `SelecionarTudo` e `LimparSelecao`), e o
                // GPUI despacha a ação pelo caminho do foco: com ele aqui, é
                // este diálogo quem responde primeiro, e a grade da sessão por
                // trás não marca nada (dono, 2026-09-21).
                .track_focus(&self.foco)
                .on_action(cx.listener(|origem, _: &crate::app::SelecionarTudo, _, cx| {
                    origem.marcar_todas(true, cx)
                }))
                .on_action(cx.listener(|origem, _: &crate::app::LimparSelecao, _, cx| {
                    origem.marcar_todas(false, cx)
                }))
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(16.))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w(px(0.))
                                .gap(px(3.))
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Escolha as fotos da pasta"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(tema.muted_foreground)
                                        .truncate()
                                        .child(format!(
                                            "{nome_da_pasta} — clique para selecionar; Shift seleciona um intervalo."
                                        )),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .id("origem-minimizar-selecao-pasta")
                                        .p(px(8.))
                                        .rounded(px(7.))
                                        .cursor_pointer()
                                        .hover(|d| d.bg(tema.accent))
                                        .tooltip(|window, cx| {
                                            gpui_kit::component::tooltip::Tooltip::new("Minimizar")
                                                .build(window, cx)
                                        })
                                        .child(Icon::new(Icone::Minus).size(px(17.)))
                                        .on_click(cx.listener(|origem, _, _, cx| {
                                            origem.alternar_minimizacao(cx)
                                        })),
                                )
                                .child(
                                    div()
                                        .id("origem-maximizar-selecao-pasta")
                                        .p(px(8.))
                                        .rounded(px(7.))
                                        .cursor_pointer()
                                        .hover(|d| d.bg(tema.accent))
                                        .tooltip(move |window, cx| {
                                            gpui_kit::component::tooltip::Tooltip::new(if maximizada {
                                                "Restaurar tamanho"
                                            } else {
                                                "Maximizar"
                                            })
                                            .build(window, cx)
                                        })
                                        .child(
                                            Icon::new(if maximizada {
                                                Icone::Minimize2
                                            } else {
                                                Icone::Maximize2
                                            })
                                            .size(px(17.)),
                                        )
                                        .on_click(cx.listener(|origem, _, _, cx| {
                                            origem.alternar_maximizacao(cx)
                                        })),
                                ),
                        ),
                )
                .when(!minimizada, |modal| {
                    modal
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .gap(px(8.))
                                .p(px(8.))
                                .rounded(px(9.))
                                .bg(tema.muted)
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(Icon::new(Icone::ZoomOut).size(px(15.)))
                                        .child(
                                            div()
                                                .w(px(240.))
                                                .child(Slider::new(&self.zoom).horizontal()),
                                        )
                                        .child(Icon::new(Icone::ZoomIn).size(px(15.)))
                                        .child(
                                            div()
                                                .w(px(42.))
                                                .text_xs()
                                                .text_color(tema.muted_foreground)
                                                .child(format!("{:.0}%", zoom * 100.)),
                                        ),
                                )
                                .child(
                                    Checkbox::new("origem-marcar-todas-da-pasta")
                                        .label(if todas {
                                            "Desmarcar todas"
                                        } else {
                                            "Marcar todas"
                                        })
                                        .checked(todas)
                                        .on_click(cx.listener(move |origem, marcado: &bool, _, cx| {
                                            origem.marcar_todas(*marcado, cx)
                                        })),
                                ),
                        )
                        .child(
                            v_flex()
                                .id("origem-fotos-da-pasta-lista")
                                .flex_1()
                                .when(selecao.lendo, |lista| {
                                    lista
                                        .min_h(px(220.))
                                        .items_center()
                                        .justify_center()
                                        .gap(px(10.))
                                        .text_sm()
                                        .text_color(tema.muted_foreground)
                                        .child(Icon::new(Icone::ImagePlus).size(px(26.)))
                                        .child("Lendo as fotos…")
                                })
                                .min_h(px(0.))
                                .overflow_y_scroll()
                                .when(!selecao.lendo, |lista| {
                                    lista.child(
                                    div()
                                        .id("origem-fotos-da-pasta-grade")
                                        .grid()
                                        .grid_cols(colunas)
                                        .gap(px(10.))
                                        .children(selecao.fotos.iter().enumerate().map(
                                            |(indice, (caminho, marcado))| {
                                                let nome = std::path::Path::new(caminho)
                                                    .file_name()
                                                    .map(|n| n.to_string_lossy().to_string())
                                                    .unwrap_or_else(|| caminho.clone());
                                                let miniatura =
                                                    self.miniaturas.get(caminho).cloned();
                                                let tem_miniatura = miniatura.is_some();
                                                let data_hora = self
                                                    .metadados
                                                    .get(caminho)
                                                    .map(|descricao| {
                                                        data_hora_da_foto(&descricao.data)
                                                    })
                                                    .unwrap_or_else(|| "Lendo data…".into());
                                                v_flex()
                                                    .id(SharedString::from(format!(
                                                        "origem-foto-da-pasta-{indice}"
                                                    )))
                                                    .relative()
                                                    .min_w(px(0.))
                                                    .p(px(7.))
                                                    .gap(px(6.))
                                                    .rounded(px(10.))
                                                    .border_1()
                                                    .border_color(if *marcado {
                                                        tema.primary
                                                    } else {
                                                        tema.border
                                                    })
                                                    .when(*marcado, |card| {
                                                        card.bg(tema.primary.opacity(0.12))
                                                    })
                                                    .hover(|card| card.bg(tema.muted))
                                                    .on_click(cx.listener(
                                                        move |origem, evento: &ClickEvent, _, cx| {
                                                            origem.marcar_foto(
                                                                indice,
                                                                evento.modifiers().shift,
                                                                cx,
                                                            )
                                                        },
                                                    ))
                                                    .child(
                                                        div()
                                                            .relative()
                                                            .w_full()
                                                            .h(px(132. * zoom))
                                                            .rounded(px(7.))
                                                            .overflow_hidden()
                                                            .bg(tema.muted)
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .when_some(miniatura, |quadro, imagem| {
                                                                quadro.child(
                                                                    img(imagem).size_full().object_fit(
                                                                        gpui_kit::ObjectFit::Cover,
                                                                    ),
                                                                )
                                                            })
                                                            .when(!tem_miniatura, |quadro| {
                                                                quadro.child(
                                                                    Icon::new(Icone::ImagePlus)
                                                                        .size(px(26.))
                                                                        .text_color(
                                                                            tema.muted_foreground,
                                                                        ),
                                                                )
                                                            })
                                                            .child(
                                                                div()
                                                                    .absolute()
                                                                    .top(px(7.))
                                                                    .right(px(7.))
                                                                    .size(px(25.))
                                                                    .rounded_full()
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .border_1()
                                                                    .border_color(if *marcado {
                                                                        tema.primary
                                                                    } else {
                                                                        tema.border
                                                                    })
                                                                    .when(*marcado, |selo| {
                                                                        selo.bg(tema.primary)
                                                                            .text_color(tema.primary_foreground)
                                                                            .child(
                                                                                Icon::new(Icone::Check)
                                                                                    .size(px(14.)),
                                                                            )
                                                                    }),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .truncate()
                                                            .text_xs()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .child(nome),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .justify_between()
                                                            .gap(px(6.))
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(
                                                                        tema.muted_foreground,
                                                                    )
                                                                    .child(format!(
                                                                        "Foto {}",
                                                                        indice + 1
                                                                    )),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(
                                                                        tema.muted_foreground,
                                                                    )
                                                                    .truncate()
                                                                    .child(data_hora),
                                                            ),
                                                    )
                                                    .into_any_element()
                                            },
                                        )),
                                    )
                                }),
                        )
                })
                .child(
                    h_flex()
                        .mt(px(4.))
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_sm()
                                .text_color(tema.muted_foreground)
                                .child(if selecao.lendo {
                                    "Lendo as fotos…".to_string()
                                } else {
                                    format!(
                                        "{} de {} selecionadas",
                                        selecionadas,
                                        selecao.fotos.len()
                                    )
                                }),
                        )
                        .child(div().flex_1())
                        .child(
                            estilo::botao_contorno("origem-cancelar-selecao-pasta", cx)
                                .child("Cancelar")
                                .on_click(
                                    cx.listener(|origem, _, window, cx| {
                                        origem.cancelar(window, cx)
                                    }),
                                ),
                        )
                        .child(
                            estilo::botao_primario("origem-importar-selecao-pasta", cx)
                                .debug_selector(|| "origem-importar-selecao-pasta".into())
                                .child(format!(
                                    "Importar {}",
                                    super::nova::estado::plural(selecionadas, "foto", "fotos")
                                ))
                                .on_click(cx.listener(|origem, _, window, cx| {
                                    origem.importar_selecao(window, cx)
                                })),
                        ),
                )
    }
}

fn data_hora_da_foto(valor: &str) -> String {
    let mut partes = valor.split_whitespace();
    let Some(data) = partes.next() else {
        return "Lendo data…".into();
    };
    let hora = partes.next().unwrap_or("");
    if data.len() >= 10 && data.as_bytes().get(4) == Some(&b':') {
        let minutos = hora.get(..5).unwrap_or(hora);
        return format!(
            "{}/{}/{} {}",
            &data[8..10],
            &data[5..7],
            &data[..4],
            minutos
        );
    }
    if valor.trim().is_empty() {
        "Data não disponível".into()
    } else {
        valor.to_string()
    }
}

#[cfg(test)]
impl OrigemDasFotos {
    /// 🧪 Abre a janela já varrida, com as fotos dadas desmarcadas.
    pub(crate) fn abrir_selecao_para_teste(
        &mut self,
        fotos: &[&str],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.abrir_selecao_lendo("/cartao".into(), window, cx);
        if let Some(selecao) = self.selecao.as_mut() {
            selecao.lendo = false;
            selecao.fotos = fotos.iter().map(|f| (f.to_string(), false)).collect();
        }
        cx.notify();
    }

    /// 🧪 Quantas fotos estão marcadas na janela.
    pub(crate) fn marcadas(&self) -> usize {
        self.selecao
            .as_ref()
            .map_or(0, |s| s.fotos.iter().filter(|(_, m)| *m).count())
    }
}
