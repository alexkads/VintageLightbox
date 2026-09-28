//! As colunas das telas grandes: puxadas pela borda e recolhidas pelas
//! setinhas, como no Lightroom.
//!
//! 🔑 **É o `DockArea` do gpui-kit** (dono, 27/set/2026: *"utilize o
//! componente dock para eu conseguir mudar o tamanho dos painéis ou esconder as
//! barras"*), mas **sem aba e sem título**. A Revelação já foi um dock de cinco
//! painéis, e o dono pediu duas vezes que saísse: cada painel ganhava uma aba
//! com nome ("Foto", "Ajustes", "Presets"), e a tela deixava de ser a do site.
//! O que ele pediu agora é o que o dock tem de bom — a borda que se arrasta e a
//! coluna que some — e não a aparência dele. O gpui-kit separa as duas coisas
//! (`DockAreaRenderer`/`TabGroupRenderer`), e esta pele fica com o comportamento
//! e o puxador de borda do `DockSkin`, e desenha o grupo sem barra de abas.
//!
//! O centro fica fixo (é a foto, ou a grade); as colunas são docas laterais.
//! Nada se arrasta de um lugar para outro — revelar é sempre o mesmo gesto, e
//! o arranjo que o dono vetou era exatamente o que mudava de lugar.
//!
//! As **setas das bordas** são faixas finas nas beiradas da área, sempre à
//! vista, como as do Lightroom: com a coluna aberta apontam para fora
//! (esconder); fechada, para dentro (mostrar). A tira de baixo não é doca —
//! ela corre de ponta a ponta sob as colunas, como no Lightroom, e já tem a
//! alça de altura —, mas tem a sua seta e a sua lembrança aqui.
//!
//! ⚠️ A arrumação é **desta máquina**, num JSON ao lado do catálogo, como o
//! resto da arrumação das telas: dois balcões arrumam de jeitos diferentes.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::component::dock::{
    BasePanel, DockArea, DockAreaRenderer, DockContext, DockEvent, DockLayout, DockPlacement,
    DockSkin, PanelEvent, TabGroupContext, TabGroupRenderer,
};
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, ClickEvent, Context, Entity, EventEmitter, FocusHandle,
    Focusable, Render, SharedString, Subscription, WeakEntity, Window,
};
use serde::{Deserialize, Serialize};

use gpui_kit::component::ActiveTheme as _;

/// A largura da faixa da seta, em pontos.
pub const LARGURA_DA_SETA: f32 = 12.0;
/// Quanto se espera, parado, antes de gravar a arrumação.
const ESPERA_DA_GRAVACAO_MS: u64 = 400;

/// De que lado fica uma coluna, ou a tira.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    Esquerda,
    Direita,
    Baixo,
}

impl Lado {
    fn doca(self) -> DockPlacement {
        match self {
            Lado::Esquerda => DockPlacement::Left,
            Lado::Direita => DockPlacement::Right,
            Lado::Baixo => DockPlacement::Bottom,
        }
    }

    /// O triângulo da seta: aberta aponta para fora (esconde), fechada para
    /// dentro (mostra) — o do Lightroom.
    pub fn triangulo(self, aberta: bool) -> &'static str {
        match (self, aberta) {
            (Lado::Esquerda, true) | (Lado::Direita, false) => "◂",
            (Lado::Esquerda, false) | (Lado::Direita, true) => "▸",
            (Lado::Baixo, true) => "▾",
            (Lado::Baixo, false) => "▴",
        }
    }
}

/// Quanto uma coluna pode ter, em pontos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limites {
    pub minimo: f32,
    pub maximo: f32,
    pub padrao: f32,
}

impl Limites {
    pub fn limitar(&self, largura: f32) -> f32 {
        if largura.is_finite() {
            largura.clamp(self.minimo, self.maximo)
        } else {
            self.padrao
        }
    }
}

/// Uma coluna como ficou: aberta ou não, e com que largura.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Coluna {
    pub aberta: bool,
    pub largura: f32,
}

/// A arrumação de uma tela, como vai para o disco.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Arrumacao {
    #[serde(default)]
    pub esquerda: Option<Coluna>,
    #[serde(default)]
    pub direita: Option<Coluna>,
    #[serde(default)]
    pub tira_aberta: Option<bool>,
}

impl Arrumacao {
    fn coluna(&self, lado: Lado) -> Option<Coluna> {
        match lado {
            Lado::Esquerda => self.esquerda,
            Lado::Direita => self.direita,
            Lado::Baixo => None,
        }
    }

    fn coluna_mut(&mut self, lado: Lado) -> Option<&mut Option<Coluna>> {
        match lado {
            Lado::Esquerda => Some(&mut self.esquerda),
            Lado::Direita => Some(&mut self.direita),
            Lado::Baixo => None,
        }
    }
}

/// Onde a arrumação de uma tela mora. Nos testes, em lugar nenhum: a suíte não
/// pode mexer na tela de quem roda `cargo test`.
#[cfg(not(test))]
fn arquivo(tela: &str) -> Option<PathBuf> {
    Some(infrastructure::paths::AppPaths::catalog_root().join(format!("docas-{tela}.json")))
}

#[cfg(test)]
fn arquivo(_tela: &str) -> Option<PathBuf> {
    None
}

/// Lê a arrumação. Qualquer problema é "a padrão": não poder lembrar não pode
/// impedir de abrir a tela.
pub fn ler(tela: &str) -> Arrumacao {
    arquivo(tela)
        .and_then(|caminho| std::fs::read_to_string(caminho).ok())
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default()
}

pub fn gravar(tela: &str, arrumacao: &Arrumacao) {
    let Some(caminho) = arquivo(tela) else {
        return;
    };
    let Ok(texto) = serde_json::to_string_pretty(arrumacao) else {
        return;
    };
    if let Some(pasta) = caminho.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    if let Err(erro) = std::fs::write(&caminho, texto) {
        crate::telemetria::avisar!("⚠️ a arrumação das colunas ({tela}) não foi gravada: {erro}");
    }
}

// ------------------------------------------------------------------ a pele

/// O `DockSkin` do gpui-kit para o puxador de borda, e nada de abas.
struct PeleSemAba {
    pele: Rc<DockSkin>,
}

impl DockAreaRenderer for PeleSemAba {
    fn render_dock(
        &self,
        doca: &DockContext,
        conteudo: AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        self.pele.render_dock(doca, conteudo, window, cx)
    }

    fn tab_group_renderer(&self) -> Rc<dyn TabGroupRenderer> {
        Rc::new(GrupoSemAba)
    }
}

/// O grupo de um painel só, sem a barra de abas — é ela que o dono vetou.
struct GrupoSemAba;

impl TabGroupRenderer for GrupoSemAba {
    fn render_tab_bar(&self, _: &TabGroupContext, _: &mut Window, _: &mut App) -> AnyElement {
        div().into_any_element()
    }
}

// --------------------------------------------------------------- os painéis

/// Como uma coluna se desenha: um método da tela dona do estado.
pub type Desenho<T> = Rc<dyn Fn(&mut T, &mut Window, &mut Context<T>) -> AnyElement>;

/// Um painel fino por cima da tela — o mesmo arranjo da Biblioteca
/// (`biblioteca::paineis`): o desenho continua sendo método da tela, com os
/// `cx.listener` dela, e o painel só o chama. A volta é fraca, para o ciclo
/// tela → dock → painel → tela não prender memória.
pub struct PainelDaTela<T: 'static> {
    nome: &'static str,
    tela: WeakEntity<T>,
    desenho: Desenho<T>,
    foco: FocusHandle,
}

impl<T: 'static> Render for PainelDaTela<T> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(tela) = self.tela.upgrade() else {
            return div().into_any_element();
        };
        let desenho = self.desenho.clone();
        tela.update(cx, |tela, cx| desenho(tela, window, cx))
    }
}

impl<T: 'static> Focusable for PainelDaTela<T> {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.foco.clone()
    }
}

impl<T: 'static> EventEmitter<PanelEvent> for PainelDaTela<T> {}

impl<T: 'static> BasePanel for PainelDaTela<T> {
    fn panel_name(&self) -> &'static str {
        self.nome
    }

    /// Coluna não fecha: some pela seta e volta pela seta.
    fn closable(&self, _cx: &App) -> bool {
        false
    }

    fn zoomable(&self, _cx: &App) -> bool {
        false
    }
}

/// Uma coluna a montar: o nome (fixo — é o que o dock guarda), o desenho e o
/// quanto ela pode ter.
pub struct Lateral<T: 'static> {
    pub nome: &'static str,
    pub desenho: Desenho<T>,
    pub limites: Limites,
}

// ---------------------------------------------------------------- a bancada

/// O dock de uma tela: o centro fixo, as colunas dos lados, e a tira.
pub struct Docas {
    pub area: Entity<DockArea>,
    tela: &'static str,
    arrumacao: Rc<RefCell<Arrumacao>>,
    geracao: Rc<Cell<u64>>,
    _assinatura: Subscription,
}

impl Docas {
    /// Monta o dock com a arrumação lembrada.
    ///
    /// 🚨 Só depois de a tela existir: os painéis guardam uma referência fraca
    /// a ela. Quem monta é a própria tela, no primeiro quadro.
    pub fn montar<T: 'static>(
        tela: &'static str,
        eu: &Entity<T>,
        centro: (&'static str, Desenho<T>),
        laterais: Vec<(Lado, Lateral<T>)>,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Self {
        let arrumacao = ler(tela);
        let area = cx.new(|cx| {
            let pele = DockSkin::new(cx);
            DockArea::new(tela, None, window, cx).with_renderer(Rc::new(PeleSemAba { pele }))
        });
        let painel = |nome: &'static str, desenho: Desenho<T>, cx: &mut Context<T>| {
            let tela = eu.downgrade();
            cx.new(|cx| PainelDaTela {
                nome,
                tela,
                desenho,
                foco: cx.focus_handle(),
            })
        };
        let meio = painel(centro.0, centro.1, cx);
        let mut limites: Vec<(Lado, Limites)> = Vec::new();
        let docas: Vec<_> = laterais
            .into_iter()
            .map(|(lado, lateral)| {
                limites.push((lado, lateral.limites));
                (
                    lado,
                    painel(lateral.nome, lateral.desenho, cx),
                    lateral.limites,
                )
            })
            .collect();

        area.update(cx, |area, cx| {
            area.set_center(DockLayout::tabs().panel(meio), window, cx);
            for (lado, painel, limites) in docas {
                area.set_dock(lado.doca(), DockLayout::tabs().panel(painel), window, cx);
                area.set_dock_collapsible(lado.doca(), true, window, cx);
                let lembrada = arrumacao.coluna(lado);
                let largura = limites.limitar(lembrada.map_or(limites.padrao, |c| c.largura));
                area.set_dock_size(lado.doca(), px(largura), window, cx);
                let aberta = lembrada.is_none_or(|c| c.aberta);
                if area.is_dock_open(lado.doca()) != aberta {
                    area.toggle_dock(lado.doca(), window, cx);
                }
            }
        });

        let arrumacao = Rc::new(RefCell::new(arrumacao));
        let geracao = Rc::new(Cell::new(0u64));
        // Toda mudança do dock: a largura volta aos limites da coluna (o dock
        // só conhece o mínimo dele, de 100 px), e a arrumação é gravada depois
        // de parar — um arrasto de borda emite dezenas de mudanças.
        let _assinatura = {
            let (arrumacao, geracao) = (arrumacao.clone(), geracao.clone());
            cx.subscribe_in(
                &area,
                window,
                move |_tela: &mut T, area, evento: &DockEvent, window, cx| {
                    if !matches!(evento, DockEvent::LayoutChanged) {
                        return;
                    }
                    for (lado, limites) in &limites {
                        let doca = lado.doca();
                        let Some(tamanho) = area.read(cx).dock_size(doca) else {
                            continue;
                        };
                        let largura = f32::from(tamanho);
                        let certa = limites.limitar(largura);
                        if (certa - largura).abs() > 0.5 {
                            area.update(cx, |area, cx| {
                                area.set_dock_size(doca, px(certa), window, cx)
                            });
                        }
                        let aberta = area.read(cx).is_dock_open(doca);
                        if let Some(coluna) = arrumacao.borrow_mut().coluna_mut(*lado) {
                            *coluna = Some(Coluna {
                                aberta,
                                largura: certa,
                            });
                        }
                    }
                    agendar_gravacao(tela, &arrumacao, &geracao, cx);
                },
            )
        };

        Self {
            area,
            tela,
            arrumacao,
            geracao,
            _assinatura,
        }
    }

    /// A coluna deste lado está à vista?
    pub fn aberta(&self, lado: Lado, cx: &App) -> bool {
        match lado {
            Lado::Baixo => self.tira_aberta(),
            _ => self.area.read(cx).is_dock_open(lado.doca()),
        }
    }

    /// A largura que a coluna ocupa agora — zero se escondida.
    pub fn largura(&self, lado: Lado, cx: &App) -> f32 {
        let area = self.area.read(cx);
        if !area.is_dock_open(lado.doca()) {
            return 0.0;
        }
        area.dock_size(lado.doca()).map_or(0.0, f32::from)
    }

    pub fn tira_aberta(&self) -> bool {
        self.arrumacao.borrow().tira_aberta.unwrap_or(true)
    }

    /// Mostra ou esconde a coluna — ou a tira.
    pub fn alternar(&self, lado: Lado, window: &mut Window, cx: &mut App) {
        if lado == Lado::Baixo {
            let aberta = !self.tira_aberta();
            self.arrumacao.borrow_mut().tira_aberta = Some(aberta);
            agendar_gravacao(self.tela, &self.arrumacao, &self.geracao, cx);
            return;
        }
        self.area
            .update(cx, |area, cx| area.toggle_dock(lado.doca(), window, cx));
    }

    /// Deixa a coluna como pedido (sem nada a fazer se já está).
    pub fn definir(&self, lado: Lado, aberta: bool, window: &mut Window, cx: &mut App) {
        if self.aberta(lado, cx) != aberta {
            self.alternar(lado, window, cx);
        }
    }

    /// O `Tab` do Lightroom: as duas colunas somem juntas, ou voltam juntas.
    /// Com uma aberta e outra não, a aberta some — o próximo `Tab` traz as duas.
    pub fn alternar_as_colunas(&self, window: &mut Window, cx: &mut App) {
        let lados: Vec<Lado> = [Lado::Esquerda, Lado::Direita]
            .into_iter()
            .filter(|lado| self.area.read(cx).has_dock(lado.doca()))
            .collect();
        let alguma = lados.iter().any(|lado| self.aberta(*lado, cx));
        for lado in lados {
            self.definir(lado, !alguma, window, cx);
        }
    }

    /// O `⇧Tab` do Lightroom: colunas e tira, tudo junto.
    pub fn alternar_tudo(&self, window: &mut Window, cx: &mut App) {
        let lados: Vec<Lado> = [Lado::Esquerda, Lado::Direita]
            .into_iter()
            .filter(|lado| self.area.read(cx).has_dock(lado.doca()))
            .chain([Lado::Baixo])
            .collect();
        let alguma = lados.iter().any(|lado| self.aberta(*lado, cx));
        for lado in lados {
            self.definir(lado, !alguma, window, cx);
        }
    }
}

fn agendar_gravacao(
    tela: &'static str,
    arrumacao: &Rc<RefCell<Arrumacao>>,
    geracao: &Rc<Cell<u64>>,
    cx: &mut App,
) {
    let esta = geracao.get() + 1;
    geracao.set(esta);
    let (arrumacao, geracao) = (arrumacao.clone(), geracao.clone());
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(std::time::Duration::from_millis(ESPERA_DA_GRAVACAO_MS))
            .await;
        if geracao.get() == esta {
            gravar(tela, &arrumacao.borrow());
        }
    })
    .detach();
}

/// A faixa da seta, na beirada: fina, sempre à vista, e o clique inteiro dela
/// alterna a coluna.
pub fn seta(
    id: &'static str,
    lado: Lado,
    aberta: bool,
    dica: &'static str,
    cx: &App,
    ao_clicar: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let tema = cx.theme();
    let (apagada, acesa, fundo) = (tema.muted_foreground, tema.foreground, tema.muted);
    let faixa = div()
        .id(id)
        .debug_selector(move || id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(apagada)
        .hover(move |d| d.bg(fundo).text_color(acesa))
        .tooltip(move |window, cx| {
            gpui_kit::component::tooltip::Tooltip::new(dica).build(window, cx)
        })
        .on_click(ao_clicar)
        .child(
            div()
                .text_size(px(11.))
                .line_height(px(11.))
                .child(SharedString::new_static(lado.triangulo(aberta))),
        );
    match lado {
        Lado::Esquerda | Lado::Direita => faixa.w(px(LARGURA_DA_SETA)).h_full(),
        Lado::Baixo => faixa.h(px(LARGURA_DA_SETA)).w_full(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_seta_aponta_para_fora_aberta_e_para_dentro_fechada() {
        assert_eq!(Lado::Esquerda.triangulo(true), "◂");
        assert_eq!(Lado::Esquerda.triangulo(false), "▸");
        assert_eq!(Lado::Direita.triangulo(true), "▸");
        assert_eq!(Lado::Direita.triangulo(false), "◂");
        assert_eq!(Lado::Baixo.triangulo(true), "▾");
        assert_eq!(Lado::Baixo.triangulo(false), "▴");
    }

    #[test]
    fn a_largura_fica_nos_limites_da_coluna() {
        let l = Limites {
            minimo: 180.0,
            maximo: 420.0,
            padrao: 224.0,
        };
        assert_eq!(l.limitar(100.0), 180.0);
        assert_eq!(l.limitar(900.0), 420.0);
        assert_eq!(l.limitar(300.0), 300.0);
        assert_eq!(l.limitar(f32::NAN), 224.0);
    }

    /// Um arquivo de outra versão, ou sem um dos campos, não derruba nada.
    #[test]
    fn a_arrumacao_lida_tolera_campo_faltando() {
        let lida: Arrumacao =
            serde_json::from_str(r#"{"direita":{"aberta":false,"largura":350.0}}"#).unwrap();
        assert_eq!(lida.esquerda, None);
        assert_eq!(
            lida.direita,
            Some(Coluna {
                aberta: false,
                largura: 350.0
            })
        );
        assert_eq!(lida.tira_aberta, None);
    }
}
