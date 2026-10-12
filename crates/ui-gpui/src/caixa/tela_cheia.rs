//! 🖥️ O caixa da galeria **em tela cheia**: o cupom, a foto em foco, a venda
//! (pessoas, desconto e pagamento), as vendas da sessão e a gaveta
//! — tudo à vista, sem diálogo por cima (dono, 12/out/2026: *"uma visão
//! estendida mais completa e sem muitos popups, opcional"*; *"que preencha toda
//! a tela"*).
//!
//! # O mesmo caixa, noutro desenho
//!
//! É o terceiro estado do painel (minimizado · compacto · tela cheia), ligado
//! pelo `F10` e pelo botão da barra, e lembrado por máquina. A conta, as
//! gravações e as teclas são as do painel.
//!
//! # Os formulários ficam montados juntos
//!
//! No painel cada formulário é um diálogo, um de cada vez, em
//! [`Caixa::dialogo`]. Aqui vários estão na tela ao mesmo tempo. A lógica deles
//! não foi duplicada: [`Caixa::dialogo`] passa a ser **o formulário da vez** —
//! o que recebeu o último gesto — e os outros esperam em
//! [`Caixa::estacionados`]. Quem troca a vez é [`Caixa::ativar`]:
//!
//! - o clique numa região, na captura, antes de qualquer botão dela;
//! - a tecla, no interceptador do painel, pelo contexto de quem tem o foco;
//! - a resposta da rede, pelo rótulo da gravação.
//!
//! Para desenhar, cada região toma a vez só enquanto monta o corpo dela
//! ([`Caixa::regiao`]) e devolve.
//!
//! 🚨 Um formulário que "fecha" (a venda concluída, o `Esc`) some, e o desenho
//! seguinte monta outro em branco — com chave de idempotência nova, como o
//! diálogo reaberto.

use biblioteca_core::caixa::{self as regras, Desfecho};
use domain::services::pos_venda::EstadoDaFotoNoSite;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon};
use gpui_kit::{
    div, img, prelude::*, px, AnyElement, ClickEvent, Context, Div, Entity, FocusHandle,
    FontWeight, KeyContext, MouseDownEvent, ObjectFit, SharedString, Window,
};

use super::dialogos::{Dialogo, TipoDeDialogo};
use super::flutuante::{guardar, CUPOM};
use super::tela::{Caixa, TipoDeRecado, DIALOGO};
use crate::recursos::Icone;
use crate::sessoes::detalhe::{Detalhe, FotoParaOCaixa};

/// As regiões da tela cheia, pelo contexto do formulário de cada uma
/// ([`Dialogo::contexto`]). "Vendas" não tem formulário: é a lista.
pub(super) const CONTEXTOS: [&str; 8] = [
    "Pagamento",
    "Pessoas",
    "Movimento",
    "Abrir",
    "Estorno",
    "Fechamento",
    "Negociacao",
    "Vendas",
];

/// Abaixo disto as três colunas não cabem: vale o painel compacto.
pub(super) const LARGURA_MINIMA: f32 = 960.;
/// A faixa das guias e dos controles da janela fica de fora.
const TOPO: f32 = 36.;

impl Caixa {
    /// O caixa está em tela cheia agora — e não só escolhido assim.
    pub(super) fn em_tela_cheia(&self) -> bool {
        self.painel_ref()
            .is_some_and(|p| p.tela_cheia && !p.minimizado && p.cabe)
    }

    /// `F10` e o botão da barra: entra na tela cheia e sai dela.
    ///
    /// 🔑 **A volta à galeria não custa nada**: o pagamento lançado, o motivo
    /// da sangria e o resto ficam guardados e reaparecem no `F10` seguinte.
    /// Sem isso, ir à galeria no meio da venda seria perder o que foi feito.
    pub fn alternar_tela_cheia(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.estacionar_tudo();
        let Some(p) = self.painel_mut() else {
            return;
        };
        p.tela_cheia = !p.tela_cheia;
        p.minimizado = false;
        p.editando = None;
        guardar(p);
        self.focar_o_cupom(window, cx);
        cx.notify();
    }

    pub fn tela_cheia(&self) -> bool {
        self.em_tela_cheia()
    }

    /// Saindo da tela cheia, os formulários dela esperam a volta: o pagamento
    /// pela metade não vira um diálogo aberto no painel, e também não se
    /// perde. O que é por cima de tudo (os atalhos) fecha.
    pub(super) fn estacionar_tudo(&mut self) {
        if let Some(da_vez) = self.dialogo.take() {
            self.estacionados.push(da_vez);
        }
        self.estacionados
            .retain(|d| !matches!(d, Dialogo::Atalhos | Dialogo::Vendas | Dialogo::Desconto(_)));
        self.depois_das_pessoas = false;
        self.foco_antes.esquecer();
    }

    // ── A galeria, sem sair do caixa ────────────────────────────────────────

    /// `P` na tela cheia: a foto em foco vai ao cupom, ou sai dele.
    pub(super) fn levar_ou_devolver_a_foto_em_foco(&mut self, cx: &mut Context<Self>) {
        let Some((detalhe, Some(foto))) = self
            .painel_ref()
            .map(|p| (p.detalhe.clone(), p.em_foco.clone()))
        else {
            return;
        };
        detalhe.update(cx, |d, cx| d.levar_ou_devolver(&foto, cx));
    }

    /// `1`–`5` na tela cheia: a nota da foto em foco, que a deixa ir ao balcão.
    pub(super) fn dar_nota_a_foto_em_foco(&mut self, nota: u8, cx: &mut Context<Self>) {
        let Some((detalhe, Some(foto))) = self
            .painel_ref()
            .map(|p| (p.detalhe.clone(), p.em_foco.clone()))
        else {
            return;
        };
        detalhe.update(cx, |d, cx| d.dar_nota_a(&foto, nota, cx));
    }

    pub(super) fn focar_o_cupom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(foco) = self.painel_ref().map(|p| p.foco_do_cupom.clone()) {
            window.focus(&foco, cx);
        }
    }

    // ── A vez ───────────────────────────────────────────────────────────────

    /// Passa a vez ao formulário deste contexto. Devolve se ele existe.
    ///
    /// Só na tela cheia: no painel o que ela guardou continua guardado, e a
    /// vez é do diálogo aberto.
    pub(super) fn ativar(&mut self, contexto: &str) -> bool {
        if !self.em_tela_cheia() {
            return self
                .dialogo
                .as_ref()
                .is_some_and(|d| d.contexto() == contexto);
        }
        if self
            .dialogo
            .as_ref()
            .is_some_and(|d| d.contexto() == contexto)
        {
            return true;
        }
        let Some(i) = self
            .estacionados
            .iter()
            .position(|d| d.contexto() == contexto)
        else {
            return false;
        };
        let novo = self.estacionados.swap_remove(i);
        if let Some(antigo) = self.dialogo.replace(novo) {
            self.estacionados.push(antigo);
        }
        true
    }

    /// Um formulário novo toma a vez. Na tela cheia o que estava nela espera.
    pub(super) fn por_dialogo(&mut self, dialogo: Dialogo) {
        if self.em_tela_cheia() {
            let contexto = dialogo.contexto();
            self.estacionados.retain(|d| d.contexto() != contexto);
            if let Some(antigo) = self.dialogo.take() {
                if antigo.contexto() != contexto {
                    self.estacionados.push(antigo);
                }
            }
        }
        self.dialogo = Some(dialogo);
    }

    fn formularios(&self) -> impl Iterator<Item = &Dialogo> {
        self.dialogo.iter().chain(self.estacionados.iter())
    }

    pub(super) fn tem_form(&self, contexto: &str) -> bool {
        self.formularios().any(|d| d.contexto() == contexto)
    }

    /// O foco do formulário da vez: o do diálogo, ou o da região dele.
    pub(super) fn foco_do_form(&self) -> FocusHandle {
        if self.em_tela_cheia() {
            if let Some(foco) = self
                .dialogo
                .as_ref()
                .and_then(|d| self.foco_da_regiao(d.contexto()))
            {
                return foco;
            }
        }
        self.foco_do_dialogo.clone()
    }

    fn foco_da_regiao(&self, contexto: &str) -> Option<FocusHandle> {
        self.painel_ref()?.focos.get(contexto).cloned()
    }

    /// O cupom não aceita tecla: no painel, com qualquer diálogo aberto; na
    /// tela cheia, só com um formulário que dependa do total parado.
    pub(super) fn cupom_parado(&self) -> bool {
        if self.em_tela_cheia() {
            self.formularios().any(Dialogo::para_o_cupom)
        } else {
            self.dialogo.is_some()
        }
    }

    /// O que ainda abre por cima na tela cheia toma a vez. Devolve se há.
    pub(super) fn ativar_o_que_ainda_e_dialogo(&mut self) -> bool {
        let Some(contexto) = self
            .formularios()
            .find(|d| d.ainda_e_dialogo())
            .map(Dialogo::contexto)
        else {
            return false;
        };
        self.ativar(contexto)
    }

    /// A escolha das pessoas vale na hora: não há "Confirmar" na tela cheia.
    pub(super) fn guardar_pessoas_da_tela_cheia(&mut self) {
        if !self.em_tela_cheia() {
            return;
        }
        let escolhidas = self.formularios().find_map(Dialogo::pessoas_escolhidas);
        if let Some(pessoas) = escolhidas {
            self.guardar_pessoas(pessoas);
        }
    }

    /// Os formulários que a tela cheia mostra sempre: as pessoas e, com o
    /// caixa aberto, o pagamento e a gaveta; fechado, o de abrir.
    pub(super) fn montar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(vista) = self.vista.as_ref() else {
            return;
        };
        let aberto = vista.caixa.is_some();
        let fora: &[&str] = if aberto {
            &["Abrir"]
        } else {
            &["Pagamento", "Movimento"]
        };
        if self
            .dialogo
            .as_ref()
            .is_some_and(|d| fora.contains(&d.contexto()))
        {
            self.dialogo = None;
        }
        self.estacionados.retain(|d| !fora.contains(&d.contexto()));
        let fixos: &[(&str, TipoDeDialogo)] = if aberto {
            &[
                ("Pessoas", TipoDeDialogo::Pessoas),
                ("Pagamento", TipoDeDialogo::Pagamento),
                ("Movimento", TipoDeDialogo::Movimento),
            ]
        } else {
            &[
                ("Pessoas", TipoDeDialogo::Pessoas),
                ("Abrir", TipoDeDialogo::Abrir),
            ]
        };
        for (contexto, tipo) in fixos {
            if !self.tem_form(contexto) {
                let (form, _) = self.novo_dialogo(*tipo, window, cx);
                self.estacionados.push(form);
            }
        }
    }

    /// Os formulários montados, em ordem — só os testes.
    #[cfg(test)]
    pub(super) fn formularios_montados(&self) -> Vec<&'static str> {
        let mut todos: Vec<_> = self.formularios().map(Dialogo::contexto).collect();
        todos.sort_unstable();
        todos
    }

    // ── As teclas F ─────────────────────────────────────────────────────────

    /// Leva o teclado a uma região: ao primeiro campo dela, ou à região.
    pub(super) fn focar_regiao(
        &mut self,
        contexto: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ativar(contexto);
        let do_form = self
            .dialogo
            .as_ref()
            .filter(|d| d.contexto() == contexto)
            .and_then(|d| d.foco_inicial(cx));
        if let Some(foco) = do_form.or_else(|| self.foco_da_regiao(contexto)) {
            window.focus(&foco, cx);
        }
        cx.notify();
    }

    /// O desfecho de uma tecla F na tela cheia: em vez de abrir o diálogo, o
    /// foco vai à região dele.
    pub(super) fn resolver_na_tela_cheia(
        &mut self,
        desfecho: Desfecho,
        seguir: TipoDeDialogo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match desfecho {
            Desfecho::Erro(frase) => self.avisar(frase, TipoDeRecado::Erro, cx),
            Desfecho::Aviso(frase) => self.avisar(frase, TipoDeRecado::Informacao, cx),
            Desfecho::AbrirCaixa => self.focar_regiao("Abrir", window, cx),
            Desfecho::PessoasAntes => {
                self.avisar(
                    "Escolha quem fotografou, quem atendeu e quem auxiliou.",
                    TipoDeRecado::Informacao,
                    cx,
                );
                self.focar_regiao("Pessoas", window, cx);
            }
            Desfecho::Seguir => match seguir {
                TipoDeDialogo::Desconto => {
                    if self.ativar("Pagamento") {
                        self.focar_o_desconto(window, cx);
                    }
                }
                // O primeiro `F4` leva ao pagamento; lá dentro, conclui.
                TipoDeDialogo::Pagamento => {
                    let ja_esta = self
                        .foco_da_regiao("Pagamento")
                        .is_some_and(|f| f.contains_focused(window, cx));
                    if ja_esta {
                        self.concluir_venda(window, cx);
                    } else {
                        self.focar_regiao("Pagamento", window, cx);
                    }
                }
                TipoDeDialogo::Movimento => self.focar_regiao("Movimento", window, cx),
                TipoDeDialogo::Abrir => self.focar_regiao("Abrir", window, cx),
                TipoDeDialogo::Fechar => {
                    if self.tem_form("Fechamento") {
                        self.focar_regiao("Fechamento", window, cx);
                    } else {
                        self.abrir_dialogo(TipoDeDialogo::Fechar, window, cx);
                    }
                }
                TipoDeDialogo::Pessoas => self.focar_regiao("Pessoas", window, cx),
                TipoDeDialogo::Vendas => self.focar_regiao("Vendas", window, cx),
                TipoDeDialogo::Atalhos => self.abrir_dialogo(TipoDeDialogo::Atalhos, window, cx),
            },
        }
    }

    // ── O desenho ───────────────────────────────────────────────────────────

    /// Uma região: a moldura com título, o contexto de teclado do formulário
    /// dela e o corpo — montado com o formulário na vez, que depois volta a
    /// quem a tinha.
    fn regiao(
        &mut self,
        contexto: &'static str,
        titulo: &str,
        tecla: Option<&'static str>,
        window: &mut Window,
        cx: &mut Context<Self>,
        corpo: impl FnOnce(&mut Self, &mut Context<Self>) -> AnyElement,
    ) -> AnyElement {
        let antes = self.dialogo.as_ref().map(Dialogo::contexto);
        let tomou = self.ativar(contexto);
        let miolo = corpo(self, cx);
        match antes {
            Some(antes) => {
                self.ativar(antes);
            }
            None if tomou => {
                if let Some(form) = self.dialogo.take() {
                    self.estacionados.push(form);
                }
            }
            None => {}
        }

        let foco = self
            .foco_da_regiao(contexto)
            .unwrap_or_else(|| self.foco_do_dialogo.clone());
        let com_foco = foco.contains_focused(window, cx);
        // 🚪 O que toma o lugar da foto só enquanto dura (estorno, fechamento,
        // negociação) tem a saída à vista — e não só no `Esc`.
        let voltar = matches!(contexto, "Estorno" | "Fechamento" | "Negociacao").then(|| {
            crate::estilo::botao_fantasma_pequeno(
                SharedString::from(format!("caixa-cheia-voltar-{contexto}")),
                cx,
            )
            .child("Voltar")
            .child(crate::estilo::tecla("Esc"))
            .on_click(cx.listener(move |tela, _: &ClickEvent, window, cx| {
                tela.ativar(contexto);
                tela.fechar_dialogo(window, cx);
            }))
        });
        let tema = cx.theme();
        let (borda, apagado, aceso) = (tema.border, tema.muted_foreground, tema.ring);
        let chave =
            KeyContext::parse(&format!("{DIALOGO} {contexto} Flutuante")).unwrap_or_default();
        v_flex()
            .id(SharedString::from(format!("caixa-regiao-{contexto}")))
            .key_context(chave)
            .track_focus(&foco)
            .map(|d| Self::acoes_do_form(d, cx))
            .capture_any_mouse_down(cx.listener(move |tela, _: &MouseDownEvent, _, _| {
                tela.ativar(contexto);
            }))
            .size_full()
            .min_h(px(0.))
            .border_1()
            .border_color(if com_foco { aceso } else { borda })
            .rounded(crate::tema::canto(10.))
            .overflow_hidden()
            .child(
                h_flex()
                    .flex_none()
                    .justify_between()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(borda)
                    .text_size(crate::tema::letra::em(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(apagado)
                    .child(titulo.to_uppercase())
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .children(voltar)
                            .children(tecla.map(crate::estilo::tecla)),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(format!(
                        "caixa-regiao-{contexto}-rolagem"
                    )))
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p(px(12.))
                    .child(miolo),
            )
            .into_any_element()
    }

    /// A moldura de uma coluna que não é formulário.
    fn moldura(titulo: &str, direita: Option<AnyElement>, cx: &Context<Self>) -> Div {
        let tema = cx.theme();
        let (borda, apagado) = (tema.border, tema.muted_foreground);
        v_flex()
            .size_full()
            .min_h(px(0.))
            .border_1()
            .border_color(borda)
            .rounded(crate::tema::canto(10.))
            .overflow_hidden()
            .child(
                h_flex()
                    .flex_none()
                    .justify_between()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(borda)
                    .text_size(crate::tema::letra::em(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(apagado)
                    .child(titulo.to_uppercase())
                    .children(direita),
            )
    }

    pub(super) fn render_tela_cheia(
        &mut self,
        janela: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.montar(window, cx);
        if std::mem::take(&mut self.foco_no_cupom_depois) {
            self.focar_o_cupom(window, cx);
        }
        // O ajuste do item acompanha o cursor do cupom, sempre aberto.
        let cupom = self.cupom();
        // Com o cursor numa foto que não está no cupom (escolhida na fila da
        // sessão), o ajuste fecha: o que se oferece ali é pô-la no cupom.
        let em_foco_na_galeria = self.painel_ref().and_then(|p| p.em_foco.clone());
        let alvo = match (self.foco_no_cupom(), em_foco_na_galeria) {
            (Some(no_cupom), _) => Some(no_cupom),
            (None, Some(_)) => None,
            (None, None) => cupom.itens.first().map(|i| i.foto_id.clone()),
        };
        let editado = self
            .painel_ref()
            .and_then(|p| p.editando_em())
            .map(str::to_string);
        match (&alvo, &editado) {
            (Some(alvo), editado) if editado.as_ref() != Some(alvo) => {
                self.nova_edicao(alvo.clone(), window, cx)
            }
            (None, Some(_)) => {
                if let Some(p) = self.painel_mut() {
                    p.editando = None;
                }
            }
            _ => {}
        }

        let tema = cx.theme();
        let (fundo, frente, borda, apagado, realce) = (
            tema.background,
            tema.foreground,
            tema.border,
            tema.muted_foreground,
            tema.muted,
        );
        let mono = tema.mono_font_family.clone();
        let (largura, altura) = (janela.0, janela.1 - TOPO);
        let lado_do_cupom = (largura * 0.27).clamp(320., 460.);
        let lado_da_venda = (largura * 0.31).clamp(400., 560.);
        let faixa_de_baixo = (altura * 0.3).clamp(230., 320.);
        let lado_da_gaveta = (largura * 0.24).clamp(300., 420.);
        let aberto = self.vista.as_ref().is_some_and(|v| v.caixa.is_some());
        let n = cupom.itens.len();

        // ── A barra ──
        let (situacao, cor) = self.situacao_do_painel(cx);
        let botao = |id: &'static str, icone: Icone, rotulo: &'static str, tecla: &'static str| {
            crate::estilo::botao_fantasma_pequeno(id, cx)
                .child(Icon::new(icone).size(px(14.)))
                .child(rotulo)
                .child(crate::estilo::tecla(tecla))
        };
        let barra = h_flex()
            .flex_none()
            .gap(px(8.))
            .px(px(16.))
            .py(px(8.))
            .border_b_1()
            .border_color(borda)
            .bg(realce.opacity(0.6))
            .child(Icon::new(Icone::ShoppingCart).size(px(16.)))
            .child(
                div()
                    .font_family(mono.clone())
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("CAIXA"),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .text_xs()
                    .text_color(apagado)
                    .child(div().size(px(8.)).rounded_full().bg(cor))
                    .child(situacao),
            )
            .child(div().flex_1())
            .child(
                botao("caixa-cheia-atalhos", Icone::Keyboard, "Atalhos", "F1").on_click(
                    cx.listener(|t, _: &ClickEvent, w, cx| {
                        t.abrir_dialogo(TipoDeDialogo::Atalhos, w, cx)
                    }),
                ),
            )
            .child(
                botao("caixa-cheia-minimizar", Icone::Minus, "Minimizar", "F9")
                    .on_click(cx.listener(|t, _: &ClickEvent, _, cx| t.alternar_painel(cx))),
            )
            .child(
                botao(
                    "caixa-cheia-voltar",
                    Icone::Minimize2,
                    "Voltar à galeria",
                    "F10",
                )
                .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.alternar_tela_cheia(w, cx))),
            );

        // ── O cupom ──
        let em_foco = self.painel_ref().and_then(|p| p.em_foco.clone());
        let parado = self.cupom_parado();
        let cabecalho_da_selecao = (n > 0).then(|| self.cabecalho_da_selecao(&cupom, cx));
        let lista = self.itens_do_cupom(&cupom, em_foco.as_deref(), cx);
        let (resumo, total) = self.resumo_e_total(&cupom, cx);
        let foco_do_cupom = self
            .painel_ref()
            .map(|p| p.foco_do_cupom.clone())
            .unwrap_or_else(|| self.foco.clone());
        let coluna_do_cupom = Self::moldura(&format!("Cupom · {}", regras::itens(n)), None, cx)
            .w(px(lado_do_cupom))
            .flex_none()
            .when(parado, |d| {
                d.child(
                    div()
                        .flex_none()
                        .px(px(12.))
                        .py(px(6.))
                        .border_b_1()
                        .border_color(borda)
                        .text_xs()
                        .text_color(apagado)
                        .child("Em espera: termine ou feche (Esc) o que está aberto ao lado."),
                )
            })
            .children(cabecalho_da_selecao)
            .child(div().flex_1().min_h(px(0.)).flex().flex_col().child(lista))
            .child(div().h(px(1.)).bg(borda))
            .child(resumo)
            .child(total);

        // ── O palco: a foto em foco, ou o formulário que pede lugar ──
        let palco = if self.tem_form("Fechamento") {
            let titulo = if aberto {
                "Fechar o caixa"
            } else {
                "Caixa fechado"
            };
            self.regiao("Fechamento", titulo, Some("F8"), window, cx, |t, cx| {
                t.corpo_fechamento(cx)
            })
        } else if self.tem_form("Estorno") {
            self.regiao("Estorno", "Estornar a venda", None, window, cx, |t, cx| {
                t.corpo_estorno(cx)
            })
        } else if self.tem_form("Negociacao") {
            self.regiao(
                "Negociacao",
                "Negociação",
                Some("N"),
                window,
                cx,
                |t, cx| t.corpo_negociacao(cx),
            )
        } else {
            // Abaixo de ~1000 px de altura o ajuste do item ocupa o palco.
            self.foto_em_foco(&cupom, altura >= 960., cx)
        };

        // ── A venda ──
        let pessoas = self.regiao(
            "Pessoas",
            "Quem fotografou, atendeu e auxiliou",
            Some("F3"),
            window,
            cx,
            |t, cx| t.corpo_pessoas(cx),
        );
        let (sem_estudio, indisponivel) = self
            .vista
            .as_ref()
            .map_or((false, false), |v| (v.estudio_id.is_none(), v.indisponivel));
        let pagamento = if sem_estudio {
            self.escolha_do_estudio(cx)
        } else if indisponivel {
            self.sem_resposta_do_caixa(cx)
        } else if aberto {
            self.regiao("Pagamento", "Pagamento", Some("F4"), window, cx, |t, cx| {
                t.corpo_pagamento(cx)
            })
        } else {
            self.regiao("Abrir", "Abrir o caixa", Some("F8"), window, cx, |t, cx| {
                t.corpo_abrir(cx)
            })
        };
        let coluna_da_venda = v_flex()
            .w(px(lado_da_venda))
            .flex_none()
            .min_h(px(0.))
            .gap(px(12.))
            .child(div().flex_none().child(pessoas))
            .child(div().flex_1().min_h(px(0.)).child(pagamento));

        // ── A faixa de baixo: vendas, movimento e gaveta ──
        let vendas = self.regiao(
            "Vendas",
            "Vendas da sessão",
            Some("F7"),
            window,
            cx,
            |t, cx| t.corpo_vendas(cx),
        );
        let gaveta = if aberto {
            self.regiao(
                "Movimento",
                "Sangria ou suprimento",
                Some("F6"),
                window,
                cx,
                |t, cx| t.corpo_movimento(cx),
            )
        } else {
            Self::moldura("Gaveta", None, cx)
                .child(
                    div()
                        .p(px(12.))
                        .text_color(apagado)
                        .child("Caixa fechado. Abra ao lado (F8) para vender e registrar sangria."),
                )
                .into_any_element()
        };
        let fechar = crate::estilo::botao_contorno_pequeno("caixa-cheia-f8", cx)
            .child(if aberto {
                "Fechar caixa"
            } else {
                "Abrir caixa"
            })
            .child(crate::estilo::tecla("F8"))
            .on_click(cx.listener(|t, _: &ClickEvent, w, cx| t.abrir_ou_fechar(w, cx)));
        // As vendas (com o movimento do caixa do estúdio) sob o cupom, e a
        // gaveta sob o palco: a coluna da venda fica com a altura toda.
        let embaixo = h_flex()
            .flex_none()
            .h(px(faixa_de_baixo))
            .items_stretch()
            .gap(px(12.))
            .child(div().flex_1().min_w(px(0.)).child(vendas))
            .child(
                v_flex()
                    .w(px(lado_da_gaveta))
                    .flex_none()
                    .gap(px(8.))
                    .child(div().flex_1().min_h(px(0.)).child(gaveta))
                    .child(h_flex().flex_none().justify_end().child(fechar)),
            );

        let dialogo = self.render_dialogo(window, cx);
        let recados = self.render_recados(window, cx);
        let rodape = crate::app::rodape::ALTURA_DO_RODAPE;
        let tela = v_flex()
            .id("caixa-flutuante")
            .key_context(CUPOM)
            .track_focus(&foco_do_cupom)
            .occlude()
            .absolute()
            .top(px(TOPO))
            .left_0()
            .right_0()
            .bottom(px(rodape))
            .bg(fundo)
            .text_color(frente)
            .text_sm()
            .child(barra)
            .child(
                h_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .items_stretch()
                    .p(px(12.))
                    .gap(px(12.))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .min_h(px(0.))
                            .gap(px(12.))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_h(px(0.))
                                    .items_stretch()
                                    .gap(px(12.))
                                    .child(coluna_do_cupom)
                                    .child(div().flex_1().min_w(px(0.)).min_h(px(0.)).child(palco)),
                            )
                            .child(embaixo),
                    )
                    .child(coluna_da_venda),
            );
        div()
            .absolute()
            .inset_0()
            .child(tela)
            .children(dialogo)
            .children(recados)
            .into_any_element()
    }

    /// O palco: a fila das fotos da sessão, a foto em foco e o que se faz com
    /// ela — o ajuste do item que está no cupom, ou pôr no cupom a que não está.
    ///
    /// 🔑 **Nada aqui manda o balconista de volta à galeria**: sinalizar mais
    /// uma foto, tirar uma do cupom, dar a nota que falta e tirar a rejeição
    /// são os mesmos gestos da grade (`P`, `1`–`5`, `X`), feitos daqui.
    fn foto_em_foco(
        &mut self,
        cupom: &regras::Cupom,
        alta: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme();
        let (apagado, borda, aceso, realce) =
            (tema.muted_foreground, tema.border, tema.ring, tema.muted);
        let mono = tema.mono_font_family.clone();
        let Some((detalhe, em_foco, foco_do_cupom)) = self.painel_ref().map(|p| {
            (
                p.detalhe.clone(),
                p.em_foco.clone(),
                p.foco_do_cupom.clone(),
            )
        }) else {
            return div().into_any_element();
        };
        let fotos = detalhe.read(cx).fotos_para_o_caixa();
        let no_cupom = |id: &str| cupom.itens.iter().any(|i| i.foto_id == id);
        let item = self.item_editado();
        // A foto que o palco mostra: o item em ajuste, ou a que o balconista
        // escolheu na fila e ainda não está no cupom.
        let fora = em_foco
            .as_deref()
            .filter(|id| !no_cupom(id))
            .and_then(|id| fotos.iter().find(|f| f.id == id))
            .cloned();
        let id_da_vez = item
            .as_ref()
            .map(|i| i.foto_id.clone())
            .or_else(|| fora.as_ref().map(|f| f.id.clone()));
        let imagem = id_da_vez
            .as_deref()
            .and_then(|id| detalhe.read(cx).imagem_vista(id));
        let titulo = match (&item, &fora) {
            (Some(i), _) => format!("{} · {}", i.codigo(), i.arquivo),
            (None, Some(f)) => f.arquivo.clone(),
            (None, None) => "Fotos da sessão".to_string(),
        };
        let valor = item.as_ref().map(|i| {
            div()
                .font_family(mono)
                .child(biblioteca_core::dinheiro::formatar(i.cobrado))
                .into_any_element()
        });

        // ── A fila: todas as fotos da sessão, com o que cada uma é ──
        let fila = h_flex()
            .id("caixa-cheia-fila")
            .flex_none()
            .gap(px(6.))
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(borda)
            .overflow_x_scroll()
            .children(fotos.iter().map(|f| {
                let id_do_clique = f.id.clone();
                let da_vez = id_da_vez.as_deref() == Some(f.id.as_str());
                let esta_no_cupom = no_cupom(&f.id);
                let vendida = !esta_no_cupom && f.estado != EstadoDaFotoNoSite::Disponivel;
                let miniatura = detalhe.read(cx).imagem_vista(&f.id);
                let (detalhe, foco) = (detalhe.clone(), foco_do_cupom.clone());
                let dica = format!(
                    "{} — {}",
                    f.arquivo,
                    if esta_no_cupom {
                        "no cupom"
                    } else if vendida {
                        "já vendida"
                    } else if f.rejeitada {
                        "rejeitada"
                    } else {
                        "à venda"
                    }
                );
                div()
                    .id(SharedString::from(format!("caixa-cheia-foto-{}", f.id)))
                    .debug_selector(|| format!("caixa-cheia-foto-{}", f.id))
                    .relative()
                    .flex_none()
                    .size(px(56.))
                    .rounded(crate::tema::canto(6.))
                    .overflow_hidden()
                    .border_2()
                    .border_color(if da_vez { aceso } else { borda })
                    .bg(realce)
                    .cursor_pointer()
                    .when(vendida || f.rejeitada, |d| d.opacity(0.45))
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(dica.clone()).build(window, cx)
                    })
                    .children(
                        miniatura
                            .map(|imagem| img(imagem).size_full().object_fit(ObjectFit::Cover)),
                    )
                    .when(esta_no_cupom, |d| {
                        d.child(
                            div()
                                .absolute()
                                .right(px(2.))
                                .bottom(px(2.))
                                .size(px(16.))
                                .rounded_full()
                                .bg(crate::tema::cores::aceso())
                                .text_color(crate::tema::cores::sobre_aceso())
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(Icone::Check).size(px(11.))),
                        )
                    })
                    .on_click(move |_: &ClickEvent, window, cx| {
                        detalhe.update(cx, |d, cx| {
                            d.so_esta(&id_do_clique, cx);
                        });
                        // O teclado fica no cupom: `P` e a nota valem em seguida.
                        window.focus(&foco, cx);
                    })
            }));

        let quadro = div()
            .flex_1()
            .min_h(px(0.))
            .p(px(12.))
            .flex()
            .items_center()
            .justify_center()
            .child(match imagem {
                // `max_*`, e não `size_full`: o `Img` impõe a proporção da foto,
                // e com o tamanho cheio o retrato passaria da moldura
                // (`imagem::a_moldura_manda`).
                Some(imagem) => img(imagem)
                    .max_w_full()
                    .max_h_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                None => v_flex()
                    .items_center()
                    .gap(px(8.))
                    .text_color(apagado)
                    .child(Icon::new(Icone::Camera).size(px(32.)))
                    .child(if id_da_vez.is_some() {
                        "A foto aparece quando passar pela grade."
                    } else if fotos.is_empty() {
                        "A sessão ainda não tem fotos no site."
                    } else {
                        "Clique numa foto da fila acima para pô-la no cupom."
                    })
                    .into_any_element(),
            });

        // ── O que se faz com a foto da vez ──
        let acoes: Option<AnyElement> = match (&item, &fora) {
            (Some(i), _) => {
                let id = i.foto_id.clone();
                let para_tirar = detalhe.clone();
                Some(
                    v_flex()
                        .flex_none()
                        .child(self.edicao_rapida(i, cx))
                        .child(
                            h_flex().justify_end().px(px(12.)).pb(px(8.)).child(
                                crate::estilo::botao_contorno_pequeno(
                                    "caixa-cheia-tirar-do-cupom",
                                    cx,
                                )
                                .child("Tirar do cupom")
                                .child(crate::estilo::tecla("P"))
                                .on_click(
                                    move |_: &ClickEvent, _, cx| {
                                        para_tirar.update(cx, |d, cx| d.levar_ou_devolver(&id, cx));
                                    },
                                ),
                            ),
                        )
                        .into_any_element(),
                )
            }
            (None, Some(f)) => Some(Self::acoes_da_foto_fora_do_cupom(f, &detalhe, cx)),
            (None, None) => None,
        };
        // Numa janela baixa o ajuste do item não deixa altura para a foto
        // grande: sobraria um selo. A foto já está na fila (acesa) e na linha
        // do cupom, e o lugar fica para o ajuste.
        let mostra_a_foto = alta || item.is_none();
        Self::moldura(&titulo, valor, cx)
            .child(fila)
            .map(|d| {
                if mostra_a_foto {
                    d.child(quadro)
                } else {
                    d.child(div().flex_1().min_h(px(0.)))
                }
            })
            .children(acoes)
            .into_any_element()
    }

    /// A foto escolhida na fila que não está no cupom: a nota, e pô-la nele.
    fn acoes_da_foto_fora_do_cupom(
        foto: &FotoParaOCaixa,
        detalhe: &Entity<Detalhe>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tema = cx.theme();
        let (apagado, borda) = (tema.muted_foreground, tema.border);
        let vendida = foto.estado != EstadoDaFotoNoSite::Disponivel;
        let caixa = v_flex()
            .flex_none()
            .gap(px(8.))
            .px(px(12.))
            .py(px(10.))
            .border_t_1()
            .border_color(borda);
        if vendida {
            return caixa
                .child("Esta foto já foi vendida.")
                .child(
                    div()
                        .text_xs()
                        .text_color(apagado)
                        .child("Para desfazer, estorne a venda dela em Vendas da sessão (F7)."),
                )
                .into_any_element();
        }
        if foto.rejeitada {
            let (id, d) = (foto.id.clone(), detalhe.clone());
            return caixa
                .child("Foto rejeitada: não vai ao balcão enquanto estiver assim.")
                .child(
                    h_flex().child(
                        crate::estilo::botao_contorno_pequeno("caixa-cheia-tirar-rejeicao", cx)
                            .child("Tirar a rejeição")
                            .on_click(move |_: &ClickEvent, _, cx| {
                                d.update(cx, |d, cx| d.tirar_a_rejeicao_de(&id, cx));
                            }),
                    ),
                )
                .into_any_element();
        }
        let nota = foto.nota.unwrap_or(0);
        let estrelas = h_flex().gap(px(2.)).children((1..=5u8).map(|n| {
            let (id, d) = (foto.id.clone(), detalhe.clone());
            crate::estilo::botao_fantasma_pequeno(
                SharedString::from(format!("caixa-cheia-nota-{n}")),
                cx,
            )
            .child(
                div()
                    .text_size(crate::tema::letra::em(16.))
                    .text_color(if n <= nota {
                        crate::tema::cores::nota()
                    } else {
                        apagado
                    })
                    .child("★"),
            )
            .on_click(move |_: &ClickEvent, _, cx| {
                d.update(cx, |d, cx| d.dar_nota_a(&id, n, cx));
            })
        }));
        let (id, d) = (foto.id.clone(), detalhe.clone());
        caixa
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(div().text_color(apagado).child("Nota"))
                    .child(estrelas)
                    .child(crate::estilo::tecla("1"))
                    .child("–")
                    .child(crate::estilo::tecla("5")),
            )
            .when(foto.nota.is_none(), |c| {
                c.child(
                    div()
                        .text_xs()
                        .text_color(apagado)
                        .child("Dê a nota primeiro: sem ela a foto não vai ao balcão."),
                )
            })
            .child(
                h_flex().child(
                    crate::estilo::botao_primario_pequeno("caixa-cheia-por-no-cupom", cx)
                        .child("Pôr no cupom")
                        .child(crate::estilo::tecla("P"))
                        .on_click(move |_: &ClickEvent, _, cx| {
                            d.update(cx, |d, cx| d.levar_ou_devolver(&id, cx));
                        }),
                ),
            )
            .into_any_element()
    }

    /// 🏢 A sessão sem estúdio: o caixa é do estúdio, e ele se escolhe aqui —
    /// o cabeçalho da galeria, onde fica o seletor, está coberto.
    fn escolha_do_estudio(&self, cx: &mut Context<Self>) -> AnyElement {
        let apagado = cx.theme().muted_foreground;
        let Some(detalhe) = self.painel_ref().map(|p| p.detalhe.clone()) else {
            return div().into_any_element();
        };
        let estudios = detalhe.read(cx).estudios_para_escolher();
        let lista: Vec<AnyElement> = estudios
            .into_iter()
            .map(|(id, nome)| {
                let d = detalhe.clone();
                crate::estilo::botao_contorno(
                    SharedString::from(format!("caixa-cheia-estudio-{id}")),
                    cx,
                )
                .w_full()
                .child(nome)
                .on_click(move |_: &ClickEvent, _, cx| {
                    d.update(cx, |d, cx| d.escolher_estudio(id.clone(), cx));
                })
                .into_any_element()
            })
            .collect();
        let vazia = lista.is_empty();
        Self::moldura("Estúdio da sessão", None, cx)
            .child(
                v_flex()
                    .gap(px(8.))
                    .p(px(12.))
                    .child("Esta sessão ainda não tem estúdio, e o caixa é do estúdio.")
                    .child(div().text_xs().text_color(apagado).child(if vazia {
                        "Nenhum estúdio no seu perfil de acesso: peça a quem administra a conta."
                    } else {
                        "Escolha o estúdio desta sessão para vender:"
                    }))
                    .children(lista),
            )
            .into_any_element()
    }

    /// 📡 A API não respondeu: tentar de novo é daqui mesmo.
    fn sem_resposta_do_caixa(&self, cx: &mut Context<Self>) -> AnyElement {
        let apagado = cx.theme().muted_foreground;
        Self::moldura("Caixa", None, cx)
            .child(
                v_flex()
                    .gap(px(8.))
                    .p(px(12.))
                    .child("O caixa não respondeu.")
                    .child(
                        div()
                            .text_xs()
                            .text_color(apagado)
                            .child("Sem a resposta não dá para saber se ele está aberto."),
                    )
                    .child(
                        h_flex().child(
                            crate::estilo::botao_primario_pequeno("caixa-cheia-tentar", cx)
                                .child("Tentar de novo")
                                .on_click(cx.listener(|t, _: &ClickEvent, _, cx| t.recarregar(cx))),
                        ),
                    ),
            )
            .into_any_element()
    }
}
