//! 🗂️ A área de trabalho do editor: os painéis à direita, em grupos de abas
//! do `DockArea` do gpui-kit, como as docas do Photoshop.
//!
//! - **Cor** e **Amostras** em cima, **Propriedades**, **Pincel** e
//!   **Histórico** no meio, **Camadas** embaixo com a maior altura;
//! - arrastar a aba reordena, leva o painel a outro grupo ou cria um grupo
//!   novo (o dock do kit faz o arrasto e as divisórias);
//! - o menu Janela mostra e esconde cada painel; "Recolher painéis" vira a
//!   coluna numa faixa de ícones (clicar num ícone abre a coluna nele);
//!   "Restaurar área de trabalho" volta ao padrão;
//! - `Tab` esconde barra de ferramentas, opções e painéis; `⇧Tab` só os
//!   painéis. Esconder assim é passageiro: não grava nada.
//!
//! 🔑 **Não é o `docas.rs` da Revelação.** Aquele é um dock sem abas e sem
//! arrasto, de propósito (o dono vetou mudar a Revelação de lugar); este é o
//! do Photoshop, só do editor. Um não muda o outro.
//!
//! 💾 **A arrumação é desta máquina** (`editor-area-de-trabalho.json` ao lado
//! do catálogo, como as outras arrumações): grupos, abas, tamanhos, quem está
//! escondido, a largura da coluna e as colunas da barra. Esquema com versão;
//! arquivo de outra versão, ilegível ou com painel desconhecido cai no
//! padrão (o que der para aproveitar, aproveita). A gravação espera o
//! arrasto parar. **Não é do projeto da foto** e não entra no desfazer.
//!
//! 🧱 Os painéis não têm estado próprio: cada um é uma entidade fina que
//! desenha com um método do `EditorDeFoto` (o arranjo da Biblioteca e de
//! `docas::PainelDaTela`), com a volta fraca para não prender a janela.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::dock::{
    panel_handle, BasePanel, DockArea, DockEvent, DockLayout, DockPlacement, DockSkin, PaneNode,
    PaneRef, Panel, PanelEvent, PanelId, PanelInfo, PanelState, PanelStyle,
};
use gpui_kit::component::menu::PopupMenu;
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable,
    Render, SharedString, Subscription, WeakEntity, Window,
};
use serde::{Deserialize, Serialize};

use super::aparencia::{self, medida};
use super::EditorDeFoto;
use crate::recursos::Icone;

/// A versão do esquema gravado. Mudou o formato, sobe — e o arquivo velho cai
/// no padrão em vez de virar uma tela pela metade.
pub const VERSAO: u32 = 1;

/// Quanto se espera, parado, antes de gravar.
const ESPERA_DA_GRAVACAO_MS: u64 = 400;

/// A largura da coluna dos painéis, em pontos.
pub const LIMITES_DA_COLUNA: crate::docas::Limites = crate::docas::Limites {
    minimo: 240.0,
    maximo: 560.0,
    padrao: 300.0,
};

/// Os painéis do editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QualPainel {
    Cor,
    Amostras,
    Propriedades,
    Pincel,
    Historico,
    Camadas,
    Navegador,
    Info,
    /// O painel Caminhos (a Caneta): o de trabalho, os nomeados e a máscara
    /// vetorial da camada escolhida.
    Caminhos,
    Ajustes,
}

impl QualPainel {
    pub const TODOS: [QualPainel; 10] = [
        QualPainel::Cor,
        QualPainel::Amostras,
        QualPainel::Navegador,
        QualPainel::Info,
        QualPainel::Propriedades,
        QualPainel::Pincel,
        QualPainel::Historico,
        QualPainel::Camadas,
        QualPainel::Ajustes,
        QualPainel::Caminhos,
    ];

    /// 🚨 **O nome é o que o arquivo guarda** — mudá-lo faz a arrumação
    /// gravada perder o painel.
    pub fn nome(self) -> &'static str {
        match self {
            QualPainel::Cor => "editor:cor",
            QualPainel::Amostras => "editor:amostras",
            QualPainel::Propriedades => "editor:propriedades",
            QualPainel::Pincel => "editor:pincel",
            QualPainel::Historico => "editor:historico",
            QualPainel::Camadas => "editor:camadas",
            QualPainel::Navegador => "editor:navegador",
            QualPainel::Info => "editor:info",
            QualPainel::Ajustes => "editor:ajustes",
            QualPainel::Caminhos => "editor:caminhos",
        }
    }

    pub fn do_nome(nome: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|q| q.nome() == nome)
    }

    pub fn titulo(self) -> &'static str {
        match self {
            QualPainel::Cor => "Cor",
            QualPainel::Amostras => "Amostras",
            QualPainel::Propriedades => "Propriedades",
            QualPainel::Pincel => "Pincel",
            QualPainel::Historico => "Histórico",
            QualPainel::Camadas => "Camadas",
            QualPainel::Navegador => "Navegador",
            QualPainel::Info => "Info",
            QualPainel::Ajustes => "Ajustes",
            QualPainel::Caminhos => "Caminhos",
        }
    }

    pub fn icone(self) -> Icone {
        match self {
            QualPainel::Cor => Icone::Palette,
            QualPainel::Amostras => Icone::Grid3x3,
            QualPainel::Propriedades => Icone::SlidersHorizontal,
            QualPainel::Pincel => Icone::Paintbrush,
            QualPainel::Historico => Icone::History,
            QualPainel::Camadas => Icone::Layers,
            QualPainel::Navegador => Icone::Map,
            QualPainel::Info => Icone::Info,
            QualPainel::Ajustes => Icone::Contrast,
            QualPainel::Caminhos => Icone::PenTool,
        }
    }

    /// O seletor dos testes e do roteiro.
    pub fn seletor(self) -> &'static str {
        match self {
            QualPainel::Cor => "editor-painel-cor",
            QualPainel::Amostras => "editor-painel-amostras",
            QualPainel::Propriedades => "editor-painel-propriedades",
            QualPainel::Pincel => "editor-painel-pincel",
            QualPainel::Historico => "editor-painel-historico",
            QualPainel::Camadas => "editor-painel-camadas",
            QualPainel::Navegador => "editor-painel-navegador",
            QualPainel::Info => "editor-painel-info",
            QualPainel::Ajustes => "editor-painel-ajustes",
            QualPainel::Caminhos => "editor-painel-caminhos",
        }
    }
}

// ------------------------------------------------------------ o que grava

/// A arrumação, como vai para o disco.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arranjo {
    pub versao: u32,
    /// A largura da coluna dos painéis.
    pub largura: f32,
    /// A coluna recolhida na faixa de ícones.
    #[serde(default)]
    pub recolhido: bool,
    /// Os painéis escondidos (pelo nome).
    #[serde(default)]
    pub ocultos: Vec<String>,
    #[serde(default)]
    pub barra_em_duas_colunas: bool,
    /// As réguas em volta do palco (⌘R).
    #[serde(default)]
    pub reguas: bool,
    /// Visualizar › Guias desligado (⌘;) e Travar guias (⌥⌘;).
    #[serde(default)]
    pub guias_ocultas: bool,
    #[serde(default)]
    pub guias_travadas: bool,
    /// Visualizar › Ajustar desligado (⇧⌘;) — ligado é o padrão de lá.
    #[serde(default)]
    pub sem_ajuste: bool,
    /// Os grupos, as abas e os tamanhos — a árvore do dock do kit.
    #[serde(default)]
    pub grupos: Option<PanelState>,
}

impl Default for Arranjo {
    fn default() -> Self {
        Self {
            versao: VERSAO,
            largura: LIMITES_DA_COLUNA.padrao,
            recolhido: false,
            ocultos: Vec::new(),
            barra_em_duas_colunas: false,
            reguas: false,
            guias_ocultas: false,
            guias_travadas: false,
            sem_ajuste: false,
            grupos: None,
        }
    }
}

impl Arranjo {
    /// Lê o texto gravado. Outra versão ou texto ilegível: `None` (quem chama
    /// cai no padrão).
    pub fn do_texto(texto: &str) -> Option<Self> {
        let mut lido: Arranjo = serde_json::from_str(texto).ok()?;
        if lido.versao != VERSAO {
            return None;
        }
        lido.largura = LIMITES_DA_COLUNA.limitar(lido.largura);
        lido.ocultos.retain(|n| QualPainel::do_nome(n).is_some());
        Some(lido)
    }

    pub fn oculto(&self, qual: QualPainel) -> bool {
        self.ocultos.iter().any(|n| n == qual.nome())
    }

    pub fn definir_oculto(&mut self, qual: QualPainel, oculto: bool) {
        self.ocultos.retain(|n| n != qual.nome());
        if oculto {
            self.ocultos.push(qual.nome().to_string());
        }
    }
}

#[cfg(not(test))]
fn arquivo() -> Option<PathBuf> {
    Some(infrastructure::paths::AppPaths::catalog_root().join("editor-area-de-trabalho.json"))
}

/// Nos testes, em lugar nenhum: a suíte não mexe na arrumação de quem roda.
#[cfg(test)]
fn arquivo() -> Option<PathBuf> {
    None
}

/// A arrumação gravada, ou a padrão. Sem arquivo novo, a largura vem da
/// arrumação de antes (`docas-editor.json`, só a largura do painel).
pub fn ler() -> Arranjo {
    let Some(caminho) = arquivo() else {
        return Arranjo::default();
    };
    match std::fs::read_to_string(&caminho) {
        Ok(texto) => Arranjo::do_texto(&texto).unwrap_or_else(|| {
            crate::telemetria::avisar!(
                "⚠️ a área de trabalho do editor ({}) é de outra versão ou está ilegível: volta ao padrão",
                caminho.display()
            );
            Arranjo::default()
        }),
        Err(_) => Arranjo {
            largura: LIMITES_DA_COLUNA.limitar(
                crate::docas::ler("editor")
                    .direita
                    .map_or(LIMITES_DA_COLUNA.padrao, |c| c.largura),
            ),
            ..Arranjo::default()
        },
    }
}

fn gravar(arranjo: &Arranjo) {
    let Some(caminho) = arquivo() else {
        return;
    };
    let Ok(texto) = serde_json::to_string_pretty(arranjo) else {
        return;
    };
    if let Some(pasta) = caminho.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    if let Err(erro) = std::fs::write(&caminho, texto) {
        crate::telemetria::avisar!("⚠️ a área de trabalho do editor não foi gravada: {erro}");
    }
}

// ------------------------------------------------------------ os painéis

/// Um painel: o nome e a volta fraca ao editor, que desenha.
pub struct PainelDoEditor {
    qual: QualPainel,
    editor: WeakEntity<EditorDeFoto>,
    foco: FocusHandle,
    /// 🚨 O dock do kit guarda o desenho do painel em cache e só o refaz
    /// quando o **painel** muda: sem isto, Camadas e Propriedades ficavam no
    /// quadro de antes de a foto abrir. Toda notificação do editor notifica o
    /// painel.
    _editor_mudou: Subscription,
}

impl Render for PainelDoEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let qual = self.qual;
        let Some(editor) = self.editor.upgrade() else {
            return div().into_any_element();
        };
        let conteudo = editor.update(cx, |ed, cx| ed.desenhar_painel(qual, window, cx));
        div()
            .debug_selector(move || qual.seletor().into())
            .size_full()
            .flex()
            .flex_col()
            .min_h(px(0.))
            // A rolagem de dentro do painel também refaz o desenho em cache
            // (as caixas medidas no quadro — o gráfico das Curvas — andam
            // junto).
            .on_scroll_wheel(cx.listener(|_, _: &gpui_kit::ScrollWheelEvent, _, cx| cx.notify()))
            .child(conteudo)
            .into_any_element()
    }
}

impl Focusable for PainelDoEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.foco.clone()
    }
}

impl EventEmitter<PanelEvent> for PainelDoEditor {}

impl BasePanel for PainelDoEditor {
    fn panel_name(&self) -> &'static str {
        self.qual.nome()
    }

    /// Fechar é esconder pelo menu Janela (ou pelo ⋯ do grupo): o painel
    /// continua vivo e volta onde estava.
    fn closable(&self, _cx: &App) -> bool {
        false
    }

    fn zoomable(&self, _cx: &App) -> bool {
        false
    }

    fn visible(&self, cx: &App) -> bool {
        self.editor
            .upgrade()
            .is_none_or(|ed| !ed.read(cx).arranjo.oculto(self.qual))
    }
}

impl Panel for PainelDoEditor {
    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        SharedString::from(self.qual.titulo())
    }

    fn zoom_control(&self, _cx: &App) -> Option<gpui_kit::component::dock::PanelControl> {
        None
    }

    fn inner_padding(&self, _cx: &App) -> bool {
        false
    }

    /// O ⋯ do grupo: esconder o painel, como o "Fechar" do Photoshop.
    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> PopupMenu {
        let (qual, editor) = (self.qual, self.editor.clone());
        menu.item(
            gpui_kit::component::menu::PopupMenuItem::new(format!("Fechar {}", qual.titulo()))
                .on_click(move |_, _window, cx| {
                    let _ = editor.update(cx, |ed, cx| ed.definir_painel_oculto(qual, true, cx));
                }),
        )
    }
}

// ------------------------------------------------------------ a doca

/// O dock montado e os painéis dele.
pub struct AreaDeTrabalho {
    pub area: Entity<DockArea>,
    pub paineis: Vec<(QualPainel, Entity<PainelDoEditor>)>,
    _assinatura: Subscription,
}

impl AreaDeTrabalho {
    pub fn painel(&self, qual: QualPainel) -> Option<&Entity<PainelDoEditor>> {
        self.paineis
            .iter()
            .find(|(q, _)| *q == qual)
            .map(|(_, p)| p)
    }
}

/// O arranjo padrão: Cor/Amostras em cima, Propriedades/Pincel/Histórico no
/// meio, Camadas embaixo com o resto da altura.
fn arranjo_padrao(paineis: &[(QualPainel, Entity<PainelDoEditor>)], cx: &App) -> DockLayout {
    let p = |q: QualPainel| {
        let entidade = paineis
            .iter()
            .find(|(x, _)| *x == q)
            .map(|(_, e)| e.clone())
            .expect("todo painel existe");
        panel_handle(entidade)
    };
    DockLayout::v_split()
        .child(
            DockLayout::tabs()
                .panel_view(p(QualPainel::Cor), cx)
                .panel_view(p(QualPainel::Amostras), cx)
                .panel_view(p(QualPainel::Navegador), cx)
                .panel_view(p(QualPainel::Info), cx),
            Some(px(150.)),
        )
        .child(
            DockLayout::tabs()
                .panel_view(p(QualPainel::Propriedades), cx)
                .panel_view(p(QualPainel::Pincel), cx)
                .panel_view(p(QualPainel::Historico), cx),
            Some(px(316.)),
        )
        .child(
            DockLayout::tabs()
                .panel_view(p(QualPainel::Camadas), cx)
                .panel_view(p(QualPainel::Caminhos), cx)
                .panel_view(p(QualPainel::Ajustes), cx),
            None,
        )
}

/// A árvore gravada de volta em layout, com os painéis desta janela. Nome
/// desconhecido ou repetido sai; grupo vazio sai; o que sobrar sem lugar
/// ganha um grupo embaixo. `None`: nada aproveitável.
pub fn arranjo_gravado(
    estado: &PanelState,
    paineis: &[(QualPainel, Entity<PainelDoEditor>)],
    cx: &App,
) -> Option<DockLayout> {
    /// Os nomes que a árvore gravada cita.
    fn nomes(estado: &PanelState, saida: &mut HashSet<String>) {
        saida.insert(estado.panel_name.to_string());
        for filho in &estado.children {
            nomes(filho, saida);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn montar(
        estado: &PanelState,
        paineis: &[(QualPainel, Entity<PainelDoEditor>)],
        usados: &mut HashSet<QualPainel>,
        novos: &mut Vec<QualPainel>,
        profundidade: usize,
        cx: &App,
    ) -> Option<DockLayout> {
        if profundidade > 8 {
            return None;
        }
        let painel = |q: QualPainel| {
            paineis
                .iter()
                .find(|(x, _)| *x == q)
                .map(|(_, e)| panel_handle(e.clone()))
        };
        match &estado.info {
            PanelInfo::Stack { sizes, axis } => {
                let mut l = if *axis == 0 {
                    DockLayout::h_split()
                } else {
                    DockLayout::v_split()
                };
                let mut algum = false;
                for (i, filho) in estado.children.iter().enumerate() {
                    if let Some(f) = montar(filho, paineis, usados, novos, profundidade + 1, cx) {
                        let tamanho = sizes
                            .get(i)
                            .map(|t| f32::from(*t))
                            .filter(|t| t.is_finite() && *t >= 40.0)
                            .map(px);
                        l = l.child(f, tamanho);
                        algum = true;
                    }
                }
                algum.then_some(l)
            }
            PanelInfo::Tabs { active_index } => {
                let mut l = DockLayout::tabs();
                let mut n = 0usize;
                for filho in &estado.children {
                    let Some(q) = QualPainel::do_nome(&filho.panel_name) else {
                        continue;
                    };
                    if !usados.insert(q) {
                        continue;
                    }
                    if let Some(p) = painel(q) {
                        l = l.panel_view(p, cx);
                        n += 1;
                    }
                }
                // Um painel que a versão de antes não tinha entra como aba
                // num grupo que já existe — o Navegador no primeiro, o Info
                // no das Camadas —, e não num grupo novo que apertaria os
                // outros.
                if n > 0 {
                    let com_camadas = usados.contains(&QualPainel::Camadas)
                        && estado
                            .children
                            .iter()
                            .any(|c| c.panel_name == QualPainel::Camadas.nome());
                    let primeiro = usados.len() == n;
                    let mut ficam = Vec::new();
                    for q in novos.drain(..) {
                        let aqui = match q {
                            QualPainel::Navegador => primeiro,
                            QualPainel::Info | QualPainel::Ajustes | QualPainel::Caminhos => {
                                com_camadas
                            }
                            _ => false,
                        };
                        match painel(q).filter(|_| aqui && usados.insert(q)) {
                            Some(p) => l = l.panel_view(p, cx),
                            None => ficam.push(q),
                        }
                    }
                    *novos = ficam;
                }
                (n > 0).then(|| l.active_index((*active_index).min(n - 1)))
            }
            PanelInfo::Panel(_) => {
                let q = QualPainel::do_nome(&estado.panel_name)?;
                if !usados.insert(q) {
                    return None;
                }
                Some(DockLayout::tabs().panel_view(painel(q)?, cx))
            }
        }
    }
    let mut usados = HashSet::new();
    let mut citados = HashSet::new();
    nomes(estado, &mut citados);
    let mut novos: Vec<QualPainel> = paineis
        .iter()
        .map(|(q, _)| *q)
        .filter(|q| {
            matches!(
                q,
                QualPainel::Navegador
                    | QualPainel::Info
                    | QualPainel::Ajustes
                    | QualPainel::Caminhos
            )
        })
        .filter(|q| !citados.contains(q.nome()))
        .collect();
    let layout = montar(estado, paineis, &mut usados, &mut novos, 0, cx)?;
    let faltando: Vec<_> = paineis
        .iter()
        .filter(|(q, _)| !usados.contains(q))
        .map(|(_, e)| panel_handle(e.clone()))
        .collect();
    if faltando.is_empty() {
        return Some(layout);
    }
    let mut extra = DockLayout::tabs();
    for p in faltando {
        extra = extra.panel_view(p, cx);
    }
    Some(
        DockLayout::v_split()
            .child(layout, None)
            .child(extra, Some(px(160.))),
    )
}

impl EditorDeFoto {
    /// Monta o dock no primeiro quadro (os painéis guardam a volta fraca ao
    /// editor, que já tem de existir).
    pub(super) fn garantir_a_area_de_trabalho(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.area_de_trabalho.is_some() {
            return;
        }
        let eu_forte = cx.entity();
        let eu = eu_forte.downgrade();
        let paineis: Vec<(QualPainel, Entity<PainelDoEditor>)> = QualPainel::TODOS
            .into_iter()
            .map(|qual| {
                let editor = eu.clone();
                let observado = eu_forte.clone();
                (
                    qual,
                    cx.new(|cx| PainelDoEditor {
                        qual,
                        editor,
                        foco: cx.focus_handle(),
                        _editor_mudou: cx.observe(&observado, |_, _, cx| cx.notify()),
                    }),
                )
            })
            .collect();
        let (area, pele) = DockSkin::dock_area("editor-paineis", Some(VERSAO as usize), window, cx);
        pele.set_panel_style(PanelStyle::TabBar, cx);
        pele.set_toggle_button_visible(false, cx);
        let gravado = self.arranjo.grupos.clone();
        area.update(cx, |area, cx| {
            let layout = gravado
                .as_ref()
                .and_then(|g| arranjo_gravado(g, &paineis, cx))
                .unwrap_or_else(|| arranjo_padrao(&paineis, cx));
            area.set_center(layout, window, cx);
        });
        let _assinatura = cx.subscribe_in(
            &area,
            window,
            |ed: &mut Self, _area, evento: &DockEvent, _window, cx| {
                if matches!(evento, DockEvent::LayoutChanged) {
                    ed.area_de_trabalho_mudou(cx);
                }
            },
        );
        self.area_de_trabalho = Some(AreaDeTrabalho {
            area,
            paineis,
            _assinatura,
        });
    }

    /// Algo da arrumação mudou: grava depois de parar (um arrasto de
    /// divisória emite dezenas de mudanças).
    pub(super) fn area_de_trabalho_mudou(&mut self, cx: &mut Context<Self>) {
        self._gravacao_da_area = Some(cx.spawn(async move |ed, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(ESPERA_DA_GRAVACAO_MS))
                .await;
            let _ = ed.update(cx, |ed, cx| {
                if let Some(a) = ed.area_de_trabalho.as_ref() {
                    ed.arranjo.grupos = Some(a.area.read(cx).dump(cx).center);
                }
                ed.arranjo.barra_em_duas_colunas = ed.barra_em_duas_colunas;
                gravar(&ed.arranjo);
                ed.gravacoes_da_area += 1;
            });
        }));
    }

    /// Quantas vezes a arrumação foi gravada (o roteiro e os testes conferem
    /// que mexer na tela não grava a cada quadro).
    pub fn gravacoes_da_area_de_trabalho(&self) -> usize {
        self.gravacoes_da_area
    }

    pub fn arranjo(&self) -> &Arranjo {
        &self.arranjo
    }

    pub fn painel_visivel(&self, qual: QualPainel) -> bool {
        !self.arranjo.oculto(qual)
    }

    pub fn definir_painel_oculto(
        &mut self,
        qual: QualPainel,
        oculto: bool,
        cx: &mut Context<Self>,
    ) {
        self.arranjo.definir_oculto(qual, oculto);
        self.area_de_trabalho_mudou(cx);
        cx.notify();
    }

    /// O painel está à vista: não escondido, na frente do grupo dele e com a
    /// coluna aberta.
    pub fn painel_na_frente(&self, qual: QualPainel, cx: &App) -> bool {
        fn achar(no: &PaneNode, id: PanelId) -> Option<bool> {
            match no.kind() {
                PaneRef::Split { children, .. } => children.iter().find_map(|f| achar(f, id)),
                PaneRef::Tabs { panels, active_ix } => panels
                    .iter()
                    .position(|p| *p == id)
                    .map(|i| i == active_ix.min(panels.len().saturating_sub(1))),
            }
        }
        if !self.painel_visivel(qual) || self.recolhido_ou_escondido() {
            return false;
        }
        let Some(a) = self.area_de_trabalho.as_ref() else {
            return false;
        };
        let Some(p) = a.painel(qual) else {
            return false;
        };
        let id = PanelId::from(p.entity_id());
        a.area
            .read(cx)
            .layout(DockPlacement::Center)
            .and_then(|t| achar(t.root(), id))
            .unwrap_or(false)
    }

    /// O item do menu Janela: o painel à vista some; o escondido (ou atrás
    /// de outra aba) vem para a frente — como no Photoshop.
    pub fn alternar_painel(
        &mut self,
        qual: QualPainel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.painel_na_frente(qual, cx) {
            self.definir_painel_oculto(qual, true, cx);
        } else {
            self.mostrar_painel(qual, window, cx);
        }
    }

    fn recolhido_ou_escondido(&self) -> bool {
        self.arranjo.recolhido || self.paineis_ocultos || self.interface_oculta
    }

    /// Mostra o painel na frente do grupo dele — abrindo a coluna, se ela
    /// estiver recolhida ou escondida pelo Tab.
    pub fn mostrar_painel(
        &mut self,
        qual: QualPainel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.arranjo.definir_oculto(qual, false);
        self.arranjo.recolhido = false;
        self.paineis_ocultos = false;
        self.interface_oculta = false;
        if let Some(a) = self.area_de_trabalho.as_ref() {
            if let Some(p) = a.painel(qual) {
                let id = PanelId::from(p.entity_id());
                a.area
                    .update(cx, |area, cx| area.select_panel(id, window, cx));
            }
        }
        self.area_de_trabalho_mudou(cx);
        cx.notify();
    }

    pub fn alternar_recolhido(&mut self, cx: &mut Context<Self>) {
        self.arranjo.recolhido = !self.arranjo.recolhido;
        self.paineis_ocultos = false;
        self.area_de_trabalho_mudou(cx);
        cx.notify();
    }

    /// `Tab`: barra, opções e painéis somem juntos, ou voltam juntos.
    pub fn alternar_interface(&mut self, cx: &mut Context<Self>) {
        self.interface_oculta = !self.interface_oculta;
        self.fechar_flyout(cx);
        cx.notify();
    }

    /// `⇧Tab`: só os painéis.
    pub fn alternar_paineis(&mut self, cx: &mut Context<Self>) {
        if self.interface_oculta {
            self.interface_oculta = false;
            self.paineis_ocultos = true;
        } else {
            self.paineis_ocultos = !self.paineis_ocultos;
        }
        cx.notify();
    }

    pub fn interface_oculta(&self) -> bool {
        self.interface_oculta
    }

    pub fn paineis_ocultos(&self) -> bool {
        self.paineis_ocultos
    }

    /// "Restaurar área de trabalho": tudo volta ao padrão e grava.
    pub fn restaurar_area_de_trabalho(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.arranjo = Arranjo::default();
        self.barra_em_duas_colunas = false;
        self.interface_oculta = false;
        self.paineis_ocultos = false;
        let largura = self.arranjo.largura;
        self.colunas.update(cx, |estado, cx| {
            estado.resize_panel(1, px(largura), window, cx);
        });
        if let Some(a) = self.area_de_trabalho.as_ref() {
            let paineis = a.paineis.clone();
            a.area.update(cx, |area, cx| {
                let layout = arranjo_padrao(&paineis, cx);
                area.set_center(layout, window, cx);
            });
        }
        self.area_de_trabalho_mudou(cx);
        cx.notify();
    }

    /// 🧪 Remonta a doca no próximo quadro com esta árvore (o que a leitura
    /// do disco faria ao abrir).
    #[cfg(test)]
    pub fn remontar_a_area_com(&mut self, grupos: Option<PanelState>, cx: &mut Context<Self>) {
        self.arranjo.grupos = grupos;
        self.area_de_trabalho = None;
        cx.notify();
    }

    /// Os painéis à vista no dock, pelo que o dock tem (para os testes e o
    /// roteiro).
    pub fn paineis_no_dock(&self, cx: &App) -> Vec<QualPainel> {
        fn juntar(no: &PaneNode, saida: &mut Vec<PanelId>) {
            match no.kind() {
                PaneRef::Split { children, .. } => children.iter().for_each(|f| juntar(f, saida)),
                PaneRef::Tabs { panels, .. } => saida.extend(panels.iter().copied()),
            }
        }
        let Some(a) = self.area_de_trabalho.as_ref() else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        if let Some(t) = a.area.read(cx).layout(DockPlacement::Center) {
            juntar(t.root(), &mut ids);
        }
        ids.into_iter()
            .filter_map(|id| {
                a.paineis
                    .iter()
                    .find(|(_, e)| PanelId::from(e.entity_id()) == id)
                    .map(|(q, _)| *q)
            })
            .collect()
    }

    /// Os grupos de abas do dock, de cima para baixo, pelo nome dos painéis.
    pub fn grupos_do_dock(&self, cx: &App) -> Vec<Vec<String>> {
        fn juntar(no: &PaneNode, saida: &mut Vec<Vec<PanelId>>) {
            match no.kind() {
                PaneRef::Split { children, .. } => children.iter().for_each(|f| juntar(f, saida)),
                PaneRef::Tabs { panels, .. } => saida.push(panels.to_vec()),
            }
        }
        let Some(a) = self.area_de_trabalho.as_ref() else {
            return Vec::new();
        };
        let mut grupos = Vec::new();
        if let Some(t) = a.area.read(cx).layout(DockPlacement::Center) {
            juntar(t.root(), &mut grupos);
        }
        grupos
            .into_iter()
            .map(|ids| {
                ids.into_iter()
                    .filter_map(|id| {
                        a.paineis
                            .iter()
                            .find(|(_, e)| PanelId::from(e.entity_id()) == id)
                            .map(|(q, _)| q.nome().to_string())
                    })
                    .collect()
            })
            .collect()
    }

    /// A largura da coluna dos painéis.
    pub(super) fn largura_do_painel(&self) -> f32 {
        LIMITES_DA_COLUNA.limitar(self.arranjo.largura)
    }

    /// A borda entre o documento e os painéis foi arrastada.
    pub(super) fn guardar_a_largura_do_painel(&mut self, cx: &mut Context<Self>) {
        let larguras: Vec<f32> = self
            .colunas
            .read(cx)
            .sizes()
            .iter()
            .map(|l| f32::from(*l))
            .collect();
        if let [_, painel] = larguras[..] {
            if painel > 0. {
                self.arranjo.largura = LIMITES_DA_COLUNA.limitar(painel);
                self.area_de_trabalho_mudou(cx);
            }
        }
        cx.notify();
    }

    /// A coluna dos painéis: o cabeçalho com o recolher, e o dock — ou, com
    /// a coluna recolhida, a faixa de ícones.
    pub(super) fn coluna_de_paineis(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let Some(a) = self.area_de_trabalho.as_ref() else {
            return div().into_any_element();
        };
        if self.arranjo.recolhido {
            return self.faixa_de_icones(cx);
        }
        div()
            .id("editor-coluna-de-paineis")
            .debug_selector(|| "editor-coluna-de-paineis".into())
            .size_full()
            .flex()
            .flex_col()
            .bg(c.painel)
            .border_l_1()
            .border_color(c.borda)
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .justify_end()
                    .items_center()
                    .h(px(18.))
                    .px(px(2.))
                    .border_b_1()
                    .border_color(c.borda)
                    .child(
                        crate::estilo::botao_icone(
                            "editor-recolher-paineis",
                            Icone::ChevronsRight,
                            16.,
                            12.,
                        )
                        .tooltip("Recolher os painéis em ícones")
                        .on_click(cx.listener(|ed, _, _, cx| ed.alternar_recolhido(cx))),
                    ),
            )
            .child(div().flex_1().min_h(px(0.)).child(a.area.clone()))
            .into_any_element()
    }

    /// Os painéis recolhidos: um ícone por painel à vista; o clique abre a
    /// coluna nele.
    fn faixa_de_icones(&self, cx: &mut Context<Self>) -> AnyElement {
        let c = aparencia::cores(cx);
        let mut faixa = div()
            .id("editor-faixa-de-icones")
            .debug_selector(|| "editor-faixa-de-icones".into())
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.))
            .w(px(medida::FAIXA_DE_ICONES))
            .h_full()
            .flex_shrink_0()
            .bg(c.painel)
            .border_l_1()
            .border_color(c.borda)
            .child(
                crate::estilo::botao_icone(
                    "editor-expandir-paineis",
                    Icone::ChevronsLeft,
                    16.,
                    12.,
                )
                .tooltip("Expandir os painéis")
                .on_click(cx.listener(|ed, _, _, cx| ed.alternar_recolhido(cx))),
            );
        for qual in QualPainel::TODOS {
            if !self.painel_visivel(qual) {
                continue;
            }
            let id: &'static str = match qual {
                QualPainel::Cor => "editor-icone-cor",
                QualPainel::Amostras => "editor-icone-amostras",
                QualPainel::Propriedades => "editor-icone-propriedades",
                QualPainel::Pincel => "editor-icone-pincel",
                QualPainel::Historico => "editor-icone-historico",
                QualPainel::Camadas => "editor-icone-camadas",
                QualPainel::Navegador => "editor-icone-navegador",
                QualPainel::Info => "editor-icone-info",
                QualPainel::Caminhos => "editor-icone-caminhos",
                QualPainel::Ajustes => "editor-icone-ajustes",
            };
            faixa = faixa.child(
                crate::estilo::botao_icone(id, qual.icone(), 28., 16.)
                    .tooltip(qual.titulo())
                    .on_click(
                        cx.listener(move |ed, _, window, cx| ed.mostrar_painel(qual, window, cx)),
                    ),
            );
        }
        faixa.into_any_element()
    }

    /// O que cada painel desenha.
    pub(super) fn desenhar_painel(
        &mut self,
        qual: QualPainel,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match qual {
            QualPainel::Cor => self.painel_de_cor(cx),
            QualPainel::Amostras => self.painel_de_amostras(cx),
            QualPainel::Propriedades => self.painel_de_propriedades(cx),
            QualPainel::Pincel => self.painel_do_pincel(cx),
            QualPainel::Historico => self.painel_do_historico(cx).into_any_element(),
            QualPainel::Camadas => self.painel_de_camadas(cx).into_any_element(),
            QualPainel::Navegador => self.painel_do_navegador(cx),
            QualPainel::Info => self.painel_de_info(cx),
            QualPainel::Ajustes => self.painel_de_ajustes(cx),
            QualPainel::Caminhos => self.painel_de_caminhos(cx),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn os_nomes_dos_paineis_nao_mudam() {
        let nomes: Vec<_> = QualPainel::TODOS.iter().map(|q| q.nome()).collect();
        assert_eq!(
            nomes,
            [
                "editor:cor",
                "editor:amostras",
                "editor:navegador",
                "editor:info",
                "editor:propriedades",
                "editor:pincel",
                "editor:historico",
                "editor:camadas",
                "editor:ajustes",
                "editor:caminhos"
            ]
        );
        for q in QualPainel::TODOS {
            assert_eq!(QualPainel::do_nome(q.nome()), Some(q));
        }
    }

    /// Arquivo de outra versão, ilegível, com largura absurda ou painel que
    /// não existe mais: nada derruba, e o aproveitável fica.
    #[test]
    fn a_arrumacao_gravada_tolera_o_que_vier() {
        assert_eq!(Arranjo::do_texto("isto não é json"), None);
        assert_eq!(
            Arranjo::do_texto(r#"{"versao":99,"largura":300}"#),
            None,
            "outra versão cai no padrão"
        );
        let lido = Arranjo::do_texto(
            r#"{"versao":1,"largura":9000,"ocultos":["editor:historico","editor:sumiu"]}"#,
        )
        .unwrap();
        assert_eq!(lido.largura, LIMITES_DA_COLUNA.maximo);
        assert_eq!(lido.ocultos, vec!["editor:historico".to_string()]);
        assert!(lido.oculto(QualPainel::Historico));
        assert!(!lido.recolhido);
        assert_eq!(lido.grupos, None);
    }

    #[test]
    fn a_arrumacao_vai_e_volta_pelo_disco() {
        let mut a = Arranjo::default();
        a.definir_oculto(QualPainel::Amostras, true);
        a.recolhido = true;
        a.barra_em_duas_colunas = true;
        a.grupos = Some(PanelState {
            panel_name: "".into(),
            children: vec![PanelState::new("editor:camadas")],
            info: PanelInfo::tabs(0),
        });
        let texto = serde_json::to_string(&a).unwrap();
        assert_eq!(Arranjo::do_texto(&texto), Some(a));
    }
}
