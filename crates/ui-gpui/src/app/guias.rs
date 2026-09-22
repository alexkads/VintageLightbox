//! As guias de sessão: cada guia é uma sessão fotográfica aberta, como uma aba
//! do navegador com a rota `[id]` do site.
//!
//! 🔑 **Por que guias, e não uma cópia da tela por sessão.** Na web o operador
//! já trabalha assim — uma aba por cliente —, e o navegador paga uma página
//! inteira por aba. Aqui a galeria, a grade e a Revelação são **uma só**, e a
//! guia guarda só o que precisa sobreviver à troca: qual sessão é, o nome dela
//! e a fila do "Salvar na galeria". Trocar de guia é sair de uma sessão e
//! entrar na outra — o mesmo caminho testado de sempre —, e a memória de vídeo
//! não cresce com o número de clientes (a Haswell já caiu por falta dela).
//!
//! 🚨 **A fila do "Salvar" fica estacionada na guia, e não jogada fora.** Sair
//! da sessão a zerava para não mandar ao site foto de outro cliente; com guias,
//! jogá-la fora na troca apagaria o aviso de que o cliente ainda vê o JPEG
//! antigo. Estacionada, ela volta quando a guia volta, e continua sem vazar
//! para a sessão ao lado.
//!
//! As guias ficam **lembradas entre aberturas** (`guias-das-sessoes.json`, ao
//! lado do catálogo), presas à conta que as abriu: outra conta nesta máquina
//! não herda as sessões de quem saiu.

use std::collections::HashMap;
use std::path::PathBuf;

use domain::value_objects::CropSettings;
use gpui::{div, prelude::*, px, Context, FontWeight, MouseButton, SharedString};
use gpui_component::tooltip::Tooltip;
use gpui_component::{h_flex, ActiveTheme, Icon};
use serde::{Deserialize, Serialize};

use super::{Aplicativo, Tela};
use crate::recursos::Icone;
use crate::revelacao::processador::Ajustes;

/// Altura da faixa — a das abas do Chrome, menos o arredondado.
const ALTURA_DA_FAIXA: f32 = 36.;
/// Largura máxima de uma guia; com muitas abertas elas encolhem até o mínimo.
const LARGURA_MAXIMA: f32 = 220.;
const LARGURA_MINIMA: f32 = 96.;

/// Uma revelação à espera do "Salvar na galeria": `(id no site, receita, corte)`.
pub(super) type Pendente = (String, Ajustes, CropSettings);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guia {
    /// O id da galeria no site.
    pub id: String,
    /// O nome da sessão, quando já se sabe. `None` enquanto a galeria não
    /// respondeu — a guia diz "Carregando…".
    #[serde(default)]
    pub titulo: Option<String>,
}

/// A lista de guias, sem tela — é aqui que moram as regras de abrir, fechar e
/// andar, e é isto que os testes conferem.
#[derive(Debug, Default)]
pub struct Guias {
    lista: Vec<Guia>,
    /// As filas do "Salvar" das guias que não estão à frente, por galeria.
    estacionadas: HashMap<String, Vec<Pendente>>,
}

impl Guias {
    pub fn lista(&self) -> &[Guia] {
        &self.lista
    }

    pub fn vazia(&self) -> bool {
        self.lista.is_empty()
    }

    /// Abre a guia da sessão, ou só a encontra se ela já estiver aberta.
    ///
    /// 🔑 **Uma sessão, uma guia.** Abrir de novo a que já está aberta leva a
    /// ela, como o "trocar para esta aba" do navegador — duas guias da mesma
    /// sessão seriam duas filas do "Salvar" para as mesmas fotos.
    ///
    /// A nova entra **logo depois da ativa**, e não no fim: é o que o navegador
    /// faz com a aba aberta a partir de outra, e mantém juntas as sessões que
    /// o operador abriu uma a partir da outra.
    pub fn abrir(&mut self, id: &str, depois_de: Option<&str>) -> bool {
        if self.posicao(id).is_some() {
            return false;
        }
        let guia = Guia {
            id: id.to_string(),
            titulo: None,
        };
        match depois_de.and_then(|ativa| self.posicao(ativa)) {
            Some(i) => self.lista.insert(i + 1, guia),
            None => self.lista.push(guia),
        }
        true
    }

    /// Fecha a guia e diz qual assume o lugar dela: a da direita, ou a da
    /// esquerda quando a fechada era a última — a regra de todo navegador.
    pub fn fechar(&mut self, id: &str) -> Option<String> {
        let i = self.posicao(id)?;
        self.lista.remove(i);
        self.estacionadas.remove(id);
        self.lista
            .get(i)
            .or_else(|| i.checked_sub(1).and_then(|j| self.lista.get(j)))
            .map(|g| g.id.clone())
    }

    /// A guia `passo` casas adiante (ou atrás), dando a volta nas pontas, como
    /// `Ctrl+Tab`. Sem guia ativa, anda a partir da primeira.
    pub fn vizinha(&self, ativa: Option<&str>, passo: isize) -> Option<String> {
        let n = self.lista.len() as isize;
        if n == 0 {
            return None;
        }
        let i = match ativa.and_then(|a| self.posicao(a)) {
            Some(i) => (i as isize + passo).rem_euclid(n),
            None => 0,
        };
        Some(self.lista[i as usize].id.clone())
    }

    /// A guia na posição `n` (0 = a primeira), como `Cmd+1`…`Cmd+8`; `Cmd+9`
    /// é sempre a última, no navegador também.
    pub fn na_posicao(&self, n: usize) -> Option<String> {
        let guia = if n >= 8 {
            self.lista.last()
        } else {
            self.lista.get(n)
        };
        guia.map(|g| g.id.clone())
    }

    pub fn dar_nome(&mut self, id: &str, titulo: &str) -> bool {
        let titulo = titulo.trim();
        if titulo.is_empty() {
            return false;
        }
        match self.lista.iter_mut().find(|g| g.id == id) {
            Some(guia) if guia.titulo.as_deref() != Some(titulo) => {
                guia.titulo = Some(titulo.to_string());
                true
            }
            _ => false,
        }
    }

    /// Guarda a fila do "Salvar" da guia que sai da frente.
    pub fn estacionar(&mut self, id: &str, fila: Vec<Pendente>) {
        if fila.is_empty() || self.posicao(id).is_none() {
            self.estacionadas.remove(id);
        } else {
            self.estacionadas.insert(id.to_string(), fila);
        }
    }

    /// Devolve a fila que a guia deixou ao sair da frente.
    pub fn retomar(&mut self, id: &str) -> Vec<Pendente> {
        self.estacionadas.remove(id).unwrap_or_default()
    }

    /// Quantas revelações a guia (fora da frente) ainda tem por salvar.
    pub fn por_salvar(&self, id: &str) -> usize {
        self.estacionadas.get(id).map_or(0, Vec::len)
    }

    /// A foto subiu: sai de toda fila estacionada. A resposta do site pode
    /// chegar depois de o operador trocar de guia, e a guia de antes não pode
    /// continuar dizendo que a foto não foi salva.
    pub fn deu_baixa(&mut self, no_site: &str) {
        for fila in self.estacionadas.values_mut() {
            fila.retain(|(id, _, _)| id != no_site);
        }
        self.estacionadas.retain(|_, fila| !fila.is_empty());
    }

    /// Esquece tudo — é o "Sair" da conta.
    pub fn esvaziar(&mut self) {
        self.lista.clear();
        self.estacionadas.clear();
    }

    fn posicao(&self, id: &str) -> Option<usize> {
        self.lista.iter().position(|g| g.id == id)
    }

    // ── O arquivo ────────────────────────────────────────────────────────

    /// O que vai para o disco: a conta e as guias, sem as filas — a receita
    /// não salva já mora no depósito do app, e é de lá que ela volta.
    pub fn em_json(&self, conta: Option<&str>) -> String {
        serde_json::to_string_pretty(&Arquivo {
            conta: conta.map(str::to_string),
            guias: self.lista.clone(),
        })
        .unwrap_or_default()
    }

    /// Reabre as guias de uma abertura anterior, **só se forem desta conta**.
    pub fn de_json(json: &str, conta: &str) -> Vec<Guia> {
        match serde_json::from_str::<Arquivo>(json) {
            Ok(arquivo) if arquivo.conta.as_deref() == Some(conta) => arquivo.guias,
            _ => Vec::new(),
        }
    }

    pub fn repor(&mut self, guias: Vec<Guia>) {
        for guia in guias {
            if self.posicao(&guia.id).is_none() {
                self.lista.push(guia);
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Arquivo {
    conta: Option<String>,
    guias: Vec<Guia>,
}

/// Onde as guias ficam entre aberturas. Os testes nunca tocam o arquivo de
/// quem trabalha.
pub(super) fn arquivo() -> Option<PathBuf> {
    (!cfg!(test))
        .then(|| infrastructure::paths::AppPaths::catalog_root().join("guias-das-sessoes.json"))
}

impl Aplicativo {
    /// Grava as guias ao lado do catálogo.
    pub(super) fn guardar_guias(&self) {
        // 🚨 **Sem conta, não grava**: antes de o `/auth/me` responder, gravar
        // apagaria as guias da última abertura, que ainda nem foram repostas.
        let (Some(arquivo), Some(conta)) = (&self.arquivo_das_guias, &self.conta) else {
            return;
        };
        let _ = std::fs::write(arquivo, self.guias.em_json(Some(&conta.email)));
    }

    /// A conta chegou: as guias da última abertura voltam, se forem dela.
    pub(super) fn repor_guias(&mut self) {
        let (Some(arquivo), Some(conta)) = (&self.arquivo_das_guias, &self.conta) else {
            return;
        };
        let Ok(json) = std::fs::read_to_string(arquivo) else {
            return;
        };
        self.guias.repor(Guias::de_json(&json, &conta.email));
    }

    /// Clique numa guia, `Ctrl+Tab` ou `Cmd+1`…`9`.
    pub fn ir_para_a_guia(
        &mut self,
        id: String,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.sessao_aberta.as_deref() == Some(id.as_str()) && self.tela == Tela::Sessao {
            return;
        }
        self.entrar_na_sessao(id, cx);
        window.focus(&self.foco);
    }

    /// O `×` da guia (ou `Cmd+W`).
    ///
    /// 🔑 **Fechar a guia da frente leva à vizinha**; fechar a última leva à
    /// lista de sessões, que é a "página nova" deste app.
    pub fn fechar_guia(&mut self, id: String, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let da_frente = self.sessao_aberta.as_deref() == Some(id.as_str());
        if da_frente {
            // A fila sai da frente antes de a guia sumir: senão `sair_da_sessao`
            // a estacionaria numa guia que já não existe.
            self.a_subir.clear();
        }
        let vizinha = self.guias.fechar(&id);
        if da_frente {
            match vizinha {
                Some(proxima) => self.ir_para_a_guia(proxima, window, cx),
                None => self.ir_para(Tela::Sessoes, window, cx),
            }
        }
        self.guardar_guias();
        cx.notify();
    }

    /// Anda `passo` guias a partir da da frente.
    pub fn andar_nas_guias(
        &mut self,
        passo: isize,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.guias.vizinha(self.sessao_aberta.as_deref(), passo) {
            self.ir_para_a_guia(id, window, cx);
        }
    }

    pub fn guia_na_posicao(&mut self, n: usize, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if let Some(id) = self.guias.na_posicao(n) {
            self.ir_para_a_guia(id, window, cx);
        }
    }

    /// O nome da sessão da frente, assim que a galeria responde.
    pub(super) fn nomear_a_guia_da_frente(&mut self, cx: &Context<Self>) {
        if let Some(id) = self.sessao_aberta.clone() {
            self.nomear_a_guia(&id, cx);
        }
    }

    /// O nome vem da galeria que a tela da sessão tem aberta — e só se for a
    /// desta guia: na troca, a tela ainda pode estar com a de antes.
    pub(super) fn nomear_a_guia(&mut self, id: &str, cx: &Context<Self>) {
        let titulo = self
            .detalhe
            .read(cx)
            .aberta()
            .filter(|a| a.galeria.id == id)
            .map(|a| a.galeria.titulo.clone());
        if let Some(titulo) = titulo {
            if self.guias.dar_nome(id, &titulo) {
                self.guardar_guias();
            }
        }
    }

    /// As teclas do navegador. 🚨 **Só na galeria e na lista**: na Revelação
    /// `Cmd+W` fecharia a sessão com a foto aberta no meio de um ajuste.
    pub(super) fn ouvir_atalhos_das_guias(
        &self,
        raiz: gpui::Div,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        if !matches!(self.tela, Tela::Sessao | Tela::Sessoes) {
            return raiz;
        }
        raiz.on_action(cx.listener(|raiz, _: &super::GuiaNova, window, cx| {
            raiz.ir_para(Tela::Sessoes, window, cx);
        }))
        .on_action(cx.listener(|raiz, _: &super::FecharGuia, window, cx| {
            if raiz.tela == Tela::Sessao {
                if let Some(id) = raiz.sessao_aberta.clone() {
                    raiz.fechar_guia(id, window, cx);
                }
            }
        }))
        .on_action(cx.listener(|raiz, _: &super::ProximaGuia, window, cx| {
            raiz.andar_nas_guias(1, window, cx);
        }))
        .on_action(cx.listener(|raiz, _: &super::GuiaAnterior, window, cx| {
            raiz.andar_nas_guias(-1, window, cx);
        }))
        .on_action(cx.listener(|raiz, _: &super::Guia1, w, cx| raiz.guia_na_posicao(0, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia2, w, cx| raiz.guia_na_posicao(1, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia3, w, cx| raiz.guia_na_posicao(2, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia4, w, cx| raiz.guia_na_posicao(3, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia5, w, cx| raiz.guia_na_posicao(4, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia6, w, cx| raiz.guia_na_posicao(5, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia7, w, cx| raiz.guia_na_posicao(6, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::Guia8, w, cx| raiz.guia_na_posicao(7, w, cx)))
        .on_action(cx.listener(|raiz, _: &super::UltimaGuia, w, cx| raiz.guia_na_posicao(8, w, cx)))
    }

    pub fn titulos_das_guias_para_teste(&self) -> Vec<Option<String>> {
        self.guias
            .lista()
            .iter()
            .map(|g| g.titulo.clone())
            .collect()
    }

    pub fn guias_para_teste(&self) -> Vec<String> {
        self.guias.lista().iter().map(|g| g.id.clone()).collect()
    }

    // ── O desenho ────────────────────────────────────────────────────────

    /// A faixa das guias, sobre a galeria e sobre a lista de sessões.
    ///
    /// 🔑 **Só aparece com guia aberta**: quem trabalha com uma sessão por vez
    /// não ganha uma faixa a mais na tela.
    pub(super) fn faixa_das_guias(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.guias.vazia() || !matches!(self.tela, Tela::Sessao | Tela::Sessoes) {
            return None;
        }
        let tema = cx.theme();
        let (borda, fundo_da_faixa, fundo_ativo) = (tema.border, tema.muted, tema.background);
        let (frente, apagado, destaque) = (tema.foreground, tema.muted_foreground, tema.primary);
        let da_frente = (self.tela == Tela::Sessao)
            .then(|| self.sessao_aberta.clone())
            .flatten();

        let guias = self.guias.lista().iter().enumerate().map(|(i, guia)| {
            let ativa = da_frente.as_deref() == Some(guia.id.as_str());
            let por_salvar = if ativa {
                self.a_subir.len()
            } else {
                self.guias.por_salvar(&guia.id)
            };
            let titulo: SharedString = guia
                .titulo
                .clone()
                .unwrap_or_else(|| "Carregando…".into())
                .into();
            let dica = match (i, por_salvar) {
                (0..=7, 0) => format!("{titulo}  ·  ⌘{}", i + 1),
                (_, 0) => titulo.to_string(),
                (_, n) => format!("{titulo}  ·  {n} revelação(ões) por salvar"),
            };
            let id_clique = guia.id.clone();
            let id_meio = guia.id.clone();
            let id_fechar = guia.id.clone();
            h_flex()
                .id(SharedString::from(format!("guia-{}", guia.id)))
                .flex_1()
                .min_w(px(LARGURA_MINIMA))
                .max_w(px(LARGURA_MAXIMA))
                .h_full()
                .pl(px(12.))
                .pr(px(4.))
                .gap(px(6.))
                .border_r_1()
                .border_color(borda)
                .cursor_pointer()
                .text_sm()
                .when(ativa, |g| {
                    g.bg(fundo_ativo)
                        .text_color(frente)
                        .font_weight(FontWeight::MEDIUM)
                        .border_t_2()
                        .border_color(destaque)
                })
                .when(!ativa, |g| {
                    g.text_color(apagado)
                        .hover(move |g| g.bg(fundo_ativo.opacity(0.5)).text_color(frente))
                })
                .tooltip(move |window, cx| Tooltip::new(dica.clone()).build(window, cx))
                .on_click(cx.listener(move |raiz, _, window, cx| {
                    raiz.ir_para_a_guia(id_clique.clone(), window, cx);
                }))
                // O botão do meio fecha, como em todo navegador.
                .on_mouse_down(
                    MouseButton::Middle,
                    cx.listener(move |raiz, _, window, cx| {
                        raiz.fechar_guia(id_meio.clone(), window, cx);
                    }),
                )
                .child(Icon::new(Icone::Camera).size(px(14.)).flex_none())
                .child(div().flex_1().min_w(px(0.)).truncate().child(titulo))
                // 🔑 O ponto diz "tem revelação por salvar", como o ponto oco da
                // tira — trocar de guia não pode esconder isso.
                .when(por_salvar > 0, |g| {
                    g.child(div().flex_none().size(px(6.)).rounded_full().bg(destaque))
                })
                .child(
                    div()
                        .id(SharedString::from(format!("fechar-guia-{}", guia.id)))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(20.))
                        .rounded(px(4.))
                        .hover(move |b| b.bg(borda))
                        .child(Icon::new(Icone::X).size(px(12.)))
                        .on_click(cx.listener(move |raiz, _, window, cx| {
                            cx.stop_propagation();
                            raiz.fechar_guia(id_fechar.clone(), window, cx);
                        })),
                )
        });

        Some(
            h_flex()
                .id("faixa-das-guias")
                .flex_none()
                .w_full()
                .h(px(ALTURA_DA_FAIXA))
                .bg(fundo_da_faixa)
                .border_b_1()
                .border_color(borda)
                .overflow_x_scroll()
                .children(guias)
                .child(
                    div()
                        .id("guia-nova")
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(ALTURA_DA_FAIXA))
                        .text_color(apagado)
                        .cursor_pointer()
                        .hover(move |b| b.text_color(frente).bg(fundo_ativo.opacity(0.5)))
                        .tooltip(|window, cx| {
                            Tooltip::new("Abrir outra sessão numa guia").build(window, cx)
                        })
                        .child(Icon::new(Icone::Plus).size(px(16.)))
                        .on_click(cx.listener(|raiz, _, window, cx| {
                            raiz.ir_para(Tela::Sessoes, window, cx);
                        })),
                ),
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pendente(id: &str) -> Pendente {
        (id.into(), Ajustes::default(), CropSettings::default())
    }

    fn ids(guias: &Guias) -> Vec<&str> {
        guias.lista().iter().map(|g| g.id.as_str()).collect()
    }

    #[test]
    fn abrir_a_mesma_sessao_duas_vezes_nao_duplica_a_guia() {
        let mut guias = Guias::default();
        assert!(guias.abrir("g1", None));
        assert!(!guias.abrir("g1", Some("g1")));
        assert_eq!(ids(&guias), ["g1"]);
    }

    #[test]
    fn a_guia_nova_entra_logo_depois_da_ativa() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        guias.abrir("g2", Some("g1"));
        guias.abrir("g3", Some("g1"));
        assert_eq!(ids(&guias), ["g1", "g3", "g2"]);
        // Sem ativa (vindo da lista), vai para o fim.
        guias.abrir("g4", None);
        assert_eq!(ids(&guias), ["g1", "g3", "g2", "g4"]);
    }

    #[test]
    fn fechar_passa_para_a_direita_e_na_ultima_para_a_esquerda() {
        let mut guias = Guias::default();
        for id in ["g1", "g2", "g3"] {
            guias.abrir(id, None);
        }
        assert_eq!(guias.fechar("g2").as_deref(), Some("g3"));
        assert_eq!(guias.fechar("g3").as_deref(), Some("g1"));
        assert_eq!(guias.fechar("g1"), None);
        assert!(guias.vazia());
        assert_eq!(guias.fechar("nenhuma"), None);
    }

    #[test]
    fn ctrl_tab_da_a_volta_nas_pontas() {
        let mut guias = Guias::default();
        for id in ["g1", "g2", "g3"] {
            guias.abrir(id, None);
        }
        assert_eq!(guias.vizinha(Some("g3"), 1).as_deref(), Some("g1"));
        assert_eq!(guias.vizinha(Some("g1"), -1).as_deref(), Some("g3"));
        assert_eq!(guias.vizinha(None, 1).as_deref(), Some("g1"));
        assert_eq!(Guias::default().vizinha(None, 1), None);
    }

    #[test]
    fn cmd_9_e_sempre_a_ultima() {
        let mut guias = Guias::default();
        for id in ["g1", "g2", "g3"] {
            guias.abrir(id, None);
        }
        assert_eq!(guias.na_posicao(0).as_deref(), Some("g1"));
        assert_eq!(guias.na_posicao(8).as_deref(), Some("g3"));
        assert_eq!(guias.na_posicao(5), None);
    }

    #[test]
    fn a_fila_do_salvar_volta_com_a_guia_e_nao_vaza_para_a_vizinha() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        guias.abrir("g2", None);
        guias.estacionar("g1", vec![pendente("f1"), pendente("f2")]);
        assert_eq!(guias.por_salvar("g1"), 2);
        assert!(guias.retomar("g2").is_empty());
        let fila = guias.retomar("g1");
        assert_eq!(fila.len(), 2);
        // Retomada, sai do estacionamento: não volta duas vezes.
        assert_eq!(guias.por_salvar("g1"), 0);
    }

    #[test]
    fn a_foto_que_subiu_sai_da_fila_estacionada() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        guias.estacionar("g1", vec![pendente("f1"), pendente("f2")]);
        guias.deu_baixa("f1");
        assert_eq!(guias.por_salvar("g1"), 1);
        guias.deu_baixa("f2");
        assert_eq!(guias.por_salvar("g1"), 0);
    }

    #[test]
    fn fechar_a_guia_esquece_a_fila_dela() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        guias.estacionar("g1", vec![pendente("f1")]);
        guias.fechar("g1");
        guias.abrir("g1", None);
        assert_eq!(guias.por_salvar("g1"), 0);
    }

    #[test]
    fn as_guias_so_voltam_para_a_mesma_conta() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        guias.dar_nome("g1", "Casamento Ana");
        let json = guias.em_json(Some("a@x.com"));
        let de_volta = Guias::de_json(&json, "a@x.com");
        assert_eq!(de_volta.len(), 1);
        assert_eq!(de_volta[0].titulo.as_deref(), Some("Casamento Ana"));
        assert!(Guias::de_json(&json, "outra@x.com").is_empty());
        assert!(Guias::de_json("lixo", "a@x.com").is_empty());
    }

    #[test]
    fn nome_vazio_nao_apaga_o_nome_da_guia() {
        let mut guias = Guias::default();
        guias.abrir("g1", None);
        assert!(guias.dar_nome("g1", "Ensaio"));
        assert!(!guias.dar_nome("g1", "   "));
        assert!(!guias.dar_nome("g1", "Ensaio"));
        assert_eq!(guias.lista()[0].titulo.as_deref(), Some("Ensaio"));
    }
}
