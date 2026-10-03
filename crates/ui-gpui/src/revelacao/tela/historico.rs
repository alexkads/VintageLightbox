//! 📜 O painel **Histórico** do Lightroom, no pé da coluna das predefinições
//! (dono, 03/10/2026, com o print do painel do Lightroom Classic).
//!
//! | Gesto | Faz |
//! |---|---|
//! | passar o mouse numa linha | mostra aquele passo na foto, sem mudar nada |
//! | clicar numa linha | a foto volta àquele passo e grava — os de depois ficam, até o próximo gesto |
//! | ✕ | limpa: fica só a foto de agora |
//! | clicar no título | recolhe ou abre, e lembra neste computador |
//!
//! Cada linha é um gesto: nome, quanto andou e onde parou ("Contraste −1 55").
//! O nome sai de [`crate::revelacao::historico::rotular`]; a pilha é a mesma
//! do `⌘Z`.
//!
//! 🔑 **O histórico fica com a foto**: vai ao catálogo a cada gravação
//! ([`Revelacao::gravar_o_historico`]) e volta na abertura
//! ([`Revelacao::ler_o_historico_gravado`]).

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable, Icon};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, SharedString, Window};

use super::Revelacao;
use crate::recursos::Icone;
use crate::revelacao::historico::{Historico, Passo};

/// A altura de uma linha. Fixa: é ela que faz a lista ter o tamanho dos
/// passos, e não o da janela.
const ALTURA_DA_LINHA: f32 = 20.;
/// Quantas linhas a lista mostra antes de rolar, numa tela alta.
const LINHAS_A_VISTA: usize = 10;
/// A janela mais baixa que o app atende. Nela, com o Navegador aberto, não
/// sobra linha para o Histórico sem empurrar as Minhas para baixo da dobra
/// (dono, 29/09/2026): ele fica só no título até o Navegador recolher.
const JANELA_MAIS_BAIXA: f32 = 720.;
/// O que o Navegador recolhido devolve à coluna (a miniatura e os atalhos).
const ALTURA_DO_NAVEGADOR: f32 = 120.;

impl Revelacao {
    // ------------------------------------------------------------ gravação

    /// Pede ao disco o histórico gravado da foto que acabou de abrir.
    ///
    /// ⚠️ **No executor de fundo**: é uma ida ao SQLite, e a abertura da foto
    /// (setas na tira) não espera por ela. Enquanto não volta, nada se grava
    /// ([`Self::gravar_o_historico`]).
    pub(super) fn ler_o_historico_gravado(&mut self, cx: &mut Context<Self>) {
        self.geracao_do_historico += 1;
        let Some(id) = self.aberta.as_ref().map(|a| a.foto.id.clone()) else {
            self.historico_por_ler = None;
            self._leitura_do_historico = None;
            return;
        };
        let geracao = self.geracao_do_historico;
        self.historico_por_ler = Some(geracao);
        let gravador = self.gravador.clone();
        let leitura = cx
            .background_executor()
            .spawn(async move { gravador.ler_historico(&id) });
        self._leitura_do_historico = Some(cx.spawn(async move |esta, cx| {
            let json = leitura.await;
            let _ = esta.update(cx, |tela, cx| tela.retomar_o_historico(geracao, json, cx));
        }));
    }

    /// O gravado chegou: entra por baixo do que esta abertura já fez.
    fn retomar_o_historico(&mut self, geracao: u64, json: Option<String>, cx: &mut Context<Self>) {
        // Outra foto abriu nesse meio-tempo: a leitura é da que saiu.
        if self.historico_por_ler != Some(geracao) {
            return;
        }
        self.historico_por_ler = None;
        let Some(json) = json else {
            // Nunca revelada aqui: o histórico começa agora. O que esta
            // abertura já fez vai ao disco; só o "Início", não — andar pela
            // tira com as setas não escreve nada.
            if self.historico.passos().len() > 1 {
                self.gravar_o_historico();
            }
            return;
        };
        match Historico::retomar_sob(&json, &self.historico) {
            Ok(historico) => {
                self.historico = historico;
                self.previa_do_passo = None;
            }
            Err(erro) => {
                let nome = self.aberta.as_ref().map(|a| a.foto.name.clone());
                crate::telemetria::avisar!(
                    "⚠️ [Histórico] {}: {erro} — começa de novo",
                    nome.unwrap_or_default()
                );
            }
        }
        self.gravar_o_historico();
        cx.notify();
    }

    /// Leva o histórico da foto aberta ao catálogo.
    ///
    /// Nada vai enquanto o gravado não chegou, nem da foto comprada (que não se
    /// revela).
    pub(super) fn gravar_o_historico(&self) {
        if self.historico_por_ler.is_some() {
            return;
        }
        let Some(aberta) = self.aberta.as_ref() else {
            return;
        };
        if aberta.foto.revelacao_travada {
            return;
        }
        self.gravador
            .gravar_historico(aberta.foto.id.clone(), self.historico.em_json());
    }

    // ------------------------------------------------------------ gestos

    /// Os passos do histórico da foto aberta e o atual — para o painel e para
    /// os testes.
    pub fn passos_do_historico(&self) -> (&[Passo], usize) {
        (self.historico.passos(), self.historico.atual())
    }

    /// O passo sob o ponteiro, se há um.
    pub(super) fn passo_em_previa(&self) -> Option<&Passo> {
        self.previa_do_passo
            .and_then(|i| self.historico.passos().get(i))
    }

    /// Mostra (ou tira) um passo na foto, sem mudar nada.
    pub fn prever_passo(&mut self, indice: Option<usize>, cx: &mut Context<Self>) {
        // O passo atual é a foto que já está na tela.
        let indice = indice.filter(|i| *i != self.historico.atual());
        if self.previa_do_passo == indice {
            return;
        }
        self.previa_do_passo = indice;
        self.previa = None;
        self.pedir_revelacao(cx);
        cx.notify();
    }

    /// O clique numa linha: a foto volta àquele passo, e grava.
    ///
    /// 🔑 **O mesmo caminho do `⌘Z`** ([`Self::aplicar_do_historico`]): o
    /// gesto em curso fecha antes, a tela e o banco vão juntos, e nenhum passo
    /// novo é escrito — andar no histórico não é escrever nele.
    pub fn ir_para_no_historico(
        &mut self,
        indice: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.controles_ligados() {
            return;
        }
        self.previa_do_passo = None;
        self.gravar_o_que_estiver_pendente();
        if let Some(estado) = self.historico.ir_para(indice) {
            self.aplicar_do_historico(estado, window, cx);
        } else {
            // Já era o atual: só a prévia saiu.
            self.pedir_revelacao(cx);
            cx.notify();
        }
    }

    /// O ✕: fica só a foto de agora. Não muda a foto.
    pub fn limpar_o_historico(&mut self, cx: &mut Context<Self>) {
        self.gravar_o_que_estiver_pendente();
        self.previa_do_passo = None;
        self.historico.limpar();
        self.gravar_o_historico();
        self.pedir_revelacao(cx);
        cx.notify();
    }

    /// Recolhe ou abre o painel, e lembra neste computador.
    pub fn alternar_historico(&mut self, cx: &mut Context<Self>) {
        self.predefinicoes.ordem.alternar_historico();
        crate::revelacao::presets::ordem::gravar(&self.predefinicoes.ordem);
        if self.predefinicoes.ordem.historico_recolhido() {
            self.prever_passo(None, cx);
        }
        cx.notify();
    }

    // ------------------------------------------------------------ desenho

    /// O painel: o título que recolhe, o ✕ e a lista, mais novo no topo.
    pub(super) fn painel_do_historico(
        &self,
        altura_da_janela: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme();
        let (apagado, frente, borda) = (tema.muted_foreground, tema.foreground, tema.border);
        let recolhido = self.predefinicoes.ordem.historico_recolhido();
        let (passos, atual) = self.passos_do_historico();
        let desligado = !self.controles_ligados();

        let pasta = h_flex()
            .id("historico-pasta")
            .debug_selector(|| "historico-pasta".into())
            .flex_1()
            .min_w(px(0.))
            .items_center()
            .gap(px(4.))
            .rounded(crate::tema::canto(4.))
            .cursor_pointer()
            .hover(move |s| s.text_color(frente))
            .child(
                Icon::new(if recolhido {
                    Icone::ChevronRight
                } else {
                    Icone::ChevronDown
                })
                .size(px(12.)),
            )
            .child(div().flex_none().child("HISTÓRICO"))
            .child(
                div()
                    .flex_none()
                    .text_color(apagado.opacity(0.6))
                    .child(SharedString::from(passos.len().to_string())),
            )
            .tooltip(move |window, cx| {
                Tooltip::new(if recolhido {
                    "Abrir o Histórico"
                } else {
                    "Recolher o Histórico"
                })
                .build(window, cx)
            })
            .on_click(cx.listener(|tela, ev: &gpui_kit::ClickEvent, _, cx| {
                if ev.standard_click() {
                    tela.alternar_historico(cx);
                }
            }));

        let limpar = crate::estilo::botao_icone_pequeno("historico-limpar", Icone::X)
            .debug_selector(|| "historico-limpar".into())
            .tooltip("Limpar o histórico")
            .disabled(desligado || passos.len() < 2)
            .on_click(cx.listener(|tela, _, _, cx| tela.limpar_o_historico(cx)));

        let cabecalho = h_flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .text_size(crate::tema::letra::em(11.))
            .text_color(apagado)
            .child(pasta)
            .child(limpar);

        // Encolhe (com a lista rolando) quando a coluna é baixa; nunca cresce.
        v_flex()
            .id("historico")
            .debug_selector(|| "historico".into())
            .flex_none()
            .mt(px(4.))
            .pt(px(4.))
            .border_t_1()
            .border_color(borda)
            .child(cabecalho)
            .when(!recolhido, |painel| {
                let linhas = self.linhas_do_historico(altura_da_janela);
                painel.when(linhas > 0, |painel| {
                    painel.child(self.lista_do_historico(passos, atual, linhas, desligado, cx))
                })
            })
            .into_any_element()
    }

    /// Quantas linhas cabem sem tirar da lista das predefinições o piso dela.
    ///
    /// 🚨 **A conta é pela altura da janela, e não um teto fixo.** Com 240 px
    /// fixos, numa tela 1280×720 a lista das predefinições ficava com 111 px —
    /// e as predefinições encolhem por inteiro antes (base 0 no flex), então
    /// nenhum `min_h` nelas segura: quem tem de ceder é o Histórico.
    fn linhas_do_historico(&self, altura_da_janela: f32) -> usize {
        let mut sobra = altura_da_janela - JANELA_MAIS_BAIXA;
        if self.predefinicoes.ordem.navegador_recolhido() {
            sobra += ALTURA_DO_NAVEGADOR;
        }
        ((sobra.max(0.) / ALTURA_DA_LINHA) as usize).min(LINHAS_A_VISTA)
    }

    /// As linhas, do mais novo ao mais velho — como no Lightroom.
    fn lista_do_historico(
        &self,
        passos: &[Passo],
        atual: usize,
        linhas_a_vista: usize,
        desligado: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme();
        let (apagado, frente, realce) = (tema.muted_foreground, tema.foreground, tema.muted);

        let linhas = passos.iter().enumerate().rev().map(|(i, passo)| {
            let e_atual = i == atual;
            // Os desfeitos: ainda lá (o `⌘⇧Z` os alcança), mas esmaecidos —
            // o próximo gesto os apaga.
            let desfeito = i > atual;
            let numero = |texto: Option<&String>, largura: f32| {
                div()
                    .flex_none()
                    .w(px(largura))
                    .text_right()
                    .text_color(apagado)
                    .children(texto.cloned())
            };
            h_flex()
                .id(("historico-passo", i))
                .debug_selector(move || format!("historico-passo-{i}"))
                .flex_none()
                .h(px(ALTURA_DA_LINHA))
                .items_center()
                .gap(px(6.))
                .px(px(6.))
                .rounded(crate::tema::canto(4.))
                .text_size(crate::tema::letra::em(11.))
                .text_color(if desfeito {
                    apagado.opacity(0.6)
                } else {
                    frente
                })
                .when(e_atual, |l| l.bg(realce))
                .when(!desligado, |l| {
                    l.cursor_pointer()
                        .hover(move |s| s.bg(realce.opacity(0.6)))
                        .on_hover(cx.listener(move |tela, sobre: &bool, _, cx| {
                            if *sobre {
                                tela.prever_passo(Some(i), cx);
                            } else if tela.previa_do_passo == Some(i) {
                                tela.prever_passo(None, cx);
                            }
                        }))
                        .on_click(cx.listener(move |tela, _, window, cx| {
                            tela.ir_para_no_historico(i, window, cx);
                        }))
                })
                // O nome que não cabe na coluna ("Predefinição: RecordarFotos
                // P&B") aparece inteiro na dica.
                .when(passo.rotulo.nome.chars().count() > 22, |l| {
                    let nome = SharedString::from(passo.rotulo.nome.clone());
                    l.tooltip(move |window, cx| Tooltip::new(nome.clone()).build(window, cx))
                })
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .truncate()
                        .child(SharedString::from(passo.rotulo.nome.clone())),
                )
                .child(numero(passo.rotulo.variacao.as_ref(), 36.))
                .child(numero(passo.rotulo.valor.as_ref(), 44.))
        });

        // O invólucro é a janela da rolagem: do tamanho dos passos até o teto,
        // e encolhe junto com o painel.
        let altura = passos.len().min(linhas_a_vista) as f32 * ALTURA_DA_LINHA;
        div()
            .id("janela-do-historico")
            .debug_selector(|| "janela-do-historico".into())
            .flex_none()
            .h(px(altura))
            .mt(px(4.))
            .mr(px(-8.))
            .child(
                v_flex()
                    .id("lista-do-historico")
                    .pr(px(8.))
                    .children(linhas)
                    .overflow_y_scrollbar(),
            )
            .into_any_element()
    }
}
